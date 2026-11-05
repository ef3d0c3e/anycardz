//! Profiles, avatars, friends and blocks.

use std::collections::HashMap;

use actix_web::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use actix_web::{HttpResponse, delete, get, post, put, web};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::schema::{blocks, friendships, owned_cards, users};

diesel::define_sql_function!(fn lower(x: diesel::sql_types::Text) -> diesel::sql_types::Text);

const MAX_BIO: usize = 280;
const AVATAR_SIZE: u32 = 256;
pub const MAX_AVATAR_BYTES: usize = 4 * 1024 * 1024;

/// (id, username)
pub async fn by_name(conn: &mut AsyncPgConnection, name: &str) -> ApiResult<(i64, String)> {
    users::table
        .filter(lower(users::username).eq(name.trim().to_lowercase()))
        .select((users::id, users::username))
        .first(conn)
        .await
        .optional()?
        .ok_or(ApiError::NotFound("unknown_player"))
}

/// Whether either player blocked the other
pub async fn blocked_between(conn: &mut AsyncPgConnection, a: i64, b: i64) -> ApiResult<bool> {
    let n: i64 = blocks::table
        .filter(
            (blocks::user_id.eq(a).and(blocks::blocked_id.eq(b)))
                .or(blocks::user_id.eq(b).and(blocks::blocked_id.eq(a))),
        )
        .count()
        .get_result(conn)
        .await?;
    Ok(n > 0)
}

/// Players the user blocked
pub async fn blocked_by(conn: &mut AsyncPgConnection, uid: i64) -> ApiResult<Vec<i64>> {
    Ok(blocks::table.filter(blocks::user_id.eq(uid)).select(blocks::blocked_id).load(conn).await?)
}

pub fn avatar_url(username: &str, version: i32, has_avatar: bool) -> Option<String> {
    has_avatar.then(|| format!("/api/avatars/{username}?v={version}"))
}

/// The viewer's link to another player
async fn relation(conn: &mut AsyncPgConnection, viewer: i64, other: i64) -> ApiResult<&'static str> {
    if viewer == other {
        return Ok("self");
    }
    let blocked: i64 = blocks::table
        .filter(blocks::user_id.eq(viewer).and(blocks::blocked_id.eq(other)))
        .count()
        .get_result(conn)
        .await?;
    if blocked > 0 {
        return Ok("blocked");
    }
    let f: Option<(i64, bool)> = friendships::table
        .filter(
            (friendships::requester_id.eq(viewer).and(friendships::addressee_id.eq(other)))
                .or(friendships::requester_id.eq(other).and(friendships::addressee_id.eq(viewer))),
        )
        .select((friendships::requester_id, friendships::accepted))
        .first(conn)
        .await
        .optional()?;
    Ok(match f {
        Some((_, true)) => "friends",
        Some((r, false)) if r == viewer => "request_sent",
        Some(_) => "request_received",
        None => "none",
    })
}

#[derive(Serialize)]
struct CollectionStat {
    slug: String,
    owned: usize,
    total: usize,
}

#[derive(Serialize)]
struct ProfileDto {
    username: String,
    bio: String,
    avatar: Option<String>,
    joined: DateTime<Utc>,
    online: bool,
    /// Distinct cards, all copies, foil copies
    cards: usize,
    copies: i64,
    foils: i64,
    /// Most complete collections first
    collections: Vec<CollectionStat>,
    relation: &'static str,
    /// Completed sets and achievements, newest first
    achievements: Vec<crate::progress::EarnedDto>,
}

#[get("/players/{name}")]
pub async fn get_profile(
    state: web::Data<AppState>,
    user: CurrentUser,
    name: web::Path<String>,
) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let (id, username) = by_name(&mut conn, &name).await?;
    let (bio, joined, version, has_avatar): (String, DateTime<Utc>, i32, bool) = users::table
        .find(id)
        .select((users::bio, users::created_at, users::avatar_version, users::avatar.is_not_null()))
        .first(&mut conn)
        .await?;
    let rows: Vec<(String, i64, i64)> = owned_cards::table
        .filter(owned_cards::user_id.eq(id))
        .group_by(owned_cards::card)
        .select((owned_cards::card, diesel::dsl::count_star(), diesel::dsl::count(owned_cards::foil)))
        .load(&mut conn)
        .await?;
    let mut per: HashMap<&str, usize> = HashMap::new();
    for (slug, _, _) in &rows {
        if let Some(card) = state.catalog.card(slug) {
            for c in &card.collections {
                *per.entry(c.as_str()).or_default() += 1;
            }
        }
    }
    let mut collections: Vec<CollectionStat> = state
        .catalog
        .collections
        .iter()
        .filter_map(|c| {
            per.get(c.slug.as_str()).map(|&owned| CollectionStat { slug: c.slug.clone(), owned, total: c.cards.len() })
        })
        .collect();
    collections.sort_by(|a, b| (b.owned * a.total).cmp(&(a.owned * b.total)).then(b.owned.cmp(&a.owned)));
    let relation = relation(&mut conn, user.id, id).await?;
    Ok(HttpResponse::Ok().json(ProfileDto {
        avatar: avatar_url(&username, version, has_avatar),
        online: state.live.is_online(id),
        username,
        bio,
        joined,
        cards: rows.len(),
        copies: rows.iter().map(|r| r.1).sum(),
        foils: rows.iter().map(|r| r.2).sum(),
        collections,
        relation,
        achievements: crate::progress::earned(&mut conn, id).await?,
    }))
}

