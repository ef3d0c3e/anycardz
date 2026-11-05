//! Card catalog, loaded once at startup from the data repository (see its README).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::i18n::Lang;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rarity {
    Common,
    Rare,
    Legendary,
}

impl Rarity {
    pub const ALL: [Rarity; 3] = [Rarity::Common, Rarity::Rare, Rarity::Legendary];
}

/// Messages of one .ftl file
#[derive(Debug, Clone, Default)]
pub struct Messages(HashMap<String, String>);

impl Messages {
    pub fn get(&self, id: &str) -> Option<&str> {
        self.0.get(id).map(String::as_str)
    }
}

/// Texts in every language, English is always there
#[derive(Debug, Clone)]
pub struct Texts {
    en: Messages,
    fr: Messages,
}

impl Texts {
    /// Falls back to English, then to an empty string
    pub fn get(&self, lang: Lang, id: &str) -> &str {
        let wanted = match lang {
            Lang::En => &self.en,
            Lang::Fr => &self.fr,
        };
        wanted.get(id).or_else(|| self.en.get(id)).unwrap_or("")
    }
}

#[derive(Debug, Clone)]
pub struct Card {
    pub slug: String,
    pub number: u32,
    pub rarity: Rarity,
    pub tags: Vec<String>,
    pub primary: String,
    pub secondary: String,
    pub texts: Texts,
    pub artwork: PathBuf,
    /// Collections holding the card, in load order
    pub collections: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Collection {
    pub slug: String,
    pub number: u32,
    pub tags: Vec<String>,
    pub primary: String,
    pub secondary: String,
    pub texts: Texts,
    /// Card slugs, in the order of collection.toml
    pub cards: Vec<String>,
    /// Card shown on the booster: the first legendary, else the first card
    pub featured: String,
}

#[derive(Debug)]
pub struct Catalog {
    pub collections: Vec<Collection>,
    pub cards: HashMap<String, Card>,
    pub assets_dir: PathBuf,
    collection_index: HashMap<String, usize>,
}

impl Catalog {
    pub fn collection(&self, slug: &str) -> Option<&Collection> {
        self.collection_index.get(slug).map(|&i| &self.collections[i])
    }

    pub fn card(&self, slug: &str) -> Option<&Card> {
        self.cards.get(slug)
    }

    /// Cards of a collection grouped by rarity, for drawing packs
    pub fn pool(&self, collection: &Collection) -> HashMap<Rarity, Vec<&Card>> {
        let mut pool: HashMap<Rarity, Vec<&Card>> = HashMap::new();
        for slug in &collection.cards {
            let card = &self.cards[slug];
            pool.entry(card.rarity).or_default().push(card);
        }
        pool
    }

