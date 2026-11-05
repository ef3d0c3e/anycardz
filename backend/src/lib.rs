pub mod auth;
pub mod config;
pub mod data;
pub mod db;
pub mod error;
pub mod game;
pub mod i18n;
pub mod inventory;
pub mod live;
pub mod ratelimit;
pub mod routes;
pub mod schema;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

use std::sync::Arc;

pub use config::Config;
pub use data::Catalog;

pub struct AppState {
    pub pool: db::DbPool,
    pub live: live::Hub,
    pub limits: ratelimit::AuthLimits,
    pub catalog: Arc<Catalog>,
    pub config: Arc<Config>,
}
