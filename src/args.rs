use std::fmt;
use std::num::NonZero;
use std::path::PathBuf;

use clap::ValueEnum;
use everygarf::Source;

#[derive(clap::Parser)]
pub struct Args {
    pub directory: Option<PathBuf>,

    #[arg(long = "tree")]
    pub file_tree: bool,

    #[arg(short = 's', long = "start")]
    pub start_date: Option<chrono::NaiveDate>,

    #[arg(short = 'm', long = "max")]
    pub max_images: Option<NonZero<usize>>,

    #[arg(short = 'j', long = "jobs", default_value_t = const { NonZero::new(20).unwrap() })]
    pub job_count: NonZero<usize>,

    #[arg(short = 'a', long = "attempts")]
    pub max_attempts: Option<NonZero<usize>>,

    #[arg(short = 't', long = "timeout")]
    pub request_timeout: Option<NonZero<usize>>,

    #[arg(short = 'T', long = "initial-timeout")]
    pub request_timeout_initial: Option<NonZero<usize>>,

    #[arg(short = 'N', long = "notify-on-fail")]
    pub notify_on_fail: bool,

    #[arg(long = "remove-all")]
    pub remove_all: bool,

    #[arg(short = 'p', long = "proxy", conflicts_with = "no_proxy", default_value = everygarf::PROXY_DEFAULT)]
    pub proxy: PathBuf,

    #[arg(short = 'P', long = "no-proxy", conflicts_with = "proxy")]
    pub no_proxy: bool,

    #[arg(long = "always-ping")]
    pub always_ping: bool,

    #[arg(short = 'c', long = "cache", default_value = everygarf::CACHE_DEFAULT, conflicts_with = "source")]
    pub cache: PathBuf,

    #[arg(short = 'C', long = "no-cache", conflicts_with = "cache")]
    pub no_cache: bool,

    #[arg(long = "save-cache")]
    pub save_cache: Option<PathBuf>,

    #[arg(short = 'S', long = "source", requires = "no_cache", default_value_t = Source::default())]
    pub source: Source,

    #[arg(short = 'f', long = "format", ignore_case = true, default_value_t = Default::default())]
    pub format: ImageFormat,

    #[arg(short = 'q', long = "query")]
    pub query: bool,
}

/// Image format (and file extension) to save images as.
#[derive(Default, Clone, Copy, ValueEnum)]
pub enum ImageFormat {
    #[default]
    Gif,
    Png,
    Jpg,
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_possible_value().unwrap().get_name())
    }
}
