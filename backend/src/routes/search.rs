//! One search box for cards, collections and players.

use actix_web::{HttpResponse, get, web};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::i18n::Lang;
use crate::routes::catalog::CardDto;
use crate::routes::social::avatar_url;
use crate::schema::users;

const CARDS: usize = 12;
const COLLECTIONS: usize = 5;
const PLAYERS: i64 = 5;

diesel::define_sql_function!(fn lower(x: diesel::sql_types::Text) -> diesel::sql_types::Text);

/// Lowercase without accents: "Élément" and "element" match
pub fn fold(s: &str) -> String {
    s.nfd().filter(|c| !is_combining_mark(*c)).collect::<String>().to_lowercase()
}

/// 0 for a match at the start, 1 at the start of a word, 2 anywhere; None without one
pub fn rank(haystack: &str, needle: &str) -> Option<u8> {
    let h = fold(haystack);
    let at = h.find(needle)?;
    Some(if at == 0 {
        0
    } else if h[..at].ends_with(|c: char| !c.is_alphanumeric()) {
        1
    } else {
        2
    })
}

#[derive(Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    q: String,
}

#[derive(Serialize)]
struct CollectionHit<'a> {
    slug: &'a str,
    title: &'a str,
    primary: &'a str,
}

#[derive(Serialize)]
struct PlayerHit {
    username: String,
    avatar: Option<String>,
}

#[get("/search")]
pub async fn search(
    state: web::Data<AppState>,
    user: CurrentUser,
    lang: Lang,
    q: web::Query<SearchQuery>,
) -> ApiResult<HttpResponse> {
    let needle = fold(q.q.trim());
    if needle.chars().count() < 2 {
        return Ok(HttpResponse::Ok().json(serde_json::json!({ "cards": [], "collections": [], "players": [] })));
    }
    let cat = &state.catalog;
    let mut cards: Vec<(u8, &str, &crate::data::Card)> = cat
        .cards
        .values()
        .filter_map(|c| {
            let title = c.texts.get(lang, "title");
            rank(title, &needle).map(|r| (r, title, c))
        })
        .collect();
    cards.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.len().cmp(&b.1.len())).then(a.1.cmp(b.1)));
    let cards: Vec<CardDto> = cards.into_iter().take(CARDS).map(|(_, _, c)| CardDto::new(cat, c, lang)).collect();

    let mut collections: Vec<(u8, CollectionHit)> = cat
        .collections
        .iter()
        .filter_map(|c| {
            let title = c.texts.get(lang, "title");
            rank(title, &needle).map(|r| (r, CollectionHit { slug: &c.slug, title, primary: &c.primary }))
        })
        .collect();
    collections.sort_by_key(|(r, c)| (*r, c.title.len()));
    let collections: Vec<CollectionHit> = collections.into_iter().take(COLLECTIONS).map(|(_, c)| c).collect();

    let pattern =
        format!("%{}%", q.q.trim().to_lowercase().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let mut conn = state.pool.get().await?;
    let mut players: Vec<(String, i32, bool)> = users::table
        .filter(lower(users::username).like(pattern))
        .filter(users::id.ne(user.id))
        .select((users::username, users::avatar_version, users::avatar.is_not_null()))
        .limit(PLAYERS * 4)
        .load(&mut conn)
        .await?;
    players.sort_by_key(|(n, _, _)| (rank(n, &needle).unwrap_or(3), n.len()));
    let players: Vec<PlayerHit> = players
        .into_iter()
        .take(PLAYERS as usize)
        .map(|(n, v, a)| PlayerHit { avatar: avatar_url(&n, v, a), username: n })
        .collect();
    Ok(HttpResponse::Ok().json(serde_json::json!({ "cards": cards, "collections": collections, "players": players })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_case_and_accents() {
        assert_eq!(fold("Élément Chōkai"), "element chokai");
        assert_eq!(fold("ÇA"), "ca");
    }

    #[test]
    fn ranks_prefix_then_word_then_anywhere() {
        assert_eq!(rank("Bismarck", "bis"), Some(0));
        assert_eq!(rank("USS Iowa (BB-61)", "iowa"), Some(1));
        assert_eq!(rank("Tirpitz", "pit"), Some(2));
        assert_eq!(rank("Tirpitz", "zz"), None);
        assert_eq!(rank("Éléments", "elem"), Some(0));
    }
}
