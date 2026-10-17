//! Explicit, user-confirmed Git writes. Arguments never pass through a shell.
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
#[derive(Clone, Copy)]
pub enum Action {
    Publish,
    Add,
    Commit,
    Push,
}
impl Action {
    pub fn needs_message(self) -> bool {
        matches!(self, Self::Publish | Self::Commit)
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Publish => "WHOLE REPOSITORY: git add -A -> commit -> push",
            Self::Add => {
                "git add -A: selected item only (includes deletions within selected folder)"
            }
            Self::Commit => "git commit: all already-staged changes in repository",
            Self::Push => "git push: configured remote/refspecs (no force)",
        }
    }
}
fn git(root: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("--literal-pathspecs").arg("-C").arg(root);
    // Selection determines the repository, even if filex was launched from another Git context.
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_PREFIX",
    ] {
        command.env_remove(name);
    }
    command
}
pub fn repository(selected: &Path) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(selected).map_err(|e| e.to_string())?;
    let directory = if metadata.is_dir() {
        selected
    } else {
        selected.parent().ok_or("No parent directory")?
    };
    let output = git(directory)
        .args(["rev-parse", "--show-toplevel"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Cannot run git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Not a working-tree repository: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut bytes = output.stdout;
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    #[cfg(windows)]
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    #[cfg(unix)]
    let path = {
        use std::os::unix::ffi::OsStringExt;
        PathBuf::from(OsString::from_vec(bytes))
    };
    #[cfg(windows)]
    let path = PathBuf::from(OsString::from(
        String::from_utf8(bytes).map_err(|e| e.to_string())?,
    ));
    fs::canonicalize(path).map_err(|e| e.to_string())
}
pub fn commands(
    root: &Path,
    selected: &Path,
    action: Action,
    message: &str,
) -> Result<Vec<Command>, String> {
    if action.needs_message() && message.trim().is_empty() {
        return Err("Commit message cannot be empty".into());
    }
    let mut commands = Vec::new();
    if matches!(action, Action::Add | Action::Publish) {
        let mut command = git(root);
        command.args(["add", "-A", "--"]);
        if matches!(action, Action::Publish) {
            command.arg(".");
        } else {
            let metadata = fs::symlink_metadata(selected).map_err(|e| e.to_string())?;
            let path = if metadata.is_dir() {
                fs::canonicalize(selected)
            } else {
                selected
                    .parent()
                    .ok_or_else(|| std::io::Error::other("No parent"))
                    .and_then(fs::canonicalize)
                    .map(|p| p.join(selected.file_name().unwrap_or_default()))
            }
            .map_err(|e| e.to_string())?;
            let relative = path
                .strip_prefix(root)
                .map_err(|_| "Selected item is outside repository")?;
            command.arg(if relative.as_os_str().is_empty() {
                Path::new(".")
            } else {
                relative
            });
        }
        commands.push(command);
    }
    if action.needs_message() {
        let mut command = git(root);
        command.args(["commit", "-m", message]);
        commands.push(command);
    }
    if matches!(action, Action::Push | Action::Publish) {
        let mut command = git(root);
        command.arg("push");
        commands.push(command);
    }
    Ok(commands)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pipeline_keeps_message_literal_and_orders_operations() {
        let root = Path::new(".");
        let message = "fix: 中文 ' \" $(touch SHOULD_NOT_RUN) --amend";
        let commands = commands(root, root, Action::Publish, message).unwrap();
        let args: Vec<Vec<OsString>> = commands
            .iter()
            .map(|c| c.get_args().map(OsString::from).collect())
            .collect();
        assert_eq!(args.len(), 3);
        assert_eq!(args[0][3..], ["add", "-A", "--", "."].map(OsString::from));
        assert_eq!(args[1][3..], ["commit", "-m", message].map(OsString::from));
        assert_eq!(args[2][3..], [OsString::from("push")]);
        assert!(super::commands(root, root, Action::Commit, "  ").is_err());
    }
    #[test]
    fn individual_actions_do_not_implicitly_stage_or_commit() {
        let root = Path::new(".");
        let commit = commands(root, root, Action::Commit, "message").unwrap();
        assert_eq!(commit.len(), 1);
        assert_eq!(commit[0].get_args().nth(3).unwrap(), "commit");
        let push = commands(root, root, Action::Push, "").unwrap();
        assert_eq!(push.len(), 1);
        assert_eq!(push[0].get_args().nth(3).unwrap(), "push");
    }
}
