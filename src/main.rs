mod args;
mod dates;
mod download;
mod io;

use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use clap::Parser;
use futures::StreamExt;
use reqwest::Client;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

use crate::args::{Args, defaults};
use crate::download::{DownloadOptions, download_image};
use crate::io::{create_target_directory, get_target_directory};

fn main() -> Result<()> {
    let args = Args::parse();

    if let Some(option) = check_unimplemented_args(&args) {
        unimplemented!("option {}", option);
    }

    let directory = match args.directory {
        Some(directory) => directory,
        None => get_target_directory()
            .with_context(|| "failed to find appropriate target directory path")?,
    };

    create_target_directory(&directory, args.remove_all)
        .with_context(|| "failed to create/clear target directory")?;

    let date_start = args.start_date.unwrap_or(dates::FIRST_DATE);
    let date_end = dates::latest();

    if date_start < dates::FIRST_DATE {
        bail!(
            "Start date ({}) must not be before date of first comic ({})",
            date_start,
            dates::FIRST_DATE,
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
        dates::date_iter(date_start..=date_end).filter(|date| !existing_dates.contains(date));

    // Must be collected to get count initially
    let pending_dates: Vec<_> = match args.max_images {
        Some(max_images) => missing_dates.take(max_images.into()).collect(),
        None => missing_dates.collect(),
    };
    let pending_count = pending_dates.len();

    let request_timeout_main = Duration::from_secs(args.timeout_main.into());

    let client_main = Client::builder()
        .user_agent(&args.user_agent)
        .timeout(request_timeout_main)
        .build()
        .expect("Failed to build request client (main). This error should never occur.");

    let (tx, mut rx) = mpsc::channel::<()>(args.job_count.into());

    Runtime::new().unwrap().block_on(async move {
        tokio::spawn(async move {
            download_pending_images(
                tx,
                pending_dates,
                client_main.clone(),
                directory,
                args.job_count,
                args.max_attempts,
                args.image_format,
            )
            .await
        });

        draw_progress(0, pending_count);
        for i in 1.. {
            if rx.recv().await.is_none() {
                continue;
            };
            draw_progress(i, pending_count);
        }
    });

    Ok(())
}

async fn download_pending_images(
    tx: mpsc::Sender<()>,
    pending_dates: Vec<NaiveDate>,
    client: Client,
    directory: std::path::PathBuf,
    job_count: std::num::NonZero<usize>,
    max_attempts: std::num::NonZero<usize>,
    image_format: everygarf::ImageFormat,
) {
    let futures = pending_dates.into_iter().map(|date| {
        let tx = tx.clone();
        let client = client.clone();
        let directory = &directory;

        async move {
            download_image(DownloadOptions {
                date,
                client,
                directory,
                max_attempts,
                image_format,
            })
            .await?;
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
}

fn draw_progress(current: usize, total: usize) {
    let line_count = 2;
    let bar_width = 40;

    let percent = current as f32 * 100.0 / total as f32;
    let bar_progress = current * bar_width / total;

    if current > 0 {
        for _ in 0..line_count {
            print!("\r\x1b[1A");
        }
    }

    println!("{:6} {:6}", current, total);

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

fn check_unimplemented_args(args: &Args) -> Option<&'static str> {
    if args.file_tree {
        return Some("--tree");
    }
    if args.timeout_initial != defaults::TIMEOUT_INITIAL {
        return Some("--initial-timeout");
    }
    if args.notify_on_fail {
        return Some("--notify-on-fail");
    }
    if args.proxy != Path::new(defaults::PROXY) {
        return Some("--proxy");
    }
    if args.no_proxy {
        return Some("--no-proxy");
    }
    if args.cache != Path::new(defaults::CACHE) {
        return Some("--cache");
    }
    if args.no_cache {
        return Some("--no-cache");
    }
    if args.save_cache.is_some() {
        return Some("--save-cache");
    }
    if args.query {
        return Some("--query");
    }
    None
}
