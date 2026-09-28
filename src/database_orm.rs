use std::{time::Duration};
use anyhow::Result;
use sea_orm::{ConnectOptions, Database, DatabaseConnection, QueryFilter, entity::*};
use migration::{Migrator, MigratorTrait};

use entity::{*, prelude::*};
pub async fn database_connect(dsn: &String) -> Result<DatabaseConnection> {
    let mut opt: ConnectOptions = ConnectOptions::new(dsn);
    opt.max_connections(10)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(false); // disable SQLx logging

    let conn: DatabaseConnection = Database::connect(opt).await?;
    Migrator::up(&conn, None).await?;
    Ok( conn )
}

pub async fn get_directory_id(conn: &DatabaseConnection, path: &str) -> Result<Option<i64>> {
    let result: Option<directories::Model> = Directories::find()
        .filter(directories::Column::Path.eq(path))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_file_primary_id_from_name(conn: &DatabaseConnection, directory_id: i64, name: &str) -> Result<Option<i64>> {
    let result: Option<files::Model> = Files::find()
        .filter(files::Column::DirectoryId.eq(directory_id))
        .filter(files::Column::Name.eq(name))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_file_primary_id_from_hash(conn: &DatabaseConnection, directory_id: i64, hash: &str) -> Result<Option<i64>> {
    let result: Option<files::Model> = Files::find()
        .filter(files::Column::DirectoryId.eq(directory_id))
        .filter(files::Column::Hash.eq(hash))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_media_file_primary_id_from_file_id(conn: &DatabaseConnection, file_id: i64) -> Result<Option<i64>> {
    let result: Option<media_files::Model> = MediaFiles::find()
        .filter(media_files::Column::FileId.eq(file_id))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_subtitle_file_primary_id_from_file_id(conn: &DatabaseConnection, file_id: i64) -> Result<Option<i64>> {
    let result: Option<subtitle_files::Model> = SubtitleFiles::find()
        .filter(subtitle_files::Column::FileId.eq(file_id))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_video_stream_primary_id_from_file_stream(conn: &DatabaseConnection, file_id: i64, index: i32) -> Result<Option<i64>> {
    let result: Option<video_streams::Model> = VideoStreams::find()
        .filter(video_streams::Column::FileId.eq(file_id))
        .filter(video_streams::Column::StreamIndex.eq(index))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_audio_stream_primary_id_from_file_stream(conn: &DatabaseConnection, file_id: i64, index: i32) -> Result<Option<i64>> {
    let result: Option<audio_streams::Model> = AudioStreams::find()
        .filter(audio_streams::Column::FileId.eq(file_id))
        .filter(audio_streams::Column::StreamIndex.eq(index))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_subtitle_stream_primary_id_from_file_stream(conn: &DatabaseConnection, file_id: i64, index: i32) -> Result<Option<i64>> {
    let result: Option<subtitle_streams::Model> = SubtitleStreams::find()
        .filter(subtitle_streams::Column::FileId.eq(file_id))
        .filter(subtitle_streams::Column::StreamIndex.eq(index))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}
