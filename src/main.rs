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
struct FileConfig {
    pub enable: bool,
    pub concurrent_parse: usize,
    pub concurrent_hash: usize,
}

#[derive(Clone)]
struct AppConfig {
    pub database_path: String,
    pub media_path: PathBuf,
    pub file: FileConfig,
}

impl AppConfig {
    pub fn new() -> Result<Self, ConfigError> {
        let settings = Config::builder()
            .set_default("file.enabled", true)?
            .set_default("file.concurrent_parse", 4)?
            .set_default("file.concurrent_hash", 4)?
            .add_source(File::with_name("config.toml"))
            .build()?;
        let database_path: String = settings.get_string("database_path")?;
        let media_path: String = settings.get_string("media_path")?;
        let file_enable: bool = settings.get_bool("file.enable")?;
        let file_concurrent_parse: usize = settings.get::<usize>("file.concurrent_parse")?;
        let file_concurrent_hash: usize = settings.get::<usize>("file.concurrent_hash")?;

        if file_concurrent_parse <= 0 || file_concurrent_hash <= 0 {
            return Err(ConfigError::Message(
                "file.concurrent_* must be at least 1".into(),
            ));
        }

        Ok(AppConfig {
            database_path: database_path.into(),
            media_path: media_path.into(),
            file: FileConfig {
                enable: file_enable,
                concurrent_parse: file_concurrent_parse,
                concurrent_hash: file_concurrent_hash,
            },
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

    if config.file.enable {
        println!("Starting file scanner");
        tokio::spawn(hotpath::future!(
            indexing(config.clone(), orm_database),
            label = "indexing_worker"
        ));
    };
    webserver(&config).await?;

    Ok(())
}
