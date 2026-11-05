//! Daily missions: three small goals a day, drawn from what the game offers.
//!
//! The day's three are drawn from the player id and the date, then stored the
//! first time they are needed, so they stay put for the day. Play moves them
//! along; a finished mission pays at once, and finishing all three adds a
//! free pack to the pile. Days are UTC, like the streak.

use chrono::{Datelike, NaiveDate, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use rand::SeedableRng;
use rand::seq::IndexedRandom;
use serde::Serialize;

use crate::AppState;
use crate::error::ApiResult;
use crate::game;
use crate::schema::{boosters, daily_missions as dm, users};

/// What moves a mission along
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    OpenPack,
    /// A rare or legendary card out of a pack
    PullRare,
    Tradeup,
    Roulette,
    Trade,
    MarketList,
    MarketBuy,
    ShopSell,
    FreePacks,
}

pub struct Mission {
    pub key: &'static str,
    pub event: Event,
    pub goal: i32,
    pub coins: i64,
}

const fn m(key: &'static str, event: Event, goal: i32, coins: i64) -> Mission {
    Mission { key, event, goal, coins }
}

pub const MISSIONS: &[Mission] = &[
    m("open_packs", Event::OpenPack, 3, 150),
    m("pull_rare", Event::PullRare, 2, 150),
    m("tradeup", Event::Tradeup, 1, 200),
    m("roulette", Event::Roulette, 1, 150),
    m("trade", Event::Trade, 1, 200),
    m("market_list", Event::MarketList, 1, 100),
    m("market_buy", Event::MarketBuy, 1, 150),
    m("shop_sell", Event::ShopSell, 3, 100),
    m("free_packs", Event::FreePacks, 1, 100),
];

/// Missions a day
pub const PER_DAY: usize = 3;
/// The row that marks the all-three bonus as paid
const BONUS: &str = "*bonus";

pub fn mission(key: &str) -> Option<&'static Mission> {
    MISSIONS.iter().find(|m| m.key == key)
}

pub fn today() -> NaiveDate {
    Utc::now().date_naive()
}

/// The day's three for a player: the same all day, different from day to day
pub fn draw(uid: i64, day: NaiveDate) -> Vec<&'static str> {
    let seed = (uid as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (day.num_days_from_ce() as u64);
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    MISSIONS.choose_multiple(&mut rng, PER_DAY).map(|m| m.key).collect()
}

