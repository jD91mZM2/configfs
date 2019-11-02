use std::{
    collections::HashMap,
    ffi::{OsString, OsStr},
    io,
    path::{PathBuf, Path},
    rc::Rc,
};

use easyfuse::{
    dir,
    returns,
    AttrBuilder,
    Directory,
    DirectoryResource,
    EasyFuse,
    FileResource,
    Inode,
    Request,
    Result,
};
use fuse::{FileAttr, FileType};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    /// Where the filesystem should be mounted
    mountpoint: PathBuf,
}

fn main() -> std::io::Result<()> {
    env_logger::init();
    let opt = Opt::from_args();

    let mut fs = EasyFuse::new();
    fs.set_root(DirectoryResource(ConfigFs::new()));

    fuse::mount(fs, &opt.mountpoint, &[])
}

mod config;
mod convert;

use self::config::Config;
use self::convert::ConversionKind;

fn cvt<T>(err: io::Result<T>) -> Result<T> {
    err.map_err(|err| err.raw_os_error().unwrap_or(libc::EIO))
}

#[derive(Debug)]
struct ConfigFs {
    configs: HashMap<Inode, Config>,
    paths:   HashMap<OsString, Inode>,
    attrs:   FileAttr,
}
impl ConfigFs {
    fn new() -> Self {
        Self {
            configs: HashMap::new(),
            paths:   HashMap::new(),
            attrs:   AttrBuilder::directory().build()
        }
    }
}
impl Directory for ConfigFs {
    fn getattr(&mut self, _req: &mut Request) -> Result<returns::Attr> {
        Ok(returns::Attr::from(self.attrs))
    }
    fn lookup(&mut self, _req: &mut Request, name: &OsStr) -> Result<returns::Entry> {
        if let Some(inode) = self.paths.get(name) {
            let config = self.configs.get(inode).expect("missing config for valid inode");
            let mut attrs = cvt(config.stat())?;
            attrs.ino = inode.0;
            Ok(returns::Entry::from(returns::Attr::from(attrs)))
        } else {
            Err(libc::ENOENT)
        }
    }
    fn readdir(&mut self, _req: &mut Request, output: &mut Vec<returns::DirEntry>) -> Result<()> {
        for (path, &inode) in &self.paths {
            output.push(returns::DirEntry::new(inode, FileType::Directory, path.clone()));
        }
        Ok(())
    }
    fn symlink(&mut self, req: &mut Request, link: &OsStr, origin: &Path) -> Result<returns::Entry> {
        println!("Symlink {:?} to {}", link, origin.display());

        let origin = Rc::new(origin.to_owned());

        let from_kind = ConversionKind::guess(link)?;

        let root = Config {
            from_kind,
            source: Rc::clone(&origin),
            to_kind: ConversionKind::Root,
            cache: None,
            readers: 0,
        };

        let attrs = returns::Attr::from(cvt(root.stat())?);
        let mut dir = dir::StaticDirectory::new(attrs);

        for &to_kind in ConversionKind::all() {
            dir.bind(to_kind.file(), req.fs.register(FileResource(Config {
                from_kind,
                source: Rc::clone(&origin),
                to_kind,
                cache: None,
                readers: 0,
            })));
        }

        let inode = req.fs.register(dir);
        self.configs.insert(inode, root);
        self.paths.insert(link.to_owned(), inode);

        Ok(returns::Entry::from(attrs))
    }
}
