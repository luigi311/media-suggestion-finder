use anyhow::{Result, anyhow};
use mediainfo::{self, MediaInfo, Stream, StreamKind};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Language {
    English,
    Spanish,
    French,
    German,
    Chinese,
    Japanese,
    Korean,
    Russian,
    Italian,
    Portuguese,
}

impl Language {
    pub fn iso_639_2(self) -> &'static str {
        match self {
            Language::English => "eng",
            Language::Spanish => "spa",
            Language::French => "fra",
            Language::German => "deu",
            Language::Chinese => "zho",
            Language::Japanese => "jpn",
            Language::Korean => "kor",
            Language::Russian => "rus",
            Language::Italian => "ita",
            Language::Portuguese => "por",
        }
    }
}

#[derive(Clone, Debug)]
pub struct VideoStream {
    pub index: i32,
    pub codec: String,
    pub width: i32,
    pub height: i32,
    pub bitdepth: Option<i8>,
    pub hdr: Option<String>,
    pub bitrate: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct AudioStream {
    pub index: i32,
    pub title: Option<String>,
    pub codec: String,
    pub language: Option<Language>,
    pub channels: String,
    pub bitrate: Option<i64>,
    pub default: bool,
    pub forced: bool,
}

#[derive(Clone, Debug)]
pub struct SubtitleStream {
    pub index: i32,
    pub title: Option<String>,
    pub codec: String,
    pub language: Option<Language>,
    pub default: bool,
    pub forced: bool,
    pub sdh: bool,
}

#[derive(Clone, Debug)]
pub struct Media {
    pub path: PathBuf,
    pub videos: Vec<VideoStream>,
    pub audios: Vec<AudioStream>,
    pub subtitles: Vec<SubtitleStream>,
}

fn parse_language(language_str: &str) -> Result<Option<Language>> {
    let language: Option<Language> = match language_str.trim().to_lowercase().as_str() {
        "english" | "en" => Some(Language::English),
        "french" | "fr" | "fr-fr" => Some(Language::French),
        "german" | "de" => Some(Language::German),
        "japanese" | "ja" => Some(Language::Japanese),
        "italian" | "it" => Some(Language::Italian),
        _ => None,
    };

    Ok(language)
}

fn parse_bitdepth(bitdepth_str: &str) -> Option<i8> {
    let bitdepth_parse: Result<i8, _> = bitdepth_str.parse();
    match bitdepth_parse {
        Ok(i) => Some(i),
        _ => None,
    }
}

fn parse_bitrate(bitrate_str: &str) -> Option<i64> {
    let bitrate_parse: Result<i64, _> = bitrate_str.parse();
    match bitrate_parse {
        Ok(i) => Some(i),
        _ => None,
    }
}

fn str_to_bool(s: &str) -> Result<bool> {
    match s.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" | "" => Ok(false),
        _ => Err(anyhow!("Failed to parse bool {}", s)),
    }
}

fn print_fields(stream: &Stream) -> () {
    for (n, t, _, _) in stream.fields() {
        println!("{n}: {t}");
    }
}

pub async fn parse_media(path: &PathBuf) -> Result<Media> {
    let mut media = MediaInfo::new();

    let mut videos: Vec<VideoStream> = Vec::new();
    let mut audios: Vec<AudioStream> = Vec::new();
    let mut subtitles: Vec<SubtitleStream> = Vec::new();

    // AI: force catch media open panics instead of propegating upwards due to MediaInfo.open not returning a result and instead force panicing
    let opened = catch_unwind(AssertUnwindSafe(|| media.open(&path)));
    match opened {
        Ok(true) => {
            println!("Parsing: {}", path.display());
            let video_streams = media.streams(StreamKind::Video);
            let audio_streams = media.streams(StreamKind::Audio);
            let subtitle_streams = media.streams(StreamKind::Text);

            for stream in video_streams {
                let index: i32 = stream.get("StreamKindID").to_string().parse()?;
                let codec: String = stream.get("Format").to_string();
                let width: i32 = stream.get("Width").parse()?;
                let height: i32 = stream.get("Height").parse()?;
                let bitdepth = parse_bitdepth(stream.get("BitDepth"));
                let bitrate = parse_bitrate(stream.get("BitRate"));

                videos.push(VideoStream {
                    index,
                    codec,
                    width,
                    height,
                    bitdepth,
                    bitrate,
                    hdr: None,
                });
            }

            for stream in audio_streams {
                let index: i32 = stream.get("StreamKindID").to_string().parse()?;
                let title: Option<String> = stream.get("Title").to_string().into();
                let codec: String = stream.get("Format").to_string();
                let language = parse_language(stream.get("Language"))?;
                let channels: String = stream.get("Channel(s)/String").to_string();
                let bitrate = parse_bitrate(stream.get("BitRate"));
                let default = str_to_bool(stream.get("Default"))?;
                let forced = str_to_bool(stream.get("Forced"))?;

                audios.push(AudioStream {
                    index,
                    title,
                    codec,
                    language,
                    channels,
                    bitrate,
                    default,
                    forced,
                });
            }

            for stream in subtitle_streams {
                let index: i32 = stream.get("StreamKindID").to_string().parse()?;
                let title: Option<String> = stream.get("Title").to_string().into();
                let codec: String = stream.get("Format").to_string();
                let language = parse_language(stream.get("Language"))?;
                let forced = str_to_bool(stream.get("Forced"))?;
                // TODO: Parse sdh
                let sdh = false;
                let default = str_to_bool(stream.get("Default"))?;

                subtitles.push(SubtitleStream {
                    index,
                    title,
                    codec,
                    language,
                    forced,
                    sdh,
                    default,
                });
            }
        }
        // MediaInfo simply couldn't recognize/open the file.
        Ok(false) => {
            println!("MediaInfo couldnt open {}", path.display());
        }

        // MediaInfo itself panicked while parsing the file.
        Err(payload) => {
            let message = if let Some(s) = payload.downcast_ref::<&str>() {
                *s
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.as_str()
            } else {
                "unknown panic"
            };

            return Err(anyhow!(
                "MediaInfo panicked while parsing {}: {}",
                path.display(),
                message
            ));
        }
    }

    Ok(Media {
        path: path.clone(),
        videos,
        audios,
        subtitles,
    })
}
