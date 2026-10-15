use anyhow::{Context, Result};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Router, routing::get};
use sea_orm::DatabaseConnection;

use crate::AppConfig;
use crate::comparison::{MediaOutlier, SiblingAnalysis, analyse_siblings, generate_outliers};
use crate::database_orm::{
    MediaMetadata, SingleList, database_connect, get_all_metadata,
    get_directories_with_media_files, get_file_metadata, get_metadata_from_directory,
};

#[hotpath::measure]
pub async fn webserver(config: &AppConfig) -> Result<()> {
    let dsn_ro = format!("sqlite://{}?mode=ro", &config.database_path);
    let orm_database = database_connect(&dsn_ro, false)
        .await
        .context("failed to connect to database")?;

    // build our application with a route
    let app = Router::new()
        .route("/", get(outliers))
        .route("/metadata", get(metadata))
        .route("/metadata/{file_id}", get(metadata_file))
        .route("/dominant", get(dominant))
        .with_state(orm_database);
    let app = hotpath::axum!(app);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await?;

    Ok(())
}

#[hotpath::measure]
async fn metadata(
    State(orm_database): State<DatabaseConnection>,
) -> Result<Json<Vec<MediaMetadata>>, StatusCode> {
    let metadata = get_all_metadata(&orm_database).await.map_err(|e| {
        eprintln!("Failed fetching media metadata: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(metadata))
}

#[hotpath::measure]
async fn metadata_file(
    Path(file_id): Path<i64>,
    State(orm_database): State<DatabaseConnection>,
) -> Result<Response, StatusCode> {
    let metadata = get_file_metadata(&orm_database, file_id)
        .await
        .map_err(|e| {
            eprintln!("Failed fetching media metadata: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(match metadata {
        Some(metadata) => Json(metadata).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    })
}

#[hotpath::measure]
async fn dominant(
    State(orm_database): State<DatabaseConnection>,
) -> Result<Json<SiblingAnalysis>, StatusCode> {
    let metadata = get_all_metadata(&orm_database).await.map_err(|e| {
        eprintln!("Failed fetching media metadata: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let analysis = analyse_siblings(&metadata).await.map_err(|e| {
        eprintln!("Failed analysing sibling metadata: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(analysis))
}

#[hotpath::measure]
async fn outliers(
    State(orm_database): State<DatabaseConnection>,
) -> Result<Json<Vec<MediaOutlier>>, StatusCode> {
    let directories_ids = get_directories_with_media_files(&orm_database)
        .await
        .map_err(|e| {
            eprintln!("failed fetching directories with media files: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let mut outliers: Vec<MediaOutlier> = Vec::new();
    let metadatas = get_metadata_from_directory(&orm_database, SingleList::List(directories_ids))
        .await
        .map_err(|e| {
            eprintln!("failed fetching media metadata for directory ids: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    for (dir_id, metadata) in metadatas {
        let analysis = generate_outliers(metadata).await.map_err(|e| {
            eprintln!("failed to generate outliers for directory id {dir_id}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        outliers.extend(analysis);
    }

    Ok(Json(outliers))
}
