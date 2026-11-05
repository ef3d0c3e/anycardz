//! The marketplace: fixed-price listings of single copies, bought at once.
//!
//! A listed copy leaves the seller's binder for the listing (escrow), so
//! trades, the forge and selling never see it. Withdrawing puts it back as it
//! was; buying moves it to the buyer. Each sale burns a small fee, and prices
//! and purchases per seller are bounded so listings are a poor way to move
//! coins between one's own accounts.

use actix_web::{HttpResponse, delete, get, post, web};
use chrono::{DateTime, Duration, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::data::Rarity;
use crate::error::{ApiError, ApiResult};
use crate::game::{self, MARKET_FEE_PERCENT, MARKET_MAX_LISTINGS, MARKET_PAIR_LIMIT};
use crate::i18n::Lang;
use crate::inventory::{self, Copies};
use crate::progress::{self, Unlock};
use crate::routes::catalog::CardDto;
use crate::routes::notifications;
use crate::routes::social::{avatar_url, blocked_between, blocked_by};
use crate::schema::{blocks, listings, owned_cards, users};

const PER_PAGE: i64 = 24;
/// Sales shown in the seller's history
const HISTORY: i64 = 20;

#[derive(Serialize)]
struct ListingDto<'a> {
    id: i64,
    card: CardDto<'a>,
    foil: Option<String>,
    price: i64,
    seller: String,
    seller_avatar: Option<String>,
    created_at: DateTime<Utc>,
    mine: bool,
}

type ListingRow = (i64, String, Option<String>, i64, DateTime<Utc>, i64, String, i32, bool);

fn listing_cols() -> (
    listings::id,
    listings::card,
    listings::foil,
    listings::price,
    listings::created_at,
    listings::seller_id,
    users::username,
    users::avatar_version,
    diesel::dsl::IsNotNull<users::avatar>,
) {
    (
        listings::id,
        listings::card,
        listings::foil,
        listings::price,
        listings::created_at,
        listings::seller_id,
        users::username,
        users::avatar_version,
        users::avatar.is_not_null(),
    )
}

fn dtos<'a>(state: &'a AppState, lang: Lang, uid: i64, rows: Vec<ListingRow>) -> Vec<ListingDto<'a>> {
    rows.into_iter()
        .filter_map(|(id, card, foil, price, created_at, seller_id, name, v, has)| {
            let c = state.catalog.card(&card)?;
            Some(ListingDto {
                id,
                card: CardDto::new(&state.catalog, c, lang),
                foil,
                price,
                seller_avatar: avatar_url(&name, v, has),
                seller: name,
                created_at,
                mine: seller_id == uid,
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    Price,
    New,
}

#[derive(Deserialize)]
pub struct BrowseQuery {
    /// Words of the card name
    q: Option<String>,
    /// Exactly this card, for the card inspector
    card: Option<String>,
    collection: Option<String>,
    rarity: Option<Rarity>,
    foil: Option<bool>,
    sort: Option<Sort>,
    #[serde(default)]
    page: i64,
}

/// Open listings by other players, cheapest first
#[get("/market")]
pub async fn browse(
    state: web::Data<AppState>,
    user: CurrentUser,
    lang: Lang,
    q: web::Query<BrowseQuery>,
) -> ApiResult<HttpResponse> {
    let cat = &state.catalog;
    let words = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_lowercase);
    // Name, set and rarity are catalog facts: they become a list of card slugs
    let slugs: Option<Vec<String>> =
        if words.is_some() || q.card.is_some() || q.collection.is_some() || q.rarity.is_some() {
            let set = match &q.collection {
                Some(s) => Some(cat.collection(s).ok_or(ApiError::NotFound("unknown_collection"))?),
                None => None,
            };
            Some(
                cat.cards
                    .values()
                    .filter(|c| q.card.as_ref().is_none_or(|s| &c.slug == s))
                    .filter(|c| q.rarity.is_none_or(|r| c.rarity == r))
                    .filter(|c| set.is_none_or(|s| s.cards.contains(&c.slug)))
                    .filter(|c| {
                        words.as_ref().is_none_or(|w| {
                            [Lang::En, lang]
                                .iter()
                                .any(|l| c.texts.get(*l, "title").to_lowercase().contains(w.as_str()))
                        })
                    })
                    .map(|c| c.slug.clone())
                    .collect(),
            )
        } else {
            None
        };
    let mut conn = state.pool.get().await?;
    // Players who blocked the viewer, or whom the viewer blocked, stay out of sight
    let mut hidden = blocked_by(&mut conn, user.id).await?;
    hidden.extend(
        blocks::table.filter(blocks::blocked_id.eq(user.id)).select(blocks::user_id).load::<i64>(&mut conn).await?,
    );
    let base = || {
        let mut b = listings::table
            .filter(listings::state.eq("open"))
            .filter(listings::seller_id.ne(user.id))
            .filter(listings::seller_id.ne_all(hidden.clone()))
            .into_boxed();
        if let Some(s) = &slugs {
            b = b.filter(listings::card.eq_any(s.clone()));
        }
        match q.foil {
            Some(true) => b = b.filter(listings::foil.is_not_null()),
            Some(false) => b = b.filter(listings::foil.is_null()),
            None => {}
        }
        b
    };
    let total: i64 = base().count().get_result(&mut conn).await?;
    let page = q.page.max(0);
    let mut rows = base().inner_join(users::table.on(users::id.eq(listings::seller_id))).select(listing_cols());
    rows = match q.sort.unwrap_or(Sort::Price) {
        Sort::Price => rows.order((listings::price.asc(), listings::id.asc())),
        Sort::New => rows.order(listings::id.desc()),
    };
    let rows: Vec<ListingRow> = rows.limit(PER_PAGE).offset(page * PER_PAGE).load(&mut conn).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "items": dtos(&state, lang, user.id, rows),
        "total": total,
        "page": page,
        "per_page": PER_PAGE,
    })))
}

