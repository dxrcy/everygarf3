use std::fmt;
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use chrono::NaiveDate;
use clap::ValueEnum;
use reqwest::Url;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Source {
    #[default]
    Gocomics,
}

/// Image format (and file extension) to save images as.
#[derive(Default, Clone, Copy, PartialEq, ValueEnum)]
pub enum ImageFormat {
    #[default]
    Gif,
    Png,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_possible_value().unwrap().get_name())
    }
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_possible_value().unwrap().get_name())
    }
}

#[derive(Debug)]
pub struct DateUrl {
    pub date: NaiveDate,
    pub image_url: Option<Url>,
}

// TODO(refactor) Rename!!
pub enum UrlPath {
    Remote(Url),
    Local(PathBuf),
}

impl UrlPath {
    pub fn from(path: PathBuf) -> Result<Self> {
        let string = path.to_str().with_context(|| "converting path to string")?;
        if is_remote_url(string) {
            let url = reqwest::Url::parse(string).with_context(|| "parsing remote url")?;
            Ok(Self::Remote(url))
        } else {
            Ok(Self::Local(path))
        }
    }
}

fn is_remote_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}
