use anyhow::{Context, Result};
use async_recursion::async_recursion;
use imohash::Hasher;
use std::path::PathBuf;
use tokio::{
    fs::read_dir,
    task::{JoinSet, spawn_blocking},
};

const VIDEO_EXTENSIONS: [&str; 5] = ["mp4", "mkv", "avi", "mov", "wmv"];
const AUDIO_EXTENSIONS: [&str; 5] = ["mp3", "flac", "wav", "aac", "ogg"];
const SUBTITLE_EXTENSIONS: [&str; 5] = ["srt", "ass", "ssa", "vtt", "sub"];

#[derive(Clone, Debug)]
pub struct DirectoryScan {
    pub media_files: Vec<PathBuf>,
    pub subtitle_files: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct MediaPair {
    pub media: Option<PathBuf>,
    pub subtitles: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct MediaPairHashes {
    // Path, Hash
    pub media: (PathBuf, String),
    pub subtitles: Vec<(PathBuf, String)>,
}

#[hotpath::measure]
#[async_recursion]
pub async fn iterate_folder(path: &PathBuf) -> Result<Vec<DirectoryScan>> {
    let mut results: Vec<DirectoryScan> = Vec::new();
    let mut media_files: Vec<PathBuf> = Vec::new();
    let mut subtitle_files: Vec<PathBuf> = Vec::new();
    let mut subdir_results: Vec<DirectoryScan> = Vec::new();

    let mut entries = read_dir(&path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let entry_path = entry.path();

        if entry_path.is_dir() {
            subdir_results.extend(iterate_folder(&entry_path).await?);
            continue;
        }

        let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let ext = ext.to_lowercase();

        if VIDEO_EXTENSIONS.contains(&ext.as_str()) || AUDIO_EXTENSIONS.contains(&ext.as_str()) {
            media_files.push(entry_path);
        } else if SUBTITLE_EXTENSIONS.contains(&ext.as_str()) {
            subtitle_files.push(entry_path);
        }
    }

    if !media_files.is_empty() || !subtitle_files.is_empty() {
        results.push(DirectoryScan {
            media_files,
            subtitle_files,
        });
    }
    results.extend(subdir_results);

    Ok(results)
}

#[hotpath::measure]
pub async fn find_media_subtitle_pairs(files: Vec<DirectoryScan>) -> Vec<MediaPair> {
    let mut pairs: Vec<MediaPair> = Vec::new();

    for directory in files {
        if directory.media_files.is_empty() {
            pairs.push(MediaPair {
                media: None,
                subtitles: directory.subtitle_files,
            });
            continue;
        }

        if directory.subtitle_files.is_empty() {
            for media in directory.media_files {
                pairs.push(MediaPair {
                    media: Some(media),
                    subtitles: Vec::new(),
                });
            }
            continue;
        }

        // AI media <-> subtitling magic
        {
            let mut media_with_stems: Vec<(String, PathBuf)> = directory
                .media_files
                .into_iter()
                .filter_map(|p| {
                    p.file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| (s.to_string(), p.clone()))
                })
                .collect();

            // Longest stem first so more-specific media wins when one stem
            // is a prefix of another.
            media_with_stems.sort_by_key(|(stem, _)| std::cmp::Reverse(stem.len()));

            // Track which media each entry belongs to.
            let mut buckets: Vec<(PathBuf, Vec<PathBuf>)> = media_with_stems
                .iter()
                .map(|(_, path)| (path.clone(), Vec::new()))
                .collect();

            let mut unmatched_subs: Vec<PathBuf> = Vec::new();

            'subs: for sub_path in directory.subtitle_files {
                let Some(sub_stem) = sub_path.file_stem().and_then(|s| s.to_str()) else {
                    unmatched_subs.push(sub_path);
                    continue;
                };

                for (i, (media_stem, _)) in media_with_stems.iter().enumerate() {
                    if sub_stem == media_stem
                        || sub_stem
                            .strip_prefix(media_stem.as_str())
                            .map_or(false, |rest| rest.starts_with('.'))
                    {
                        buckets[i].1.push(sub_path);
                        continue 'subs;
                    }
                }
                unmatched_subs.push(sub_path);
            }

            for (media, subtitles) in buckets {
                pairs.push(MediaPair {
                    media: Some(media),
                    subtitles,
                });
            }
            pairs.push(MediaPair {
                media: None,
                subtitles: unmatched_subs,
            });
        }
    }

    pairs
}

#[hotpath::measure]
pub async fn hash_media_pairs(
    pairs: Vec<MediaPair>,
    hasher: Hasher,
    max_concurrent_hashes: usize,
) -> Result<Vec<MediaPairHashes>> {
    let mut hashed_pairs: Vec<MediaPairHashes> = Vec::new();
    let mut pending = pairs.into_iter();
    let mut jobs = JoinSet::new();

    loop {
        // Fill the available slots.
        while jobs.len() < max_concurrent_hashes {
            let Some(pair) = pending.next() else {
                break;
            };

            jobs.spawn(hash_media_pair(pair, hasher));
        }

        // Collect one result, freeing a slot for another task.
        let Some(result) = jobs.join_next().await else {
            break;
        };

        if let Some(pair) = result.context("hashing task failed")?? {
            hashed_pairs.push(pair);
        }
    }

    Ok(hashed_pairs)
}

