#![allow(clippy::uninlined_format_args)]

mod args;
mod dates;
mod download;
mod io;
mod state;
// TODO(refactor): Rename
mod controller;

use std::fs;
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use clap::Parser;
use controller::Sender;
use everygarf::DateUrl;
use reqwest::Client;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

use crate::args::{Args, defaults};
use crate::io::{create_target_directory, get_target_directory};

fn main() -> ExitCode {
    println!("everygarf");
    if let Err(error) = run() {
        println!("failed: {}", error);
        ExitCode::FAILURE
    } else {
        println!("done!");
        ExitCode::SUCCESS
    }
}

fn run() -> Result<()> {
    let args = Args::parse();

    if let Some(option) = check_unimplemented_args(&args) {
        unimplemented!("option {}", option);
    }

    let directory = match args.directory {
        Some(directory) => directory,
        None => get_target_directory()
            .with_context(|| "failed to find appropriate target directory path")?,
    };

    let date_start = args.start_date.unwrap_or(dates::FIRST_DATE);
    let date_end = dates::latest();

    // TODO(refactor): Extract as function
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

    let request_timeout_primary = Duration::from_secs(args.timeout_primary.into());
    let request_timeout_initial = Duration::from_secs(args.timeout_initial.into());

    // TODO(refactor): Rename `proxy_url`, `args.proxy_url`, `args.cache_url`
    let proxy = Some(args.proxy).filter(|_| !args.no_proxy);

    let cache_url = if args.no_cache {
        None
    } else {
        // TODO(feat): Handle better
        let cache_url = args
            .cache
            .to_str()
            .with_context(|| "converting cache path to string")?;
        let cache_url = reqwest::Url::parse(cache_url).with_context(
            || "parsing cache url. reading cache from a local file is not yet implemented",
        )?;
        Some(cache_url)
    };

    create_target_directory(&directory, args.remove_all)
        .with_context(|| "failed to create/clear target directory")?;

    let existing_dates = get_existing_dates(&directory)?;

    let missing_dates = dates::date_iter(date_start..=date_end)
        .filter(|date| !existing_dates.contains(date))
        .map(|date| DateUrl {
            date,
            image_url: None,
        });

    // Must be collected to get count initially
    let mut pending_dates: Vec<DateUrl> = match args.max_images {
        Some(max_images) => missing_dates.take(max_images.into()).collect(),
        None => missing_dates.collect(),
    };
    let pending_count = pending_dates.len();

    let client_primary = Client::builder()
        .user_agent(&args.user_agent)
        .timeout(request_timeout_primary)
        .build()
        .expect("Failed to build request client (primary). This error should never occur.");
    let client_initial = Client::builder()
        .user_agent(&args.user_agent)
        .timeout(request_timeout_initial)
        .build()
        .expect("Failed to build request client (initial). This error should never occur.");

    let (tx, mut rx) = mpsc::channel(args.job_count.into());
    let tx = Sender::new(tx);

    Runtime::new().unwrap().block_on(async move {
        // TODO(refactor): Rename task to `worker` in all contexts
        let downloader_handle = tokio::spawn(async move {
            if download::check_proxy(&tx, &client_initial, proxy.as_ref())
                .await
                .is_err()
            {
                return;
            };

            if let Some(cache_url) = cache_url {
                let mut cache_data = download::fetch_cached_urls(&tx, &client_initial, cache_url)
                    .await
                    .unwrap();
                // Assumes `pending_dates` has no duplicates (which it shouldn't)
                for date_url in &mut pending_dates {
                    date_url.image_url = cache_data.remove(&date_url.date);
                }
            }

            controller::Downloader {
                tx,
                pending_dates,
                client: client_primary,
                directory,
                job_count: args.job_count,
                max_attempts: args.max_attempts,
                image_format: args.image_format,
                proxy,
            }
            .download_pending_images()
            .await;
        });

        if let Err(error) = controller::draw_progress_loop(&mut rx, pending_count).await {
            downloader_handle.abort();
            // Wait for any additional messages, to prevent sender panicking
            while rx.recv().await.is_some() {}
            return Err(error);
        }

        // TODO(feat): Handle better
        if let Err(error) = downloader_handle.await {
            panic!("{}", error);
        }

        Ok(())
    })
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
    if args.save_cache.is_some() {
        return Some("--save-cache");
    }
    if args.query {
        return Some("--query");
    }
    None
}
