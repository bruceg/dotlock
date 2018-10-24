//! Create ".lock" files atomically on any filesystem.
//!
//! This crate contains support for creating lock files as are used on
//! FIXME. This is similar to the `lockfile` program from
//! [procmail](http://www.procmail.org) or the `dotlockfile` program
//! from [liblockfile](https://github.com/miquels/liblockfile).
//!
//! They are called ".lock" files, because they are traditionally named
//! the same as the file they are referencing with the extension of
//! `.lock`.
//!
//! The algorithm that is used to create a lock file in an atomic way is
//! as follows:
//!
//! 1. A unique file is created using
//! [`tempfile`](https://docs.rs/tempfile).
//!
//! 2. The destination lock file is created using the `link` system
//! call. This operation is atomic across all filesystems including
//! NFS. The result of this operation is ignored, as success is based on
//! subsequent results.
//!
//! 3. Delete the temporary file.
//!
//! 4. The metadata of the destination is retrieved. If this fails,
//! repeat the process.
//!
//! 5. The metadata of the temporary file and the destination lock file
//! are compared. If they are the same file, then we have successfully
//! locked the file. Return the opened file.
//!
//! 6. If the lock file is stale (older than a configured age), delete
//! the existing lock file.
//!
//! 7. Before retrying, sleep briefly (defaults to 5 seconds).

extern crate tempfile;

use std::fs::{remove_file, File, Metadata, Permissions};
use std::io::{Error, ErrorKind, Read, Result, Seek, SeekFrom, Write};
use std::os::linux::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use tempfile::Builder;

const DEFAULT_PAUSE: Duration = Duration::from_secs(5);
const DEFAULT_TRIES: usize = 10;

// Do the two Metadata reference the same file?
fn meta_eq(a: &Metadata, b: &Metadata) -> bool {
    a.st_dev() == b.st_dev() && a.st_ino() == b.st_ino()
}

/// A created ".lock" file.
#[derive(Debug)]
pub struct Dotlock {
    file: File,
    path: Option<PathBuf>,
}

impl Dotlock {
    fn create_in(path: &Path, options: DotlockOptions, tempdir: &Path) -> Result<File> {
        for trynum in 0..options.tries {
            if trynum > 0 {
                // Pause only before retrying
                sleep(options.pause);
            }
            // Create a unique temporary file in the same directory
            let temp = Builder::new().tempfile_in(tempdir)?;
            let tempmeta = temp.as_file().metadata()?;
            // link temporary file to destination, ignore the result
            std::fs::hard_link(temp.path(), &path).ok();
            // Drop the temporary file
            let temp = temp.into_file();
            // stat the destination lock file
            let destmeta = match std::fs::metadata(&path) {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            // Compare result of stat to temporary file
            if meta_eq(&destmeta, &tempmeta) {
                if let Some(perm) = options.permissions {
                    temp.set_permissions(perm)?;
                }
                return Ok(temp);
            }
            // Is the existing lock stale?
            if let Some(stale_age) = options.stale_age {
                let now = SystemTime::now();
                if let Ok(modtime) = destmeta.modified() {
                    if let Ok(age) = now.duration_since(modtime) {
                        if age < stale_age {
                            remove_file(&path).ok();
                        }
                    }
                }
            }
        }
        Err(Error::new(ErrorKind::TimedOut, "Timed out"))
    }

    fn create_with(path: PathBuf, options: DotlockOptions) -> Result<Self> {
        let file = Self::create_in(&path, options, &path.parent().unwrap_or(Path::new(".")))?;
        Ok(Self {
            file,
            path: Some(path),
        })
    }

    /// Attempts to create the named lock file using the default options.
    pub fn create<T: Into<PathBuf>>(path: T) -> Result<Self> {
        DotlockOptions::new().create(path.into())
    }

    /// Unlocks the lock by removing the file. The lock will be
    /// automatically removed when this `Dotlock` is dropped.
    pub fn unlock(&mut self) -> Result<()> {
        self.path.take().map_or(Ok(()), |path| remove_file(path))
    }

    /// Attempts to sync all OS-internal metadata to disk. Calls
    /// [`File::sync_all`](https://doc.rust-lang.org/std/fs/struct.File.html#method.sync_all).
    pub fn sync_all(&self) -> Result<()> {
        self.file.sync_all()
    }

    /// Attempts to sync all OS-internal data to disk except
    /// metadata. Calls
    /// [`File::sync_data`](https://doc.rust-lang.org/std/fs/struct.File.html#method.sync_data).
    pub fn sync_data(&self) -> Result<()> {
        self.file.sync_all()
    }

    /// Truncates or extends the underlying file, updating the size of
    /// this file to become `size`. Calls
    /// [`File::set_len`](https://doc.rust-lang.org/std/fs/struct.File.html#method.set_len).
    pub fn set_len(&self, size: u64) -> Result<()> {
        self.file.set_len(size)
    }

    /// Queries metadata about the underlying file. Calls
    /// [`File::metadata`](https://doc.rust-lang.org/std/fs/struct.File.html#method.metadata).
    pub fn metadata(&self) -> Result<Metadata> {
        self.file.metadata()
    }

    /// Changes the permissions on the underlying file. Calls
    /// [`File::set_permissions`](https://doc.rust-lang.org/std/fs/struct.File.html#method.set_permissions).
    pub fn set_permissions(&self, perm: Permissions) -> Result<()> {
        self.file.set_permissions(perm)
    }
}

impl Drop for Dotlock {
    fn drop(&mut self) {
        self.unlock().ok();
    }
}

impl Read for Dotlock {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.file.read(buf)
    }
}

