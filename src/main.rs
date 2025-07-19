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
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use clap::Parser;
use reqwest::Client;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

use crate::args::{Args, defaults};
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

    let request_timeout_primary = Duration::from_secs(args.timeout_primary.into());
    let request_timeout_initial = Duration::from_secs(args.timeout_initial.into());

    let proxy = Some(args.proxy).filter(|_| !args.no_proxy);

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

    let (tx, rx) = mpsc::channel(args.job_count.into());

    Runtime::new().unwrap().block_on(async move {
        tokio::spawn(async move {
            if controller::check_proxy(&tx, &client_initial, proxy.as_ref())
                .await
                .is_err()
            {
                return;
            };

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
        controller::draw_progress_loop(rx, pending_count).await;
    });

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
