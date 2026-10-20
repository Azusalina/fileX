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
#[test]
fn semantic_theme_is_distinct_in_shared_sixteen_color_mode() {
    let conf = Config::default();
    let samples = [
        "main.PY",
        "run.sh",
        "settings.toml",
        "readme.md",
        "photo.webp",
        "video.mkv",
        "backup.tar.gz",
    ];
    let mut normal = std::collections::HashSet::new();
    let mut subdued = std::collections::HashSet::new();
    for name in samples {
        let color = conf
            .rule(std::path::Path::new(name), &conf.extension_colors, false)
            .unwrap();
        let part = |start| u8::from_str_radix(&color[start..start + 2], 16).unwrap();
        let rgb = (part(1), part(3), part(5));
        normal.insert(crate::terminal_profile::shared_color(rgb, false, false).0);
        subdued.insert(crate::terminal_profile::shared_color(rgb, false, true).0);
    }
    assert_eq!(normal.len(), samples.len());
    assert_eq!(subdued.len(), samples.len());
    assert!(conf.extension_colors.len() >= 100);
}
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
    let metadata = files::metadata(&path, &f.0, &Config::default());
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
    assert_eq!(key_code("CTRL+F5"), "ctrl+f5");
    assert_eq!(Config::default().action("ctrl+f5"), Some("refresh"));
    fs::write(&path, "[keys]\nrefresh='Ctrl+F5'\n").unwrap();
    assert_eq!(
        Config::load(&path).unwrap().action("ctrl+f5"),
        Some("refresh")
    );
    let defaults = Config::default();
    for name in ["sample.js", "sample.MJS"] {
        assert_eq!(
            defaults
                .rule(&f.0.join(name), &defaults.extension_colors, false)
                .map(String::as_str),
            Some("#FFD700")
        );
    }
    fs::write(&path, "[extension_colors]\n'.js'='#ffd700'\n").unwrap();
    let conf = Config::load(&path).unwrap();
    assert_eq!(
        conf.rule(&f.0.join("test.JS"), &conf.extension_colors, false)
            .map(String::as_str),
        Some("#ffd700")
    );
    for content in [
        "[keys]\nquit='j'",
        "[keys]\ncopy='Ctrl+F5'",
        "[openers.bad]\ncommand=['echo']",
        "[open_defaults]\n'.py'='missing'",
        "[what]",
        "[layout]\ncolumn_gap=true",
        "[extension_colors]\n'.js'='FFD700'",
    ] {
        fs::write(&path, content).unwrap();
        assert!(Config::load(&path).is_err(), "{content}");
    }
    Config::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fileX.example.conf")).unwrap();
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
    assert!(browser.selected.borrow().path.ends_with("right/deep"));
    browser.step(-1);
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
fn old_config_migrates_to_horizontal_and_new_shortcuts() {
    let fixture = Fixture::new();
    let path = fixture.0.join("settings.conf");
    fs::write(&path, "[layout]\norientation='vertical'\n[keys]\nparent='u'\ntoggle_tree='v'\nrefresh='r'\nrename='R'\n[tmux]\nopen_in_right_pane=false").unwrap();
    let conf = Config::load(&path).unwrap();
    assert_eq!(conf.action("left"), Some("parent"));
    assert_eq!(conf.action("u"), None);
    assert_eq!(conf.action("v"), Some("toggle_grouped"));
    assert_eq!(conf.action("r"), Some("rename"));
    assert_eq!(conf.action("R"), Some("rename"));
    assert_eq!(conf.action("\x12"), Some("refresh"));
    assert_eq!(conf.action("ctrl+alt+c"), Some("copy_path"));
    assert!(!conf.tmux_right);
    assert_eq!(crate::config::key_code("Ctrl+Alt+C"), "ctrl+alt+c");
    assert_eq!(conf.arrow_action("up"), Some("up"));
    let mut browser = Browser::new(&fixture.0, conf).unwrap();
    browser.expand_all().unwrap();
    let layout = Layout::new(&browser);
    assert!(layout.items[0].y < layout.items[1].y);
    assert!(layout.items[1].x > layout.items[0].x);
}

