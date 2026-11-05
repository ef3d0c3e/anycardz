//! Everything tied to the logged-in player: profile summary, pile, shop,
//! pack opening, daily streak, binder, selling.

use std::collections::HashMap;

use actix_web::{HttpResponse, get, post, web};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::data::Rarity;
use crate::error::{ApiError, ApiResult};
use crate::game::{self, BOOSTER_PRICE, StreakStatus};
use crate::inventory;
use crate::schema::{boosters, owned_cards, streaks, users};

#[derive(Serialize)]
pub struct StreakDto {
    /// Day number of today's claim, made or to make
    day: i32,
    best: i32,
    claimed_today: bool,
    /// Coins of today's claim
    reward: i64,
    /// Coins of tomorrow's claim if the streak holds
    next_reward: i64,
    rewards: [i64; 5],
}

async fn streak_dto(conn: &mut AsyncPgConnection, user_id: i64) -> ApiResult<StreakDto> {
    let row: Option<(i32, i32, Option<chrono::NaiveDate>)> = streaks::table
        .find(user_id)
        .select((streaks::day, streaks::best, streaks::last_claim))
        .first(conn)
        .await
        .optional()?;
    let (day, best, last) = row.unwrap_or((0, 0, None));
    let StreakStatus { day, claimed_today, reward } = game::streak_status(day, last, Utc::now().date_naive());
    Ok(StreakDto {
        day,
        best,
        claimed_today,
        reward,
        next_reward: game::streak_reward(day + 1),
        rewards: game::STREAK_REWARDS,
    })
}

/// Collection slugs of the unopened boosters, oldest first
async fn pile(conn: &mut AsyncPgConnection, user_id: i64) -> ApiResult<Vec<String>> {
    Ok(boosters::table
        .filter(boosters::user_id.eq(user_id))
        .order(boosters::id.asc())
        .select(boosters::collection)
        .load(conn)
        .await?)
}

#[derive(Serialize)]
struct MeDto {
    id: i64,
    username: String,
    avatar: Option<String>,
    /// Friend requests waiting for an answer
    friend_requests: i64,
    /// Unread notifications
    notifications: i64,
    email: String,
    coins: i64,
    pile: Vec<String>,
    streak: StreakDto,
    /// The player has not accepted the current terms yet
    terms_outdated: bool,
    free_packs: FreePacksDto,
    missions: crate::missions::MissionsDto,
    /// Packs to open before a legendary is guaranteed, the next one included
    pity_left: i32,
}

#[derive(Serialize)]
struct FreePacksDto {
    /// Claimable now
    ready: bool,
    /// When they are, if not now
    next_at: Option<DateTime<Utc>>,
    count: i64,
    every_hours: i64,
}

fn free_packs_dto(last: Option<DateTime<Utc>>) -> FreePacksDto {
    let next = last.map(|t| t + chrono::Duration::hours(game::FREE_PACK_HOURS)).filter(|t| *t > Utc::now());
    FreePacksDto { ready: next.is_none(), next_at: next, count: game::FREE_PACKS, every_hours: game::FREE_PACK_HOURS }
}

/// Three packs from every collection, twice a day
#[post("/free-packs/claim")]
pub async fn claim_free_packs(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let uid = user.id;
    let (at, pile) = conn
        .transaction::<_, ApiError, _>(async move |conn| {
            let now = Utc::now();
            // The check and the stamp are one statement: a double tap claims once
            let since = now - chrono::Duration::hours(game::FREE_PACK_HOURS);
            let claimed = diesel::update(
                users::table.find(uid).filter(users::free_packs_at.is_null().or(users::free_packs_at.le(since))),
            )
            .set(users::free_packs_at.eq(now))
            .execute(conn)
            .await?;
            if claimed == 0 {
                return Err(ApiError::Conflict("free_packs_not_ready"));
            }
            let rows: Vec<_> = (0..game::FREE_PACKS)
                .map(|_| (boosters::user_id.eq(uid), boosters::collection.eq(game::FREE_PACK)))
                .collect();
            diesel::insert_into(boosters::table).values(rows).execute(conn).await?;
            Ok((now, pile(conn, uid).await?))
        })
        .await?;
    state.metrics.free_packs();
    crate::missions::progress(&state, user.id, crate::missions::Event::FreePacks, 1).await;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "pile": pile, "free_packs": free_packs_dto(Some(at)) })))
}

/// Today's missions and how far they got
#[get("/missions")]
pub async fn get_missions(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    Ok(HttpResponse::Ok().json(crate::missions::dto(&mut conn, user.id).await?))
}

