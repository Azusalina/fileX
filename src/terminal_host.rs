//! Temporarily release Konsole's Ctrl+Shift+N action in the hosting window.
//! A window lease keeps overlapping filex instances from restoring it too early.
use std::{
    collections::BTreeSet,
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
    },
    path::PathBuf,
    process::Command,
};

pub struct TmuxKeys(Option<TmuxLease>);
struct TmuxLease {
    file: PathBuf,
    socket: String,
}
fn tmux(socket: &str, args: &[&str]) -> Option<String> {
    let output = Command::new("tmux")
        .args(["-S", socket])
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
fn tmux_state(file: &mut File, mode: &str, format: &str) -> (String, String, BTreeSet<u32>) {
    let mut text = String::new();
    let _ = file.read_to_string(&mut text);
    let pids = text
        .lines()
        .skip(1)
        .filter_map(|pid| pid.parse::<u32>().ok())
        .filter(|pid| {
            fs::read_link(format!("/proc/{pid}/exe"))
                .ok()
                .is_some_and(|exe| {
                    exe.file_name().is_some_and(|name| {
                        name.to_string_lossy().trim_end_matches(" (deleted)") == "filex"
                    })
                })
        })
        .collect();
    let original = if text.lines().nth(1).is_some() {
        text.lines().next().unwrap_or("").split_once(' ')
    } else {
        None
    };
    let (mode, format) = original.unwrap_or((mode, format));
    (mode.into(), format.into(), pids)
}
fn save_tmux(
    file: &mut File,
    mode: &str,
    format: &str,
    pids: &BTreeSet<u32>,
) -> std::io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    writeln!(file, "{mode} {format}")?;
    for pid in pids {
        writeln!(file, "{pid}")?;
    }
    Ok(())
}
impl TmuxKeys {
    pub fn new(enabled: bool) -> Self {
        Self(if enabled { Self::acquire() } else { None })
    }
    fn acquire() -> Option<TmuxLease> {
        let context = env::var("TMUX").ok()?;
        let socket = context.rsplitn(3, ',').last()?.to_owned();
        // CSI-u preserves Ctrl+Shift; crossterm supports it without a parser fork.
        let mode = tmux(&socket, &["show-options", "-s", "-v", "extended-keys"])?;
        let format = tmux(
            &socket,
            &["show-options", "-s", "-v", "extended-keys-format"],
        )?;
        if !["off", "on", "always"].contains(&mode.as_str())
            || !["xterm", "csi-u"].contains(&format.as_str())
        {
            return None;
        }
        let directory = env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| env::temp_dir().join(format!("filex-{}", unsafe { libc::getuid() })))
            .join("filex-terminal");
        fs::create_dir_all(&directory).ok()?;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).ok()?;
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        socket.hash(&mut hash);
        let path = directory.join(format!("tmux-{:x}.lock", hash.finish()));
        let mut file = lock(&path)?;
        let (original_mode, original_format, mut pids) = tmux_state(&mut file, &mode, &format);
        if mode == "off" {
            tmux(&socket, &["set-option", "-s", "extended-keys", "on"])?;
        }
        if tmux(
            &socket,
            &["set-option", "-s", "extended-keys-format", "csi-u"],
        )
        .is_none()
        {
            if pids.is_empty() {
                let _ = tmux(
                    &socket,
                    &["set-option", "-s", "extended-keys", &original_mode],
                );
            }
            return None;
        }
        let empty = pids.is_empty();
        pids.insert(std::process::id());
        if save_tmux(&mut file, &original_mode, &original_format, &pids).is_err() {
            if empty {
                let _ = tmux(
                    &socket,
                    &["set-option", "-s", "extended-keys", &original_mode],
                );
                let _ = tmux(
                    &socket,
                    &["set-option", "-s", "extended-keys-format", &original_format],
                );
            }
            return None;
        }
        Some(TmuxLease { file: path, socket })
    }
}
impl Drop for TmuxKeys {
    fn drop(&mut self) {
        let Some(lease) = &self.0 else {
            return;
        };
        let Some(mut file) = lock(&lease.file) else {
            return;
        };
        let (mode, format, mut pids) = tmux_state(&mut file, "off", "xterm");
        pids.remove(&std::process::id());
        if pids.is_empty() {
            let _ = tmux(&lease.socket, &["set-option", "-s", "extended-keys", &mode]);
            let _ = tmux(
                &lease.socket,
                &["set-option", "-s", "extended-keys-format", &format],
            );
        }
        let _ = save_tmux(&mut file, &mode, &format, &pids);
    }
}

