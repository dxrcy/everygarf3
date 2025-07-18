mod args;

use std::num::NonZero;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, io};

use anyhow::{Context, Result, bail};
use bytes::Bytes;
use chrono::{NaiveDate, NaiveTime, Utc};
use clap::Parser;
use futures::StreamExt;
use reqwest::Client;
use tokio::runtime::Runtime;

use crate::args::{Args, ImageFormat, defaults};

fn main() -> Result<()> {
    let args = Args::parse();

    if args.file_tree {
        unimplemented!("--tree");
    }
    if args.timeout_initial != defaults::TIMEOUT_INITIAL {
        unimplemented!("--initial-timeout");
    }
    if args.notify_on_fail {
        unimplemented!("--notify-on-fail");
    }
    if args.proxy != Path::new(defaults::PROXY) {
        unimplemented!("--roxy");
    }
    if args.no_proxy {
        unimplemented!("--no-proxy");
    }
    if args.cache != Path::new(defaults::CACHE) {
        unimplemented!("--cache");
    }
    if args.no_cache {
        unimplemented!("--no-cache");
    }
    if args.save_cache.is_some() {
        unimplemented!("--save-cache");
    }
    if args.format != ImageFormat::default() {
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
    let request_timeout_main = Duration::from_secs(args.timeout_main.into());
    let max_attempts = args.max_attempts;

    type Message = ();

    Runtime::new().unwrap().block_on(async move {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Message>(job_count.into());
        // Trigger initial progress display
        tx.send(()).await.unwrap();


    const REQUEST_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/118.0.0.0 Safari/537.36";

        let client_main = Client::builder()
            .user_agent(REQUEST_USER_AGENT)
            .timeout(request_timeout_main)
            .build()
            .expect("Failed to build request client (main). This error should never occur.");

        tokio::spawn(async move {
            let futures = pending_dates.into_iter().map(|date| {
                let tx = tx.clone();
                let client = client_main.clone();

                async move {
                    download_image(DownloadOptions { client, date, max_attempts }).await?;
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

struct DownloadOptions {
    client: Client,
    date: NaiveDate,
    max_attempts: NonZero<usize>,
}

async fn download_image(options: DownloadOptions) -> Result<()> {
    // TODO(feat): Add error contexts

    let image_url = try_attempts(options.max_attempts.into(), || {
        fetch_image_url(&options.client, options.date)
    })
    .await?;

    eprintln!("{}", image_url);

    let image_bytes = try_attempts(options.max_attempts.into(), || {
        fetch_image_bytes(&options.client, &image_url)
    })
    .await?;

    eprintln!("bytes: {}", image_bytes.len());

    Ok(())
}

async fn try_attempts<F, T, E, R>(attempts: usize, mut func: F) -> Result<T, E>
where
    F: FnMut() -> R,
    R: Future<Output = Result<T, E>>,
{
    assert!(attempts > 0);
    let mut i = 0;
    loop {
        match func().await {
            Ok(ok) => return Ok(ok),
            Err(err) if i >= attempts => return Err(err),
            _ => (),
        }
        i += 1;
    }
}

async fn fetch_image_bytes(client: &Client, url: &str) -> Result<Bytes> {
    // TODO(feat): Add error contexts
    let response = client.get(url).send().await?.error_for_status()?;
    let bytes = response.bytes().await?;
    Ok(bytes)
}

async fn fetch_image_url(client: &Client, date: NaiveDate) -> Result<String> {
    let page_url = format!(
        "https://www.gocomics.com/garfield/{}",
        date.format("%Y/%m/%d")
    );

    // TODO(feat): Add error contexts
    let response = client.get(&page_url).send().await?.error_for_status()?;
    let body = response.text().await?;
    let image_url = find_image_url(&body).with_context(|| "no url in body")?;
    Ok(image_url.to_string())
}

pub fn find_image_url(body: &str) -> Option<&str> {
    const IMAGE_URL_PREFIX: &str = "https://featureassets.gocomics.com/assets/";
    const IMAGE_URL_LENGTH: usize = 74;

    let char_index = body.find(IMAGE_URL_PREFIX)?;
    body.get(char_index..char_index + IMAGE_URL_LENGTH)
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
    // TODO(refactor)
    use chrono::Duration;

    let (start, end) = (*range.start(), *range.end());
    (0..=(end - start).num_days()).map(move |days| start + Duration::days(days))
}

pub const FIRST_DATE: NaiveDate =
    NaiveDate::from_ymd_opt(1978, 6, 19).expect("Failed to parse const date");

pub fn latest() -> NaiveDate {
    // TODO(refactor)
    use chrono::Duration;

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
