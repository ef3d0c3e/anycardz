//! Coin top-ups. No payment provider yet: a top-up grants its coins at once
//! and is logged in `topups`.

use actix_web::{HttpResponse, get, post, web};
use diesel::prelude::*;
use diesel_async::{AsyncConnection, RunQueryDsl};
use serde::Deserialize;

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::game::{TOPUP_PACKS, topup_pack};
use crate::schema::{topups, users};

#[get("/shop/topups")]
pub async fn list_topups() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({ "packs": TOPUP_PACKS, "checkout": false }))
}

#[derive(Deserialize)]
pub struct TopupBody {
    pack: String,
}

#[post("/shop/topup")]
pub async fn topup(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<TopupBody>,
) -> ApiResult<HttpResponse> {
    let pack = topup_pack(&body.pack).ok_or(ApiError::NotFound("unknown_pack"))?;
    let mut conn = state.pool.get().await?;
    let uid = user.id;
    let coins = conn
        .transaction::<_, ApiError, _>(async move |conn| {
            diesel::insert_into(topups::table)
                .values((
                    topups::user_id.eq(uid),
                    topups::pack.eq(pack.id),
                    topups::coins.eq(pack.coins),
                    topups::price_cents.eq(pack.price_cents),
                ))
                .execute(conn)
                .await?;
            let coins: i64 = diesel::update(users::table.find(uid))
                .set(users::coins.eq(users::coins + pack.coins))
                .returning(users::coins)
                .get_result(conn)
                .await?;
            Ok(coins)
        })
        .await?;
    state.metrics.earned("topup", pack.coins);
    Ok(HttpResponse::Ok().json(serde_json::json!({ "coins": coins, "added": pack.coins })))
}
