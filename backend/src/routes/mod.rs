use actix_web::{HttpResponse, get, web};

use crate::auth;

pub mod catalog;
pub mod forge;
pub mod player;
pub mod players;
pub mod shop;
pub mod trades;

#[get("/health")]
async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({ "ok": true }))
}

/// Every route lives under /api
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .app_data(web::JsonConfig::default().limit(16 * 1024).error_handler(|err, _| {
                tracing::debug!("bad json: {err}");
                crate::error::ApiError::Invalid("invalid_body").into()
            }))
            .service(health)
            .service(auth::register)
            .service(auth::login)
            .service(auth::logout)
            .service(catalog::list_collections)
            .service(catalog::get_collection)
            .service(catalog::card_art)
            .service(catalog::get_card)
            .service(catalog::get_asset)
            .service(player::get_me)
            .service(player::buy_booster)
            .service(shop::list_topups)
            .service(shop::topup)
            .service(player::open_booster)
            .service(player::claim_streak)
            .service(player::get_binder)
            .service(player::sell_card)
            .service(forge::tradeup)
            .service(forge::roulette)
            .service(players::search_players)
            .service(players::player_cards)
            .service(trades::open_trade)
            .service(trades::list_trades)
            .service(trades::get_trade)
            .service(trades::edit_trade)
            .service(trades::seal_trade)
            .service(trades::cancel_trade),
    );
}