#[test]
fn visible_level_navigation_crosses_parents_and_wraps_both_ends() {
    let f = Fixture::new();
    fs::write(f.0.join("left/local.txt"), "mixed file at directory level").unwrap();
    let mut browser = Browser::new(&f.0, Config::default()).unwrap();
    browser.expand_all().unwrap();
    let find = |browser: &Browser, path: &str| {
        browser
            .visible()
            .into_iter()
            .find(|node| node.borrow().path == f.0.join(path))
            .unwrap()
    };
    let first = find(&browser, "left/deep");
    let last = find(&browser, "right/deep");
    browser.selected = first.clone();
    browser.step(-1);
    assert!(Rc::ptr_eq(&browser.selected, &last));
    browser.step(1);
    assert!(Rc::ptr_eq(&browser.selected, &first));
    browser.step(1);
    assert!(browser.selected.borrow().path.ends_with("left/local.txt"));
    browser.step(1);
    assert!(Rc::ptr_eq(&browser.selected, &last));
    browser.back().unwrap();
    assert!(browser.selected.borrow().path.ends_with("right"));
    browser.selected = find(&browser, "left/deep/needle 中文.PY");
    browser.step(1);
    assert!(
        browser
            .selected
            .borrow()
            .path
            .ends_with("right/deep/needle 中文.PY")
    );
    browser.step(1);
    assert!(
        browser
            .selected
            .borrow()
            .path
            .ends_with("left/deep/needle 中文.PY")
    );
    let right = find(&browser, "right");
    right.borrow_mut().expanded = false;
    browser.selected = first.clone();
    browser.step(-1);
    assert!(browser.selected.borrow().path.ends_with("left/local.txt"));
    assert!(first.borrow().expanded);
    assert!(last.borrow().expanded);
    browser.selected = browser.root.clone();
    browser.step(-1);
    assert!(Rc::ptr_eq(&browser.selected, &browser.root));
}

#[test]
fn dashboard_width_and_exact_folder_name_colors_are_configurable() {
    let f = Fixture::new();
    let path = f.0.join("settings.conf");
    fs::write(&path, "[dashboard]\ndashboard_width=100\n[colors]\npath_color=' #9cc5d1 '\n[extension_colors]\n'.js'=' #FFD700 '\n[folder_colors]\nnode_modules='#123456'\nCustom='#FFD700'\n'.git'=' #FFFEF7 '\n__pycache__=' #ADD8E6'\n").unwrap();
    let conf = Config::load(&path).unwrap();
    assert_eq!(conf.dashboard_columns(140), 100);
    assert_eq!(conf.dashboard_columns(100), 62);
    assert_eq!(conf.dashboard_columns(70), 32);
    assert_eq!(conf.dashboard_columns(69), 0);
    assert_eq!(conf.folder_color_for(&f.0.join("node_modules")), "#123456");
    assert_eq!(conf.folder_color_for(&f.0.join("Custom")), "#FFD700");
    assert_eq!(conf.folder_color_for(&f.0.join(".git")), "#FFFEF7");
    assert_eq!(conf.folder_color_for(&f.0.join("__pycache__")), "#ADD8E6");
    assert_eq!(conf.path_color, "#9cc5d1");
    assert_eq!(conf.extension_colors[".js"], "#FFD700");
    assert_eq!(
        conf.folder_color_for(&f.0.join("custom")),
        conf.folder_color
    );
    assert_eq!(conf.keys["dashboard_narrow"], "[");
    assert_eq!(conf.keys["dashboard_widen"], "]");
    for invalid in [
        "[dashboard]\ndashboard_width=19",
        "[dashboard]\ndashboard_width=241",
        "[folder_colors]\nnode_modules='red'",
        "[folder_colors]\n'a/b'='#123456'",
        "[folder_colors]\n''='#123456'",
    ] {
        fs::write(&path, invalid).unwrap();
        assert!(Config::load(&path).is_err(), "{invalid}");
    }
    fs::write(&path, "[keys]\ncopy='['\n").unwrap();
    assert_eq!(Config::load(&path).unwrap().keys["dashboard_narrow"], "{");
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

#[test]
fn python_runner_and_extra_extension_colors() {
    let conf = Config::default();
    let fixture = Fixture::new();
    let script = fixture.0.join("script with 空格.py");
    fs::write(&script, "print('hello')").unwrap();
    let (command, terminal) = files::open_command(&script, &conf, "python").unwrap();
    assert!(terminal);
    assert_eq!(command.get_program(), "python");
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        vec![script.as_os_str()]
    );
    assert_eq!(command.get_current_dir(), Some(fixture.0.as_path()));
    assert_eq!(conf.defaults[".md"], "markdown");
    assert!(conf.openers["markdown"].copy_path);
    for extension in [".r", ".cpp", ".sh", ".mp3", ".mp4"] {
        assert!(conf.extension_colors.contains_key(extension));
    }
}

