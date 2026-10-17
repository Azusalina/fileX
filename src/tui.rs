use crate::{
    config::{Config, key_code},
    files,
    model::{Browser, Layout, clean, clip},
    terminal_profile::{self, Colors, label},
    tmux,
};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{
        self, DisableFocusChange, EnableFocusChange, Event, KeyCode, KeyEventKind, KeyModifiers,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute, queue,
    style::{
        Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
    },
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    rc::Rc,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
fn ioerr(e: impl std::fmt::Display) -> String {
    e.to_string()
}
struct Guard;
impl Guard {
    fn new() -> Result<Self, String> {
        terminal::enable_raw_mode().map_err(ioerr)?;
        let guard = Self;
        Self::enter()?;
        Ok(guard)
    }
    fn enter() -> Result<(), String> {
        if terminal_profile::get().console {
            execute!(io::stdout(), Clear(ClearType::All), MoveTo(0, 0)).map_err(ioerr)?;
        } else {
            execute!(io::stdout(), EnterAlternateScreen).map_err(ioerr)?;
        }
        if terminal_profile::get().enhanced {
            execute!(
                io::stdout(),
                EnableFocusChange,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES),
                Print("\x1b[>4;2m")
            )
            .map_err(ioerr)?;
        }
        execute!(io::stdout(), Hide).map_err(ioerr)
    }
    fn leave() -> Result<(), String> {
        execute!(
            io::stdout(),
            SetAttribute(Attribute::Reset),
            ResetColor,
            Show
        )
        .map_err(ioerr)?;
        if terminal_profile::get().enhanced {
            execute!(
                io::stdout(),
                PopKeyboardEnhancementFlags,
                DisableFocusChange,
                Print("\x1b[>4;0m")
            )
            .map_err(ioerr)?;
        }
        if terminal_profile::get().console {
            execute!(io::stdout(), Clear(ClearType::All), MoveTo(0, 0)).map_err(ioerr)
        } else {
            execute!(io::stdout(), LeaveAlternateScreen).map_err(ioerr)
        }
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = Self::leave();
    }
}
fn read_key(auto_english: bool) -> Result<String, String> {
    loop {
        if terminal_profile::stopping() {
            return Ok("\x03".into());
        }
        if !event::poll(std::time::Duration::from_millis(200)).map_err(ioerr)? {
            if crate::git_status::ready() {
                return Ok("git_refresh".into());
            }
            continue;
        }
        match event::read().map_err(ioerr)? {
            Event::FocusLost => continue,
            Event::FocusGained => {
                // Reapply raw mode without replacing crossterm's saved shell state.
                #[cfg(unix)]
                {
                    let mut settings = std::mem::MaybeUninit::<libc::termios>::uninit();
                    if unsafe { libc::tcgetattr(libc::STDIN_FILENO, settings.as_mut_ptr()) } == 0 {
                        let mut settings = unsafe { settings.assume_init() };
                        unsafe {
                            libc::cfmakeraw(&mut settings);
                            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &settings);
                            libc::tcflow(libc::STDOUT_FILENO, libc::TCOON);
                        }
                    }
                }
                #[cfg(windows)]
                terminal::enable_raw_mode().map_err(ioerr)?;
                execute!(io::stdout(), Hide).map_err(ioerr)?;
                if terminal_profile::get().enhanced {
                    execute!(io::stdout(), Print("\x1b[=1u\x1b[>4;2m")).map_err(ioerr)?;
                }
                crate::input::ensure_english(auto_english && terminal_profile::get().desktop);
                return Ok("focus_gained".into());
            }
            Event::Resize(_, _) => return Ok("resize".into()),
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let name = match key.code {
                    KeyCode::Char(c) => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) && c.is_ascii_alphabetic()
                        {
                            let mut prefix = "Ctrl".to_owned();
                            if key.modifiers.contains(KeyModifiers::ALT) {
                                prefix.push_str("+Alt");
                            }
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                prefix.push_str("+Shift");
                            }
                            key_code(&format!("{prefix}+{c}"))
                        } else {
                            c.to_string()
                        }
                    }
                    KeyCode::Enter => "enter".into(),
                    KeyCode::Tab | KeyCode::BackTab => "\t".into(),
                    KeyCode::Esc => "escape".into(),
                    KeyCode::Backspace => "backspace".into(),
                    KeyCode::Delete => "delete".into(),
                    KeyCode::F(5) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        "ctrl+f5".into()
                    }
                    KeyCode::F(5) => "f5".into(),
                    KeyCode::Up => "up".into(),
                    KeyCode::Down => "down".into(),
                    KeyCode::Left => "left".into(),
                    KeyCode::Right => "right".into(),
                    KeyCode::PageUp => "pageup".into(),
                    KeyCode::PageDown => "pagedown".into(),
                    KeyCode::Home => "home".into(),
                    KeyCode::End => "end".into(),
                    _ => continue,
                };
                return Ok(name);
            }
            _ => {}
        }
    }
}
fn rgb(value: &str) -> Color {
    Color::Rgb {
        r: u8::from_str_radix(&value[1..3], 16).unwrap_or(200),
        g: u8::from_str_radix(&value[3..5], 16).unwrap_or(200),
        b: u8::from_str_radix(&value[5..7], 16).unwrap_or(200),
    }
}
fn ansi_foreground(index: u8) -> String {
    format!(
        "\x1b[{}m",
        if index < 8 {
            30 + index
        } else {
            90 + index - 8
        }
    )
}
fn consistent_foreground(
    out: &mut impl Write,
    rgb: (u8, u8, u8),
    bold: bool,
    dim: bool,
) -> Result<(), String> {
    let (index, (r, g, b)) = terminal_profile::shared_color(rgb, bold, dim);
    if terminal_profile::get().console {
        queue!(out, Print(ansi_foreground(index))).map_err(ioerr)
    } else {
        queue!(out, SetForegroundColor(Color::Rgb { r, g, b })).map_err(ioerr)
    }
}
fn consistent_background(out: &mut impl Write) -> Result<(), String> {
    if terminal_profile::get().console {
        queue!(out, Print("\x1b[40m")).map_err(ioerr)
    } else {
        let (r, g, b) = terminal_profile::shared_palette()[0];
        queue!(out, SetBackgroundColor(Color::Rgb { r, g, b })).map_err(ioerr)
    }
}
fn default_style(out: &mut impl Write) -> Result<(), String> {
    queue!(out, SetAttribute(Attribute::Reset), ResetColor).map_err(ioerr)?;
    if terminal_profile::get().colors == Colors::Consistent16 {
        consistent_background(out)?;
        consistent_foreground(out, terminal_profile::shared_palette()[15], false, false)?;
    }
    Ok(())
}
fn clear_screen(out: &mut impl Write) -> Result<(), String> {
    default_style(out)?;
    queue!(out, Clear(ClearType::All)).map_err(ioerr)
}
fn put(
    out: &mut impl Write,
    y: usize,
    x: isize,
    text: &str,
    width: usize,
    color: Option<&str>,
    reverse: bool,
) -> Result<(), String> {
    put_styled(out, y, x, text, width, color, reverse, false, false)
}
#[allow(clippy::too_many_arguments)]
fn put_styled(
    out: &mut impl Write,
    y: usize,
    mut x: isize,
    text: &str,
    width: usize,
    color: Option<&str>,
    reverse: bool,
    bold: bool,
    dim: bool,
) -> Result<(), String> {
    let mut value = clean(text);
    while x < 0 && !value.is_empty() {
        let c = value.chars().next().unwrap();
        value.drain(..c.len_utf8());
        x += c.width().unwrap_or(0) as isize;
    }
    if x < 0 || x as usize >= width {
        return Ok(());
    }
    let value = clip(&value, width.saturating_sub(x as usize + 1));
    queue!(
        out,
        MoveTo(x as u16, y as u16),
        SetAttribute(Attribute::Reset)
    )
    .map_err(ioerr)?;
    let consistent = terminal_profile::get().colors == Colors::Consistent16;
    if consistent {
        consistent_background(out)?;
    }
    if dim && !consistent {
        queue!(out, SetAttribute(Attribute::Dim)).map_err(ioerr)?;
    }
    if let Some(color) = color {
        let Color::Rgb { r, g, b } = rgb(color) else {
            unreachable!()
        };
        match terminal_profile::get().colors {
            Colors::None => {}
            Colors::Consistent16 => consistent_foreground(out, (r, g, b), bold, dim)?,
            Colors::TrueColor => {
                queue!(out, SetForegroundColor(Color::Rgb { r, g, b })).map_err(ioerr)?
            }
            Colors::Ansi256 => queue!(
                out,
                SetForegroundColor(Color::AnsiValue(terminal_profile::palette_index(
                    r, g, b, 256
                )))
            )
            .map_err(ioerr)?,
            Colors::Ansi16 => {
                let index = terminal_profile::palette_index(r, g, b, 16);
                queue!(
                    out,
                    Print(format!(
                        "\x1b[{}m",
                        if index < 8 {
                            30 + index
                        } else {
                            90 + index - 8
                        }
                    ))
                )
                .map_err(ioerr)?;
            }
        }
    } else if consistent {
        consistent_foreground(out, terminal_profile::shared_palette()[15], bold, dim)?;
    } else {
        queue!(out, ResetColor).map_err(ioerr)?
    }
    if reverse {
        queue!(out, SetAttribute(Attribute::Reverse)).map_err(ioerr)?
    }
    if bold {
        queue!(out, SetAttribute(Attribute::Bold)).map_err(ioerr)?;
    }
    queue!(out, Print(value)).map_err(ioerr)?;
    Ok(())
}
#[derive(Clone, Copy)]
struct TreeArea {
    x: usize,
    width: usize,
    mirrored: bool,
}
impl TreeArea {
    #[allow(clippy::too_many_arguments)]
    fn paint(
        &self,
        out: &mut impl Write,
        y: usize,
        x: isize,
        text: &str,
        color: Option<&str>,
        reverse: bool,
        bold: bool,
        dim: bool,
    ) -> Result<(), String> {
        let text = clean(text);
        let text = if self.mirrored {
            let mut text = text
                .chars()
                .map(|c| match c {
                    '┌' => '┐',
                    '┐' => '┌',
                    '└' => '┘',
                    '┘' => '└',
                    '├' => '┤',
                    '┤' => '├',
                    '▸' => '◂',
                    _ => c,
                })
                .collect::<String>();
            if terminal_profile::get().ascii && text.starts_with('>') {
                text.replace_range(..1, "<");
            }
            text
        } else {
            text
        };
        let mut x = if self.mirrored {
            self.width as isize - x - text.width() as isize
        } else {
            x
        };
        let mut text = text.as_str();
        while x < 0 && !text.is_empty() {
            let c = text.chars().next().unwrap();
            text = &text[c.len_utf8()..];
            x += c.width().unwrap_or(0) as isize;
        }
        if x < 0 || x >= self.width as isize {
            return Ok(());
        }
        put_styled(
            out,
            y,
            self.x as isize + x,
            text,
            self.x + self.width,
            color,
            reverse,
            bold,
            dim,
        )
    }
}
fn draw_tree(
    out: &mut impl Write,
    pane: &mut Pane,
    area: TreeArea,
    body: usize,
    active: bool,
) -> Result<(), String> {
    let tree = area.width;
    let layout = Layout::new(&pane.browser);
    let selected = layout
        .items
        .iter()
        .find(|p| Rc::ptr_eq(&p.node, &pane.browser.selected))
        .ok_or("选中节点不在树中")?;
    if pane.focus {
        let margin = usize::from(selected.node.borrow().marked);
        if selected.y.saturating_sub(margin) < pane.top {
            pane.top = selected.y.saturating_sub(margin);
        }
        if selected.y + margin >= pane.top + body {
            pane.top = (selected.y + margin + 1).saturating_sub(body);
        }
        if selected.x < pane.left {
            pane.left = selected.x
        }
        if selected.x + selected.label.width() > pane.left + tree - 2 {
            pane.left = (selected.x + selected.label.width()).saturating_sub(tree - 2)
        }
        pane.focus = false;
    }
    pane.left = pane.left.min(layout.width.saturating_sub(tree - 2));
    pane.top = pane.top.min(layout.height.saturating_sub(body));
    area.paint(
        out,
        1,
        1,
        &pane.browser.root.borrow().path.to_string_lossy(),
        Some(
            pane.browser
                .conf
                .folder_color_for(&pane.browser.root.borrow().path),
        ),
        false,
        false,
        false,
    )?;
    let route = if pane.browser.conf.highlight_path {
        layout.path_cells(
            &pane.browser.selected,
            pane.browser.conf.gap,
            pane.left,
            pane.top,
            tree - 2,
            body,
        )
    } else {
        Default::default()
    };
    let mut ancestors = std::collections::HashSet::new();
    let mut ancestor = Some(pane.browser.selected.clone());
    while let Some(node) = ancestor {
        ancestors.insert(Rc::as_ptr(&node) as usize);
        ancestor = node.borrow().parent.upgrade();
    }
    for ((x, y), c) in layout.cells(pane.left, pane.top, tree - 2, body) {
        let active = route.contains(&(x, y));
        area.paint(
            out,
            y - pane.top + 3,
            (x - pane.left + 1) as isize,
            &c.to_string(),
            Some(if active {
                &pane.browser.conf.path_color
            } else {
                &pane.browser.conf.line_color
            }),
            false,
            false,
            pane.browser.conf.highlight_path && !active,
        )?
    }
    for item in &layout.items {
        if item.y >= pane.top && item.y < pane.top + body {
            let n = item.node.borrow();
            if n.marked {
                let width = item.label.width().saturating_sub(2);
                let top = if terminal_profile::get().ascii {
                    format!("+{}+", "-".repeat(width))
                } else {
                    format!("┌{}┐", "─".repeat(width))
                };
                let bottom = if terminal_profile::get().ascii {
                    top.clone()
                } else {
                    format!("└{}┘", "─".repeat(width))
                };
                for (y, text, cursor) in [
                    (item.y.saturating_sub(1), top.as_str(), false),
                    (item.y, item.label.as_str(), true),
                    (item.y + 1, bottom.as_str(), false),
                ] {
                    if y >= pane.top && y < pane.top + body {
                        area.paint(
                            out,
                            y - pane.top + 3,
                            item.x as isize - pane.left as isize + 1,
                            text,
                            Some(&pane.browser.conf.mark_color),
                            cursor && active && Rc::ptr_eq(&item.node, &pane.browser.selected),
                            true,
                            false,
                        )?;
                    }
                }
                if n.git != crate::git_status::State::None {
                    let tip = item.label.chars().skip(2).take(2).collect::<String>();
                    area.paint(
                        out,
                        item.y - pane.top + 3,
                        item.x as isize - pane.left as isize + 3,
                        &tip,
                        Some(n.git.color()),
                        active && Rc::ptr_eq(&item.node, &pane.browser.selected),
                        true,
                        false,
                    )?;
                }
                continue;
            }
            let color = if n.dir {
                pane.browser.conf.folder_color_for(&n.path)
            } else {
                pane.browser
                    .conf
                    .rule(&n.path, &pane.browser.conf.extension_colors, false)
                    .unwrap_or(&pane.browser.conf.file_color)
            };
            let dim = pane.browser.conf.highlight_path
                && !ancestors.contains(&(Rc::as_ptr(&item.node) as usize));
            area.paint(
                out,
                item.y - pane.top + 3,
                item.x as isize - pane.left as isize + 1,
                item.link_start
                    .map_or(item.label.as_str(), |start| &item.label[..start]),
                Some(color),
                active
                    && Rc::ptr_eq(&item.node, &pane.browser.selected)
                    && pane.link_target.is_none(),
                false,
                dim,
            )?;
            if n.git != crate::git_status::State::None {
                let tip = item.label.chars().take(2).collect::<String>();
                area.paint(
                    out,
                    item.y - pane.top + 3,
                    item.x as isize - pane.left as isize + 1,
                    &tip,
                    Some(n.git.color()),
                    active
                        && Rc::ptr_eq(&item.node, &pane.browser.selected)
                        && pane.link_target.is_none(),
                    true,
                    false,
                )?;
            }
            if let Some(start) = item.link_start {
                let target_selected =
                    Rc::ptr_eq(&item.node, &pane.browser.selected) && pane.link_target.is_some();
                let tail = &item.label[start..];
                // Keep the connector's bold color; reverse only the target name.
                let split = target_selected || area.mirrored;
                let connector_end = if split { 6 } else { tail.len() };
                area.paint(
                    out,
                    item.y - pane.top + 3,
                    (item.x + item.label[..start].width()) as isize - pane.left as isize + 1,
                    &tail[..connector_end],
                    Some(&pane.browser.conf.link_color),
                    false,
                    true,
                    dim,
                )?;
                if split {
                    area.paint(
                        out,
                        item.y - pane.top + 3,
                        (item.x + item.label[..start].width() + 6) as isize - pane.left as isize
                            + 1,
                        &tail[6..],
                        Some(&pane.browser.conf.link_color),
                        active && target_selected,
                        true,
                        dim,
                    )?;
                }
            }
        }
    }
    Ok(())
}
struct Pane {
    browser: Browser,
    left: usize,
    top: usize,
    focus: bool,
    query: String,
    link_target: Option<PathBuf>,
}
impl Pane {
    fn new(browser: Browser) -> Self {
        Self {
            browser,
            left: 0,
            top: 0,
            focus: true,
            query: String::new(),
            link_target: None,
        }
    }
}
struct Pending {
    sources: Vec<PathBuf>,
    moving: bool,
}
struct Ui {
    clipboard: String,
    marked: std::collections::BTreeSet<PathBuf>,
    pending: Option<Pending>,
    pane: Pane,
    other: Option<Pane>,
    right_active: bool,
    node_budget: usize,
    config_path: PathBuf,
    dashboard: bool,
    meta_path: Option<PathBuf>,
    meta: String,
    message: String,
    exit_directory: Option<PathBuf>,
}
impl Ui {
    fn sources(&self) -> Result<Vec<PathBuf>, String> {
        if self.pane.link_target.is_some() {
            return Err("Select the link itself before operating".into());
        }
        let mut paths: Vec<PathBuf> = if self.marked.is_empty() {
            vec![self.selection_path()]
        } else {
            self.marked.iter().cloned().collect()
        };
        paths.sort_by_key(|p| p.components().count());
        let mut sources: Vec<PathBuf> = Vec::new();
        for path in paths {
            std::fs::symlink_metadata(&path).map_err(ioerr)?;
            if !sources.iter().any(|parent| path.starts_with(parent)) {
                sources.push(path);
            }
        }
        Ok(sources)
    }
    fn protected_root(&self, path: &Path) -> bool {
        std::iter::once(&self.pane)
            .chain(self.other.iter())
            .any(|p| p.browser.root.borrow().path == path)
    }
    fn refresh_panes(&mut self) {
        self.pane.browser.refresh();
        self.pane.link_target = None;
        if let Some(other) = &mut self.other {
            other.browser.refresh();
            other.link_target = None;
        }
        self.meta_path = None;
    }
    fn begin_transfer(&mut self, moving: bool) -> Result<(), String> {
        let sources = self.sources()?;
        if moving && sources.iter().any(|p| self.protected_root(p)) {
            return Err("Cannot move the displayed root; first navigate to its parent".into());
        }
        self.pending = Some(Pending { sources, moving });
        self.pane.browser.set_destination_mode(true);
        if let Some(other) = &mut self.other {
            other.browser.set_destination_mode(true);
            self.switch_pane();
        }
        self.message =
            "Choose destination: arrows + Enter; t types a path; + creates folder; Esc cancels"
                .into();
        Ok(())
    }
    fn cancel_transfer(&mut self) {
        self.pending = None;
        self.pane.browser.set_destination_mode(false);
        if let Some(other) = &mut self.other {
            other.browser.set_destination_mode(false);
        }
        self.meta_path = None;
    }
    fn create_destination(&mut self) -> Result<(), String> {
        let directory = self.pane.browser.target();
        let Some(name) = self.prompt("New destination folder", "")? else {
            return Ok(());
        };
        let created = files::create(&directory, &name, true)?;
        let parent = self
            .pane
            .browser
            .visible()
            .into_iter()
            .find(|n| n.borrow().path == directory && !n.borrow().create_entry);
        if let Some(parent) = parent {
            self.pane.browser.expand(&parent, Some(&created));
        }
        if let Some(node) = self
            .pane
            .browser
            .visible()
            .into_iter()
            .find(|n| n.borrow().path == created)
        {
            self.pane.browser.selected = node;
        }
        self.meta_path = None;
        Ok(())
    }
    fn commit_transfer(&mut self) -> Result<(), String> {
        if self.pane.browser.selected.borrow().create_entry {
            return self.create_destination();
        }
        let target = self.selection_path();
        self.transfer_to(&target)
    }
    fn transfer_path(&mut self) -> Result<(), String> {
        let initial = self.pane.browser.target().to_string_lossy().into_owned();
        let Some(value) = self.prompt("Destination path (file or directory)", &initial)? else {
            return Ok(());
        };
        if value.is_empty() {
            return Ok(());
        }
        let source = self
            .pending
            .as_ref()
            .ok_or("No pending operation")?
            .sources
            .first()
            .ok_or("No pending items")?;
        let target = files::destination(source, &value);
        self.transfer_to(&target)
    }
    fn transfer_to(&mut self, directory: &Path) -> Result<(), String> {
        let pending = self.pending.as_ref().ok_or("No pending operation")?;
        let plan = crate::transfer::plan_to(&pending.sources, directory)?;
        let moving = pending.moving;
        let total = plan.len();
        let replacements: Vec<bool> = plan
            .iter()
            .map(|(source, dest)| crate::transfer::validate_target(source, dest))
            .collect::<Result<_, _>>()?;
        for ((_, destination), replace) in plan.iter().zip(&replacements) {
            if *replace
                && !self.confirm(&format!(
                    "Replace existing file: {}",
                    clean(&destination.to_string_lossy())
                ))?
            {
                self.message = "Replacement cancelled; choose destination or press Esc".into();
                return Ok(());
            }
        }
        for (index, ((source, destination), replace)) in
            plan.into_iter().zip(replacements).enumerate()
        {
            if let Err(error) = crate::transfer::execute(&source, &destination, moving, replace) {
                self.refresh_panes();
                return Err(format!(
                    "Completed {index}/{total}; remaining items kept pending: {error}"
                ));
            }
            self.marked.retain(|p| !p.starts_with(&source));
            self.pending
                .as_mut()
                .unwrap()
                .sources
                .retain(|p| p != &source);
        }
        self.cancel_transfer();
        self.refresh_panes();
        let selected = self.pane.browser.selected.clone();
        self.pane.browser.expand(&selected, None);
        self.message = format!(
            "{} {total} items to {}",
            if moving { "Moved" } else { "Copied" },
            directory.display()
        );
        Ok(())
    }

