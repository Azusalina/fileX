## What is it?

filex 1.4 是面向 Arch Linux / KDE Wayland 的横向树文件管理器。默认 TUI 已用 Rust 重构，母子目录和文件由线连接，支持多分支展开、搜索定位、文件信息和常用文件操作。现有 Qt GUI 保留，通过 `--gui` 启动。

配置、架构、内存测量、限制和历史记录见 [description.md](description.md)。[GUI 预览](preview.png)。

## How to use

本机已经编译好 TUI，并安装 `filex` Bash alias 和 `~/.local/bin/filex` 命令，可在 tmux 内使用。已有终端先执行一次：

```bash
source ~/.bashrc
filex               # 以当前工作目录为根，启动 Rust TUI
filex ~/Documents   # 指定根目录
filex --gui         # 以当前工作目录为根，启动现有 Qt GUI
```

也可直接运行 `./filex.sh`，无须 alias。TUI 首次编译需要 Rust / Cargo；编译后运行不需要 Python。GUI 需要 Python 3.11+ / PySide6，文件打开需要 xdg-utils，回收站需要 glib2 的 gio，TUI 复制路径需要 wl-clipboard；GUI 打开终端编辑器需要 Konsole。Arch 缺少依赖时安装：

```bash
sudo pacman -S rust python pyside6 xdg-utils glib2 konsole wl-clipboard fcitx5
./scripts/build.sh          # 构建 / 更新 release TUI
```

启动脚本在缺少 release 程序时自动编译。VSCodium、Vim、Neovim、Nano、Firefox 是可选打开程序，使用前需已安装。

| 默认键 | 操作 |
|---|---|
| `↑/↓`；或 `k/j` | 只选择同级兄弟节点，跳过展开目录的后代 |
| `→`；或 `l` | 展开选中目录；已展开时进入其第一个子节点 |
| `←` | 返回父节点；在根节点时引入上级根目录并选中它，保留原树展开状态 |
| `h` | 只收起选中目录，不移动到父节点 |
| `f` | 当前 folder 设为根并刷新；光标在文件上时使用其所在 folder，保留新根内的展开状态 |
| `q` | 切换右侧 Dashboard：创建时间、修改时间、大小；随选中节点更新 |
| `Ctrl+F` 或 `/` | 递归搜寻根目录下文件 / 目录名，含未展开目录；Enter 定位 |
| `n`，`N` | 下一个 / 上一个搜索结果，保留其他展开分支 |
| `.` | 显示 / 隐藏隐藏文件和目录 |
| `E`，`C` | 展开 / 收起选中目录下的全部分支（受配置上限约束） |
| `Enter` 或 `w` | 选择打开方式：系统默认、VSCodium、Vim、Neovim、Nano、Firefox、Python 执行、Toolkit Markdown 或配置中的程序 |
| `o` | 直接使用该扩展名的默认打开方式 |
| `g` / `Ctrl+L` | 输入其他根目录；显式更换根目录时 Rust TUI 重建树缓存 |
| `p` | 文本预览，Esc 返回 |
| `a`，`A` | 新建空文件 / 目录，位置为选中目录或文件的父目录 |
| `r` / `Shift+R`，`c`，`m` | 重命名、复制、移动 |
| `Ctrl+Alt+C` | 复制选中 file/folder 的绝对路径到系统剪贴板 |
| `Delete` 或 `D` | 移到回收站；输入 `YES` 确认，可在 Dolphin 回收站恢复 |
| `Ctrl+R` / `F5`，`,` | 刷新目录、重载配置 |
| `?`，`Q` | 帮助、退出（小写 q 用于 Dashboard） |

`H/L` 横移，PgUp/PgDn 同级翻页，Home/End 选择首末节点；输入时 Enter 确认、Esc 取消、Ctrl+U 清空。复制 / 移动填写**完整目标路径，包含目标名称**；相对路径以源项目的父目录为基准，不覆盖已有目标，目标父目录必须存在。文件树不自动遍历符号链接。

