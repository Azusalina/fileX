//! Bounded, read-only, asynchronous Git snapshots. Never fetches or writes index.
use std::{
    collections::HashMap,
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
static READY: AtomicBool = AtomicBool::new(false);
pub fn ready() -> bool {
    READY.swap(false, Ordering::Relaxed)
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    #[default]
    None,
    Committed,
    Synced,
    Ahead,
    Behind,
    Diverged,
    Untracked,
    Staged,
    Modified,
    Conflict,
}
impl State {
    pub fn marker(self, ascii: bool) -> &'static str {
        match self {
            Self::None => "",
            Self::Committed => "C",
            Self::Synced => "P",
            Self::Ahead => {
                if ascii {
                    "^"
                } else {
                    "↑"
                }
            }
            Self::Behind => {
                if ascii {
                    "v"
                } else {
                    "↓"
                }
            }
            Self::Diverged => {
                if ascii {
                    "D"
                } else {
                    "↕"
                }
            }
            Self::Untracked => "?",
            Self::Staged => "S",
            Self::Modified => "M",
            Self::Conflict => "!",
        }
    }
    pub fn color(self) -> &'static str {
        match self {
            Self::Conflict => "#FF5555",
            Self::Modified => "#FFFF55",
            Self::Staged => "#55FF55",
            Self::Untracked => "#FF55FF",
            Self::Ahead => "#5555FF",
            Self::Behind | Self::Diverged => "#FF55FF",
            Self::Committed => "#AAAAAA",
            _ => "#55FFFF",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Committed => "tracked / clean; no upstream",
            Self::Synced => "clean / matches cached upstream",
            Self::Ahead => "local commits ahead of cached upstream",
            Self::Behind => "behind cached upstream",
            Self::Diverged => "diverged from cached upstream",
            Self::Untracked => "untracked",
            Self::Staged => "staged / awaiting commit",
            Self::Modified => "modified / unstaged",
            Self::Conflict => "merge conflict",
        }
    }
}
struct Snapshot {
    states: HashMap<PathBuf, State>,
}
impl Snapshot {
    fn insert(&mut self, root: &Path, path: PathBuf, state: State, limit: usize) {
        let mut current = Some(path.as_path());
        while let Some(p) = current {
            if self.states.len() >= limit && !self.states.contains_key(p) {
                break;
            }
            self.states
                .entry(p.to_owned())
                .and_modify(|old| *old = (*old).max(state))
                .or_insert(state);
            if p == root {
                break;
            }
            current = p.parent().filter(|p| p.starts_with(root));
        }
    }
    fn state(&self, path: &Path) -> State {
        self.states.get(path).copied().unwrap_or(State::None)
    }
}
struct Entry {
    pending: Option<mpsc::Receiver<Option<Snapshot>>>,
    snapshot: Option<Snapshot>,
}
#[derive(Default)]
pub struct Cache {
    entries: HashMap<PathBuf, Entry>,
}
impl Cache {
    pub fn clear(&mut self) {
        self.entries.retain(|_, e| e.pending.is_some());
        for e in self.entries.values_mut() {
            e.snapshot = None;
        }
    }
    pub fn request(&mut self, path: &Path, limit: usize) {
        let Some(root) = path
            .ancestors()
            .find(|p| p.join(".git").exists())
            .map(Path::to_owned)
        else {
            return;
        };
        if self.entries.contains_key(&root) || self.entries.len() >= 4 {
            return;
        }
        let (send, receive) = mpsc::channel();
        let folder = root.clone();
        self.entries.insert(
            root,
            Entry {
                pending: Some(receive),
                snapshot: None,
            },
        );
        thread::spawn(move || {
            let result = scan(&folder, limit);
            let _ = send.send(result);
            READY.store(true, Ordering::Relaxed);
        });
    }
    pub fn poll(&mut self) {
        for entry in self.entries.values_mut() {
            if let Some(receiver) = &entry.pending {
                match receiver.try_recv() {
                    Ok(snapshot) => {
                        entry.snapshot = snapshot;
                        entry.pending = None
                    }
                    Err(mpsc::TryRecvError::Disconnected) => entry.pending = None,
                    _ => {}
                }
            }
        }
    }
    pub fn state(&self, path: &Path) -> State {
        self.entries
            .iter()
            .filter(|(root, _)| path.starts_with(root))
            .max_by_key(|(root, _)| root.components().count())
            .and_then(|(_, e)| e.snapshot.as_ref())
            .map_or(State::None, |s| s.state(path))
    }
}
fn command(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let mut child = Command::new("git")
        .args([
            "--no-optional-locks",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
            "-C",
        ])
        .arg(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
        .ok()?;
    let pipe = child.stdout.take()?;
    let (send, receive) = mpsc::channel();
    thread::spawn(move || {
        let mut data = Vec::new();
        let result = pipe.take(512 * 1024 + 1).read_to_end(&mut data);
        let _ = send.send((result, data));
    });
    let deadline = Instant::now() + Duration::from_millis(1500);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let (read, data) = receive.recv_timeout(Duration::from_millis(200)).ok()?;
                return (status.success() && read.is_ok() && data.len() <= 512 * 1024)
                    .then_some(data);
            }
            Ok(None) => {}
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        thread::sleep(Duration::from_millis(10));
    }
}
fn path(bytes: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        PathBuf::from(OsString::from_vec(bytes.to_vec()))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(OsString::from(String::from_utf8_lossy(bytes).into_owned()))
    }
}
fn scan(root: &Path, limit: usize) -> Option<Snapshot> {
    let status = command(
        root,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=dirty",
        ],
    )?;
    let records: Vec<_> = status.split(|b| *b == 0).collect();
    let upstream = records.iter().any(|r| r.starts_with(b"# branch.upstream "));
    let mut ahead = 0;
    let mut behind = 0;
    for r in &records {
        if r.starts_with(b"# branch.ab ") {
            let s = String::from_utf8_lossy(r);
            let values: Vec<_> = s.split_whitespace().collect();
            ahead = values
                .get(2)
                .and_then(|n| n.trim_start_matches('+').parse::<u64>().ok())
                .unwrap_or(0);
            behind = values
                .get(3)
                .and_then(|n| n.trim_start_matches('-').parse::<u64>().ok())
                .unwrap_or(0);
        }
    }
    let unborn = records
        .iter()
        .any(|r| r.starts_with(b"# branch.oid (initial)"));
    let branch = if unborn {
        State::None
    } else {
        match (ahead > 0, behind > 0, upstream) {
            (true, true, _) => State::Diverged,
            (true, false, _) => State::Ahead,
            (false, true, _) => State::Behind,
            (_, _, true) => State::Synced,
            _ => State::Committed,
        }
    };
    let tracked = command(root, &["ls-files", "--cached", "-z"])?;
    let changed = if upstream {
        Some(command(
            root,
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--name-only",
                "-z",
                "@{upstream}",
                "HEAD",
                "--",
            ],
        )?)
    } else {
        None
    };
    let changed: std::collections::HashSet<_> = changed
        .as_deref()
        .unwrap_or_default()
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(path)
        .collect();
    let mut snapshot = Snapshot {
        states: HashMap::new(),
    };
    snapshot.states.insert(root.to_owned(), branch);
    for name in tracked
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .take(limit)
    {
        let name = path(name);
        let state = if upstream && !changed.contains(&name) {
            State::Synced
        } else {
            branch
        };
        snapshot.insert(root, root.join(name), state, limit);
    }
    if upstream {
        for name in &changed {
            snapshot.insert(root, root.join(name), branch, limit);
        }
    }
    snapshot.states.insert(root.to_owned(), branch);
    let mut i = 0;
    while i < records.len() {
        let r = records[i];
        i += 1;
        if r.starts_with(b"? ") {
            let p = root.join(path(&r[2..]));
            snapshot.insert(root, p, State::Untracked, limit);
            continue;
        }
        let fields: Vec<_> = match r.first() {
            Some(b'1') => r.splitn(9, |b| *b == b' ').collect(),
            Some(b'2') => {
                i += 1;
                r.splitn(10, |b| *b == b' ').collect()
            }
            Some(b'u') => r.splitn(11, |b| *b == b' ').collect(),
            _ => continue,
        };
        let Some(name) = fields.last() else { continue };
        let xy = fields.get(1).copied().unwrap_or(b"..");
        let state = if r.first() == Some(&b'u') {
            State::Conflict
        } else if xy.get(1).is_some_and(|b| *b != b'.') {
            State::Modified
        } else {
            State::Staged
        };
        snapshot.insert(root, root.join(path(name)), state, limit);
    }
    Some(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "filex-git-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&p).unwrap();
            Self(std::fs::canonicalize(p).unwrap())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
                "-C",
            ])
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[test]
    fn local_states_rename_unicode_and_directory_aggregation() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let f = Fixture::new();
        git(&f.0, &["init", "-q"]);
        assert_eq!(scan(&f.0, 4000).unwrap().state(&f.0), State::None);
        std::fs::create_dir(f.0.join("folder")).unwrap();
        std::fs::write(f.0.join(".gitignore"), "folder/ignored.txt\n").unwrap();
        for name in ["clean.txt", "modified.txt", "rename me 中文.txt"] {
            std::fs::write(f.0.join(name), "initial").unwrap();
        }
        git(&f.0, &["add", "."]);
        git(&f.0, &["commit", "-qm", "initial"]);
        std::fs::write(f.0.join("modified.txt"), "changed").unwrap();
        std::fs::write(f.0.join("staged.txt"), "staged").unwrap();
        git(&f.0, &["add", "staged.txt"]);
        git(&f.0, &["mv", "rename me 中文.txt", "renamed 中文.txt"]);
        std::fs::write(f.0.join("folder/untracked.txt"), "new").unwrap();
        std::fs::write(f.0.join("folder/ignored.txt"), "ignored").unwrap();
        let index = std::fs::read(f.0.join(".git/index")).unwrap();
        let s = scan(&f.0, 4000).unwrap();
        assert_eq!(std::fs::read(f.0.join(".git/index")).unwrap(), index);
        assert_eq!(s.state(&f.0.join("clean.txt")), State::Committed);
        assert_eq!(s.state(&f.0.join("modified.txt")), State::Modified);
        assert_eq!(s.state(&f.0.join("staged.txt")), State::Staged);
        assert_eq!(s.state(&f.0.join("renamed 中文.txt")), State::Staged);
        assert_eq!(s.state(&f.0.join("folder/untracked.txt")), State::Untracked);
        assert_eq!(s.state(&f.0.join("folder/ignored.txt")), State::None);
        assert_eq!(s.state(&f.0), State::Modified);
        assert!(scan(&f.0, 2).unwrap().states.len() <= 2);
    }
    #[test]
    fn upstream_is_cached_local_state_and_ahead_only_marks_changed_files() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let f = Fixture::new();
        git(&f.0, &["init", "-q", "-b", "main"]);
        std::fs::write(f.0.join("stable"), "same").unwrap();
        std::fs::write(f.0.join("feature"), "old").unwrap();
        git(&f.0, &["add", "."]);
        git(&f.0, &["commit", "-qm", "initial"]);
        git(
            &f.0,
            &[
                "remote",
                "add",
                "origin",
                "https://example.invalid/never-contact",
            ],
        );
        git(&f.0, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        git(&f.0, &["branch", "--set-upstream-to=origin/main", "main"]);
        assert_eq!(
            scan(&f.0, 4000).unwrap().state(&f.0.join("feature")),
            State::Synced
        );
        std::fs::write(f.0.join("feature"), "local").unwrap();
        git(&f.0, &["add", "feature"]);
        git(&f.0, &["commit", "-qm", "local"]);
        let s = scan(&f.0, 4000).unwrap();
        assert_eq!(s.state(&f.0), State::Ahead);
        assert_eq!(s.state(&f.0.join("feature")), State::Ahead);
        assert_eq!(s.state(&f.0.join("stable")), State::Synced);
        git(&f.0, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        git(&f.0, &["reset", "--hard", "HEAD~1"]);
        assert_eq!(
            scan(&f.0, 4000).unwrap().state(&f.0.join("feature")),
            State::Behind
        );
        std::fs::write(f.0.join("feature"), "different").unwrap();
        git(&f.0, &["add", "feature"]);
        git(&f.0, &["commit", "-qm", "diverge"]);
        assert_eq!(
            scan(&f.0, 4000).unwrap().state(&f.0.join("feature")),
            State::Diverged
        );
    }
}
