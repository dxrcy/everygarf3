use std::fmt;
use std::num::NonZero;
use std::path::PathBuf;

use clap::ValueEnum;
use everygarf::Source;

pub mod defaults {
    use std::num::NonZero;

    pub const JOB_COUNT: NonZero<usize> = NonZero::new(20).unwrap();
    pub const MAX_ATTEMPTS: NonZero<usize> = NonZero::new(10).unwrap();

    pub const TIMEOUT: NonZero<u64> = NonZero::new(5).unwrap();
    pub const TIMEOUT_INITIAL: NonZero<u64> = NonZero::new(20).unwrap();

    pub const CACHE: &str = "https://raw.githubusercontent.com/dxrcy/everygarf-cache/master/cache";
    pub const PROXY: &str = "https://proxy.darcy-700.workers.dev/cors-proxy";
}

#[derive(clap::Parser)]
pub struct Args {
    pub directory: Option<PathBuf>,

    #[arg(long = "tree")]
    pub file_tree: bool,

    #[arg(short = 's', long = "start")]
    pub start_date: Option<chrono::NaiveDate>,

    #[arg(short = 'm', long = "max")]
    pub max_images: Option<NonZero<usize>>,

    #[arg(short = 'j', long = "jobs", default_value_t = defaults::JOB_COUNT)]
    pub job_count: NonZero<usize>,

    #[arg(short = 'a', long = "attempts", default_value_t = defaults::MAX_ATTEMPTS)]
    pub max_attempts: NonZero<usize>,

    #[arg(short = 't', long = "timeout", default_value_t = defaults::TIMEOUT)]
    pub timeout_main: NonZero<u64>,

    #[arg(short = 'T', long = "initial-timeout", default_value_t = defaults::TIMEOUT_INITIAL)]
    pub timeout_initial: NonZero<u64>,

    #[arg(short = 'N', long = "notify-on-fail")]
    pub notify_on_fail: bool,

    #[arg(long = "remove-all")]
    pub remove_all: bool,

    #[arg(short = 'p', long = "proxy", default_value = defaults::PROXY, conflicts_with = "no_proxy")]
    pub proxy: PathBuf,

    #[arg(short = 'P', long = "no-proxy", conflicts_with = "proxy")]
    pub no_proxy: bool,

    #[arg(long = "always-ping")]
    pub always_ping: bool,

    #[arg(short = 'c', long = "cache", default_value = defaults::CACHE, conflicts_with = "source")]
    pub cache: PathBuf,

    #[arg(short = 'C', long = "no-cache", conflicts_with = "cache")]
    pub no_cache: bool,

    #[arg(long = "save-cache")]
    pub save_cache: Option<PathBuf>,

    #[arg(short = 'S', long = "source", requires = "no_cache", default_value_t = Default::default())]
    pub source: Source,

    #[arg(short = 'f', long = "format", ignore_case = true, default_value_t = Default::default())]
    pub format: ImageFormat,

    #[arg(short = 'q', long = "query")]
    pub query: bool,
}

/// Image format (and file extension) to save images as.
#[derive(Default, Clone, Copy, PartialEq, ValueEnum)]
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
