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
