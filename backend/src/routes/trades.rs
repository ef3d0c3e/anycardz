//! Trades between two players, with seals.
//!
//! Each offer has a `version` that grows with every edit. Sealing means "I agree
//! to version N"; an edit clears both seals, so nobody can swap a card at the last
//! second. When the second seal lands on the same version, the swap runs at once.
//! Cards stay in the binders until then: the swap checks that both players still
//! have everything, and fails cleanly when they do not.

use std::collections::HashMap;

use actix_web::{HttpResponse, get, post, put, web};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::data::Catalog;
use crate::error::{ApiError, ApiResult};
use crate::i18n::Lang;
use crate::inventory::{self, Copies};
use crate::routes::catalog::CardDto;
use crate::routes::notifications::{Kind, notify};
use crate::schema::{owned_cards, trade_lines, trades, users};

/// Cards one side may put on the table
const MAX_CARDS_PER_SIDE: i64 = 50;
const MAX_COINS: i64 = 1_000_000_000;

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = trades)]
struct Trade {
    id: i64,
    from_id: i64,
    to_id: i64,
    state: String,
    version: i32,
    from_coins: i64,
    to_coins: i64,
    from_sealed: Option<i32>,
    to_sealed: Option<i32>,
    edited_by: Option<i64>,
    updated_at: DateTime<Utc>,
}

impl Trade {
    fn is_from(&self, uid: i64) -> bool {
        self.from_id == uid
    }
    fn partner(&self, uid: i64) -> i64 {
        if self.is_from(uid) { self.to_id } else { self.from_id }
    }
}

#[derive(Debug, Clone, Queryable)]
struct Line {
    side: String,
    card: String,
    foil: bool,
    count: i32,
}

#[derive(Serialize)]
struct SideDto {
    coins: i64,
    cards: Vec<Copies>,
    sealed: bool,
}

#[derive(Serialize)]
struct TradeDto<'a> {
    id: i64,
    version: i32,
    state: String,
    partner: String,
    /// What the caller gives
    mine: SideDto,
    /// What the partner gives
    theirs: SideDto,
    /// Who made the last change: "me", "partner", or null for a new trade
    edited_by: Option<&'static str>,
    updated_at: DateTime<Utc>,
    details: HashMap<&'a str, CardDto<'a>>,
}

/// The trade, when the caller takes part in it
async fn find(conn: &mut AsyncPgConnection, id: i64, uid: i64, lock: bool) -> ApiResult<Trade> {
    let q = trades::table.find(id).filter(trades::from_id.eq(uid).or(trades::to_id.eq(uid))).select(Trade::as_select());
    // NO KEY UPDATE: the trade's key never changes, and foreign keys pointing at it
    // (notifications) only take KEY SHARE locks, which this does not block
    let t = if lock { q.for_no_key_update().first(conn).await } else { q.first(conn).await };
    t.optional()?.ok_or(ApiError::NotFound("unknown_trade"))
}

async fn lines(conn: &mut AsyncPgConnection, id: i64) -> ApiResult<Vec<Line>> {
    Ok(trade_lines::table
        .filter(trade_lines::trade_id.eq(id))
        .order(trade_lines::id.asc())
        .select((trade_lines::side, trade_lines::card, trade_lines::foil, trade_lines::count))
        .load(conn)
        .await?)
}

async fn username(conn: &mut AsyncPgConnection, id: i64) -> ApiResult<String> {
    Ok(users::table.find(id).select(users::username).first(conn).await?)
}

async fn dto<'a>(
    conn: &mut AsyncPgConnection,
    catalog: &'a Catalog,
    lang: Lang,
    t: Trade,
    uid: i64,
) -> ApiResult<TradeDto<'a>> {
    let ls = lines(conn, t.id).await?;
    let partner = username(conn, t.partner(uid)).await?;
    let (my_side, their_side) = if t.is_from(uid) { ("from", "to") } else { ("to", "from") };
    let side = |s: &str| -> Vec<Copies> {
        ls.iter()
            .filter(|l| l.side == s)
            .map(|l| Copies { card: l.card.clone(), foil: l.foil, count: l.count as i64 })
            .collect()
    };
    let sealed = |seal: Option<i32>| seal == Some(t.version);
    let (my_coins, their_coins, my_seal, their_seal) = if t.is_from(uid) {
        (t.from_coins, t.to_coins, t.from_sealed, t.to_sealed)
    } else {
        (t.to_coins, t.from_coins, t.to_sealed, t.from_sealed)
    };
    let details = ls
        .iter()
        .filter_map(|l| catalog.card(&l.card))
        .map(|c| (c.slug.as_str(), CardDto::new(catalog, c, lang)))
        .collect();
    Ok(TradeDto {
        id: t.id,
        version: t.version,
        state: t.state.clone(),
        partner,
        mine: SideDto { coins: my_coins, cards: side(my_side), sealed: sealed(my_seal) },
        theirs: SideDto { coins: their_coins, cards: side(their_side), sealed: sealed(their_seal) },
        edited_by: t.edited_by.map(|e| if e == uid { "me" } else { "partner" }),
        updated_at: t.updated_at,
        details,
    })
}

