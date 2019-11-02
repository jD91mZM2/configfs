use std::{
    ffi::OsStr,
    path::Path,
};

use easyfuse::Result;
use log::warn;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ConversionKind {
    // The Root kind probably shouldn't exist, it's weird and doesn't fit
    Root,
    Json,
    Toml,
    Yaml,
}

impl ConversionKind {
    pub fn all() -> &'static [Self] {
        &[
            Self::Json,
            Self::Toml,
            Self::Yaml,
        ]
    }
    pub fn file(self) -> &'static str {
        match self {
            Self::Root => panic!("can't call file() on root conversion"),
            Self::Json => "config.json",
            Self::Toml => "config.toml",
            Self::Yaml => "config.yaml",
        }
    }
    pub fn guess(name: &OsStr) -> Result<Self> {
        let path = Path::new(name);
        match path.extension().and_then(|n| n.to_str()) {
            Some("json") => Ok(Self::Json),
            Some("toml") => Ok(Self::Toml),
            Some("yaml") => Ok(Self::Yaml),
            _ => {
                warn!("Can't guess config kind from file description of: {}", path.display());
                Err(libc::ENAMETOOLONG)
            }
        }
    }
}

pub fn convert(from: ConversionKind, to: ConversionKind, data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let value: toml::Value = match from {
        ConversionKind::Root => panic!("root is not a valid kind here"),
        ConversionKind::Json => json5::from_str(std::str::from_utf8(data)?)?,
        ConversionKind::Toml => toml::from_slice(data)?,
        ConversionKind::Yaml => serde_yaml::from_slice(data)?,
    };
    Ok(match to {
        ConversionKind::Root => panic!("root is not a valid kind here"),
        ConversionKind::Json => serde_json::to_vec_pretty(&value)?,
        ConversionKind::Toml => toml::to_string_pretty(&value)?.into_bytes(),
        ConversionKind::Yaml => serde_yaml::to_vec(&value)?,
    })
}
