//! Preflight every destination before the first file operation.
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

fn identity(path: &Path) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        fs::symlink_metadata(path).ok().map(|m| (m.dev(), m.ino()))
    }
    #[cfg(windows)]
    {
        crate::platform::identity(path)
    }
}
fn regular(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| {
        #[cfg(windows)]
        {
            m.is_file() && !crate::platform::reparse(&m)
        }
        #[cfg(not(windows))]
        {
            m.is_file()
        }
    })
}
pub fn validate_target(source: &Path, dest: &Path) -> Result<bool, String> {
    if fs::symlink_metadata(dest).is_err() {
        crate::files::validate(source, dest)?;
        return Ok(false);
    }
    if !regular(source) || !regular(dest) {
        return Err(format!(
            "Only regular files can be replaced (no folders or links): {}",
            dest.display()
        ));
    }
    if fs::canonicalize(source).map_err(|e| e.to_string())?
        == fs::canonicalize(dest).map_err(|e| e.to_string())?
        || identity(source).is_some_and(|id| Some(id) == identity(dest))
    {
        return Err("Source and destination are the same file (or hard links)".into());
    }
    Ok(true)
}
pub fn plan(sources: &[PathBuf], directory: &Path) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    if !directory.is_dir() {
        return Err("Destination is not a directory".into());
    }
    let mut names = HashSet::new();
    let mut plan = Vec::new();
    for source in sources {
        let name = source
            .file_name()
            .ok_or("Cannot transfer a filesystem root")?;
        let key = if cfg!(windows) {
            name.to_string_lossy().to_lowercase()
        } else {
            name.to_string_lossy().into_owned()
        };
        if !names.insert(key) {
            return Err("Selected items have duplicate destination names".into());
        }
        let destination = directory.join(name);
        validate_target(source, &destination)?;
        plan.push((source.clone(), destination));
    }
    Ok(plan)
}
pub fn plan_to(sources: &[PathBuf], target: &Path) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    if target.is_dir() {
        return plan(sources, target);
    }
    if sources.len() != 1 {
        return Err("Multiple items require a destination directory".into());
    }
    let source = sources.first().ok_or("No pending items")?;
    validate_target(source, target)?;
    Ok(vec![(source.clone(), target.to_path_buf())])
}
// Copy completely beside the destination, then atomically replace its directory entry.
// Never truncate the existing file or follow a destination link.
pub fn execute(source: &Path, dest: &Path, moving: bool, replace: bool) -> Result<(), String> {
    if !replace {
        return crate::files::transfer(source, dest, moving);
    }
    if !validate_target(source, dest)? {
        return Err("Replacement target changed; choose destination again".into());
    }
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let parent = dest.parent().ok_or("Invalid destination parent")?;
    let stage = loop {
        let candidate = parent.join(format!(
            ".filex-transfer-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&candidate) {
            Ok(()) => break candidate,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    };
    let payload = stage.join("payload");
    let result = (|| {
        crate::files::transfer(source, &payload, false)?;
        validate_target(source, dest)?;
        crate::files::replace_staged(&payload, dest)?;
        if moving {
            fs::remove_file(source)
                .map_err(|e| format!("Target replaced; source could not be removed: {e}"))?;
        }
        Ok(())
    })();
    let _ = fs::remove_file(&payload);
    let _ = fs::remove_dir(&stage);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let p = std::env::temp_dir().join(format!(
                "filex-replace-test-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn replace_stages_copy_and_move_without_leaving_temp_files() {
        let f = Fixture::new();
        let source = f.0.join("source");
        let dest = f.0.join("dest");
        fs::write(&source, "new content").unwrap();
        fs::write(&dest, "old content").unwrap();
        assert!(validate_target(&source, &dest).unwrap());
        assert!(execute(&source, &dest, false, false).is_err());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "old content");
        execute(&source, &dest, false, true).unwrap();
        assert_eq!(fs::read_to_string(&dest).unwrap(), "new content");
        assert!(source.exists());
        fs::write(&source, "moved").unwrap();
        execute(&source, &dest, true, true).unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "moved");
        assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
    }
    #[test]
    fn replacement_rejects_self_hardlinks_folders_and_symlinks() {
        let f = Fixture::new();
        let a = f.0.join("a");
        let b = f.0.join("b");
        let dir = f.0.join("dir");
        fs::write(&a, "safe").unwrap();
        fs::hard_link(&a, &b).unwrap();
        fs::create_dir(&dir).unwrap();
        assert!(validate_target(&a, &a).is_err());
        assert!(validate_target(&a, &b).is_err());
        assert!(validate_target(&a, &dir).is_err());
        assert!(validate_target(&dir, &a).is_err());
        #[cfg(unix)]
        {
            let link = f.0.join("link");
            std::os::unix::fs::symlink(&a, &link).unwrap();
            assert!(validate_target(&a, &link).is_err());
            assert!(validate_target(&link, &a).is_err());
        }
        assert_eq!(fs::read_to_string(&a).unwrap(), "safe");
    }
    #[test]
    fn typed_destination_supports_renaming_and_requires_directory_for_batch() {
        let f = Fixture::new();
        let a = f.0.join("a");
        let b = f.0.join("b");
        let dir = f.0.join("dir");
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        fs::create_dir(&dir).unwrap();
        let renamed = f.0.join("renamed");
        assert_eq!(
            plan_to(&[a.clone()], &renamed).unwrap(),
            vec![(a.clone(), renamed.clone())]
        );
        assert!(plan_to(&[a.clone(), b.clone()], &renamed).is_err());
        assert_eq!(plan_to(&[a.clone(), b], &dir).unwrap().len(), 2);
        let duplicate = dir.join("a");
        fs::write(&duplicate, "duplicate").unwrap();
        assert!(plan_to(&[a, duplicate], &f.0).is_err());
    }
}
