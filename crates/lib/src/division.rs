use std::str::FromStr;

use crate::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Division {
    EnglandAndWales,
    NorthernIreland,
    Scotland,
}

impl Division {
    pub const ALL: [Self; 3] = [Self::EnglandAndWales, Self::NorthernIreland, Self::Scotland];

    /// The division's key in the gov.uk feed.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::EnglandAndWales => "england-and-wales",
            Self::NorthernIreland => "northern-ireland",
            Self::Scotland => "scotland",
        }
    }
}

impl FromStr for Division {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "england-and-wales" | "ew" => Ok(Self::EnglandAndWales),
            "northern-ireland" | "ni" => Ok(Self::NorthernIreland),
            "scotland" => Ok(Self::Scotland),
            _ => Err(Error::UnknownDivision(s.to_owned())),
        }
    }
}
