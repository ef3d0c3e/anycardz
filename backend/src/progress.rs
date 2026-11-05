//! Set completion: owning every card of a collection pays coins once and leaves
//! a badge on the profile; owning every one of them as a foil pays more. Both
//! are achievements (`set:<slug>`, `foilset:<slug>`): earned once, never lost.

use std::collections::{BTreeSet, HashSet};

use chrono::Utc;
use diesel::prelude::*;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use serde::Serialize;

use crate::AppState;
use crate::error::{ApiError, ApiResult};
use crate::schema::{achievements, owned_cards, users};

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
            if grant(&mut conn, uid, &key, coins).await? {
                crate::routes::notifications::achievement(state, &mut conn, uid, &key).await?;
                state.live.feat(crate::live::FeatEvent { user: username.to_owned(), key: key.clone(), at: Utc::now() });
                state.metrics.achievement(if key.starts_with("foilset:") { "foil_set" } else { "set" });
                unlocked.push(Unlock { key, coins });
            }
        }
    }
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
    fn rewards_grow_with_the_set() {
        assert_eq!(set_reward(150), 1500);
        assert!(foil_set_reward(150) > set_reward(150));
    }
}
