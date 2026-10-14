use anyhow::Result;
use isolang::Language;
use serde::Serialize;

use crate::comparison::siblings::{Dominant, analyse_siblings};
use crate::database_orm::MediaMetadata;

#[derive(Clone, Debug, Serialize)]
pub struct MediaOutlier {
    pub file_id: i64,
    pub file_path: String,
    pub file_name: String,
    pub issues: Vec<OutlierReason>,
}

#[derive(Clone, Debug, Serialize)]
pub enum OutlierReason {
    Resolution {
        dominant: Dominant<(i64, i64)>,
        actual: (i64, i64),
    },
    VideoCodec {
        dominant: Dominant<String>,
        actual: String,
    },
    MissingAudioLanguage {
        language: String,
        count: usize,
        total: usize,
    },
    MissingSubtitleLanguage {
        language: String,
        count: usize,
        total: usize,
    },
}

#[hotpath::measure]
pub async fn generate_outliers(medias: Vec<MediaMetadata>) -> Result<Vec<MediaOutlier>> {
    let analysis = analyse_siblings(medias.clone()).await?;
    let res_check: bool = match &analysis.resolution {
        Some(x) => x.count != x.total,
        None => false,
    };

    let codec_check: bool = match &analysis.codec {
        Some(x) => x.count != x.total,
        None => false,
    };

    let mut outliers: Vec<MediaOutlier> = Vec::new();
    for media in medias {
        let mut reasons = Vec::new();

        for video in &media.videos {
            if res_check {
                if let Some(dominant) = &analysis.resolution {
                    if video.width != dominant.value.0 || video.height != dominant.value.1 {
                        reasons.push(OutlierReason::Resolution {
                            dominant: dominant.clone(),
                            actual: (video.width, video.height),
                        })
                    }
                }
            }

            if codec_check {
                if let Some(dominant) = &analysis.codec {
                    if video.codec != dominant.value {
                        reasons.push(OutlierReason::VideoCodec {
                            dominant: dominant.clone(),
                            actual: video.codec.clone(),
                        })
                    }
                }
            }
        }

        if let Some(dominant) = &analysis.audio_languages {
            for language in &dominant.value {
                let media_languages: Vec<String> = media
                    .audios
                    .iter()
                    .filter_map(|x| x.language.clone())
                    .map(|x| x)
                    .collect();

                let exists = media_languages.contains(language);

                if !exists {
                    let lang = match Language::from_639_3(language) {
                        Some(x) => x.to_string(),
                        None => language.clone(),
                    };

                    reasons.push(OutlierReason::MissingAudioLanguage {
                        language: lang,
                        count: dominant.count[language],
                        total: dominant.total,
                    });
                }
            }
        }

        if let Some(dominant) = &analysis.subtitle_languages {
            for language in &dominant.value {
                let media_languages: Vec<String> = media
                    .subtitles
                    .iter()
                    .filter_map(|x| x.language.clone())
                    .map(|x| x)
                    .collect();

                let exists = media_languages.contains(language);

                if !exists {
                    let lang = match Language::from_639_3(language) {
                        Some(x) => x.to_string(),
                        None => language.clone(),
                    };

                    reasons.push(OutlierReason::MissingSubtitleLanguage {
                        language: lang,
                        count: dominant.count[language],
                        total: dominant.total,
                    });
                }
            }
        }

        if !reasons.is_empty() {
            outliers.push(MediaOutlier {
                file_id: media.file_id,
                file_path: media.directory,
                file_name: format!("{}.{}", media.name, media.extension),
                issues: reasons,
            })
        };
    }

    Ok(outliers)
}