Enter 选中目录时还可选择在 fileX 展开 / 收起。TUI 的终端编辑器（如 Vim/Neovim/Nano）优先复用同一 tmux window 内右侧空闲 pane，并切换焦点过去；没有合适 pane、pane 繁忙或已有未提交命令时，在当前终端运行，退出后返回原树。GUI 在 Konsole 打开终端编辑器。GUI 单击选择，双击目录展开 / 折叠，双击文件选择打开方式；拖动空白处平移，Ctrl+滚轮缩放，工具栏和右键菜单提供操作。GUI 主程序位于 [filex/gui.py](filex/gui.py)，通过 `filex --gui` 启动。画布焦点下快捷键与 TUI 相同，Ctrl+R / F5 刷新、r / Shift+R / F2 重命名、Delete 回收站。

TUI 启动时自动使用 Fcitx5 英文键盘，退出后恢复原输入法；可在 `[input_method]` 设置 `auto_english=false`。`Enter` 选择「Python 执行」会运行 `python 文件绝对路径`，工作目录为脚本所在目录；当前终端执行结束后按 Enter 返回，GUI Konsole 保留输出；也可把 `.py` 默认方式改为 `python`。Python、编辑器、Node 和浏览器是外部程序。

`.md` / `.markdown` 默认先复制绝对路径，再用相邻 [toolkit](../toolkit/README.md) 渲染并在 Firefox 新标签页打开，保存后自动更新。需要 Node.js 22+、Firefox，以及在 toolkit 目录完成 `npm install`；本机已具备。预览使用随机本地端口，每个文件独立，不占用 toolkit 原有的 4177 端口。没有预览连接且无请求 30 分钟后自动关闭对应渲染进程。Markdown 默认打开项可在配置中覆盖，`command` 内的 `{toolkit}` 可改为其他 toolkit 的绝对目录。`.r`、`.cpp`、`.sh`、`.mp3`、`.mp4` 已有默认颜色，大小写不敏感。

个人配置位于 `~/.config/fileX/fileX.conf`（遵循 XDG_CONFIG_HOME），本机已创建。源码 [fileX.conf](fileX.conf) 是完整样本；用 `--config PATH` 可选择其他配置，缺少个人配置时 `filex --init-config` 创建且不覆盖已有文件。修改后按 `,` 或 GUI「读取配置」。

配置可以定制：隐藏文件、排序、目录显示 / 缓存 / 展开上限、搜索上限、节点间距、标签宽度、行距、Dashboard 宽度、预览大小、GUI 字号 / 主题、单字符、Ctrl+字母或 Ctrl+Alt+字母快捷键、打开程序和参数、扩展名默认打开方式、扩展名颜色，以及 tmux 右侧打开开关、TUI 自动英文输入法。例如：

```toml
[tmux]
open_in_right_pane = true

[open_defaults]
".py" = "vscodium"
".html" = "firefox"
".md" = "markdown"
# ".py" = "python" # 改为运行脚本时替换已有 .py 行

[input_method]
auto_english = true

[extension_colors]
".py" = "#e5c07b"
".html" = "#e06c75"
```

上述配置与已有区段合并编辑，不重复添加同名 TOML 区段。扩展名大小写不敏感，最长后缀优先；`"*"` 为其他文件、`""` 为无扩展名、`"<directory>"` 用于目录打开方式。颜色同时应用于 TUI 和 GUI。创建时间缺失时显示不可用；目录大小为目录本身的元数据大小，不含子文件总量。

安装 / 移除 alias 或可选 KDE 菜单入口：

```bash
python scripts/install_alias.py          # Bash alias，已在本机完成
./scripts/install.sh --command-only      # 只安装命令，已在本机完成
./scripts/install.sh                     # 可选命令与 KDE 菜单入口
./scripts/uninstall.sh                   # 移除命令与菜单入口
python scripts/install_alias.py --remove # 移除本项目的 alias
```

tmux 当前 pane 中直接运行 `filex`，根目录为该 pane 的当前目录；也可运行 `tmux split-window -h -c "#{pane_current_path}" "$HOME/.local/bin/filex"` 在新右侧 pane 启动。现有 tmux 方向键配置无需更改。

保留源码目录，移动后重新安装入口。卸载保留配置和用户文件。默认文件管理器保持原设置。
