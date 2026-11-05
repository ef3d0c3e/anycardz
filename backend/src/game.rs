//! Game rules, kept free of I/O so they can be tested on their own.

use std::collections::HashMap;

use chrono::NaiveDate;
use rand::Rng;

use crate::data::{Card, Rarity};

pub const STARTING_COINS: i64 = 1000;
pub const BOOSTER_PRICE: i64 = 250;
pub const PACK_SIZE: usize = 5;
/// Drop weights out of 100
pub const WEIGHT_COMMON: u32 = 80;
pub const WEIGHT_RARE: u32 = 15;
/// Chance for a drawn card to be foil, in percent
pub const FOIL_CHANCE: u32 = 5;
/// Foil pattern names, also the mask files in the data repo's assets/
pub const FOIL_PATTERNS: [&str; 6] = ["foil1", "foil2", "foil3", "foil4", "foil5", "foil6"];
/// Daily bonus by streak day, capped at the last value
pub const STREAK_REWARDS: [i64; 5] = [200, 300, 500, 700, 1000];

/// Coin packs of the top-up screen. Prices are displayed only: no checkout yet.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct TopupPack {
    pub id: &'static str,
    pub coins: i64,
    pub price_cents: i32,
}

pub const TOPUP_PACKS: [TopupPack; 5] = [
    TopupPack { id: "small", coins: 100, price_cents: 99 },
    TopupPack { id: "medium", coins: 500, price_cents: 449 },
    TopupPack { id: "large", coins: 1000, price_cents: 799 },
    TopupPack { id: "xlarge", coins: 5000, price_cents: 2999 },
    TopupPack { id: "xxlarge", coins: 10000, price_cents: 4999 },
];

pub fn topup_pack(id: &str) -> Option<TopupPack> {
    TOPUP_PACKS.iter().copied().find(|p| p.id == id)
}

pub fn roll_rarity(rng: &mut impl Rng) -> Rarity {
    let n = rng.random_range(0..100);
    if n < WEIGHT_COMMON {
        Rarity::Common
    } else if n < WEIGHT_COMMON + WEIGHT_RARE {
        Rarity::Rare
    } else {
        Rarity::Legendary
    }
}

pub fn roll_foil(rng: &mut impl Rng) -> Option<&'static str> {
    (rng.random_range(0..100) < FOIL_CHANCE).then(|| FOIL_PATTERNS[rng.random_range(0..FOIL_PATTERNS.len())])
}

#[derive(Debug, Clone)]
pub struct Drawn<'a> {
    pub card: &'a Card,
    pub foil: Option<&'static str>,
}

/// Draws a pack from a collection's pool. A rarity the collection lacks falls
/// back to the closest one below, then above. Sorted best-last, so the reveal
/// builds up instead of front-loading the hit.
pub fn draw_pack<'a>(rng: &mut impl Rng, pool: &HashMap<Rarity, Vec<&'a Card>>) -> Vec<Drawn<'a>> {
    let mut pack: Vec<Drawn> = (0..PACK_SIZE)
        .filter_map(|_| {
            let wanted = roll_rarity(rng);
            let mut order: Vec<Rarity> = Rarity::ALL.iter().copied().filter(|r| *r <= wanted).rev().collect();
            order.extend(Rarity::ALL.iter().copied().filter(|r| *r > wanted));
            let cards = order.iter().find_map(|r| pool.get(r).filter(|c| !c.is_empty()))?;
            Some(Drawn { card: cards[rng.random_range(0..cards.len())], foil: roll_foil(rng) })
        })
        .collect();
    pack.sort_by_key(|d| (d.card.rarity, d.foil.is_some()));
    pack
}

/// Cards a trade-up consumes
pub const TRADEUP_COST: i64 = 10;

/// The rarity a trade-up of `r` pays out, None for the top tier
pub fn tier_above(r: Rarity) -> Option<Rarity> {
    match r {
        Rarity::Common => Some(Rarity::Rare),
        Rarity::Rare => Some(Rarity::Legendary),
        Rarity::Legendary => None,
    }
}