#[derive(Deserialize)]
pub struct OpenBody {
    with: String,
}

/// Opens a trade with a player, or returns the open one with them
#[post("/trades")]
pub async fn open_trade(
    state: web::Data<AppState>,
    user: CurrentUser,
    lang: Lang,
    body: web::Json<OpenBody>,
) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let partner: i64 = users::table
        .filter(super::players::lower(users::username).eq(body.with.trim().to_lowercase()))
        .select(users::id)
        .first(&mut conn)
        .await
        .optional()?
        .ok_or(ApiError::NotFound("unknown_player"))?;
    if partner == user.id {
        return Err(ApiError::Invalid("trade_self"));
    }
    if super::social::blocked_between(&mut conn, user.id, partner).await? {
        return Err(ApiError::Conflict("player_unavailable"));
    }
    let existing: Option<i64> = trades::table
        .filter(trades::state.eq("open"))
        .filter(
            (trades::from_id.eq(user.id).and(trades::to_id.eq(partner)))
                .or(trades::from_id.eq(partner).and(trades::to_id.eq(user.id))),
        )
        .select(trades::id)
        .first(&mut conn)
        .await
        .optional()?;
    let id = match existing {
        Some(id) => id,
        None => {
            let inserted = diesel::insert_into(trades::table)
                .values((trades::from_id.eq(user.id), trades::to_id.eq(partner)))
                .returning(trades::id)
                .get_result::<i64>(&mut conn)
                .await;
            match inserted {
                Ok(id) => id,
                // Both players opened a trade with each other at the same moment
                Err(diesel::result::Error::DatabaseError(diesel::result::DatabaseErrorKind::UniqueViolation, _)) => {
                    trades::table
                        .filter(trades::state.eq("open"))
                        .filter(trades::from_id.eq(partner).and(trades::to_id.eq(user.id)))
                        .select(trades::id)
                        .first(&mut conn)
                        .await?
                }
                Err(e) => return Err(e.into()),
            }
        }
    };
    let t = find(&mut conn, id, user.id, false).await?;
    state.live.trade(t.id, [t.from_id, t.to_id]);
    if existing.is_none() {
        notify(&state, &mut conn, t.partner(user.id), Kind::TradeOpened, user.id, Some(t.id)).await?;
    }
    let status = if existing.is_some() { 200 } else { 201 };
    let body = dto(&mut conn, &state.catalog, lang, t, user.id).await?;
    Ok(HttpResponse::build(actix_web::http::StatusCode::from_u16(status).unwrap()).json(body))
}

#[derive(Serialize)]
struct TradeSummary {
    id: i64,
    partner: String,
    state: String,
    /// Cards each side gives
    mine: i64,
    theirs: i64,
    you_sealed: bool,
    they_sealed: bool,
    edited_by: Option<&'static str>,
    updated_at: DateTime<Utc>,
}

