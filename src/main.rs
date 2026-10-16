macro_rules! ui_format {
    ($zh:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        if crate::terminal_profile::get().ascii { format!($en $(, $arg)*) }
        else { format!($zh $(, $arg)*) }
    };
}
mod config;
mod edit;
mod files;
mod git_status;
mod input;
mod jobs;
mod links;
mod model;
mod terminal_host;
mod terminal_profile;
mod tmux;
mod transfer;
mod tui;
use std::{
    env,
    fs::OpenOptions,
    io::{self, IsTerminal, Write},
    os::unix::process::CommandExt,
    path::PathBuf,
    process::Command,
};
fn main() {
    if let Err(error) = run() {
        eprintln!("fileX: {error}");
        std::process::exit(1)
    }
}
fn run() -> Result<(), String> {
    let mut path = PathBuf::from(".");
    let mut conf_path = config::config_path();
    let (mut explicit, mut init, mut print) = (false, false, false);
    let (mut tty, mut ascii) = (false, false);
    let mut args = env::args().skip(1);
    let mut positional = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => {
                println!("filex {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "filex\nfilex [DIRECTORY] [--config PATH] [--tty] [--ascii] [--print-tree] [--init-config]\nCurrent directory by default. Linux TTY / SSH: terminal openers, a/A create, y copy path, Ctrl+V paste path.\nq dashboard / Q quit / Ctrl+F search / Enter folder expand / file open with / w menu / r rename / v dual panes.\nPipes and TERM=dumb automatically print a tree. --tty forces console compatibility."
                );
                return Ok(());
            }
            "--config" => {
                conf_path = config::expand_home(&args.next().ok_or("--config 缺少路径")?);
                explicit = true
            }
            "--init-config" => init = true,
            "--print-tree" => print = true,
            "--tty" => tty = true,
            "--ascii" => ascii = true,
            "--" => {
                path = PathBuf::from(args.next().ok_or("缺少目录")?);
                if args.next().is_some() {
                    return Err("只接受一个根目录".into());
                }
                break;
            }
            _ => {
                if arg.starts_with('-') {
                    return Err(format!("未知选项: {arg}"));
                }
                if positional {
                    return Err("只接受一个根目录".into());
                }
                path = config::expand_home(&arg);
                positional = true
            }
        }
    }
    if init {
        if let Some(parent) = conf_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&conf_path)
            .map_err(|e| e.to_string())?;
        file.write_all(include_bytes!("../fileX.example.conf"))
            .map_err(|e| e.to_string())?;
        println!("已创建配置: {}", conf_path.display());
        return Ok(());
    }
    if explicit && !conf_path.exists() {
        return Err(format!("配置不存在: {}", conf_path.display()));
    }
    let conf = config::Config::load(&conf_path)?;
    terminal_profile::init(&conf, tty, ascii);
    let browser = model::Browser::new(&path, conf)?;
    if print
        || !io::stdin().is_terminal()
        || !io::stdout().is_terminal()
        || env::var("TERM").is_ok_and(|t| t == "dumb")
    {
        let result = io::stdout()
            .lock()
            .write_all(model::Layout::new(&browser).plain().as_bytes());
        if let Err(error) = result
            && error.kind() != io::ErrorKind::BrokenPipe
        {
            return Err(error.to_string());
        }
        return Ok(());
    }
    if let Some(directory) = tui::run(browser, conf_path)?
        && env::var_os("FILEX_CWD_FILE").is_none()
    {
        // An executable cannot cd its caller. Replace this already-restored
        // process with an interactive shell in the chosen directory instead.
        let shell = env::var_os("SHELL").unwrap_or_else(|| "/bin/bash".into());
        let error = Command::new(shell)
            .arg("-i")
            .current_dir(directory)
            .env_remove("FILEX_CWD_FILE")
            .exec();
        return Err(format!("启动所选目录的 shell 失败: {error}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
