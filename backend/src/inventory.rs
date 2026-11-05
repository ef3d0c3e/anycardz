//! Taking copies out of a player's binder, shared by everything that spends
//! cards (selling, trade-ups, the foil roulette, trades).

use std::collections::HashMap;

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};

use crate::data::Catalog;
use crate::error::{ApiError, ApiResult};
use crate::schema::owned_cards;

/// Some copies of one card in a request: `{"card": "ak-47", "foil": false, "count": 3}`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Copies {
    pub card: String,
    #[serde(default)]
    pub foil: bool,
    #[serde(default = "one")]
    pub count: i64,
}

fn one() -> i64 {
    1
}

/// Merges duplicate entries and checks every card exists and every count is positive
pub fn normalize(catalog: &Catalog, list: &[Copies]) -> ApiResult<Vec<Copies>> {
    let mut merged: HashMap<(String, bool), i64> = HashMap::new();
    for c in list {
        if c.count < 1 {
            return Err(ApiError::Invalid("invalid_count"));
        }
        if catalog.card(&c.card).is_none() {
            return Err(ApiError::NotFound("unknown_card"));
        }
        *merged.entry((c.card.clone(), c.foil)).or_default() += c.count;
    }
    let mut out: Vec<Copies> = merged.into_iter().map(|((card, foil), count)| Copies { card, foil, count }).collect();
    // A stable order: locks are always taken in the same order, no deadlock
    out.sort_by(|a, b| (&a.card, a.foil).cmp(&(&b.card, b.foil)));
    Ok(out)
}

/// Row-locks `count` copies of a card the player owns, newest first (the oldest
/// copy stays as "first obtained"). Fails with `not_owned` when there are fewer.
/// Waits for other transactions holding the same rows, so a copy is never spent twice.
pub async fn lock_copies(conn: &mut AsyncPgConnection, user_id: i64, c: &Copies) -> ApiResult<Vec<i64>> {
    let mine = owned_cards::table
        .filter(owned_cards::user_id.eq(user_id))
        .filter(owned_cards::card.eq(&c.card))
        .order(owned_cards::id.desc())
        .select(owned_cards::id)
        .limit(c.count);
    let ids: Vec<i64> = if c.foil {
        mine.filter(owned_cards::foil.is_not_null()).for_update().load(conn).await?
    } else {
        mine.filter(owned_cards::foil.is_null()).for_update().load(conn).await?
    };
    if (ids.len() as i64) < c.count {
        return Err(ApiError::Conflict("not_owned"));
    }
    Ok(ids)
}

pub async fn delete_copies(conn: &mut AsyncPgConnection, ids: &[i64]) -> ApiResult<()> {
    diesel::delete(owned_cards::table.filter(owned_cards::id.eq_any(ids))).execute(conn).await?;
    Ok(())
}

/// Copies of a card the player owns, plain and foil
pub async fn count(conn: &mut AsyncPgConnection, user_id: i64, card: &str) -> ApiResult<i64> {
    Ok(owned_cards::table
        .filter(owned_cards::user_id.eq(user_id))
        .filter(owned_cards::card.eq(card))
        .count()
        .get_result(conn)
        .await?)
}

/// Foil patterns of a player's foil copies, per card, newest copy first
pub async fn foil_patterns(conn: &mut AsyncPgConnection, user_id: i64) -> ApiResult<HashMap<String, Vec<String>>> {
    let rows: Vec<(String, Option<String>)> = owned_cards::table
        .filter(owned_cards::user_id.eq(user_id))
        .filter(owned_cards::foil.is_not_null())
        .order(owned_cards::id.desc())
        .select((owned_cards::card, owned_cards::foil))
        .load(conn)
        .await?;
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for (card, foil) in rows {
        out.entry(card).or_default().extend(foil);
    }
    Ok(out)
}