/// Open trades first, then the last closed ones
#[get("/trades")]
pub async fn list_trades(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let uid = user.id;
    let rows: Vec<Trade> = trades::table
        .filter(trades::from_id.eq(uid).or(trades::to_id.eq(uid)))
        .order((diesel::dsl::sql::<diesel::sql_types::Bool>("state = 'open'").desc(), trades::updated_at.desc()))
        .limit(30)
        .select(Trade::as_select())
        .load(&mut conn)
        .await?;
    let ids: Vec<i64> = rows.iter().map(|t| t.id).collect();
    let partner_ids: Vec<i64> = rows.iter().map(|t| t.partner(uid)).collect();
    let names: HashMap<i64, String> = users::table
        .filter(users::id.eq_any(&partner_ids))
        .select((users::id, users::username))
        .load::<(i64, String)>(&mut conn)
        .await?
        .into_iter()
        .collect();
    let counts: Vec<(i64, String, i32)> = trade_lines::table
        .filter(trade_lines::trade_id.eq_any(&ids))
        .select((trade_lines::trade_id, trade_lines::side, trade_lines::count))
        .load(&mut conn)
        .await?;
    let list: Vec<TradeSummary> = rows
        .into_iter()
        .map(|t| {
            let mine_side = if t.is_from(uid) { "from" } else { "to" };
            let (mut mine, mut theirs) = (0, 0);
            for (_, side, n) in counts.iter().filter(|(id, _, _)| *id == t.id) {
                if side == mine_side { mine += *n as i64 } else { theirs += *n as i64 }
            }
            let (my_seal, their_seal) =
                if t.is_from(uid) { (t.from_sealed, t.to_sealed) } else { (t.to_sealed, t.from_sealed) };
            TradeSummary {
                id: t.id,
                partner: names.get(&t.partner(uid)).cloned().unwrap_or_default(),
                state: t.state.clone(),
                mine,
                theirs,
                you_sealed: my_seal == Some(t.version),
                they_sealed: their_seal == Some(t.version),
                edited_by: t.edited_by.map(|e| if e == uid { "me" } else { "partner" }),
                updated_at: t.updated_at,
            }
        })
        .collect();
    Ok(HttpResponse::Ok().json(list))
}

#[get("/trades/{id}")]
pub async fn get_trade(
    state: web::Data<AppState>,
    user: CurrentUser,
    lang: Lang,
    id: web::Path<i64>,
) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let t = find(&mut conn, *id, user.id, false).await?;
    Ok(HttpResponse::Ok().json(dto(&mut conn, &state.catalog, lang, t, user.id).await?))
}

#[derive(Deserialize)]
pub struct EditBody {
    /// The version the edit is based on: an edit of an older one is refused
    version: i32,
    #[serde(default)]
    give: Vec<Copies>,
    #[serde(default)]
    get: Vec<Copies>,
    #[serde(default)]
    give_coins: i64,
    #[serde(default)]
    get_coins: i64,
}

/// Owned copies per (card, foil) among some cards
async fn holdings(conn: &mut AsyncPgConnection, uid: i64, cards: &[&str]) -> ApiResult<HashMap<(String, bool), i64>> {
    let rows: Vec<(String, Option<String>)> = owned_cards::table
        .filter(owned_cards::user_id.eq(uid))
        .filter(owned_cards::card.eq_any(cards))
        .select((owned_cards::card, owned_cards::foil))
        .load(conn)
        .await?;
    let mut out = HashMap::new();
    for (card, foil) in rows {
        *out.entry((card, foil.is_some())).or_default() += 1;
    }
    Ok(out)
}

async fn check_owned(conn: &mut AsyncPgConnection, uid: i64, list: &[Copies]) -> ApiResult<()> {
    let cards: Vec<&str> = list.iter().map(|c| c.card.as_str()).collect();
    let have = holdings(conn, uid, &cards).await?;
    if list.iter().any(|c| have.get(&(c.card.clone(), c.foil)).copied().unwrap_or(0) < c.count) {
        return Err(ApiError::Conflict("trade_not_owned"));
    }
    Ok(())
}

