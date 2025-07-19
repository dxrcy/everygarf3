use std::fmt;

use clap::ValueEnum;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Source {
    #[default]
    Gocomics,
}

/// Image format (and file extension) to save images as.
#[derive(Default, Clone, Copy, PartialEq, ValueEnum)]
pub enum ImageFormat {
    #[default]
    Gif,
    Png,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_possible_value().unwrap().get_name())
    }
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_possible_value().unwrap().get_name())
    }
}
