use anyhow::{Context, Result};
use definitions::{JetstreamCursor, Record};
use log::{debug, info};
use surrealdb::{
    engine::remote::ws::{Client, Ws},
    opt::auth::Root,
    RecordId, Surreal,
};

mod definitions;
pub mod handlers;
mod utils;

/// Connect to the database
pub async fn connect(
    dbhost: String,
    username: &str,
    password: &str,
) -> anyhow::Result<Surreal<Client>> {
    // connect to the database
    info!(target: "indexer", "Connecting to the database at {}", dbhost);
    let db: Surreal<Client> = Surreal::init();
    db.connect::<Ws>(&dbhost)
        .await
        .with_context(|| format!("Unable to open database connection to {}", dbhost))?;

    // sign in to the server
    debug!(target: "indexer", "Signing in as {}", username);
    db.signin(Root { username, password })
        .await
        .with_context(|| format!("Failed to sign in as {}", username))?;

    definitions::init(&db)
        .await
        .context("Failed to initialize database schema")?;

    Ok(db)
}

/// Fetch the current cursor from the database
pub async fn fetch_cursor(db: &Surreal<Client>, host: &str) -> Result<Option<JetstreamCursor>> {
    let res: Option<JetstreamCursor> = db.select(("cursor", host)).await?;

    Ok(res)
}

/// Write the cursor to the database
pub async fn write_cursor(db: &Surreal<Client>, host: &str, cursor: u64) -> Result<()> {
    let _: Option<Record> = db
        .upsert(("cursor", host))
        .content(JetstreamCursor {
            time_us: (cursor - 10_000_000),
        })
        .await?;

    Ok(())
}

/// Delete a record from the database
async fn delete_record(db: &Surreal<Client>, table: &str, key: &str) -> anyhow::Result<()> {
    let _: Option<()> = db.delete(RecordId::from_table_key(table, key)).await?;

    Ok(())
}
