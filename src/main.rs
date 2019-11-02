use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{PathBuf, Path},
};

use fuse::*;
use structopt::StructOpt;
use time::Timespec;

#[derive(Debug, StructOpt)]
struct Opt {
    /// Where the filesystem should be mounted
    mountpoint: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = Opt::from_args();

    println!("{}", opt.mountpoint.display());
    fuse::mount(ConfigFs::new(), &opt.mountpoint, &[])?;
    Ok(())
}

const ROOT_ID: u64  = 1;
const TTL: Timespec = Timespec { sec: 1, nsec: 0 }; // idk what this is

mod config;

use self::config::{
    Config,
    ConfigId,
    ConfigRef,
    ConversionKind,
    RootConfigId,
};

#[derive(Debug)]
struct ConfigFs {
    configs: HashMap<RootConfigId, Config>,
    paths:   HashMap<PathBuf, ConfigId>,
    next_id: RootConfigId,
}

impl ConfigFs {
    fn new() -> Self {
        Self {
            configs: HashMap::new(),
            paths:   HashMap::new(),
            next_id: RootConfigId(1),
        }
    }
    fn ino_to_config(&'_ self, ino: u64) -> Option<ConfigRef<'_>> {
        ConfigId::from_ino(ino)
            .and_then(|id| {
                self.configs.get(&id.root())
                    .map(|root| ConfigRef { root, id })
            })
    }
    fn fake_stat_file(ino: u64) -> FileAttr {
        FileAttr {
            ino,
            size: 0,
            blocks: 0,
            atime: time::now().to_timespec(),
            mtime: time::now().to_timespec(),
            ctime: time::now().to_timespec(),
            crtime: time::now().to_timespec(),
            kind: FileType::RegularFile,
            perm: 0o444,
            nlink: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
            flags: 0,
        }
    }
    fn fake_stat_dir(ino: u64) -> FileAttr {
        FileAttr {
            kind: FileType::Directory,
            perm: 0o755,
            ..Self::fake_stat_file(ino)
        }
    }
}

impl Filesystem for ConfigFs {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        println!("Looking up: {:?}", name);
        let path = Path::new(name);
        if parent == ROOT_ID {
            if let Some(&id) = self.paths.get(path) {
                let config = ConfigRef {
                    root: self.configs.get(&id.root()).expect("missing config for valid id"),
                    id,
                };
                match config.stat() {
                    Ok(stat) => reply.entry(&TTL, &stat, 0),
                    Err(err) => reply.error(err.raw_os_error().unwrap_or(libc::EIO)),
                }
            } else {
                reply.error(libc::ENOENT);
            }
        } else if let Some(root) = ConfigId::from_ino(parent).and_then(|id| id.as_root()) {
            let kind = ConversionKind::all().iter()
                .find(|kind| Path::new(kind.file()) == path);

            if let Some((config, &kind)) = self.configs.get(&root).and_then(|root| kind.map(|kind| (root, kind))) {
                let id = ConfigId::from(root).convert(kind);
                let config = ConfigRef {
                    root: config,
                    id,
                };
                match config.stat() {
                    Ok(stat) => reply.entry(&TTL, &stat, 0),
                    Err(err) => reply.error(err.raw_os_error().unwrap_or(libc::EIO)),
                }
            }
        } else {
            reply.error(libc::ENOENT);
        }
    }
    fn symlink(&mut self, _req: &Request, _parent: u64, link: &OsStr, origin: &Path, reply: ReplyEntry) {
        println!("Symlink {:?} to {}", link, origin.display());

        let config = Config {
            source: PathBuf::from(origin),
        };
        let id = ConfigId::from(self.next_id);
        let config_ref = ConfigRef {
            root: &config,
            id,
        };
        let stat = match config_ref.stat() {
            Ok(stat) => stat,
            Err(e) => {
                reply.error(e.raw_os_error().unwrap_or(libc::EIO));
                return;
            },
        };
        self.configs.insert(self.next_id, config);

        self.paths.insert(PathBuf::from(link), id);

        for &kind in ConversionKind::all() {
            self.paths.insert(Path::new(link).join(kind.file()), id.convert(kind));
        }

        self.next_id = RootConfigId(self.next_id.0 + 1);
        reply.entry(&TTL, &stat, 0);
    }
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        if ino == ROOT_ID {
            reply.attr(&time::now().to_timespec(), &Self::fake_stat_dir(ino));
        } else if let Some(config) = self.ino_to_config(ino) {
            match config.stat() {
                Ok(stat) => reply.attr(&time::now().to_timespec(), &stat),
                Err(err) => reply.error(err.raw_os_error().unwrap_or(libc::EIO)),
            }
        } else {
            println!("Unknown inode: {}", ino);
            dbg!(&self.paths);
            reply.error(libc::ENOENT);
        }
    }
    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        let mut entries = Vec::new();
        let mut i = 2;
        if ino == ROOT_ID {
            entries.push((ROOT_ID, 1, FileType::Directory, Path::new("..")));
            entries.push((ROOT_ID, 2, FileType::Directory, Path::new(".")));
            for (path, &id) in &self.paths {
                if id.kind() != ConversionKind::Root {
                    continue;
                }
                i += 1;
                entries.push((
                    id.inode(),          // inode
                    i,                   // offset
                    FileType::Directory, // kind
                    &path,               // path
                ));
            }
        } else if let Some(id) = ConfigId::from_ino(ino).and_then(|id| id.as_root()) {
            entries.push((ROOT_ID,                    1, FileType::Directory, Path::new("..")));
            entries.push((ConfigId::from(id).inode(), 2, FileType::Directory, Path::new(".")));
            if self.configs.contains_key(&id) {
                for &kind in ConversionKind::all() {
                    i += 1;
                    entries.push((
                        ConfigId::from(id).convert(kind).inode(), // inode
                        i,                                        // offset
                        FileType::RegularFile,                    // kind
                        Path::new(kind.file()),                   // path
                    ));
                }
            }
        }
        for (inode, offset, kind, path) in entries.into_iter().skip(offset as usize) {
            reply.add(inode, offset, kind, path);
        }
        reply.ok();
    }
}