/// Picks an item with odds proportional to its weight. None when nothing can pay out.
pub fn weighted_pick<'a, T>(rng: &mut impl Rng, items: &'a [(T, u32)]) -> Option<&'a T> {
    let total: u32 = items.iter().map(|(_, w)| w).sum();
    if total == 0 {
        return None;
    }
    let mut n = rng.random_range(0..total);
    for (item, w) in items {
        if n < *w {
            return Some(item);
        }
        n -= w;
    }
    None
}

/// Foil roulette chance, in basis points (1/100 of a percent)
pub const ROULETTE_CAP: u32 = 5000;
pub const ROULETTE_SCALE: u32 = 10_000;
/// Enough commons to reach the cap on a common, anything past that is a mistake
pub const ROULETTE_MAX_BURN: i64 = 100;

const fn roulette_odds(r: Rarity) -> (u32, u32) {
    // (chance of the rolled card on its own, bonus each burned card adds)
    match r {
        Rarity::Common => (100, 50),
        Rarity::Rare => (200, 100),
        Rarity::Legendary => (500, 200),
    }
}

pub fn roulette_chance(target: Rarity, burned: &[Rarity]) -> u32 {
    let (base, _) = roulette_odds(target);
    let bonus: u32 = burned.iter().map(|r| roulette_odds(*r).1).sum();
    (base + bonus).min(ROULETTE_CAP)
}

/// The won foil pattern, None when the roll is lost
pub fn roulette_roll(rng: &mut impl Rng, chance: u32) -> Option<&'static str> {
    (rng.random_range(0..ROULETTE_SCALE) < chance).then(|| FOIL_PATTERNS[rng.random_range(0..FOIL_PATTERNS.len())])
}

pub fn sell_price(rarity: Rarity, foil: bool) -> i64 {
    let base = match rarity {
        Rarity::Common => 10,
        Rarity::Rare => 50,
        Rarity::Legendary => 250,
    };
    if foil { base * 2 } else { base }
}

pub fn streak_reward(day: i32) -> i64 {
    STREAK_REWARDS[(day.max(1) as usize - 1).min(STREAK_REWARDS.len() - 1)]
}

/// Where a streak stands on a given day
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreakStatus {
    /// Day number of today's claim, made or to make
    pub day: i32,
    pub claimed_today: bool,
    pub reward: i64,
}

