use anyhow::Result;
use itertools::Itertools;
use serde::Serialize;
use std::hash::Hash;

use crate::database_orm::MediaMetadata;

#[derive(Clone, Debug, Serialize)]
pub struct Dominant<T> {
    pub value: T,
    pub count: usize,
    pub total: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SiblingAnalysis {
    pub resolution: Option<Dominant<(i64, i64)>>,
    pub codec: Option<Dominant<String>>,
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

pub async fn analyse_siblings(siblings: Vec<MediaMetadata>) -> Result<SiblingAnalysis> {
    let mut resolutions: Vec<(i64, i64)> = Vec::new();
    let mut codecs: Vec<String> = Vec::new();

    for media in siblings {
        for video in media.videos {
            let res: (i64, i64) = (video.width, video.height);
            resolutions.push(res);

            codecs.push(video.codec);
        }
    }

    let res_dom: Option<Dominant<(i64, i64)>> = counter(resolutions);
    let codec_dom: Option<Dominant<String>> = counter(codecs);

    Ok(SiblingAnalysis {
        resolution: res_dom,
        codec: codec_dom
    })
}
