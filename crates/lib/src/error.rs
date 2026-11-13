use std::{error, fmt, io};

use crate::Division;

#[derive(Debug)]
pub enum Error {
    CacheWrite(io::Error),
    EmptyDivision(Division),
    Feed(serde_json::Error),
    Fetch(ureq::Error),
    UnknownDivision(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CacheWrite(error) => write!(f, "couldn't write the cache: {error}"),
            Self::EmptyDivision(division) => {
                write!(f, "the feed has no events for {}", division.key())
            }
            Self::Feed(error) => write!(f, "couldn't read the feed: {error}"),
            Self::Fetch(error) => write!(f, "couldn't fetch the feed: {error}"),
            Self::UnknownDivision(division) => write!(
                f,
                "unknown division '{division}' (expected england-and-wales, scotland, \
                 northern-ireland, ew, or ni)"
            ),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::CacheWrite(error) => Some(error),
            Self::Feed(error) => Some(error),
            Self::Fetch(error) => Some(error),
            Self::EmptyDivision(_) | Self::UnknownDivision(_) => None,
        }
    }
}