#[derive(Serialize)]
struct SaleDto<'a> {
    id: i64,
    card: CardDto<'a>,
    foil: Option<String>,
    price: i64,
    fee: i64,
    buyer: Option<String>,
    at: Option<DateTime<Utc>>,
}

type SaleRow = (i64, String, Option<String>, i64, i64, Option<String>, Option<DateTime<Utc>>);

/// The player's open listings and recent sales
#[get("/market/mine")]
pub async fn mine(state: web::Data<AppState>, user: CurrentUser, lang: Lang) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let open: Vec<ListingRow> = listings::table
        .inner_join(users::table.on(users::id.eq(listings::seller_id)))
        .filter(listings::seller_id.eq(user.id))
        .filter(listings::state.eq("open"))
        .order(listings::id.desc())
        .select(listing_cols())
        .load(&mut conn)
        .await?;
    let sold: Vec<SaleRow> = listings::table
        .left_join(users::table.on(listings::buyer_id.eq(users::id.nullable())))
        .filter(listings::seller_id.eq(user.id))
        .filter(listings::state.eq("sold"))
        .order(listings::closed_at.desc())
        .limit(HISTORY)
        .select((
            listings::id,
            listings::card,
            listings::foil,
            listings::price,
            listings::fee,
            users::username.nullable(),
            listings::closed_at,
        ))
        .load(&mut conn)
        .await?;
    let sold: Vec<SaleDto> = sold
        .into_iter()
        .filter_map(|(id, card, foil, price, fee, buyer, at)| {
            let c = state.catalog.card(&card)?;
            Some(SaleDto { id, card: CardDto::new(&state.catalog, c, lang), foil, price, fee, buyer, at })
        })
        .collect();
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "open": dtos(&state, lang, user.id, open),
        "sold": sold,
        "max_listings": MARKET_MAX_LISTINGS,
        "fee_percent": MARKET_FEE_PERCENT,
    })))
}

#[derive(Deserialize)]
pub struct ListBody {
    card: String,
    #[serde(default)]
    foil: bool,
    price: i64,
}

