extern crate dotlock;

use dotlock::*;
use std::fs::metadata;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

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
    assert!(exists(lockfile));

    let lock2 = DotlockOptions::new().tries(1).stale_age(Duration::from_secs(1)).create(lockfile);
    assert!(lock2.is_err());
    assert!(exists(lockfile));

    sleep(Duration::from_millis(1100));

    let lock3 = DotlockOptions::new().tries(1).stale_age(Duration::from_secs(1)).create(lockfile);
    assert!(lock3.is_ok());
    assert!(exists(lockfile));
}