pub struct KonsoleGuard(Option<Lease>);
struct Lease {
    service: String,
    window: String,
    file: PathBuf,
}
fn remote(service: &str, args: &[&str]) -> Option<String> {
    let result = Command::new("qdbus6")
        .arg(service)
        .args(args)
        .output()
        .ok()?;
    result
        .status
        .success()
        .then(|| String::from_utf8_lossy(&result.stdout).trim().to_owned())
}
fn lock(path: &PathBuf) -> Option<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
        .ok()?;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return None;
    }
    Some(file)
}
fn read(file: &mut File, enabled: bool) -> (bool, BTreeSet<u32>) {
    let mut text = String::new();
    let _ = file.read_to_string(&mut text);
    let pids: BTreeSet<_> = text
        .lines()
        .skip(1)
        .filter_map(|pid| pid.parse::<u32>().ok())
        .filter(|pid| {
            fs::read_link(format!("/proc/{pid}/exe"))
                .ok()
                .is_some_and(|exe| {
                    exe.file_name().is_some_and(|name| {
                        name.to_string_lossy().trim_end_matches(" (deleted)") == "filex"
                    })
                })
        })
        .collect();
    let original = if pids.is_empty() && enabled {
        true
    } else {
        text.lines().next().map_or(enabled, |line| line == "true")
    };
    (original, pids)
}
fn save(file: &mut File, enabled: bool, pids: &BTreeSet<u32>) -> std::io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    writeln!(file, "{enabled}")?;
    for pid in pids {
        writeln!(file, "{pid}")?;
    }
    Ok(())
}
impl KonsoleGuard {
    pub fn new(enabled: bool) -> Self {
        Self(if enabled { Self::acquire() } else { None })
    }
    fn acquire() -> Option<Lease> {
        let service = env::var("KONSOLE_DBUS_SERVICE").ok()?;
        let session = env::var("KONSOLE_DBUS_SESSION").ok()?;
        let id = session
            .strip_prefix("/Sessions/")?
            .parse::<u32>()
            .ok()?
            .to_string();
        let paths = remote(&service, &[])?;
        for path in paths.lines().filter(|path| path.starts_with("/Windows/")) {
            let window_id = path.strip_prefix("/Windows/")?.parse::<u32>().ok()?;
            let sessions = remote(&service, &[path, "org.kde.konsole.Window.sessionList"])?;
            if !sessions.lines().any(|session| session.trim() == id) {
                continue;
            }
            let window = format!("/konsole/MainWindow_{window_id}");
            let active = remote(
                &service,
                &[&window, "org.kde.KMainWindow.actionIsEnabled", "new-window"],
            )?;
            if !["true", "false"].contains(&active.as_str()) {
                return None;
            }
            let directory = env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    env::temp_dir().join(format!("filex-{}", unsafe { libc::getuid() }))
                })
                .join("filex-terminal");
            fs::create_dir_all(&directory).ok()?;
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).ok()?;
            let safe_service: String = service
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || ".-_".contains(c) {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let file_path = directory.join(format!("{safe_service}-{window_id}.lock"));
            let mut file = lock(&file_path)?;
            let (original, mut pids) = read(&mut file, active == "true");
            if active == "true"
                && remote(
                    &service,
                    &[&window, "org.kde.KMainWindow.disableAction", "new-window"],
                )? != "true"
            {
                return None;
            }
            let empty = pids.is_empty();
            pids.insert(std::process::id());
            if save(&mut file, original, &pids).is_err() {
                if original && empty {
                    let _ = remote(
                        &service,
                        &[&window, "org.kde.KMainWindow.enableAction", "new-window"],
                    );
                }
                return None;
            }
            return Some(Lease {
                service,
                window,
                file: file_path,
            });
        }
        None
    }
}
impl Drop for KonsoleGuard {
    fn drop(&mut self) {
        let Some(lease) = &self.0 else {
            return;
        };
        let Some(mut file) = lock(&lease.file) else {
            return;
        };
        let (original, mut pids) = read(&mut file, false);
        pids.remove(&std::process::id());
        if pids.is_empty() && original {
            let _ = remote(
                &lease.service,
                &[
                    &lease.window,
                    "org.kde.KMainWindow.enableAction",
                    "new-window",
                ],
            );
        }
        let _ = save(&mut file, original, &pids);
    }
}

/// Focus reports are leased separately from the keyboard option lease, so old
/// filex instances retain their compatible state format.
pub struct TmuxFocus(Option<TmuxLease>);
impl TmuxFocus {
    pub fn new(enabled: bool) -> Self {
        Self(if enabled { Self::acquire() } else { None })
    }
    fn acquire() -> Option<TmuxLease> {
        use std::hash::{Hash, Hasher};
        let context = env::var("TMUX").ok()?;
        let socket = context.rsplitn(3, ',').last()?.to_owned();
        let current = tmux(&socket, &["show-options", "-s", "-v", "focus-events"])?;
        if !["on", "off"].contains(&current.as_str()) {
            return None;
        }
        let directory = env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| env::temp_dir().join(format!("filex-{}", unsafe { libc::getuid() })))
            .join("filex-terminal");
        fs::create_dir_all(&directory).ok()?;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).ok()?;
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        socket.hash(&mut hash);
        let path = directory.join(format!("tmux-focus-{:x}.lock", hash.finish()));
        let mut file = lock(&path)?;
        let (original, mut pids) = read(&mut file, current == "on");
        if current == "off" {
            tmux(&socket, &["set-option", "-s", "focus-events", "on"])?;
        }
        pids.insert(std::process::id());
        if save(&mut file, original, &pids).is_err() {
            if pids.len() == 1 {
                let _ = tmux(
                    &socket,
                    &[
                        "set-option",
                        "-s",
                        "focus-events",
                        if original { "on" } else { "off" },
                    ],
                );
            }
            return None;
        }
        Some(TmuxLease { file: path, socket })
    }
}
impl Drop for TmuxFocus {
    fn drop(&mut self) {
        let Some(lease) = &self.0 else {
            return;
        };
        let Some(mut file) = lock(&lease.file) else {
            return;
        };
        let (original, mut pids) = read(&mut file, false);
        pids.remove(&std::process::id());
        if pids.is_empty() {
            let _ = tmux(
                &lease.socket,
                &[
                    "set-option",
                    "-s",
                    "focus-events",
                    if original { "on" } else { "off" },
                ],
            );
        }
        let _ = save(&mut file, original, &pids);
    }
}