/// Replaces the whole offer. Clears both seals.
#[put("/trades/{id}")]
pub async fn edit_trade(
    state: web::Data<AppState>,
    user: CurrentUser,
    lang: Lang,
    id: web::Path<i64>,
    body: web::Json<EditBody>,
) -> ApiResult<HttpResponse> {
    let EditBody { version, give, get, give_coins, get_coins } = body.into_inner();
    let catalog = state.catalog.clone();
    let give = inventory::normalize(&catalog, &give)?;
    let get = inventory::normalize(&catalog, &get)?;
    let total = |l: &[Copies]| l.iter().map(|c| c.count).sum::<i64>();
    if total(&give) > MAX_CARDS_PER_SIDE || total(&get) > MAX_CARDS_PER_SIDE {
        return Err(ApiError::Invalid("trade_too_big"));
    }
    if !(0..=MAX_COINS).contains(&give_coins) || !(0..=MAX_COINS).contains(&get_coins) {
        return Err(ApiError::Invalid("invalid_coins"));
    }
    let mut conn = state.pool.get().await?;
    let uid = user.id;
    let id = *id;
    conn.transaction::<_, ApiError, _>(async move |conn| {
        let t = find(conn, id, uid, true).await?;
        if t.state != "open" {
            return Err(ApiError::Conflict("trade_closed"));
        }
        if t.version != version {
            return Err(ApiError::Conflict("trade_changed"));
        }
        // Early checks for a clear message; the swap checks again
        check_owned(conn, uid, &give).await?;
        check_owned(conn, t.partner(uid), &get).await?;

        let (from_lines, to_lines, from_coins, to_coins) =
            if t.is_from(uid) { (&give, &get, give_coins, get_coins) } else { (&get, &give, get_coins, give_coins) };
        diesel::delete(trade_lines::table.filter(trade_lines::trade_id.eq(id))).execute(conn).await?;
        let rows: Vec<_> = from_lines
            .iter()
            .map(|c| ("from", c))
            .chain(to_lines.iter().map(|c| ("to", c)))
            .map(|(side, c)| {
                (
                    trade_lines::trade_id.eq(id),
                    trade_lines::side.eq(side),
                    trade_lines::card.eq(&c.card),
                    trade_lines::foil.eq(c.foil),
                    trade_lines::count.eq(c.count as i32),
                )
            })
            .collect();
        diesel::insert_into(trade_lines::table).values(rows).execute(conn).await?;
        diesel::update(trades::table.find(id))
            .set((
                trades::version.eq(trades::version + 1),
                trades::from_coins.eq(from_coins),
                trades::to_coins.eq(to_coins),
                trades::from_sealed.eq(None::<i32>),
                trades::to_sealed.eq(None::<i32>),
                trades::edited_by.eq(uid),
                trades::updated_at.eq(Utc::now()),
            ))
            .execute(conn)
            .await?;
        Ok(())
    })
    .await?;
    let t = find(&mut conn, id, uid, false).await?;
    state.live.trade(t.id, [t.from_id, t.to_id]);
    notify(&state, &mut conn, t.partner(uid), Kind::TradeChanged, uid, Some(t.id)).await?;
    Ok(HttpResponse::Ok().json(dto(&mut conn, &state.catalog, lang, t, uid).await?))
}

#[derive(Deserialize)]
pub struct SealBody {
    version: i32,
}

/// Seals the caller's side of a version. The second seal on it swaps.
#[post("/trades/{id}/seal")]
pub async fn seal_trade(
    state: web::Data<AppState>,
    user: CurrentUser,
    lang: Lang,
    id: web::Path<i64>,
    body: web::Json<SealBody>,
) -> ApiResult<HttpResponse> {
    let catalog = state.catalog.clone();
    let mut conn = state.pool.get().await?;
    let (uid, id, version) = (user.id, *id, body.version);
    conn.transaction::<_, ApiError, _>(async move |conn| {
        // The row lock serializes both players' seals: exactly one of them swaps
        let t = find(conn, id, uid, true).await?;
        if t.state != "open" {
            return Err(ApiError::Conflict("trade_closed"));
        }
        if t.version != version {
            return Err(ApiError::Conflict("trade_changed"));
        }
        let ls = lines(conn, id).await?;
        if ls.is_empty() && t.from_coins == 0 && t.to_coins == 0 {
            return Err(ApiError::Invalid("trade_empty"));
        }
        let partner_sealed = if t.is_from(uid) { t.to_sealed } else { t.from_sealed } == Some(version);
        // Each seal column is its own type: one branch per side
        let now = Utc::now();
        if t.is_from(uid) {
            diesel::update(trades::table.find(id))
                .set((trades::from_sealed.eq(version), trades::updated_at.eq(now)))
                .execute(conn)
                .await?;
        } else {
            diesel::update(trades::table.find(id))
                .set((trades::to_sealed.eq(version), trades::updated_at.eq(now)))
                .execute(conn)
                .await?;
        }
        if partner_sealed {
            swap(conn, &catalog, &t, &ls).await?;
        }
        Ok(())
    })
    .await?;
    let t = find(&mut conn, id, uid, false).await?;
    state.live.trade(t.id, [t.from_id, t.to_id]);
    let kind = if t.state == "done" { Kind::TradeDone } else { Kind::TradeSealed };
    if t.state == "done" {
        state.metrics.trade("done");
        // Each player may have just completed a set with what they received
        let ls = lines(&mut conn, t.id).await?;
        let names: Vec<(i64, String)> = crate::schema::users::table
            .filter(crate::schema::users::id.eq_any([t.from_id, t.to_id]))
            .select((crate::schema::users::id, crate::schema::users::username))
            .load(&mut conn)
            .await?;
        for (receiver, given_by) in [(t.to_id, "from"), (t.from_id, "to")] {
            let Some((_, name)) = names.iter().find(|(id, _)| *id == receiver) else { continue };
            let got = ls.iter().filter(|l| l.side == given_by).map(|l| l.card.as_str());
            crate::progress::cards_gained(&state, receiver, name, got).await?;
            crate::progress::award(&state, receiver, name, &[], &[crate::progress::Metric::Trades]).await?;
            crate::missions::progress(&state, receiver, crate::missions::Event::Trade, 1).await;
        }
    }
    notify(&state, &mut conn, t.partner(uid), kind, uid, Some(t.id)).await?;
    Ok(HttpResponse::Ok().json(dto(&mut conn, &state.catalog, lang, t, uid).await?))
}

