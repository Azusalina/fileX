use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};
use toml::Value;

#[derive(Clone, Debug)]
pub struct Opener {
    pub label: String,
    pub command: Vec<String>,
    pub terminal: bool,
}
#[derive(Clone, Debug)]
pub struct Config {
    pub hidden: bool,
    pub dirs_first: bool,
    pub sort: String,
    pub max_entries: usize,
    pub max_expand: usize,
    pub max_loaded: usize,
    pub gap: usize,
    pub label_width: usize,
    pub row_spacing: usize,
    pub orientation: String,
    pub tmux_right: bool,
    pub preview_bytes: usize,
    pub dashboard_width: usize,
    pub search_results: usize,
    pub search_limit: usize,
    pub folder_color: String,
    pub file_color: String,
    pub line_color: String,
    pub keys: BTreeMap<String, String>,
    pub openers: BTreeMap<String, Opener>,
    pub defaults: BTreeMap<String, String>,
    pub extension_colors: BTreeMap<String, String>,
}
pub fn config_path() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"))
        .join("fileX/fileX.conf")
}
pub fn expand_home(value: &str) -> PathBuf {
    if value == "~" {
        PathBuf::from(env::var_os("HOME").unwrap_or_default())
    } else if let Some(tail) = value.strip_prefix("~/") {
        PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(tail)
    } else {
        PathBuf::from(value)
    }
}
pub fn key_code(value: &str) -> String {
    if value.eq_ignore_ascii_case("Left") {
        return "left".into();
    }
    if value.len() == 6
        && value.as_bytes()[..5].eq_ignore_ascii_case(b"ctrl+")
        && value.as_bytes()[5].is_ascii_alphabetic()
    {
        return char::from(value.as_bytes()[5].to_ascii_lowercase() - b'a' + 1).to_string();
    }
    value.to_owned()
}
impl Default for Config {
    fn default() -> Self {
        let keys = [
            ("quit", "Q"),
            ("dashboard", "q"),
            ("expand", "l"),
            ("collapse", "h"),
            ("up", "k"),
            ("down", "j"),
            ("expand_all", "E"),
            ("collapse_all", "C"),
            ("open", "o"),
            ("open_with", "w"),
            ("preview", "p"),
            ("hidden", "."),
            ("refresh", "r"),
            ("search", "Ctrl+f"),
            ("next_match", "n"),
            ("previous_match", "N"),
            ("goto", "g"),
            ("parent", "Left"),
            ("toggle_tree", "v"),
            ("focus_root", "f"),
            ("new_file", "a"),
            ("new_folder", "A"),
            ("rename", "R"),
            ("copy", "c"),
            ("move", "m"),
            ("trash", "D"),
            ("help", "?"),
            ("config", ","),
        ]
        .into_iter()
        .map(|(a, b)| (a.into(), b.into()))
        .collect();
        let openers = [
            ("system", "系统默认", vec!["xdg-open", "{path}"], false),
            (
                "vscodium",
                "VSCodium",
                vec!["codium", "--", "{path}"],
                false,
            ),
            ("vim", "Vim", vec!["vim", "--", "{path}"], true),
            ("nvim", "Neovim", vec!["nvim", "--", "{path}"], true),
            ("nano", "Nano", vec!["nano", "--", "{path}"], true),
            ("firefox", "Firefox", vec!["firefox", "{path}"], false),
        ]
        .into_iter()
        .map(|(id, label, command, terminal)| {
            (
                id.into(),
                Opener {
                    label: label.into(),
                    command: command.into_iter().map(str::to_string).collect(),
                    terminal,
                },
            )
        })
        .collect();
        Self {
            hidden: false,
            dirs_first: true,
            sort: "name".into(),
            max_entries: 1000,
            max_expand: 5000,
            max_loaded: 6000,
            gap: 6,
            label_width: 32,
            row_spacing: 1,
            orientation: "horizontal".into(),
            tmux_right: true,
            preview_bytes: 65536,
            dashboard_width: 36,
            search_results: 500,
            search_limit: 100000,
            folder_color: "#7dcfff".into(),
            file_color: "#c8d3df".into(),
            line_color: "#526779".into(),
            keys,
            openers,
            defaults: [
                (".py", "vscodium"),
                (".html", "firefox"),
                (".htm", "firefox"),
                ("*", "system"),
            ]
            .into_iter()
            .map(|(a, b)| (a.into(), b.into()))
            .collect(),
            extension_colors: [
                (".py", "#e5c07b"),
                (".html", "#e06c75"),
                (".png", "#c792ea"),
                (".jpg", "#c792ea"),
                (".md", "#98c379"),
            ]
            .into_iter()
            .map(|(a, b)| (a.into(), b.into()))
            .collect(),
        }
    }
}
fn color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}
fn string(value: &Value, key: &str) -> Result<String, String> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{key}: 应为字符串"))
}
fn number(value: &Value, key: &str, low: usize, high: usize) -> Result<usize, String> {
    value
        .as_integer()
        .filter(|&n| n >= low as i64 && n <= high as i64)
        .map(|n| n as usize)
        .ok_or_else(|| format!("{key}: 应为 {low}..{high} 的整数"))
}
impl Config {
    pub fn arrow_action(&self, key: &str) -> Option<&'static str> {
        match (self.orientation.as_str(), key) {
            ("vertical", "up") => Some("parent"),
            ("vertical", "down") => Some("expand"),
            ("vertical", "left") => Some("up"),
            ("vertical", "right") => Some("down"),
            (_, "up") => Some("up"),
            (_, "down") => Some("down"),
            (_, "left") => Some("parent"),
            (_, "right") => Some("expand"),
            _ => None,
        }
    }
    pub fn arrow_hint(&self) -> &'static str {
        if self.orientation == "vertical" {
            "←→ 同级 ↑ 父级 ↓ 展开"
        } else {
            "↑↓ 同级 ← 父级 → 展开"
        }
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let data: Value = toml::from_str(&text).map_err(|e| e.to_string())?;
        let table = data.as_table().ok_or("配置应为 TOML 表")?;
        let mut conf = Self::default();
        for (section, values) in table {
            if ![
                "browser",
                "layout",
                "preview",
                "dashboard",
                "search",
                "gui",
                "colors",
                "keys",
                "openers",
                "open_defaults",
                "extension_colors",
                "tmux",
            ]
            .contains(&section.as_str())
            {
                return Err(format!("未知配置区段: {section}"));
            }
            let values = values
                .as_table()
                .ok_or_else(|| format!("{section}: 应为表"))?;
            for (key, value) in values {
                match (section.as_str(), key.as_str()) {
                    ("browser", "show_hidden") => {
                        conf.hidden = value.as_bool().ok_or("show_hidden 应为布尔值")?
                    }
                    ("browser", "directories_first") => {
                        conf.dirs_first = value.as_bool().ok_or("directories_first 应为布尔值")?
                    }
                    ("browser", "sort") => {
                        conf.sort = string(value, key)?;
                        if !["name", "size", "modified"].contains(&conf.sort.as_str()) {
                            return Err("sort 应为 name / size / modified".into());
                        }
                    }
                    ("browser", "max_entries") => conf.max_entries = number(value, key, 1, 100000)?,
                    ("browser", "max_expand_nodes") => {
                        conf.max_expand = number(value, key, 1, 100000)?
                    }
                    ("browser", "max_loaded_nodes") => {
                        conf.max_loaded = number(value, key, 100, 100000)?
                    }
                    ("layout", "column_gap") => conf.gap = number(value, key, 3, 20)?,
                    ("layout", "max_label_width") => {
                        conf.label_width = number(value, key, 12, 120)?
                    }
                    ("layout", "row_spacing") => conf.row_spacing = number(value, key, 1, 4)?,
                    ("layout", "orientation") => {
                        conf.orientation = string(value, key)?;
                        if !["horizontal", "vertical"].contains(&conf.orientation.as_str()) {
                            return Err("orientation 应为 horizontal / vertical".into());
                        }
                    }
                    ("tmux", "open_in_right_pane") => {
                        conf.tmux_right = value.as_bool().ok_or("open_in_right_pane 应为布尔值")?;
                    }
                    ("preview", "preview_bytes") => {
                        conf.preview_bytes = number(value, key, 1024, 1048576)?
                    }
                    ("dashboard", "dashboard_width") => {
                        conf.dashboard_width = number(value, key, 28, 80)?
                    }
                    ("search", "search_max_results") => {
                        conf.search_results = number(value, key, 1, 10000)?
                    }
                    ("search", "search_scan_limit") => {
                        conf.search_limit = number(value, key, 1, 10000000)?
                    }
                    ("gui", "gui_font_size") => {
                        number(value, key, 8, 32)?;
                    }
                    ("colors", _) => {
                        if ![
                            "background",
                            "foreground",
                            "folder_color",
                            "file_color",
                            "selected_color",
                            "line_color",
                        ]
                        .contains(&key.as_str())
                        {
                            return Err(format!("未知颜色选项: {key}"));
                        }
                        let text = string(value, key)?;
                        if !color(&text) {
                            return Err(format!("{key}: 应为 #RRGGBB"));
                        }
                        match key.as_str() {
                            "folder_color" => conf.folder_color = text,
                            "file_color" => conf.file_color = text,
                            "line_color" => conf.line_color = text,
                            _ => {}
                        }
                    }
                    ("keys", _) => {
                        if !conf.keys.contains_key(key) {
                            return Err(format!("未知动作: {key}"));
                        }
                        conf.keys.insert(key.clone(), string(value, key)?);
                    }
                    ("openers", _) => {
                        let item = value.as_table().ok_or("程序应为表")?;
                        let mut opener = conf.openers.get(key).cloned().unwrap_or(Opener {
                            label: key.clone(),
                            command: vec![],
                            terminal: false,
                        });
                        for (field, v) in item {
                            match field.as_str() {
                                "label" => opener.label = string(v, field)?,
                                "terminal" => {
                                    opener.terminal = v.as_bool().ok_or("terminal 应为布尔值")?
                                }
                                "command" => {
                                    opener.command = v
                                        .as_array()
                                        .ok_or("command 应为参数列表")?
                                        .iter()
                                        .map(|v| string(v, "command"))
                                        .collect::<Result<_, _>>()?
                                }
                                _ => return Err(format!("未知程序字段: {field}")),
                            }
                        }
                        if opener.command.is_empty()
                            || opener
                                .command
                                .iter()
                                .any(|v| v.is_empty() || v.contains('\0'))
                            || !opener.command.iter().any(|v| v.contains("{path}"))
                            || opener.label.is_empty()
                        {
                            return Err(format!(
                                "openers.{key}: 需要非空参数列表、{{path}} 和 label"
                            ));
                        }
                        conf.openers.insert(key.clone(), opener);
                    }
                    ("open_defaults" | "extension_colors", _) => {
                        if !["*", "", "<directory>"].contains(&key.as_str())
                            && (!key.starts_with('.') || key.contains('/'))
                        {
                            return Err(format!("扩展名应以 . 开始: {key}"));
                        }
                        let text = string(value, key)?;
                        if section == "extension_colors" {
                            if !color(&text) {
                                return Err(format!("{key}: 应为 #RRGGBB"));
                            }
                            conf.extension_colors.insert(key.to_lowercase(), text);
                        } else {
                            conf.defaults.insert(key.to_lowercase(), text);
                        }
                    }
                    _ => return Err(format!("未知配置: {section}.{key}")),
                }
            }
        }
        if table
            .get("keys")
            .and_then(|v| v.get("quit"))
            .and_then(Value::as_str)
            == Some("q")
            && table.get("keys").and_then(|v| v.get("dashboard")).is_none()
        {
            conf.keys.insert("quit".into(), "Q".into());
        }
        let mut used = std::collections::HashSet::new();
        if conf.keys.get("parent").map(String::as_str) == Some("u") {
            conf.keys.insert("parent".into(), "Left".into());
        }
        for (action, key) in &conf.keys {
            let code = key_code(key);
            if (code.chars().count() != 1 && !(action == "parent" && code == "left"))
                || (code == *key && key.chars().any(char::is_control))
                || [
                    "\n", "\r", "\t", " ", "\x1b", "\x03", "\x15", "\x13", "\x11", "\x1a", "H",
                    "L", ":",
                ]
                .contains(&code.as_str())
            {
                return Err(format!("无效快捷键: {action}={key}"));
            }
            if ["\x06", "/"].contains(&code.as_str()) && action != "search"
                || code == "\x0c" && action != "goto"
                || code == "left" && action != "parent"
            {
                return Err("Ctrl+F / Ctrl+L 保留给搜索 / 路径".into());
            }
            if !used.insert(code) {
                return Err("快捷键冲突".into());
            }
        }
        for method in conf.defaults.values() {
            if !conf.openers.contains_key(method) {
                return Err(format!("打开程序未定义: {method}"));
            }
        }
        Ok(conf)
    }
    pub fn action(&self, key: &str) -> Option<&str> {
        self.keys
            .iter()
            .find(|(_, v)| key_code(v) == key)
            .map(|(a, _)| a.as_str())
    }
    pub fn rule<'a>(
        &self,
        path: &Path,
        rules: &'a BTreeMap<String, String>,
        dir: bool,
    ) -> Option<&'a String> {
        if dir {
            return rules.get("<directory>").or_else(|| rules.get("*"));
        }
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        rules
            .iter()
            .filter(|(key, _)| key.starts_with('.') && name.ends_with(key.as_str()))
            .max_by_key(|(key, _)| key.len())
            .map(|(_, v)| v)
            .or_else(|| {
                if path.extension().is_none() {
                    rules.get("").or_else(|| rules.get("*"))
                } else {
                    rules.get("*")
                }
            })
    }
}
