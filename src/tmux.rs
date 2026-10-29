//! Discover adjacent idle shells and recheck the chosen tmux pane before opening.
use std::{env, fs, path::PathBuf, process::Command};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}
impl Direction {
    fn name(self, ascii: bool) -> &'static str {
        match (self, ascii) {
            (Self::Up, true) => "Up",
            (Self::Up, false) => "上方",
            (Self::Down, true) => "Down",
            (Self::Down, false) => "下方",
            (Self::Left, true) => "Left",
            (Self::Left, false) => "左侧",
            (Self::Right, true) => "Right",
            (Self::Right, false) => "右侧",
        }
    }
}
#[derive(Debug)]
pub struct Target {
    pub id: String,
    source_id: String,
    pid: u32,
    direction: Direction,
    detail: String,
}
impl Target {
    pub fn description(&self, ascii: bool) -> String {
        format!(
            "{} · {} · {}",
            self.direction.name(ascii),
            self.id,
            self.detail
        )
    }
}

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
    cwd: String,
}
impl Pane {
    fn parse(line: &str) -> Option<Self> {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 15
            || !fields[0]
                .strip_prefix('%')
                .is_some_and(|id| !id.is_empty() && id.bytes().all(|c| c.is_ascii_digit()))
        {
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
            normal: fields[9..14].iter().all(|v| *v == "0"),
            cwd: fields[14].into(),
        })
    }
    fn direction_of(&self, source: &Self) -> Option<Direction> {
        // tmux leaves one border cell between rectangles. Require touching
        // edges and overlap: diagonal panes and panes behind a neighbour are out.
        let near = |end: usize, start: usize| start >= end && start - end <= 1;
        let overlaps_y =
            self.top < source.top + source.height && source.top < self.top + self.height;
        let overlaps_x =
            self.left < source.left + source.width && source.left < self.left + self.width;
        if overlaps_y && near(self.left + self.width, source.left) {
            Some(Direction::Left)
        } else if overlaps_y && near(source.left + source.width, self.left) {
            Some(Direction::Right)
        } else if overlaps_x && near(self.top + self.height, source.top) {
            Some(Direction::Up)
        } else if overlaps_x && near(source.top + source.height, self.top) {
            Some(Direction::Down)
        } else {
            None
        }
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

const PANE_FORMAT: &str = "#{pane_id}\t#{pane_left}\t#{pane_top}\t#{pane_width}\t#{pane_height}\t#{pane_pid}\t#{pane_current_command}\t#{cursor_x}\t#{cursor_y}\t#{pane_in_mode}\t#{pane_dead}\t#{alternate_on}\t#{pane_input_off}\t#{synchronize-panes}\t#{pane_current_path}";
fn panes(source_id: &str) -> Option<Vec<Pane>> {
    let listing = tmux(&["list-panes", "-t", source_id, "-F", PANE_FORMAT])?;
    Some(listing.lines().filter_map(Pane::parse).collect())
}

pub fn idle_panes() -> Vec<Target> {
    if env::var_os("TMUX").is_none() {
        return vec![];
    }
    let Ok(source_id) = env::var("TMUX_PANE") else {
        return vec![];
    };
    let Some(panes) = panes(&source_id) else {
        return vec![];
    };
    let Some(source) = panes.iter().find(|pane| pane.id == source_id) else {
        return vec![];
    };
    let mut candidates: Vec<_> = panes
        .iter()
        .filter_map(|pane| {
            let direction = pane.direction_of(source)?;
            pane.idle().then_some((direction, pane))
        })
        .collect();
    candidates.sort_by_key(|(direction, pane)| (*direction, pane.top, pane.left));
    candidates
        .into_iter()
        .map(|(direction, pane)| Target {
            id: pane.id.clone(),
            source_id: source_id.clone(),
            pid: pane.pid,
            direction,
            detail: format!(
                "{} · {}x{} · {}",
                pane.shell, pane.width, pane.height, pane.cwd
            ),
        })
        .collect()
}

fn command_line(command: &Command) -> Option<String> {
    let program = executable(command)?;
    let mut args = vec![program.to_str()?.to_owned()];
    for arg in command.get_args() {
        args.push(arg.to_str()?.to_owned());
    }
    if args.iter().any(|arg| arg.contains(['\r', '\n', '\0'])) {
        return None;
    }
    let cwd = match command.get_current_dir() {
        Some(path) => Some(path.to_str()?),
        None => None,
    };
    if cwd.is_some_and(|path| path.contains(['\r', '\n', '\0'])) {
        return None;
    }
    let prefix = cwd
        .map(|dir| format!("env -u FILEX_CWD_FILE -C {} --", shell_quote(dir)))
        .unwrap_or_else(|| "env -u FILEX_CWD_FILE --".into());
    Some(format!(
        "{prefix} {}",
        args.iter()
            .map(|arg| shell_quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    ))
}

pub fn open_in_pane(command: &Command, target: &Target) -> Result<(), String> {
    let line = command_line(command)
        .ok_or("Cannot send this command to tmux; choose Current terminal instead")?;
    // Selection may stay open for a while. Re-read the window geometry and
    // foreground shell, then re-check its prompt immediately before typing.
    let source_id = env::var("TMUX_PANE").map_err(|_| "tmux source pane is unavailable")?;
    if source_id != target.source_id {
        return Err("tmux source pane changed; choose again".into());
    }
    let panes = panes(&source_id).ok_or("tmux window is unavailable; choose again")?;
    let source = panes
        .iter()
        .find(|pane| pane.id == source_id)
        .ok_or("tmux source pane closed")?;
    let pane = panes
        .iter()
        .find(|pane| pane.id == target.id && pane.pid == target.pid)
        .filter(|pane| pane.direction_of(source).is_some() && pane.idle())
        .ok_or(
            "Pane changed or busy; choose again",
        )?;
    // No Ctrl+U, respawn or shell interpolation: preserve drafts and running work.
    tmux(&[
        "send-keys",
        "-t",
        &pane.id,
        "-l",
        &line,
        ";",
        "send-keys",
        "-t",
        &pane.id,
        "Enter",
    ])
    .ok_or("Could not send the command to the chosen tmux pane")?;
    let _ = tmux(&["select-pane", "-t", &pane.id]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pane(id: &str, left: usize, top: usize, width: usize, height: usize) -> Pane {
        Pane::parse(&format!(
            "{id}\t{left}\t{top}\t{width}\t{height}\t123\tbash\t2\t0\t0\t0\t0\t0\t0\t/tmp"
        ))
        .unwrap()
    }
    #[test]
    fn neighbours_cover_four_edges_and_exclude_diagonal_and_farther_panes() {
        let source = pane("%0", 21, 11, 20, 10);
        assert_eq!(
            pane("%1", 21, 0, 20, 10).direction_of(&source),
            Some(Direction::Up)
        );
        assert_eq!(
            pane("%2", 21, 22, 20, 10).direction_of(&source),
            Some(Direction::Down)
        );
        assert_eq!(
            pane("%3", 0, 11, 20, 10).direction_of(&source),
            Some(Direction::Left)
        );
        assert_eq!(
            pane("%4", 42, 11, 20, 10).direction_of(&source),
            Some(Direction::Right)
        );
        assert_eq!(pane("%5", 0, 0, 20, 10).direction_of(&source), None);
        assert_eq!(pane("%6", 63, 11, 20, 10).direction_of(&source), None);
        assert_eq!(source.direction_of(&source), None);
        // A split edge can have two neighbours; keep both choices.
        assert_eq!(
            pane("%7", 42, 11, 20, 4).direction_of(&source),
            Some(Direction::Right)
        );
        assert_eq!(
            pane("%8", 42, 16, 20, 5).direction_of(&source),
            Some(Direction::Right)
        );
    }
    #[test]
    fn copy_mode_dead_alternate_input_off_and_sync_are_not_idle() {
        let row = "%1\t0\t0\t20\t10\t123\tbash\t2\t0\t0\t0\t0\t0\t0\t/tmp";
        for flag in 9..14 {
            let mut fields: Vec<_> = row.split('\t').collect();
            fields[flag] = "1";
            let parsed = Pane::parse(&fields.join("\t")).unwrap();
            assert!(!parsed.normal);
            assert!(!parsed.idle());
        }
        assert!(Pane::parse("broken row").is_none());
        assert!(Pane::parse(&row.replacen("%1", "bad", 1)).is_none());
    }
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
