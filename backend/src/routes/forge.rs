//! The Forge: trade-ups (10 cards into 1 of the next rarity) and the foil roulette.

use std::collections::BTreeMap;

use actix_web::{HttpResponse, post, web};
use diesel::prelude::*;
use diesel_async::{AsyncConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::data::Rarity;
use crate::error::{ApiError, ApiResult};
use crate::game::{self, ROULETTE_MAX_BURN, TRADEUP_COST};
use crate::inventory::{self, Copies};
use crate::schema::owned_cards;

#[derive(Deserialize)]
pub struct TradeupBody {
    cards: Vec<Copies>,
}

#[derive(Serialize)]
struct Odds {
    collection: String,
    /// Basis points
    chance: u32,
}

#[post("/forge/tradeup")]
pub async fn tradeup(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<TradeupBody>,
) -> ApiResult<HttpResponse> {
    let catalog = state.catalog.clone();
    let takes = inventory::normalize(&catalog, &body.cards)?;
    if takes.iter().map(|c| c.count).sum::<i64>() != TRADEUP_COST {
        return Err(ApiError::Invalid("tradeup_count"));
    }
    let rarity = catalog.cards[&takes[0].card].rarity;
    if takes.iter().any(|c| catalog.cards[&c.card].rarity != rarity) {
        return Err(ApiError::Invalid("tradeup_rarity"));
    }
    let target = game::tier_above(rarity).ok_or(ApiError::Invalid("tradeup_tier"))?;

    // Each collection weighs as many cards as it put in, when it has a card to pay out
    let mut weights: BTreeMap<&str, u32> = BTreeMap::new();
    for c in &takes {
        *weights.entry(catalog.cards[&c.card].collections[0].as_str()).or_default() += c.count as u32;
    }
    let outcomes: Vec<(&str, u32)> = weights
        .into_iter()
        .filter(|(slug, _)| catalog.collection(slug).is_some_and(|col| catalog.pool(col).contains_key(&target)))
        .collect();
    let total: u32 = outcomes.iter().map(|(_, w)| w).sum();
    let odds: Vec<Odds> =
        outcomes.iter().map(|(c, w)| Odds { collection: (*c).to_owned(), chance: w * 10_000 / total.max(1) }).collect();
    let (won, rng_collection) = {
        let mut rng = rand::rng();
        let col = *game::weighted_pick(&mut rng, &outcomes).ok_or(ApiError::Invalid("tradeup_empty"))?;
        let pool = catalog.pool(catalog.collection(col).unwrap());
        let cards = &pool[&target];
        (cards[rand::Rng::random_range(&mut rng, 0..cards.len())].slug.clone(), col.to_owned())
    };

    let mut conn = state.pool.get().await?;
    let uid = user.id;
    let won_slug = won.clone();
    let (is_new, copies) = conn
        .transaction::<_, ApiError, _>(async move |conn| {
            for c in &takes {
                let ids = inventory::lock_copies(conn, uid, c).await?;
                inventory::delete_copies(conn, &ids).await?;
            }
            let had = inventory::count(conn, uid, &won_slug).await?;
            diesel::insert_into(owned_cards::table)
                .values((owned_cards::user_id.eq(uid), owned_cards::card.eq(&won_slug)))
                .execute(conn)
                .await?;
            Ok((had == 0, had + 1))
        })
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "card": won, "rarity": target, "collection": rng_collection, "is_new": is_new, "copies": copies, "odds": odds,
    })))
}

#[derive(Deserialize)]
pub struct RouletteBody {
    /// Card to turn foil: one of its plain copies is rolled
    target: String,
    #[serde(default)]
    burn: Vec<Copies>,
}

#[post("/forge/roulette")]
pub async fn roulette(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<RouletteBody>,
) -> ApiResult<HttpResponse> {
    let catalog = state.catalog.clone();
    let RouletteBody { target, burn } = body.into_inner();
    let target_rarity = catalog.card(&target).ok_or(ApiError::NotFound("unknown_card"))?.rarity;
    let burn = inventory::normalize(&catalog, &burn)?;
    if burn.iter().map(|c| c.count).sum::<i64>() > ROULETTE_MAX_BURN {
        return Err(ApiError::Invalid("roulette_count"));
    }
    let burned: Vec<Rarity> =
        burn.iter().flat_map(|c| std::iter::repeat_n(catalog.cards[&c.card].rarity, c.count as usize)).collect();
    let chance = game::roulette_chance(target_rarity, &burned);
    let won = game::roulette_roll(&mut rand::rng(), chance);

    // The rolled copy and the burned ones come from the same stacks: lock them together
    let mut takes = burn.clone();
    match takes.iter_mut().find(|c| c.card == target && !c.foil) {
        Some(c) => c.count += 1,
        None => takes.push(Copies { card: target.clone(), foil: false, count: 1 }),
    }
    let takes = inventory::normalize(&catalog, &takes)?;

    let mut conn = state.pool.get().await?;
    let uid = user.id;
    let tgt = target.clone();
    conn.transaction::<_, ApiError, _>(async move |conn| {
        for c in &takes {
            let mut ids = inventory::lock_copies(conn, uid, c).await?;
            if c.card == tgt && !c.foil {
                // The rolled copy: the newest plain one. A win keeps it, as a foil.
                let rolled = ids.remove(0);
                match won {
                    Some(pattern) => {
                        diesel::update(owned_cards::table.find(rolled))
                            .set(owned_cards::foil.eq(pattern))
                            .execute(conn)
                            .await?;
                    }
                    None => ids.push(rolled),
                }
            }
            inventory::delete_copies(conn, &ids).await?;
        }
        Ok(())
    })
    .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "won": won.is_some(), "foil": won, "chance": chance, "burned": burned.len(),
    })))
}
