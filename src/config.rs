use std::{
    io,
    os::unix::fs::MetadataExt,
    path::PathBuf,
};

use fuse::*;
use time::Timespec;

const CONFIG_BITS: u64 = 3; // Giving us up to 3 binary digits of different config formats

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ConversionKind {
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RootConfigId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConfigId {
    id: RootConfigId,
    kind: ConversionKind,
}
impl ConfigId {
    #[inline(always)]
    pub fn from_ino(ino: u64) -> Option<Self> {
        Some(Self {
            id: RootConfigId(ino >> CONFIG_BITS),
            kind: match ino & ((1 << CONFIG_BITS) - 1) {
                0 => ConversionKind::Root,
                1 => ConversionKind::Json,
                2 => ConversionKind::Toml,
                3 => ConversionKind::Yaml,
                _ => return None,
            },
        })
    }
    #[inline(always)]
    pub fn root_inode(self) -> u64 {
        self.id.0 << CONFIG_BITS
    }
    #[inline(always)]
    pub fn inode(self) -> u64 {
        self.root_inode() + self.kind as u64
    }
    #[inline(always)]
    pub fn convert(self, kind: ConversionKind) -> Self {
        Self {
            kind,
            ..self
        }
    }
    #[inline(always)]
    pub fn kind(self) -> ConversionKind {
        self.kind
    }
    #[inline(always)]
    pub fn root(self) -> RootConfigId {
        self.id
    }
    #[inline(always)]
    pub fn as_root(self) -> Option<RootConfigId> {
        Some(self.id).filter(|_| self.kind == ConversionKind::Root)
    }
}

impl From<RootConfigId> for ConfigId {
    fn from(id: RootConfigId) -> Self {
        Self {
            id,
            kind: ConversionKind::Root,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub source: PathBuf,
}

pub struct ConfigRef<'a> {
    pub root: &'a Config,
    pub id: ConfigId,
}

impl<'a> ConfigRef<'a> {
    pub fn stat(&self) -> io::Result<FileAttr> {
        let stat = self.root.source.metadata()?;

        Ok(FileAttr {
            ino: self.id.root_inode(),
            size: stat.size(),
            blocks: stat.blocks(),
            atime: Timespec::new(stat.atime(), 0),
            mtime: Timespec::new(stat.mtime(), 0),
            ctime: Timespec::new(stat.ctime(), 0),
            crtime: Timespec::new(stat.ctime(), 0),
            kind: if self.id.kind() == ConversionKind::Root {
                FileType::Directory
            } else {
                FileType::RegularFile
            },
            perm: if self.id.kind() == ConversionKind::Root {
                0o555
            } else {
                0o444
            },
            nlink: 0,
            uid: stat.uid(),
            gid: stat.gid(),
            rdev: stat.rdev() as u32,
            flags: 0,
        })
    }
}
