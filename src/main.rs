mod args;

use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::{fs, io};

use anyhow::{Context, Result, bail};
use chrono::{Duration, NaiveDate, NaiveTime, Utc};
use clap::Parser;
use futures::StreamExt;
use tokio::runtime::Runtime;
use tracing::info;

use crate::args::ImageFormat;

fn main() -> Result<()> {
    let format = tracing_subscriber::fmt::format().without_time();
    tracing_subscriber::fmt().event_format(format).init();

    let args = args::Args::parse();

    if args.file_tree {
        unimplemented!("--tree");
    }
    if args.max_images.is_some() {
        unimplemented!("--max");
    }
    if args.max_attempts.is_some() {
        unimplemented!("--attempts");
    }
    if args.request_timeout.is_some() {
        unimplemented!("--timeout");
    }
    if args.request_timeout_initial.is_some() {
        unimplemented!("--initial-timeout");
    }
    if args.notify_on_fail {
        unimplemented!("--notify-on-fail");
    }
    if args.proxy != Path::new(everygarf::PROXY_DEFAULT) {
        unimplemented!("--roxy");
    }
    if args.no_proxy {
        unimplemented!("--no-proxy");
    }
    if args.cache != Path::new(everygarf::CACHE_DEFAULT) {
        unimplemented!("--cache");
    }
    if args.no_cache {
        unimplemented!("--no-cache");
    }
    if args.save_cache.is_some() {
        unimplemented!("--save-cache");
    }
    if !matches!(args.format, ImageFormat::Gif) {
        unimplemented!("--format");
    }
    if args.query {
        unimplemented!("--query");
    }

    let directory = match args.directory {
        Some(directory) => directory,
        None => get_target_directory()
            .with_context(|| "failed to find appropriate target directory path")?,
    };

    create_target_directory(&directory, args.remove_all)
        .with_context(|| "failed to create/clear target directory")?;

    let date_start = args.start_date.unwrap_or(FIRST_DATE);
    let date_end = latest();

    if date_start < FIRST_DATE {
        bail!(
            "Start date ({}) must not be before date of first comic ({})",
            date_start,
            FIRST_DATE,
        );
    }
    if date_start > date_end {
        bail!(
            "Start date ({}) must not be after date of latest comic ({})",
            date_start,
            date_end,
        );
    }

    let existing_dates = get_existing_dates(&directory)?;

    // Must be collected to get length initially
    let missing_dates: Vec<_> = date_iter(date_start..=date_end)
        .filter(|date| !existing_dates.contains(date))
        .collect();

    info!("{}", missing_dates.len());

    let job_count = args.job_count;

    let rt = Runtime::new().unwrap();
    rt.block_on(async move {
        let futures = missing_dates
            .into_iter()
            .map(|date| async move { run_download(date).await });

        let results = futures::stream::iter(futures)
            .buffer_unordered(job_count.into())
            .collect::<Vec<_>>()
            .await;

        for result in results {
            result.unwrap()
        }
    });

    Ok(())
}

pub async fn run_download(date: NaiveDate) -> Result<()> {
    info!(date = %date, "request");
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    Ok(())
}

pub fn get_existing_dates(directory: impl AsRef<Path>) -> Result<Vec<NaiveDate>> {
    let mut dates = Vec::new();
    for child in fs::read_dir(directory)? {
        let child = child?;
        if let Some(date) = get_filename_date(child.path()) {
            dates.push(date);
        };
    }
    Ok(dates)
}

fn get_filename_date(path: impl AsRef<Path>) -> Option<NaiveDate> {
    let stem = path.as_ref().file_stem()?.to_str()?;
    NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok()
}

pub fn date_iter(range: RangeInclusive<NaiveDate>) -> impl Iterator<Item = NaiveDate> {
    let (start, end) = (*range.start(), *range.end());
    (0..=(end - start).num_days()).map(move |days| start + Duration::days(days))
}

pub const FIRST_DATE: NaiveDate =
    NaiveDate::from_ymd_opt(1978, 6, 19).expect("Failed to parse const date");

pub fn latest() -> NaiveDate {
    let now = Utc::now();

    // Get naive time (UTC) for when comic is published to gocomics.com
    // Estimated time is:
    //      0000-0300 EST
    //      0400-0700 UTC
    //      1400-1700 AEST
    // And a margin of error is added just in case
    let time_of_publish = NaiveTime::from_hms_opt(7, 0, 0)
        .expect("Static time failed to parse. This error should never occur.");

    // Today if currently AFTER time of publish for todays comic
    // Yesterday if currently BEFORE time of publish for todays comic
    now.date_naive() - Duration::days(if now.time() > time_of_publish { 0 } else { 1 })
}

pub fn create_target_directory(path: impl AsRef<Path>, remove_existing: bool) -> Result<()> {
    let path = path.as_ref();
    if path.exists() {
        if path.is_file() {
            bail!(io::Error::from(io::ErrorKind::NotADirectory));
        }
        if !remove_existing {
            return Ok(());
        }
        fs::remove_dir_all(path).with_context(|| "removing existing directory")?;
    }
    return fs::create_dir_all(path).with_context(|| "creating empty directory");
}

pub fn get_target_directory() -> Option<PathBuf> {
    const DEFAULT_DIRECTORY_NAME: &str = "garfield";

    let parent = dirs_next::picture_dir()
        .or_else(dirs_next::download_dir)
        .or_else(dirs_next::home_dir)?;
    Some(parent.join(DEFAULT_DIRECTORY_NAME))
}
