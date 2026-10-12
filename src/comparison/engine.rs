use anyhow::Result;
use serde::Serialize;

use crate::comparison::siblings::{Dominant, analyse_siblings};
use crate::database_orm::MediaMetadata;

#[derive(Clone, Debug, Serialize)]
pub struct MediaOutlier {
    pub file_id: i64,
    pub issues: Vec<OutlierReason>,
}

#[derive(Clone, Debug, Serialize)]
pub enum OutlierReason {
    Resolution {
        dominant: Dominant<(i64, i64)>,
        actual: (i64, i64),
    },
    Codec {
        dominant: Dominant<String>,
        actual: String
    }
}

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

        if res_check {
            for video in &media.videos {
                if let Some(dominant) = &analysis.resolution {
                    if video.width != dominant.value.0 || video.height != dominant.value.1 {
                        reasons.push(OutlierReason::Resolution {
                            dominant: dominant.clone(),
                            actual: (video.width, video.height),
                        })
                    }
                }
            }
        }

        if codec_check {
            for video in &media.videos {
                if let Some(dominant) = &analysis.codec {
                    if video.codec != dominant.value {
                        reasons.push(OutlierReason::Codec {
                            dominant: dominant.clone(),
                            actual: video.codec.clone(),
                        })
                    }
                }
            }
        }

        if !reasons.is_empty() {
            outliers.push(MediaOutlier {
                file_id: media.file_id,
                issues: reasons,
            })
        };
    }

    Ok(outliers)
}