impl Seek for Dotlock {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        self.file.seek(pos)
    }
}

impl Write for Dotlock {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.file.write(buf)
    }
    fn flush(&mut self) -> Result<()> {
        self.file.flush()
    }
}

/// Options which can be used to configure how a lock file is created.
///
/// This builder exposes the ability to configure how a lock file is
/// created. The [`Dotlock::create`] method is an alias for the
/// [`create`] method here.
///
/// To use `DotlockOptions`, first call [`new`], then chain calls to
/// methods to set each option required, and finally call [`create`]
/// with the full path of the lock file to create. This will give you a
/// `io::Result` with a [`Dotlock`] inside.
///
/// [`new`]: struct.DotlockOptions.html#method.new
/// [`create`]: struct.DotlockOptions.html#method.create
/// [`Dotlock`]: struct.Dotlock.html
/// [`Dotlock::create`]: struct.Dotlock.html#method.create
///
/// # Examples
///
/// Create a lock file using the defaults:
///
/// ```no_run
/// use dotlock::DotlockOptions;
/// DotlockOptions::new().create("database.lock").unwrap();
/// ```
///
/// Create a lock file, but failing immediately if creating it fails,
/// and remove lock files older than 5 minutes.
///
/// ```no_run
/// use dotlock::DotlockOptions;
/// DotlockOptions::new()
///                .tries(1)
///                .stale_age(std::time::Duration::from_secs(300))
///                .create("database.lock").unwrap();
/// ```
#[derive(Debug)]
pub struct DotlockOptions {
    pause: Duration,
    tries: usize,
    permissions: Option<Permissions>,
    stale_age: Option<Duration>,
}

impl DotlockOptions {
    /// Create a new set of options.
    pub fn new() -> Self {
        Self {
            pause: DEFAULT_PAUSE,
            tries: DEFAULT_TRIES,
            permissions: None,
            stale_age: None,
        }
    }

    /// Set the time `Dotlock` will pause between attempts to create the
    /// lock file. Defaults to 5 seconds.
    pub fn pause<T: Into<Duration>>(mut self, pause: T) -> Self {
        self.pause = pause.into();
        self
    }

    /// Set the number of times `Dotlock` will try to create the lock
    /// file. Defaults to 10 times.
    pub fn tries(mut self, tries: usize) -> Self {
        self.tries = tries.max(1);
        self
    }

    /// Set the permissions on the newly created lock file. If not set,
    /// the lock file permissions will be based on the current umask.
    pub fn permissions(mut self, perm: Permissions) -> Self {
        self.permissions = Some(perm);
        self
    }

    /// Set the age at which a lock file is considered stale. If not
    /// set, the existing file age will not be considered for staleness.
    pub fn stale_age<T: Into<Duration>>(mut self, age: T) -> Self {
        self.stale_age = Some(age.into());
        self
    }

    /// Create the lock file at `path` with the options in `self`.
    pub fn create<T: Into<PathBuf>>(self, path: T) -> Result<Dotlock> {
        Dotlock::create_with(path.into(), self)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::metadata;
    use std::thread::sleep;
    use std::time::Duration;
    use super::*;

    fn exists<T: AsRef<Path>>(path: T) -> bool {
        metadata(path.as_ref()).is_ok()
    }

    #[test]
    fn create_drop() {
        let lockfile = "dotlock-create-drop.lock";
        {
            let lock = DotlockOptions::new().tries(1).create(lockfile);
            assert!(lock.is_ok());
            assert!(exists(lockfile));

            let lock2 = DotlockOptions::new().tries(1).create(lockfile);
            assert!(lock2.is_err());
            assert!(exists(lockfile));

            let lock3 = DotlockOptions::new().tries(10).pause(Duration::from_millis(1)).create(lockfile);
            assert!(lock3.is_err());
            assert!(exists(lockfile));
            // Drop the lock
        }
        assert!(!exists(lockfile));
    }

    #[test]
    fn remove_stale() {
        let lockfile = "dotlock-remove-stale.lock";

        let lock1 = Dotlock::create(lockfile);
        assert!(lock1.is_ok());
        assert!(metadata(lockfile).is_ok());

        let lock2 = DotlockOptions::new().tries(1).stale_age(Duration::from_secs(1)).create(lockfile);
        assert!(lock2.is_err());

        sleep(Duration::from_millis(1100));

        let lock3 = DotlockOptions::new().stale_age(Duration::from_secs(1)).create(lockfile);
        assert!(lock3.is_ok());
    }
}