#[get("/me")]
pub async fn get_me(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let pile = pile(&mut conn, user.id).await?;
    let streak = streak_dto(&mut conn, user.id).await?;
    #[allow(clippy::type_complexity)]
    let (version, has_avatar, terms, free_at, dry): (i32, bool, Option<i32>, Option<DateTime<Utc>>, i32) = users::table
        .find(user.id)
        .select((
            users::avatar_version,
            users::avatar.is_not_null(),
            users::terms_version,
            users::free_packs_at,
            users::dry_packs,
        ))
        .first(&mut conn)
        .await?;
    let avatar = crate::routes::social::avatar_url(&user.username, version, has_avatar);
    let friend_requests: i64 = crate::schema::friendships::table
        .filter(crate::schema::friendships::addressee_id.eq(user.id))
        .filter(crate::schema::friendships::accepted.eq(false))
        .count()
        .get_result(&mut conn)
        .await?;
    Ok(HttpResponse::Ok().json(MeDto {
        id: user.id,
        avatar,
        friend_requests,
        notifications: crate::routes::notifications::unread(&mut conn, user.id).await?,
        terms_outdated: terms != Some(crate::routes::legal::TERMS_VERSION),
        free_packs: free_packs_dto(free_at),
        missions: crate::missions::dto(&mut conn, user.id).await?,
        pity_left: game::pity_left(dry),
        username: user.username,
        email: user.email,
        coins: user.coins,
        pile,
        streak,
    }))
}

#[derive(Deserialize)]
pub struct BuyBody {
    collection: String,
}

#[post("/shop/boosters")]
pub async fn buy_booster(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<BuyBody>,
) -> ApiResult<HttpResponse> {
    let collection =
        state.catalog.collection(&body.collection).ok_or(ApiError::NotFound("unknown_collection"))?.slug.clone();
    let mut conn = state.pool.get().await?;
    let uid = user.id;
    let (coins, pile) = conn
        .transaction::<_, ApiError, _>(async move |conn| {
            {
                // The balance check and the debit are one statement: two purchases
                // at once can never overspend
                let coins: i64 = diesel::update(users::table.find(uid).filter(users::coins.ge(BOOSTER_PRICE)))
                    .set(users::coins.eq(users::coins - BOOSTER_PRICE))
                    .returning(users::coins)
                    .get_result(conn)
                    .await
                    .optional()?
                    .ok_or(ApiError::Conflict("not_enough_coins"))?;
                diesel::insert_into(boosters::table)
                    .values((boosters::user_id.eq(uid), boosters::collection.eq(&collection)))
                    .execute(conn)
                    .await?;
                Ok((coins, pile(conn, uid).await?))
            }
        })
        .await?;
    state.metrics.booster_bought(&body.collection);
    state.metrics.spent("booster", BOOSTER_PRICE);
    Ok(HttpResponse::Ok().json(serde_json::json!({ "coins": coins, "pile": pile })))
}

#[derive(Deserialize, Default)]
pub struct OpenBody {
    /// Open a booster of this collection, else the oldest one
    collection: Option<String>,
}

#[derive(Serialize)]
struct PulledDto {
    card: String,
    rarity: Rarity,
    foil: Option<&'static str>,
    /// First copy the player owns
    is_new: bool,
    /// Copies owned after this pack
    copies: i64,
}

