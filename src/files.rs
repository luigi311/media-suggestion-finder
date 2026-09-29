use anyhow::Result;
use async_recursion::async_recursion;
use std::path::PathBuf;
use tokio::fs::read_dir;

const VIDEO_EXTENSIONS: [&str; 5] = ["mp4", "mkv", "avi", "mov", "wmv"];
const AUDIO_EXTENSIONS: [&str; 5] = ["mp3", "flac", "wav", "aac", "ogg"];
const SUBTITLE_EXTENSIONS: [&str; 5] = ["srt", "ass", "ssa", "vtt", "sub"];

#[derive(Clone, Debug)]
pub struct DirectoryScan {
    pub path: PathBuf,
    pub media_files: Vec<PathBuf>,
    pub subtitle_files: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct MediaPair {
    pub media: Option<PathBuf>,
    pub subtitles: Vec<PathBuf>,
}

#[async_recursion]
pub async fn iterate_folder(path: PathBuf) -> Result<Vec<DirectoryScan>> {
    let mut results: Vec<DirectoryScan> = Vec::new();
    let mut media_files: Vec<PathBuf> = Vec::new();
    let mut subtitle_files: Vec<PathBuf> = Vec::new();
    let mut subdir_results: Vec<DirectoryScan> = Vec::new();

    let mut entries = read_dir(&path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let entry_path = entry.path();

        if entry_path.is_dir() {
            subdir_results.extend(iterate_folder(entry_path).await?);
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
            path,
            media_files,
            subtitle_files,
        });
    }
    results.extend(subdir_results);

    Ok(results)
}

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
