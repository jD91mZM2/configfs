use crate::{cvt, convert::{self, ConversionKind}};

use std::{
    borrow::Cow,
    cmp,
    convert::TryInto,
    io,
    os::unix::fs::MetadataExt,
    path::PathBuf,
    rc::Rc,
};

use easyfuse::{
    returns,
    File,
    FileHandle,
    Permissions,
    Request,
    Result,
};
use fuse::{FileAttr, FileType};
use log::warn;
use time::Timespec;

#[derive(Clone, Debug)]
pub struct Cache {
    stat: FileAttr,
    data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub from_kind: ConversionKind,
    pub source: Rc<PathBuf>,
    pub to_kind: ConversionKind,

    pub readers: u64,
    pub cache: Option<Cache>,
}
impl Config {
    pub fn stat(&self) -> io::Result<FileAttr> {
        let stat = self.source.metadata()?;

        Ok(FileAttr {
            ino: 0,
            size: stat.size(),
            blocks: stat.blocks(),
            atime: Timespec::new(stat.atime(), 0),
            mtime: Timespec::new(stat.mtime(), 0),
            ctime: Timespec::new(stat.ctime(), 0),
            crtime: Timespec::new(stat.ctime(), 0),
            kind: if self.to_kind == ConversionKind::Root {
                FileType::Directory
            } else {
                FileType::RegularFile
            },
            perm: if self.to_kind == ConversionKind::Root {
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
impl File for Config {
    fn getattr(&mut self, _req: &mut Request) -> Result<returns::Attr> {
        let mut stat = cvt(self.stat())?;
        stat.size = 1_000_000;
        Ok(returns::Attr::from(stat))
    }
    fn open(&'_ mut self, _req: &mut Request, _flags: u32) -> Result<FileHandle> {
        if self.readers == 0 {
            let data = cvt(std::fs::read(&*self.source))?;
            let mut data = convert::convert(self.from_kind, self.to_kind, &data)
                .map_err(|err| {
                    warn!("Error converting configs: {}", err);
                    libc::EINVAL
                })?;
            data.push(b'\n');
            self.cache = Some(Cache {
                stat: cvt(self.stat())?,
                data,
            });
        }
        self.readers += 1;
        Ok(FileHandle(0))
    }
    fn close(&'_ mut self, _req: &mut Request, _fh: FileHandle, _flags: u32) -> Result<()> {
        self.readers -= 1;
        if self.readers == 0 {
            self.cache = None;
        }
        Ok(())
    }
    fn read(&'_ mut self, req: &mut Request, _fh: FileHandle, offset: i64, len: u32) -> Result<Cow<'_, [u8]>> {
        let cache = self.cache.as_ref().unwrap();

        req.ensure_access(&cache.stat, Permissions::READ)?;
        let start: usize = offset.try_into().unwrap_or(0);
        let end: usize = cmp::min(
            len.try_into().ok().and_then(|len| start.checked_add(len)).expect("integer overflow"),
            cache.data.len()
        );

        let buf = &cache.data.get(start..end).ok_or(libc::ERANGE)?;
        Ok(Cow::Borrowed(&buf))
    }
}
