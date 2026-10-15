use anyhow::{Context, Result};
use imohash::Hasher;
use std::path::PathBuf;
use tokio::task::JoinSet;
use tokio::time::{Duration, sleep};

use sea_orm::DatabaseConnection;

use crate::AppConfig;
use crate::database_orm::{get_file_primary_id_from_hash, update_missing_files};
use crate::database_process::{
    process_audio_stream, process_directory, process_file, process_media_file,
    process_subtitle_file, process_subtitle_stream, process_video_stream,
};
use crate::files::{MediaPairHashes, find_media_subtitle_pairs, hash_media_pairs, iterate_folder};
use crate::media::{Media, parse_media};

#[derive(Clone, Debug)]
struct ParsedMedia {
    // Hash, Media
    media: (String, Media),
    subtitles: Vec<(String, Media)>,
}

#[hotpath::measure]
async fn should_parse_file(
    orm_database: &DatabaseConnection,
    file: &PathBuf,
    hash: &String,
) -> Result<(bool, Option<i64>)> {
    let parent = file
        .parent()
        .with_context(|| format!("Failed to get parent path"))?;
    let directory_id = process_directory(orm_database, parent)
        .await
        .with_context(|| format!("failed processing directory {}", parent.display()))?;

    // Check if hash exists in directory_id if not parse and process
    let parsed_media: (bool, Option<i64>) =
        match get_file_primary_id_from_hash(orm_database, directory_id, hash).await? {
            Some(file_id) => (false, Some(file_id)),
            None => (true, None),
        };

    Ok(parsed_media)
}

async fn actually_parse_file(pair: MediaPairHashes) -> Result<ParsedMedia> {
    let parsed_media = parse_media(&pair.media.0)
        .await
        .with_context(|| format!("failed processing media {}", pair.media.0.display()))?;

    let mut parsed_subtitles: Vec<(String, Media)> = Vec::new();

    for sub in pair.subtitles {
        let parsed_subtitle = parse_media(&sub.0)
            .await
            .with_context(|| format!("failed processing subtitle {}", pair.media.0.display()))?;

        parsed_subtitles.push((sub.1, parsed_subtitle));
    }

    Ok(ParsedMedia {
        media: (pair.media.1, parsed_media),
        subtitles: parsed_subtitles,
    })
}

