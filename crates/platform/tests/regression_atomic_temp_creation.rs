use craft_anything_platform::fs::write_atomic;
#[cfg(unix)]
use craft_anything_platform::fs::write_atomic_private;
use std::path::{Path, PathBuf};

fn temporary(dir: &Path, name: &str, counter: u64) -> PathBuf {
    dir.join(format!(".{name}.tmp-{}-{counter}", std::process::id()))
}

#[test]
fn atomic_writes_preserve_colliding_files_and_links() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("settings.json");
    let stale: Vec<_> = (0..4).map(|n| temporary(dir.path(), "settings.json", n)).collect();
    for file in &stale {
        std::fs::write(file, b"existing temporary data").unwrap();
    }
    write_atomic(&target, b"new settings").unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"new settings");
    for file in &stale {
        assert_eq!(std::fs::read(file).unwrap(), b"existing temporary data");
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let other = dir.path().join("other.txt");
        std::fs::write(&other, b"keep this file").unwrap();
        let links: Vec<_> = (0..16).map(|n| temporary(dir.path(), "private.json", n)).collect();
        for link in &links {
            symlink(&other, link).unwrap();
        }
        let private = dir.path().join("private.json");
        write_atomic_private(&private, b"private fixture").unwrap();
        assert_eq!(std::fs::read(&other).unwrap(), b"keep this file");
        assert_eq!(std::fs::read(&private).unwrap(), b"private fixture");
        assert_eq!(std::fs::metadata(&private).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(links.iter().all(|link| std::fs::symlink_metadata(link).unwrap().file_type().is_symlink()));

        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
        write_atomic(&target, b"replacement settings").unwrap();
        assert_eq!(std::fs::metadata(&target).unwrap().permissions().mode() & 0o777, 0o640);
    }

    // A bounded retry must leave both the destination and every colliding file alone.
    let destination = dir.path().join("crowded.json");
    std::fs::write(&destination, b"original settings").unwrap();
    let occupied: Vec<_> = (0..256).map(|n| temporary(dir.path(), "crowded.json", n)).collect();
    for file in &occupied {
        std::fs::write(file, b"someone else's temporary data").unwrap();
    }
    assert!(write_atomic(&destination, b"replacement").is_err());
    assert_eq!(std::fs::read(&destination).unwrap(), b"original settings");
    for file in &occupied {
        assert_eq!(std::fs::read(file).unwrap(), b"someone else's temporary data");
    }

    // An error after creating our temporary file cleans up only that new file.
    let blocked = dir.path().join("directory.json");
    std::fs::create_dir(&blocked).unwrap();
    let mut before: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|entry| entry.unwrap().path()).collect();
    before.sort();
    assert!(write_atomic(&blocked, b"cannot replace a directory").is_err());
    let mut after: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|entry| entry.unwrap().path()).collect();
    after.sort();
    assert_eq!(after, before);
}
