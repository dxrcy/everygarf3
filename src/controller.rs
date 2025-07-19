use std::num::NonZero;
use std::path::PathBuf;

use anyhow::Result;
use chrono::NaiveDate;
use futures::StreamExt as _;
use reqwest::Client;
use tokio::sync::mpsc;

use crate::download::{DownloadOptions, download_image};

#[derive(Debug)]
pub enum Message {
    Start,
    End,
    CompletedDate(NaiveDate),
}

pub struct Downloader {
    pub tx: mpsc::Sender<Message>,
    pub pending_dates: Vec<NaiveDate>,
    pub client: Client,
    pub directory: PathBuf,
    pub job_count: NonZero<usize>,
    pub max_attempts: NonZero<usize>,
    pub image_format: everygarf::ImageFormat,
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
            };

            async move {
                download_image(options).await?;
                tx.send(Message::CompletedDate(date)).await.unwrap();
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

pub async fn draw_progress_loop(mut rx: mpsc::Receiver<Message>, pending_count: usize) {
    draw_progress(Message::Start, 0, pending_count);

    let mut i = 0;
    while let Some(msg) = rx.recv().await {
        i += 1;
        draw_progress(msg, i, pending_count);
    }

    draw_progress(Message::End, i, pending_count);
}

fn draw_progress(msg: Message, current: usize, total: usize) {
    let line_count = 2;
    let bar_width = 40;

    let percent = current as f32 * 100.0 / total as f32;
    let bar_progress = current * bar_width / total;

    if current > 0 {
        for _ in 0..line_count {
            print!("\r"); // Move cursor to beginning of line
            print!("\x1b[1A"); // Move cursor up
            print!("\x1b[2K"); // Clear entire line
        }
    }

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

    print!("status: ");
    match msg {
        Message::Start => println!("waiting..."),
        Message::End => println!("all done."),
        Message::CompletedDate(date) => println!("downloaded {}.", date),
    }
}
