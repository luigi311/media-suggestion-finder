use anyhow::{Context, Result};
use imohash::Hasher;
use sea_orm::DatabaseConnection;
use std::path::PathBuf;

use config::{Config, ConfigError, File};

mod database_orm;
mod database_process;
mod files;
mod media;

use database_orm::{
    database_connect, get_directories_with_media_files, get_metadata_from_directory,
};
use files::{find_media_subtitle_pairs, iterate_folder};
use media::{Media, parse_media};

use database_process::{
    process_audio_stream, process_directory, process_file, process_media_file,
    process_subtitle_file, process_subtitle_stream, process_video_stream,
};

struct AppConfig {
    pub dsn: String,
    pub media_path: PathBuf,
}

impl AppConfig {
    pub fn new() -> Result<Self, ConfigError> {
        let settings = Config::builder()
            .add_source(File::with_name("config.toml"))
            .build()?;

        let media_path: String = settings.get_string("media_path")?;
        let dsn: String = settings.get_string("dsn")?;

        Ok(AppConfig {
            dsn,
            media_path: media_path.into(),
        })
    }
}

#[derive(Clone, Debug)]
struct ParsedMedia {
    media: Media,
    subtitles: Vec<Media>,
}

async fn process_library(path: PathBuf, orm_database: &DatabaseConnection) -> Result<()> {
    let hasher = Hasher::new();
    let all_files = iterate_folder(path.clone())
        .await
        .with_context(|| format!("failed scanning {}", path.display()))?;

    let files = find_media_subtitle_pairs(all_files).await;
    let mut parsed: Vec<ParsedMedia> = Vec::new();

    for file in files {
        let mut parsed_subtitles: Vec<Media> = Vec::new();
        let media_file = &file.media;
        let parsed_media: Media = match media_file {
            None => {
                println!("No file for {:?}, skipping", &file);
                continue;
            }
            Some(media_file) => {
                let file_name = media_file
                    .file_name()
                    .expect("Failed to get filename")
                    .to_str()
                    .expect("Failed to convert to string")
                    .to_string();

                let Ok(parse_media) = parse_media(media_file).await else {
                    println!("Failed to parse {file_name}");
                    continue;
                };
                parse_media
            }
        };

        let subtitle_files = &file.subtitles;
        for sub_file in subtitle_files {
            let file_name = sub_file
                .file_name()
                .expect("Failed to get filename")
                .to_str()
                .expect("Failed to convert to string")
                .to_string();

            let Ok(parse_subtitle) = parse_media(sub_file).await else {
                println!("Failed to parse {file_name}");
                continue;
            };
            parsed_subtitles.push(parse_subtitle);
        }

        parsed.push(ParsedMedia {
            media: parsed_media,
            subtitles: parsed_subtitles,
        })
    }

    for media in parsed {
        println!("Processing {}", media.media.path.display());
        let parent = media
            .media
            .path
            .parent()
            .expect("Failed to get parent path");
        let directory_id = process_directory(&orm_database, parent)
            .await
            .with_context(|| format!("failed processing directory {}", parent.display()))?;
        let file_id = process_file(&orm_database, directory_id, &media.media.path, hasher)
            .await
            .with_context(|| format!("failed processing file {}", media.media.path.display()))?;
        let media_id = process_media_file(&orm_database, file_id)
            .await
            .with_context(|| {
                format!(
                    "failed processing media file file_id={file_id} {}",
                    media.media.path.display()
                )
            })?;

        // Video Streams
        for video in media.media.videos {
            let video_index = video.index;

            process_video_stream(&orm_database, file_id, video)
                .await
                .with_context(|| {
                    format!(
                        "failed processing video stream {video_index} for {} (file_id={file_id})",
                        media.media.path.display()
                    )
                })?;
        }

        // Audio Streams
        for audio in media.media.audios {
            let audio_index = audio.index;

            process_audio_stream(&orm_database, file_id, audio)
                .await
                .with_context(|| {
                    format!(
                        "failed processing audio stream {audio_index} for {} (file_id={file_id})",
                        media.media.path.display()
                    )
                })?;
        }

        // Subtitle streams
        for sub in media.media.subtitles {
            let sub_index = sub.index;

            process_subtitle_stream(&orm_database, file_id, sub)
                .await
                .with_context(|| {
                    format!(
                        "failed processing subtitle stream {sub_index} for {} (file_id={file_id})",
                        media.media.path.display()
                    )
                })?;
        }

        for subtitle in media.subtitles {
            println!("Processing {}", subtitle.path.display());
            let sub_file_id = process_file(&orm_database, directory_id, &subtitle.path, hasher)
                .await
                .with_context(|| {
                    format!(
                        "failed processing subtitle file {}",
                        subtitle.path.display()
                    )
                })?;
            process_subtitle_file(&orm_database, sub_file_id, media_id)
                .await
                .with_context(|| {
                    format!(
                        "failed associating subtitle {} with media_id={media_id}",
                        subtitle.path.display()
                    )
                })?;

            for sub in subtitle.subtitles {
                let sub_index = sub.index;

                process_subtitle_stream(&orm_database, sub_file_id, sub)
                    .await
                    .with_context(|| {
                        format!(
                            "failed processing subtitle stream {sub_index} for {} (file_id={sub_file_id})",
                            subtitle.path.display()
                        )
                    })?;
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::new().context("failed to load application configuration")?;

    let orm_database = database_connect(&config.dsn)
        .await
        .context("failed to connect to database")?;

    let path = PathBuf::from(&config.media_path);
    process_library(path.clone(), &orm_database)
        .await
        .with_context(|| format!("failed processing library {}", path.display()))?;

    let directories_ids = get_directories_with_media_files(&orm_database)
        .await
        .with_context(|| format!("failed fetching directories with media files"))?;

    for dir_id in directories_ids {
        let metadata = get_metadata_from_directory(&orm_database, dir_id)
            .await
            .with_context(|| format!("failed fetching media metadata for directory id {dir_id}"))?;
    }
    orm_database
        .close()
        .await
        .context("failed to close database connection")?;

    Ok(())
}