#[post("/pile/open")]
pub async fn open_booster(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: Option<web::Json<OpenBody>>,
) -> ApiResult<HttpResponse> {
    let wanted = body.and_then(|b| b.into_inner().collection);
    let catalog = state.catalog.clone();
    let mut conn = state.pool.get().await?;
    let uid = user.id;
    let result = conn
        .transaction::<_, ApiError, _>(async move |conn| {
            {
                // SKIP LOCKED: two opens at once take two different boosters.
                // Locking needs an unboxed query, hence the two branches.
                let mine = boosters::table.filter(boosters::user_id.eq(uid)).order(boosters::id.asc());
                let found: Option<(i64, String)> = match &wanted {
                    Some(c) => {
                        mine.filter(boosters::collection.eq(c))
                            .select((boosters::id, boosters::collection))
                            .for_update()
                            .skip_locked()
                            .first(conn)
                            .await
                    }
                    None => {
                        mine.select((boosters::id, boosters::collection)).for_update().skip_locked().first(conn).await
                    }
                }
                .optional()?;
                let (booster_id, collection) = found.ok_or(ApiError::Conflict("no_booster"))?;
                diesel::delete(boosters::table.find(booster_id)).execute(conn).await?;

                let pool = if collection == game::FREE_PACK {
                    catalog.pool_all()
                } else {
                    let col = catalog
                        .collection(&collection)
                        .ok_or(ApiError::Internal(anyhow::anyhow!("booster of unknown collection {collection}")))?;
                    catalog.pool(col)
                };
                let dry: i32 = users::table.find(uid).select(users::dry_packs).for_no_key_update().first(conn).await?;
                let drawn = game::draw_pack(&mut rand::rng(), &pool, dry + 1 >= game::PITY_PACKS);
                let dry = if drawn.iter().any(|d| d.card.rarity == Rarity::Legendary) { 0 } else { dry + 1 };
                diesel::update(users::table.find(uid))
                    .set((users::dry_packs.eq(dry), users::packs_opened.eq(users::packs_opened + 1)))
                    .execute(conn)
                    .await?;

                let slugs: Vec<&str> = drawn.iter().map(|d| d.card.slug.as_str()).collect();
                let before: HashMap<String, i64> = owned_cards::table
                    .filter(owned_cards::user_id.eq(uid))
                    .filter(owned_cards::card.eq_any(&slugs))
                    .group_by(owned_cards::card)
                    .select((owned_cards::card, diesel::dsl::count_star()))
                    .load::<(String, i64)>(conn)
                    .await?
                    .into_iter()
                    .collect();
                let rows: Vec<_> = drawn
                    .iter()
                    .map(|d| {
                        (owned_cards::user_id.eq(uid), owned_cards::card.eq(&d.card.slug), owned_cards::foil.eq(d.foil))
                    })
                    .collect();
                diesel::insert_into(owned_cards::table).values(rows).execute(conn).await?;

                // Copies are counts after the pack. A card drawn twice in one pack
                // is only new the first time it shows.
                let mut totals: HashMap<&str, i64> = HashMap::new();
                for d in &drawn {
                    *totals.entry(&d.card.slug).or_insert_with(|| before.get(&d.card.slug).copied().unwrap_or(0)) += 1;
                }
                let mut shown = std::collections::HashSet::new();
                let cards: Vec<PulledDto> = drawn
                    .iter()
                    .map(|d| {
                        let slug = d.card.slug.as_str();
                        let is_new = !before.contains_key(slug) && shown.insert(slug);
                        PulledDto {
                            card: slug.to_owned(),
                            rarity: d.card.rarity,
                            foil: d.foil,
                            is_new,
                            copies: totals[slug],
                        }
                    })
                    .collect();
                Ok((collection, cards, pile(conn, uid).await?, dry))
            }
        })
        .await?;
    let (collection, cards, pile, dry) = result;
    state.metrics.booster_opened(if collection == game::FREE_PACK { "free" } else { &collection });
    for c in &cards {
        state.metrics.card("pack", c.rarity, c.foil.is_some());
    }
    crate::missions::progress(&state, uid, crate::missions::Event::OpenPack, 1).await;
    let rares = cards.iter().filter(|c| c.rarity != crate::data::Rarity::Common).count() as i32;
    crate::missions::progress(&state, uid, crate::missions::Event::PullRare, rares).await;
    for c in &cards {
        state.live.pull(crate::live::PullEvent {
            user: user.username.clone(),
            card: c.card.clone(),
            rarity: c.rarity,
            foil: c.foil.map(Into::into),
            source: crate::live::PullSource::Pack,
            at: chrono::Utc::now(),
        });
    }
    let mut unlocked =
        crate::progress::cards_gained(&state, uid, &user.username, cards.iter().map(|c| c.card.as_str())).await?;
    let mut events = Vec::new();
    if cards.iter().any(|c| c.rarity == Rarity::Legendary) {
        events.push("first_legendary");
    }
    if cards.iter().any(|c| c.foil.is_some()) {
        events.push("first_foil");
    }
    if cards.iter().any(|c| c.rarity == Rarity::Legendary && c.foil.is_some()) {
        events.push("legendary_foil");
    }
    use crate::progress::Metric;
    unlocked.extend(crate::progress::award(&state, uid, &user.username, &events, &[Metric::Packs]).await?);
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "collection": collection, "cards": cards, "pile": pile, "pity_left": game::pity_left(dry),
        "unlocked": unlocked,
    })))
}

