mod config;
mod files;
mod model;
mod tmux;
mod tui;
use std::{
    env,
    fs::OpenOptions,
    io::{self, IsTerminal, Write},
    path::PathBuf,
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
                    "filex\nfilex [目录] [--config PATH] [--print-tree] [--init-config]\nfilex --gui [目录]\n默认当前工作目录；q 信息 / Q 退出 / Ctrl+F 搜索 / Enter 打开方式 / v 切换树方向"
                );
                return Ok(());
            }
            "--config" => {
                conf_path = config::expand_home(&args.next().ok_or("--config 缺少路径")?);
                explicit = true
            }
            "--init-config" => init = true,
            "--print-tree" => print = true,
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
        file.write_all(include_bytes!("../fileX.conf"))
            .map_err(|e| e.to_string())?;
        println!("已创建配置: {}", conf_path.display());
        return Ok(());
    }
    if explicit && !conf_path.exists() {
        return Err(format!("配置不存在: {}", conf_path.display()));
    }
    let conf = config::Config::load(&conf_path)?;
    let browser = model::Browser::new(&path, conf)?;
    if print {
        print!("{}", model::Layout::new(&browser).plain());
        return Ok(());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("TUI 需要交互终端；可用 --print-tree 或 --gui".into());
    }
    tui::run(browser, conf_path)
}

#[cfg(test)]
mod tests;
