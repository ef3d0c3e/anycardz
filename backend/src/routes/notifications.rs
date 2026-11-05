//! Notifications: friend requests and trade events, stored and pushed live.

use actix_web::{HttpResponse, get, post, web};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::routes::social::avatar_url;
use crate::schema::{notifications, users};

const LIST: i64 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    FriendRequest,
    FriendAccepted,
    TradeOpened,
    TradeChanged,
    TradeSealed,
    TradeDone,
    TradeCancelled,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::FriendRequest => "friend_request",
            Kind::FriendAccepted => "friend_accepted",
            Kind::TradeOpened => "trade_opened",
            Kind::TradeChanged => "trade_changed",
            Kind::TradeSealed => "trade_sealed",
            Kind::TradeDone => "trade_done",
            Kind::TradeCancelled => "trade_cancelled",
        }
    }
}

#[derive(Serialize)]
struct NotificationDto {
    id: i64,
    kind: String,
    actor: Option<String>,
    actor_avatar: Option<String>,
    trade: Option<i64>,
    read: bool,
    at: DateTime<Utc>,
}

type Row = (i64, String, Option<i64>, Option<i64>, Option<DateTime<Utc>>, DateTime<Utc>);

async fn dtos(conn: &mut AsyncPgConnection, rows: Vec<Row>) -> ApiResult<Vec<NotificationDto>> {
    let actors: Vec<i64> = rows.iter().filter_map(|r| r.2).collect();
    let names: std::collections::HashMap<i64, (String, i32, bool)> = users::table
        .filter(users::id.eq_any(&actors))
        .select((users::id, users::username, users::avatar_version, users::avatar.is_not_null()))
        .load::<(i64, String, i32, bool)>(conn)
        .await?
        .into_iter()
        .map(|(id, n, v, a)| (id, (n, v, a)))
        .collect();
    Ok(rows
        .into_iter()
        .map(|(id, kind, actor, trade, read_at, at)| {
            let a = actor.and_then(|a| names.get(&a));
            NotificationDto {
                id,
                kind,
                actor: a.map(|(n, _, _)| n.clone()),
                actor_avatar: a.and_then(|(n, v, has)| avatar_url(n, *v, *has)),
                trade,
                read: read_at.is_some(),
                at,
            }
        })
        .collect())
}

/// Stores a notification for `user` and pushes it to their open sockets.
/// A trade edit while the last one is still unread refreshes it instead of
/// stacking a new one for every tap.
pub async fn notify(
    state: &AppState,
    conn: &mut AsyncPgConnection,
    user: i64,
    kind: Kind,
    actor: i64,
    trade: Option<i64>,
) -> ApiResult<()> {
    let now = Utc::now();
    let mut id: Option<i64> = None;
    if kind == Kind::TradeChanged {
        id = diesel::update(
            notifications::table
                .filter(notifications::user_id.eq(user))
                .filter(notifications::kind.eq(kind.as_str()))
                .filter(notifications::trade_id.eq(trade))
                .filter(notifications::read_at.is_null()),
        )
        .set((notifications::created_at.eq(now), notifications::actor_id.eq(actor)))
        .returning(notifications::id)
        .get_result(conn)
        .await
        .optional()?;
    }
    let id = match id {
        Some(id) => id,
        None => {
            diesel::insert_into(notifications::table)
                .values((
                    notifications::user_id.eq(user),
                    notifications::kind.eq(kind.as_str()),
                    notifications::actor_id.eq(actor),
                    notifications::trade_id.eq(trade),
                ))
                .returning(notifications::id)
                .get_result(conn)
                .await?
        }
    };
    let row: Row = (id, kind.as_str().to_owned(), Some(actor), trade, None, now);
    let dto = dtos(conn, vec![row]).await?.pop().unwrap();
    state.live.notify(user, serde_json::to_value(dto).expect("notifications serialize"));
    Ok(())
}

pub async fn unread(conn: &mut AsyncPgConnection, user: i64) -> ApiResult<i64> {
    Ok(notifications::table
        .filter(notifications::user_id.eq(user))
        .filter(notifications::read_at.is_null())
        .count()
        .get_result(conn)
        .await?)
}

#[get("/notifications")]
pub async fn list(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let rows: Vec<Row> = notifications::table
        .filter(notifications::user_id.eq(user.id))
        .order(notifications::created_at.desc())
        .limit(LIST)
        .select((
            notifications::id,
            notifications::kind,
            notifications::actor_id,
            notifications::trade_id,
            notifications::read_at,
            notifications::created_at,
        ))
        .load(&mut conn)
        .await?;
    let items = dtos(&mut conn, rows).await?;
    let unread = unread(&mut conn, user.id).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "items": items, "unread": unread })))
}

#[derive(Deserialize)]
pub struct ReadBody {
    /// These ones, or all of them when absent
    ids: Option<Vec<i64>>,
}

#[post("/notifications/read")]
pub async fn mark_read(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<ReadBody>,
) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let mine = notifications::table.filter(notifications::user_id.eq(user.id)).filter(notifications::read_at.is_null());
    match &body.ids {
        Some(ids) => {
            diesel::update(mine.filter(notifications::id.eq_any(ids)))
                .set(notifications::read_at.eq(Utc::now()))
                .execute(&mut conn)
                .await?
        }
        None => diesel::update(mine).set(notifications::read_at.eq(Utc::now())).execute(&mut conn).await?,
    };
    Ok(HttpResponse::Ok().json(serde_json::json!({ "unread": unread(&mut conn, user.id).await? })))
}