    /// Loads the whole data directory. Any broken file fails the load: better no
    /// server than cards that vanish from players' binders.
    pub fn load(dir: &Path, dev_collections: bool) -> anyhow::Result<Catalog> {
        let mut collections = Vec::new();
        for entry in sorted_dirs(&dir.join("collections"))? {
            let slug = file_name(&entry);
            let collection = load_collection(&entry, &slug).with_context(|| format!("collection {slug}"))?;
            if collection.tags.iter().any(|t| t == "dev") && !dev_collections {
                continue;
            }
            collections.push(collection);
        }
        collections.sort_by_key(|c| c.number);

        let mut cards = HashMap::new();
        for collection in &mut collections {
            for slug in &collection.cards {
                if !cards.contains_key(slug) {
                    let card = load_card(&dir.join("cards").join(slug), slug, collection)
                        .with_context(|| format!("card {slug} of collection {}", collection.slug))?;
                    cards.insert(slug.clone(), card);
                }
                cards.get_mut(slug).unwrap().collections.push(collection.slug.clone());
            }
            collection.featured = collection
                .cards
                .iter()
                .find(|s| cards[*s].rarity == Rarity::Legendary)
                .or(collection.cards.first())
                .cloned()
                .ok_or_else(|| anyhow!("collection {} has no cards", collection.slug))?;
        }

        let collection_index = collections.iter().enumerate().map(|(i, c)| (c.slug.clone(), i)).collect();
        Ok(Catalog { collections, cards, assets_dir: dir.join("assets"), collection_index })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CollectionToml {
    id: u32,
    #[serde(default)]
    tags: Vec<String>,
    color_primary: String,
    color_secondary: String,
    #[serde(default)]
    cards: Vec<CardRef>,
}

#[derive(Deserialize)]
struct CardRef {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CardToml {
    id: u32,
    rarity: Rarity,
    #[serde(default)]
    tags: Vec<String>,
    color_primary: Option<String>,
    color_secondary: Option<String>,
}

fn load_collection(dir: &Path, slug: &str) -> anyhow::Result<Collection> {
    let toml: CollectionToml = toml::from_str(&read(&dir.join("collection.toml"))?)?;
    let texts = load_texts(dir)?;
    Ok(Collection {
        slug: slug.to_owned(),
        number: toml.id,
        tags: toml.tags,
        primary: toml.color_primary,
        secondary: toml.color_secondary,
        texts,
        cards: toml.cards.into_iter().map(|c| c.name).collect(),
        featured: String::new(),
    })
}

fn load_card(dir: &Path, slug: &str, collection: &Collection) -> anyhow::Result<Card> {
    let toml: CardToml = toml::from_str(&read(&dir.join("card.toml"))?)?;
    let texts = load_texts(dir)?;
    let artwork = ["jpg", "JPG", "jpeg", "png", "webp"]
        .iter()
        .map(|ext| dir.join(format!("artwork.{ext}")))
        .find(|p| p.is_file())
        .ok_or_else(|| anyhow!("no artwork"))?;
    Ok(Card {
        slug: slug.to_owned(),
        number: toml.id,
        rarity: toml.rarity,
        tags: toml.tags,
        primary: toml.color_primary.unwrap_or_else(|| collection.primary.clone()),
        secondary: toml.color_secondary.unwrap_or_else(|| collection.secondary.clone()),
        texts,
        artwork,
        collections: Vec::new(),
    })
}

fn load_texts(dir: &Path) -> anyhow::Result<Texts> {
    let en = parse_ftl(&read(&dir.join("en.ftl"))?);
    if en.get("title").is_none() {
        bail!("en.ftl has no title");
    }
    let fr_path = dir.join("fr.ftl");
    let fr = if fr_path.is_file() { parse_ftl(&read(&fr_path)?) } else { Messages::default() };
    Ok(Texts { en, fr })
}

/// The subset of Fluent the data uses: `id = value`, indented continuation
/// lines, `#` comments
pub fn parse_ftl(source: &str) -> Messages {
    let mut messages = HashMap::new();
    let mut current: Option<String> = None;
    for line in source.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            if let Some(id) = &current {
                let value: &mut String = messages.get_mut(id).unwrap();
                if !value.is_empty() {
                    value.push(' ');
                }
                value.push_str(line.trim());
            }
            continue;
        }
        if let Some((id, value)) = line.split_once('=') {
            let id = id.trim().to_owned();
            messages.insert(id.clone(), value.trim().to_owned());
            current = Some(id);
        }
    }
    Messages(messages)
}

fn read(path: &Path) -> anyhow::Result<String> {
    fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

fn sorted_dirs(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    Ok(dirs)
}

fn file_name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ftl_lines_and_continuations() {
        let m = parse_ftl("# comment\ntitle = AK-47\ndescription = First line\n    second line\n\nempty =\n");
        assert_eq!(m.get("title"), Some("AK-47"));
        assert_eq!(m.get("description"), Some("First line second line"));
        assert_eq!(m.get("empty"), Some(""));
    }

    #[test]
    fn loads_fixture_data() {
        let cat = Catalog::load(&crate::test_support::fixture_dir(), false).unwrap();
        assert_eq!(cat.collections.len(), 1, "dev collection skipped");
        let c = cat.collection("minerals").unwrap();
        assert_eq!(c.cards.len(), 6);
        assert_eq!(c.featured, "diamond");
        let quartz = cat.card("quartz").unwrap();
        assert_eq!(quartz.texts.get(Lang::Fr, "title"), "Quartz (fr)");
        // No French description: English instead
        assert_eq!(quartz.texts.get(Lang::Fr, "description"), "A common mineral.");
        assert_eq!(cat.card("diamond").unwrap().primary, "#ffffff", "card color override");
        assert_eq!(cat.pool(c)[&Rarity::Rare].len(), 2);
    }

    #[test]
    fn loads_dev_collections_on_request() {
        let cat = Catalog::load(&crate::test_support::fixture_dir(), true).unwrap();
        assert_eq!(cat.collections.len(), 2);
        assert_eq!(cat.card("quartz").unwrap().collections, vec!["minerals", "dev"]);
    }

    /// The real data repository, when it sits next to this one
    #[test]
    fn loads_real_data_if_present() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../anycardz-data");
        if !dir.is_dir() {
            return;
        }
        let cat = Catalog::load(&dir, false).unwrap();
        assert!(cat.collections.len() >= 10);
        assert!(cat.cards.len() > 1000);
    }
}
