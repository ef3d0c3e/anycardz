//! The achievement list with the player's progress, and leaderboards.

use std::collections::HashMap;

use actix_web::{HttpResponse, get, web};
use chrono::{DateTime, Duration, Utc};
use diesel::prelude::*;
use diesel::sql_types::{Array, BigInt, Bool, Int4, Text, Timestamptz};
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::data::Rarity;
use crate::error::{ApiError, ApiResult};
use crate::progress::{ACHIEVEMENTS, Metric, earned, metric};
use crate::routes::social::avatar_url;

#[derive(Serialize)]
struct AchievementDto {
    id: &'static str,
    coins: i64,
    /// For goals: what is counted and how many are needed
    metric: Option<Metric>,
    goal: Option<i64>,
    /// The player's count now
    have: Option<i64>,
    earned_at: Option<DateTime<Utc>>,
}

#[get("/achievements")]
pub async fn list(state: web::Data<AppState>, user: CurrentUser) -> ApiResult<HttpResponse> {
    let mut conn = state.pool.get().await?;
    let mine: HashMap<String, DateTime<Utc>> =
        earned(&mut conn, user.id).await?.into_iter().map(|e| (e.key, e.at)).collect();
    let mut values: HashMap<Metric, i64> = HashMap::new();
    let mut items = Vec::new();
    for a in ACHIEVEMENTS {
        let (m, goal) = a.goal.unzip();
        let have = match m {
            Some(m) => Some(match values.get(&m) {
                Some(v) => *v,
                None => {
                    let v = metric(&mut conn, user.id, m).await?;
                    values.insert(m, v);
                    v
                }
            }),
            None => None,
        };
        items.push(AchievementDto {
            id: a.id,
            coins: a.coins,
            metric: m,
            goal,
            have,
            earned_at: mine.get(a.id).copied(),
        });
    }
    let sets = mine.keys().filter(|k| k.starts_with("set:")).count();
    let foil_sets = mine.keys().filter(|k| k.starts_with("foilset:")).count();
    Ok(HttpResponse::Ok().json(serde_json::json!({ "items": items, "sets": sets, "foil_sets": foil_sets })))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Board {
    /// Sets completed
    Sets,
    /// Achievements earned, sets aside
    Achievements,
    /// Legendary copies held, obtained in the period
    Legendaries,
    /// Foil copies held, obtained in the period
    Foils,
    /// Trades done
    Trades,
    /// Best daily streak, all time only
    Streak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    Week,
    All,
}

#[derive(Deserialize)]
pub struct BoardQuery {
    board: Board,
    #[serde(default = "all")]
    period: Period,
}

fn all() -> Period {
    Period::All
}

const TOP: i64 = 20;

/// Per player counts for a board, as `(user_id, n)` rows. $1: period start,
/// $2: legendary card slugs.
fn counts_sql(board: Board) -> &'static str {
    match board {
        Board::Sets => {
            "SELECT user_id, count(*) AS n FROM achievements
             WHERE key LIKE 'set:%' AND created_at >= $1 GROUP BY user_id"
        }
        Board::Achievements => {
            "SELECT user_id, count(*) AS n FROM achievements
             WHERE key NOT LIKE 'set:%' AND key NOT LIKE 'foilset:%' AND created_at >= $1 GROUP BY user_id"
        }
        Board::Legendaries => {
            "SELECT user_id, count(*) AS n FROM owned_cards
             WHERE card = ANY($2) AND obtained_at >= $1 GROUP BY user_id"
        }
        Board::Foils => {
            "SELECT user_id, count(*) AS n FROM owned_cards
             WHERE foil IS NOT NULL AND obtained_at >= $1 GROUP BY user_id"
        }
        Board::Trades => {
            "SELECT p AS user_id, count(*) AS n FROM (
                 SELECT from_id AS p FROM trades WHERE state = 'done' AND updated_at >= $1
                 UNION ALL
                 SELECT to_id AS p FROM trades WHERE state = 'done' AND updated_at >= $1
             ) t GROUP BY p"
        }
        Board::Streak => "SELECT user_id, best::bigint AS n FROM streaks",
    }
}

#[derive(QueryableByName)]
struct RankRow {
    #[diesel(sql_type = Text)]
    username: String,
    #[diesel(sql_type = Int4)]
    avatar_version: i32,
    #[diesel(sql_type = Bool)]
    has_avatar: bool,
    #[diesel(sql_type = BigInt)]
    n: i64,
    #[diesel(sql_type = BigInt)]
    rank: i64,
    #[diesel(sql_type = Bool)]
    me: bool,
}

#[derive(Serialize)]
struct RankDto {
    rank: i64,
    username: String,
    avatar: Option<String>,
    value: i64,
    me: bool,
}

/// The top players of a board, and where the caller stands
#[get("/leaderboards")]
pub async fn leaderboard(
    state: web::Data<AppState>,
    user: CurrentUser,
    q: web::Query<BoardQuery>,
) -> ApiResult<HttpResponse> {
    if q.board == Board::Streak && q.period == Period::Week {
        return Err(ApiError::Invalid("board_all_time_only"));
    }
    let since = match q.period {
        Period::Week => Utc::now() - Duration::days(7),
        // Postgres timestamps do not reach chrono's minimum: the epoch is old enough
        Period::All => DateTime::<Utc>::UNIX_EPOCH,
    };
    let legendaries: Vec<String> =
        state.catalog.cards.values().filter(|c| c.rarity == Rarity::Legendary).map(|c| c.slug.clone()).collect();
    let sql = format!(
        "WITH s AS ({}), r AS (
             SELECT user_id, n, rank() OVER (ORDER BY n DESC) AS rank FROM s WHERE n > 0
         )
         SELECT u.username, u.avatar_version, u.avatar IS NOT NULL AS has_avatar, r.n, r.rank, r.user_id = $3 AS me
         FROM r JOIN users u ON u.id = r.user_id
         WHERE r.rank <= $4 OR r.user_id = $3
         ORDER BY r.rank, u.username",
        counts_sql(q.board)
    );
    let mut conn = state.pool.get().await?;
    let rows: Vec<RankRow> = diesel::sql_query(sql)
        .bind::<Timestamptz, _>(since)
        .bind::<Array<Text>, _>(&legendaries)
        .bind::<BigInt, _>(user.id)
        .bind::<BigInt, _>(TOP)
        .load(&mut conn)
        .await?;
    let mut top: Vec<RankDto> = rows
        .into_iter()
        .map(|r| RankDto {
            rank: r.rank,
            avatar: avatar_url(&r.username, r.avatar_version, r.has_avatar),
            username: r.username,
            value: r.n,
            me: r.me,
        })
        .collect();
    // Ties can fill the list past the cut: the caller's row is kept apart
    let me = top.iter().position(|r| r.me && r.rank > TOP).map(|i| top.remove(i));
    let me = me.or_else(|| {
        top.iter().find(|r| r.me).map(|r| RankDto { username: r.username.clone(), avatar: r.avatar.clone(), ..*r })
    });
    top.truncate(TOP as usize);
    Ok(HttpResponse::Ok().json(serde_json::json!({ "board": q.board, "period": q.period, "top": top, "me": me })))
}
