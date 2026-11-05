//! Public catalog: collections, cards, artwork thumbnails, foil masks.

use std::path::PathBuf;

use actix_files::NamedFile;
use actix_web::http::header::{CacheControl, CacheDirective};
use actix_web::{HttpRequest, HttpResponse, get, web};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::data::{Card, Catalog, Collection, Rarity};
use crate::error::{ApiError, ApiResult};
use crate::game::BOOSTER_PRICE;
use crate::i18n::Lang;

/// Widths the thumbnail endpoint renders. A fixed list keeps the cache bounded.
pub const THUMB_WIDTHS: [u32; 2] = [420, 840];

#[derive(Serialize)]
pub struct CardDto<'a> {
    pub slug: &'a str,
    pub number: u32,
    pub rarity: Rarity,
    pub title: &'a str,
    pub description: &'a str,
    pub primary: &'a str,
    pub secondary: &'a str,
    /// Collection the number refers to
    pub collection: &'a str,
    pub total: usize,
}

impl<'a> CardDto<'a> {
    pub fn new(catalog: &'a Catalog, card: &'a Card, lang: Lang) -> Self {
        let collection = catalog.collection(&card.collections[0]).expect("card collections exist");
        CardDto {
            slug: &card.slug,
            number: card.number,
            rarity: card.rarity,
            title: card.texts.get(lang, "title"),
            description: card.texts.get(lang, "description"),
            primary: &card.primary,
            secondary: &card.secondary,
            collection: &collection.slug,
            total: collection.cards.len(),
        }
    }
}

#[derive(Serialize)]
pub struct CollectionDto<'a> {
    pub slug: &'a str,
    pub number: u32,
    pub title: &'a str,
    pub description: &'a str,
    pub primary: &'a str,
    pub secondary: &'a str,
    pub total: usize,
    pub price: i64,
    /// Cards per rarity, e.g. whether a trade-up into this collection can pay out
    pub rarities: std::collections::BTreeMap<Rarity, usize>,
    /// Coins for owning every card of the set, and every one as a foil
    pub set_reward: i64,
    pub foil_set_reward: i64,
    pub featured: CardDto<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cards: Option<Vec<CardDto<'a>>>,
}

impl<'a> CollectionDto<'a> {
    pub fn new(catalog: &'a Catalog, c: &'a Collection, lang: Lang, with_cards: bool) -> Self {
        CollectionDto {
            slug: &c.slug,
            number: c.number,
            title: c.texts.get(lang, "title"),
            description: c.texts.get(lang, "description"),
            primary: &c.primary,
            secondary: &c.secondary,
            total: c.cards.len(),
            price: BOOSTER_PRICE,
            rarities: catalog.pool(c).into_iter().map(|(r, cards)| (r, cards.len())).collect(),
            set_reward: crate::progress::set_reward(c.cards.len()),
            foil_set_reward: crate::progress::foil_set_reward(c.cards.len()),
            featured: CardDto::new(catalog, &catalog.cards[&c.featured], lang),
            cards: with_cards.then(|| c.cards.iter().map(|s| CardDto::new(catalog, &catalog.cards[s], lang)).collect()),
        }
    }
}

#[get("/collections")]
pub async fn list_collections(state: web::Data<AppState>, lang: Lang) -> HttpResponse {
    let cat = &state.catalog;
    let list: Vec<_> = cat.collections.iter().map(|c| CollectionDto::new(cat, c, lang, false)).collect();
    HttpResponse::Ok().json(list)
}

#[get("/collections/{slug}")]
pub async fn get_collection(
    state: web::Data<AppState>,
    lang: Lang,
    slug: web::Path<String>,
) -> ApiResult<HttpResponse> {
    let cat = &state.catalog;
    let c = cat.collection(&slug).ok_or(ApiError::NotFound("unknown_collection"))?;
    Ok(HttpResponse::Ok().json(CollectionDto::new(cat, c, lang, true)))
}

#[get("/cards/{slug}")]
pub async fn get_card(state: web::Data<AppState>, lang: Lang, slug: web::Path<String>) -> ApiResult<HttpResponse> {
    let cat = &state.catalog;
    let card = cat.card(&slug).ok_or(ApiError::NotFound("unknown_card"))?;
    Ok(HttpResponse::Ok().json(CardDto::new(cat, card, lang)))
}

#[derive(Deserialize)]
pub struct ArtQuery {
    w: Option<u32>,
}

/// Card artwork, resized to a thumbnail on first request and cached on disk
#[get("/cards/{slug}/art")]
pub async fn card_art(
    state: web::Data<AppState>,
    req: HttpRequest,
    slug: web::Path<String>,
    q: web::Query<ArtQuery>,
) -> ApiResult<HttpResponse> {
    let card = state.catalog.card(&slug).ok_or(ApiError::NotFound("unknown_card"))?;
    let width = q.w.unwrap_or(THUMB_WIDTHS[0]);
    if !THUMB_WIDTHS.contains(&width) {
        return Err(ApiError::Invalid("unsupported_width"));
    }
    let target = state.config.cache_dir.join(format!("art-{width}")).join(format!("{}.jpg", card.slug));
    if !target.is_file() {
        let (source, out) = (card.artwork.clone(), target.clone());
        web::block(move || make_thumbnail(&source, &out, width)).await.map_err(|e| ApiError::Internal(e.into()))??;
    }
    Ok(cached_file(target, &req)?)
}

fn make_thumbnail(source: &std::path::Path, out: &std::path::Path, width: u32) -> anyhow::Result<()> {
    let img = image::ImageReader::open(source)?.with_guessed_format()?.decode()?;
    let img =
        if img.width() > width { img.resize(width, u32::MAX, image::imageops::FilterType::Lanczos3) } else { img };
    std::fs::create_dir_all(out.parent().unwrap())?;
    // Write then rename: concurrent first requests never serve a half-written file
    let tmp = out.with_extension(format!("tmp{}", std::process::id()));
    let mut file = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 82).encode_image(&img.to_rgb8())?;
    drop(file);
    std::fs::rename(tmp, out)?;
    Ok(())
}

/// Foil masks and the wrapper texture from the data repo's assets/
#[get("/assets/{name}")]
pub async fn get_asset(
    state: web::Data<AppState>,
    req: HttpRequest,
    name: web::Path<String>,
) -> ApiResult<HttpResponse> {
    let valid = name.len() < 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        && !name.starts_with('.')
        && (name.ends_with(".png") || name.ends_with(".jpg"));
    if !valid {
        return Err(ApiError::NotFound("unknown_asset"));
    }
    let path: PathBuf = state.catalog.assets_dir.join(name.as_str());
    if !path.is_file() {
        return Err(ApiError::NotFound("unknown_asset"));
    }
    Ok(cached_file(path, &req)?)
}

fn cached_file(path: PathBuf, req: &HttpRequest) -> anyhow::Result<HttpResponse> {
    let mut res = NamedFile::open(path)?.use_etag(true).into_response(req);
    res.headers_mut().insert(
        actix_web::http::header::CACHE_CONTROL,
        CacheControl(vec![CacheDirective::Public, CacheDirective::MaxAge(86_400)]).to_string().parse().unwrap(),
    );
    Ok(res)
}