pub fn streak_status(day: i32, last_claim: Option<NaiveDate>, today: NaiveDate) -> StreakStatus {
    let (day, claimed_today) = match last_claim {
        Some(last) if last == today => (day, true),
        Some(last) if today.pred_opt() == Some(last) => (day + 1, false),
        _ => (1, false),
    };
    StreakStatus { day, claimed_today, reward: streak_reward(day) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Catalog;
    use rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn rarity_odds_match_the_rules() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut counts = HashMap::new();
        let n = 200_000;
        for _ in 0..n {
            *counts.entry(roll_rarity(&mut rng)).or_insert(0) += 1;
        }
        let share = |r| counts[&r] as f64 / n as f64;
        assert!((share(Rarity::Common) - 0.80).abs() < 0.01);
        assert!((share(Rarity::Rare) - 0.15).abs() < 0.01);
        assert!((share(Rarity::Legendary) - 0.05).abs() < 0.01);
    }

    #[test]
    fn foil_odds_match_the_rules() {
        let mut rng = StdRng::seed_from_u64(2);
        let n = 200_000;
        let foils = (0..n).filter(|_| roll_foil(&mut rng).is_some()).count();
        assert!((foils as f64 / n as f64 - 0.05).abs() < 0.005);
    }

    #[test]
    fn packs_have_five_cards_best_last() {
        let cat = Catalog::load(&crate::test_support::fixture_dir(), false).unwrap();
        let pool = cat.pool(cat.collection("minerals").unwrap());
        let mut rng = StdRng::seed_from_u64(3);
        for _ in 0..500 {
            let pack = draw_pack(&mut rng, &pool);
            assert_eq!(pack.len(), PACK_SIZE);
            assert!(
                pack.windows(2)
                    .all(|w| (w[0].card.rarity, w[0].foil.is_some()) <= (w[1].card.rarity, w[1].foil.is_some()))
            );
        }
    }

    #[test]
    fn missing_rarity_falls_back() {
        let cat = Catalog::load(&crate::test_support::fixture_dir(), false).unwrap();
        let mut pool = cat.pool(cat.collection("minerals").unwrap());
        pool.remove(&Rarity::Legendary);
        pool.remove(&Rarity::Rare);
        let mut rng = StdRng::seed_from_u64(4);
        let pack = draw_pack(&mut rng, &pool);
        assert!(pack.iter().all(|d| d.card.rarity == Rarity::Common));
    }

    #[test]
    fn tiers() {
        assert_eq!(tier_above(Rarity::Common), Some(Rarity::Rare));
        assert_eq!(tier_above(Rarity::Rare), Some(Rarity::Legendary));
        assert_eq!(tier_above(Rarity::Legendary), None);
    }

    #[test]
    fn weighted_pick_follows_weights() {
        let mut rng = StdRng::seed_from_u64(5);
        let items = [("a", 7), ("b", 3), ("never", 0)];
        let n = 100_000;
        let a = (0..n).filter(|_| *weighted_pick(&mut rng, &items).unwrap() == "a").count();
        assert!((a as f64 / n as f64 - 0.7).abs() < 0.01);
        assert!((0..1000).all(|_| *weighted_pick(&mut rng, &items).unwrap() != "never"));
        assert!(weighted_pick::<&str>(&mut rng, &[]).is_none());
    }

    #[test]
    fn roulette_chances_match_the_old_rules() {
        use Rarity::*;
        assert_eq!(roulette_chance(Common, &[]), 100);
        assert_eq!(roulette_chance(Legendary, &[]), 500);
        assert_eq!(roulette_chance(Rare, &[Common, Rare, Legendary]), 200 + 50 + 100 + 200);
        // 98 commons bring a common to the 50% cap
        assert_eq!(roulette_chance(Common, &[Common; 98]), ROULETTE_CAP);
        assert_eq!(roulette_chance(Legendary, &[Legendary; 100]), ROULETTE_CAP);
    }

    #[test]
    fn roulette_roll_odds() {
        let mut rng = StdRng::seed_from_u64(6);
        let n = 100_000;
        let wins = (0..n).filter(|_| roulette_roll(&mut rng, 2500).is_some()).count();
        assert!((wins as f64 / n as f64 - 0.25).abs() < 0.01);
        assert!((0..1000).all(|_| roulette_roll(&mut rng, 0).is_none()));
    }

    #[test]
    fn sell_prices() {
        assert_eq!(sell_price(Rarity::Common, false), 10);
        assert_eq!(sell_price(Rarity::Rare, true), 100);
        assert_eq!(sell_price(Rarity::Legendary, false), 250);
    }

    #[test]
    fn streak_transitions() {
        let d = |s| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        let today = d("2026-11-05");
        assert_eq!(streak_status(0, None, today), StreakStatus { day: 1, claimed_today: false, reward: 200 });
        assert_eq!(
            streak_status(2, Some(d("2026-11-04")), today),
            StreakStatus { day: 3, claimed_today: false, reward: 500 }
        );
        assert_eq!(streak_status(3, Some(today), today), StreakStatus { day: 3, claimed_today: true, reward: 500 });
        // Missed a day: back to day 1
        assert_eq!(streak_status(4, Some(d("2026-11-03")), today).day, 1);
        // Capped reward past day 5
        assert_eq!(streak_status(11, Some(d("2026-11-04")), today).reward, 1000);
    }
}
