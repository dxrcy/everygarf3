use std::fmt::Write as _;
use std::fs;
use std::num::NonZero;
use std::path::Path;

use anyhow::{Context as _, Result};
use bytes::Bytes;
use chrono::NaiveDate;
use everygarf::ImageFormat;
use reqwest::{Client, Url};
use tokio::sync::mpsc;

use crate::state::Update;

pub struct DownloadOptions<'a> {
    pub date: NaiveDate,
    pub client: Client,
    pub directory: &'a Path,
    pub max_attempts: NonZero<usize>,
    pub image_format: ImageFormat,
    pub proxy: Option<&'a Url>,
}

pub async fn download_image<'a>(
    tx: &mpsc::Sender<Result<Update>>,
    options: DownloadOptions<'a>,
) -> Result<()> {
    // TODO(feat): Add error contexts

    let date = options.date;

    let image_url = try_attempts(options.max_attempts.into(), || {
        fetch_image_url(date, &options.client, options.proxy)
    })
    .await
    .with_context(|| "failed to fetch image url")?;

    tx.send(Ok(Update::FetchUrl { date })).await.unwrap();

    let image_bytes = try_attempts(options.max_attempts.into(), || {
        fetch_image_bytes(&image_url, &options.client)
    })
    .await
    .with_context(|| "failed to fetch image data")?;

    tx.send(Ok(Update::FetchImage { date })).await.unwrap();

    save_image(date, image_bytes, options.directory, options.image_format)
        .with_context(|| "failed to save image")?;

    tx.send(Ok(Update::SaveImage { date })).await.unwrap();

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

fn save_image(
    date: NaiveDate,
    bytes: Bytes,
    directory: impl AsRef<Path>,
    image_format: ImageFormat,
) -> Result<()> {
    let filename = format!("{}.{}", date.format("%Y-%m-%d"), image_format);
    let path = directory.as_ref().join(filename);

    if image_format == ImageFormat::Gif {
        fs::write(path, &bytes)?;
    } else {
        let image = image::load_from_memory(&bytes).with_context(|| "loading image from bytes")?;
        image.save(path)?;
    }
    Ok(())
}

async fn fetch_image_bytes(url: &str, client: &Client) -> Result<Bytes> {
    // TODO(feat): Add error contexts
    let response = client.get(url).send().await?.error_for_status()?;
    let bytes = response.bytes().await?;
    Ok(bytes)
}

async fn fetch_image_url(date: NaiveDate, client: &Client, proxy: Option<&Url>) -> Result<String> {
    let page_url = get_page_url(proxy, "https://www.gocomics.com/garfield", date);

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

fn get_page_url(proxy: Option<&Url>, base: &str, date: NaiveDate) -> String {
    let mut url = String::new();
    if let Some(proxy) = proxy {
        write!(url, "{}?", proxy).unwrap();
    }
    write!(url, "{}/{}", base, date.format("%Y/%m/%d")).unwrap();
    url
}
