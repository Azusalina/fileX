use crate::{
    config::{Config, expand_home},
    terminal_profile::label,
};
use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, HashSet},
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
pub type NodeRef = Rc<RefCell<Node>>;
pub struct Node {
    pub path: PathBuf,
    pub parent: Weak<RefCell<Node>>,
    pub dir: bool,
    pub link: bool,
    pub links: u64,
    pub create_entry: bool,
    pub marked: bool,
    pub git: crate::git_status::State,
    pub expanded: bool,
    pub children: Vec<NodeRef>,
    pub error: String,
    pub omitted: usize,
}
impl Node {
    fn new(path: PathBuf, parent: Weak<RefCell<Node>>, dir: bool, link: bool) -> NodeRef {
        Rc::new(RefCell::new(Self {
            path,
            parent,
            dir,
            link,
            links: 1,
            create_entry: false,
            marked: false,
            git: Default::default(),
            expanded: false,
            children: vec![],
            error: String::new(),
            omitted: 0,
        }))
    }
    pub fn label(&self, width: usize) -> (String, Option<usize>) {
        if self.create_entry {
            return (clip("+", width), None);
        }
        if self.marked {
            let (text, link) = self.base_label(width.saturating_sub(4));
            let edge = crate::terminal_profile::label("│", "|");
            return (
                format!("{edge} {text} {edge}"),
                link.map(|i| i + edge.len() + 1),
            );
        }
        self.base_label(width)
    }
    fn base_label(&self, width: usize) -> (String, Option<usize>) {
        let arrow = if self.dir {
            if crate::terminal_profile::get().ascii {
                if self.expanded { "v " } else { "> " }
            } else if self.expanded {
                "▾ "
            } else {
                "▸ "
            }
        } else {
            crate::terminal_profile::label("· ", ". ")
        };
        let prefix = if self.git != crate::git_status::State::None {
            format!(
                "{}{} ",
                if self.dir {
                    arrow.trim()
                } else {
                    crate::terminal_profile::label("▸", ">")
                },
                self.git.marker(crate::terminal_profile::get().ascii)
            )
        } else {
            arrow.to_owned()
        };
        let mut suffix = if self.link {
            crate::terminal_profile::label(" ↗", " @").to_owned()
        } else {
            String::new()
        };
        if !self.dir && self.links > 1 {
            suffix.push_str(&format!(
                " H{}{}",
                crate::terminal_profile::label("×", "x"),
                self.links
            ));
        }
        let name = clean(
            &self
                .path
                .file_name()
                .unwrap_or(self.path.as_os_str())
                .to_string_lossy(),
        );
        if self.link
            && let Ok(target) = fs::read_link(&self.path)
        {
            let target = clean(
                &target
                    .file_name()
                    .unwrap_or(target.as_os_str())
                    .to_string_lossy(),
            );
            let available = width.saturating_sub(prefix.width() + 6);
            let target_width = target.width().min(available / 2);
            let head = format!(
                "{prefix}{}",
                clip(&name, available.saturating_sub(target_width))
            );
            let start = head.len();
            let tail = clip(&target, width.saturating_sub(head.width() + 6));
            return (format!("{head}------{tail}"), Some(start));
        }
        let available = width.saturating_sub(prefix.width() + suffix.width());
        (
            clip(
                &format!("{prefix}{}{suffix}", clip(&name, available)),
                width,
            ),
            None,
        )
    }
}
pub fn clean(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_control() {
                if crate::terminal_profile::get().ascii {
                    '?'
                } else {
                    '�'
                }
            } else {
                c
            }
        })
        .collect()
}
pub fn clip(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.into();
    }
    if width == 0 {
        return String::new();
    }
    let mut result = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w > width - 1 {
            break;
        }
        result.push(c);
        used += w;
    }
    result.push(if crate::terminal_profile::get().ascii {
        '.'
    } else {
        '…'
    });
    result
}
#[derive(Eq, PartialEq)]
struct Entry {
    key: (i8, usize, bool, i64, String),
    path: PathBuf,
    dir: bool,
    link: bool,
    links: u64,
}
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
pub struct Browser {
    pub root: NodeRef,
    pub selected: NodeRef,
    pub conf: Config,
    pub results: Vec<PathBuf>,
    pub search_key: Option<(String, PathBuf, bool)>,
    pub search_summary: String,
    pub destination_mode: bool,
    git_cache: crate::git_status::Cache,
}
impl Browser {
    pub fn new(path: &Path, conf: Config) -> Result<Self, String> {
        let path = fs::canonicalize(path).map_err(|e| e.to_string())?;
        if !path.is_dir() {
            return Err(label("不是目录", "Not a directory").into());
        }
        let root = Node::new(path, Weak::new(), true, false);
        let mut browser = Self {
            root: root.clone(),
            selected: root.clone(),
            conf,
            results: vec![],
            search_key: None,
            search_summary: String::new(),
            destination_mode: false,
            git_cache: Default::default(),
        };
        browser.expand(&root, None);
        Ok(browser)
    }
    pub fn visible(&self) -> Vec<NodeRef> {
        let mut result = vec![];
        let mut stack = vec![self.root.clone()];
        while let Some(node) = stack.pop() {
            let n = node.borrow();
            if n.expanded {
                stack.extend(n.children.iter().rev().cloned())
            }
            drop(n);
            result.push(node)
        }
        result
    }
    pub fn loaded(&self) -> usize {
        let mut count = 0;
        let mut stack = vec![self.root.clone()];
        while let Some(node) = stack.pop() {
            count += 1;
            stack.extend(node.borrow().children.iter().cloned())
        }
        count
    }
    fn prune(&self) {
        let mut stack = vec![self.root.clone()];
        while let Some(node) = stack.pop() {
            let mut n = node.borrow_mut();
            if n.expanded {
                stack.extend(n.children.iter().cloned())
            } else {
                n.children.clear()
            }
        }
    }
    pub fn expand(&mut self, node: &NodeRef, force: Option<&Path>) {
        if !node.borrow().dir || node.borrow().link {
            return;
        }
        if self.loaded() >= self.conf.max_loaded {
            self.prune()
        }
        let loaded = self.loaded();
        let old: HashMap<PathBuf, NodeRef> = node
            .borrow()
            .children
            .iter()
            .map(|n| (n.borrow().path.clone(), n.clone()))
            .collect();
        let path = node.borrow().path.clone();
        if self.conf.git_enabled {
            self.git_cache.request(&path, self.conf.git_max_entries);
        }
        let directory = path.clone();
        let mut heap = BinaryHeap::new();
        let mut pinned = vec![];
        let mut count: usize = 0;
        let read = match fs::read_dir(&path) {
            Ok(v) => v,
            Err(e) => {
                let mut n = node.borrow_mut();
                n.error = e.to_string();
                n.expanded = false;
                return;
            }
        };
        for item in read.flatten() {
            let path = item.path();
            let name = item.file_name().to_string_lossy().into_owned();
            if !self.conf.hidden && name.starts_with('.') && force != Some(path.as_path()) {
                continue;
            }
            count += 1;
            let info = fs::symlink_metadata(&path).ok();
            let dir = info.as_ref().is_some_and(|m| m.is_dir());
            let link = info.as_ref().is_some_and(|m| m.file_type().is_symlink());
            let metric = match self.conf.sort.as_str() {
                "size" => -(info.as_ref().map_or(0, |m| m.len().min(i64::MAX as u64)) as i64),
                "modified" => -info.as_ref().map_or(0, |m| m.mtime()),
                _ => 0,
            };
            let entry = Entry {
                key: {
                    let (group, rank) = self.conf.order_rank(&directory, &name, dir);
                    (
                        group,
                        rank,
                        self.conf.dirs_first && !dir,
                        metric,
                        name.to_lowercase(),
                    )
                },
                path,
                dir,
                link,
                links: info.as_ref().map_or(1, |m| m.nlink()),
            };
            if force == Some(entry.path.as_path())
                || old.get(&entry.path).is_some_and(|n| n.borrow().expanded)
            {
                pinned.push(entry);
                continue;
            }
            heap.push(entry);
            if heap.len() > self.conf.max_entries {
                heap.pop();
            }
        }
        let mut entries = heap.into_vec();
        entries.extend(pinned);
        entries.sort();
        let mut free = self.conf.max_loaded.saturating_sub(loaded);
        let mut children = Vec::with_capacity(entries.len().min(self.conf.max_loaded));
        for item in entries {
            let child = if let Some(child) = old.get(&item.path) {
                child.clone()
            } else {
                if free == 0 {
                    continue;
                }
                free -= 1;
                Node::new(item.path.clone(), Rc::downgrade(node), item.dir, item.link)
            };
            {
                let mut c = child.borrow_mut();
                c.dir = item.dir;
                c.link = item.link;
                c.links = item.links;
                c.parent = Rc::downgrade(node);
                if !c.dir || c.link {
                    c.children.clear();
                    c.expanded = false
                }
            }
            children.push(child);
        }
        let mut n = node.borrow_mut();
        n.omitted = count.saturating_sub(children.len());
        if self.destination_mode {
            let entry = Node::new(n.path.clone(), Rc::downgrade(node), false, false);
            entry.borrow_mut().create_entry = true;
            children.push(entry);
        }
        n.children = children;
        n.expanded = true;
        n.error.clear();
    }
    pub fn set_destination_mode(&mut self, enabled: bool) {
        self.destination_mode = enabled;
        if self.selected.borrow().create_entry {
            let parent = self.selected.borrow().parent.upgrade();
            if let Some(parent) = parent {
                self.selected = parent;
            }
        }
        let mut stack = vec![self.root.clone()];
        while let Some(node) = stack.pop() {
            let mut n = node.borrow_mut();
            n.children.retain(|c| !c.borrow().create_entry);
            stack.extend(n.children.iter().cloned());
            if enabled && n.dir && n.expanded {
                let entry = Node::new(n.path.clone(), Rc::downgrade(&node), false, false);
                entry.borrow_mut().create_entry = true;
                n.children.push(entry);
            }
        }
    }
    pub fn update_git(&mut self) {
        self.git_cache.poll();
        let mut stack = vec![self.root.clone()];
        while let Some(node) = stack.pop() {
            let mut n = node.borrow_mut();
            n.git = if self.conf.git_enabled && !n.create_entry {
                self.git_cache.state(&n.path)
            } else {
                Default::default()
            };
            stack.extend(n.children.iter().cloned());
        }
    }
    pub fn refresh(&mut self) {
        self.git_cache.clear();
        self.search_key = None;
        let selected = self.selected.borrow().path.clone();
        let mut stack = vec![self.root.clone()];
        while let Some(node) = stack.pop() {
            if node.borrow().expanded {
                self.expand(&node, None);
                stack.extend(node.borrow().children.iter().cloned())
            }
        }
        self.selected = self
            .visible()
            .into_iter()
            .find(|n| n.borrow().path == selected)
            .unwrap_or_else(|| self.root.clone());
    }
    pub fn step(&mut self, direction: isize) {
        let mut depth = 0;
        let mut ancestor = self.selected.clone();
        loop {
            let parent = ancestor.borrow().parent.upgrade();
            let Some(parent) = parent else {
                break;
            };
            depth += 1;
            ancestor = parent;
        }
        let mut nodes = Vec::new();
        let mut stack = vec![(self.root.clone(), 0)];
        while let Some((node, level)) = stack.pop() {
            if level == depth {
                nodes.push(node);
            } else if level < depth && node.borrow().expanded {
                stack.extend(
                    node.borrow()
                        .children
                        .iter()
                        .rev()
                        .map(|child| (child.clone(), level + 1)),
                );
            }
        }
        if nodes.is_empty() {
            return;
        }
        let index = nodes
            .iter()
            .position(|n| Rc::ptr_eq(n, &self.selected))
            .unwrap_or(0);
        let next = (index as isize + direction).rem_euclid(nodes.len() as isize);
        self.selected = nodes[next as usize].clone();
    }
    pub fn right(&mut self) {
        let node = self.selected.clone();
        if node.borrow().expanded {
            let child = node.borrow().children.first().cloned();
            if let Some(child) = child {
                self.selected = child
            }
        } else {
            self.expand(&node, None)
        }
    }
    pub fn left(&mut self) {
        if self.selected.borrow().expanded {
            self.selected.borrow_mut().expanded = false
        }
    }
    pub fn back(&mut self) -> Result<String, String> {
        let parent = self.selected.borrow().parent.upgrade();
        if let Some(parent) = parent {
            self.selected = parent;
            Ok(String::new())
        } else {
            let message = self.ascend()?;
            self.selected = self.root.clone();
            Ok(message)
        }
    }
    pub fn set_root(&mut self, value: &str) -> Result<(), String> {
        let replacement = Self::new(&expand_home(value), self.conf.clone())?;
        *self = replacement;
        Ok(())
    }
    pub fn focus_root(&mut self) -> String {
        let node = if self.selected.borrow().dir && !self.selected.borrow().link {
            self.selected.clone()
        } else {
            self.selected
                .borrow()
                .parent
                .upgrade()
                .unwrap_or_else(|| self.root.clone())
        };
        node.borrow_mut().parent = Weak::new();
        node.borrow_mut().expanded = true;
        self.root = node.clone();
        self.selected = node;
        self.results.clear();
        self.search_key = None;
        self.search_summary.clear();
        self.refresh();
        ui_format!(
            "当前目录设为根: {}",
            "Current folder set as root: {}",
            self.root.borrow().path.display()
        )
    }
    pub fn ascend(&mut self) -> Result<String, String> {
        let old = self.root.clone();
        let path = old
            .borrow()
            .path
            .parent()
            .map(Path::to_path_buf)
            .ok_or(label("已在文件系统根目录", "Already at filesystem root"))?;
        let selected = self.selected.clone();
        let new = Node::new(path.clone(), Weak::new(), true, false);
        // Parent listing is bounded; the old subtree is spliced back intact.
        let old_path = old.borrow().path.clone();
        self.expand(&new, Some(&old_path));
        if !new.borrow().expanded {
            return Err(new.borrow().error.clone());
        }
        {
            let mut parent = new.borrow_mut();
            if let Some(index) = parent
                .children
                .iter()
                .position(|n| n.borrow().path == old_path)
            {
                parent.children[index] = old.clone();
            } else {
                parent.children.push(old.clone());
                parent.omitted = parent.omitted.saturating_sub(1);
            }
        }
        old.borrow_mut().parent = Rc::downgrade(&new);
        self.root = new;
        self.selected = selected;
        self.search_key = None;
        Ok(ui_format!(
            "引入上级目录: {}",
            "Parent introduced: {}",
            path.display()
        ))
    }
    pub fn expand_all(&mut self) -> Result<String, String> {
        if !self.selected.borrow().dir || self.selected.borrow().link {
            return Err(label("请选择普通目录", "Select a regular directory").into());
        }
        let mut stack = vec![self.selected.clone()];
        let mut count = 0;
        let mut limited = false;
        while let Some(node) = stack.pop() {
            if count >= self.conf.max_expand {
                limited = true;
                break;
            }
            count += 1;
            if node.borrow().dir && !node.borrow().link {
                self.expand(&node, None);
                limited |= node.borrow().omitted > 0;
                stack.extend(node.borrow().children.iter().rev().cloned());
            }
        }
        Ok(ui_format!(
            "已访问 {count} 个节点{}",
            "Visited {count} nodes{}",
            if limited {
                label(
                    "；达到显示 / 缓存 / 展开上限",
                    "; display / cache / expansion limit reached",
                )
            } else {
                ""
            }
        ))
    }
    pub fn collapse_all(&mut self) -> Result<String, String> {
        if !self.selected.borrow().dir {
            return Err(label("请选择目录", "Select a directory").into());
        }
        let mut stack = vec![self.selected.clone()];
        while let Some(node) = stack.pop() {
            let mut n = node.borrow_mut();
            n.expanded = false;
            stack.extend(n.children.iter().cloned());
        }
        Ok(label(
            "已收起选中目录的全部分支",
            "All selected folder branches collapsed",
        )
        .into())
    }
    pub fn search(&mut self, query: &str) {
        let root = self.root.borrow().path.clone();
        let mut stack = vec![];
        if let Ok(read) = fs::read_dir(&root) {
            stack.push(read)
        }
        let mut results = vec![];
        let (mut scanned, mut skipped, mut limited) = (0, 0, false);
        let needle = query.to_lowercase();
        while !stack.is_empty() {
            let entry = stack.last_mut().unwrap().next();
            let item = match entry {
                None => {
                    stack.pop();
                    continue;
                }
                Some(Err(_)) => {
                    skipped += 1;
                    continue;
                }
                Some(Ok(item)) => item,
            };
            scanned += 1;
            if scanned > self.conf.search_limit {
                limited = true;
                break;
            }
            let name = item.file_name().to_string_lossy().into_owned();
            if !self.conf.hidden && name.starts_with('.') {
                continue;
            }
            let path = item.path();
            if name.to_lowercase().contains(&needle) {
                results.push(path.clone());
                if results.len() >= self.conf.search_results {
                    limited = true;
                    break;
                }
            }
            if item.file_type().is_ok_and(|v| v.is_dir()) {
                if stack.len() >= 128 {
                    skipped += 1;
                    continue;
                }
                match fs::read_dir(path) {
                    Ok(read) => stack.push(read),
                    Err(_) => skipped += 1,
                }
            }
        }
        results.sort_by_key(|p| (p.components().count(), p.to_string_lossy().to_lowercase()));
        self.results = results;
        self.search_key = Some((query.into(), root, self.conf.hidden));
        self.search_summary = ui_format!(
            "{} 个结果 · 检查 {scanned} 项{}{}",
            "{} results - scanned {scanned} entries{}{}",
            self.results.len(),
            if limited {
                label(" · 已达搜索上限", " - search limit reached")
            } else {
                ""
            },
            if skipped > 0 {
                ui_format!(
                    " · 跳过 {skipped} 个不可读 / 过深目录",
                    " - skipped {skipped} unreadable / overly deep folders"
                )
            } else {
                String::new()
            }
        );
    }
    pub fn reveal(&mut self, path: &Path) -> Result<(), String> {
        let root = self.root.borrow().path.clone();
        let relative = path.strip_prefix(&root).map_err(|e| e.to_string())?;
        let mut node = self.root.clone();
        for part in relative.components() {
            let target = node.borrow().path.join(part);
            self.expand(&node, Some(&target));
            let next = node
                .borrow()
                .children
                .iter()
                .find(|n| n.borrow().path == target)
                .cloned()
                .ok_or(label(
                    "无法定位，文件已变更或达到缓存上限",
                    "Cannot locate: file changed or cache limit reached",
                ))?;
            node = next;
        }
        self.selected = node;
        Ok(())
    }
    pub fn find_next(&mut self, query: &str, direction: isize) -> Result<String, String> {
        let signature = (
            query.into(),
            self.root.borrow().path.clone(),
            self.conf.hidden,
        );
        let fresh = self.search_key.as_ref() != Some(&signature);
        if fresh {
            self.search(query)
        }
        if self.results.is_empty() {
            return Ok(ui_format!(
                "没有匹配；{}",
                "No matches; {}",
                self.search_summary
            ));
        }
        let selected = self.selected.borrow().path.clone();
        let index = if !fresh {
            self.results
                .iter()
                .position(|p| *p == selected)
                .map_or(0, |i| {
                    (i as isize + direction).rem_euclid(self.results.len() as isize) as usize
                })
        } else {
            0
        };
        let path = self.results[index].clone();
        self.reveal(&path)?;
        Ok(ui_format!(
            "{}/{} · {}",
            "{}/{} - {}",
            index + 1,
            self.results.len(),
            self.search_summary
        ))
    }
    pub fn target(&self) -> PathBuf {
        let node = self.selected.borrow();
        if node.dir || node.create_entry {
            node.path.clone()
        } else {
            node.path.parent().unwrap_or(Path::new("/")).to_path_buf()
        }
    }
}
pub struct Placement {
    pub node: NodeRef,
    pub x: usize,
    pub y: usize,
    pub label: String,
    pub link_start: Option<usize>,
}
pub struct Layout {
    pub items: Vec<Placement>,
    pub lines: Vec<(usize, usize, usize, usize)>,
    pub width: usize,
    pub height: usize,
}
impl Layout {
    /// Ancestor edges only, with junction shapes retained by the normal canvas.
    #[allow(clippy::too_many_arguments)]
    pub fn path_cells(
        &self,
        selected: &NodeRef,
        gap: usize,
        left: usize,
        top: usize,
        width: usize,
        height: usize,
    ) -> HashSet<(usize, usize)> {
        let mut segments = Vec::new();
        let mut node = selected.clone();
        loop {
            let parent = node.borrow().parent.upgrade();
            let Some(parent) = parent else {
                break;
            };
            let Some(child) = self.items.iter().find(|item| Rc::ptr_eq(&item.node, &node)) else {
                break;
            };
            let Some(p) = self
                .items
                .iter()
                .find(|item| Rc::ptr_eq(&item.node, &parent))
            else {
                break;
            };
            let first = parent.borrow().children.first().cloned();
            let Some(first) = first.and_then(|first| {
                self.items
                    .iter()
                    .find(|item| Rc::ptr_eq(&item.node, &first))
            }) else {
                break;
            };
            let branch = first.x - gap + 2;
            segments.extend([
                (p.x + p.label.width(), p.y, branch, p.y),
                (branch, p.y, branch, child.y),
                (branch, child.y, child.x - 1, child.y),
            ]);
            node = parent;
        }
        let route = Layout {
            items: vec![],
            lines: segments,
            width: self.width,
            height: self.height,
        };
        route.cells(left, top, width, height).into_keys().collect()
    }
    pub fn new(browser: &Browser) -> Self {
        let nodes = browser.visible();
        let boxed = nodes.iter().any(|n| n.borrow().marked);
        let row_spacing = browser.conf.row_spacing.max(if boxed { 3 } else { 1 });
        let mut depths = HashMap::new();
        let mut widths: Vec<usize> = vec![];
        let mut labels = HashMap::new();
        for node in &nodes {
            let n = node.borrow();
            let id = Rc::as_ptr(node) as usize;
            let depth = n
                .parent
                .upgrade()
                .and_then(|p| depths.get(&(Rc::as_ptr(&p) as usize)).copied())
                .map_or(0, |v: usize| v + 1);
            depths.insert(id, depth);
            let (label, link_start) = n.label(browser.conf.label_width);
            if widths.len() <= depth {
                widths.resize(depth + 1, 0)
            }
            widths[depth] = widths[depth].max(label.width());
            labels.insert(id, (label, link_start));
        }
        let mut columns = vec![0; widths.len()];
        for d in 1..columns.len() {
            columns[d] = columns[d - 1] + widths[d - 1] + browser.conf.gap;
        }
        let mut heights = HashMap::new();
        for node in nodes.iter().rev() {
            let n = node.borrow();
            let h = if n.expanded && !n.children.is_empty() {
                n.children
                    .iter()
                    .map(|c| heights[&(Rc::as_ptr(c) as usize)])
                    .sum()
            } else {
                row_spacing
            };
            heights.insert(Rc::as_ptr(node) as usize, h);
        }
        let mut rows = HashMap::new();
        rows.insert(Rc::as_ptr(&browser.root) as usize, usize::from(boxed));
        let mut items = vec![];
        let mut locations = HashMap::new();
        let mut width = 0;
        let mut height = 0;
        for node in nodes {
            let id = Rc::as_ptr(&node) as usize;
            let x = columns[depths[&id]];
            let y = rows[&id];
            let (label, link_start) = labels.remove(&id).unwrap();
            width = width.max(x + label.width());
            height = height.max(y + if boxed { 2 } else { 1 });
            locations.insert(id, (x, y, label.width()));
            {
                let n = node.borrow();
                if n.expanded {
                    let mut row = y;
                    for child in &n.children {
                        let child_id = Rc::as_ptr(child) as usize;
                        rows.insert(child_id, row);
                        row += heights[&child_id]
                    }
                }
            }
            items.push(Placement {
                node,
                x,
                y,
                label,
                link_start,
            });
        }
        let mut lines = vec![];
        for item in &items {
            let n = item.node.borrow();
            if n.expanded && !n.children.is_empty() {
                let first = locations[&(Rc::as_ptr(&n.children[0]) as usize)];
                let last = locations[&(Rc::as_ptr(n.children.last().unwrap()) as usize)];
                let branch = first.0 - browser.conf.gap + 2;
                lines.push((item.x + item.label.width(), item.y, branch, item.y));
                lines.push((branch, item.y, branch, last.1));
                for child in &n.children {
                    let (x, y, _) = locations[&(Rc::as_ptr(child) as usize)];
                    lines.push((branch, y, x - 1, y));
                }
            }
        }
        Self {
            items,
            lines,
            width,
            height,
        }
    }
    pub fn cells(
        &self,
        left: usize,
        top: usize,
        width: usize,
        height: usize,
    ) -> HashMap<(usize, usize), char> {
        let mut masks: HashMap<(usize, usize), u8> = HashMap::new();
        for &(x1, y1, x2, y2) in &self.lines {
            if y1 == y2 && y1 >= top && y1 < top + height {
                for x in x1.max(left)..=x2.min(left + width.saturating_sub(1)) {
                    let mask = if x > x1 { 8 } else { 0 } | if x < x2 { 2 } else { 0 };
                    *masks.entry((x, y1)).or_default() |= mask;
                }
            } else if x1 == x2 && x1 >= left && x1 < left + width {
                for y in y1.max(top)..=y2.min(top + height.saturating_sub(1)) {
                    let mask = if y > y1 { 1 } else { 0 } | if y < y2 { 4 } else { 0 };
                    *masks.entry((x1, y)).or_default() |= mask;
                }
            }
        }
        masks
            .into_iter()
            .map(|(pos, m)| {
                (
                    pos,
                    crate::terminal_profile::glyph(match m {
                        1 | 4 | 5 => '│',
                        3 => '└',
                        6 => '┌',
                        9 => '┘',
                        12 => '┐',
                        7 => '├',
                        11 => '┴',
                        13 => '┤',
                        14 => '┬',
                        15 => '┼',
                        _ => '─',
                    }),
                )
            })
            .collect()
    }
    pub fn plain(&self) -> String {
        let cells = self.cells(0, 0, self.width, self.height);
        let mut output = String::new();
        for y in 0..self.height {
            let mut x = 0;
            for item in self.items.iter().filter(|p| p.y == y) {
                while x < item.x {
                    output.push(*cells.get(&(x, y)).unwrap_or(&' '));
                    x += 1
                }
                output.push_str(&item.label);
                x += item.label.width();
            }
            while x < self.width {
                output.push(*cells.get(&(x, y)).unwrap_or(&' '));
                x += 1
            }
            while output.ends_with(' ') {
                output.pop();
            }
            output.push('\n');
        }
        output
    }
}
