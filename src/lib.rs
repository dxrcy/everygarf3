use std::fmt;

use clap::ValueEnum;

pub const PROXY_DEFAULT: &str = "https://proxy.darcy-700.workers.dev/cors-proxy";
pub const CACHE_DEFAULT: &str =
    "https://raw.githubusercontent.com/dxrcy/everygarf-cache/master/cache";

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Source {
    #[default]
    Gocomics,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_possible_value().unwrap().get_name())
    }
}
