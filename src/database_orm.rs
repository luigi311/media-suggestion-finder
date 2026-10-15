use anyhow::Result;
use chrono::Utc;
use migration::{Expr, Migrator, MigratorTrait};
use sea_orm::{
    ConnectOptions, Database, DatabaseConnection, QueryFilter, QuerySelect, entity::*,
    prelude::DateTimeWithTimeZone,
};
use serde::Serialize;
use std::{collections::HashMap, time::Duration};

use entity::{prelude::*, *};

// Chunk size to avoid to many parameters at once due to database limits
const CHUNK_SIZE: usize = 1000;

pub enum SingleList {
    Single(i64),
    List(Vec<i64>),
}

#[hotpath::measure]
pub async fn database_connect(dsn: &String, migrate: bool) -> Result<DatabaseConnection> {
    let mut opt: ConnectOptions = ConnectOptions::new(dsn);
    opt.max_connections(10)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(cfg!(feature = "hotpath")); // SQL events are needed only while profiling.

    let conn: DatabaseConnection = Database::connect(opt).await?;
    if migrate {
        Migrator::up(&conn, None).await?
    };
    Ok(conn)
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
    hash: &String,
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

#[hotpath::measure]
pub async fn get_directories_with_media_files(conn: &DatabaseConnection) -> Result<Vec<i64>> {
    let result: Vec<i64> = MediaFiles::find()
        .select_only()
        .distinct()
        .column(files::Column::DirectoryId)
        .inner_join(files::Entity)
        .into_tuple()
        .all(conn)
        .await?;

    Ok(result)
}

#[derive(Clone, Debug, Serialize)]
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

#[derive(Default)]
struct Streams {
    videos: Vec<video_streams::Model>,
    audios: Vec<audio_streams::Model>,
    subtitles: Vec<subtitle_streams::Model>,
}

#[hotpath::measure]
pub async fn get_metadata_from_directory(
    conn: &DatabaseConnection,
    directory_id: SingleList,
) -> Result<HashMap<i64, Vec<MediaMetadata>>> {
    let directory_ids = match directory_id {
        SingleList::Single(id) => vec![id],
        SingleList::List(ids) => ids,
    };

    let mut metadata: HashMap<i64, Vec<MediaMetadata>> = HashMap::new();

    for dir_chunk in directory_ids.chunks(CHUNK_SIZE) {
        let files: Vec<(i64, i64, String, String, String, i64)> = Files::find()
            .select_only()
            .column(files::Column::Id)
            .column(files::Column::DirectoryId)
            .column(directories::Column::Path)
            .column(files::Column::Name)
            .column(files::Column::Extension)
            .column(files::Column::SizeBytes)
            .filter(files::Column::DirectoryId.is_in(dir_chunk))
            .filter(files::Column::MissingSince.is_null())
            .inner_join(Directories)
            .inner_join(MediaFiles) // Only fetch media file ids
            .into_tuple()
            .all(conn)
            .await?;

        let file_ids: Vec<i64> = files.iter().map(|x| x.0).collect();
        let mut streams: HashMap<i64, Streams> = HashMap::new();

        //rechunk since its possible to get a ton of file_ids
        for file_chunk in file_ids.chunks(CHUNK_SIZE) {
            for stream in VideoStreams::find()
                .filter(video_streams::Column::FileId.is_in(file_chunk))
                .all(conn)
                .await?
            {
                streams
                    .entry(stream.file_id)
                    .or_default()
                    .videos
                    .push(stream);
            }

            for stream in AudioStreams::find()
                .filter(audio_streams::Column::FileId.is_in(file_chunk))
                .all(conn)
                .await?
            {
                streams
                    .entry(stream.file_id)
                    .or_default()
                    .audios
                    .push(stream)
            }

            for stream in SubtitleStreams::find()
                .filter(subtitle_streams::Column::FileId.is_in(file_chunk))
                .all(conn)
                .await?
            {
                streams
                    .entry(stream.file_id)
                    .or_default()
                    .subtitles
                    .push(stream)
            }
        }

        for file in files {
            let stream = streams.remove(&file.0).unwrap_or_default();

            metadata.entry(file.1).or_default().push(MediaMetadata {
                file_id: file.0,
                directory_id: file.1,
                directory: file.2,
                name: file.3,
                extension: file.4,
                size: file.5,
                videos: stream.videos,
                audios: stream.audios,
                subtitles: stream.subtitles,
            })
        }
    }
    Ok(metadata)
}

#[hotpath::measure]
pub async fn get_file_metadata(
    conn: &DatabaseConnection,
    file_id: i64,
) -> Result<Option<MediaMetadata>> {
    let file_ids: Option<(i64, i64, String, String, String, i64)> = Files::find()
        .select_only()
        .column(files::Column::Id)
        .column(files::Column::DirectoryId)
        .column(directories::Column::Path)
        .column(files::Column::Name)
        .column(files::Column::Extension)
        .column(files::Column::SizeBytes)
        .filter(files::Column::Id.eq(file_id))
        .filter(files::Column::MissingSince.is_null())
        .inner_join(Directories)
        .inner_join(MediaFiles) // Only fetch media file ids
        .into_tuple()
        .one(conn)
        .await?;

    match file_ids {
        Some(file) => {
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

            Ok(Some(MediaMetadata {
                file_id: file.0,
                directory_id: file.1,
                directory: file.2,
                name: file.3,
                extension: file.4,
                size: file.5,
                videos: v_streams,
                audios: a_streams,
                subtitles: s_streams,
            }))
        }
        None => Ok(None),
    }
}

#[hotpath::measure]
pub async fn get_all_metadata(conn: &DatabaseConnection) -> Result<Vec<MediaMetadata>> {
    let files: Vec<(i64, i64, String, String, String, i64)> = Files::find()
        .select_only()
        .column(files::Column::Id)
        .column(files::Column::DirectoryId)
        .column(directories::Column::Path)
        .column(files::Column::Name)
        .column(files::Column::Extension)
        .column(files::Column::SizeBytes)
        .inner_join(Directories)
        .inner_join(MediaFiles) // Only fetch media file ids
        .into_tuple()
        .all(conn)
        .await?;

    let file_ids: Vec<i64> = files.iter().map(|x| x.0).collect();
    let mut streams: HashMap<i64, Streams> = HashMap::new();

    for file_chunk in file_ids.chunks(CHUNK_SIZE) {
        for stream in VideoStreams::find()
            .filter(video_streams::Column::FileId.is_in(file_chunk))
            .all(conn)
            .await?
        {
            streams
                .entry(stream.file_id)
                .or_default()
                .videos
                .push(stream);
        }

        for stream in AudioStreams::find()
            .filter(audio_streams::Column::FileId.is_in(file_chunk))
            .all(conn)
            .await?
        {
            streams
                .entry(stream.file_id)
                .or_default()
                .audios
                .push(stream);
        }

        for stream in SubtitleStreams::find()
            .filter(subtitle_streams::Column::FileId.is_in(file_chunk))
            .all(conn)
            .await?
        {
            streams
                .entry(stream.file_id)
                .or_default()
                .subtitles
                .push(stream);
        }
    }

    let mut metadata: Vec<MediaMetadata> = Vec::new();
    for file in files {
        let stream = streams.remove(&file.0).unwrap_or_default();

        metadata.push(MediaMetadata {
            file_id: file.0,
            directory_id: file.1,
            directory: file.2,
            name: file.3,
            extension: file.4,
            size: file.5,
            videos: stream.videos,
            audios: stream.audios,
            subtitles: stream.subtitles,
        })
    }

    Ok(metadata)
}

#[hotpath::measure]
pub async fn update_missing_files(conn: &DatabaseConnection, found: Vec<i64>) -> Result<()> {
    let file_ids_exists: Vec<i64> = Files::find()
        .select_only()
        .column(files::Column::Id)
        .filter(files::Column::MissingSince.is_null())
        .into_tuple()
        .all(conn)
        .await?;
    let file_ids_missing: Vec<i64> = Files::find()
        .select_only()
        .column(files::Column::Id)
        .filter(files::Column::MissingSince.is_not_null())
        .into_tuple()
        .all(conn)
        .await?;

    // Previously present, but wasn't found during this scan.
    let newly_missing: Vec<i64> = file_ids_exists
        .into_iter()
        .filter(|id| !found.contains(id))
        .collect();

    // Previously missing, but was found again during this scan.
    let restored: Vec<i64> = file_ids_missing
        .into_iter()
        .filter(|id| found.contains(id))
        .collect();

    if !newly_missing.is_empty() {
        println!("Newly missing {:?}", newly_missing);
        let now: DateTimeWithTimeZone = Utc::now().into();

        Files::update_many()
            .col_expr(files::Column::MissingSince, Expr::value(now))
            .filter(files::Column::Id.is_in(newly_missing))
            .exec(conn)
            .await?;
    }

    if !restored.is_empty() {
        println!("Restored {:?}", restored);
        Files::update_many()
            .col_expr(
                files::Column::MissingSince,
                Expr::value(Option::<DateTimeWithTimeZone>::None),
            )
            .filter(files::Column::Id.is_in(restored))
            .exec(conn)
            .await?;
    }

    Ok(())
}
