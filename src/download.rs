use std::fmt::Write as _;
use std::fs;
use std::num::NonZero;
use std::path::Path;

use anyhow::{Context as _, Result, anyhow};
use bytes::Bytes;
use chrono::NaiveDate;
use everygarf::{DateUrl, ImageFormat};
use reqwest::{Client, Url};

use crate::controller::Sender;
use crate::state::{UpdateSuccess, UpdateWarning};

// TODO(refactor): Move these
const IMAGE_URL_PREFIX: &str = "https://featureassets.gocomics.com/assets/";
const IMAGE_URL_LENGTH: usize = 74;

pub type CacheData = std::collections::HashMap<NaiveDate, Url>;

pub struct DownloadOptions<'a> {
    pub date_url: DateUrl,
    pub client: Client,
    pub directory: &'a Path,
    pub max_attempts: NonZero<usize>,
    pub image_format: ImageFormat,
    pub proxy: Option<&'a Url>,
}

pub async fn check_proxy(tx: &Sender, client: &Client, proxy: Option<&Url>) -> Result<(), ()> {
    let Some(proxy) = proxy else {
        return Ok(());
    };

    if let Err(_error) = fetch_response(client, proxy.clone()).await {
        tx.send_error(anyhow!("failed to access proxy server"))
            .await;
        return Err(());
    };

    tx.send_success(UpdateSuccess::ProxyPing).await;
    Ok(())
}

pub async fn fetch_cached_urls(tx: &Sender, client: &Client, cache_url: Url) -> Result<CacheData> {
    // TODO(feat): Support path to local file
    let text = fetch_text(client, cache_url).await?;
    let cache_data = parse_cached_urls(&text).with_context(|| "malformed cache file");
    tx.send_success(UpdateSuccess::FetchCache).await;
    cache_data
}

fn parse_cached_urls(text: &str) -> Result<CacheData> {
    let mut entries = CacheData::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // TODO(feat): Handle errors
        let (date_string, url_path) =
            split_columns(line).with_context(|| "bad line or something")?;
        // TODO(feat): Extract format string to constant in all contexts
        let date =
            NaiveDate::parse_from_str(date_string, "%Y-%m-%d").with_context(|| "bad datej")?;
        let image_url = expand_image_url(url_path.trim()).with_context(|| "bad url path")?;
        entries.insert(date, image_url);
    }
    Ok(entries)
}

fn split_columns(line: &str) -> Option<(&str, &str)> {
    Some(line.split_at(line.find(' ')?))
}

fn expand_image_url(url_path: &str) -> Option<Url> {
    // Assumes base url is well-formed. The only parsing error should be a malformed path
    let url = IMAGE_URL_PREFIX.to_string() + url_path;
    Url::parse(&url).ok()
}

pub async fn download_image<'a>(tx: &Sender, options: DownloadOptions<'a>) -> Result<()> {
    let date = options.date_url.date;

    let image_url = match options.date_url.image_url {
        Some(image_url) => image_url,
        None => {
            let image_url = try_attempts(
                &tx,
                options.max_attempts.into(),
                || fetch_image_url(date, &options.client, options.proxy),
                |attempt, _| UpdateWarning::FetchUrl { attempt, date },
            )
            .await
            .with_context(|| "failed to fetch image url")?;

            tx.send_success(UpdateSuccess::FetchUrl { date }).await;
            image_url
        }
    };

    let image_bytes = try_attempts(
        &tx,
        options.max_attempts.into(),
        || fetch_bytes(&options.client, image_url.clone()),
        |attempt, _| UpdateWarning::FetchImage { attempt, date },
    )
    .await
    .with_context(|| "failed to fetch image data")?;

    tx.send_success(UpdateSuccess::FetchImage { date }).await;

    save_image(date, image_bytes, options.directory, options.image_format)
        .with_context(|| "failed to save image")?;

    tx.send_success(UpdateSuccess::SaveImage { date }).await;

    Ok(())
}

async fn try_attempts<F, R, T, W>(
    tx: &Sender,
    attempts: usize,
    mut func: F,
    mut warning: W,
) -> Result<T>
where
    F: FnMut() -> R,
    R: Future<Output = Result<T>>,
    W: FnMut(usize, anyhow::Error) -> UpdateWarning,
{
    assert!(attempts > 0);
    let mut i = 0;
    loop {
        match func().await {
            Ok(ok) => return Ok(ok),
            Err(error) if i < attempts => tx.send_warning(warning(i, error)).await,
            Err(error) => return Err(error),
        }
        i += 1;
    }
}

async fn fetch_image_url(date: NaiveDate, client: &Client, proxy: Option<&Url>) -> Result<Url> {
    // TODO(refactor): Extract as constant `&str`
    let page_url = get_page_url(proxy, "https://www.gocomics.com/garfield", date);

    // TODO(feat): Add error contexts
    let response = client.get(&page_url).send().await?.error_for_status()?;
    let body = response.text().await?;
    let image_url = find_image_url(&body).with_context(|| "extracting image url from page")?;
    Ok(image_url)
}

fn get_page_url(proxy: Option<&Url>, base_url: &str, date: NaiveDate) -> String {
    let mut url = String::new();
    if let Some(proxy) = proxy {
        write!(url, "{}?", proxy).unwrap();
    }
    write!(url, "{}/{}", base_url, date.format("%Y/%m/%d")).unwrap();
    url
}

fn find_image_url(body: &str) -> Result<Url> {
    let char_index = body
        .find(IMAGE_URL_PREFIX)
        .with_context(|| "no url in body")?;

    let image_url = body
        .get(char_index..char_index + IMAGE_URL_LENGTH)
        .with_context(|| "no url in body")?;

    let image_url = Url::parse(image_url).with_context(|| "failed to parse url")?;

    Ok(image_url)
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

async fn fetch_text(client: &Client, url: Url) -> Result<String> {
    // TODO(feat): Add error contexts
    fetch_response(client, url)
        .await?
        .text()
        .await
        .with_context(|| "...")
}

async fn fetch_bytes(client: &Client, url: Url) -> Result<Bytes> {
    // TODO(feat): Add error contexts
    fetch_response(client, url)
        .await?
        .bytes()
        .await
        .with_context(|| "...")
}

async fn fetch_response(client: &Client, url: Url) -> Result<reqwest::Response> {
    // TODO(feat): Add error contexts
    client
        .get(url)
        .send()
        .await
        .with_context(|| "...")?
        .error_for_status()
        .with_context(|| "...")
}
