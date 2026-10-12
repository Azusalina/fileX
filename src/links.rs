use crate::{config::Config, model::clean};
use std::{
    fs::{self, Metadata},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

pub struct PeerScan {
    pub paths: Vec<PathBuf>,
    pub scanned: usize,
    pub skipped: usize,
    pub limited: bool,
}

/// Stream directory iterators rather than retaining a filesystem-wide inode index.
pub fn peers(
    source: &Path,
    root: &Path,
    info: &Metadata,
    limit: usize,
    results: usize,
) -> PeerScan {
    let mut found = PeerScan {
        paths: vec![],
        scanned: 0,
        skipped: 0,
        limited: false,
    };
    if info.is_dir() || info.nlink() <= 1 {
        return found;
    }
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => {
            found.skipped = 1;
            return found;
        }
    };
    let mut stack = vec![(entries, 0)];
    while let Some((entries, depth)) = stack.last_mut() {
        if found.scanned >= limit {
            found.limited = true;
            break;
        }
        let depth = *depth;
        let item = match entries.next() {
            Some(Ok(item)) => item,
            Some(Err(_)) => {
                found.scanned += 1;
                found.skipped += 1;
                continue;
            }
            None => {
                stack.pop();
                continue;
            }
        };
        found.scanned += 1;
        let path = item.path();
        let stat = match fs::symlink_metadata(&path) {
            Ok(stat) => stat,
            Err(_) => {
                found.skipped += 1;
                continue;
            }
        };
        if stat.dev() == info.dev() && stat.ino() == info.ino() && path != source {
            found.paths.push(path.clone());
            if found.paths.len() as u64 >= info.nlink() - 1 {
                break;
            }
            if found.paths.len() >= results {
                found.limited = true;
                break;
            }
        }
        // Do not follow symlinks, including links to ancestor directories.
        if stat.is_dir() {
            if depth >= 128 {
                found.skipped += 1;
                continue;
            }
            match fs::read_dir(&path) {
                Ok(entries) => stack.push((entries, depth + 1)),
                Err(_) => found.skipped += 1,
            }
        }
    }
    found.paths.sort();
    found
}

pub fn details(path: &Path, root: &Path, info: &Metadata, conf: &Config) -> String {
    let mut text = format!(
        "Links / 链接数: {}\nDevice / Inode: {} / {}\n",
        info.nlink(),
        info.dev(),
        info.ino()
    );
    if info.file_type().is_symlink() {
        match fs::read_link(path) {
            Ok(target) => {
                let absolute = if target.is_absolute() {
                    target.clone()
                } else {
                    path.parent().unwrap_or(Path::new(".")).join(&target)
                };
                let state = match path.try_exists() {
                    Ok(true) => "有效".to_owned(),
                    Ok(false) => "断开".to_owned(),
                    Err(error) => format!("无法访问: {error}"),
                };
                text.push_str(&format!(
                    "Soft link / 软链接 → {}\nTarget / 目标路径: {}\n状态: {state}\n",
                    clean(&target.to_string_lossy()),
                    clean(&absolute.to_string_lossy())
                ));
            }
            Err(error) => text.push_str(&format!("软链接目标无法读取: {error}\n")),
        }
    }
    if info.is_dir() {
        text.push_str("目录链接数含 . / 子目录 ..，不表示文件硬链接\n");
    } else if info.nlink() > 1 {
        let found = peers(path, root, info, conf.link_scan_limit, conf.link_results);
        text.push_str(&format!(
            "Hard links / 硬链接: {} 个名称共享此 inode\n其他路径（当前根目录内）: {}\n",
            info.nlink(),
            found.paths.len()
        ));
        for peer in &found.paths {
            text.push_str(&format!("{}\n", clean(&peer.to_string_lossy())));
        }
        if found.paths.is_empty() {
            text.push_str("未找到其他路径；可能位于根目录外或不可读\n");
        }
        text.push_str(&format!(
            "扫描 {} 项，跳过 {} 处{}\n",
            found.scanned,
            found.skipped,
            if found.limited {
                "，达到扫描 / 结果上限"
            } else {
                ""
            }
        ));
    }
    text
}