    fn rebudget(&mut self) {
        self.pane.browser.conf.max_loaded = self.node_budget;
        if let Some(other) = &mut self.other {
            other.browser.conf.max_loaded = self.node_budget;
        }
    }
    fn toggle_dual(&mut self) -> Result<(), String> {
        if self.other.is_some() {
            self.other = None;
            self.right_active = false;
            self.message = label("单栏：保留当前目录", "Single pane: current folder kept").into();
        } else {
            let directory = files::exit_directory_path(&self.selection_path())?;
            let mut conf = self.pane.browser.conf.clone();
            conf.max_loaded = self.node_budget;
            self.other = Some(Pane::new(Browser::new(&directory, conf)?));
            self.message = label(
                "双栏：Tab 切换 · c/m 到另一侧 · 右栏 ← 展开 / → 父级",
                "Dual panes: Tab switch, c/m transfer; right pane Left child / Right parent",
            )
            .into();
        }
        self.pane.focus = true;
        self.meta_path = None;
        self.rebudget();
        Ok(())
    }
    fn switch_pane(&mut self) {
        if let Some(other) = &mut self.other {
            std::mem::swap(&mut self.pane, other);
            self.right_active = !self.right_active;
            self.pane.focus = true;
            self.meta_path = None;
            self.message = if self.right_active {
                label("右侧：← 展开 · → 父级", "Right: Left child / Right parent")
            } else {
                label("左侧：→ 展开 · ← 父级", "Left: Right child / Left parent")
            }
            .into();
            self.rebudget();
        }
    }
    fn selection_path(&self) -> PathBuf {
        self.pane
            .link_target
            .clone()
            .unwrap_or_else(|| self.pane.browser.selected.borrow().path.clone())
    }
    fn draw(&mut self) -> Result<(), String> {
        self.pane.browser.update_git();
        if let Some(other) = &mut self.other {
            other.browser.update_git();
        }
        if self.pending.is_some() {
            if !self.pane.browser.destination_mode {
                self.pane.browser.set_destination_mode(true);
            }
            if let Some(other) = &mut self.other {
                if !other.browser.destination_mode {
                    other.browser.set_destination_mode(true);
                }
            }
        }
        for pane in std::iter::once(&self.pane).chain(self.other.iter()) {
            for node in pane.browser.visible() {
                let marked =
                    self.marked.contains(&node.borrow().path) && !node.borrow().create_entry;
                node.borrow_mut().marked = marked;
            }
        }
        let (w, h) = terminal::size().map_err(ioerr)?;
        let (w, h) = (w as usize, h as usize);
        let mut out = io::stdout().lock();
        clear_screen(&mut out)?;
        if w < if self.other.is_some() { 75 } else { 36 } || h < 8 {
            put(
                &mut out,
                0,
                0,
                &ui_format!(
                    "请放大终端到 {}×8；{} 单栏 / {} 退出",
                    "Resize to {}x8; {} single pane / {} quit",
                    if self.other.is_some() { 75 } else { 36 },
                    self.pane.browser.conf.keys["toggle_dual"],
                    self.pane.browser.conf.keys["quit"]
                ),
                w,
                None,
                false,
            )?;
            return out.flush().map_err(ioerr);
        }
        let panel = if self.dashboard && w >= if self.other.is_some() { 110 } else { 70 } {
            self.pane
                .browser
                .conf
                .dashboard_columns(w)
                .min(w - if self.other.is_some() { 77 } else { 38 })
        } else {
            0
        };
        let tree = w - panel - if panel > 0 { 2 } else { 0 };
        let body = h - 5;
        put(&mut out, 0, 1, "filex", w, None, false)?;
        if let Some(other) = &mut self.other {
            let middle = tree / 2;
            let left_area = TreeArea {
                x: 0,
                width: middle,
                mirrored: false,
            };
            let right_area = TreeArea {
                x: middle + 1,
                width: tree - middle - 1,
                mirrored: true,
            };
            if self.right_active {
                draw_tree(&mut out, other, left_area, body, false)?;
                draw_tree(&mut out, &mut self.pane, right_area, body, true)?;
            } else {
                draw_tree(&mut out, &mut self.pane, left_area, body, true)?;
                draw_tree(&mut out, other, right_area, body, false)?;
            }
            for y in 0..h - 2 {
                put(
                    &mut out,
                    y,
                    middle as isize,
                    label("│", "|"),
                    tree,
                    Some(&self.pane.browser.conf.line_color),
                    false,
                )?;
            }
            put(
                &mut out,
                0,
                1,
                if self.right_active {
                    label("filex 左", "filex left")
                } else {
                    label("filex 左 ●", "filex left *")
                },
                middle,
                None,
                false,
            )?;
            put(
                &mut out,
                0,
                (middle + 2) as isize,
                if self.right_active {
                    label("右 ●", "right *")
                } else {
                    label("右", "right")
                },
                tree,
                None,
                false,
            )?;
        } else {
            draw_tree(
                &mut out,
                &mut self.pane,
                TreeArea {
                    x: 0,
                    width: tree,
                    mirrored: false,
                },
                body,
                true,
            )?;
        }
        if panel > 0 {
            for y in 3..h - 2 {
                put(
                    &mut out,
                    y,
                    tree as isize,
                    label("│", "|"),
                    w,
                    Some(&self.pane.browser.conf.line_color),
                    false,
                )?
            }
            let path = self.selection_path();
            if self.meta_path.as_ref() != Some(&path) {
                self.meta = files::dashboard(
                    &path,
                    &self.pane.browser.root.borrow().path,
                    &self.pane.browser.conf,
                );
                self.meta_path = Some(path)
            }
            put(
                &mut out,
                3,
                (tree + 2) as isize,
                &ui_format!(
                    "Dashboard · q 收起 · {} / {} 调宽",
                    "Dashboard - q close - {} / {} width",
                    self.pane.browser.conf.keys["dashboard_narrow"],
                    self.pane.browser.conf.keys["dashboard_widen"]
                ),
                w,
                None,
                false,
            )?;
            for (i, line) in self.meta.lines().take(body - 1).enumerate() {
                put(&mut out, 4 + i, (tree + 2) as isize, line, w, None, false)?
            }
        }
        let selected = self.pane.browser.selected.borrow();
        let detail = if let Some(target) = &self.pane.link_target {
            ui_format!(
                "链接目标: {} · {} 返回链接 · x 进入目标目录",
                "Link target: {} - {} return - x exit to target folder",
                target.display(),
                if self.right_active { "→" } else { "←" }
            )
        } else if !selected.error.is_empty() {
            selected.error.clone()
        } else if selected.omitted > 0 {
            ui_format!(
                "还有 {} 项未显示；受 max_entries / max_loaded_nodes 限制",
                "{} entries omitted (max_entries / max_loaded_nodes)",
                selected.omitted
            )
        } else {
            if selected.git == crate::git_status::State::None {
                selected.path.to_string_lossy().into_owned()
            } else {
                format!(
                    "{} | Git: {}",
                    selected.path.display(),
                    selected.git.label()
                )
            }
        };
        put(
            &mut out,
            h - 2,
            1,
            if self.message.is_empty() {
                &detail
            } else {
                &self.message
            },
            w,
            None,
            false,
        )?;
        let footer = if let Some(pending) = &self.pending {
            format!(
                "{} {} items: arrows choose | Enter execute | t path | + folder | Esc cancel",
                if pending.moving { "MOVE" } else { "COPY" },
                pending.sources.len()
            )
        } else if terminal_profile::get().ascii {
            format!(
                "{} {}quit ?help Enter:expand/file {}menu {}info y:path {}fold {}rename {}dual {}switch",
                if self.right_active {
                    "Up/Down peers Left child Right parent"
                } else {
                    "Up/Down peers Left parent Right child"
                },
                self.pane.browser.conf.keys["quit"],
                self.pane.browser.conf.keys["open_with"],
                self.pane.browser.conf.keys["dashboard"],
                self.pane.browser.conf.keys["collapse"],
                self.pane.browser.conf.keys["rename"],
                self.pane.browser.conf.keys["toggle_dual"],
                self.pane.browser.conf.keys["switch_pane"]
            )
        } else {
            format!(
                "{} {}收起 {}重命名 Ctrl+Alt+C/y路径 Enter展开/文件打开 {}菜单 {}信息 {}退出 {}双栏 {}切换 ?帮助",
                if self.right_active {
                    "↑↓ 同层循环 → 父级 ← 展开"
                } else {
                    self.pane.browser.conf.arrow_hint()
                },
                self.pane.browser.conf.keys["collapse"],
                self.pane.browser.conf.keys["rename"],
                self.pane.browser.conf.keys["open_with"],
                self.pane.browser.conf.keys["dashboard"],
                self.pane.browser.conf.keys["quit"],
                self.pane.browser.conf.keys["toggle_dual"],
                self.pane.browser.conf.keys["switch_pane"]
            )
        };
        let footer = if self.pending.is_none() {
            format!("Space:mark [{}] | {footer}", self.marked.len())
        } else {
            footer
        };
        put(&mut out, h - 1, 1, &footer, w, None, false)?;
        queue!(out, SetAttribute(Attribute::Reset), ResetColor).map_err(ioerr)?;
        out.flush().map_err(ioerr)
    }
    fn prompt(&mut self, label: &str, initial: &str) -> Result<Option<String>, String> {
        let mut edit = crate::edit::Edit::new(initial);
        loop {
            self.draw()?;
            let (w, h) = terminal::size().map_err(ioerr)?;
            let prefix = format!("{label}: ");
            let full = format!("{prefix}{}", edit.text);
            let cursor = prefix.len() + edit.cursor;
            let mut start = 0;
            let available = w.saturating_sub(2).max(1) as usize;
            while clean(&full[start..cursor]).width() >= available {
                start += full[start..].chars().next().unwrap().len_utf8();
            }
            let value = clip(&clean(&full[start..]), available);
            let column = clean(&full[start..cursor]).width() as u16;
            let mut out = io::stdout().lock();
            default_style(&mut out)?;
            queue!(
                out,
                MoveTo(0, h - 2),
                Clear(ClearType::CurrentLine),
                Show,
                Print(&value),
                MoveTo(column, h - 2)
            )
            .map_err(ioerr)?;
            out.flush().map_err(ioerr)?;
            drop(out);
            let key = read_key(self.pane.browser.conf.auto_english)?;
            match key.as_str() {
                "enter" => {
                    execute!(io::stdout(), Hide).map_err(ioerr)?;
                    return Ok(Some(edit.text));
                }
                "escape" => {
                    execute!(io::stdout(), Hide).map_err(ioerr)?;
                    return Ok(None);
                }
                "\x03" => return Err("已取消".into()),
                _ => edit.key(&key, &self.clipboard),
            }
        }
    }
    fn pager(&mut self, title: &str, content: &str) -> Result<(), String> {
        let rows: Vec<&str> = content.lines().collect();
        let (mut top, mut left) = (0usize, 0usize);
        loop {
            let (w, h) = terminal::size().map_err(ioerr)?;
            let (w, h) = (w as usize, h as usize);
            let body = h.saturating_sub(3).max(1);
            let mut out = io::stdout().lock();
            clear_screen(&mut out)?;
            put(&mut out, 0, 1, title, w, None, false)?;
            for (i, line) in rows.iter().skip(top).take(body).enumerate() {
                put(&mut out, i + 1, 1 - left as isize, line, w, None, false)?
            }
            put(
                &mut out,
                h - 1,
                1,
                label(
                    "↑↓ 滚动 ←→ 横移 Esc / q / Enter 返回",
                    "Up/Down scroll, Left/Right pan, Esc/q/Enter return",
                ),
                w,
                None,
                false,
            )?;
            out.flush().map_err(ioerr)?;
            drop(out);
            match read_key(self.pane.browser.conf.auto_english)?.as_str() {
                "escape" | "q" | "enter" | "\x03" => return Ok(()),
                "down" | "j" => top = (top + 1).min(rows.len().saturating_sub(body)),
                "up" | "k" => top = top.saturating_sub(1),
                "pagedown" => top = (top + body).min(rows.len().saturating_sub(body)),
                "pageup" => top = top.saturating_sub(body),
                "right" => left += 8,
                "left" => left = left.saturating_sub(8),
                _ => {}
            }
        }
    }
    fn choose(
        &mut self,
        title: &str,
        options: &[(Option<String>, String)],
    ) -> Result<Option<usize>, String> {
        let mut selected: usize = 0;
        loop {
            let (w, h) = terminal::size().map_err(ioerr)?;
            let (w, h) = (w as usize, h as usize);
            let body = h.saturating_sub(4).max(1);
            let first = selected.saturating_sub(body - 1);
            let mut out = io::stdout().lock();
            clear_screen(&mut out)?;
            put(&mut out, 0, 1, title, w, None, false)?;
            for (i, (_, label)) in options.iter().enumerate().skip(first).take(body) {
                put(
                    &mut out,
                    i - first + 2,
                    2,
                    &format!("{}. {label}", i + 1),
                    w,
                    None,
                    i == selected,
                )?
            }
            put(
                &mut out,
                h - 1,
                1,
                label(
                    "↑↓ 选择 Enter 确认 Esc 取消",
                    "Up/Down select, Enter confirm, Esc cancel",
                ),
                w,
                None,
                false,
            )?;
            out.flush().map_err(ioerr)?;
            drop(out);
            match read_key(self.pane.browser.conf.auto_english)?.as_str() {
                "escape" | "\x03" => return Ok(None),
                "enter" => return Ok(Some(selected)),
                "down" | "j" => selected = (selected + 1) % options.len(),
                "up" | "k" => selected = (selected + options.len() - 1) % options.len(),
                _ => {}
            }
        }
    }
    fn git_menu(&mut self) -> Result<(), String> {
        use crate::git_actions::{self, Action};
        let selected = self.selection_path();
        let root = git_actions::repository(&selected)?;
        let actions = [Action::Publish, Action::Add, Action::Commit, Action::Push];
        let options = [
            "Add ALL + commit + push",
            "Add selected file/folder",
            "Commit staged changes",
            "Push",
        ]
        .into_iter()
        .map(|name| (None, name.into()))
        .collect::<Vec<_>>();
        let Some(index) = self.choose(
            &format!("Git repository: {}", clean(&root.to_string_lossy())),
            &options,
        )?
        else {
            return Ok(());
        };
        let action = actions[index];
        let message = if action.needs_message() {
            let Some(message) = self.prompt("Git commit message", "")? else {
                return Ok(());
            };
            if message.trim().is_empty() {
                return Ok(());
            }
            message
        } else {
            String::new()
        };
        let commands = git_actions::commands(&root, &selected, action, &message)?;
        if !self.confirm(&format!(
            "{} | {}",
            action.description(),
            clean(&root.to_string_lossy())
        ))? {
            return Ok(());
        }
        terminal::disable_raw_mode().map_err(ioerr)?;
        Guard::leave()?;
        let mut result = Ok(());
        for mut command in commands {
            println!("> {command:?}");
            match command.status() {
                Ok(status) if status.success() => {}
                Ok(status) => {
                    result = Err(format!("Git stopped: {status}; completed steps are kept"));
                    break;
                }
                Err(error) => {
                    result = Err(format!("Cannot run git: {error}"));
                    break;
                }
            }
        }
        println!(
            "\n{} - Enter to return to filex",
            if result.is_ok() {
                "Git operation completed"
            } else {
                "Git operation failed (later steps skipped)"
            }
        );
        terminal::enable_raw_mode().map_err(ioerr)?;
        loop {
            match read_key(self.pane.browser.conf.auto_english) {
                Ok(key) if ["enter", "escape", "\x03"].contains(&key.as_str()) => break,
                Ok(_) => {}
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        Guard::enter()?;
        self.refresh_panes();
        result?;
        self.message = "Git operation completed; tree status refreshed".into();
        Ok(())
    }
    fn open(&mut self, method: &str) -> Result<(), String> {
        let path = self.selection_path();
        let cli = !terminal_profile::get().desktop;
        if method == "@preview" {
            return self.pager(
                label("文件预览", "File preview"),
                &files::preview(&path, self.pane.browser.conf.preview_bytes),
            );
        }
        if cli
            && path.is_dir()
            && self.pane.link_target.is_none()
            && !self
                .pane
                .browser
                .conf
                .openers
                .get(method)
                .is_some_and(|o| o.terminal)
        {
            let node = self.pane.browser.selected.clone();
            if node.borrow().expanded {
                node.borrow_mut().expanded = false;
            } else {
                self.pane.browser.expand(&node, None);
            }
            return Ok(());
        }
        let method = if cli {
            let Some(id) = files::cli_method(&self.pane.browser.conf, method) else {
                return self.pager(
                    label("文件预览", "File preview"),
                    &files::preview(&path, self.pane.browser.conf.preview_bytes),
                );
            };
            id.to_owned()
        } else {
            method.to_owned()
        };
        let method = method.as_str();
        if self.pane.browser.conf.openers[method].copy_path {
            self.clipboard = std::path::absolute(&path)
                .map_err(ioerr)?
                .to_string_lossy()
                .into_owned();
        }
        let (mut command, terminal) = files::open_command(&path, &self.pane.browser.conf, method)?;
        if terminal
            && self.pane.browser.conf.tmux_right
            && let Some(pane) = tmux::open_in_right_pane(&command)
        {
            self.meta_path = None;
            self.message = ui_format!(
                "已用 {} 在右侧 pane {pane} 打开",
                "Opened with {} in right pane {pane}",
                self.pane.browser.conf.openers[method].label
            );
            return Ok(());
        }
        if terminal {
            terminal::disable_raw_mode().map_err(ioerr)?;
            Guard::leave()?;
            let mut result = command.status().map_err(ioerr);
            if self.pane.browser.conf.openers[method].pause_on_exit
                && let Ok(status) = &result
            {
                println!(
                    "\n{} ({status}) - {}",
                    label("程序结束", "Program exited"),
                    label("按 Enter 返回 filex", "Enter to return to filex")
                );
                terminal::enable_raw_mode().map_err(ioerr)?;
                loop {
                    match read_key(self.pane.browser.conf.auto_english) {
                        Ok(key) if ["enter", "escape", "\x03"].contains(&key.as_str()) => break,
                        Ok(_) => {}
                        Err(error) => {
                            result = Err(error);
                            break;
                        }
                    }
                }
            }
            terminal::enable_raw_mode().map_err(ioerr)?;
            Guard::enter()?;
            result?;
        } else if command.get_args().any(|arg| {
            arg.to_string_lossy()
                .ends_with("/scripts/open_markdown.mjs")
        }) {
            let url = files::start_markdown(&mut command)?;
            self.message = format!("Markdown 预览已就绪: {url}");
            self.meta_path = None;
            return Ok(());
        } else {
            command
                .stdin(std::process::Stdio::null())
                .spawn()
                .map_err(ioerr)?;
        }
        self.meta_path = None;
        self.message = ui_format!(
            "已用 {} 打开",
            "Opened with {}",
            self.pane.browser.conf.openers[method].label
        );
        Ok(())
    }
    fn choose_open(&mut self) -> Result<(), String> {
        let node = self.pane.browser.selected.clone();
        let path = self.selection_path();
        let dir = if self.pane.link_target.is_some() {
            path.is_dir()
        } else {
            node.borrow().dir
        };
        let mut default = self
            .pane
            .browser
            .conf
            .rule(&path, &self.pane.browser.conf.defaults, dir)
            .cloned()
            .unwrap_or("system".into());
        if !self
            .pane
            .browser
            .conf
            .openers
            .get(&default)
            .is_some_and(|o| o.supports(&path))
        {
            default = "system".into();
        }
        if !terminal_profile::get().desktop {
            default = files::cli_method(&self.pane.browser.conf, &default)
                .unwrap_or("@preview")
                .to_owned();
        }
        if default != "@preview"
            && !self
                .pane
                .browser
                .conf
                .openers
                .get(&default)
                .is_some_and(|o| o.supports(&path))
        {
            default = "@preview".into();
        }
        let mut ids = vec![default.clone()];
        ids.extend(
            self.pane
                .browser
                .conf
                .openers
                .keys()
                .filter(|id| {
                    **id != default
                        && self.pane.browser.conf.openers[*id].supports(&path)
                        && (terminal_profile::get().desktop
                            || self.pane.browser.conf.openers[*id].terminal)
                })
                .cloned(),
        );
        let mut options: Vec<(Option<String>, String)> = ids
            .iter()
            .map(|id| {
                if id == "@preview" {
                    return (Some(id.clone()), "Preview [default]".into());
                }
                let opener = &self.pane.browser.conf.openers[id];
                (
                    Some(id.clone()),
                    format!(
                        "{} ({id}){}{}",
                        opener.label,
                        if id == &default {
                            label(" [默认]", " [default]")
                        } else {
                            ""
                        },
                        if files::available(&opener.command[0]) {
                            ""
                        } else {
                            label(" [未安装]", " [not installed]")
                        }
                    ),
                )
            })
            .collect();
        if dir && self.pane.link_target.is_none() {
            options.insert(
                0,
                (
                    None,
                    label("fileX 展开 / 收起目录", "Expand / collapse folder").into(),
                ),
            );
        }
        if let Some(index) = self.choose(
            &ui_format!("打开方式: {}", "Open with: {}", path.display()),
            &options,
        )? {
            if let Some(method) = &options[index].0 {
                self.open(method)?
            } else if node.borrow().expanded {
                node.borrow_mut().expanded = false
            } else {
                self.pane.browser.expand(&node, None)
            }
        }
        Ok(())
    }
    fn confirm(&mut self, title: &str) -> Result<bool, String> {
        loop {
            self.draw()?;
            let (width, height) = terminal::size().map_err(ioerr)?;
            let mut out = io::stdout().lock();
            default_style(&mut out)?;
            queue!(
                out,
                MoveTo(0, height.saturating_sub(2)),
                Clear(ClearType::CurrentLine)
            )
            .map_err(ioerr)?;
            put(
                &mut out,
                height.saturating_sub(2) as usize,
                0,
                title,
                width as usize,
                None,
                false,
            )?;
            put(
                &mut out,
                height.saturating_sub(1) as usize,
                0,
                label(
                    "再次按 Enter 确认 · Esc 取消",
                    "Press Enter again to confirm, Esc cancel",
                ),
                width as usize,
                None,
                false,
            )?;
            out.flush().map_err(ioerr)?;
            drop(out);
            match read_key(self.pane.browser.conf.auto_english)?.as_str() {
                "enter" => return Ok(true),
                "escape" | "\x03" => return Ok(false),
                _ => {}
            }
        }
    }
    fn operation(&mut self, action: &str) -> Result<(), String> {
        if matches!(action, "copy" | "move") {
            return self.begin_transfer(action == "move");
        }
        if action == "trash" && !self.marked.is_empty() {
            let sources = self.sources()?;
            if sources.iter().any(|p| self.protected_root(p)) {
                return Err("Cannot trash the displayed root".into());
            }
            if !self.confirm(&format!("Trash {} selected items?", sources.len()))? {
                return Ok(());
            }
            for path in sources {
                if let Err(error) = files::trash(&path) {
                    self.refresh_panes();
                    return Err(error);
                }
                self.marked.retain(|p| !p.starts_with(&path));
            }
            self.refresh_panes();
            return Ok(());
        }
        if self.pane.link_target.is_some() {
            return Err(ui_format!(
                "请按 {} 选回链接，或按 f 进入目标目录后操作文件",
                "{} returns to link; f enters target folder for file operations",
                if self.right_active { "→" } else { "←" }
            ));
        }
        let source = self.pane.browser.selected.borrow().path.clone();
        if Rc::ptr_eq(&self.pane.browser.selected, &self.pane.browser.root)
            && ["rename", "move", "trash"].contains(&action)
        {
            return Err(ui_format!(
                "不能操作当前根目录；先按 {} 引入其父目录",
                "Cannot modify root; use {} to introduce its parent",
                if self.right_active { "→" } else { "←" }
            ));
        }
        let dest = match action {
            "trash" => {
                if !self.confirm(&ui_format!(
                    "送入回收站 {}？",
                    "Trash {}?",
                    source.display()
                ))? {
                    return Ok(());
                }
                files::trash(&source)?;
                source.parent().unwrap_or(Path::new("/")).to_path_buf()
            }
            _ => {
                let label = match action {
                    "new_file" => label("新文件名称", "New file name"),
                    "new_folder" => label("新目录名称", "New folder name"),
                    "rename" => label("新名称", "New name"),
                    "copy" => label(
                        "复制到完整目标路径（含名称）",
                        "Copy to full path (including name)",
                    ),
                    _ => label(
                        "移动到完整目标路径（含名称）",
                        "Move to full path (including name)",
                    ),
                };
                let initial = if ["copy", "move"].contains(&action)
                    && let Some(other) = &self.other
                {
                    let path = other
                        .link_target
                        .clone()
                        .unwrap_or_else(|| other.browser.selected.borrow().path.clone());
                    files::exit_directory_path(&path)?
                        .join(source.file_name().ok_or("源名称无效")?)
                        .to_string_lossy()
                        .into_owned()
                } else if action == "rename" {
                    source
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                } else {
                    String::new()
                };
                let Some(value) = self.prompt(label, &initial)? else {
                    return Ok(());
                };
                if value.is_empty() {
                    return Ok(());
                }
                match action {
                    "new_file" | "new_folder" => {
                        let target = self.pane.browser.target();
                        let path = files::create(&target, &value, action == "new_folder")?;
                        let parent = if self.pane.browser.selected.borrow().dir {
                            self.pane.browser.selected.clone()
                        } else {
                            self.pane
                                .browser
                                .selected
                                .borrow()
                                .parent
                                .upgrade()
                                .unwrap()
                        };
                        self.pane.browser.expand(&parent, Some(&path));
                        path
                    }
                    "rename" => {
                        files::name_valid(&value)?;
                        let dest = source.parent().unwrap().join(value);
                        files::rename(&source, &dest)?;
                        dest
                    }
                    _ => {
                        let dest = files::destination(&source, &value);
                        files::transfer(&source, &dest, action == "move")?;
                        dest
                    }
                }
            }
        };
        self.pane.browser.refresh();
        if let Some(other) = &mut self.other {
            other.browser.conf.max_loaded = self.node_budget;
            other.browser.refresh();
            if let Some(parent) = other
                .browser
                .visible()
                .into_iter()
                .find(|node| node.borrow().path == dest.parent().unwrap_or(Path::new("/")))
                && parent.borrow().expanded
            {
                other.browser.expand(&parent, Some(&dest));
            }
            other.link_target = None;
        }
        if let Some(node) = self
            .pane
            .browser
            .visible()
            .into_iter()
            .find(|n| n.borrow().path == dest)
        {
            self.pane.browser.selected = node
        }
        self.meta_path = None;
        self.message = ui_format!("完成: {}", "Done: {}", dest.display());
        Ok(())
    }
    fn action(&mut self, key: &str) -> Result<bool, String> {
        self.rebudget();
        if self.pending.is_some() {
            if key == "escape" {
                self.cancel_transfer();
                return Ok(false);
            }
            if key == "enter" {
                self.commit_transfer()?;
                return Ok(false);
            }
            if key == "t" {
                self.transfer_path()?;
                return Ok(false);
            }
            if key == "+" {
                self.create_destination()?;
                return Ok(false);
            }
        }
        if key == " " && self.pending.is_none() {
            let node = self.pane.browser.selected.borrow();
            if !Rc::ptr_eq(&self.pane.browser.selected, &self.pane.browser.root)
                && !node.create_entry
            {
                let path = node.path.clone();
                drop(node);
                if !self.marked.remove(&path) {
                    if self.marked.len() >= self.node_budget {
                        return Err("Selection limit reached (max_loaded_nodes)".into());
                    }
                    self.marked.insert(path);
                }
            }
            return Ok(false);
        }
        if key == "escape" {
            self.marked.clear();
            return Ok(false);
        }
        let navigation_key = if self.right_active {
            match key {
                "left" => "right",
                "right" => "left",
                _ => key,
            }
        } else {
            key
        };
        let action = self
            .pane
            .browser
            .conf
            .arrow_action(navigation_key)
            .unwrap_or_else(|| match key {
                " " => "expand",
                "enter"
                    if self.pane.link_target.is_none()
                        && self.pane.browser.selected.borrow().dir =>
                {
                    "expand_folder"
                }
                "enter" => "open_with",
                "delete" => "trash",
                "f5" | "ctrl+f5" => "refresh",
                "\x06" | "/" => "search",
                "\x0c" => "goto",
                _ => self.pane.browser.conf.action(key).unwrap_or(if key == "y" {
                    "copy_path"
                } else if key == "\x03" {
                    "quit"
                } else {
                    ""
                }),
            })
            .to_string();
        if self.pending.is_some()
            && ![
                "up",
                "down",
                "expand",
                "collapse",
                "parent",
                "expand_all",
                "collapse_all",
                "hidden",
                "refresh",
                "goto",
                "search",
                "next_match",
                "previous_match",
                "switch_pane",
                "quit",
            ]
            .contains(&action.as_str())
            && !["home", "end", "pageup", "pagedown", "H", "L", "resize"].contains(&key)
        {
            return Ok(false);
        }
        if matches!(
            action.as_str(),
            "up" | "down"
                | "collapse"
                | "expand_all"
                | "collapse_all"
                | "hidden"
                | "refresh"
                | "config"
                | "goto"
                | "search"
                | "next_match"
                | "previous_match"
        ) || matches!(key, "home" | "end" | "pageup" | "pagedown" | "escape")
        {
            self.pane.link_target = None;
        }
        match action.as_str() {
            "toggle_dual" => self.toggle_dual()?,
            "switch_pane" => self.switch_pane(),
            "quit" => return Ok(true),
            "exit_here" => {
                self.exit_directory = Some(files::exit_directory(&self.selection_path())?);
                return Ok(true);
            }
            "dashboard" => {
                self.dashboard = !self.dashboard;
                self.meta_path = None;
                if self.dashboard
                    && (terminal::size().map_err(ioerr)?.0 as usize)
                        < if self.other.is_some() { 110 } else { 70 }
                {
                    self.message = ui_format!(
                        "Dashboard 需要至少 {} 列",
                        "Dashboard needs at least {} columns",
                        if self.other.is_some() { 110 } else { 70 }
                    )
                }
            }
            "dashboard_narrow" | "dashboard_widen" => {
                let width = terminal::size().map_err(ioerr)?.0 as usize;
                if width < if self.other.is_some() { 110 } else { 70 } {
                    self.message = ui_format!(
                        "Dashboard 需要至少 {} 列",
                        "Dashboard needs at least {} columns",
                        if self.other.is_some() { 110 } else { 70 }
                    );
                } else {
                    let current = self.pane.browser.conf.dashboard_columns(width);
                    let desired = if action == "dashboard_narrow" {
                        current.saturating_sub(4)
                    } else {
                        current + 4
                    };
                    self.pane.browser.conf.dashboard_width = desired.clamp(
                        20,
                        (width - if self.other.is_some() { 77 } else { 38 }).min(240),
                    );
                    self.dashboard = true;
                    self.message = ui_format!(
                        "信息面板宽度: {} 列",
                        "Dashboard width: {} columns",
                        self.pane.browser.conf.dashboard_width
                    );
                }
            }
            "up" => self.pane.browser.step(-1),
            "down" => self.pane.browser.step(1),
            "expand_folder" => {
                let node = self.pane.browser.selected.clone();
                if !node.borrow().expanded {
                    self.pane.browser.expand(&node, None);
                }
            }
            "expand" => {
                if self.pane.link_target.is_none() {
                    if self.pane.browser.selected.borrow().link {
                        let target = files::link_target(&self.pane.browser.selected.borrow().path)?;
                        self.pane.link_target = Some(target);
                        self.meta_path = None;
                    } else {
                        self.pane.browser.right();
                    }
                }
            }
            "collapse" => self.pane.browser.left(),
            "expand_all" => {
                self.message = label("正在展开…", "Expanding...").into();
                self.draw()?;
                self.message = self.pane.browser.expand_all()?
            }
            "collapse_all" => self.message = self.pane.browser.collapse_all()?,
            "parent" => {
                if self.pane.link_target.take().is_none() {
                    self.message = self.pane.browser.back()?;
                    self.pane.left = 0;
                    self.pane.top = 0;
                }
            }
            "focus_root" => {
                if let Some(target) = &self.pane.link_target {
                    let folder = files::exit_directory_path(target)?;
                    self.pane.browser.set_root(&folder.to_string_lossy())?;
                    self.pane.link_target = None;
                    self.message = ui_format!("根目录: {}", "Root: {}", folder.display());
                } else {
                    self.message = self.pane.browser.focus_root();
                }
                self.pane.query.clear();
                self.meta_path = None;
                self.pane.left = 0;
                self.pane.top = 0;
            }
            "hidden" => {
                self.pane.browser.conf.hidden = !self.pane.browser.conf.hidden;
                self.pane.browser.refresh()
            }
            "git_menu" => self.git_menu()?,
            "run_background" => {
                let directory = self.selection_path();
                if !directory.is_dir() {
                    return Err("Select a folder to run a background command".into());
                }
                if let Some(command) = self.prompt(
                    label("在此目录后台执行命令", "Run background command in folder"),
                    "",
                )? && !command.trim().is_empty()
                {
                    let job = crate::jobs::start(&directory, &command)?;
                    self.message = format!(
                        "Started PID {} | log: {}",
                        job.pid,
                        clean(&job.log.to_string_lossy())
                    );
                }
            }
            "copy_path" => {
                let path = self.selection_path();
                self.clipboard = std::path::absolute(&path)
                    .map_err(ioerr)?
                    .to_string_lossy()
                    .into_owned();
                if terminal_profile::get().desktop {
                    files::copy_path(&path)?;
                    self.message = "已复制绝对路径".into();
                } else {
                    self.message = format!(
                        "Path copied (Ctrl+V in prompts): {}",
                        clean(&self.clipboard)
                    );
                }
            }
            "refresh" => {
                self.pane.browser.refresh();
                self.meta_path = None
            }
            "config" => {
                self.pane.browser.conf = Config::load(&self.config_path)?;
                self.node_budget = self.pane.browser.conf.max_loaded;
                if let Some(other) = &mut self.other {
                    other.browser.conf = self.pane.browser.conf.clone();
                    other.browser.conf.max_loaded = self.node_budget;
                    other.link_target = None;
                    other.browser.refresh();
                }
                self.rebudget();
                self.pane.browser.refresh();
                self.meta_path = None;
                self.message = label("配置已重新读取", "Config reloaded").into()
            }
            "goto" => {
                let path = self
                    .pane
                    .browser
                    .root
                    .borrow()
                    .path
                    .to_string_lossy()
                    .into_owned();
                if let Some(value) = self.prompt(label("进入目录", "Go to directory"), &path)?
                    && !value.is_empty()
                {
                    self.pane.browser.set_root(&value)?;
                    self.pane.left = 0;
                    self.pane.top = 0
                }
            }
            "search" => {
                let query = self.pane.query.clone();
                if let Some(value) = self.prompt(
                    label(
                        "搜索根目录（含未展开目录）",
                        "Search root (including closed folders)",
                    ),
                    &query,
                )? {
                    self.pane.query = value;
                    if !self.pane.query.is_empty() {
                        self.message = label("正在搜索…", "Searching...").into();
                        self.draw()?;
                        self.pane.browser.search_key = None;
                        self.message = self.pane.browser.find_next(&self.pane.query, 1)?
                    }
                }
            }
            "next_match" | "previous_match" => {
                if !self.pane.query.is_empty() {
                    self.message = self.pane.browser.find_next(
                        &self.pane.query,
                        if action == "next_match" { 1 } else { -1 },
                    )?
                }
            }
            "open_with" => self.choose_open()?,
            "open" => {
                let path = self.selection_path();
                let dir = if self.pane.link_target.is_some() {
                    path.is_dir()
                } else {
                    self.pane.browser.selected.borrow().dir
                };
                let method = self
                    .pane
                    .browser
                    .conf
                    .rule(&path, &self.pane.browser.conf.defaults, dir)
                    .cloned()
                    .unwrap_or("system".into());
                self.open(&method)?
            }
            "link_info" => {
                let path = self.selection_path();
                let text = files::dashboard(
                    &path,
                    &self.pane.browser.root.borrow().path,
                    &self.pane.browser.conf,
                );
                self.pager(label("文件 / 链接信息", "File / link information"), &text)?;
            }
            "preview" => {
                let path = self.selection_path();
                let text = files::preview(&path, self.pane.browser.conf.preview_bytes);
                self.pager(&path.to_string_lossy(), &text)?
            }
            "new_file" | "new_folder" | "rename" | "copy" | "move" | "trash" => {
                self.operation(&action)?
            }
            "help" => {
                let mut text = self
                    .pane
                    .browser
                    .conf
                    .keys
                    .iter()
                    .map(|(action, key)| format!("{key:12} {action}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                if terminal_profile::get().ascii {
                    text.push_str("\n\nUp/Down (k/j): all visible peers at the same depth; wrap around.\nRight (l): expand / enter child; Left: parent (mirrored on right pane).\nh collapse; E/C expand/collapse all; . hidden files; f focus root.\na / Ctrl+N new file; A new folder (Ctrl+Shift+N may equal Ctrl+N on TTY).\nr rename; c/m copy/move; Delete/D trash, Enter again confirms, Esc cancels.\ny / Ctrl+Alt+C copy absolute path; Ctrl+V paste it in input prompts.\nPaths use the Wayland clipboard and an internal clipboard.\nx exit to selected folder (file: parent); Q / Ctrl+C quit.\n! run a shell command in the selected folder in background (PID/log shown).\nG Git menu: add selected / commit staged / push / add ALL+commit+push.\nEnter expands folders (cursor stays); files: open with. w menu; o default; p preview; q dashboard; I link information.\nHeadless defaults: Nano, Neovim, Vim, then built-in preview; no automatic script execution.\nPython / Rscript / Bash can be explicitly selected to run scripts.\nCtrl+F / / search; n/N matches; g / Ctrl+L change root.\nCtrl+R / F5 refresh; comma reload config; [ / ] dashboard width.\nv toggle mirrored panes; Tab switch; c/m switch to the other pane to choose a destination.\nH/L pan; PgUp/PgDn page peers; Ctrl+U clear input.\nTerminal openers reuse an idle pane on the right in tmux.\nSpace marks/unmarks files/folders; Esc clears marks. c/m: choose destination with arrows, Enter executes, + creates folder; t types a destination file/directory path; Esc cancels. Existing regular files require Enter confirmation to replace; folders/links are not replaced. Rename prompts: Left/Right/Home/End, Delete/Backspace edit at cursor.");
                } else {
                    text.push_str("\n\n横向：↑↓ 同层跨父目录，首尾循环 · → 展开 / 进入子级 · ← 返回父级\n根节点的返回父级键引入上级目录。\nh 收起 · [ / ] 缩窄 / 加宽 q 信息面板（每次 4 列）\nx 退出并将启动 terminal 切到选中目录（文件使用父目录）\nCtrl+N 新建文件 · Ctrl+Shift+N 新建目录（a/A 也可）\n↗ 软链接 · H×N 硬链接数 · q 查看信息 · I 查看完整链接路径\nr / Shift+R 重命名 · Ctrl+R / F5 / Ctrl+F5 刷新\nCtrl+Alt+C / y 复制选中项目的绝对路径；输入框 Ctrl+V 粘贴\nf 当前目录设为根并刷新\nEnter 展开目录（光标保留）/ 文件选择打开方式 · w 打开菜单 · o 默认程序\nCtrl+F / / 搜索 · q 右侧时间 / 大小\nDelete / D 移到回收站，再次按 Enter 确认\nv 双栏开关 · Tab 切换操作侧 · 右栏 ← 展开 / → 父级\n双栏 c/m 切到另一侧，方向键选目标目录，Enter 执行 / Esc 取消\nE/C 展开 / 收起全部\nH/L 横移 · PgUp/PgDn 同级翻页 · Ctrl+U 清空输入\n终端程序优先使用 tmux 右侧空闲 pane。\nSpace 多选，Esc 清空；c/m 后方向键选目标目录，Enter 执行，+ 新建目录，Esc 取消。不覆盖已有目标。输入框支持左右/Home/End及光标处增删。\n缓存达到上限时请收起部分分支或增大 max_loaded_nodes。");
                }
                self.pager(label("filex 快捷键", "filex keys"), &text)?
            }
            _ => match key {
                "escape" => {
                    self.pane.query.clear();
                    self.message.clear()
                }
                "\x03" => return Ok(true),
                "H" | "L" if self.right_active => {
                    self.pane.left = if key == "H" {
                        self.pane.left + 12
                    } else {
                        self.pane.left.saturating_sub(12)
                    };
                    self.pane.focus = false
                }
                "H" => {
                    self.pane.left = self.pane.left.saturating_sub(12);
                    self.pane.focus = false
                }
                "L" => {
                    self.pane.left += 12;
                    self.pane.focus = false
                }
                "home" => self.pane.browser.selected = self.pane.browser.root.clone(),
                "end" => {
                    self.pane.browser.selected = self.pane.browser.visible().last().unwrap().clone()
                }
                "pageup" => self
                    .pane
                    .browser
                    .step(-(terminal::size().map_err(ioerr)?.1.saturating_sub(5) as isize)),
                "pagedown" => self
                    .pane
                    .browser
                    .step(terminal::size().map_err(ioerr)?.1.saturating_sub(5) as isize),
                _ => {}
            },
        }
        Ok(false)
    }
}
pub fn run(browser: Browser, path: PathBuf) -> Result<Option<PathBuf>, String> {
    let _signals = terminal_profile::Signals::new().map_err(ioerr)?;
    let profile = terminal_profile::get();
    let _input = crate::input::EnglishGuard::new(browser.conf.auto_english && profile.desktop);
    let _host =
        crate::terminal_host::KonsoleGuard::new(browser.conf.konsole_shortcuts && profile.desktop);
    let _tmux_keys =
        crate::terminal_host::TmuxKeys::new(browser.conf.tmux_extended_keys && profile.enhanced);
    let _tmux_focus = crate::terminal_host::TmuxFocus::new(profile.enhanced);
    let _guard = Guard::new()?;
    let dual = browser.conf.dual_pane;
    let budget = browser.conf.max_loaded;
    let mut ui = Ui {
        clipboard: String::new(),
        marked: Default::default(),
        pending: None,
        pane: Pane::new(browser),
        other: None,
        right_active: false,
        node_budget: budget,
        config_path: path,
        dashboard: false,
        meta_path: None,
        meta: String::new(),
        message: String::new(),
        exit_directory: None,
    };
    if dual {
        ui.toggle_dual()?;
    }
    loop {
        ui.draw()?;
        let key = read_key(ui.pane.browser.conf.auto_english)?;
        ui.pane.focus = true;
        if !["H", "L", "resize", "git_refresh"].contains(&key.as_str()) {
            ui.message.clear()
        }
        match ui.action(&key) {
            Ok(true) => break,
            Ok(false) => {}
            Err(e) => ui.message = ui_format!("操作失败: {}", "Operation failed: {}", clean(&e)),
        }
    }
    Ok(ui.exit_directory)
}

#[cfg(test)]
mod dual_tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "filex-dual-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            for i in 0..8 {
                let folder = root.join(format!("branch{i}"));
                fs::create_dir_all(&folder).unwrap();
                for j in 0..30 {
                    fs::write(folder.join(format!("file{j}.txt")), "test").unwrap();
                }
            }
            Self(fs::canonicalize(root).unwrap())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn ui(root: &Path) -> Ui {
        let conf = Config {
            max_loaded: 100,
            ..Config::default()
        };
        Ui {
            clipboard: String::new(),
            marked: Default::default(),
            pending: None,
            pane: Pane::new(Browser::new(root, conf).unwrap()),
            other: None,
            right_active: false,
            node_budget: 100,
            config_path: root.join("settings.conf"),
            dashboard: false,
            meta_path: None,
            meta: String::new(),
            message: String::new(),
            exit_directory: None,
        }
    }
    #[test]
    fn mirror_keeps_text_readable_and_clips_within_its_region() {
        let area = TreeArea {
            x: 41,
            width: 40,
            mirrored: true,
        };
        let mut out = Vec::new();
        area.paint(&mut out, 3, 4, "▸ 文件", None, false, false, false)
            .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\x1b[4;72H"));
        assert!(text.contains("◂ 文件"));
        let mut out = Vec::new();
        area.paint(&mut out, 3, 2, "├", None, false, false, false)
            .unwrap();
        assert!(String::from_utf8(out).unwrap().contains('┤'));
        let mut out = Vec::new();
        area.paint(&mut out, 3, 100, "outside", None, false, false, false)
            .unwrap();
        assert!(out.is_empty());
    }
    #[test]
    fn enter_expands_folders_without_moving_or_closing_other_branches() {
        let f = Fixture::new();
        let mut ui = ui(&f.0);
        ui.action("right").unwrap();
        let first = ui.pane.browser.selected.clone();
        ui.action("enter").unwrap();
        assert!(first.borrow().expanded);
        assert!(!first.borrow().children.is_empty());
        assert!(Rc::ptr_eq(&ui.pane.browser.selected, &first));
        let child = first.borrow().children[0].clone();
        let loaded = ui.pane.browser.loaded();
        ui.action("enter").unwrap();
        assert!(Rc::ptr_eq(&ui.pane.browser.selected, &first));
        assert!(Rc::ptr_eq(&first.borrow().children[0], &child));
        assert_eq!(ui.pane.browser.loaded(), loaded);
        ui.action("down").unwrap();
        let second = ui.pane.browser.selected.clone();
        ui.action("enter").unwrap();
        assert!(first.borrow().expanded && second.borrow().expanded);
        assert!(Rc::ptr_eq(&ui.pane.browser.selected, &second));
        ui.action("v").unwrap();
        ui.action("\t").unwrap();
        ui.action("right").unwrap(); // Mirrored pane: introduce parent.
        ui.action("left").unwrap(); // Select a folder at the parent level.
        let mirrored = ui.pane.browser.selected.clone();
        ui.action("h").unwrap();
        ui.action("enter").unwrap();
        assert!(ui.right_active && mirrored.borrow().expanded);
        assert!(Rc::ptr_eq(&ui.pane.browser.selected, &mirrored));
        assert!(first.borrow().expanded && second.borrow().expanded);
    }
    #[test]
    fn independent_selection_mirrored_arrows_and_bounded_pane_caches() {
        let f = Fixture::new();
        let mut ui = ui(&f.0);
        ui.action("v").unwrap();
        ui.action("right").unwrap();
        let left = ui.pane.browser.selected.clone();
        ui.action("\t").unwrap();
        assert!(ui.right_active);
        ui.action("left").unwrap();
        let right = ui.pane.browser.selected.clone();
        ui.action("left").unwrap();
        ui.action("left").unwrap();
        assert!(ui.pane.browser.selected.borrow().path.extension().is_some());
        ui.action("right").unwrap();
        assert!(Rc::ptr_eq(&ui.pane.browser.selected, &right));
        ui.action("\t").unwrap();
        assert!(Rc::ptr_eq(&ui.pane.browser.selected, &left));
        ui.rebudget();
        ui.pane.browser.expand_all().unwrap();
        ui.switch_pane();
        ui.pane.browser.expand_all().unwrap();
        assert!(
            ui.pane.browser.loaded() + ui.other.as_ref().unwrap().browser.loaded()
                <= ui.node_budget * 2
        );
        let retained = ui.pane.browser.root.clone();
        ui.toggle_dual().unwrap();
        assert!(ui.other.is_none());
        assert!(!ui.right_active);
        assert!(Rc::ptr_eq(&ui.pane.browser.root, &retained));
        assert_eq!(ui.pane.browser.conf.max_loaded, 100);
    }
    #[test]
    fn marked_batch_transfer_uses_cursor_destination_and_cleans_virtual_entries() {
        let f = Fixture::new();
        let mut ui = ui(&f.0);
        let a = f.0.join("alpha.txt");
        let b = f.0.join("beta.txt");
        fs::write(&a, "alpha").unwrap();
        fs::write(&b, "beta").unwrap();
        ui.pane.browser.refresh();
        for p in [&a, &b] {
            ui.pane.browser.selected = ui
                .pane
                .browser
                .visible()
                .into_iter()
                .find(|n| &n.borrow().path == p)
                .unwrap();
            ui.action(" ").unwrap();
        }
        assert_eq!(ui.marked.len(), 2);
        ui.action("c").unwrap();
        assert!(
            ui.pane
                .browser
                .root
                .borrow()
                .children
                .last()
                .unwrap()
                .borrow()
                .create_entry
        );
        let dest = f.0.join("branch0");
        ui.pane.browser.selected = ui
            .pane
            .browser
            .visible()
            .into_iter()
            .find(|n| n.borrow().path == dest)
            .unwrap();
        ui.action("enter").unwrap();
        assert_eq!(fs::read_to_string(dest.join("alpha.txt")).unwrap(), "alpha");
        assert!(a.exists() && b.exists());
        assert!(ui.pending.is_none() && ui.marked.is_empty());
        assert!(
            !ui.pane
                .browser
                .visible()
                .iter()
                .any(|n| n.borrow().create_entry)
        );
        // Single selection moves through the same cursor workflow.
        ui.pane.browser.selected = ui
            .pane
            .browser
            .visible()
            .into_iter()
            .find(|n| n.borrow().path == a)
            .unwrap();
        ui.action("m").unwrap();
        let dest = f.0.join("branch1");
        ui.pane.browser.selected = ui
            .pane
            .browser
            .visible()
            .into_iter()
            .find(|n| n.borrow().path == dest)
            .unwrap();
        ui.action("enter").unwrap();
        assert!(!a.exists());
        assert!(dest.join("alpha.txt").exists());
    }
    #[test]
    fn preflight_prevents_partial_copy_on_collision_and_escape_cancels() {
        let f = Fixture::new();
        let mut ui = ui(&f.0);
        let a = f.0.join("alpha");
        let b = f.0.join("beta");
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        let dest = f.0.join("branch0");
        fs::create_dir(dest.join("beta")).unwrap();
        fs::write(dest.join("beta/keep"), "original").unwrap();
        ui.marked.extend([a.clone(), b.clone()]);
        ui.action("c").unwrap();
        ui.pane.browser.selected = ui
            .pane
            .browser
            .visible()
            .into_iter()
            .find(|n| n.borrow().path == dest)
            .unwrap();
        assert!(ui.action("enter").is_err());
        assert!(!dest.join("alpha").exists());
        assert_eq!(
            fs::read_to_string(dest.join("beta/keep")).unwrap(),
            "original"
        );
        ui.action("escape").unwrap();
        assert!(ui.pending.is_none());
        assert_eq!(ui.marked.len(), 2);
        assert!(
            !ui.pane
                .browser
                .visible()
                .iter()
                .any(|n| n.borrow().create_entry)
        );
        ui.action("escape").unwrap();
        assert!(ui.marked.is_empty());
    }
    #[test]
    fn destination_plus_is_transient_and_parent_child_marks_deduplicate() {
        let f = Fixture::new();
        let mut ui = ui(&f.0);
        let parent = f.0.join("branch0");
        ui.marked.extend([parent.clone(), parent.join("file0.txt")]);
        assert_eq!(ui.sources().unwrap(), vec![parent.clone()]);
        ui.begin_transfer(false).unwrap();
        let node = ui
            .pane
            .browser
            .visible()
            .into_iter()
            .find(|n| n.borrow().path == parent)
            .unwrap();
        ui.pane.browser.expand(&node, None);
        assert!(node.borrow().children.last().unwrap().borrow().create_entry);
        ui.pane.browser.selected = node.borrow().children.last().unwrap().clone();
        assert_eq!(ui.pane.browser.target(), parent);
        ui.cancel_transfer();
        assert!(Rc::ptr_eq(&ui.pane.browser.selected, &node));
        assert!(
            !node
                .borrow()
                .children
                .iter()
                .any(|n| n.borrow().create_entry)
        );
    }
    #[test]
    fn marked_items_have_non_overlapping_bright_rectangles() {
        let f = Fixture::new();
        let mut ui = ui(&f.0);
        let children = ui.pane.browser.root.borrow().children.clone();
        children[0].borrow_mut().marked = true;
        children[1].borrow_mut().marked = true;
        let layout = Layout::new(&ui.pane.browser);
        let positions: Vec<_> = layout
            .items
            .iter()
            .filter(|p| p.node.borrow().marked)
            .collect();
        assert!(positions[1].y >= positions[0].y + 3);
        assert!(positions[0].label.starts_with("│ ") && positions[0].label.ends_with(" │"));
        let mut output = Vec::new();
        draw_tree(
            &mut output,
            &mut ui.pane,
            TreeArea {
                x: 0,
                width: 100,
                mirrored: false,
            },
            20,
            true,
        )
        .unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains('┌') && output.contains('┘'));
        assert_eq!(ui.pane.browser.conf.mark_color, "#FF55FF");
        assert!(output.contains("\x1b[1m"));
    }
    #[test]
    fn specialized_openers_match_only_their_extensions_and_custom_rules() {
        let f = Fixture::new();
        let conf = Config::default();
        for (id, yes, no) in [
            ("python", "MAIN.PY", "main.txt"),
            ("rscript", "stats.R", "stats.py"),
            ("bash", "run.sh", "run.zip"),
            ("unzip", "data.zip", "data.sh"),
            ("markdown", "readme.md", "readme.r"),
        ] {
            assert!(conf.openers[id].supports(&f.0.join(yes)));
            assert!(!conf.openers[id].supports(&f.0.join(no)));
            assert!(!conf.openers[id].supports(&f.0));
            assert!(files::open_command(&f.0.join(no), &conf, id).is_err());
        }
        let path = f.0.join("rules.conf");
        fs::write(
            &path,
            "[colors]\nmark_color='#00FF00'\n[openers.python]\nextensions=['.py','.ipy']\n",
        )
        .unwrap();
        let conf = Config::load(&path).unwrap();
        assert_eq!(conf.mark_color, "#00FF00");
        assert!(conf.openers["python"].supports(&f.0.join("a.IPY")));
        assert!(!conf.openers["python"].supports(&f.0.join("a.txt")));
    }
    #[test]
    fn directory_pins_override_directory_first_and_bottom_order() {
        let f = Fixture::new();
        fs::write(f.0.join("index.html"), "index").unwrap();
        fs::create_dir(f.0.join("node_modules")).unwrap();
        let path = f.0.join("ordering.conf");
        fs::write(
            &path,
            "[order]\ntop=['index.html']\nbottom=['node_modules']\n",
        )
        .unwrap();
        let conf = Config::load(&path).unwrap();
        let browser = Browser::new(&f.0, conf).unwrap();
        let names: Vec<_> = browser
            .root
            .borrow()
            .children
            .iter()
            .map(|n| {
                n.borrow()
                    .path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names.first().unwrap(), "index.html");
        assert_eq!(names.last().unwrap(), "node_modules");
        let key = f.0.file_name().unwrap().to_string_lossy();
        fs::write(&path,format!("[order]\ntop=['index.html']\nbottom=['node_modules']\n[directory_order.'{key}']\ntop=['ordering.conf']\nbottom=[]\n")).unwrap();
        let conf = Config::load(&path).unwrap();
        assert_eq!(conf.order_rank(&f.0, "ordering.conf", false), (-1, 0));
        assert_eq!(conf.order_rank(&f.0, "node_modules", true), (0, 0));
        assert_eq!(
            conf.order_rank(Path::new("/other"), "index.html", false),
            (-1, 0)
        );
    }
    #[test]
    fn other_folders_selector_respects_explicit_names_and_directory_scope() {
        let f = Fixture::new();
        fs::write(f.0.join("index.html"), "index").unwrap();
        fs::write(f.0.join("notes.txt"), "notes").unwrap();
        fs::create_dir(f.0.join("node_modules")).unwrap();
        let key = f.0.to_string_lossy();
        let path = f.0.join("ordering.conf");
        fs::write(&path, format!("[order]\ntop=['notes.txt']\n[directory_order.'{key}']\ntop=['index.html', 'branch0']\nbottom=['<other-folders>', 'node_modules']\n")).unwrap();
        let conf = Config::load(&path).unwrap();
        assert_eq!(conf.order_rank(&f.0, "branch0", true), (-1, 1));
        assert_eq!(conf.order_rank(&f.0, "branch1", true), (1, 0));
        assert_eq!(conf.order_rank(&f.0, "node_modules", true), (1, 1));
        assert_eq!(conf.order_rank(&f.0, "notes.txt", false), (0, 0));
        assert_eq!(conf.order_rank(&f.0, "link", false), (0, 0));
        assert_eq!(
            conf.order_rank(&f.0.join("branch1"), "notes.txt", false),
            (-1, 0)
        );
        let browser = Browser::new(&f.0, conf.clone()).unwrap();
        let names: Vec<_> = browser
            .root
            .borrow()
            .children
            .iter()
            .map(|n| {
                n.borrow()
                    .path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(&names[..2], &["index.html", "branch0"]);
        assert!(
            names.iter().position(|n| n == "notes.txt") < names.iter().position(|n| n == "branch1")
        );
        assert_eq!(names.last().unwrap(), "node_modules");
        fs::write(
            &path,
            "[order]\ntop=['<other-folders>']\nbottom=['branch0']\n",
        )
        .unwrap();
        let conf = Config::load(&path).unwrap();
        assert_eq!(conf.order_rank(&f.0, "branch0", true), (1, 0));
        assert_eq!(conf.order_rank(&f.0, "branch1", true), (-1, 0));
        assert_eq!(conf.order_rank(&f.0, "notes.txt", false), (0, 0));
    }
}