/// Stores the day's missions when they are not yet
async fn ensure(conn: &mut AsyncPgConnection, uid: i64, day: NaiveDate) -> ApiResult<()> {
    let have: i64 = dm::table
        .filter(dm::user_id.eq(uid))
        .filter(dm::day.eq(day))
        .filter(dm::key.ne(BONUS))
        .count()
        .get_result(conn)
        .await?;
    if have > 0 {
        return Ok(());
    }
    let rows: Vec<_> =
        draw(uid, day).into_iter().map(|k| (dm::user_id.eq(uid), dm::day.eq(day), dm::key.eq(k))).collect();
    diesel::insert_into(dm::table).values(rows).on_conflict_do_nothing().execute(conn).await?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct MissionDto {
    pub key: String,
    pub goal: i32,
    pub progress: i32,
    pub coins: i64,
    pub done: bool,
}

#[derive(Debug, Serialize)]
pub struct MissionsDto {
    pub items: Vec<MissionDto>,
    /// All three done: the free pack was given
    pub bonus: bool,
    /// When the next ones come (midnight UTC)
    pub resets_at: chrono::DateTime<Utc>,
}

pub async fn dto(conn: &mut AsyncPgConnection, uid: i64) -> ApiResult<MissionsDto> {
    let day = today();
    ensure(conn, uid, day).await?;
    let rows: Vec<(String, i32, Option<chrono::DateTime<Utc>>)> = dm::table
        .filter(dm::user_id.eq(uid))
        .filter(dm::day.eq(day))
        .order(dm::key)
        .select((dm::key, dm::progress, dm::done_at))
        .load(conn)
        .await?;
    let bonus = rows.iter().any(|r| r.0 == BONUS);
    let items = rows
        .into_iter()
        .filter_map(|(key, progress, done)| {
            let m = mission(&key)?;
            Some(MissionDto { key, goal: m.goal, progress: progress.min(m.goal), coins: m.coins, done: done.is_some() })
        })
        .collect();
    let resets_at = (day + chrono::Duration::days(1)).and_hms_opt(0, 0, 0).expect("midnight exists").and_utc();
    Ok(MissionsDto { items, bonus, resets_at })
}

/// Moves the player's missions on `event` along by `n`. Pays what this
/// finishes, and the bonus pack when the day's three are done. Errors are
/// logged, never returned: play went through, missions follow.
pub async fn progress(state: &AppState, uid: i64, event: Event, n: i32) {
    if n < 1 {
        return;
    }
    if let Err(e) = bump(state, uid, event, n).await {
        tracing::error!("missions: {e}");
    }
}

async fn bump(state: &AppState, uid: i64, event: Event, n: i32) -> ApiResult<()> {
    let keys: Vec<&str> = MISSIONS.iter().filter(|m| m.event == event).map(|m| m.key).collect();
    let mut conn = state.pool.get().await?;
    let day = today();
    ensure(&mut conn, uid, day).await?;
    let moved: Vec<(String, i32)> = diesel::update(
        dm::table
            .filter(dm::user_id.eq(uid))
            .filter(dm::day.eq(day))
            .filter(dm::key.eq_any(&keys))
            .filter(dm::done_at.is_null()),
    )
    .set(dm::progress.eq(dm::progress + n))
    .returning((dm::key, dm::progress))
    .get_results(&mut conn)
    .await?;
    for (key, progress) in moved {
        let Some(m) = mission(&key) else { continue };
        if progress < m.goal {
            continue;
        }
        // Only the update that marks it done pays
        let finished = diesel::update(
            dm::table
                .filter(dm::user_id.eq(uid))
                .filter(dm::day.eq(day))
                .filter(dm::key.eq(&key))
                .filter(dm::done_at.is_null()),
        )
        .set(dm::done_at.eq(Utc::now()))
        .execute(&mut conn)
        .await?;
        if finished == 0 {
            continue;
        }
        diesel::update(users::table.find(uid)).set(users::coins.eq(users::coins + m.coins)).execute(&mut conn).await?;
        state.metrics.earned("mission", m.coins);
        state.metrics.mission(m.key);
        crate::routes::notifications::mission(state, &mut conn, uid, &format!("{key}:{}", m.coins)).await?;
    }
    // All of today's done: the bonus, once
    let open: i64 = dm::table
        .filter(dm::user_id.eq(uid))
        .filter(dm::day.eq(day))
        .filter(dm::key.ne(BONUS))
        .filter(dm::done_at.is_null())
        .count()
        .get_result(&mut conn)
        .await?;
    if open == 0 {
        let paid = diesel::insert_into(dm::table)
            .values((dm::user_id.eq(uid), dm::day.eq(day), dm::key.eq(BONUS), dm::done_at.eq(Utc::now())))
            .on_conflict_do_nothing()
            .execute(&mut conn)
            .await?;
        if paid > 0 {
            diesel::insert_into(boosters::table)
                .values((boosters::user_id.eq(uid), boosters::collection.eq(game::FREE_PACK)))
                .execute(&mut conn)
                .await?;
            state.metrics.mission("bonus");
            crate::routes::notifications::mission(state, &mut conn, uid, "bonus:0").await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_different_missions_a_day_that_change_with_the_day() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();
        for uid in 1..200 {
            let a = draw(uid, day);
            assert_eq!(a.len(), PER_DAY);
            assert!(a[0] != a[1] && a[1] != a[2] && a[0] != a[2], "{a:?}");
            assert_eq!(a, draw(uid, day), "stable for the day");
        }
        let changed = (1..200).filter(|&u| draw(u, day) != draw(u, day.succ_opt().unwrap())).count();
        assert!(changed > 150, "most players get new ones the next day ({changed})");
        // Every mission comes up for someone
        for m in MISSIONS {
            assert!((1..200).any(|u| draw(u, day).contains(&m.key)), "{}", m.key);
        }
    }
}