/// Moves the cards and coins of both sides, or nothing
async fn swap(conn: &mut AsyncPgConnection, catalog: &Catalog, t: &Trade, ls: &[Line]) -> ApiResult<()> {
    // Both players' rows, in id order: concurrent swaps of the same players queue up
    let _: Vec<i64> = users::table
        .filter(users::id.eq_any([t.from_id, t.to_id]))
        .order(users::id.asc())
        .select(users::id)
        // Coins change, keys do not: KEY SHARE locks from foreign key checks
        // (a notification being written) go through instead of deadlocking
        .for_no_key_update()
        .load(conn)
        .await?;
    let unavailable = |e: ApiError| match e {
        ApiError::Conflict("not_owned") => ApiError::Conflict("trade_unavailable"),
        e => e,
    };
    for (side, owner, receiver) in [("from", t.from_id, t.to_id), ("to", t.to_id, t.from_id)] {
        let copies: Vec<Copies> = ls
            .iter()
            .filter(|l| l.side == side)
            .map(|l| Copies { card: l.card.clone(), foil: l.foil, count: l.count as i64 })
            .collect();
        for c in inventory::normalize(catalog, &copies)? {
            let ids = inventory::lock_copies(conn, owner, &c).await.map_err(unavailable)?;
            diesel::update(owned_cards::table.filter(owned_cards::id.eq_any(&ids)))
                .set((owned_cards::user_id.eq(receiver), owned_cards::obtained_at.eq(Utc::now())))
                .execute(conn)
                .await?;
        }
    }
    for (payer, payee, amount) in [(t.from_id, t.to_id, t.from_coins), (t.to_id, t.from_id, t.to_coins)] {
        if amount == 0 {
            continue;
        }
        let paid = diesel::update(users::table.find(payer).filter(users::coins.ge(amount)))
            .set(users::coins.eq(users::coins - amount))
            .execute(conn)
            .await?;
        if paid == 0 {
            return Err(ApiError::Conflict("trade_unavailable"));
        }
        diesel::update(users::table.find(payee)).set(users::coins.eq(users::coins + amount)).execute(conn).await?;
    }
    diesel::update(trades::table.find(t.id))
        .set((trades::state.eq("done"), trades::updated_at.eq(Utc::now())))
        .execute(conn)
        .await?;
    Ok(())
}

#[post("/trades/{id}/cancel")]
pub async fn cancel_trade(
    state: web::Data<AppState>,
    user: CurrentUser,
    lang: Lang,
    id: web::Path<i64>,
) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let (uid, id) = (user.id, *id);
    conn.transaction::<_, ApiError, _>(async move |conn| {
        let t = find(conn, id, uid, true).await?;
        if t.state != "open" {
            return Err(ApiError::Conflict("trade_closed"));
        }
        diesel::update(trades::table.find(id))
            .set((trades::state.eq("cancelled"), trades::edited_by.eq(uid), trades::updated_at.eq(Utc::now())))
            .execute(conn)
            .await?;
        Ok(())
    })
    .await?;
    state.metrics.trade("cancelled");
    let t = find(&mut conn, id, uid, false).await?;
    state.live.trade(t.id, [t.from_id, t.to_id]);
    notify(&state, &mut conn, t.partner(uid), Kind::TradeCancelled, uid, Some(t.id)).await?;
    Ok(HttpResponse::Ok().json(dto(&mut conn, &state.catalog, lang, t, uid).await?))
}
