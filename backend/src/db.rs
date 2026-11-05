use diesel_async::AsyncPgConnection;
use diesel_async::async_connection_wrapper::AsyncConnectionWrapper;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::deadpool::Pool;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

pub type DbPool = Pool<AsyncPgConnection>;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn pool(url: &str) -> anyhow::Result<DbPool> {
    let manager = AsyncDieselConnectionManager::<AsyncPgConnection>::new(url);
    Ok(Pool::builder(manager).max_size(16).build()?)
}

/// Migrations run on a blocking thread: the harness is synchronous
pub async fn run_migrations(url: &str) -> anyhow::Result<()> {
    let url = url.to_owned();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        use diesel::Connection;
        let mut conn = AsyncConnectionWrapper::<AsyncPgConnection>::establish(&url)?;
        let applied = conn.run_pending_migrations(MIGRATIONS).map_err(|e| anyhow::anyhow!(e))?;
        for m in applied {
            tracing::info!("applied migration {m}");
        }
        Ok(())
    })
    .await?
}
