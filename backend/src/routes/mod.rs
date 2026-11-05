use actix_web::{HttpResponse, get, web};

use crate::auth;

pub mod account;
pub mod achievements;
pub mod catalog;
pub mod forge;
pub mod legal;
pub mod live;
pub mod market;
pub mod notifications;
pub mod player;
pub mod players;
pub mod push;
pub mod search;
pub mod shop;
pub mod social;
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
            .app_data(web::PayloadConfig::new(social::MAX_AVATAR_BYTES))
            .service(health)
            .service(auth::register)
            .service(auth::login)
            .service(auth::logout)
            .service(legal::get_legal)
            .service(legal::accept)
            .service(account::change_password)
            .service(account::change_email)
            .service(account::list_sessions)
            .service(account::revoke_other_sessions)
            .service(account::revoke_session)
            .service(account::export)
            .service(account::delete_account)
            .service(live::live)
            .service(live::get_shoutbox)
            .service(live::post_shout)
            .service(live::cards_batch)
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
            .service(player::claim_free_packs)
            .service(player::get_missions)
            .service(player::scratch_ticket)
            .service(player::get_binder)
            .service(player::sell_card)
            .service(forge::tradeup)
            .service(forge::roulette)
            .service(players::search_players)
            .service(players::player_cards)
            .service(social::get_profile)
            .service(social::edit_profile)
            .service(social::put_avatar)
            .service(social::delete_avatar)
            .service(social::get_avatar)
            .service(social::list_friends)
            .service(social::add_friend)
            .service(social::remove_friend)
            .service(social::list_blocks)
            .service(social::block)
            .service(social::unblock)
            .service(notifications::list)
            .service(notifications::mark_read)
            .service(search::search)
            .service(achievements::list)
            .service(achievements::leaderboard)
            .service(push::status)
            .service(push::subscribe)
            .service(push::unsubscribe)
            .service(market::browse)
            .service(market::mine)
            .service(market::list)
            .service(market::withdraw)
            .service(market::buy)
            .service(trades::open_trade)
            .service(trades::list_trades)
            .service(trades::get_trade)
            .service(trades::edit_trade)
            .service(trades::seal_trade)
            .service(trades::cancel_trade),
    );
}
