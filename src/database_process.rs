use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use entity::{
    audio_streams, directories, files, media_files, subtitle_files, subtitle_streams, video_streams,
};
use imohash::Hasher;
use sea_orm::{
    ActiveValue::Set, DatabaseConnection, EntityTrait, InsertResult, prelude::DateTimeWithTimeZone,
};
use std::path::Path;
use tokio::fs;

use crate::{database_orm, media};
use database_orm::{
    get_audio_stream_primary_id_from_file_stream, get_directory_id, get_file_primary_id_from_name,
    get_media_file_primary_id_from_file_id, get_subtitle_file_primary_id_from_file_id,
    get_subtitle_stream_primary_id_from_file_stream, get_video_stream_primary_id_from_file_stream,
};

use media::{AudioStream, SubtitleStream, VideoStream};

pub async fn process_directory(database: &DatabaseConnection, directory: &Path) -> Result<i64> {
    let dir_str = directory.to_str().expect("Failed to convert to str");
    let mut directory_id: Option<i64> = get_directory_id(database, dir_str).await?;

    if directory_id.is_none() {
        // navigate the ancestors in reverse order to build the full tree
        let ancestors: Vec<&Path> = directory.ancestors().collect();
        for ancestor in ancestors.iter().rev() {
            let anscestor_str: &str = ancestor
                .to_str()
                .expect("Failed to convert anscestor to str");
            let ancestor_directory_id = get_directory_id(database, anscestor_str).await?;

            // If ancestor already exists, skip
            // set ancestor_directory_id to directory_id for parent mapping on missing
            if ancestor_directory_id.is_some() {
                directory_id = ancestor_directory_id;
                continue;
            }

            println!("Inserting directory: {anscestor_str}");
            let directory: directories::ActiveModel = directories::ActiveModel {
                path: Set(anscestor_str.to_string()),
                parent_id: Set(directory_id),
                ..Default::default()
            };
            let res: InsertResult<directories::ActiveModel> =
                directories::Entity::insert(directory)
                    .exec(database)
                    .await?;
            directory_id = Some(res.last_insert_id);
        }
    }

    directory_id.context("Failed to generate directory id")
}

pub async fn process_file(
    database: &DatabaseConnection,
    directory_id: i64,
    file: &Path,
    hasher: Hasher,
) -> Result<i64> {
    let file_path_str = file.to_str().expect("Failed to convert to str");
    let file_name = file
        .file_name()
        .expect("Failed to get file name")
        .to_str()
        .expect("Failed to convert to str");

    // TODO: Check via hash first, if exists, then skip, if not then fallback to path and then update info
    let mut file_id = get_file_primary_id_from_name(database, directory_id, file_name)
        .await
        .with_context(|| {
            format!("failed looking up file from directory_id={directory_id} file_name={file_name}")
        })?;

    match file_id {
        Some(_) => {}
        None => {
            println!("Inserting file: {}", file.display());
            let extension = file
                .extension()
                .expect("Failed to get extension")
                .to_str()
                .expect("Failed to convert to string");

            let metadata = fs::metadata(file).await?;
            let size: i64 = metadata.len() as i64;
            let created_at = metadata.created()?;
            let modified_at = metadata.modified()?;
            // TODO: Replace with jiff as chrono is soft dropped, waiting on seaorm support
            let created_chrono: DateTimeWithTimeZone = DateTime::<Utc>::from(created_at).into();
            let modified_chrono: DateTimeWithTimeZone = DateTime::<Utc>::from(modified_at).into();

            let hash = hasher.sum_file(file_path_str)?;

            let file: files::ActiveModel = files::ActiveModel {
                directory_id: Set(directory_id),
                name: Set(file_name.to_string()),
                extension: Set(extension.to_string()),
                hash: Set(hash.to_string()),
                hash_algorithm: Set("imohash".to_string()),
                size_bytes: Set(size),
                created_at: Set(created_chrono),
                modified_at: Set(modified_chrono),
                ..Default::default()
            };
            let res: InsertResult<files::ActiveModel> = files::Entity::insert(file)
                .exec(database)
                .await
                .with_context(|| {
                    format!(
                        "failed inserting files: \
                            directory_id={directory_id}, \
                            name={file_name}"
                    )
                })?;

            file_id = Some(res.last_insert_id);
        }
    }

    file_id.context("Failed to generate file id")
}

pub async fn process_media_file(database: &DatabaseConnection, file_id: i64) -> Result<i64> {
    let mut media_id = get_media_file_primary_id_from_file_id(database, file_id)
        .await
        .with_context(|| format!("failed looking up media_file from file_id={file_id}"))?;

    match media_id {
        Some(_) => {}
        None => {
            let media: media_files::ActiveModel = media_files::ActiveModel {
                file_id: Set(file_id),
                ..Default::default()
            };
            let res: InsertResult<media_files::ActiveModel> = media_files::Entity::insert(media)
                .exec(database)
                .await
                .with_context(|| {
                    format!(
                        "failed inserting media_files: \
                            file_id={file_id}"
                    )
                })?;

            media_id = Some(res.last_insert_id);
        }
    }

    media_id.context("Failed to generate media id")
}