#[test]
fn link_details_bounded_peers_and_visible_badges() {
    use std::os::unix::fs::{MetadataExt, symlink};
    let f = Fixture::new();
    let name = "very_long_file_name_with_Chinese_中文.txt";
    let path = f.0.join(name);
    fs::write(&path, "same inode").unwrap();
    let peer = f.0.join(".hidden-hard-link");
    fs::hard_link(&path, &peer).unwrap();
    fs::hard_link(&path, f.0.join("second-hard-link")).unwrap();
    symlink(name, f.0.join("soft")).unwrap();
    symlink("missing", f.0.join("broken")).unwrap();
    symlink(&f.0, f.0.join("directory-link")).unwrap();
    let conf = Config::default();
    let text = files::metadata(&path, &f.0, &conf);
    assert!(text.contains("硬链接: 3"));
    assert!(text.contains(&peer.to_string_lossy().to_string()));
    let info = fs::symlink_metadata(&path).unwrap();
    assert_eq!(info.nlink(), 3);
    let limited = crate::links::peers(&path, &f.0, &info, 1, 32);
    assert!(limited.limited);
    assert!(limited.scanned <= 1);
    let capped = crate::links::peers(&path, &f.0, &info, 10000, 1);
    assert_eq!(capped.paths.len(), 1);
    assert!(capped.limited);
    let soft = files::metadata(&f.0.join("soft"), &f.0, &conf);
    assert!(soft.contains(&format!("软链接 → {name}")));
    assert!(soft.contains("状态: 有效"));
    assert!(files::metadata(&f.0.join("broken"), &f.0, &conf).contains("状态: 断开"));
    let mut browser = Browser::new(&f.0, conf).unwrap();
    let node = browser
        .root
        .borrow()
        .children
        .iter()
        .find(|node| node.borrow().path == path)
        .unwrap()
        .clone();
    assert!(node.borrow().label(14).0.ends_with("H×3"));
    assert!(!browser.root.borrow().label(32).0.contains("H×"));
    fs::remove_file(&peer).unwrap();
    browser.refresh();
    assert_eq!(node.borrow().links, 2);
}

#[test]
fn symlink_target_has_separate_bounded_label_segment() {
    let f = Fixture::new();
    fs::write(f.0.join("b.txt"), "target").unwrap();
    std::os::unix::fs::symlink("b.txt", f.0.join("a.js")).unwrap();
    std::os::unix::fs::symlink("missing", f.0.join("broken")).unwrap();
    std::os::unix::fs::symlink(
        "../long/path/目标\nfile.txt",
        f.0.join("中文_very_long_source.js"),
    )
    .unwrap();
    fs::write(f.0.join("ordinary------b"), "ordinary").unwrap();
    let browser = Browser::new(&f.0, Config::default()).unwrap();
    let layout = Layout::new(&browser);
    let item = layout
        .items
        .iter()
        .find(|item| item.node.borrow().path.ends_with("a.js"))
        .unwrap();
    assert_eq!(item.label, "· a.js------b.txt");
    assert_eq!(&item.label[item.link_start.unwrap()..], "------b.txt");
    for item in &layout.items {
        if item.node.borrow().link {
            assert!(
                unicode_width::UnicodeWidthStr::width(item.label.as_str())
                    <= browser.conf.label_width
            );
            assert!(!item.label.contains('\n'));
        } else {
            assert!(item.link_start.is_none());
        }
    }
    assert!(layout.plain().contains("broken------missing"));
    assert_eq!(browser.conf.link_color, "#4B0082");
}

