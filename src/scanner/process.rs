use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use entity::{
    audio_streams, directories, files, media_files, subtitle_files, subtitle_streams, video_streams,
};
use sea_orm::ActiveModelTrait;
use sea_orm::{
    ActiveValue::Set, DatabaseConnection, EntityTrait, InsertResult, prelude::DateTimeWithTimeZone,
};
use std::path::Path;
use tokio::fs;

use crate::database_orm::{
    drop_audio_stream_from_file_stream, drop_file_from_id, drop_subtitle_stream_from_file_stream,
    drop_video_stream_from_file_stream, get_audio_stream_indexes_from_file,
    get_audio_stream_primary_id_from_file_stream, get_directory_id, get_file_primary_id_from_hash,
    get_file_primary_id_from_name_extension, get_media_file_primary_id_from_file_id,
    get_subtitle_file_primary_id_from_file_id, get_subtitle_stream_indexes_from_file,
    get_subtitle_stream_primary_id_from_file_stream, get_video_stream_indexes_from_file,
    get_video_stream_primary_id_from_file_stream,
};
use crate::scanner::media::{AudioStream, SubtitleStream, VideoStream};

#[hotpath::measure]
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

#[hotpath::measure]
pub async fn process_file(
    database: &DatabaseConnection,
    directory_id: i64,
    file: &Path,
    hash: String,
) -> Result<i64> {
    let file_name = file
        .file_stem()
        .expect("Failed to get file name")
        .to_string_lossy()
        .to_string();

    let file_extension = file
        .extension()
        .expect("Failed to get file extension")
        .to_string_lossy()
        .to_lowercase()
        .to_string();

    let metadata = fs::metadata(file).await?;
    let size: i64 = metadata.len() as i64;
    let created_at = metadata.created()?;
    let modified_at = metadata.modified()?;
    // TODO: Replace with jiff as chrono is soft dropped, waiting on seaorm support
    let created_chrono: DateTimeWithTimeZone = DateTime::<Utc>::from(created_at).into();
    let modified_chrono: DateTimeWithTimeZone = DateTime::<Utc>::from(modified_at).into();

    let path_file_id = get_file_primary_id_from_name_extension(database, directory_id, &file_name, &file_extension)
        .await
        .with_context(|| {
            format!("failed looking up file from directory_id={directory_id} file_name={file_name} file_extension={file_extension}")
        })?;

    let mut file_id = get_file_primary_id_from_hash(database, directory_id, &hash)
        .await
        .with_context(|| {
            format!("failed looking up file from directory_id={directory_id} hash={hash}")
        })?;

    match file_id {
        Some(id) => {
            let file: Option<files::Model> = files::Entity::find_by_id(id)
                .one(database)
                .await
                .with_context(|| {
                    format!("failed loading existing file id={id} for metadata update")
                })?;

            match file {
                Some(file) => {
                    let mut changes: files::ActiveModel = file.into();

                    changes.name = Set(file_name.clone());
                    changes.extension = Set(file_extension.clone());
                    changes.hash = Set(hash);
                    changes.size_bytes = Set(size);
                    changes.created_at = Set(created_chrono);
                    changes.modified_at = Set(modified_chrono);

                    // If there is an existing file id at the path then delete it to avoid conflicts
                    if let Some(path_id) = path_file_id {
                        if path_id != id {
                            drop_file_from_id(database, path_id).await?;
                        }
                    }

                    let res: files::Model = changes.update(database).await.with_context(|| {
                        format!(
                            "failed updating file: \
                                id={id},
                                directory_id={directory_id}, \
                                name={file_name}"
                        )
                    })?;

                    file_id = Some(res.id);
                }
                None => {
                    println!(
                        "Unable to find file to update: \
                        id={id},
                        directory_id={directory_id}, \
                        name={file_name}"
                    )
                }
            };
        }
        None => {
            // If there is an existing file id at the path then delete it to avoid conflicts
            if let Some(path_id) = path_file_id {
                drop_file_from_id(database, path_id).await?;
            }

            let file: files::ActiveModel = files::ActiveModel {
                directory_id: Set(directory_id),
                name: Set(file_name.clone()),
                extension: Set(file_extension),
                hash: Set(hash),
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

#[hotpath::measure]
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

#[hotpath::measure]
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

#[hotpath::measure]
pub async fn cleanup_video_streams(
    database: &DatabaseConnection,
    file_id: i64,
    indexes: Vec<i64>,
) -> Result<()> {
    // Fetch all existing indexes for file id
    let mut cleanup: Vec<i64> = get_video_stream_indexes_from_file(database, file_id).await?;

    // Keep only indexes that are not in the new files indexes
    cleanup.retain(|x| !indexes.contains(x));
    if cleanup.len() == 0 {
        return Ok(());
    }

    drop_video_stream_from_file_stream(database, file_id, cleanup).await?;
    Ok(())
}

#[hotpath::measure]
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
                    "failed looking up video stream file_id={file_id}, stream_index={video_index}"
                )
            })?;

    match video_stream_id {
        Some(id) => {
            let stream: Option<video_streams::Model> = video_streams::Entity::find_by_id(id)
                .one(database)
                .await
                .with_context(|| {
                    format!("failed loading existing file id={id} for metadata update")
                })?;

            match stream {
                Some(stream) => {
                    let mut changes: video_streams::ActiveModel = stream.into();
                    let vid_bit: Option<i64> = video.bitdepth.map(|l| l as i64);

                    changes.codec = Set(video.codec);
                    changes.width = Set(video.width);
                    changes.height = Set(video.height);
                    changes.bit_depth = Set(vid_bit);
                    changes.bitrate = Set(video.bitrate);

                    let res: video_streams::Model =
                        changes.update(database).await.with_context(|| {
                            format!(
                                "failed updating video stream: \
                                id={id},
                                file_id={file_id},
                                stream_index={video_index}
                            "
                            )
                        })?;

                    video_stream_id = Some(res.id);
                }
                None => {
                    println!(
                        "Unable to find video stream to update: \
                            id={id},
                            file_id={file_id},
                            stream_index={video_index}
                        "
                    )
                }
            };
        }
        None => {
            let vid_bit: Option<i64> = video.bitdepth.map(|l| l as i64);
            let stream: video_streams::ActiveModel = video_streams::ActiveModel {
                file_id: Set(file_id),
                stream_index: Set(video_index),
                codec: Set(video.codec),
                width: Set(video.width),
                height: Set(video.height),
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

#[hotpath::measure]
pub async fn cleanup_audio_streams(
    database: &DatabaseConnection,
    file_id: i64,
    indexes: Vec<i64>,
) -> Result<()> {
    // Fetch all existing indexes for file id
    let mut cleanup: Vec<i64> = get_audio_stream_indexes_from_file(database, file_id).await?;

    // Keep only indexes that are not in the new files indexes
    cleanup.retain(|x| !indexes.contains(x));
    if cleanup.len() == 0 {
        return Ok(());
    }

    drop_audio_stream_from_file_stream(database, file_id, cleanup).await?;
    Ok(())
}

#[hotpath::measure]
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
        Some(id) => {
            let stream: Option<audio_streams::Model> = audio_streams::Entity::find_by_id(id)
                .one(database)
                .await
                .with_context(|| {
                    format!("failed loading existing file id={id} for metadata update")
                })?;

            match stream {
                Some(stream) => {
                    let mut changes: audio_streams::ActiveModel = stream.into();
                    changes.title = Set(audio.title);
                    changes.codec = Set(audio.codec);
                    changes.channels = Set(audio.channels.into());
                    changes.bitrate = Set(audio.bitrate);
                    changes.is_default = Set(audio.default);
                    changes.is_forced = Set(audio.forced);

                    let res: audio_streams::Model =
                        changes.update(database).await.with_context(|| {
                            format!(
                                "failed updating audio stream: \
                                id={id},
                                file_id={file_id},
                                stream_index={audio_index}
                            "
                            )
                        })?;

                    audio_stream_id = Some(res.id);
                }
                None => {
                    println!(
                        "Unable to find audio stream to update: \
                            id={id},
                            file_id={file_id},
                            stream_index={audio_index}
                        "
                    )
                }
            };
        }
        None => {
            let stream: audio_streams::ActiveModel = audio_streams::ActiveModel {
                file_id: Set(file_id),
                stream_index: Set(audio.index),
                title: Set(audio.title),
                codec: Set(audio.codec),
                language: Set(audio.language),
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

#[hotpath::measure]
pub async fn cleanup_subtitle_streams(
    database: &DatabaseConnection,
    file_id: i64,
    indexes: Vec<i64>,
) -> Result<()> {
    // Fetch all existing indexes for file id
    let mut cleanup: Vec<i64> = get_subtitle_stream_indexes_from_file(database, file_id).await?;

    // Keep only indexes that are not in the new files indexes
    cleanup.retain(|x| !indexes.contains(x));
    if cleanup.len() == 0 {
        return Ok(());
    }

    drop_subtitle_stream_from_file_stream(database, file_id, cleanup).await?;
    Ok(())
}

#[hotpath::measure]
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
        Some(id) => {
            let stream: Option<subtitle_streams::Model> = subtitle_streams::Entity::find_by_id(id)
                .one(database)
                .await
                .with_context(|| {
                    format!("failed loading existing file id={id} for metadata update")
                })?;

            match stream {
                Some(stream) => {
                    let mut changes: subtitle_streams::ActiveModel = stream.into();

                    changes.title = Set(subtitle.title);
                    changes.codec = Set(subtitle.codec);
                    changes.language = Set(subtitle.language);
                    changes.is_default = Set(subtitle.default);
                    changes.is_forced = Set(subtitle.forced);
                    changes.is_sdh = Set(subtitle.sdh);

                    let res: subtitle_streams::Model =
                        changes.update(database).await.with_context(|| {
                            format!(
                                "failed updating subtitle stream: \
                                id={id},
                                file_id={file_id},
                                stream_index={sub_index}
                            "
                            )
                        })?;

                    subtitle_stream_id = Some(res.id);
                }
                None => {
                    println!(
                        "Unable to find subtitle stream to update: \
                            id={id},
                            file_id={file_id},
                            stream_index={sub_index}
                        "
                    )
                }
            };
        }
        None => {
            let stream: subtitle_streams::ActiveModel = subtitle_streams::ActiveModel {
                file_id: Set(file_id),
                stream_index: Set(subtitle.index),
                title: Set(subtitle.title),
                codec: Set(subtitle.codec),
                language: Set(subtitle.language),
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
