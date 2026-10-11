use anyhow::{Context, Result};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::{Router, routing::get};
use sea_orm::DatabaseConnection;
use tokio::fs;
use tokio::time::{Duration, sleep};

use crate::AppConfig;
use crate::database_orm::{MediaMetadata, database_connect, get_all_metadata};

pub async fn webserver(config: &AppConfig) -> Result<()> {
    // Wait for database to exist first
    loop {
        match fs::try_exists(&config.database_path).await {
            Ok(true) => break,
            Ok(false) => sleep(Duration::from_secs(3)),
            Err(e) => sleep(Duration::from_secs(3)),
        }
        .await
    }

    let dsn_ro = format!("sqlite://{}?mode=ro", &config.database_path);
    let orm_database = database_connect(&dsn_ro)
        .await
        .context("failed to connect to database")?;

    // build our application with a route
    let app = Router::new()
        .route("/metadata", get(metadata))
        .with_state(orm_database);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await?;

    Ok(())
}

async fn metadata(
    State(orm_database): State<DatabaseConnection>,
) -> Result<Json<Vec<MediaMetadata>>, StatusCode> {
    let metadata = get_all_metadata(&orm_database).await.map_err(|e| {
        eprintln!("Failed fetching media metadata: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(metadata))
}