#[derive(Deserialize)]
pub struct ProfileBody {
    bio: String,
}

#[put("/me/profile")]
pub async fn edit_profile(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<ProfileBody>,
) -> ApiResult<HttpResponse> {
    let bio = body.bio.trim().to_owned();
    if bio.chars().count() > MAX_BIO {
        return Err(ApiError::Invalid("bio_too_long"));
    }
    let mut conn = state.pool.get().await?;
    diesel::update(users::table.find(user.id)).set(users::bio.eq(&bio)).execute(&mut conn).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "bio": bio })))
}

/// Center crop to a square, 256 px, JPEG. Refuses anything that does not decode.
fn make_avatar(bytes: &[u8]) -> ApiResult<Vec<u8>> {
    let img = image::load_from_memory(bytes).map_err(|_| ApiError::Invalid("invalid_image"))?;
    let side = img.width().min(img.height());
    let square = img.crop_imm((img.width() - side) / 2, (img.height() - side) / 2, side, side);
    let small = square.resize_exact(AVATAR_SIZE, AVATAR_SIZE, image::imageops::FilterType::Lanczos3);
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85)
        .encode_image(&small.to_rgb8())
        .map_err(|e| ApiError::Internal(e.into()))?;
    Ok(out)
}

/// The raw image file as the request body
#[put("/me/avatar")]
pub async fn put_avatar(state: web::Data<AppState>, user: CurrentUser, body: web::Bytes) -> ApiResult<HttpResponse> {
    if body.is_empty() {
        return Err(ApiError::Invalid("invalid_image"));
    }
    let jpeg = web::block(move || make_avatar(&body)).await.map_err(|e| ApiError::Internal(e.into()))??;
    let mut conn = state.pool.get().await?;
    let version: i32 = diesel::update(users::table.find(user.id))
        .set((users::avatar.eq(jpeg), users::avatar_version.eq(users::avatar_version + 1)))
        .returning(users::avatar_version)
        .get_result(&mut conn)
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "avatar": avatar_url(&user.username, version, true) })))
}

#[delete("/me/avatar")]
pub async fn delete_avatar(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    diesel::update(users::table.find(user.id))
        .set((users::avatar.eq(None::<Vec<u8>>), users::avatar_version.eq(users::avatar_version + 1)))
        .execute(&mut conn)
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "avatar": null })))
}

/// Public: avatars show in the shoutbox and on profiles. The ?v= in the URL busts caches.
#[get("/avatars/{name}")]
pub async fn get_avatar(state: web::Data<AppState>, name: web::Path<String>) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let avatar: Option<Option<Vec<u8>>> = users::table
        .filter(lower(users::username).eq(name.to_lowercase()))
        .select(users::avatar)
        .first(&mut conn)
        .await
        .optional()?;
    let bytes = avatar.flatten().ok_or(ApiError::NotFound("no_avatar"))?;
    Ok(HttpResponse::Ok()
        .insert_header((CONTENT_TYPE, "image/jpeg"))
        .insert_header((CACHE_CONTROL, "public, max-age=86400"))
        .body(bytes))
}

#[derive(Serialize)]
struct FriendDto {
    username: String,
    avatar: Option<String>,
    online: bool,
}

#[get("/friends")]
pub async fn list_friends(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let uid = user.id;
    let rows: Vec<(i64, i64, bool)> = friendships::table
        .filter(friendships::requester_id.eq(uid).or(friendships::addressee_id.eq(uid)))
        .select((friendships::requester_id, friendships::addressee_id, friendships::accepted))
        .load(&mut conn)
        .await?;
    let others: Vec<i64> = rows.iter().map(|(r, a, _)| if *r == uid { *a } else { *r }).collect();
    let info: HashMap<i64, (String, i32, bool)> = users::table
        .filter(users::id.eq_any(&others))
        .select((users::id, users::username, users::avatar_version, users::avatar.is_not_null()))
        .load::<(i64, String, i32, bool)>(&mut conn)
        .await?
        .into_iter()
        .map(|(id, n, v, a)| (id, (n, v, a)))
        .collect();
    let dto = |id: i64| {
        let (n, v, a) = &info[&id];
        FriendDto { username: n.clone(), avatar: avatar_url(n, *v, *a), online: state.live.is_online(id) }
    };
    let (mut friends, mut incoming, mut outgoing) = (Vec::new(), Vec::new(), Vec::new());
    for (r, a, accepted) in rows {
        let other = if r == uid { a } else { r };
        match (accepted, r == uid) {
            (true, _) => friends.push(dto(other)),
            (false, true) => outgoing.push(dto(other)),
            (false, false) => incoming.push(dto(other)),
        }
    }
    // Online friends first
    friends.sort_by(|a, b| b.online.cmp(&a.online).then(a.username.to_lowercase().cmp(&b.username.to_lowercase())));
    Ok(HttpResponse::Ok().json(serde_json::json!({ "friends": friends, "incoming": incoming, "outgoing": outgoing })))
}

