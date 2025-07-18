use std::num::NonZero;
use std::path::PathBuf;

use anyhow::Result;
use chrono::NaiveDate;
use futures::StreamExt as _;
use reqwest::{Client, Url};
use tokio::sync::mpsc;

use crate::download::{DownloadOptions, download_image};
use crate::state::{State, Status, Update};

pub struct Downloader {
    pub tx: mpsc::Sender<Update>,
    pub pending_dates: Vec<NaiveDate>,
    pub client: Client,
    pub directory: PathBuf,
    pub job_count: NonZero<usize>,
    pub max_attempts: NonZero<usize>,
    pub image_format: everygarf::ImageFormat,
    pub proxy: Option<Url>,
}

pub async fn check_proxy(
    tx: &mpsc::Sender<Update>,
    client: &Client,
    proxy: Option<&Url>,
) -> Result<(), ()> {
    let Some(proxy) = proxy else {
        return Ok(());
    };
    // TODO(feat): Send error value
    if let Err(_error) = try_ping(client, proxy.clone()).await {
        tx.send(Update::ProxyPingFail).await.unwrap();
        return Err(());
    };
    return Ok(());
}

impl Downloader {
    pub async fn download_pending_images(self) {
        let futures = self.pending_dates.into_iter().map(|date| {
            let tx = self.tx.clone();
            let options = DownloadOptions {
                date,
                client: self.client.clone(),
                directory: &self.directory,
                max_attempts: self.max_attempts,
                image_format: self.image_format,
                proxy: self.proxy.as_ref(),
            };

            async move {
                download_image(&tx, options).await?;
                Ok(())
            }
        });

        let results: Vec<Result<()>> = futures::stream::iter(futures)
            .buffer_unordered(self.job_count.into())
            .collect()
            .await;

        for result in results {
            // TODO(feat): Send errors to main thread
            result.unwrap()
        }
    }
}

pub async fn draw_progress_loop(mut rx: mpsc::Receiver<Update>, pending_count: usize) {
    let mut state = State::new(pending_count);

    draw_progress(&mut state);
    state.advance_status();

    while let Some(msg) = rx.recv().await {
        state.update(msg);
        draw_progress(&mut state);
    }

    state.advance_status();
    draw_progress(&mut state);
}

fn draw_progress(state: &mut State) {
    let line_count = 3;
    let bar_width = 40;

    let current = state.completed_units();
    let total = state.total_units();

    let percent = current as f32 * 100.0 / total as f32;
    let bar_progress = current * bar_width / total;

    if !state.record_draw() {
        for _ in 0..line_count {
            print!("\r"); // Move cursor to beginning of line
            print!("\x1b[1A"); // Move cursor up
            print!("\x1b[2K"); // Clear entire line
        }
    }

    print!("{:6.2}%", percent);
    print!(" [");
    for i in 0..bar_width {
        if i < bar_progress {
            print!("#");
        } else {
            print!(".");
        }
    }
    print!("]");
    println!();

    print!("status: ");
    match state.status() {
        Status::PingProxy => println!("pinging proxy server..."),
        Status::Working { .. } => println!("in progress..."),
        Status::Epilogue => println!("all done."),
        Status::Failed => println!("failed!"),
    }

    print!("latest: ");
    if let Some(update) = state.latest_update() {
        match update {
            Update::ProxyPingFail => println!("unable to access proxy server"),

            Update::FetchUrlSuccess { date } => println!("{} | found image url.", date),
            Update::FetchImageSuccess { date } => {
                println!("{} | downloaded image.", date)
            }
            Update::SaveImageSuccess { date } => println!("{} | saved image.", date),
        }
    } else {
        println!("-")
    }
}

async fn try_ping(client: &Client, proxy: Url) -> reqwest::Result<()> {
    client.get(proxy).send().await?.error_for_status()?;
    Ok(())
}
