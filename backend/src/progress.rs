//! Achievements, earned once and never lost, each paying coins once:
//! - set completion: every card of a collection (`set:<slug>`), every one as a
//!   foil (`foilset:<slug>`);
//! - the fixed list in ACHIEVEMENTS: one-off events (a first legendary) and
//!   goals on counters (packs opened, trades done...).

use std::collections::{BTreeSet, HashSet};

use chrono::Utc;
use diesel::prelude::*;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use serde::Serialize;

use crate::AppState;
use crate::error::{ApiError, ApiResult};
use crate::schema::{achievements, friendships, owned_cards, streaks, trades, users};

/// What a goal counts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    /// Packs opened (counted from when achievements arrived)
    Packs,
    /// Trades done
    Trades,
    /// Sets completed
    Sets,
    /// Accepted friends
    Friends,
    /// Distinct cards owned
    Cards,
    /// Best daily streak
    Streak,
    /// Foil roulette wins in a row, right now
    RouletteStreak,
}

#[derive(Debug, Clone, Copy)]
pub struct Achievement {
    pub id: &'static str,
    pub coins: i64,
    /// Earned when the metric reaches the number; None: earned on an event
    pub goal: Option<(Metric, i64)>,
}

const fn a(id: &'static str, coins: i64, goal: Option<(Metric, i64)>) -> Achievement {
    Achievement { id, coins, goal }
}

/// Every achievement besides sets, in the order players see them
pub const ACHIEVEMENTS: &[Achievement] = &[
    a("first_pack", 50, Some((Metric::Packs, 1))),
    a("packs_50", 300, Some((Metric::Packs, 50))),
    a("packs_250", 1000, Some((Metric::Packs, 250))),
    a("first_legendary", 200, None),
    a("first_foil", 100, None),
    a("legendary_foil", 1000, None),
    a("first_tradeup", 100, None),
    a("roulette_win", 200, None),
    a("roulette_streak_3", 1000, Some((Metric::RouletteStreak, 3))),
    a("first_trade", 150, Some((Metric::Trades, 1))),
    a("trades_25", 750, Some((Metric::Trades, 25))),
    a("first_sale", 100, None),
    a("streak_7", 300, Some((Metric::Streak, 7))),
    a("streak_30", 1500, Some((Metric::Streak, 30))),
    a("first_set", 250, Some((Metric::Sets, 1))),
    a("sets_5", 1500, Some((Metric::Sets, 5))),
    a("friends_5", 200, Some((Metric::Friends, 5))),
    a("collector_500", 1000, Some((Metric::Cards, 500))),
];

pub fn achievement(id: &str) -> Option<&'static Achievement> {
    ACHIEVEMENTS.iter().find(|a| a.id == id)
}

/// The player's value of a metric
pub async fn metric(conn: &mut AsyncPgConnection, uid: i64, m: Metric) -> ApiResult<i64> {
    Ok(match m {
        Metric::Packs => users::table.find(uid).select(users::packs_opened).first::<i32>(conn).await? as i64,
        Metric::RouletteStreak => {
            users::table.find(uid).select(users::roulette_streak).first::<i32>(conn).await? as i64
        }
        Metric::Trades => {
            trades::table
                .filter(trades::state.eq("done"))
                .filter(trades::from_id.eq(uid).or(trades::to_id.eq(uid)))
                .count()
                .get_result(conn)
                .await?
        }
        Metric::Sets => {
            achievements::table
                .filter(achievements::user_id.eq(uid))
                .filter(achievements::key.like("set:%"))
                .count()
                .get_result(conn)
                .await?
        }
        Metric::Friends => {
            friendships::table
                .filter(friendships::accepted.eq(true))
                .filter(friendships::requester_id.eq(uid).or(friendships::addressee_id.eq(uid)))
                .count()
                .get_result(conn)
                .await?
        }
        Metric::Cards => {
            owned_cards::table
                .filter(owned_cards::user_id.eq(uid))
                .select(diesel::dsl::count(owned_cards::card).aggregate_distinct())
                .first(conn)
                .await?
        }
        Metric::Streak => {
            streaks::table.find(uid).select(streaks::best).first::<i32>(conn).await.optional()?.unwrap_or(0) as i64
        }
    })
}

