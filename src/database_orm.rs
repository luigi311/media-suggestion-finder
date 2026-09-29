use anyhow::Result;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, QueryFilter, QuerySelect, entity::*};
use std::time::Duration;

use entity::{prelude::*, *};

use crate::media::Language;

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
    Ok(conn)
}

pub async fn get_language_id(conn: &DatabaseConnection, language: Language) -> Result<Option<i64>> {
    let result = Languages::find()
        .filter(languages::Column::Iso6392.eq(language.iso_639_2()))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_directory_id(conn: &DatabaseConnection, path: &str) -> Result<Option<i64>> {
    let result: Option<directories::Model> = Directories::find()
        .filter(directories::Column::Path.eq(path))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_file_primary_id_from_name_extension(
    conn: &DatabaseConnection,
    directory_id: i64,
    name: &str,
    extension: &str,
) -> Result<Option<i64>> {
    let result: Option<files::Model> = Files::find()
        .filter(files::Column::DirectoryId.eq(directory_id))
        .filter(files::Column::Name.eq(name))
        .filter(files::Column::Extension.eq(extension))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_file_primary_id_from_hash(
    conn: &DatabaseConnection,
    directory_id: i64,
    hash: &str,
) -> Result<Option<i64>> {
    let result: Option<files::Model> = Files::find()
        .filter(files::Column::DirectoryId.eq(directory_id))
        .filter(files::Column::Hash.eq(hash))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_media_file_primary_id_from_file_id(
    conn: &DatabaseConnection,
    file_id: i64,
) -> Result<Option<i64>> {
    let result: Option<media_files::Model> = MediaFiles::find()
        .filter(media_files::Column::FileId.eq(file_id))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_subtitle_file_primary_id_from_file_id(
    conn: &DatabaseConnection,
    file_id: i64,
) -> Result<Option<i64>> {
    let result: Option<subtitle_files::Model> = SubtitleFiles::find()
        .filter(subtitle_files::Column::FileId.eq(file_id))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_video_stream_primary_id_from_file_stream(
    conn: &DatabaseConnection,
    file_id: i64,
    index: i32,
) -> Result<Option<i64>> {
    let result: Option<video_streams::Model> = VideoStreams::find()
        .filter(video_streams::Column::FileId.eq(file_id))
        .filter(video_streams::Column::StreamIndex.eq(index))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_audio_stream_primary_id_from_file_stream(
    conn: &DatabaseConnection,
    file_id: i64,
    index: i32,
) -> Result<Option<i64>> {
    let result: Option<audio_streams::Model> = AudioStreams::find()
        .filter(audio_streams::Column::FileId.eq(file_id))
        .filter(audio_streams::Column::StreamIndex.eq(index))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_subtitle_stream_primary_id_from_file_stream(
    conn: &DatabaseConnection,
    file_id: i64,
    index: i32,
) -> Result<Option<i64>> {
    let result: Option<subtitle_streams::Model> = SubtitleStreams::find()
        .filter(subtitle_streams::Column::FileId.eq(file_id))
        .filter(subtitle_streams::Column::StreamIndex.eq(index))
        .one(conn)
        .await?;

    Ok(result.map(|res| res.id))
}

pub async fn get_directories_with_media_files(conn: &DatabaseConnection) -> Result<Vec<i64>> {
    let result: Vec<i64> = MediaFiles::find()
        .select_only()
        .column(files::Column::DirectoryId)
        .inner_join(files::Entity)
        .into_tuple()
        .all(conn)
        .await?;

    Ok(result)
}

#[derive(Clone, Debug)]
pub struct MediaMetadata {
    pub file_id: i64,
    pub directory_id: i64,
    pub directory: String,
    pub name: String,
    pub extension: String,
    pub size: i64,

    pub videos: Vec<video_streams::Model>,
    pub audios: Vec<audio_streams::Model>,
    pub subtitles: Vec<subtitle_streams::Model>,
}

pub async fn get_metadata_from_directory(
    conn: &DatabaseConnection,
    directory_id: i64,
) -> Result<Vec<MediaMetadata>> {
    let file_ids: Vec<(i64, i64, String, String, String, i64)> = Files::find()
        .select_only()
        .column(files::Column::Id)
        .column(files::Column::DirectoryId)
        .column(directories::Column::Path)
        .column(files::Column::Name)
        .column(files::Column::Extension)
        .column(files::Column::SizeBytes)
        .filter(files::Column::DirectoryId.eq(directory_id))
        .inner_join(Directories)
        .inner_join(MediaFiles) // Only fetch media file ids
        .into_tuple()
        .all(conn)
        .await?;

    let mut metadata: Vec<MediaMetadata> = Vec::new();
    for file in file_ids {
        let v_streams: Vec<video_streams::Model> = VideoStreams::find()
            .filter(video_streams::Column::FileId.eq(file.0))
            .all(conn)
            .await?;

        let a_streams: Vec<audio_streams::Model> = AudioStreams::find()
            .filter(audio_streams::Column::FileId.eq(file.0))
            .all(conn)
            .await?;

        let s_streams: Vec<subtitle_streams::Model> = SubtitleStreams::find()
            .filter(subtitle_streams::Column::FileId.eq(file.0))
            .all(conn)
            .await?;

        metadata.push(MediaMetadata {
            file_id: file.0,
            directory_id: file.1,
            directory: file.2,
            name: file.3,
            extension: file.4,
            size: file.5,
            videos: v_streams,
            audios: a_streams,
            subtitles: s_streams,
        })
    }

    Ok(metadata)
}
