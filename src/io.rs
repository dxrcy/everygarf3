use std::path::{Path, PathBuf};
use std::{fs, io};

use anyhow::{Context as _, Result, bail};

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
