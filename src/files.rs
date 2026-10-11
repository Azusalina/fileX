use crate::{
    config::{Config, expand_home},
    model::clean,
};
use std::{
    ffi::CString,
    fs::{self, File, FileTimes, OpenOptions},
    io::{self, Read},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, symlink},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn cname(path: &Path) -> Result<CString, String> {
    CString::new(path.as_os_str().as_bytes()).map_err(err)
}
pub fn rename(source: &Path, dest: &Path) -> Result<(), String> {
    let s = cname(source)?;
    let d = cname(dest)?;
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            s.as_ptr(),
            libc::AT_FDCWD,
            d.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result != 0 {
        Err(io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}
fn rename_io(source: &Path, dest: &Path) -> io::Result<()> {
    let s = cname(source).map_err(io::Error::other)?;
    let d = cname(dest).map_err(io::Error::other)?;
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            s.as_ptr(),
            libc::AT_FDCWD,
            d.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
pub fn name_valid(name: &str) -> Result<(), String> {
    if name.is_empty() || [".", ".."].contains(&name) || name.contains('/') || name.contains('\0') {
        Err("请输入单个有效名称".into())
    } else {
        Ok(())
    }
}
pub fn create(directory: &Path, name: &str, folder: bool) -> Result<PathBuf, String> {
    name_valid(name)?;
    let path = directory.join(name);
    if folder {
        fs::create_dir(&path).map_err(err)?
    } else {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(err)?;
    }
    Ok(path)
}
pub fn destination(source: &Path, text: &str) -> PathBuf {
    let path = expand_home(text);
    if path.is_absolute() {
        path
    } else {
        source.parent().unwrap_or(Path::new("/")).join(path)
    }
}
fn validate(source: &Path, dest: &Path) -> Result<(), String> {
    if fs::symlink_metadata(dest).is_ok() {
        return Err(format!("目标已存在: {}", dest.display()));
    }
    let parent = dest.parent().ok_or("目标父目录无效")?;
    let parent = fs::canonicalize(parent).map_err(err)?;
    if !parent.is_dir() {
        return Err("目标父目录不是目录".into());
    }
    let info = fs::symlink_metadata(source).map_err(err)?;
    if info.is_dir()
        && parent
            .join(dest.file_name().ok_or("无效目标名称")?)
            .starts_with(fs::canonicalize(source).map_err(err)?)
    {
        return Err("不能将目录复制 / 移动到自身内部".into());
    }
    Ok(())
}
fn copy_one(source: &Path, dest: &Path, depth: usize) -> Result<(), String> {
    if depth > 128 {
        return Err("复制目录超过 128 层；源保留，目标可能包含部分副本".into());
    }
    let info = fs::symlink_metadata(source).map_err(err)?;
    if info.file_type().is_symlink() {
        symlink(fs::read_link(source).map_err(err)?, dest).map_err(err)?
    } else if info.is_dir() {
        fs::create_dir(dest).map_err(err)?;
        for item in fs::read_dir(source).map_err(err)? {
            let item = item.map_err(err)?;
            copy_one(&item.path(), &dest.join(item.file_name()), depth + 1)?
        }
        fs::set_permissions(dest, info.permissions()).map_err(err)?;
    } else if info.is_file() {
        let mut input = File::open(source).map_err(err)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dest)
            .map_err(err)?;
        io::copy(&mut input, &mut output).map_err(err)?;
        output.set_permissions(info.permissions()).map_err(err)?;
        let mut times = FileTimes::new();
        if let Ok(time) = info.modified() {
            times = times.set_modified(time)
        }
        if let Ok(time) = info.accessed() {
            times = times.set_accessed(time)
        }
        output.set_times(times).map_err(err)?;
    } else {
        return Err("仅支持普通文件、目录和符号链接".into());
    }
    Ok(())
}
pub fn transfer(source: &Path, dest: &Path, moving: bool) -> Result<(), String> {
    validate(source, dest)?;
    if moving {
        match rename_io(source, dest) {
            Ok(()) => return Ok(()),
            Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {}
            Err(e) => return Err(err(e)),
        }
    }
    copy_one(source, dest, 0)
        .map_err(|e| format!("{e}；源保留，请检查可能存在的部分目标: {}", dest.display()))?;
    if moving {
        let info = fs::symlink_metadata(source).map_err(err)?;
        if info.is_dir() {
            fs::remove_dir_all(source).map_err(err)?
        } else {
            fs::remove_file(source).map_err(err)?
        }
    }
    Ok(())
}
pub fn trash(path: &Path) -> Result<(), String> {
    let output = Command::new("gio")
        .args(["trash", "--"])
        .arg(path)
        .output()
        .map_err(err)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "回收站操作失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
pub fn preview(path: &Path, limit: usize) -> String {
    let read = || -> Result<String, String> {
        let info = fs::symlink_metadata(path).map_err(err)?;
        if info.file_type().is_symlink() {
            return Ok(format!(
                "符号链接 → {}",
                clean(&fs::read_link(path).map_err(err)?.to_string_lossy())
            ));
        }
        if info.is_dir() {
            return Ok(format!(
                "目录\n{}\n展开以查看内容",
                clean(&path.to_string_lossy())
            ));
        }
        if !info.is_file() {
            return Ok("特殊文件（设备、管道或套接字），不读取内容".into());
        }
        let mut data = Vec::new();
        File::open(path)
            .map_err(err)?
            .take((limit + 1) as u64)
            .read_to_end(&mut data)
            .map_err(err)?;
        let truncated = data.len() > limit;
        data.truncate(limit);
        let text = match std::str::from_utf8(&data) {
            Ok(v) => v,
            Err(e) if truncated && e.error_len().is_none() => {
                std::str::from_utf8(&data[..e.valid_up_to()]).map_err(err)?
            }
            _ => return Ok(format!("{} bytes\n非 UTF-8 文本 / 二进制文件", info.len())),
        };
        if data.contains(&0) {
            return Ok(format!("{} bytes\n二进制文件，用默认程序打开", info.len()));
        }
        Ok(format!(
            "{} bytes\n\n{}{}",
            info.len(),
            text.lines()
                .map(|line| clean(&line.replace('\t', "    ")))
                .collect::<Vec<_>>()
                .join("\n"),
            if truncated {
                "\n…预览已截断"
            } else {
                ""
            }
        ))
    };
    read().unwrap_or_else(|e| format!("无法预览: {e}"))
}
fn date(seconds: i64) -> String {
    let time = seconds as libc::time_t;
    let mut tm = unsafe { std::mem::zeroed::<libc::tm>() };
    if unsafe { libc::localtime_r(&time, &mut tm) }.is_null() {
        return "不可用".into();
    }
    let mut buffer = [0u8; 64];
    let format = c"%Y-%m-%d %H:%M:%S %Z";
    let length = unsafe {
        libc::strftime(
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            format.as_ptr(),
            &tm,
        )
    };
    String::from_utf8_lossy(&buffer[..length]).into_owned()
}
pub fn metadata(path: &Path) -> String {
    let read = || -> Result<String, String> {
        let info = fs::symlink_metadata(path).map_err(err)?;
        let name = cname(path)?;
        let mut extended = unsafe { std::mem::zeroed::<libc::statx>() };
        let born = unsafe {
            libc::statx(
                libc::AT_FDCWD,
                name.as_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
                libc::STATX_BTIME,
                &mut extended,
            )
        };
        let created = if born == 0 && extended.stx_mask & libc::STATX_BTIME != 0 {
            date(extended.stx_btime.tv_sec)
        } else {
            "不可用（文件系统未提供）".into()
        };
        let kind = if info.is_dir() {
            "目录"
        } else if info.file_type().is_symlink() {
            "符号链接"
        } else {
            "文件"
        };
        let note = if info.is_dir() {
            "目录自身大小，不含子文件总量"
        } else if info.file_type().is_symlink() {
            "符号链接自身大小"
        } else {
            ""
        };
        Ok(format!(
            "{}\n{kind}\n\nCreated / 创建时间\n{created}\n\nModified / 修改时间\n{}\n\nSize / 大小\n{} bytes\n{note}\n\nPath / 路径\n{}",
            clean(
                &path
                    .file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy()
            ),
            date(info.mtime()),
            info.len(),
            clean(&path.to_string_lossy())
        ))
    };
    read().unwrap_or_else(|e| format!("无法读取信息: {e}"))
}
pub fn available(command: &str) -> bool {
    if command.contains('/') {
        return Path::new(command).is_file();
    }
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|p| p.join(command).is_file()))
}
pub fn open_command(path: &Path, conf: &Config, method: &str) -> Result<(Command, bool), String> {
    let opener = conf.openers.get(method).ok_or("打开程序未定义")?;
    let args = opener
        .command
        .iter()
        .map(|s| s.replace("{path}", &path.to_string_lossy()))
        .collect::<Vec<_>>();
    if !available(&args[0]) {
        return Err(format!("程序未安装: {}", args[0]));
    }
    let mut command = Command::new(&args[0]);
    command.args(&args[1..]);
    if !opener.terminal {
        command.stdout(Stdio::null()).stderr(Stdio::null());
    }
    Ok((command, opener.terminal))
}