/// Coins per card of the set, so bigger sets pay more
pub const SET_REWARD_PER_CARD: i64 = 10;
pub const FOIL_SET_REWARD_PER_CARD: i64 = 40;

pub fn set_reward(cards: usize) -> i64 {
    cards as i64 * SET_REWARD_PER_CARD
}

pub fn foil_set_reward(cards: usize) -> i64 {
    cards as i64 * FOIL_SET_REWARD_PER_CARD
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Unlock {
    pub key: String,
    pub coins: i64,
}

/// Records `key` for the player and pays its coins, once. False when it was
/// already earned.
pub async fn grant(conn: &mut AsyncPgConnection, uid: i64, key: &str, coins: i64) -> ApiResult<bool> {
    let key = key.to_owned();
    conn.transaction::<_, ApiError, _>(async move |conn| {
        let new = diesel::insert_into(achievements::table)
            .values((achievements::user_id.eq(uid), achievements::key.eq(&key), achievements::coins.eq(coins)))
            .on_conflict_do_nothing()
            .execute(conn)
            .await?
            == 1;
        if new && coins > 0 {
            diesel::update(users::table.find(uid)).set(users::coins.eq(users::coins + coins)).execute(conn).await?;
        }
        Ok(new)
    })
    .await
}

/// Grants, then tells the player and everyone
async fn unlock(
    state: &AppState,
    conn: &mut AsyncPgConnection,
    uid: i64,
    username: &str,
    key: String,
    coins: i64,
    kind: &'static str,
) -> ApiResult<Option<Unlock>> {
    if !grant(conn, uid, &key, coins).await? {
        return Ok(None);
    }
    crate::routes::notifications::achievement(state, conn, uid, &key).await?;
    state.live.feat(crate::live::FeatEvent { user: username.to_owned(), key: key.clone(), at: Utc::now() });
    state.metrics.achievement(kind);
    Ok(Some(Unlock { key, coins }))
}

/// Awards these event achievements, and every goal on these metrics that is
/// reached. Asking for one already earned does nothing.
pub async fn award(
    state: &AppState,
    uid: i64,
    username: &str,
    events: &[&str],
    metrics: &[Metric],
) -> ApiResult<Vec<Unlock>> {
    let mut unlocked = Vec::new();
    if events.is_empty() && metrics.is_empty() {
        return Ok(unlocked);
    }
    let mut conn = state.pool.get().await?;
    for id in events {
        let Some(a) = achievement(id).filter(|a| a.goal.is_none()) else { continue };
        if let Some(u) = unlock(state, &mut conn, uid, username, a.id.to_owned(), a.coins, "achievement").await? {
            unlocked.push(u);
        }
    }
    for m in metrics {
        let value = metric(&mut conn, uid, *m).await?;
        for a in ACHIEVEMENTS.iter().filter(|a| matches!(a.goal, Some((g, n)) if g == *m && value >= n)) {
            if let Some(u) = unlock(state, &mut conn, uid, username, a.id.to_owned(), a.coins, "achievement").await? {
                unlocked.push(u);
            }
        }
    }
    Ok(unlocked)
}

/// What a player owns of a collection: distinct cards, and distinct foil cards
fn completion(owned: &[(String, bool)], set: &[String]) -> (bool, bool) {
    let set: HashSet<&str> = set.iter().map(String::as_str).collect();
    let have: HashSet<&str> = owned.iter().map(|(c, _)| c.as_str()).filter(|c| set.contains(c)).collect();
    let foils: HashSet<&str> =
        owned.iter().filter(|(_, f)| *f).map(|(c, _)| c.as_str()).filter(|c| set.contains(c)).collect();
    (!set.is_empty() && have.len() == set.len(), !set.is_empty() && foils.len() == set.len())
}

/// After cards land in a binder: completes the sets they belong to, pays,
/// notifies the player and tells everyone. Returns what was earned now.
pub async fn cards_gained<'a>(
    state: &AppState,
    uid: i64,
    username: &str,
    cards: impl IntoIterator<Item = &'a str>,
) -> ApiResult<Vec<Unlock>> {
    let touched: BTreeSet<&str> = cards
        .into_iter()
        .filter_map(|c| state.catalog.card(c))
        .flat_map(|c| c.collections.iter().map(String::as_str))
        .collect();
    let mut unlocked = Vec::new();
    if touched.is_empty() {
        return Ok(unlocked);
    }
    let mut conn = state.pool.get().await?;
    for slug in touched {
        let Some(col) = state.catalog.collection(slug) else { continue };
        let owned: Vec<(String, bool)> = owned_cards::table
            .filter(owned_cards::user_id.eq(uid))
            .filter(owned_cards::card.eq_any(&col.cards))
            .select((owned_cards::card, owned_cards::foil.is_not_null()))
            .distinct()
            .load(&mut conn)
            .await?;
        let (full, foil) = completion(&owned, &col.cards);
        let mut earned = Vec::new();
        if full {
            earned.push((format!("set:{slug}"), set_reward(col.cards.len())));
        }
        if foil {
            earned.push((format!("foilset:{slug}"), foil_set_reward(col.cards.len())));
        }
        for (key, coins) in earned {
            let kind = if key.starts_with("foilset:") { "foil_set" } else { "set" };
            if let Some(u) = unlock(state, &mut conn, uid, username, key, coins, kind).await? {
                unlocked.push(u);
            }
        }
    }
    drop(conn);
    // More cards, maybe a first or fifth set
    unlocked.extend(award(state, uid, username, &[], &[Metric::Sets, Metric::Cards]).await?);
    Ok(unlocked)
}