/// Puts one copy up for sale
#[post("/market")]
pub async fn list(
    state: web::Data<AppState>,
    user: CurrentUser,
    lang: Lang,
    body: web::Json<ListBody>,
) -> ApiResult<HttpResponse> {
    let ListBody { card, foil, price } = body.into_inner();
    let rarity = state.catalog.card(&card).ok_or(ApiError::NotFound("unknown_card"))?.rarity;
    let (min, max) = game::market_bounds(rarity, foil);
    if !(min..=max).contains(&price) {
        return Err(ApiError::Invalid("market_price"));
    }
    let mut conn = state.pool.get().await?;
    let uid = user.id;
    let slug = card.clone();
    let (id, remaining) = conn
        .transaction::<_, ApiError, _>(async move |conn| {
            // One listing at a time per player: the count below stays true
            users::table.find(uid).select(users::id).for_update().get_result::<i64>(conn).await?;
            let open: i64 = listings::table
                .filter(listings::seller_id.eq(uid))
                .filter(listings::state.eq("open"))
                .count()
                .get_result(conn)
                .await?;
            if open >= MARKET_MAX_LISTINGS {
                return Err(ApiError::Conflict("market_full"));
            }
            let ids = inventory::lock_copies(conn, uid, &Copies { card: slug.clone(), foil, count: 1 }).await?;
            let (pattern, obtained): (Option<String>, DateTime<Utc>) = owned_cards::table
                .find(ids[0])
                .select((owned_cards::foil, owned_cards::obtained_at))
                .get_result(conn)
                .await?;
            inventory::delete_copies(conn, &ids).await?;
            let id: i64 = diesel::insert_into(listings::table)
                .values((
                    listings::seller_id.eq(uid),
                    listings::card.eq(&slug),
                    listings::foil.eq(&pattern),
                    listings::obtained_at.eq(obtained),
                    listings::price.eq(price),
                ))
                .returning(listings::id)
                .get_result(conn)
                .await?;
            let remaining = inventory::count(conn, uid, &slug).await?;
            Ok((id, remaining))
        })
        .await?;
    state.metrics.market("listed");
    crate::missions::progress(&state, uid, crate::missions::Event::MarketList, 1).await;
    let row: ListingRow = listings::table
        .inner_join(users::table.on(users::id.eq(listings::seller_id)))
        .filter(listings::id.eq(id))
        .select(listing_cols())
        .get_result(&mut conn)
        .await?;
    let listing = dtos(&state, lang, uid, vec![row]).pop();
    Ok(HttpResponse::Created().json(serde_json::json!({ "listing": listing, "remaining": remaining })))
}

/// Takes a listing down: the copy goes back to the binder as it was
#[delete("/market/{id}")]
pub async fn withdraw(state: web::Data<AppState>, user: CurrentUser, id: web::Path<i64>) -> ApiResult<HttpResponse> {
    let id = id.into_inner();
    let uid = user.id;
    let mut conn = state.pool.get().await?;
    conn.transaction::<_, ApiError, _>(async move |conn| {
        let (card, foil, obtained): (String, Option<String>, DateTime<Utc>) = listings::table
            .filter(listings::id.eq(id))
            .filter(listings::seller_id.eq(uid))
            .filter(listings::state.eq("open"))
            .select((listings::card, listings::foil, listings::obtained_at))
            .for_update()
            .get_result(conn)
            .await
            .optional()?
            .ok_or(ApiError::NotFound("unknown_listing"))?;
        diesel::update(listings::table.find(id))
            .set((listings::state.eq("withdrawn"), listings::closed_at.eq(Utc::now())))
            .execute(conn)
            .await?;
        diesel::insert_into(owned_cards::table)
            .values((
                owned_cards::user_id.eq(uid),
                owned_cards::card.eq(card),
                owned_cards::foil.eq(foil),
                owned_cards::obtained_at.eq(obtained),
            ))
            .execute(conn)
            .await?;
        Ok(())
    })
    .await?;
    state.metrics.market("withdrawn");
    Ok(HttpResponse::NoContent().finish())
}

