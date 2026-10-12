use crate::{
    config::{Config, key_code},
    files,
    model::{Browser, Layout, clean, clip},
    tmux,
};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{
        self, DisableFocusChange, EnableFocusChange, Event, KeyCode, KeyEventKind, KeyModifiers,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute, queue,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
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
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            EnableFocusChange,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES),
            Print("\x1b[>4;2m"),
            Hide
        )
        .map_err(ioerr)?;
        Ok(Self)
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            SetAttribute(Attribute::Reset),
            ResetColor,
            Show,
            PopKeyboardEnhancementFlags,
            DisableFocusChange,
            Print("\x1b[>4;0m"),
            LeaveAlternateScreen
        );
    }
}
fn read_key(auto_english: bool) -> Result<String, String> {
    loop {
        match event::read().map_err(ioerr)? {
            Event::FocusLost => continue,
            Event::FocusGained => {
                // Reapply raw mode without replacing crossterm's saved shell state.
                let mut settings = std::mem::MaybeUninit::<libc::termios>::uninit();
                if unsafe { libc::tcgetattr(libc::STDIN_FILENO, settings.as_mut_ptr()) } == 0 {
                    let mut settings = unsafe { settings.assume_init() };
                    unsafe {
                        libc::cfmakeraw(&mut settings);
                        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &settings);
                        libc::tcflow(libc::STDOUT_FILENO, libc::TCOON);
                    }
                }
                execute!(io::stdout(), Hide, Print("\x1b[=1u\x1b[>4;2m")).map_err(ioerr)?;
                crate::input::ensure_english(auto_english);
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
    if dim {
        queue!(out, SetAttribute(Attribute::Dim)).map_err(ioerr)?;
    }
    if let Some(color) = color {
        queue!(out, SetForegroundColor(rgb(color))).map_err(ioerr)?
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
struct Ui {
    browser: Browser,
    config_path: PathBuf,
    left: usize,
    top: usize,
    focus: bool,
    dashboard: bool,
    meta_path: Option<PathBuf>,
    meta: String,
    query: String,
    message: String,
    exit_directory: Option<PathBuf>,
}
impl Ui {
    fn draw(&mut self) -> Result<(), String> {
        let (w, h) = terminal::size().map_err(ioerr)?;
        let (w, h) = (w as usize, h as usize);
        let mut out = io::stdout().lock();
        queue!(out, Clear(ClearType::All)).map_err(ioerr)?;
        if w < 36 || h < 8 {
            put(
                &mut out,
                0,
                0,
                &format!("请放大终端到 36×8；{} 退出", self.browser.conf.keys["quit"]),
                w,
                None,
                false,
            )?;
            return out.flush().map_err(ioerr);
        }
        let panel = if self.dashboard && w >= 70 {
            self.browser.conf.dashboard_columns(w)
        } else {
            0
        };
        let tree = w - panel - if panel > 0 { 2 } else { 0 };
        let body = h - 5;
        let layout = Layout::new(&self.browser);
        let selected = layout
            .items
            .iter()
            .find(|p| Rc::ptr_eq(&p.node, &self.browser.selected))
            .ok_or("选中节点不在树中")?;
        if self.focus {
            if selected.y < self.top {
                self.top = selected.y
            }
            if selected.y >= self.top + body {
                self.top = selected.y - body + 1
            }
            if selected.x < self.left {
                self.left = selected.x
            }
            if selected.x + selected.label.width() > self.left + tree - 2 {
                self.left = (selected.x + selected.label.width()).saturating_sub(tree - 2)
            }
            self.focus = false;
        }
        self.left = self.left.min(layout.width.saturating_sub(tree - 2));
        self.top = self.top.min(layout.height.saturating_sub(body));
        put(&mut out, 0, 1, "filex", w, None, false)?;
        put(
            &mut out,
            1,
            1,
            &self.browser.root.borrow().path.to_string_lossy(),
            w,
            Some(
                self.browser
                    .conf
                    .folder_color_for(&self.browser.root.borrow().path),
            ),
            false,
        )?;
        let route = if self.browser.conf.highlight_path {
            layout.path_cells(
                &self.browser.selected,
                self.browser.conf.gap,
                self.left,
                self.top,
                tree - 2,
                body,
            )
        } else {
            Default::default()
        };
        let mut ancestors = std::collections::HashSet::new();
        let mut ancestor = Some(self.browser.selected.clone());
        while let Some(node) = ancestor {
            ancestors.insert(Rc::as_ptr(&node) as usize);
            ancestor = node.borrow().parent.upgrade();
        }
        for ((x, y), c) in layout.cells(self.left, self.top, tree - 2, body) {
            let active = route.contains(&(x, y));
            put_styled(
                &mut out,
                y - self.top + 3,
                (x - self.left + 1) as isize,
                &c.to_string(),
                tree,
                Some(if active {
                    &self.browser.conf.path_color
                } else {
                    &self.browser.conf.line_color
                }),
                false,
                false,
                self.browser.conf.highlight_path && !active,
            )?
        }
        for item in &layout.items {
            if item.y >= self.top && item.y < self.top + body {
                let n = item.node.borrow();
                let color = if n.dir {
                    self.browser.conf.folder_color_for(&n.path)
                } else {
                    self.browser
                        .conf
                        .rule(&n.path, &self.browser.conf.extension_colors, false)
                        .unwrap_or(&self.browser.conf.file_color)
                };
                let dim = self.browser.conf.highlight_path
                    && !ancestors.contains(&(Rc::as_ptr(&item.node) as usize));
                put_styled(
                    &mut out,
                    item.y - self.top + 3,
                    item.x as isize - self.left as isize + 1,
                    &item.label,
                    tree,
                    Some(color),
                    Rc::ptr_eq(&item.node, &self.browser.selected),
                    false,
                    dim,
                )?;
                if let Some(start) = item.link_start {
                    put_styled(
                        &mut out,
                        item.y - self.top + 3,
                        (item.x + item.label[..start].width()) as isize - self.left as isize + 1,
                        &item.label[start..],
                        tree,
                        Some(&self.browser.conf.link_color),
                        false,
                        true,
                        dim,
                    )?;
                }
            }
        }
        if panel > 0 {
            for y in 3..h - 2 {
                put(
                    &mut out,
                    y,
                    tree as isize,
                    "│",
                    w,
                    Some(&self.browser.conf.line_color),
                    false,
                )?
            }
            let path = self.browser.selected.borrow().path.clone();
            if self.meta_path.as_ref() != Some(&path) {
                self.meta =
                    files::dashboard(&path, &self.browser.root.borrow().path, &self.browser.conf);
                self.meta_path = Some(path)
            }
            put(
                &mut out,
                3,
                (tree + 2) as isize,
                &format!(
                    "Dashboard · q 收起 · {} / {} 调宽",
                    self.browser.conf.keys["dashboard_narrow"],
                    self.browser.conf.keys["dashboard_widen"]
                ),
                w,
                None,
                false,
            )?;
            for (i, line) in self.meta.lines().take(body - 1).enumerate() {
                put(&mut out, 4 + i, (tree + 2) as isize, line, w, None, false)?
            }
        }
        let selected = self.browser.selected.borrow();
        let detail = if !selected.error.is_empty() {
            selected.error.clone()
        } else if selected.omitted > 0 {
            format!(
                "还有 {} 项未显示；受 max_entries / max_loaded_nodes 限制",
                selected.omitted
            )
        } else {
            selected.path.to_string_lossy().into_owned()
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
        put(
            &mut out,
            h - 1,
            1,
            &format!(
                "{} {}收起 {}重命名 Ctrl+Alt+C路径 Enter打开 {}信息 {}退出 ?帮助",
                self.browser.conf.arrow_hint(),
                self.browser.conf.keys["collapse"],
                self.browser.conf.keys["rename"],
                self.browser.conf.keys["dashboard"],
                self.browser.conf.keys["quit"]
            ),
            w,
            None,
            false,
        )?;
        queue!(out, SetAttribute(Attribute::Reset), ResetColor).map_err(ioerr)?;
        out.flush().map_err(ioerr)
    }
    fn prompt(&mut self, label: &str, initial: &str) -> Result<Option<String>, String> {
        let mut text = initial.to_owned();
        loop {
            self.draw()?;
            let (w, h) = terminal::size().map_err(ioerr)?;
            let mut value = format!("{label}: {text}");
            while value.width() >= w.saturating_sub(2) as usize && !value.is_empty() {
                let c = value.chars().next().unwrap();
                value.drain(..c.len_utf8());
            }
            let mut out = io::stdout().lock();
            queue!(
                out,
                MoveTo(0, h - 2),
                Clear(ClearType::CurrentLine),
                Show,
                SetAttribute(Attribute::Reset),
                ResetColor,
                Print(&value)
            )
            .map_err(ioerr)?;
            out.flush().map_err(ioerr)?;
            drop(out);
            let key = read_key(self.browser.conf.auto_english)?;
            match key.as_str() {
                "enter" => {
                    execute!(io::stdout(), Hide).map_err(ioerr)?;
                    return Ok(Some(text));
                }
                "escape" => {
                    execute!(io::stdout(), Hide).map_err(ioerr)?;
                    return Ok(None);
                }
                "\x03" => return Err("已取消".into()),
                "backspace" => {
                    text.pop();
                }
                "\x15" => text.clear(),
                _ => {
                    if key.chars().count() == 1 && !key.chars().any(char::is_control) {
                        text.push_str(&key)
                    }
                }
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
            queue!(out, Clear(ClearType::All)).map_err(ioerr)?;
            put(&mut out, 0, 1, title, w, None, false)?;
            for (i, line) in rows.iter().skip(top).take(body).enumerate() {
                put(&mut out, i + 1, 1 - left as isize, line, w, None, false)?
            }
            put(
                &mut out,
                h - 1,
                1,
                "↑↓ 滚动 ←→ 横移 Esc / q / Enter 返回",
                w,
                None,
                false,
            )?;
            out.flush().map_err(ioerr)?;
            drop(out);
            match read_key(self.browser.conf.auto_english)?.as_str() {
                "escape" | "q" | "enter" => return Ok(()),
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
            queue!(out, Clear(ClearType::All)).map_err(ioerr)?;
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
                "↑↓ 选择 Enter 确认 Esc 取消",
                w,
                None,
                false,
            )?;
            out.flush().map_err(ioerr)?;
            drop(out);
            match read_key(self.browser.conf.auto_english)?.as_str() {
                "escape" => return Ok(None),
                "enter" => return Ok(Some(selected)),
                "down" | "j" => selected = (selected + 1) % options.len(),
                "up" | "k" => selected = (selected + options.len() - 1) % options.len(),
                _ => {}
            }
        }
    }
    fn open(&mut self, method: &str) -> Result<(), String> {
        let path = self.browser.selected.borrow().path.clone();
        let (mut command, terminal) = files::open_command(&path, &self.browser.conf, method)?;
        if terminal
            && self.browser.conf.tmux_right
            && let Some(pane) = tmux::open_in_right_pane(&command)
        {
            self.meta_path = None;
            self.message = format!(
                "已用 {} 在右侧 pane {pane} 打开",
                self.browser.conf.openers[method].label
            );
            return Ok(());
        }
        if terminal {
            terminal::disable_raw_mode().map_err(ioerr)?;
            execute!(
                io::stdout(),
                SetAttribute(Attribute::Reset),
                ResetColor,
                Show,
                PopKeyboardEnhancementFlags,
                DisableFocusChange,
                Print("\x1b[>4;0m"),
                LeaveAlternateScreen
            )
            .map_err(ioerr)?;
            let mut result = command.status().map_err(ioerr);
            if self.browser.conf.openers[method].pause_on_exit
                && let Ok(status) = &result
            {
                println!("\n程序结束 ({status}) · 按 Enter 返回 filex");
                terminal::enable_raw_mode().map_err(ioerr)?;
                loop {
                    match read_key(self.browser.conf.auto_english) {
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
            execute!(
                io::stdout(),
                EnterAlternateScreen,
                EnableFocusChange,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES),
                Print("\x1b[>4;2m"),
                Hide
            )
            .map_err(ioerr)?;
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
        self.message = format!("已用 {} 打开", self.browser.conf.openers[method].label);
        Ok(())
    }
    fn choose_open(&mut self) -> Result<(), String> {
        let node = self.browser.selected.clone();
        let path = node.borrow().path.clone();
        let dir = node.borrow().dir;
        let default = self
            .browser
            .conf
            .rule(&path, &self.browser.conf.defaults, dir)
            .cloned()
            .unwrap_or("system".into());
        let mut ids = vec![default.clone()];
        ids.extend(
            self.browser
                .conf
                .openers
                .keys()
                .filter(|id| **id != default)
                .cloned(),
        );
        let mut options: Vec<(Option<String>, String)> = ids
            .iter()
            .map(|id| {
                let opener = &self.browser.conf.openers[id];
                (
                    Some(id.clone()),
                    format!(
                        "{} ({id}){}{}",
                        opener.label,
                        if id == &default { " [默认]" } else { "" },
                        if files::available(&opener.command[0]) {
                            ""
                        } else {
                            " [未安装]"
                        }
                    ),
                )
            })
            .collect();
        if dir {
            options.insert(0, (None, "fileX 展开 / 收起目录".into()));
        }
        if let Some(index) = self.choose(&format!("打开方式: {}", path.display()), &options)? {
            if let Some(method) = &options[index].0 {
                self.open(method)?
            } else if node.borrow().expanded {
                node.borrow_mut().expanded = false
            } else {
                self.browser.expand(&node, None)
            }
        }
        Ok(())
    }
    fn confirm(&mut self, title: &str) -> Result<bool, String> {
        loop {
            self.draw()?;
            let (width, height) = terminal::size().map_err(ioerr)?;
            let mut out = io::stdout().lock();
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
                "再次按 Enter 确认 · Esc 取消",
                width as usize,
                None,
                false,
            )?;
            out.flush().map_err(ioerr)?;
            drop(out);
            match read_key(self.browser.conf.auto_english)?.as_str() {
                "enter" => return Ok(true),
                "escape" | "\x03" => return Ok(false),
                _ => {}
            }
        }
    }
    fn operation(&mut self, action: &str) -> Result<(), String> {
        let source = self.browser.selected.borrow().path.clone();
        if Rc::ptr_eq(&self.browser.selected, &self.browser.root)
            && ["rename", "move", "trash"].contains(&action)
        {
            return Err("不能操作当前根目录；先按 ← 引入其父目录".into());
        }
        let dest = match action {
            "trash" => {
                if !self.confirm(&format!("送入回收站 {}？", source.display()))? {
                    return Ok(());
                }
                files::trash(&source)?;
                source.parent().unwrap_or(Path::new("/")).to_path_buf()
            }
            _ => {
                let label = match action {
                    "new_file" => "新文件名称",
                    "new_folder" => "新目录名称",
                    "rename" => "新名称",
                    "copy" => "复制到完整目标路径（含名称）",
                    _ => "移动到完整目标路径（含名称）",
                };
                let initial = if action == "rename" {
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
                        let target = self.browser.target();
                        let path = files::create(&target, &value, action == "new_folder")?;
                        let parent = if self.browser.selected.borrow().dir {
                            self.browser.selected.clone()
                        } else {
                            self.browser.selected.borrow().parent.upgrade().unwrap()
                        };
                        self.browser.expand(&parent, Some(&path));
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
        self.browser.refresh();
        if let Some(node) = self
            .browser
            .visible()
            .into_iter()
            .find(|n| n.borrow().path == dest)
        {
            self.browser.selected = node
        }
        self.meta_path = None;
        self.message = format!("完成: {}", dest.display());
        Ok(())
    }
    fn action(&mut self, key: &str) -> Result<bool, String> {
        let action = self
            .browser
            .conf
            .arrow_action(key)
            .unwrap_or_else(|| match key {
                " " => "expand",
                "enter" => "open_with",
                "delete" => "trash",
                "f5" | "ctrl+f5" => "refresh",
                "\x06" | "/" => "search",
                "\x0c" => "goto",
                _ => self.browser.conf.action(key).unwrap_or(""),
            })
            .to_string();
        match action.as_str() {
            "quit" => return Ok(true),
            "exit_here" => {
                self.exit_directory =
                    Some(files::exit_directory(&self.browser.selected.borrow().path)?);
                return Ok(true);
            }
            "dashboard" => {
                self.dashboard = !self.dashboard;
                self.meta_path = None;
                if self.dashboard && terminal::size().map_err(ioerr)?.0 < 70 {
                    self.message = "Dashboard 需要至少 70 列".into()
                }
            }
            "dashboard_narrow" | "dashboard_widen" => {
                let width = terminal::size().map_err(ioerr)?.0 as usize;
                if width < 70 {
                    self.message = "Dashboard 需要至少 70 列".into();
                } else {
                    let current = self.browser.conf.dashboard_columns(width);
                    let desired = if action == "dashboard_narrow" {
                        current.saturating_sub(4)
                    } else {
                        current + 4
                    };
                    self.browser.conf.dashboard_width = desired.clamp(20, (width - 38).min(240));
                    self.dashboard = true;
                    self.message =
                        format!("信息面板宽度: {} 列", self.browser.conf.dashboard_width);
                }
            }
            "up" => self.browser.step(-1),
            "down" => self.browser.step(1),
            "expand" => self.browser.right(),
            "collapse" => self.browser.left(),
            "expand_all" => {
                self.message = "正在展开…".into();
                self.draw()?;
                self.message = self.browser.expand_all()?
            }
            "collapse_all" => self.message = self.browser.collapse_all()?,
            "parent" => {
                self.message = self.browser.back()?;
                self.left = 0;
                self.top = 0
            }
            "focus_root" => {
                self.message = self.browser.focus_root();
                self.query.clear();
                self.meta_path = None;
                self.left = 0;
                self.top = 0;
            }
            "hidden" => {
                self.browser.conf.hidden = !self.browser.conf.hidden;
                self.browser.refresh()
            }
            "copy_path" => {
                files::copy_path(&self.browser.selected.borrow().path)?;
                self.message = "已复制绝对路径".into();
            }
            "refresh" => {
                self.browser.refresh();
                self.meta_path = None
            }
            "config" => {
                self.browser.conf = Config::load(&self.config_path)?;
                self.browser.refresh();
                self.meta_path = None;
                self.message = "配置已重新读取".into()
            }
            "goto" => {
                let path = self
                    .browser
                    .root
                    .borrow()
                    .path
                    .to_string_lossy()
                    .into_owned();
                if let Some(value) = self.prompt("进入目录", &path)?
                    && !value.is_empty()
                {
                    self.browser.set_root(&value)?;
                    self.left = 0;
                    self.top = 0
                }
            }
            "search" => {
                let query = self.query.clone();
                if let Some(value) = self.prompt("搜索根目录（含未展开目录）", &query)?
                {
                    self.query = value;
                    if !self.query.is_empty() {
                        self.message = "正在搜索…".into();
                        self.draw()?;
                        self.browser.search_key = None;
                        self.message = self.browser.find_next(&self.query, 1)?
                    }
                }
            }
            "next_match" | "previous_match" => {
                if !self.query.is_empty() {
                    self.message = self
                        .browser
                        .find_next(&self.query, if action == "next_match" { 1 } else { -1 })?
                }
            }
            "open_with" => self.choose_open()?,
            "open" => {
                let n = self.browser.selected.borrow();
                let method = self
                    .browser
                    .conf
                    .rule(&n.path, &self.browser.conf.defaults, n.dir)
                    .cloned()
                    .unwrap_or("system".into());
                drop(n);
                self.open(&method)?
            }
            "link_info" => {
                let path = self.browser.selected.borrow().path.clone();
                let text =
                    files::dashboard(&path, &self.browser.root.borrow().path, &self.browser.conf);
                self.pager("文件 / 链接信息", &text)?;
            }
            "preview" => {
                let path = self.browser.selected.borrow().path.clone();
                let text = files::preview(&path, self.browser.conf.preview_bytes);
                self.pager(&path.to_string_lossy(), &text)?
            }
            "new_file" | "new_folder" | "rename" | "copy" | "move" | "trash" => {
                self.operation(&action)?
            }
            "help" => {
                let mut text = self
                    .browser
                    .conf
                    .keys
                    .iter()
                    .map(|(action, key)| format!("{key:12} {action}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                text.push_str("\n\n横向：↑↓ 同层跨父目录，首尾循环 · → 展开 / 进入子级 · ← 返回父级\n根节点的返回父级键引入上级目录。\nh 收起 · [ / ] 缩窄 / 加宽 q 信息面板（每次 4 列）\nx 退出并将启动 terminal 切到选中目录（文件使用父目录）\nCtrl+N 新建文件 · Ctrl+Shift+N 新建目录（a/A 也可）\n↗ 软链接 · H×N 硬链接数 · q 查看信息 · I 查看完整链接路径\nr / Shift+R 重命名 · Ctrl+R / F5 / Ctrl+F5 刷新\nCtrl+Alt+C 复制选中项目的绝对路径\nf 当前目录设为根并刷新\nEnter 选择打开方式 · o 扩展名默认程序\nCtrl+F / / 搜索 · q 右侧时间 / 大小\nDelete / D 移到回收站，再次按 Enter 确认\nE/C 展开 / 收起全部\nH/L 横移 · PgUp/PgDn 同级翻页 · Ctrl+U 清空输入\n终端程序优先使用 tmux 右侧空闲 pane。\n复制 / 移动填写完整目标路径，不覆盖已有目标。\n缓存达到上限时请收起部分分支或增大 max_loaded_nodes。");
                self.pager("filex 快捷键", &text)?
            }
            _ => match key {
                "escape" => {
                    self.query.clear();
                    self.message.clear()
                }
                "\x03" => return Ok(true),
                "H" => {
                    self.left = self.left.saturating_sub(12);
                    self.focus = false
                }
                "L" => {
                    self.left += 12;
                    self.focus = false
                }
                "home" => self.browser.selected = self.browser.root.clone(),
                "end" => self.browser.selected = self.browser.visible().last().unwrap().clone(),
                "pageup" => self
                    .browser
                    .step(-(terminal::size().map_err(ioerr)?.1.saturating_sub(5) as isize)),
                "pagedown" => self
                    .browser
                    .step(terminal::size().map_err(ioerr)?.1.saturating_sub(5) as isize),
                _ => {}
            },
        }
        Ok(false)
    }
}
pub fn run(browser: Browser, path: PathBuf) -> Result<Option<PathBuf>, String> {
    let _input = crate::input::EnglishGuard::new(browser.conf.auto_english);
    let _host = crate::terminal_host::KonsoleGuard::new(browser.conf.konsole_shortcuts);
    let _tmux_keys = crate::terminal_host::TmuxKeys::new(browser.conf.tmux_extended_keys);
    let _tmux_focus = crate::terminal_host::TmuxFocus::new(true);
    let _guard = Guard::new()?;
    let mut ui = Ui {
        browser,
        config_path: path,
        left: 0,
        top: 0,
        focus: true,
        dashboard: false,
        meta_path: None,
        meta: String::new(),
        query: String::new(),
        message: String::new(),
        exit_directory: None,
    };
    loop {
        ui.draw()?;
        let key = read_key(ui.browser.conf.auto_english)?;
        ui.focus = true;
        if !["H", "L", "resize"].contains(&key.as_str()) {
            ui.message.clear()
        }
        match ui.action(&key) {
            Ok(true) => break,
            Ok(false) => {}
            Err(e) => ui.message = format!("操作失败: {}", clean(&e)),
        }
    }
    Ok(ui.exit_directory)
}