#[post("/streak/claim")]
pub async fn claim_streak(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let uid = user.id;
    let (reward, coins) = conn
        .transaction::<_, ApiError, _>(async move |conn| {
            {
                diesel::insert_into(streaks::table)
                    .values(streaks::user_id.eq(uid))
                    .on_conflict_do_nothing()
                    .execute(conn)
                    .await?;
                // Row lock: a double tap claims once
                let (day, best, last): (i32, i32, Option<chrono::NaiveDate>) = streaks::table
                    .find(uid)
                    .select((streaks::day, streaks::best, streaks::last_claim))
                    .for_update()
                    .first(conn)
                    .await?;
                let today = Utc::now().date_naive();
                let status = game::streak_status(day, last, today);
                if status.claimed_today {
                    return Err(ApiError::Conflict("already_claimed"));
                }
                diesel::update(streaks::table.find(uid))
                    .set((
                        streaks::day.eq(status.day),
                        streaks::best.eq(best.max(status.day)),
                        streaks::last_claim.eq(today),
                    ))
                    .execute(conn)
                    .await?;
                let coins: i64 = diesel::update(users::table.find(uid))
                    .set(users::coins.eq(users::coins + status.reward))
                    .returning(users::coins)
                    .get_result(conn)
                    .await?;
                Ok((status.reward, coins))
            }
        })
        .await?;
    state.metrics.earned("streak", reward);
    crate::progress::award(&state, uid, &user.username, &[], &[crate::progress::Metric::Streak]).await?;
    let streak = streak_dto(&mut conn, uid).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "reward": reward, "coins": coins, "streak": streak })))
}

#[derive(Serialize)]
struct OwnedDto {
    card: String,
    count: i64,
    foils: i64,
    /// Pattern of each foil copy, newest first
    patterns: Vec<String>,
}

#[derive(Deserialize)]
pub struct BinderQuery {
    /// Include each owned card's details, for screens listing cards of every set
    #[serde(default)]
    details: Option<u8>,
}

#[get("/binder")]
pub async fn get_binder(
    state: web::Data<AppState>,
    user: CurrentUser,
    lang: crate::i18n::Lang,
    q: web::Query<BinderQuery>,
) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let rows: Vec<(String, i64, i64)> = owned_cards::table
        .filter(owned_cards::user_id.eq(user.id))
        .group_by(owned_cards::card)
        .select((owned_cards::card, diesel::dsl::count_star(), diesel::dsl::count(owned_cards::foil)))
        .order(owned_cards::card.asc())
        .load(&mut conn)
        .await?;
    // Distinct cards owned per collection, for completion bars. A card in two
    // collections counts in both.
    let mut collections: HashMap<&str, usize> =
        state.catalog.collections.iter().map(|c| (c.slug.as_str(), 0)).collect();
    for (slug, _, _) in &rows {
        if let Some(card) = state.catalog.card(slug) {
            for c in &card.collections {
                if let Some(n) = collections.get_mut(c.as_str()) {
                    *n += 1;
                }
            }
        }
    }
    let collections = collections.into_iter().map(|(k, v)| (k.to_owned(), v)).collect::<HashMap<String, usize>>();
    let mut patterns = inventory::foil_patterns(&mut conn, user.id).await?;
    let cards: Vec<OwnedDto> = rows
        .into_iter()
        .map(|(card, count, foils)| OwnedDto {
            patterns: patterns.remove(&card).unwrap_or_default(),
            card,
            count,
            foils,
        })
        .collect();
    let details: Option<HashMap<&str, crate::routes::catalog::CardDto>> = (q.details == Some(1)).then(|| {
        cards
            .iter()
            .filter_map(|o| state.catalog.card(&o.card))
            .map(|c| (c.slug.as_str(), crate::routes::catalog::CardDto::new(&state.catalog, c, lang)))
            .collect()
    });
    let achievements: Vec<String> =
        crate::progress::earned(&mut conn, user.id).await?.into_iter().map(|e| e.key).collect();
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "cards": cards, "collections": collections, "details": details, "achievements": achievements,
    })))
}

#[derive(Deserialize)]
pub struct SellBody {
    card: String,
    /// Sell a foil copy instead of a plain one
    #[serde(default)]
    foil: bool,
}

#[post("/binder/sell")]
pub async fn sell_card(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<SellBody>,
) -> ApiResult<HttpResponse> {
    let SellBody { card, foil } = body.into_inner();
    let rarity = state.catalog.card(&card).ok_or(ApiError::NotFound("unknown_card"))?.rarity;
    let price = game::sell_price(rarity, foil);
    let mut conn = state.pool.get().await?;
    let uid = user.id;
    let (coins, remaining) = conn
        .transaction::<_, ApiError, _>(async move |conn| {
            let take = inventory::Copies { card: card.clone(), foil, count: 1 };
            let ids = inventory::lock_copies(conn, uid, &take).await?;
            inventory::delete_copies(conn, &ids).await?;
            let coins: i64 = diesel::update(users::table.find(uid))
                .set(users::coins.eq(users::coins + price))
                .returning(users::coins)
                .get_result(conn)
                .await?;
            let remaining = inventory::count(conn, uid, &card).await?;
            Ok((coins, remaining))
        })
        .await?;
    state.metrics.earned("sell", price);
    crate::missions::progress(&state, user.id, crate::missions::Event::ShopSell, 1).await;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "coins": coins, "sold_for": price, "remaining": remaining })))
}
