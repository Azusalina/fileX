//! Explicit folder commands, detached from the terminal with per-run logs.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
pub struct Started {
    pub pid: u32,
    pub log: PathBuf,
}
fn log_directory() -> Result<PathBuf, String> {
    #[cfg(unix)]
    {
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")))
            .ok_or("Cannot find state directory (HOME/XDG_STATE_HOME)")?;
        Ok(state.join("filex/jobs"))
    }
    #[cfg(windows)]
    {
        let state = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        Ok(state.join("win-filex/jobs"))
    }
}
pub fn start(directory: &Path, text: &str) -> Result<Started, String> {
    spawn_in(directory, text, &log_directory()?)
}
/// An interactive shell in the selected folder, attached to the caller's terminal.
pub fn foreground_command(directory: &Path, text: &str) -> Result<Command, String> {
    if !directory.is_dir() {
        return Err("Select an existing folder to run a command".into());
    }
    if text.trim().is_empty() {
        return Err("Enter a command".into());
    }
    #[cfg(unix)]
    let mut command = {
        let shell = std::env::var_os("SHELL")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "/bin/sh".into());
        let mut command = Command::new(shell);
        command.arg("-c").arg(text);
        command
    };
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command =
            Command::new(std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into()));
        command
            .args(["/D", "/S", "/C"])
            .raw_arg(format!("\"{text}\""));
        command
    };
    command
        .current_dir(directory)
        .env_remove("FILEX_CWD_FILE")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    Ok(command)
}
fn spawn_in(directory: &Path, text: &str, logs: &Path) -> Result<Started, String> {
    if text.trim().is_empty() {
        return Err("Enter a command".into());
    }
    if !directory.is_dir() {
        return Err("Select an existing folder to run a command".into());
    }
    fs::create_dir_all(logs).map_err(|e| e.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let log = logs.join(format!("{stamp}-{}.log", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options.open(&log).map_err(|e| e.to_string())?;
    writeln!(
        output,
        "directory: {}\ncommand: {text}\n",
        directory.display()
    )
    .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    let mut command = {
        use std::os::unix::process::CommandExt;
        let shell = std::env::var_os("SHELL")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "/bin/sh".into());
        let mut command = Command::new(shell);
        command.arg("-c").arg(text);
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        command
    };
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};
        let mut command =
            Command::new(std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into()));
        command
            .args(["/D", "/S", "/C"])
            .raw_arg(format!("\"{text}\""))
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
        command
    };
    let mut child = command
        .current_dir(directory)
        .env_remove("FILEX_CWD_FILE")
        .stdin(Stdio::null())
        .stdout(Stdio::from(output.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(output.try_clone().map_err(|e| e.to_string())?))
        .spawn()
        .map_err(|e| {
            let _ = writeln!(output, "Launch failed: {e}");
            format!("Launch failed: {e}; log: {}", log.display())
        })?;
    let pid = child.id();
    std::thread::Builder::new()
        .name("filex-job-wait".into())
        .stack_size(128 * 1024)
        .spawn(move || match child.wait() {
            Ok(status) => {
                let _ = writeln!(output, "\n[filex exit: {status}]");
            }
            Err(error) => {
                let _ = writeln!(output, "\n[filex wait failed: {error}]");
            }
        })
        .map_err(|e| {
            format!(
                "PID {pid} started; could not monitor exit: {e}; log: {}",
                log.display()
            )
        })?;
    Ok(Started { pid, log })
}
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn detached_command_uses_folder_captures_output_and_exit() {
        let root = std::env::temp_dir().join(format!("filex-job-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let folder = root.join("folder ' 中文");
        fs::create_dir_all(&folder).unwrap();
        let started = spawn_in(
            &folder,
            "pwd; printf 'test-output'; printf 'test-error' >&2; exit 7",
            &root.join("logs"),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let text = fs::read_to_string(&started.log).unwrap();
            if text.contains("[filex exit:") {
                assert!(text.contains(&folder.to_string_lossy().to_string()));
                assert!(text.contains("test-output"));
                assert!(text.contains("test-error"));
                assert!(text.contains("exit status: 7"));
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(spawn_in(&root.join("missing"), "true", &root.join("logs")).is_err());
        assert!(spawn_in(&folder, "  ", &root.join("logs")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
