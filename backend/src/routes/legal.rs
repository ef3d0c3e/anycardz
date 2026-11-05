//! The terms of service and privacy notice. Their text lives in the frontend
//! (both languages); the API knows the version players must accept and who
//! runs the site.

use actix_web::{HttpResponse, get, post, web};
use chrono::Utc;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::Deserialize;

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::schema::users;

/// Raise with every change to the terms players must agree to again. Keep in
/// step with TERMS_VERSION in frontend/src/lib/legal.ts.
pub const TERMS_VERSION: i32 = 1;
pub const TERMS_UPDATED: &str = "2026-11-05";

#[get("/legal")]
pub async fn get_legal(state: web::Data<AppState>) -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "version": TERMS_VERSION,
        "updated": TERMS_UPDATED,
        "operator": state.config.operator,
        "contact": state.config.contact,
    }))
}

#[derive(Deserialize)]
pub struct AcceptBody {
    version: i32,
}

/// For players whose accepted version is older than the current one
#[post("/legal/accept")]
pub async fn accept(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<AcceptBody>,
) -> ApiResult<HttpResponse> {
    // Accepting a version the player never saw would mean nothing
    if body.version != TERMS_VERSION {
        return Err(ApiError::Conflict("terms_changed"));
    }
    let mut conn = state.pool.get().await?;
    diesel::update(users::table.find(user.id))
        .set((users::terms_version.eq(TERMS_VERSION), users::terms_accepted_at.eq(Utc::now())))
        .execute(&mut conn)
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "version": TERMS_VERSION })))
}
