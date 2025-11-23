use anyhow::bail;
use serde::Deserialize;
use std::{fmt::Display, str::FromStr};

#[derive(Clone, Copy, Debug, Deserialize)]
pub enum Visibility {
    Public,
    Unlisted,
    Private,
    Direct,
}

impl FromStr for Visibility {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "public" => Ok(Self::Public),
            "unlisted" => Ok(Self::Unlisted),
            "private" => Ok(Self::Private),
            "direct" => Ok(Self::Direct),
            other => bail!("unknown VISIBILITY: {other}"),
        }
    }
}

impl Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Visibility::Public => "public",
            Visibility::Unlisted => "unlisted",
            Visibility::Private => "private",
            Visibility::Direct => "direct",
        };

        write!(f, "{}", s)
    }
}