pub async fn process_subtitle_file(
    database: &DatabaseConnection,
    file_id: i64,
    media_file_id: i64,
) -> Result<i64> {
    let mut subtitle_id = get_subtitle_file_primary_id_from_file_id(database, file_id)
        .await
        .with_context(|| format!("failed looking up subtitle_file from file_id={file_id}"))?;

    match subtitle_id {
        Some(_) => {}
        None => {
            let subtitle: subtitle_files::ActiveModel = subtitle_files::ActiveModel {
                file_id: Set(file_id),
                media_file_id: Set(media_file_id),
                ..Default::default()
            };
            let res: InsertResult<subtitle_files::ActiveModel> =
                subtitle_files::Entity::insert(subtitle)
                    .exec(database)
                    .await
                    .with_context(|| {
                        format!(
                            "failed inserting subtitle_files: \
                                file_id={file_id}, \
                                media_file_id={media_file_id}"
                        )
                    })?;

            subtitle_id = Some(res.last_insert_id);
        }
    }

    subtitle_id.context("Failed to generate subtitle id")
}

pub async fn process_video_stream(
    database: &DatabaseConnection,
    file_id: i64,
    video: VideoStream,
) -> Result<i64> {
    let video_index = video.index;

    let mut video_stream_id =
        get_video_stream_primary_id_from_file_stream(database, file_id, video_index)
            .await
            .with_context(|| {
                format!(
                    "failed looking up audio stream file_id={file_id}, stream_index={video_index}"
                )
            })?;

    match video_stream_id {
        // TODO: Handle updating existing video_stream
        Some(_) => {}
        None => {
            let vid_bit: Option<i64> = video.bitdepth.map(|l| l as i64);
            let stream: video_streams::ActiveModel = video_streams::ActiveModel {
                file_id: Set(file_id),
                stream_index: Set(video_index.into()),
                codec: Set(video.codec),
                width: Set(video.width.into()),
                height: Set(video.height.into()),
                bit_depth: Set(vid_bit),
                hdr_format: Set(None),
                bitrate: Set(video.bitrate),
                ..Default::default()
            };
            let res: InsertResult<video_streams::ActiveModel> =
                video_streams::Entity::insert(stream)
                    .exec(database)
                    .await
                    .with_context(|| {
                        format!(
                            "failed inserting video_streams: \
                            file_id={file_id}, \
                            stream_index={video_index}"
                        )
                    })?;

            video_stream_id = Some(res.last_insert_id);
        }
    }

    video_stream_id.context("Failed to generate video stream id")
}

pub async fn process_audio_stream(
    database: &DatabaseConnection,
    file_id: i64,
    audio: AudioStream,
) -> Result<i64> {
    let audio_index = audio.index;

    let mut audio_stream_id =
        get_audio_stream_primary_id_from_file_stream(database, file_id, audio_index)
            .await
            .with_context(|| {
                format!(
                    "failed looking up audio stream file_id={file_id}, stream_index={audio_index}"
                )
            })?;

    match audio_stream_id {
        // TODO: Handle updating existing audio_stream
        Some(_) => {}
        None => {
            let audio_lang: Option<i64> = audio.language.map(|l| l as i64);
            let stream: audio_streams::ActiveModel = audio_streams::ActiveModel {
                file_id: Set(file_id),
                stream_index: Set(audio.index.into()),
                title: Set(audio.title),
                codec: Set(audio.codec),
                language_id: Set(audio_lang),
                channels: Set(audio.channels.into()),
                bitrate: Set(audio.bitrate),
                is_default: Set(audio.default),
                is_forced: Set(audio.forced),
                ..Default::default()
            };
            let res: InsertResult<audio_streams::ActiveModel> =
                audio_streams::Entity::insert(stream)
                    .exec(database)
                    .await
                    .with_context(|| {
                        format!(
                            "failed inserting audio_streams: \
                            file_id={file_id}, \
                            stream_index={audio_index}"
                        )
                    })?;

            audio_stream_id = Some(res.last_insert_id);
        }
    }

    audio_stream_id.context("Failed to generate audio stream id")
}

pub async fn process_subtitle_stream(
    database: &DatabaseConnection,
    file_id: i64,
    subtitle: SubtitleStream,
) -> Result<i64> {
    let sub_index = subtitle.index;

    let mut subtitle_stream_id =
        get_subtitle_stream_primary_id_from_file_stream(database, file_id, sub_index)
            .await
            .with_context(|| {
                format!(
                    "failed looking up subtitle stream file_id={file_id}, stream_index={sub_index}"
                )
            })?;

    match subtitle_stream_id {
        // TODO: Handle updating existing subtitle_streams
        Some(_) => {}
        None => {
            let sub_lang: Option<i64> = subtitle.language.map(|l| l as i64);
            let stream: subtitle_streams::ActiveModel = subtitle_streams::ActiveModel {
                file_id: Set(file_id),
                stream_index: Set(subtitle.index.into()),
                title: Set(subtitle.title),
                codec: Set(subtitle.codec),
                language_id: Set(sub_lang),
                is_default: Set(subtitle.default),
                is_forced: Set(subtitle.forced),
                is_sdh: Set(subtitle.sdh),
                ..Default::default()
            };
            let res = subtitle_streams::Entity::insert(stream)
                .exec(database)
                .await
                .with_context(|| {
                    format!(
                        "failed inserting subtitle_stream: \
                            file_id={file_id}, \
                            stream_index={sub_index}"
                    )
                })?;

            subtitle_stream_id = Some(res.last_insert_id);
        }
    }

    subtitle_stream_id.context("Failed to generate subtitle stream id")
}
