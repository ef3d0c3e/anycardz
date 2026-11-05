//! Finding other players and seeing their cards, for trades.

use std::collections::HashMap;

use actix_web::{HttpResponse, get, web};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::i18n::Lang;
use crate::routes::catalog::CardDto;
use crate::schema::{owned_cards, users};

diesel::define_sql_function!(fn lower(x: diesel::sql_types::Text) -> diesel::sql_types::Text);

#[derive(Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    q: String,
}

#[derive(Serialize)]
struct PlayerDto {
    username: String,
}

/// Players whose name contains `q`, names starting with it first
#[get("/players")]
pub async fn search_players(
    state: web::Data<AppState>,
    user: CurrentUser,
    q: web::Query<SearchQuery>,
) -> ApiResult<HttpResponse> {
    let needle = q.q.trim().to_lowercase();
    if needle.is_empty() {
        return Ok(HttpResponse::Ok().json(Vec::<PlayerDto>::new()));
    }
    // LIKE wildcards in the query are matched literally
    let escaped = needle.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    let mut conn = state.pool.get().await?;
    let mut names: Vec<String> = users::table
        .filter(lower(users::username).like(format!("%{escaped}%")))
        .filter(users::id.ne(user.id))
        .select(users::username)
        .limit(20)
        .load(&mut conn)
        .await?;
    names.sort_by_key(|n| (!n.to_lowercase().starts_with(&needle), n.len(), n.clone()));
    names.truncate(10);
    Ok(HttpResponse::Ok().json(names.into_iter().map(|username| PlayerDto { username }).collect::<Vec<_>>()))
}

/// Another player's cards, with details: what can be asked for in a trade
#[get("/players/{name}/cards")]
pub async fn player_cards(
    state: web::Data<AppState>,
    _user: CurrentUser,
    lang: Lang,
    name: web::Path<String>,
) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let id: i64 = users::table
        .filter(lower(users::username).eq(name.to_lowercase()))
        .select(users::id)
        .first(&mut conn)
        .await
        .optional()?
        .ok_or(ApiError::NotFound("unknown_player"))?;
    let rows: Vec<(String, i64, i64)> = owned_cards::table
        .filter(owned_cards::user_id.eq(id))
        .group_by(owned_cards::card)
        .select((owned_cards::card, diesel::dsl::count_star(), diesel::dsl::count(owned_cards::foil)))
        .order(owned_cards::card.asc())
        .load(&mut conn)
        .await?;
    let details: HashMap<&str, CardDto> = rows
        .iter()
        .filter_map(|(slug, _, _)| state.catalog.card(slug))
        .map(|c| (c.slug.as_str(), CardDto::new(&state.catalog, c, lang)))
        .collect();
    let cards: Vec<_> = rows
        .iter()
        .map(|(card, count, foils)| serde_json::json!({ "card": card, "count": count, "foils": foils }))
        .collect();
    Ok(HttpResponse::Ok().json(serde_json::json!({ "cards": cards, "details": details })))
}
