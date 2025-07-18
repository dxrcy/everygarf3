mod args;

use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::{fs, io};

use anyhow::{Context, Result, bail};
use chrono::{Duration, NaiveDate, NaiveTime, Utc};
use clap::Parser;
use futures::StreamExt;
use tokio::runtime::Runtime;

use crate::args::{Args, ImageFormat};

fn main() -> Result<()> {
    let args = Args::parse();

    if args.file_tree {
        unimplemented!("--tree");
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

    let missing_dates =
        date_iter(date_start..=date_end).filter(|date| !existing_dates.contains(date));

    // Must be collected to get count initially
    let pending_dates: Vec<_> = match args.max_images {
        Some(max_images) => missing_dates.take(max_images.into()).collect(),
        None => missing_dates.collect(),
    };
    let pending_count = pending_dates.len();

    let job_count = args.job_count;

    type Message = ();

    Runtime::new().unwrap().block_on(async move {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Message>(job_count.into());

        // Trigger initial progress display
        tx.send(()).await.unwrap();

        tokio::spawn(async move {
            let futures = pending_dates.into_iter().map(|date| {
                let tx = tx.clone();
                async move {
                    run_download(date).await?;
                    tx.send(()).await.unwrap();
                    Ok::<_, anyhow::Error>(())
                }
            });

            let results = futures::stream::iter(futures)
                .buffer_unordered(job_count.into())
                .collect::<Vec<_>>()
                .await;

            for result in results {
                result.unwrap()
            }
        });

        let mut count = 0;
        while let Some(_msg) = rx.recv().await {
            let line_count = 2;
            if count > 0 {
                for _ in 0..line_count {
                    print!("\r\x1b[1A");
                }
            }

            println!("{:6} {:6}", count, pending_count);

            let percent = count as f32 * 100.0 / pending_count as f32;
            let bar_width = 40;
            let bar_progress = count * bar_width / pending_count;
            print!("{:6.2}%", percent);
            print!(" [");
            for i in 0..bar_width {
                if i <= bar_progress {
                    print!("#");
                } else {
                    print!(".");
                }
            }
            print!("]");
            println!();

            count += 1;
        }
    });

    Ok(())
}

pub async fn run_download(_date: NaiveDate) -> Result<()> {
    let ms = 300 + random_int(700) as u64;
    tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
    Ok(())
}

fn random_int(max: usize) -> usize {
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    let nanos = duration.subsec_nanos() as usize;
    nanos % max
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
