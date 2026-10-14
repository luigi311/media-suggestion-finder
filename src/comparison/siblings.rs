use anyhow::Result;
use itertools::Itertools;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    hash::Hash,
};

use crate::database_orm::MediaMetadata;

// For simple single value comparisons
#[derive(Clone, Debug, Serialize)]
pub struct Dominant<T> {
    pub value: T,
    pub count: usize,
    pub total: usize,
}

// For set comparisons, ex. languages for audio/subtitles
#[derive(Clone, Debug, Serialize)]
pub struct SetDominant<T> {
    pub value: BTreeSet<T>,
    pub count: BTreeMap<T, usize>,
    pub total: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SiblingAnalysis {
    pub resolution: Option<Dominant<(i64, i64)>>,
    pub codec: Option<Dominant<String>>,
    pub audio_languages: Option<SetDominant<String>>,
    pub subtitle_languages: Option<SetDominant<String>>,
}

fn counter<T>(values: Vec<T>) -> Option<Dominant<T>>
where
    T: Eq + Hash,
{
    let total = values.len();
    let counts = values.into_iter().counts();

    let (value, count) = counts.into_iter().max_by_key(|(_, count)| *count)?;

    Some(Dominant {
        value,
        count,
        total,
    })
}

fn counter_set<T>(values: Vec<Vec<T>>, threshold: f64) -> Option<SetDominant<T>>
where
    T: Ord + Clone,
{
    let total = values.len();
    let mut count: BTreeMap<T, usize> = BTreeMap::new();

    for values_per_media in values {
        let unique: BTreeSet<T> = values_per_media.into_iter().collect();

        for value in unique {
            *count.entry(value).or_default() += 1;
        }
    }

    // Only allow values that are above threshold, exclude low counts of languages even if it is part of the set
    let value = count
        .iter()
        .filter(|(_, value_count)| (**value_count as f64 / total as f64) >= threshold)
        .map(|(value, _)| value.clone())
        .collect();

    Some(SetDominant {
        value,
        count,
        total,
    })
}

#[hotpath::measure]
pub async fn analyse_siblings(siblings: Vec<MediaMetadata>) -> Result<SiblingAnalysis> {
    let mut resolutions: Vec<(i64, i64)> = Vec::new();
    let mut video_codecs: Vec<String> = Vec::new();
    let mut audio_languages: Vec<Vec<String>> = Vec::new();
    let mut subtitle_languages: Vec<Vec<String>> = Vec::new();

    for media in siblings {
        let mut media_audio_languages: Vec<String> = Vec::new();
        let mut media_subtitle_languages: Vec<String> = Vec::new();

        for video in media.videos {
            let res: (i64, i64) = (video.width, video.height);
            resolutions.push(res);

            video_codecs.push(video.codec);
        }

        for audio in media.audios {
            if let Some(language) = audio.language {
                media_audio_languages.push(language);
            }
        }

        for subtitle in media.subtitles {
            if let Some(language) = subtitle.language {
                media_subtitle_languages.push(language);
            }
        }

        audio_languages.push(media_audio_languages);
        subtitle_languages.push(media_subtitle_languages);
    }

    let res_dom: Option<Dominant<(i64, i64)>> = counter(resolutions);
    let codec_dom: Option<Dominant<String>> = counter(video_codecs);

    let audio_language_dom = counter_set(audio_languages, 0.6);
    let subtitle_language_dom = counter_set(subtitle_languages, 0.6);

    Ok(SiblingAnalysis {
        resolution: res_dom,
        codec: codec_dom,
        audio_languages: audio_language_dom,
        subtitle_languages: subtitle_language_dom,
    })
}