#[hotpath::measure]
async fn hash_media_pair(pair: MediaPair, hasher: Hasher) -> Result<Option<MediaPairHashes>> {
    let Some(media) = pair.media else {
        return Ok(None);
    };
    let path = media
        .to_str()
        .with_context(|| format!("Failed to get path"))?;

    let owned_path = path.to_owned();
    let Ok(hash) = spawn_blocking(move || {
        hotpath::measure_block!("hash_media_file", hasher.sum_file(&owned_path))
    })
    .await?
    else {
        return Ok(None);
    };

    let media_hashed = (media, hash.to_string());
    let mut subtitles_hashed: Vec<(PathBuf, String)> = Vec::new();
    for subtitle in pair.subtitles {
        let path = subtitle
            .to_str()
            .with_context(|| format!("Failed to get path"))?;

        let owned_path = path.to_owned();
        let Ok(hash) = spawn_blocking(move || {
            hotpath::measure_block!("hash_subtitle_file", hasher.sum_file(&owned_path))
        })
        .await?
        else {
            continue;
        };

        subtitles_hashed.push((subtitle, hash.to_string()));
    }

    Ok(Some(MediaPairHashes {
        media: media_hashed,
        subtitles: subtitles_hashed,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // With no scanned directories, there should be no output pairs.
    #[tokio::test]
    async fn empty_input_returns_no_pairs() {
        let pairs = find_media_subtitle_pairs(Vec::new()).await;

        assert!(pairs.is_empty());
    }

    // Keep every video or audio file in the results, even when it has no subtitles.
    #[tokio::test]
    async fn media_without_subtitles_get_separate_pairs() {
        let directory = DirectoryScan {
            media_files: vec!["movie.mkv".into(), "song.flac".into()],
            subtitle_files: vec![],
        };

        let pairs = find_media_subtitle_pairs(vec![directory]).await;

        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].media, Some("movie.mkv".into()));
        assert!(pairs[0].subtitles.is_empty());
        assert_eq!(pairs[1].media, Some("song.flac".into()));
        assert!(pairs[1].subtitles.is_empty());
    }

    // Group a directory's subtitles under media: None when no media file exists.
    #[tokio::test]
    async fn subtitles_without_media_stay_together() {
        let directory = DirectoryScan {
            media_files: vec![],
            subtitle_files: vec!["movie.srt".into(), "movie.en.ass".into()],
        };

        let pairs = find_media_subtitle_pairs(vec![directory]).await;

        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].media, None);
        assert_eq!(
            pairs[0].subtitles,
            vec![PathBuf::from("movie.srt"), PathBuf::from("movie.en.ass")]
        );
    }

    // Attach movie.part2.en.srt to movie.part2, even though movie also matches its prefix.
    #[tokio::test]
    async fn prefers_the_longest_matching_media_name() {
        let directory = DirectoryScan {
            media_files: vec!["movie.mkv".into(), "movie.part2.mkv".into()],
            subtitle_files: vec!["movie.en.srt".into(), "movie.part2.en.srt".into()],
        };

        let pairs = find_media_subtitle_pairs(vec![directory]).await;

        assert_eq!(pairs[0].media, Some("movie.part2.mkv".into()));
        assert_eq!(
            pairs[0].subtitles,
            vec![PathBuf::from("movie.part2.en.srt")]
        );
        assert_eq!(pairs[1].media, Some("movie.mkv".into()));
        assert_eq!(pairs[1].subtitles, vec![PathBuf::from("movie.en.srt")]);
    }

    // Only a dot can introduce a matching suffix; movie2, movie-extra, and other stay unmatched.
    #[tokio::test]
    async fn unmatched_subtitles_stay_separate_and_suffixes_require_a_dot() {
        let directory = DirectoryScan {
            media_files: vec!["movie.mkv".into()],
            subtitle_files: vec![
                "movie.en.srt".into(),
                "movie2.srt".into(),
                "movie-extra.srt".into(),
                "other.srt".into(),
            ],
        };

        let pairs = find_media_subtitle_pairs(vec![directory]).await;

        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].media, Some("movie.mkv".into()));
        assert_eq!(pairs[0].subtitles, vec![PathBuf::from("movie.en.srt")]);
        assert_eq!(pairs[1].media, None);
        assert_eq!(
            pairs[1].subtitles,
            vec![
                PathBuf::from("movie2.srt"),
                PathBuf::from("movie-extra.srt"),
                PathBuf::from("other.srt"),
            ]
        );
    }

    // Matching names in different directories must stay separate.
    #[tokio::test]
    async fn files_in_different_directories_are_not_paired() {
        let directories = vec![
            DirectoryScan {
                media_files: vec!["first/movie.mkv".into()],
                subtitle_files: vec![],
            },
            DirectoryScan {
                media_files: vec![],
                subtitle_files: vec!["second/movie.srt".into()],
            },
        ];

        let pairs = find_media_subtitle_pairs(directories).await;

        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].media, Some("first/movie.mkv".into()));
        assert!(pairs[0].subtitles.is_empty());
        assert_eq!(pairs[1].media, None);
        assert_eq!(pairs[1].subtitles, vec![PathBuf::from("second/movie.srt")]);
    }
}
