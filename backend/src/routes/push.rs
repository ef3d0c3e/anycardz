//! Web Push subscriptions, one per signed-in device, and the messages sent to
//! them when a notification arrives while the player is not on the site.

use actix_web::{HttpResponse, delete, get, put, web};
use chrono::Utc;
use diesel::prelude::*;
use diesel::upsert::excluded;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::i18n::Lang;
use crate::progress;
use crate::push::{self, Outcome, Target, Transport};
use crate::schema::push_subscriptions as ps;

/// The key browsers subscribe with, and whether this device is subscribed
#[get("/push")]
pub async fn status(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let on: i64 = ps::table.filter(ps::session_id.eq(user.session_id)).count().get_result(&mut conn).await?;
    let key = state.push.vapid.as_ref().map(|v| v.public_key());
    Ok(HttpResponse::Ok().json(serde_json::json!({ "key": key, "subscribed": on > 0 })))
}

#[derive(Deserialize)]
pub struct Keys {
    p256dh: String,
    auth: String,
}

/// `PushSubscription.toJSON()` as the browser gives it
#[derive(Deserialize)]
pub struct SubscribeBody {
    endpoint: String,
    keys: Keys,
}

/// Subscribes this device, replacing what it had
#[put("/push")]
pub async fn subscribe(
    state: web::Data<AppState>,
    user: CurrentUser,
    lang: Lang,
    body: web::Json<SubscribeBody>,
) -> ApiResult<HttpResponse> {
    if state.push.vapid.is_none() {
        return Err(ApiError::Conflict("push_disabled"));
    }
    let SubscribeBody { endpoint, keys } = body.into_inner();
    if !push::allowed_endpoint(&endpoint) {
        return Err(ApiError::Invalid("push_endpoint"));
    }
    if !push::valid_keys(&keys.p256dh, &keys.auth) {
        return Err(ApiError::Invalid("push_keys"));
    }
    let lang = match lang {
        Lang::En => "en",
        Lang::Fr => "fr",
    };
    let mut conn = state.pool.get().await?;
    // A browser keeps its endpoint across sign-ins: it moves to this session
    diesel::delete(ps::table.filter(ps::endpoint.eq(&endpoint).or(ps::session_id.eq(user.session_id))))
        .execute(&mut conn)
        .await?;
    diesel::insert_into(ps::table)
        .values((
            ps::user_id.eq(user.id),
            ps::session_id.eq(user.session_id),
            ps::endpoint.eq(&endpoint),
            ps::p256dh.eq(&keys.p256dh),
            ps::auth.eq(&keys.auth),
            ps::lang.eq(lang),
        ))
        .on_conflict(ps::session_id)
        .do_update()
        .set((
            ps::endpoint.eq(excluded(ps::endpoint)),
            ps::p256dh.eq(excluded(ps::p256dh)),
            ps::auth.eq(excluded(ps::auth)),
        ))
        .execute(&mut conn)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Stops notifications on this device
#[delete("/push")]
pub async fn unsubscribe(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    diesel::delete(ps::table.filter(ps::session_id.eq(user.session_id))).execute(&mut conn).await?;
    Ok(HttpResponse::NoContent().finish())
}

/// What a push says. The service worker shows it as is.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub title: String,
    pub body: String,
    /// Where a tap leads
    pub url: String,
    /// Replaces an earlier message with the same tag
    pub tag: String,
}

/// The fields of a notification a push is made from
pub struct Note<'a> {
    pub kind: &'a str,
    pub actor: Option<&'a str>,
    pub trade: Option<i64>,
    pub detail: Option<&'a str>,
}

