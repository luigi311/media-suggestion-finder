use anyhow::{Context, Result};
use std::path::PathBuf;

use config::{Config, ConfigError, File};

mod comparison;
mod database_orm;
mod database_process;
mod files;
mod indexer;
mod media;
mod web;

use crate::database_orm::database_connect;
use crate::indexer::indexing;
use crate::web::webserver;

#[derive(Clone)]
struct AppConfig {
    pub database_path: String,
    pub media_path: PathBuf,
}

impl AppConfig {
    pub fn new() -> Result<Self, ConfigError> {
        let settings = Config::builder()
            .add_source(File::with_name("config.toml"))
            .build()?;
        let database_path: String = settings.get_string("database_path")?;
        let media_path: String = settings.get_string("media_path")?;

        Ok(AppConfig {
            database_path: database_path.into(),
            media_path: media_path.into(),
        })
    }
}

#[tokio::main]
#[hotpath::main]
async fn main() -> Result<()> {
    hotpath::tokio_runtime!();

    #[cfg(feature = "hotpath")]
    {
        use tracing_subscriber::prelude::*;

        tracing_subscriber::registry()
            .with(hotpath::sqlx_tracing_layer())
            .init();
    }

    let config = AppConfig::new().context("failed to load application configuration")?;

    // Generate rw database connection so migrations happen first
    let dsn_rw = format!("sqlite://{}?mode=rwc", config.database_path);
    let orm_database = database_connect(&dsn_rw, true)
        .await
        .context("failed to connect to database")?;

    tokio::spawn(hotpath::future!(
        indexing(config.clone(), orm_database),
        label = "indexing_worker"
    ));
    webserver(&config).await?;

    Ok(())
}
