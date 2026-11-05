use std::sync::Arc;

use actix_web::{App, HttpServer, middleware, web};
use anycardz_api::{AppState, Catalog, Config, db, routes};
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

    let config = Config::from_env()?;
    let catalog = Catalog::load(&config.data_dir, config.dev_collections)?;
    tracing::info!("loaded {} collections, {} cards", catalog.collections.len(), catalog.cards.len());
    db::run_migrations(&config.database_url).await?;
    let pool = db::pool(&config.database_url)?;

    let bind = config.bind.clone();
    let state = web::Data::new(AppState { pool, catalog: Arc::new(catalog), config: Arc::new(config) });
    tracing::info!("listening on http://{bind}");
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Compress::default())
            .wrap(tracing_actix_web::TracingLogger::default())
            .configure(routes::configure)
    })
    .bind(bind)?
    .run()
    .await?;
    Ok(())
}