/// The message for a notification in a language, or None for kinds that
/// do not warrant a push (a trade being edited)
pub fn message(state: &AppState, lang: Lang, n: &Note) -> Option<Message> {
    let fr = lang == Lang::Fr;
    let name = n.actor.unwrap_or("?");
    let trade_url = || n.trade.map(|t| format!("/trade/{t}")).unwrap_or_else(|| "/trade".into());
    let (body, url) = match n.kind {
        "friend_request" => (
            if fr {
                format!("{name} t’a envoyé une demande d’ami")
            } else {
                format!("{name} wants to be your friend")
            },
            "/friends".to_owned(),
        ),
        "friend_accepted" => (
            if fr {
                format!("{name} a accepté ta demande d’ami")
            } else {
                format!("{name} accepted your friend request")
            },
            format!("/players/{name}"),
        ),
        "trade_opened" => (
            if fr {
                format!("{name} a ouvert un échange avec toi")
            } else {
                format!("{name} opened a trade with you")
            },
            trade_url(),
        ),
        "trade_sealed" => (
            if fr {
                format!("{name} a scellé votre échange : à toi de jouer")
            } else {
                format!("{name} sealed your trade: your turn")
            },
            trade_url(),
        ),
        "trade_done" => (
            if fr {
                format!("Ton échange avec {name} est terminé")
            } else {
                format!("Your trade with {name} went through")
            },
            trade_url(),
        ),
        "trade_cancelled" => (
            if fr { format!("{name} a annulé votre échange") } else { format!("{name} cancelled your trade") },
            trade_url(),
        ),
        "market_sold" => {
            let net = n
                .detail
                .and_then(|d| serde_json::from_str::<serde_json::Value>(d).ok())
                .and_then(|d| d["net"].as_i64())
                .unwrap_or(0);
            (
                if fr {
                    format!("{name} a acheté une de tes cartes en vente · +{net} pièces")
                } else {
                    format!("{name} bought one of your listings · +{net} coins")
                },
                "/market#selling".to_owned(),
            )
        }
        "achievement" => {
            let key = n.detail.unwrap_or("");
            let set = |slug: &str| {
                state.catalog.collection(slug).map(|c| c.texts.get(lang, "title").to_owned()).unwrap_or(slug.to_owned())
            };
            if let Some(slug) = key.strip_prefix("foilset:") {
                let t = set(slug);
                (
                    if fr { format!("Set brillant complété : {t}") } else { format!("Foil set complete: {t}") },
                    format!("/binder?set={slug}"),
                )
            } else if let Some(slug) = key.strip_prefix("set:") {
                let t = set(slug);
                (
                    if fr { format!("Set complété : {t}") } else { format!("Set complete: {t}") },
                    format!("/binder?set={slug}"),
                )
            } else {
                let coins = progress::achievement(key).map(|a| a.coins).unwrap_or(0);
                (
                    if fr {
                        format!("Succès débloqué · +{coins} pièces")
                    } else {
                        format!("Achievement unlocked · +{coins} coins")
                    },
                    "/achievements".to_owned(),
                )
            }
        }
        "mission" => {
            let detail = n.detail.unwrap_or("");
            if detail.starts_with("bonus") {
                (
                    if fr {
                        "Les trois missions du jour sont faites : un booster gratuit t’attend".to_owned()
                    } else {
                        "All three daily missions done: a free pack is in your pile".to_owned()
                    },
                    "/open".to_owned(),
                )
            } else {
                let coins = detail.rsplit(':').next().and_then(|c| c.parse::<i64>().ok()).unwrap_or(0);
                (
                    if fr {
                        format!("Mission du jour accomplie · +{coins} pièces")
                    } else {
                        format!("Daily mission done · +{coins} coins")
                    },
                    "/".to_owned(),
                )
            }
        }
        _ => return None,
    };
    Some(Message { title: "AnyCardz".into(), body, url, tag: format!("{}-{}", n.kind, n.trade.unwrap_or(0)) })
}

type SubRow = (i64, String, String, String, String);

/// Pushes a notification to the player's devices, when no page of theirs is
/// open (they saw it live then). Delivery runs in the background.
pub async fn notify(state: &AppState, conn: &mut AsyncPgConnection, user: i64, note: Note<'_>) -> ApiResult<()> {
    if state.push.vapid.is_none() || state.live.is_online(user) {
        return Ok(());
    }
    let subs: Vec<SubRow> = ps::table
        .filter(ps::user_id.eq(user))
        .select((ps::id, ps::endpoint, ps::p256dh, ps::auth, ps::lang))
        .load(conn)
        .await?;
    let now = Utc::now().timestamp();
    let mut out = Vec::new();
    for (id, endpoint, p256dh, auth, lang) in subs {
        let lang = Lang::parse(&lang).unwrap_or(Lang::En);
        let Some(msg) = message(state, lang, &note) else { return Ok(()) };
        let payload = serde_json::to_vec(&msg).expect("messages serialize");
        if let Some(d) = state.push.prepare(&Target { endpoint, p256dh, auth }, &payload, now) {
            out.push((id, d));
        }
    }
    if out.is_empty() {
        return Ok(());
    }
    let memory = matches!(state.push.transport, Transport::Memory(_));
    let push = state.push.clone();
    let pool = state.pool.clone();
    let counter = state.metrics.push_counter();
    let run = async move {
        for (id, d) in out {
            let outcome = push.transport.send(d).await;
            counter
                .get_or_create(&crate::metrics::ResultLabels::new(match outcome {
                    Outcome::Sent => "sent",
                    Outcome::Gone => "gone",
                    Outcome::Failed => "failed",
                }))
                .inc();
            if outcome == Outcome::Gone
                && let Ok(mut conn) = pool.get().await
            {
                let _ = diesel::delete(ps::table.find(id)).execute(&mut conn).await;
            }
        }
    };
    // Tests read deliveries right after the request: no background there
    if memory {
        run.await;
    } else {
        actix_web::rt::spawn(run);
    }
    Ok(())
}
