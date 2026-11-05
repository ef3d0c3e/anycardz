//! The live socket (push only) and the shoutbox. Everything a client sends
//! goes through plain REST calls: validation and errors stay in one place.

use std::collections::HashMap;
use std::time::Duration;

use actix_web::{HttpRequest, HttpResponse, get, post, web};
use actix_ws::Message;
use chrono::Utc;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::i18n::Lang;
use crate::live::ChatEvent;
use crate::routes::catalog::CardDto;
use crate::schema::{owned_cards, shout_messages, users};

const HISTORY: i64 = 50;
const MAX_BODY: usize = 300;
/// At most one message every 2 s, and 10 a minute
const MIN_GAP: chrono::Duration = chrono::Duration::seconds(2);
const PER_MINUTE: i64 = 10;
const PING_EVERY: Duration = Duration::from_secs(30);

/// id, author, body, card, foil, time
type HistoryRow = (i64, String, String, Option<String>, Option<String>, chrono::DateTime<Utc>);

/// The last messages, without those of players the viewer blocked
async fn history(conn: &mut AsyncPgConnection, viewer: i64) -> ApiResult<Vec<ChatEvent>> {
    let blocked = crate::routes::social::blocked_by(conn, viewer).await?;
    let rows: Vec<HistoryRow> = shout_messages::table
        .inner_join(users::table)
        .filter(diesel::dsl::not(shout_messages::user_id.eq_any(&blocked)))
        .order(shout_messages::id.desc())
        .limit(HISTORY)
        .select((
            shout_messages::id,
            users::username,
            shout_messages::body,
            shout_messages::card,
            shout_messages::foil,
            shout_messages::created_at,
        ))
        .load(conn)
        .await?;
    // Oldest first, as a chat reads
    Ok(rows
        .into_iter()
        .rev()
        .map(|(id, user, body, card, foil, at)| ChatEvent { id, user, body, card, foil, at })
        .collect())
}

/// Push-only WebSocket: a "hello" snapshot, then live events
#[get("/live")]
pub async fn live(
    state: web::Data<AppState>,
    user: CurrentUser,
    req: HttpRequest,
    body: web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    let messages = {
        let mut conn = state.pool.get().await.map_err(ApiError::from)?;
        history(&mut conn, user.id).await?
    };
    let hello = serde_json::json!({ "type": "hello", "pulls": state.live.recent_pulls(), "messages": messages });
    let (response, mut session, mut stream) = actix_ws::handle(&req, body)?;
    let mut rx = state.live.subscribe();
    let uid = user.id;
    let hub_state = state.clone();
    actix_web::rt::spawn(async move {
        hub_state.live.connected(uid);
        if session.text(hello.to_string()).await.is_err() {
            hub_state.live.disconnected(uid);
            return;
        }
        let mut ping = tokio::time::interval(PING_EVERY);
        ping.tick().await;
        loop {
            tokio::select! {
                event = rx.recv() => match event {
                    Ok(env) if env.is_for(uid) => {
                        if session.text(env.json.clone()).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    // A slow client missed some events: it keeps going with the next ones
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => break,
                },
                msg = stream.recv() => match msg {
                    Some(Ok(Message::Ping(bytes))) => {
                        if session.pong(&bytes).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    _ => {}
                },
                _ = ping.tick() => {
                    if session.ping(b"").await.is_err() {
                        break;
                    }
                }
            }
        }
        hub_state.live.disconnected(uid);
        let _ = session.close(None).await;
    });
    Ok(response)
}

#[get("/shoutbox")]
pub async fn get_shoutbox(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    Ok(HttpResponse::Ok().json(history(&mut conn, user.id).await?))
}

#[derive(Deserialize)]
pub struct Brag {
    card: String,
    #[serde(default)]
    foil: bool,
}

#[derive(Deserialize)]
pub struct ShoutBody {
    #[serde(default)]
    body: String,
    /// Shows off a card the player owns
    card: Option<Brag>,
}

#[post("/shoutbox")]
pub async fn post_shout(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<ShoutBody>,
) -> ApiResult<HttpResponse> {
    let ShoutBody { body, card } = body.into_inner();
    let text = body.trim().to_owned();
    if text.chars().count() > MAX_BODY {
        return Err(ApiError::Invalid("message_too_long"));
    }
    if text.is_empty() && card.is_none() {
        return Err(ApiError::Invalid("empty_message"));
    }
    let mut conn = state.pool.get().await?;
    let now = Utc::now();
    let recent: Vec<chrono::DateTime<Utc>> = shout_messages::table
        .filter(shout_messages::user_id.eq(user.id))
        .filter(shout_messages::created_at.gt(now - chrono::Duration::minutes(1)))
        .order(shout_messages::created_at.desc())
        .select(shout_messages::created_at)
        .load(&mut conn)
        .await?;
    if recent.iter().max().is_some_and(|last| now - *last < MIN_GAP) || recent.len() as i64 >= PER_MINUTE {
        return Err(ApiError::TooMany("slow_down"));
    }

    // A brag shows a copy the player really has, with its own foil pattern
    let (card, foil) = match card {
        None => (None, None),
        Some(Brag { card, foil }) => {
            state.catalog.card(&card).ok_or(ApiError::NotFound("unknown_card"))?;
            let mine = owned_cards::table.filter(owned_cards::user_id.eq(user.id)).filter(owned_cards::card.eq(&card));
            let copy: Option<Option<String>> = if foil {
                mine.filter(owned_cards::foil.is_not_null())
                    .select(owned_cards::foil)
                    .first(&mut conn)
                    .await
                    .optional()?
            } else {
                mine.select(owned_cards::foil).first(&mut conn).await.optional()?
            };
            let pattern = copy.ok_or(ApiError::Conflict("not_owned"))?;
            (Some(card), if foil { pattern } else { None })
        }
    };
    let (id, at): (i64, chrono::DateTime<Utc>) = diesel::insert_into(shout_messages::table)
        .values((
            shout_messages::user_id.eq(user.id),
            shout_messages::body.eq(&text),
            shout_messages::card.eq(&card),
            shout_messages::foil.eq(&foil),
        ))
        .returning((shout_messages::id, shout_messages::created_at))
        .get_result(&mut conn)
        .await?;
    let event = ChatEvent { id, user: user.username, body: text, card, foil, at };
    state.metrics.shout();
    state.live.chat(event.clone());
    Ok(HttpResponse::Created().json(event))
}

#[derive(Deserialize)]
pub struct CardsQuery {
    /// Comma separated
    slugs: String,
}

#[derive(Serialize)]
struct CardsDto<'a>(HashMap<&'a str, CardDto<'a>>);

/// Several cards at once, for feeds that only carry slugs
#[get("/cards")]
pub async fn cards_batch(state: web::Data<AppState>, lang: Lang, q: web::Query<CardsQuery>) -> HttpResponse {
    let cat = &state.catalog;
    let map = q
        .slugs
        .split(',')
        .take(100)
        .filter_map(|s| cat.card(s.trim()))
        .map(|c| (c.slug.as_str(), CardDto::new(cat, c, lang)))
        .collect();
    HttpResponse::Ok().json(CardsDto(map))
}
