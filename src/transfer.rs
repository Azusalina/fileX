//! Preflight every destination before the first file operation.
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
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
        crate::files::validate(source, &destination)?;
        plan.push((source.clone(), destination));
    }
    Ok(plan)
}