#[test]
fn creation_shortcuts_can_be_configured_without_modifier_loss() {
    assert_eq!(key_code("Ctrl+Shift+N"), "ctrl+shift+n");
    assert_eq!(Config::default().action("\x0e"), Some("new_file"));
    assert_eq!(Config::default().action("ctrl+shift+n"), Some("new_folder"));
    let f = Fixture::new();
    let path = f.0.join("keys.conf");
    fs::write(&path, "[keys]\nnew_file='Ctrl+n'\nnew_folder='Ctrl+Shift+n'\n[links]\nscan_limit=42\nmax_results=3").unwrap();
    let conf = Config::load(&path).unwrap();
    assert_eq!(conf.link_scan_limit, 42);
    assert_eq!(conf.link_results, 3);
    fs::write(&path, "[keys]\ncopy='Ctrl+Shift+n'").unwrap();
    assert!(Config::load(&path).is_err());
    fs::write(&path, "[keys]\nquit='x'").unwrap();
    let migrated = Config::load(&path).unwrap();
    assert_eq!(migrated.action("x"), Some("quit"));
    assert_ne!(migrated.keys["exit_here"], "x");
}

#[test]
fn dashboard_head_is_bounded_configurable_and_skips_links() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let path = f.0.join("head.txt");
    fs::write(&path, "HEAD_ONE\nHEAD_TWO\nOMITTED_THREE\n").unwrap();
    let mut conf = Config {
        preview_lines: 2,
        ..Config::default()
    };
    let text = files::dashboard(&path, &f.0, &conf);
    assert!(text.contains("HEAD_ONE\nHEAD_TWO"));
    assert!(!text.contains("OMITTED_THREE"));
    assert!(text.contains("Modified") && text.contains("Size"));
    conf.preview_lines = 0;
    assert!(!files::dashboard(&path, &f.0, &conf).contains("HEAD_ONE"));
    conf.preview_lines = 2;
    symlink(&path, f.0.join("soft-head")).unwrap();
    assert!(!files::dashboard(&f.0.join("soft-head"), &f.0, &conf).contains("HEAD_ONE"));
    fs::write(&path, b"\0BINARY_CONTENT").unwrap();
    assert!(!files::dashboard(&path, &f.0, &conf).contains("BINARY_CONTENT"));
}

#[test]
fn selected_route_highlights_ancestor_edges_without_sibling_branches() {
    use unicode_width::UnicodeWidthStr;
    let f = Fixture::new();
    let mut b = Browser::new(&f.0, Config::default()).unwrap();
    b.expand_all().unwrap();
    b.find_next("needle", 1).unwrap();
    let layout = Layout::new(&b);
    let route = layout.path_cells(&b.selected, b.conf.gap, 0, 0, layout.width, layout.height);
    let selected = layout
        .items
        .iter()
        .find(|p| Rc::ptr_eq(&p.node, &b.selected))
        .unwrap();
    assert!(route.contains(&(selected.x - 1, selected.y)));
    let right = layout
        .items
        .iter()
        .find(|p| p.node.borrow().path == f.0.join("right"))
        .unwrap();
    assert!(!route.contains(&(right.x - 1, right.y)));
    let root = &layout.items[0];
    assert!(route.contains(&(root.x + root.label.width(), root.y)));
    assert!(route.iter().all(|point| {
        layout
            .cells(0, 0, layout.width, layout.height)
            .contains_key(point)
    }));
    assert!(
        layout
            .path_cells(&b.root, b.conf.gap, 0, 0, layout.width, layout.height)
            .is_empty()
    );
}

#[test]
fn explicit_link_target_resolves_relative_absolute_chained_and_broken_paths() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let target = f.0.join("right/deep/needle 中文.PY");
    let link = f.0.join("left/a");
    symlink("../right/deep/needle 中文.PY", &link).unwrap();
    assert_eq!(files::link_target(&link).unwrap(), target);
    assert_eq!(
        files::exit_directory_path(&target).unwrap(),
        target.parent().unwrap()
    );
    let chain = f.0.join("left/chain");
    symlink("a", &chain).unwrap();
    assert_eq!(files::link_target(&chain).unwrap(), target);
    let absolute = f.0.join("absolute");
    symlink(&target, &absolute).unwrap();
    assert_eq!(files::link_target(&absolute).unwrap(), target);
    let folder = f.0.join("folder");
    symlink(f.0.join("right"), &folder).unwrap();
    let resolved = files::link_target(&folder).unwrap();
    assert_eq!(
        files::exit_directory_path(&resolved).unwrap(),
        f.0.join("right")
    );
    let broken = f.0.join("broken");
    symlink("missing", &broken).unwrap();
    assert!(files::link_target(&broken).is_err());
    let cycle = f.0.join("cycle");
    symlink("cycle", &cycle).unwrap();
    assert!(files::link_target(&cycle).is_err());
    assert!(files::link_target(&target).is_err());
}
