//! Scratch tickets: a few at hand, refilled over time. Each has three slots;
//! three of a kind win that symbol's prize, two of a kind a little coin.
//! Rules only, no I/O: the route applies the prize.

use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use rand::seq::{IndexedRandom, SliceRandom};
use serde::Serialize;

use crate::game::weighted_pick;

/// Tickets a player can hold
pub const MAX: i32 = 3;
/// One ticket comes back every this long, while fewer than MAX are held
pub const REFILL_MINUTES: i64 = 20;
/// How long three sparkles double the foil roulette
pub const BOOST_HOURS: i64 = 2;

pub const JACKPOT_COINS: i64 = 1000;
pub const COINS: i64 = 250;
pub const PAIR_COINS: i64 = 20;

/// What a slot shows
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Symbol {
    Gem,
    Coin,
    Pack,
    Sparkle,
    /// Never three of a kind: fills the losing tickets
    Star,
}

const SYMBOLS: [Symbol; 5] = [Symbol::Gem, Symbol::Coin, Symbol::Pack, Symbol::Sparkle, Symbol::Star];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Prize {
    /// Three gems
    Jackpot,
    /// Three coins
    Coins,
    /// Three packs: a free pack in the pile
    Pack,
    /// Three sparkles: the foil roulette doubled for a while
    Boost,
    /// Two of a kind
    Pair,
    Nothing,
}

impl Prize {
    pub fn coins(self) -> i64 {
        match self {
            Prize::Jackpot => JACKPOT_COINS,
            Prize::Coins => COINS,
            Prize::Pair => PAIR_COINS,
            _ => 0,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Prize::Jackpot => "jackpot",
            Prize::Coins => "coins",
            Prize::Pack => "pack",
            Prize::Boost => "boost",
            Prize::Pair => "pair",
            Prize::Nothing => "nothing",
        }
    }
}

/// Odds per ticket, out of 1000: 21 coins a ticket on average, about 28 with
/// the packs counted at a booster's price, plus the boosts
pub const ODDS: [(Prize, u32); 6] = [
    (Prize::Jackpot, 5),
    (Prize::Coins, 40),
    (Prize::Pack, 30),
    (Prize::Boost, 25),
    (Prize::Pair, 300),
    (Prize::Nothing, 600),
];

/// The outcome first, then three symbols that show it
pub fn draw(rng: &mut impl Rng) -> (Prize, [Symbol; 3]) {
    let prize = *weighted_pick(rng, &ODDS).expect("odds are set");
    let triple = |s: Symbol| [s, s, s];
    let mut slots = match prize {
        Prize::Jackpot => triple(Symbol::Gem),
        Prize::Coins => triple(Symbol::Coin),
        Prize::Pack => triple(Symbol::Pack),
        Prize::Boost => triple(Symbol::Sparkle),
        Prize::Pair => {
            let pair = *SYMBOLS.choose(rng).unwrap();
            let other = *SYMBOLS.iter().filter(|s| **s != pair).collect::<Vec<_>>().choose(rng).unwrap();
            [pair, pair, *other]
        }
        Prize::Nothing => {
            let mut all = SYMBOLS;
            all.shuffle(rng);
            [all[0], all[1], all[2]]
        }
    };
    slots.shuffle(rng);
    (prize, slots)
}

/// Tickets held: the stored count, plus one per refill period since `since`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stock {
    pub count: i32,
    pub since: DateTime<Utc>,
}

impl Stock {
    fn refilled(self, now: DateTime<Utc>) -> i64 {
        ((now - self.since).num_minutes() / REFILL_MINUTES).max(0)
    }

    pub fn available(self, now: DateTime<Utc>) -> i32 {
        (self.count as i64 + self.refilled(now)).min(MAX as i64) as i32
    }

    /// When the next one comes, unless the player is full
    pub fn next_at(self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        (self.available(now) < MAX).then(|| self.since + Duration::minutes((self.refilled(now) + 1) * REFILL_MINUTES))
    }

    /// The stock after one ticket is used, or None when there is none. A
    /// player who was full starts the clock now; otherwise the time already
    /// counted towards the next ticket is kept.
    pub fn take(self, now: DateTime<Utc>) -> Option<Stock> {
        let have = self.available(now);
        if have < 1 {
            return None;
        }
        let since = if self.count as i64 + self.refilled(now) >= MAX as i64 {
            now
        } else {
            self.since + Duration::minutes(self.refilled(now) * REFILL_MINUTES)
        };
        Some(Stock { count: have - 1, since })
    }
}

/// The roulette's chance with a boost: doubled, up to a sure win
pub fn boosted(chance: u32) -> u32 {
    (chance * 2).min(crate::game::ROULETTE_SCALE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn at(min: i64) -> DateTime<Utc> {
        DateTime::<Utc>::UNIX_EPOCH + Duration::minutes(min)
    }

    #[test]
    fn tickets_refill_every_twenty_minutes_up_to_three() {
        let full = Stock { count: 3, since: at(0) };
        assert_eq!(full.available(at(500)), 3);
        assert_eq!(full.next_at(at(500)), None);
        // Using one when full starts the clock
        let two = full.take(at(100)).unwrap();
        assert_eq!((two.count, two.available(at(100))), (2, 2));
        assert_eq!(two.next_at(at(100)), Some(at(120)));
        assert_eq!(two.available(at(119)), 2);
        assert_eq!(two.available(at(120)), 3);
        // Using one mid-way keeps the minutes already counted
        let one = two.take(at(110)).unwrap();
        assert_eq!(one.available(at(110)), 1);
        assert_eq!(one.next_at(at(110)), Some(at(120)));
        assert_eq!(one.available(at(140)), 3);
        // Empty: nothing to take until the next one comes
        let none = Stock { count: 0, since: at(0) };
        assert_eq!(none.take(at(19)), None);
        assert_eq!(none.take(at(20)).map(|s| s.available(at(20))), Some(0));
    }

    #[test]
    fn slots_show_the_prize() {
        let mut rng = StdRng::seed_from_u64(3);
        for _ in 0..5000 {
            let (prize, s) = draw(&mut rng);
            let same = |a: Symbol, b: Symbol| a == b;
            let triple = same(s[0], s[1]) && same(s[1], s[2]);
            let pair = !triple && (same(s[0], s[1]) || same(s[1], s[2]) || same(s[0], s[2]));
            match prize {
                Prize::Jackpot => assert_eq!(s, [Symbol::Gem; 3]),
                Prize::Coins => assert_eq!(s, [Symbol::Coin; 3]),
                Prize::Pack => assert_eq!(s, [Symbol::Pack; 3]),
                Prize::Boost => assert_eq!(s, [Symbol::Sparkle; 3]),
                Prize::Pair => assert!(pair, "{s:?}"),
                Prize::Nothing => assert!(!triple && !pair, "{s:?}"),
            }
        }
    }

    #[test]
    fn odds_add_up_and_pay_21_coins_a_ticket() {
        assert_eq!(ODDS.iter().map(|(_, w)| w).sum::<u32>(), 1000);
        let mean: f64 = ODDS.iter().map(|(p, w)| p.coins() as f64 * *w as f64 / 1000.0).sum();
        assert!((mean - 21.0).abs() < 0.01, "{mean}");
        assert_eq!(boosted(2500), 5000);
        assert_eq!(boosted(5000), 10_000, "capped at a sure win");
        assert_eq!(boosted(9000), 10_000);
    }
}
