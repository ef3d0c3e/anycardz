use std::sync::Arc;

use actix_web::{App, HttpServer, middleware, web};
use anycardz_api::metrics;
use anycardz_api::{AppState, Catalog, Config, db, routes};
use anyhow::Context;
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    // `anycardz-api vapid-keys`: a key pair for Web Push, to put in the environment
    if std::env::args().nth(1).as_deref() == Some("vapid-keys") {
        let (private, public) = anycardz_api::push::Vapid::generate();
        println!("ANYCARDZ_VAPID_PRIVATE_KEY={private}\n# public key (served at /api/push): {public}");
        return Ok(());
    }
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

    let config = Config::from_env()?;
    let catalog = Catalog::load(&config.data_dir, config.dev_collections)?;
    tracing::info!("loaded {} collections, {} cards", catalog.collections.len(), catalog.cards.len());
    db::run_migrations(&config.database_url).await?;
    let pool = db::pool(&config.database_url)?;

    let bind = config.bind.clone();
    let limits = if std::env::var("ANYCARDZ_RELAXED_LIMITS").is_ok_and(|v| v == "1") {
        tracing::warn!("auth rate limits relaxed (ANYCARDZ_RELAXED_LIMITS=1): development only");
        anycardz_api::ratelimit::AuthLimits::relaxed()
    } else {
        Default::default()
    };
    let push = match (&config.vapid_private_key, config.push_subject()) {
        (Some(key), Some(subject)) => {
            let vapid = anycardz_api::push::Vapid::new(key, subject)
                .context("ANYCARDZ_VAPID_PRIVATE_KEY is not a P-256 key")?;
            tracing::info!("web push on");
            anycardz_api::push::Push { vapid: Some(vapid), transport: anycardz_api::push::Transport::http() }
        }
        (Some(_), None) => {
            tracing::warn!("web push off: set ANYCARDZ_VAPID_SUBJECT or an email ANYCARDZ_CONTACT");
            anycardz_api::push::Push::disabled()
        }
        (None, _) => anycardz_api::push::Push::disabled(),
    };
    let state = web::Data::new(AppState {
        pool,
        live: Default::default(),
        metrics: Default::default(),
        limits,
        catalog: Arc::new(catalog),
        config: Arc::new(config),
        push: Arc::new(push),
    });
    // Prometheus gets its own listener, kept off the public site
    let scraper = match state.config.metrics_bind.clone() {
        Some(addr) => {
            let state = state.clone();
            tracing::info!("metrics on http://{addr}/metrics");
            let server = HttpServer::new(move || App::new().app_data(state.clone()).service(metrics::scrape))
                .workers(1)
                .bind(addr)?
                .run();
            Some(server)
        }
        None => None,
    };
    tracing::info!("listening on http://{bind}");
    let site = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(middleware::from_fn(metrics::track))
            .wrap(middleware::Compress::default())
            .wrap(tracing_actix_web::TracingLogger::default())
            .configure(routes::configure)
    })
    .bind(bind)?
    .run();
    match scraper {
        Some(scraper) => {
            futures_util::future::try_join(site, scraper).await?;
        }
        None => site.await?,
    }
    Ok(())
}
