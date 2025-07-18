use std::fmt;

use clap::ValueEnum;

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
