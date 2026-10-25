//! Capabilities are selected once, before entering raw mode. No desktop probes
//! or terminal reply queries are needed on a console/SSH connection.
use crate::config::Config;
use std::{
    env, fs,
    path::Path,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Colors {
    None,
    Consistent16,
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
static FOREGROUND_COMMAND: AtomicBool = AtomicBool::new(false);
pub struct ForegroundCommand;
impl ForegroundCommand {
    pub fn new() -> Self {
        FOREGROUND_COMMAND.store(true, Ordering::Relaxed);
        Self
    }
}
impl Drop for ForegroundCommand {
    fn drop(&mut self) {
        FOREGROUND_COMMAND.store(false, Ordering::Relaxed);
    }
}
static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn stop(signal: libc::c_int) {
    if signal == libc::SIGINT && FOREGROUND_COMMAND.load(Ordering::Relaxed) {
        return;
    }
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
        "consistent" => Colors::Consistent16,
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
pub type Palette = [(u8, u8, u8); 16];
// Linux VT's default palette, in ANSI order. Read the host defaults so vtrgb
// customizations also match the RGB colours we emit inside Konsole.
const LINUX_PALETTE: Palette = [
    (0, 0, 0),
    (170, 0, 0),
    (0, 170, 0),
    (170, 85, 0),
    (0, 0, 170),
    (170, 0, 170),
    (0, 170, 170),
    (170, 170, 170),
    (85, 85, 85),
    (255, 85, 85),
    (85, 255, 85),
    (255, 255, 85),
    (85, 85, 255),
    (255, 85, 255),
    (85, 255, 255),
    (255, 255, 255),
];
static SHARED_PALETTE: OnceLock<Palette> = OnceLock::new();
fn read_palette(directory: &Path) -> Option<Palette> {
    let channel = |name| -> Option<Vec<u8>> {
        let values = fs::read_to_string(directory.join(name))
            .ok()?
            .trim()
            .split(',')
            .map(|value| value.trim().parse::<u8>().ok())
            .collect::<Option<Vec<_>>>()?;
        (values.len() == 16).then_some(values)
    };
    let (r, g, b) = (
        channel("default_red")?,
        channel("default_grn")?,
        channel("default_blu")?,
    );
    Some(std::array::from_fn(|i| (r[i], g[i], b[i])))
}
pub fn shared_palette() -> &'static Palette {
    SHARED_PALETTE.get_or_init(|| {
        read_palette(Path::new("/sys/module/vt/parameters")).unwrap_or(LINUX_PALETTE)
    })
}
pub fn shared_color(rgb: (u8, u8, u8), bold: bool, dim: bool) -> (u8, (u8, u8, u8)) {
    let palette = shared_palette();
    let color = rgb;
    let mut index = palette
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 0 || rgb == (0, 0, 0))
        .min_by_key(|(_, p)| {
            let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
            d(color.0, p.0) + d(color.1, p.1) + d(color.2, p.2)
        })
        .map_or(7, |(i, _)| i as u8);
    // Keep the hue when dimming: nearest-colour RGB scaling can turn red into
    // brown, merging archives with code in a sixteen-colour palette.
    if dim && !bold {
        index = if index > 8 {
            index - 8
        } else if index == 7 {
            8
        } else {
            index
        };
    }
    // VT bold sets the intensity bit; choose that same bright bank for RGB.
    if bold {
        index |= 8;
    }
    (index, palette[index as usize])
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
            select_colors("consistent", "linux", "", true, false),
            Colors::Consistent16
        );
        assert_eq!(
            select_colors("consistent", "xterm-256color", "", false, false),
            Colors::Consistent16
        );
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
    #[test]
    fn shared_palette_and_styles_stay_inside_the_same_sixteen_slots() {
        let palette = shared_palette();
        for color in [
            (255, 215, 0),
            (75, 0, 130),
            (125, 207, 255),
            (170, 187, 204),
        ] {
            for (bold, dim) in [(false, false), (false, true), (true, false)] {
                let (index, rgb) = shared_color(color, bold, dim);
                assert_eq!(rgb, palette[index as usize]);
                assert!(index < 16);
                if bold {
                    assert!(index >= 8);
                }
            }
        }
        assert_eq!(shared_color((0, 0, 0), false, false), (0, palette[0]));
        assert_ne!(shared_color((1, 1, 1), false, true).0, 0);
    }
}