#[derive(Debug, Clone, Serialize)]
pub struct EarnedDto {
    pub key: String,
    pub coins: i64,
    pub at: chrono::DateTime<Utc>,
}

/// Everything the player earned, newest first
pub async fn earned(conn: &mut AsyncPgConnection, uid: i64) -> ApiResult<Vec<EarnedDto>> {
    let rows: Vec<(String, i64, chrono::DateTime<Utc>)> = achievements::table
        .filter(achievements::user_id.eq(uid))
        .order(achievements::created_at.desc())
        .select((achievements::key, achievements::coins, achievements::created_at))
        .load(conn)
        .await?;
    Ok(rows.into_iter().map(|(key, coins, at)| EarnedDto { key, coins, at }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn completion_counts_distinct_cards_of_the_set_only() {
        let set = s(&["a", "b", "c"]);
        let owned = |v: &[(&str, bool)]| v.iter().map(|(c, f)| (c.to_string(), *f)).collect::<Vec<_>>();
        assert_eq!(completion(&owned(&[("a", false), ("b", false)]), &set), (false, false));
        // Cards of other sets do not fill the gap
        assert_eq!(completion(&owned(&[("a", false), ("b", false), ("z", false)]), &set), (false, false));
        assert_eq!(completion(&owned(&[("a", false), ("b", true), ("c", false)]), &set), (true, false));
        // A foil copy counts for both
        assert_eq!(completion(&owned(&[("a", true), ("b", true), ("c", true)]), &set), (true, true));
        assert_eq!(completion(&[], &[]), (false, false), "an empty set is never complete");
    }

    #[test]
    fn achievement_ids_are_unique_and_goals_positive() {
        let ids: HashSet<&str> = ACHIEVEMENTS.iter().map(|a| a.id).collect();
        assert_eq!(ids.len(), ACHIEVEMENTS.len());
        assert!(ACHIEVEMENTS.iter().all(|a| a.goal.is_none_or(|(_, n)| n > 0) && a.coins >= 0));
        // Ids never look like set keys
        assert!(ACHIEVEMENTS.iter().all(|a| !a.id.contains(':')));
    }

    #[test]
    fn rewards_grow_with_the_set() {
        assert_eq!(set_reward(150), 1500);
        assert!(foil_set_reward(150) > set_reward(150));
    }
}
