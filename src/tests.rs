use crate::{
    config::{Config, key_code},
    files,
    model::{Browser, Layout},
};
use std::{
    fs,
    path::PathBuf,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "filex-rust-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(path.join("left/deep")).unwrap();
        fs::create_dir_all(path.join("right/deep")).unwrap();
        fs::write(path.join("left/deep/needle 中文.PY"), "hello 世界").unwrap();
        fs::write(path.join("right/deep/needle 中文.PY"), "second").unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn tree_search_preserves_expanded_branches() {
    let f = Fixture::new();
    let mut b = Browser::new(&f.0, Config::default()).unwrap();
    let left = b
        .root
        .borrow()
        .children
        .iter()
        .find(|n| n.borrow().path.ends_with("left"))
        .unwrap()
        .clone();
    b.selected = left.clone();
    b.expand_all().unwrap();
    b.find_next("needle", 1).unwrap();
    b.find_next("needle", 1).unwrap();
    assert!(b.selected.borrow().path.to_string_lossy().contains("right"));
    assert!(left.borrow().expanded);
    assert!(left.borrow().children[0].borrow().expanded);
    assert_eq!(b.results.len(), 2);
    assert!(Layout::new(&b).plain().contains("needle"));
}
#[test]
fn parent_wraps_same_tree_and_selected_file() {
    let f = Fixture::new();
    let mut b = Browser::new(&f.0, Config::default()).unwrap();
    b.expand_all().unwrap();
    b.find_next("needle", 1).unwrap();
    let root = b.root.clone();
    let selected = b.selected.clone();
    b.ascend().unwrap();
    assert!(
        b.root
            .borrow()
            .children
            .iter()
            .any(|n| Rc::ptr_eq(n, &root))
    );
    assert!(Rc::ptr_eq(&b.selected, &selected));
    assert!(root.borrow().expanded);
    assert!(b.visible().iter().any(|n| Rc::ptr_eq(n, &selected)));
}
#[test]
fn collapse_expand_and_symlinks() {
    let f = Fixture::new();
    std::os::unix::fs::symlink(&f.0, f.0.join("loop")).unwrap();
    let mut b = Browser::new(&f.0, Config::default()).unwrap();
    b.expand_all().unwrap();
    assert!(b.visible().len() > 3);
    b.collapse_all().unwrap();
    assert_eq!(b.visible().len(), 1);
    b.expand_all().unwrap();
    let link = b.visible().into_iter().find(|n| n.borrow().link).unwrap();
    assert!(!link.borrow().expanded);
}
#[test]
fn hidden_limits_and_selection_beyond_display_limit() {
    let f = Fixture::new();
    fs::create_dir(f.0.join(".hidden")).unwrap();
    fs::write(f.0.join(".hidden/needle"), "hidden").unwrap();
    let c = Config {
        max_entries: 1,
        ..Default::default()
    };
    let mut b = Browser::new(&f.0, c).unwrap();
    assert_eq!(b.root.borrow().children.len(), 1);
    b.find_next("needle", 1).unwrap();
    b.find_next("needle", 1).unwrap();
    assert!(b.selected.borrow().path.to_string_lossy().contains("right"));
    b.conf.hidden = true;
    b.search("needle");
    assert_eq!(b.results.len(), 3);
    b.conf.search_results = 1;
    b.search("needle");
    assert_eq!(b.results.len(), 1);
    assert!(b.search_summary.contains("上限"));
}
#[test]
fn filesystem_no_overwrite_self_copy_and_symlinks() {
    let f = Fixture::new();
    let source = f.0.join("left/deep/needle 中文.PY");
    let dest = f.0.join("copy ; $(command).py");
    files::transfer(&source, &dest, false).unwrap();
    assert!(source.exists());
    assert!(files::transfer(&source, &dest, false).is_err());
    let renamed = f.0.join("renamed.py");
    files::rename(&dest, &renamed).unwrap();
    assert!(!dest.exists());
    assert!(files::transfer(&f.0.join("left"), &f.0.join("left/inside"), false).is_err());
    let link = f.0.join("link");
    std::os::unix::fs::symlink("missing", &link).unwrap();
    let copy = f.0.join("link-copy");
    files::transfer(&link, &copy, false).unwrap();
    assert_eq!(fs::read_link(copy).unwrap(), PathBuf::from("missing"));
}
#[test]
fn creation_preview_and_metadata() {
    let f = Fixture::new();
    assert!(files::create(&f.0, "../bad", false).is_err());
    let path = files::create(&f.0, "created.txt", false).unwrap();
    assert!(files::create(&f.0, "created.txt", false).is_err());
    fs::write(&path, "世界\n".repeat(2000)).unwrap();
    let preview = files::preview(&path, 1024);
    assert!(preview.contains("截断"));
    assert!(preview.contains("世界"));
    let metadata = files::metadata(&path);
    assert!(metadata.contains("Created"));
    assert!(metadata.contains("Modified"));
    assert!(metadata.contains("Size"));
}
#[test]
fn config_validation_and_long_suffix() {
    let f = Fixture::new();
    let path = f.0.join("settings.conf");
    fs::write(&path,"[keys]\ndashboard='Ctrl+d'\n[open_defaults]\n'.tar.gz'='vim'\n[extension_colors]\n'.tar.gz'='#123456'\n").unwrap();
    let conf = Config::load(&path).unwrap();
    assert_eq!(conf.action("\x04"), Some("dashboard"));
    assert_eq!(
        conf.rule(&f.0.join("X.TAR.GZ"), &conf.defaults, false)
            .unwrap(),
        "vim"
    );
    assert_eq!(key_code("中文"), "中文");
    for content in [
        "[keys]\nquit='j'",
        "[openers.bad]\ncommand=['echo']",
        "[open_defaults]\n'.py'='missing'",
        "[what]",
        "[layout]\ncolumn_gap=true",
    ] {
        fs::write(&path, content).unwrap();
        assert!(Config::load(&path).is_err(), "{content}");
    }
    Config::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fileX.conf")).unwrap();
}
#[test]
fn node_cache_remains_bounded() {
    let f = Fixture::new();
    for i in 0..30 {
        let dir = f.0.join(format!("branch{i}"));
        fs::create_dir(&dir).unwrap();
        for j in 0..30 {
            fs::write(dir.join(format!("file{j}")), "").unwrap();
        }
    }
    let conf = Config {
        max_loaded: 100,
        max_expand: 10000,
        ..Default::default()
    };
    let mut b = Browser::new(&f.0, conf).unwrap();
    b.expand_all().unwrap();
    assert!(b.visible().len() <= 100);
}

#[test]
fn parent_keeps_configured_directory_first_order() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("000-file.txt"), "file").unwrap();
    let mut browser = Browser::new(&fixture.0.join("left"), Config::default()).unwrap();
    browser.ascend().unwrap();
    let root = browser.root.borrow();
    assert!(root.children.first().unwrap().borrow().dir);
    assert!(!root.children.last().unwrap().borrow().dir);
}