/// Coins moved by one purchase
async fn pay(conn: &mut AsyncPgConnection, buyer: i64, seller: i64, price: i64, fee: i64) -> ApiResult<i64> {
    // Both rows in id order: two players buying from each other never deadlock
    let mut ids = [buyer, seller];
    ids.sort();
    users::table.filter(users::id.eq_any(ids)).select(users::id).for_update().load::<i64>(conn).await?;
    let coins: i64 = diesel::update(users::table.find(buyer).filter(users::coins.ge(price)))
        .set(users::coins.eq(users::coins - price))
        .returning(users::coins)
        .get_result(conn)
        .await
        .optional()?
        .ok_or(ApiError::Conflict("not_enough_coins"))?;
    diesel::update(users::table.find(seller)).set(users::coins.eq(users::coins + (price - fee))).execute(conn).await?;
    Ok(coins)
}

/// Buys a listing at its price
#[post("/market/{id}/buy")]
pub async fn buy(state: web::Data<AppState>, user: CurrentUser, id: web::Path<i64>) -> ApiResult<HttpResponse> {
    let id = id.into_inner();
    let uid = user.id;
    let mut conn = state.pool.get().await?;
    let (coins, card, foil, price, fee, seller) = conn
        .transaction::<_, ApiError, _>(async move |conn| {
            let (seller, card, foil, price): (i64, String, Option<String>, i64) = listings::table
                .filter(listings::id.eq(id))
                .filter(listings::state.eq("open"))
                .select((listings::seller_id, listings::card, listings::foil, listings::price))
                .for_update()
                .get_result(conn)
                .await
                .optional()?
                .ok_or(ApiError::Conflict("listing_gone"))?;
            if seller == uid {
                return Err(ApiError::Invalid("market_own"));
            }
            if blocked_between(conn, uid, seller).await? {
                return Err(ApiError::Conflict("player_unavailable"));
            }
            let recent: i64 = listings::table
                .filter(listings::buyer_id.eq(uid))
                .filter(listings::seller_id.eq(seller))
                .filter(listings::state.eq("sold"))
                .filter(listings::closed_at.ge(Utc::now() - Duration::days(1)))
                .count()
                .get_result(conn)
                .await?;
            if recent >= MARKET_PAIR_LIMIT {
                return Err(ApiError::Conflict("market_seller_limit"));
            }
            let fee = game::market_fee(price);
            let coins = pay(conn, uid, seller, price, fee).await?;
            diesel::update(listings::table.find(id))
                .set((
                    listings::state.eq("sold"),
                    listings::buyer_id.eq(uid),
                    listings::fee.eq(fee),
                    listings::closed_at.eq(Utc::now()),
                ))
                .execute(conn)
                .await?;
            diesel::insert_into(owned_cards::table)
                .values((owned_cards::user_id.eq(uid), owned_cards::card.eq(&card), owned_cards::foil.eq(&foil)))
                .execute(conn)
                .await?;
            Ok((coins, card, foil, price, fee, seller))
        })
        .await?;

    let rarity = state.catalog.card(&card).map(|c| c.rarity).unwrap_or(Rarity::Common);
    state.metrics.market("sold");
    crate::missions::progress(&state, uid, crate::missions::Event::MarketBuy, 1).await;
    state.metrics.market_fee(fee);
    state.metrics.spent("market", price);
    state.metrics.earned("market", price - fee);
    state.metrics.card("market", rarity, foil.is_some());

    let detail = serde_json::json!({ "card": card, "foil": foil.is_some(), "price": price, "net": price - fee });
    let seller_name: String = users::table.find(seller).select(users::username).get_result(&mut conn).await?;
    notifications::market_sold(&state, &mut conn, seller, uid, &detail).await?;
    drop(conn);

    let mut unlocked: Vec<Unlock> = progress::cards_gained(&state, uid, &user.username, [card.as_str()]).await?;
    let mut events = Vec::new();
    if rarity == Rarity::Legendary {
        events.push("first_legendary");
    }
    if foil.is_some() {
        events.push("first_foil");
        if rarity == Rarity::Legendary {
            events.push("legendary_foil");
        }
    }
    unlocked.extend(progress::award(&state, uid, &user.username, &events, &[]).await?);
    // The seller's reward arrives through their notifications
    progress::award(&state, seller, &seller_name, &["first_sale"], &[]).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "coins": coins, "card": card, "foil": foil, "unlocked": unlocked })))
}