#[hotpath::measure]
async fn process_library(
    path: PathBuf,
    orm_database: &DatabaseConnection,
    max_concurrent_parses: usize,
    max_concurrent_hashes: usize,
) -> Result<()> {
    let hasher = Hasher::new();
    let mut found_files: Vec<i64> = Vec::new();

    let all_files = iterate_folder(path.clone())
        .await
        .with_context(|| format!("failed scanning {}", path.display()))?;

    let files = find_media_subtitle_pairs(all_files).await;
    let hashed_pairs = hash_media_pairs(files, hasher, max_concurrent_hashes).await?;
    // path, hash
    let mut to_parse: Vec<MediaPairHashes> = Vec::new();

    for pair in hashed_pairs {
        let media_to_parse: (PathBuf, String);
        let media_file = &pair.media;
        let should = should_parse_file(orm_database, &media_file.0, &media_file.1)
            .await
            .with_context(|| {
                format!("failed to check if should parse {}", media_file.0.display())
            })?;
        match should {
            (true, None) => media_to_parse = media_file.clone(),
            (false, Some(file_id)) => {
                println!("File {} is already processed", media_file.0.display());
                found_files.push(file_id);
                continue;
            }
            _ => {
                println!("Not sure how you got here");
                continue;
            }
        }

        let mut subtitles_to_parse: Vec<(PathBuf, String)> = Vec::new();
        for sub_file in &pair.subtitles {
            let should = should_parse_file(orm_database, &sub_file.0, &sub_file.1).await?;
            match should {
                (true, None) => {
                    subtitles_to_parse.push(sub_file.clone());
                }
                (false, Some(file_id)) => {
                    println!("File {} is already processed", sub_file.0.display());
                    found_files.push(file_id);
                    continue;
                }
                _ => {
                    println!("Not sure how you got here");
                    continue;
                }
            }
        }

        to_parse.push(MediaPairHashes {
            media: media_to_parse,
            subtitles: subtitles_to_parse,
        })
    }

    let mut parsed: Vec<ParsedMedia> = Vec::new();
    let mut pending = to_parse.into_iter();
    let mut jobs = JoinSet::new();
    loop {
        // Fill the available slots.
        while jobs.len() < max_concurrent_parses {
            let Some(parsing) = pending.next() else {
                break;
            };

            jobs.spawn(actually_parse_file(parsing));
        }

        // Collect one result, freeing a slot for another task.
        let Some(result) = jobs.join_next().await else {
            break;
        };

        match result {
            Ok(Ok(media)) => parsed.push(media),
            Ok(Err(error)) => eprintln!("Parsing failed: {error:#}"),
            Err(error) => eprintln!("Parsing task failed: {error}"),
        }
    }

    for parse in parsed {
        let media: Media = parse.media.1;
        let subtitles: Vec<(String, Media)> = parse.subtitles;
        let hash: String = parse.media.0;
        println!("Processing {}", media.path.display());

        let parent = media.path.parent().expect("Failed to get parent path");
        let directory_id = process_directory(&orm_database, parent)
            .await
            .with_context(|| format!("failed processing directory {}", parent.display()))?;
        let file_id = process_file(&orm_database, directory_id, &media.path, hash)
            .await
            .with_context(|| format!("failed processing file {}", media.path.display()))?;
        found_files.push(file_id);

        let media_id = process_media_file(&orm_database, file_id)
            .await
            .with_context(|| {
                format!(
                    "failed processing media file file_id={file_id} {}",
                    media.path.display()
                )
            })?;

        // Video Streams
        for video in media.videos {
            let video_index = video.index;

            process_video_stream(&orm_database, file_id, video)
                .await
                .with_context(|| {
                    format!(
                        "failed processing video stream {video_index} for {} (file_id={file_id})",
                        media.path.display()
                    )
                })?;
        }

        // Audio Streams
        for audio in media.audios {
            let audio_index = audio.index;

            process_audio_stream(&orm_database, file_id, audio)
                .await
                .with_context(|| {
                    format!(
                        "failed processing audio stream {audio_index} for {} (file_id={file_id})",
                        media.path.display()
                    )
                })?;
        }

        // Subtitle streams
        for sub in media.subtitles {
            let sub_index = sub.index;

            process_subtitle_stream(&orm_database, file_id, sub)
                .await
                .with_context(|| {
                    format!(
                        "failed processing subtitle stream {sub_index} for {} (file_id={file_id})",
                        media.path.display()
                    )
                })?;
        }

        for (sub_hash, subtitle) in subtitles {
            println!("Processing {}", subtitle.path.display());
            let hash: String = sub_hash.to_string();
            let sub_file_id = process_file(&orm_database, directory_id, &subtitle.path, hash)
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

    update_missing_files(&orm_database, found_files).await?;

    Ok(())
}

#[hotpath::measure]
pub async fn indexing(config: AppConfig, orm_database: DatabaseConnection) -> Result<()> {
    let path = PathBuf::from(&config.media_path);

    loop {
        // Box::pin required due to async future size growing like crazy especially when utilizing hotpath
        Box::pin(process_library(
            path.clone(),
            &orm_database,
            config.file.concurrent_parse,
            config.file.concurrent_hash,
        ))
        .await
        .with_context(|| format!("failed processing library {}", path.display()))?;

        sleep(Duration::from_mins(60)).await;
    }

    orm_database
        .close()
        .await
        .context("failed to close database connection")?;

    Ok(())
}
