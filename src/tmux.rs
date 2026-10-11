//! Reuse a verified idle shell to the right, without touching busy panes.
use std::{env, fs, path::PathBuf, process::Command};
use unicode_width::UnicodeWidthStr;

#[derive(Debug)]
struct Pane {
    id: String,
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    pid: u32,
    shell: String,
    cursor_x: usize,
    cursor_y: usize,
    normal: bool,
}
impl Pane {
    fn parse(line: &str) -> Option<Self> {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 12 {
            return None;
        }
        Some(Self {
            id: fields[0].into(),
            left: fields[1].parse().ok()?,
            top: fields[2].parse().ok()?,
            width: fields[3].parse().ok()?,
            height: fields[4].parse().ok()?,
            pid: fields[5].parse().ok()?,
            shell: fields[6].into(),
            cursor_x: fields[7].parse().ok()?,
            cursor_y: fields[8].parse().ok()?,
            normal: fields[9..].iter().all(|v| *v == "0"),
        })
    }
    fn right_of(&self, source: &Self) -> bool {
        self.left >= source.left + source.width
            && self.top < source.top + source.height
            && source.top < self.top + self.height
    }
    fn idle(&self) -> bool {
        if !self.normal
            || !["bash", "zsh", "fish", "sh", "dash", "ksh"].contains(&self.shell.as_str())
        {
            return false;
        }
        // An editor launched by a shell has a different foreground process group.
        let Ok(stat) = fs::read_to_string(format!("/proc/{}/stat", self.pid)) else {
            return false;
        };
        let Some((_, tail)) = stat.rsplit_once(") ") else {
            return false;
        };
        let fields: Vec<_> = tail.split_whitespace().collect();
        if fields.first() != Some(&"S")
            || fields.get(2).and_then(|v| v.parse::<u32>().ok()) != Some(self.pid)
            || fields.get(5).and_then(|v| v.parse::<u32>().ok()) != Some(self.pid)
        {
            return false;
        }
        let Ok(children) = fs::read_to_string(format!("/proc/{0}/task/{0}/children", self.pid))
        else {
            return false;
        };
        if !children.trim().is_empty() {
            return false;
        }
        let Some(screen) = tmux(&["capture-pane", "-p", "-t", &self.id]) else {
            return false;
        };
        let Some(line) = screen.lines().nth(self.cursor_y) else {
            return false;
        };
        empty_prompt(line, self.cursor_x)
    }
}

fn tmux(args: &[&str]) -> Option<String> {
    let output = Command::new("tmux").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

// Conservative support for conventional shell prompts ending in $, #, %, or >.
// Any text following the prompt (including a partially typed command) rejects it.
fn empty_prompt(line: &str, cursor_x: usize) -> bool {
    let text = line.trim_end();
    let Some(last) = text.chars().last() else {
        return false;
    };
    if !['$', '#', '%', '>'].contains(&last)
        || text[..text.len() - last.len_utf8()].contains(['$', '#', '%', '>'])
    {
        return false;
    }
    let width = text.width();
    cursor_x >= width && cursor_x <= width + 2
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn executable(command: &Command) -> Option<PathBuf> {
    let program = command.get_program();
    let path = PathBuf::from(program);
    if path.components().count() > 1 || path.is_absolute() {
        return fs::canonicalize(path).ok();
    }
    env::split_paths(&env::var_os("PATH")?)
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| fs::canonicalize(candidate).ok())
}

pub fn open_in_right_pane(command: &Command) -> Option<String> {
    env::var_os("TMUX")?;
    let source_id = env::var("TMUX_PANE").ok()?;
    let format = "#{pane_id}\t#{pane_left}\t#{pane_top}\t#{pane_width}\t#{pane_height}\t#{pane_pid}\t#{pane_current_command}\t#{cursor_x}\t#{cursor_y}\t#{pane_in_mode}\t#{pane_dead}\t#{alternate_on}";
    let listing = tmux(&["list-panes", "-t", &source_id, "-F", format])?;
    let panes: Vec<_> = listing.lines().filter_map(Pane::parse).collect();
    let source = panes.iter().find(|pane| pane.id == source_id)?;
    let mut candidates: Vec<_> = panes.iter().filter(|pane| pane.right_of(source)).collect();
    candidates.sort_by_key(|pane| {
        (
            pane.left,
            (pane.top + pane.height / 2).abs_diff(source.top + source.height / 2),
        )
    });
    let program = executable(command)?;
    let mut args = vec![program.to_str()?.to_owned()];
    for arg in command.get_args() {
        args.push(arg.to_str()?.to_owned());
    }
    if args.iter().any(|arg| arg.contains(['\r', '\n', '\0'])) {
        return None;
    }
    // Absolute executable and quoted arguments ignore aliases/functions and shell metacharacters.
    let cwd = match command.get_current_dir() {
        Some(path) => Some(path.to_str()?),
        None => None,
    };
    if cwd.is_some_and(|path| path.contains(['\r', '\n', '\0'])) {
        return None;
    }
    let prefix = cwd
        .map(|dir| format!("env -C {} --", shell_quote(dir)))
        .unwrap_or_else(|| "env --".into());
    let line = format!(
        "{prefix} {}",
        args.iter()
            .map(|arg| shell_quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    );
    for target in candidates {
        if target.idle() {
            // One tmux request queues literal text and Return in order. No Ctrl+U or respawn.
            tmux(&[
                "send-keys",
                "-t",
                &target.id,
                "-l",
                &line,
                ";",
                "send-keys",
                "-t",
                &target.id,
                "Enter",
            ])?;
            let _ = tmux(&["select-pane", "-t", &target.id]);
            return Some(target.id.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_commands_and_unrecognized_prompts() {
        assert!(empty_prompt("[a@arch 中文目录]$", 20));
        assert!(empty_prompt("$", 2));
        assert!(!empty_prompt("[a@arch ~]$ vim", 15));
        assert!(!empty_prompt("[a@arch ~]$ echo $", 18));
        assert!(!empty_prompt("[a@arch ~]$", 40));
        assert!(!empty_prompt("editor buffer", 13));
    }
    #[test]
    fn quotes_shell_metacharacters_as_literal_arguments() {
        assert_eq!(
            shell_quote("a' ; $(touch bad).txt"),
            "'a'\\'' ; $(touch bad).txt'"
        );
    }
}