#[test]
fn sibling_arrows_skip_open_descendants_and_back_keeps_expansion() {
    let fixture = Fixture::new();
    let mut browser = Browser::new(&fixture.0, Config::default()).unwrap();
    let root = browser.root.clone();
    browser.expand_all().unwrap();
    browser.right();
    let left = browser.selected.clone();
    assert!(left.borrow().path.ends_with("left"));
    browser.step(1);
    assert!(browser.selected.borrow().path.ends_with("right"));
    browser.step(-1);
    assert!(Rc::ptr_eq(&browser.selected, &left));
    browser.right();
    assert!(browser.selected.borrow().path.ends_with("left/deep"));
    browser.step(1);
    assert!(browser.selected.borrow().path.ends_with("left/deep"));
    browser.back().unwrap();
    assert!(Rc::ptr_eq(&browser.selected, &left));
    assert!(left.borrow().expanded);
    browser.back().unwrap();
    assert!(Rc::ptr_eq(&browser.selected, &root));
    browser.back().unwrap();
    assert!(Rc::ptr_eq(&browser.selected, &browser.root));
    assert!(root.borrow().expanded);
    assert!(left.borrow().expanded);
}

#[test]
fn vertical_tree_and_old_parent_binding_migrate() {
    let fixture = Fixture::new();
    let path = fixture.0.join("settings.conf");
    fs::write(
        &path,
        "[layout]\norientation='vertical'\n[keys]\nparent='u'\n[tmux]\nopen_in_right_pane=false",
    )
    .unwrap();
    let conf = Config::load(&path).unwrap();
    assert_eq!(conf.action("left"), Some("parent"));
    assert_eq!(conf.action("u"), None);
    assert!(!conf.tmux_right);
    let mut browser = Browser::new(&fixture.0, conf).unwrap();
    browser.expand_all().unwrap();
    let selected = browser.selected.clone();
    let layout = Layout::new(&browser);
    let parent = &layout.items[0];
    let children: Vec<_> = layout
        .items
        .iter()
        .filter(|item| {
            item.node
                .borrow()
                .parent
                .upgrade()
                .is_some_and(|node| Rc::ptr_eq(&node, &browser.root))
        })
        .collect();
    assert!(children.iter().all(|child| child.y > parent.y));
    assert_eq!(children[0].y, children[1].y);
    assert!(children[0].x < children[1].x);
    let center = parent.x + unicode_width::UnicodeWidthStr::width(parent.label.as_str()) / 2;
    let cells = layout.cells(0, 0, layout.width, layout.height);
    assert!(matches!(
        cells.get(&(center, children[0].y - 1)),
        Some('┴' | '┼')
    ));
    assert!(layout.plain().contains('┌'));
    assert!(layout.plain().contains('┐'));
    browser.conf.orientation = "horizontal".into();
    let horizontal = Layout::new(&browser);
    assert_eq!(horizontal.items[0].y, horizontal.items[1].y);
    assert!(Rc::ptr_eq(&browser.selected, &selected));
}

#[test]
fn focus_root_keeps_selected_folder_branch_and_uses_files_parent() {
    let fixture = Fixture::new();
    let mut browser = Browser::new(&fixture.0, Config::default()).unwrap();
    browser.expand_all().unwrap();
    browser.right();
    let folder = browser.selected.clone();
    let deep = folder.borrow().children[0].clone();
    browser.focus_root();
    assert!(Rc::ptr_eq(&browser.root, &folder));
    assert!(Rc::ptr_eq(&browser.selected, &folder));
    assert!(browser.root.borrow().parent.upgrade().is_none());
    assert!(deep.borrow().expanded);
    assert!(Rc::ptr_eq(&browser.root.borrow().children[0], &deep));
    assert!(
        browser
            .visible()
            .iter()
            .all(|node| node.borrow().path.starts_with(folder.borrow().path.clone()))
    );
    browser.find_next("needle", 1).unwrap();
    browser.focus_root();
    assert!(Rc::ptr_eq(&browser.root, &deep));
    assert!(Rc::ptr_eq(&browser.selected, &deep));
}