/// Sends a friend request, or accepts theirs
#[post("/friends/{name}")]
pub async fn add_friend(
    state: web::Data<AppState>,
    user: CurrentUser,
    name: web::Path<String>,
) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let (other, _) = by_name(&mut conn, &name).await?;
    if other == user.id {
        return Err(ApiError::Invalid("friend_self"));
    }
    let uid = user.id;
    let event = conn
        .transaction::<_, ApiError, _>(async move |conn| {
            if blocked_between(conn, uid, other).await? {
                return Err(ApiError::Conflict("player_unavailable"));
            }
            // Their pending request: accepting it
            let theirs = diesel::update(friendships::table.find((other, uid)).filter(friendships::accepted.eq(false)))
                .set(friendships::accepted.eq(true))
                .execute(conn)
                .await?;
            if theirs > 0 {
                return Ok(Some(crate::routes::notifications::Kind::FriendAccepted));
            }
            let asked = diesel::insert_into(friendships::table)
                .values((friendships::requester_id.eq(uid), friendships::addressee_id.eq(other)))
                .on_conflict_do_nothing()
                .execute(conn)
                .await?;
            // Asking again changes nothing and notifies no one
            Ok((asked > 0).then_some(crate::routes::notifications::Kind::FriendRequest))
        })
        .await?;
    if let Some(kind) = event {
        crate::routes::notifications::notify(&state, &mut conn, other, kind, uid, None).await?;
    }
    let rel = relation(&mut conn, uid, other).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "relation": rel })))
}

/// Unfriends, declines or cancels, whoever asked
#[delete("/friends/{name}")]
pub async fn remove_friend(
    state: web::Data<AppState>,
    user: CurrentUser,
    name: web::Path<String>,
) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let (other, _) = by_name(&mut conn, &name).await?;
    unfriend(&mut conn, user.id, other).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "relation": relation(&mut conn, user.id, other).await? })))
}

async fn unfriend(conn: &mut AsyncPgConnection, a: i64, b: i64) -> ApiResult<()> {
    diesel::delete(
        friendships::table.filter(
            (friendships::requester_id.eq(a).and(friendships::addressee_id.eq(b)))
                .or(friendships::requester_id.eq(b).and(friendships::addressee_id.eq(a))),
        ),
    )
    .execute(conn)
    .await?;
    Ok(())
}

#[get("/blocks")]
pub async fn list_blocks(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let names: Vec<String> = blocks::table
        .inner_join(users::table.on(users::id.eq(blocks::blocked_id)))
        .filter(blocks::user_id.eq(user.id))
        .order(users::username.asc())
        .select(users::username)
        .load(&mut conn)
        .await?;
    Ok(HttpResponse::Ok().json(names.into_iter().map(|n| serde_json::json!({ "username": n })).collect::<Vec<_>>()))
}

/// Blocks a player: ends any friendship or request between them
#[post("/blocks/{name}")]
pub async fn block(state: web::Data<AppState>, user: CurrentUser, name: web::Path<String>) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let (other, _) = by_name(&mut conn, &name).await?;
    if other == user.id {
        return Err(ApiError::Invalid("block_self"));
    }
    let uid = user.id;
    conn.transaction::<_, ApiError, _>(async move |conn| {
        unfriend(conn, uid, other).await?;
        diesel::insert_into(blocks::table)
            .values((blocks::user_id.eq(uid), blocks::blocked_id.eq(other)))
            .on_conflict_do_nothing()
            .execute(conn)
            .await?;
        Ok(())
    })
    .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "relation": "blocked" })))
}

#[delete("/blocks/{name}")]
pub async fn unblock(
    state: web::Data<AppState>,
    user: CurrentUser,
    name: web::Path<String>,
) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let (other, _) = by_name(&mut conn, &name).await?;
    diesel::delete(blocks::table.find((user.id, other))).execute(&mut conn).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "relation": relation(&mut conn, user.id, other).await? })))
}
