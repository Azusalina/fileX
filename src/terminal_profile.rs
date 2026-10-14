//! Capabilities are selected once, before entering raw mode. No desktop probes
//! or terminal reply queries are needed on a console/SSH connection.
use crate::config::Config;
use std::{
    env,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Colors {
    None,
    Ansi16,
    Ansi256,
    TrueColor,
}
#[derive(Clone, Copy, Debug)]
pub struct Profile {
    pub console: bool,
    pub desktop: bool,
    pub enhanced: bool,
    pub ascii: bool,
    pub colors: Colors,
}
static PROFILE: OnceLock<Profile> = OnceLock::new();
static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn stop(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}
pub fn stopping() -> bool {
    STOP.load(Ordering::Relaxed)
}
pub struct Signals(Vec<(libc::c_int, libc::sigaction)>);
impl Signals {
    pub fn new() -> std::io::Result<Self> {
        let mut guard = Self(Vec::new());
        for signal in [libc::SIGTERM, libc::SIGHUP, libc::SIGINT] {
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = stop as *const () as usize;
            action.sa_flags = libc::SA_RESTART;
            unsafe {
                libc::sigemptyset(&mut action.sa_mask);
            }
            let mut previous = unsafe { std::mem::zeroed() };
            if unsafe { libc::sigaction(signal, &action, &mut previous) } != 0 {
                return Err(std::io::Error::last_os_error());
            }
            guard.0.push((signal, previous));
        }
        Ok(guard)
    }
}
impl Drop for Signals {
    fn drop(&mut self) {
        for (signal, previous) in &self.0 {
            unsafe {
                libc::sigaction(*signal, previous, std::ptr::null_mut());
            }
        }
    }
}
pub fn get() -> Profile {
    *PROFILE.get_or_init(default_profile)
}
#[cfg(not(test))]
fn default_profile() -> Profile {
    detect(&Config::default(), false, false)
}
#[cfg(test)]
fn default_profile() -> Profile {
    Profile {
        console: false,
        desktop: true,
        enhanced: true,
        ascii: false,
        colors: Colors::TrueColor,
    }
}
pub fn init(conf: &Config, tty: bool, ascii: bool) {
    let _ = PROFILE.set(detect(conf, tty, ascii));
}
fn detect(conf: &Config, tty: bool, ascii: bool) -> Profile {
    let term = env::var("TERM").unwrap_or_default();
    let display = ["WAYLAND_DISPLAY", "DISPLAY"]
        .iter()
        .any(|key| env::var_os(key).is_some_and(|v| !v.is_empty()));
    let mut console_mode: libc::c_int = 0;
    // KDGETMODE only succeeds on a Linux virtual console, not a pseudoterminal.
    let physical = unsafe { libc::ioctl(libc::STDIN_FILENO, 0x4b3b, &mut console_mode) } == 0;
    let console = tty || conf.terminal_mode == "tty" || term == "linux" || physical;
    let desktop = display
        && !console
        && env::var_os("SSH_CONNECTION").is_none()
        && env::var_os("SSH_TTY").is_none();
    let mut colors = select_colors(
        &conf.color_mode,
        &term,
        &env::var("COLORTERM").unwrap_or_default(),
        console,
        env::var_os("NO_COLOR").is_some_and(|s| !s.is_empty()),
    );
    // Preserve precise desktop extension colours in modern KDE terminals.
    if desktop && conf.color_mode == "auto" && term.contains("256color") && colors != Colors::None {
        colors = Colors::TrueColor;
    }
    Profile {
        console,
        desktop,
        enhanced: desktop && !["dumb", "vt100", "vt102", "ansi", ""].contains(&term.as_str()),
        ascii: ascii || conf.charset == "ascii" || conf.charset == "auto" && (console || !desktop),
        colors,
    }
}
pub fn select_colors(
    mode: &str,
    term: &str,
    colorterm: &str,
    console: bool,
    no_color: bool,
) -> Colors {
    if no_color || mode == "none" {
        return Colors::None;
    }
    match mode {
        "16" => Colors::Ansi16,
        "256" => Colors::Ansi256,
        "truecolor" => Colors::TrueColor,
        _ if console => Colors::Ansi16,
        _ if ["truecolor", "24bit"].contains(&colorterm) => Colors::TrueColor,
        _ if term.contains("256color") => Colors::Ansi256,
        _ if term == "dumb" || term.is_empty() => Colors::None,
        _ => Colors::Ansi16,
    }
}
// ASCII changes decoration only; filenames and preview text remain untouched.
pub fn glyph(c: char) -> char {
    if !get().ascii {
        return c;
    }
    match c {
        '│' => '|',
        '─' => '-',
        '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' => '+',
        '▸' => '>',
        '◂' => '<',
        '▾' => 'v',
        '·' => '.',
        '↗' => '@',
        '×' => 'x',
        '●' => '*',
        _ => c,
    }
}
pub fn label<'a>(zh: &'a str, en: &'a str) -> &'a str {
    if get().ascii { en } else { zh }
}
const PALETTE: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (128, 0, 0),
    (0, 128, 0),
    (128, 128, 0),
    (0, 0, 128),
    (128, 0, 128),
    (0, 128, 128),
    (192, 192, 192),
    (128, 128, 128),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (0, 0, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];
pub fn palette_index(r: u8, g: u8, b: u8, count: u16) -> u8 {
    (0..count)
        .min_by_key(|&i| {
            let (pr, pg, pb) = if i < 16 {
                PALETTE[i as usize]
            } else if i < 232 {
                let n = i - 16;
                let steps = [0, 95, 135, 175, 215, 255];
                (
                    steps[(n / 36) as usize],
                    steps[(n / 6 % 6) as usize],
                    steps[(n % 6) as usize],
                )
            } else {
                let gray = (8 + 10 * (i - 232)) as u8;
                (gray, gray, gray)
            };
            let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
            d(r, pr) + d(g, pg) + d(b, pb)
        })
        .unwrap_or(7) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn colors_follow_console_and_no_color() {
        assert_eq!(
            select_colors("auto", "linux", "truecolor", true, false),
            Colors::Ansi16
        );
        assert_eq!(
            select_colors("auto", "screen-256color", "", false, false),
            Colors::Ansi256
        );
        assert_eq!(
            select_colors("auto", "xterm", "truecolor", false, false),
            Colors::TrueColor
        );
        assert_eq!(
            select_colors("truecolor", "linux", "", true, true),
            Colors::None
        );
        assert_eq!(palette_index(255, 215, 0, 256), 220);
        assert_eq!(palette_index(0, 0, 128, 16), 4);
    }
}
