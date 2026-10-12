//! Temporarily use Fcitx5's English keyboard, without changing its profile.
use std::process::{Command, Stdio};

pub struct EnglishGuard(Option<String>);
impl EnglishGuard {
    pub fn new(enabled: bool) -> Self {
        let previous = enabled
            .then(|| {
                let output = Command::new("fcitx5-remote")
                    .args(["--check", "-n"])
                    .stderr(Stdio::null())
                    .output()
                    .ok()?;
                if !output.status.success() {
                    return None;
                }
                let name = String::from_utf8(output.stdout).ok()?.trim().to_owned();
                if name.is_empty() {
                    return None;
                }
                if name == "keyboard-us" {
                    return Some(name);
                }
                let switched = Command::new("fcitx5-remote")
                    .args(["--check", "-s", "keyboard-us"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .ok()?;
                switched.success().then_some(name)
            })
            .flatten();
        Self(previous)
    }
}
impl Drop for EnglishGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.0 {
            let _ = Command::new("fcitx5-remote")
                .args(["--check", "-s", previous])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

/// Reassert English when the terminal/pane regains focus, keeping the original
/// method in EnglishGuard for final restoration.
pub fn ensure_english(enabled: bool) {
    if !enabled {
        return;
    }
    let Ok(mut child) = Command::new("fcitx5-remote")
        .args(["--check", "-s", "keyboard-us"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
