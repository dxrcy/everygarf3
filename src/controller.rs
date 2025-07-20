use std::num::NonZero;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use chrono::NaiveDate;
use futures::StreamExt as _;
use reqwest::{Client, Url};
use tokio::sync::mpsc;

use crate::download::{DownloadOptions, download_image};
use crate::state::{State, Status, Update, UpdateSuccess, UpdateWarning};

pub struct Downloader {
    pub tx: Sender,
    pub pending_dates: Vec<NaiveDate>,
    pub client: Client,
    pub directory: PathBuf,
    pub job_count: NonZero<usize>,
    pub max_attempts: NonZero<usize>,
    pub image_format: everygarf::ImageFormat,
    pub proxy: Option<Url>,
}

#[derive(Clone)]
pub struct Sender {
    tx: mpsc::Sender<Result<Update>>,
}

impl Sender {
    pub fn new(tx: mpsc::Sender<Result<Update>>) -> Self {
        Self { tx }
    }

    pub async fn send_success(&self, success: UpdateSuccess) {
        self.send(Ok(Ok(success))).await;
    }

    pub async fn send_warning(&self, warning: UpdateWarning) {
        self.send(Ok(Err(warning))).await;
    }

    pub async fn send_error(&self, error: anyhow::Error) {
        self.send(Err(error)).await;
    }

    async fn send(&self, result: Result<Update>) {
        self.tx
            .send(result)
            .await
            .expect("Failed to send message to main task");
    }
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
                if let Err(error) = download_image(&tx, options).await {
                    tx.send_error(error).await;
                };
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

pub async fn draw_progress_loop(
    rx: &mut mpsc::Receiver<Result<Update>>,
    pending_count: usize,
) -> Result<()> {
    let mut state = State::new(pending_count);

    draw_progress(&mut state);
    state.advance_status();

    while let Some(msg) = rx.recv().await {
        match msg {
            Ok(update) => {
                state.update(update);
                draw_progress(&mut state);
            }
            Err(error) => {
                state.set_failed();
                draw_progress(&mut state);
                return Err(error);
            }
        }
    }

    state.advance_status();
    draw_progress(&mut state);
    Ok(())
}

fn draw_progress(state: &mut State) {
    let line_count = 4;
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

    print!(" status: ");
    match state.status() {
        Status::PingProxy => println!("pinging proxy server..."),
        Status::Working => println!("in progress..."),
        Status::Epilogue => println!("all done."),
        Status::Failed => println!("failed!"),
    }

    print!(" update: ");
    if let Some(success) = state.latest_success() {
        match success {
            UpdateSuccess::ProxyPing => println!("proxy server working."),

            UpdateSuccess::FetchUrl { date } => println!("{} | fetched image url.", date),
            UpdateSuccess::FetchImage { date } => {
                println!("{} | downloaded image.", date)
            }
            UpdateSuccess::SaveImage { date } => println!("{} | saved image.", date),
        }
    } else {
        println!("...")
    }

    print!("warning: ");
    if let Some(warning) = state.latest_warning() {
        match warning {
            UpdateWarning::FetchUrl { attempt, date } => {
                println!(
                    "{} | failed to fetch image url (attempt {}).",
                    date, attempt
                )
            }
            UpdateWarning::FetchImage { attempt, date } => {
                println!("{} | failed to download image (attempt {}).", date, attempt)
            }
        }
    } else {
        println!("...")
    }
}

pub async fn check_proxy(tx: &Sender, client: &Client, proxy: Option<&Url>) -> Result<(), ()> {
    let Some(proxy) = proxy else {
        return Ok(());
    };

    if let Err(_error) = try_ping(client, proxy.clone()).await {
        tx.send_error(anyhow!("failed to access proxy server"))
            .await;
        return Err(());
    };

    tx.send_success(UpdateSuccess::ProxyPing).await;
    Ok(())
}

async fn try_ping(client: &Client, proxy: Url) -> reqwest::Result<()> {
    client.get(proxy).send().await?.error_for_status()?;
    Ok(())
}
