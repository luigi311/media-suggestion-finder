use anyhow::{Context, Result};
use imohash::Hasher;
use std::path::PathBuf;
use tokio::time::{Duration, sleep};

use sea_orm::DatabaseConnection;

use crate::AppConfig;
use crate::database_orm::{get_file_primary_id_from_hash, update_missing_files};
use crate::database_process::{
    process_audio_stream, process_directory, process_file, process_media_file,
    process_subtitle_file, process_subtitle_stream, process_video_stream,
};
use crate::files::{find_media_subtitle_pairs, iterate_folder};
use crate::media::{Media, parse_media};

#[derive(Clone, Debug)]
struct ParsedMedia {
    // Hash, Media
    media: (String, Media),
    subtitles: Vec<(String, Media)>,
}

#[hotpath::measure]
async fn parse_file(
    orm_database: &DatabaseConnection,
    file: &PathBuf,
    hasher: Hasher,
) -> Result<(Option<i64>, Option<(String, Media)>)> {
    let parent = file
        .parent()
        .with_context(|| format!("Failed to get parent path"))?;
    let directory_id = process_directory(orm_database, parent)
        .await
        .with_context(|| format!("failed processing directory {}", parent.display()))?;

    let path = file
        .to_str()
        .with_context(|| format!("Failed to get path"))?;
    let hash = hotpath::measure_block!("hash_media_file", {
        let hash = hasher.sum_file(path);
        hash
    })?
    .to_string();

    // Check if hash exists in directory_id if not parse and process
    let parsed_media: (Option<i64>, Option<(String, Media)>) =
        match get_file_primary_id_from_hash(orm_database, directory_id, &hash).await? {
            Some(file_id) => (Some(file_id), None),
            None => {
                let parsed_file = parse_media(file).await?;
                (None, Some((hash, parsed_file)))
            }
        };

    Ok(parsed_media)
}

#[hotpath::measure]
async fn process_library(path: PathBuf, orm_database: &DatabaseConnection) -> Result<()> {
    let hasher = Hasher::new();
    let mut found_files: Vec<i64> = Vec::new();

    let all_files = iterate_folder(path.clone())
        .await
        .with_context(|| format!("failed scanning {}", path.display()))?;

    let files = find_media_subtitle_pairs(all_files).await;
    let mut parsed: Vec<ParsedMedia> = Vec::new();

    for file in files {
        let media_file = &file.media;
        let parsed_media: (String, Media);
        match media_file {
            None => {
                println!("No file for {:?}, skipping", &file);
                continue;
            }
            Some(media_file) => {
                let parsed_file = parse_file(orm_database, media_file, hasher)
                    .await
                    .with_context(|| format!("failed parsing {}", media_file.display()))?;
                match parsed_file {
                    (None, Some(parsed)) => parsed_media = parsed,
                    (Some(file_id), None) => {
                        println!("File {} is already processed", media_file.display());
                        found_files.push(file_id);
                        continue;
                    }
                    (None, None) => {
                        println!("Not sure how you got here");
                        continue;
                    }
                    (Some(_), Some(_)) => {
                        println!("Not sure how you got here");
                        continue;
                    }
                }
            }
        };

        let mut parsed_subtitles: Vec<(String, Media)> = Vec::new();
        let subtitle_files = &file.subtitles;
        for sub_file in subtitle_files {
            let parsed_file = parse_file(orm_database, sub_file, hasher).await?;
            match parsed_file {
                (None, Some(parsed)) => {
                    parsed_subtitles.push(parsed);
                }
                (Some(file_id), None) => {
                    found_files.push(file_id);
                    continue;
                }
                (None, None) => {
                    println!("Not sure how you got here");
                    continue;
                }
                (Some(_), Some(_)) => {
                    println!("Not sure how you got here");
                    continue;
                }
            }
        }

        parsed.push(ParsedMedia {
            media: parsed_media,
            subtitles: parsed_subtitles,
        })
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
        process_library(path.clone(), &orm_database)
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
