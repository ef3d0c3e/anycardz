//! Account settings and data rights: password, email, sessions, export,
//! deletion. Every change that matters asks for the current password.

use actix_web::cookie::Cookie;
use actix_web::{HttpResponse, delete, get, post, web};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;
use crate::auth::{self, CurrentUser, SESSION_COOKIE};
use crate::error::{ApiError, ApiResult};
use crate::schema::{
    blocks, boosters, friendships, listings, notifications, owned_cards, push_subscriptions, sessions, shout_messages,
    streaks, topups, trade_lines, trades, users,
};

/// The current password, or an error. Wrong guesses count against the same
/// per-account budget as logins: settings are no way around it.
async fn check_password(
    state: &AppState,
    conn: &mut AsyncPgConnection,
    user: &CurrentUser,
    password: String,
) -> ApiResult<()> {
    let key = format!("#{}", user.id);
    if let Some(wait) = state.limits.login_account.wait(&key) {
        return Err(ApiError::RetryAfter(wait));
    }
    let hash: String = users::table.find(user.id).select(users::password_hash).first(conn).await?;
    if !auth::password_matches(hash, password).await? {
        state.limits.login_account.hit(&key);
        return Err(ApiError::Invalid("wrong_password"));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct PasswordBody {
    current: String,
    new: String,
}

/// New password; every other session is signed out
#[post("/account/password")]
pub async fn change_password(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<PasswordBody>,
) -> ApiResult<HttpResponse> {
    let PasswordBody { current, new } = body.into_inner();
    auth::validate_password(&new)?;
    let mut conn = state.pool.get().await?;
    check_password(&state, &mut conn, &user, current).await?;
    let hash = web::block(move || auth::hash_password(&new)).await.map_err(|e| ApiError::Internal(e.into()))??;
    let (uid, sid) = (user.id, user.session_id);
    conn.transaction::<_, ApiError, _>(async move |conn| {
        diesel::update(users::table.find(uid)).set(users::password_hash.eq(hash)).execute(conn).await?;
        diesel::delete(sessions::table.filter(sessions::user_id.eq(uid)).filter(sessions::id.ne(sid)))
            .execute(conn)
            .await?;
        Ok(())
    })
    .await?;
    Ok(HttpResponse::Ok().json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct EmailBody {
    password: String,
    email: String,
}

#[post("/account/email")]
pub async fn change_email(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<EmailBody>,
) -> ApiResult<HttpResponse> {
    let EmailBody { password, email } = body.into_inner();
    let email = email.trim().to_owned();
    auth::validate_email(&email)?;
    let mut conn = state.pool.get().await?;
    check_password(&state, &mut conn, &user, password).await?;
    match diesel::update(users::table.find(user.id)).set(users::email.eq(&email)).execute(&mut conn).await {
        Ok(_) => Ok(HttpResponse::Ok().json(json!({ "email": email }))),
        Err(diesel::result::Error::DatabaseError(diesel::result::DatabaseErrorKind::UniqueViolation, _)) => {
            Err(ApiError::Conflict("email_taken"))
        }
        Err(e) => Err(e.into()),
    }
}

#[derive(Serialize)]
struct SessionDto {
    id: i64,
    user_agent: String,
    created_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    current: bool,
}

#[get("/account/sessions")]
pub async fn list_sessions(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let rows: Vec<(i64, String, DateTime<Utc>, DateTime<Utc>)> = sessions::table
        .filter(sessions::user_id.eq(user.id))
        .filter(sessions::expires_at.gt(Utc::now()))
        .order(sessions::last_seen_at.desc())
        .select((sessions::id, sessions::user_agent, sessions::created_at, sessions::last_seen_at))
        .load(&mut conn)
        .await?;
    let list: Vec<SessionDto> = rows
        .into_iter()
        .map(|(id, user_agent, created_at, last_seen_at)| SessionDto {
            id,
            user_agent,
            created_at,
            last_seen_at,
            current: id == user.session_id,
        })
        .collect();
    Ok(HttpResponse::Ok().json(list))
}

/// Signs one session out (another device, usually)
#[delete("/account/sessions/{id}")]
pub async fn revoke_session(
    state: web::Data<AppState>,
    user: CurrentUser,
    id: web::Path<i64>,
) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let n = diesel::delete(sessions::table.find(*id).filter(sessions::user_id.eq(user.id))).execute(&mut conn).await?;
    if n == 0 {
        return Err(ApiError::NotFound("unknown_session"));
    }
    Ok(HttpResponse::Ok().json(json!({ "ok": true })))
}

#[post("/account/sessions/revoke-others")]
pub async fn revoke_other_sessions(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let n =
        diesel::delete(sessions::table.filter(sessions::user_id.eq(user.id)).filter(sessions::id.ne(user.session_id)))
            .execute(&mut conn)
            .await?;
    Ok(HttpResponse::Ok().json(json!({ "signed_out": n })))
}

/// Usernames of these ids
async fn names(conn: &mut AsyncPgConnection, ids: Vec<i64>) -> ApiResult<std::collections::HashMap<i64, String>> {
    Ok(users::table
        .filter(users::id.eq_any(ids))
        .select((users::id, users::username))
        .load::<(i64, String)>(conn)
        .await?
        .into_iter()
        .collect())
}

/// Everything kept about the player, as one JSON file
#[get("/account/export")]
#[allow(clippy::type_complexity)]
pub async fn export(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let conn = &mut conn;
    let uid = user.id;

    let (created, bio, has_avatar, avatar_version, terms_version, terms_at): (
        DateTime<Utc>,
        String,
        bool,
        i32,
        Option<i32>,
        Option<DateTime<Utc>>,
    ) = users::table
        .find(uid)
        .select((
            users::created_at,
            users::bio,
            users::avatar.is_not_null(),
            users::avatar_version,
            users::terms_version,
            users::terms_accepted_at,
        ))
        .first(conn)
        .await?;

    let cards: Vec<(String, Option<String>, DateTime<Utc>)> = owned_cards::table
        .filter(owned_cards::user_id.eq(uid))
        .order(owned_cards::id)
        .select((owned_cards::card, owned_cards::foil, owned_cards::obtained_at))
        .load(conn)
        .await?;
    let pile: Vec<(String, DateTime<Utc>)> = boosters::table
        .filter(boosters::user_id.eq(uid))
        .select((boosters::collection, boosters::created_at))
        .load(conn)
        .await?;
    let streak: Option<(i32, i32, Option<chrono::NaiveDate>)> = streaks::table
        .find(uid)
        .select((streaks::day, streaks::best, streaks::last_claim))
        .first(conn)
        .await
        .optional()?;
    let topups: Vec<(String, i64, DateTime<Utc>)> = topups::table
        .filter(topups::user_id.eq(uid))
        .select((topups::pack, topups::coins, topups::created_at))
        .load(conn)
        .await?;
    let shouts: Vec<(String, Option<String>, Option<String>, DateTime<Utc>)> = shout_messages::table
        .filter(shout_messages::user_id.eq(uid))
        .order(shout_messages::id)
        .select((shout_messages::body, shout_messages::card, shout_messages::foil, shout_messages::created_at))
        .load(conn)
        .await?;
    let friends: Vec<(i64, i64, bool, DateTime<Utc>)> = friendships::table
        .filter(friendships::requester_id.eq(uid).or(friendships::addressee_id.eq(uid)))
        .select((friendships::requester_id, friendships::addressee_id, friendships::accepted, friendships::created_at))
        .load(conn)
        .await?;
    let blocked: Vec<(i64, DateTime<Utc>)> = blocks::table
        .filter(blocks::user_id.eq(uid))
        .select((blocks::blocked_id, blocks::created_at))
        .load(conn)
        .await?;
    let my_trades: Vec<(i64, i64, i64, String, i64, i64, DateTime<Utc>, DateTime<Utc>)> = trades::table
        .filter(trades::from_id.eq(uid).or(trades::to_id.eq(uid)))
        .order(trades::id)
        .select((
            trades::id,
            trades::from_id,
            trades::to_id,
            trades::state,
            trades::from_coins,
            trades::to_coins,
            trades::created_at,
            trades::updated_at,
        ))
        .load(conn)
        .await?;
    let trade_ids: Vec<i64> = my_trades.iter().map(|t| t.0).collect();
    let lines: Vec<(i64, String, String, bool, i32)> = trade_lines::table
        .filter(trade_lines::trade_id.eq_any(&trade_ids))
        .select((trade_lines::trade_id, trade_lines::side, trade_lines::card, trade_lines::foil, trade_lines::count))
        .load(conn)
        .await?;
    let notes: Vec<(String, Option<i64>, Option<i64>, Option<DateTime<Utc>>, DateTime<Utc>)> = notifications::table
        .filter(notifications::user_id.eq(uid))
        .order(notifications::id)
        .select((
            notifications::kind,
            notifications::actor_id,
            notifications::trade_id,
            notifications::read_at,
            notifications::created_at,
        ))
        .load(conn)
        .await?;
    let pushes: Vec<(String, DateTime<Utc>)> = push_subscriptions::table
        .filter(push_subscriptions::user_id.eq(uid))
        .select((push_subscriptions::endpoint, push_subscriptions::created_at))
        .load(conn)
        .await?;
    let my_sessions: Vec<(String, DateTime<Utc>, DateTime<Utc>)> = sessions::table
        .filter(sessions::user_id.eq(uid))
        .select((sessions::user_agent, sessions::created_at, sessions::last_seen_at))
        .load(conn)
        .await?;

    let earned = crate::progress::earned(conn, uid).await?;
    // Listings the player made, and the ones they bought
    #[allow(clippy::type_complexity)]
    let market: Vec<(
        i64,
        i64,
        Option<i64>,
        String,
        Option<String>,
        i64,
        i64,
        String,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    )> = listings::table
        .filter(listings::seller_id.eq(uid).or(listings::buyer_id.eq(uid)))
        .order(listings::id)
        .select((
            listings::id,
            listings::seller_id,
            listings::buyer_id,
            listings::card,
            listings::foil,
            listings::price,
            listings::fee,
            listings::state,
            listings::created_at,
            listings::closed_at,
        ))
        .load(conn)
        .await?;

    let mut others: Vec<i64> = friends.iter().flat_map(|f| [f.0, f.1]).collect();
    others.extend(blocked.iter().map(|b| b.0));
    others.extend(my_trades.iter().flat_map(|t| [t.1, t.2]));
    others.extend(notes.iter().filter_map(|n| n.1));
    others.extend(market.iter().flat_map(|m| [Some(m.1), m.2]).flatten());
    let who = names(conn, others).await?;
    let name = |id: i64| who.get(&id).cloned().unwrap_or_else(|| "?".into());

    let body = json!({
        "exported_at": Utc::now(),
        "account": {
            "username": user.username,
            "email": user.email,
            "created_at": created,
            "coins": user.coins,
            "bio": bio,
            "avatar": crate::routes::social::avatar_url(&user.username, avatar_version, has_avatar),
            "terms": { "version": terms_version, "accepted_at": terms_at },
        },
        "cards": cards.iter().map(|(card, foil, at)| json!({ "card": card, "foil": foil, "obtained_at": at })).collect::<Vec<_>>(),
        "unopened_boosters": pile.iter().map(|(c, at)| json!({ "collection": c, "bought_at": at })).collect::<Vec<_>>(),
        "streak": streak.map(|(day, best, last)| json!({ "day": day, "best": best, "last_claim": last })),
        "topups": topups.iter().map(|(p, c, at)| json!({ "pack": p, "coins": c, "at": at })).collect::<Vec<_>>(),
        "shoutbox_messages": shouts.iter().map(|(b, c, f, at)| json!({ "body": b, "card": c, "foil": f, "at": at })).collect::<Vec<_>>(),
        "friends": friends.iter().map(|(r, a, ok, at)| json!({
            "player": name(if *r == uid { *a } else { *r }),
            "requested_by_you": *r == uid,
            "accepted": ok,
            "since": at,
        })).collect::<Vec<_>>(),
        "blocked": blocked.iter().map(|(b, at)| json!({ "player": name(*b), "since": at })).collect::<Vec<_>>(),
        "trades": my_trades.iter().map(|(id, from, to, st, fc, tc, created, updated)| {
            let mine = if *from == uid { "from" } else { "to" };
            let lines: Vec<_> = lines.iter().filter(|l| l.0 == *id).map(|(_, side, card, foil, count)| json!({
                "given_by_you": side == mine, "card": card, "foil": foil, "count": count,
            })).collect();
            json!({
                "partner": name(if *from == uid { *to } else { *from }),
                "state": st,
                "coins_from_you": if *from == uid { fc } else { tc },
                "coins_to_you": if *from == uid { tc } else { fc },
                "cards": lines,
                "created_at": created,
                "updated_at": updated,
            })
        }).collect::<Vec<_>>(),
        "achievements": earned,
        "market": market.iter().map(|(_, seller, buyer, card, foil, price, fee, st, created, closed)| json!({
            "sold_by_you": *seller == uid,
            "other_player": if *seller == uid { buyer.map(name) } else { Some(name(*seller)) },
            "card": card, "foil": foil, "price": price,
            "fee": if *seller == uid { Some(fee) } else { None },
            "state": st, "listed_at": created, "closed_at": closed,
        })).collect::<Vec<_>>(),
        "notifications": notes.iter().map(|(kind, actor, trade, read, at)| json!({
            "kind": kind, "from": actor.map(name), "trade": trade, "read_at": read, "at": at,
        })).collect::<Vec<_>>(),
        "push_subscriptions": pushes.iter().map(|(endpoint, at)| json!({ "endpoint": endpoint, "since": at })).collect::<Vec<_>>(),
        "sessions": my_sessions.iter().map(|(ua, created, seen)| json!({
            "device": ua, "signed_in_at": created, "last_seen_at": seen,
        })).collect::<Vec<_>>(),
    });
    let file = format!("anycardz-{}-{}.json", user.username, Utc::now().format("%Y-%m-%d"));
    Ok(HttpResponse::Ok().insert_header(("content-disposition", format!("attachment; filename=\"{file}\""))).json(body))
}

#[derive(Deserialize)]
pub struct DeleteBody {
    password: String,
    /// The username, typed again
    confirm: String,
}

/// Deletes the account and everything tied to it: cards, trades, messages,
/// friendships, notifications. There is no undo.
#[post("/account/delete")]
pub async fn delete_account(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<DeleteBody>,
) -> ApiResult<HttpResponse> {
    let DeleteBody { password, confirm } = body.into_inner();
    if !confirm.trim().eq_ignore_ascii_case(&user.username) {
        return Err(ApiError::Invalid("confirm_username"));
    }
    let mut conn = state.pool.get().await?;
    check_password(&state, &mut conn, &user, password).await?;
    let uid = user.id;
    // Partners of open trades see them vanish at once
    let open: Vec<(i64, i64, i64)> = conn
        .transaction::<_, ApiError, _>(async move |conn| {
            // Trades first, then the player, the order a seal takes its locks in:
            // a swap in flight finishes before the account goes, or never starts
            let open = trades::table
                .filter(trades::from_id.eq(uid).or(trades::to_id.eq(uid)))
                .filter(trades::state.eq("open"))
                .select((trades::id, trades::from_id, trades::to_id))
                .for_update()
                .load(conn)
                .await?;
            diesel::delete(users::table.find(uid)).execute(conn).await?;
            Ok(open)
        })
        .await?;
    for (id, from, to) in open {
        state.live.trade(id, [from, to]);
    }
    tracing::info!(user = uid, "account deleted");
    let mut gone = Cookie::build(SESSION_COOKIE, "").path("/").finish();
    gone.make_removal();
    Ok(HttpResponse::Ok().cookie(gone).json(json!({ "ok": true })))
}
