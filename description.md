## What is it?

filex 1.5 是面向 Arch、Fedora、Ubuntu、Kali 的 Linux TTY、SSH 和 Konsole 终端的横向树文件管理器。默认 TUI 已用 Rust 重构，母子目录和文件由线连接，支持多分支展开、搜索定位、文件信息和常用文件操作，可切换同一界面的中轴镜像双目录树。当前版本仅提供 Rust TUI。

配置、架构、内存测量、限制和历史记录见 [description.md](description.md)。

## How to use

本机已经编译好 TUI，并安装 `filex` Bash / Zsh 函数（替代原 alias）和 `~/.local/bin/filex` 命令，可在 tmux 内使用，无须每次手动加载 shell 配置：

```bash
filex               # 以当前工作目录为根，启动 Rust TUI
filex ~/Documents   # 指定根目录
filex --tty         # 强制 Linux 控制台兼容模式（通常自动识别）
filex --ascii       # ASCII 线条与英文控制提示，文件名保持原样
filex | less        # 非交互输出自动打印当前目录的一层树
filex --print-tree --ascii > tree.txt
```

也可直接运行 `./filex.sh`，无须 shell 函数。首次编译需要 Rust / Cargo 1.88+ 和 C 链接器；已有 release 二进制可直接运行，浏览、搜索、新建、重命名、复制、移动和预览不需要 Python、GUI、Wayland 或桌面服务。回收站需要 glib2 的 `gio`；终端编辑器可选 Nano / Neovim / Vim：

```bash
sudo pacman -S --needed base-devel rust python glib2 nano
./scripts/build.sh       
```

Fedora：
```bash 
sudo dnf install gcc glibc-devel rust cargo python3 glib2 nano
```

Ubuntu / Kali：
```bash
sudo apt update && sudo apt install build-essential python3 libglib2.0-bin nano curl
```


再准备 Rust / Cargo 1.88+。发行版 Rust 太旧时使用 [官方 rustup](https://rustup.rs/) 安装新版；`python3 scripts/check-system.py` 会显示本机版本、依赖和对应安装命令，不自动改系统。Kali 属于 Debian 系，使用 apt，与 Fedora 的 dnf 分开处理。[Kali 官方说明](https://www.kali.org/docs/policy/kali-linux-relationship-with-debian/)。

在目标发行版执行 `./scripts/build.sh`，再 `./scripts/install.sh`。不要把较新 glibc 的 Arch 二进制直接复制给旧发行版；当前本机构建需要 glibc 2.39+，旧系统应源码重建。已有兼容二进制运行时不需要 Rust。

仅在桌面会话中按需安装 `xdg-utils`、`wl-clipboard`、`fcitx5`、`qt6-tools` 及打开程序；这些不是纯 CLI / TTY 浏览的前提。
启动脚本在缺少 release 程序时自动编译。VSCodium、Vim、Neovim、Nano、Firefox、Rscript（Arch 的 r 包）、Bash 是可选打开程序，使用前需已安装。

| 默认键 | 操作 |
|---|---|
| `↑/↓`；或 `k/j` | 在所有已展开分支中选择同层级节点，可跨父目录；首尾循环，跳过更深层内容 |
| `→`；或 `l` | 展开选中目录；已展开时进入其第一个子节点；软链接上选中右侧目标 |
| `←` | 返回父节点；链接目标上先返回链接本身；根节点上引入上级并保留原树 |
| `v` | 默认同级垂直罗列；按 v 将直属文件分组，再按恢复，保留根目录、光标和展开状态 |
| `O` | 调整当前同级顺序：↑↓ 选择、K/J 上移／下移、Enter 保存到该目录配置、Esc 取消 |
| `z` / `Z` | 隐藏当前项／管理恢复 filex 隐藏项，重启后保留，独立于 `.` |
| `h` | 只收起选中目录，不移动到父节点 |
| `f` | 当前 folder 设为根并刷新；光标在文件上时使用其所在 folder，保留新根内的展开状态 |
| `q` | 切换右侧 Dashboard：创建时间、修改时间、大小、链接信息和文件开头；随选中节点更新 |
| `[` / `]` | 缩窄 / 加宽 Dashboard，每次 4 列；运行时调整，持久默认宽度写入配置 |
| `Ctrl+F` 或 `/` | 递归搜寻根目录下文件 / 目录名，含未展开目录；Enter 定位，↑↓ 切换结果，Esc 退出搜索 |
| `n`，`N` | 下一个 / 上一个搜索结果，保留其他展开分支 |
| `.` | 显示 / 隐藏隐藏文件和目录 |
| `E`，`C` | 展开 / 收起选中目录下的全部分支（受配置上限约束） |
| `Enter` | folder：展开子目录／文件树，光标留在该 folder，重复按不收起、不进入子项；普通文件：选择打开方式 |
| `w` | 对文件或 folder 选择打开方式：系统默认、VSCodium、Vim、Neovim、Nano、Firefox、Python / Rscript / Bash 执行、Toolkit Markdown 或配置中的程序 |
| `o` | 直接使用该扩展名的默认打开方式 |
| `g` / `Ctrl+L` | 输入其他根目录；显式更换根目录时 Rust TUI 重建树缓存 |
| `p` | 文本预览，Esc 返回 |
| `Ctrl+N` / `a`，`Ctrl+Shift+N` / `A` | 新建空文件 / 目录，位置为选中目录或文件的父目录 |
| `I` | 可滚动的完整文件信息、链接详情和文件开头 |
| `x` | 退出并在选中目录继续使用 shell；选中文件时进入其父目录 |
| `r` / `Shift+R`，`c`，`m` | 重命名、复制、移动 |
| `Ctrl+Alt+C` 或 `y` | 保存绝对路径到 filex 内部缓冲；桌面会话同时复制到 Wayland 剪贴板，输入框用 `Ctrl+V` 粘贴 |
| `Delete` 或 `D` | 移到回收站；再按 `Enter` 确认、`Esc` 取消，可在 Dolphin 回收站恢复 |
| `Ctrl+R` / `F5` / `Ctrl+F5`，`,` | 刷新目录、重载配置 |
| `?`，`Q` | 帮助、退出（小写 q 用于 Dashboard） |

`Space` 标记／取消标记文件或目录，显示亮洋红方框，可跨目录选择；`Esc` 清空多选。`c/m` 后用方向键选择目标 **folder**，按 `Enter` 复制／移动整批项目。此时展开目录的末尾有可选 `+`，选中后 Enter 直接将待复制／移动项目放入该目录，直接按 `+` 也可放入当前目标目录。按 `A`（或配置的新建文件夹快捷键）输入新目录名，创建后再 Enter 执行。`g` 可输入目标根目录，`Esc` 取消待执行操作并保留标记，再按一次清空标记。同名普通文件需再次 Enter 确认替换；目录／链接冲突、批内重名或目录复制到自身内部会拒绝执行；父目录与其子项同时选中时只处理父目录一次。`Delete/D` 同样支持批量回收站删除，再按 Enter 确认。重命名仅作用于当前光标项目。

`H/L` 横移，PgUp/PgDn 同层级循环翻页，Home/End 选择首末节点；重命名、创建及路径输入框支持 `←/→` 移动光标、Home/End、光标处插入、Backspace/Delete、Ctrl+U 清空和 Ctrl+V 插入内部路径缓冲。文件树不自动遍历符号链接。

Linux 原生控制台（`TERM=linux` 或识别到真实 VT）自动使用 16 色、ASCII 连线及英文控制提示；SSH / 无桌面会话也默认 ASCII 提示。默认颜色模式 `consistent` 把两边配置颜色映射到同一套 Linux 16 色；Konsole 直接显示这些颜色的 RGB，不使用主题中的 ANSI 色表。桌面保留中文提示与现代终端协议。TTY 中推荐 `a` / `A` 新建文件 / 文件夹、`y` 保存路径、`Ctrl+R` 刷新：传统终端通常无法区分 `Ctrl+Shift+N` 与 `Ctrl+N`。方向键、`v` 分组切换、`c` / `m`、搜索、回收站及 `x` 均可使用；复制路径缓冲仅在当前 filex 进程内，退出即释放。

无桌面时 `o` 自动将桌面默认打开方式替换为 Nano → Neovim → Vim（按安装情况），都没有则用内置预览；`w`（普通文件上也可 Enter）的打开菜单只列终端打开程序。可用 `terminal.cli_opener` 指定其他 `terminal=true` 程序。Python / Rscript / Bash 仍需明确选择或配置为默认，不会因后缀自动执行。Markdown 在 CLI 中交给编辑器 / 内置文本预览；在桌面仍使用 Toolkit / Firefox。`TERM=dumb`、输入或输出重定向时直接打印一层树；无需伪造桌面环境变量。

每个展开目录都有两组连接：左边是子目录及其展开树，右边是该目录的直属文件。两组连接同一个父目录；v 可回到普通横向树，方向键始终为 ← 父级、→ 子级，↑↓ 同层选择。较宽的树可用 H/L 横移，选中文件时视图自动跟随。

复制／移动始终在同一棵树中选择目标，或按 t 输入路径，目录内 + 直接放入；无需切换操作侧。

Enter 在 folder 上直接展开子树，保留其他已展开分支；子级仍通过左右方向键进入。右侧镜像栏按现有方向向左展开。`h` 收起当前 folder；`w` 才显示目录打开方式菜单。TUI 的终端编辑器（如 Vim/Neovim/Nano）优先复用同一 tmux window 内右侧空闲 pane，并切换焦点过去；没有合适 pane、pane 繁忙或已有未提交命令时，在当前终端运行，退出后返回原树。

软链接显示为 `a------b`，连接线和目标文件名 `b` 默认靛蓝色 `#4B0082`、加粗；长名称会截断，`q` / `I` 可查看完整目标路径。`[colors] link_color` 可改颜色。在 `a` 上按展开箭头（左侧 `→`、右侧 `←`）或 `l` 选中目标 `b`，目标名反色并保持链接颜色加粗；按返回父级箭头回到 `a`（左侧 `←`、右侧 `→`）。选中 `b` 后按 `x` 退出到实际目标所在目录（目标是目录时进入该目录），`q` / `I`、预览、打开方式及复制路径也对应目标；`f` 将目标目录设为根。相对路径及连续软链接会解析到实际目标，断开或循环链接显示错误。上下移动回到链接所在层的正常选择。目标名模式中的文件操作先按 `f` 进入实际目录，或 `←` 选回原链接。多个硬链接的文件标为 `H×数量`；`q` / `I` 显示链接目标、状态及当前根目录内找到的其他硬链接路径。扫描受 `[links] scan_limit` / `max_results` 限制，包含隐藏项，不跟随软链接；目录的链接计数不会误标为硬链接文件。

`x` 通过 Bash 函数启动时直接改变调用 shell 的目录。直接运行命令链接、二进制或 `./filex.sh` 时，退出 TUI 后在选中目录进入交互式 `$SHELL`；Bash 自动读取 `.bashrc`，之后的 `filex` 调用使用函数。直接启动的这一层 shell 用 `exit` 返回原 shell。普通 `Q` 退出保留原目录。

桌面会话的 TUI 启动及重新获得焦点时自动使用 Fcitx5 英文键盘，退出后恢复启动前的输入法；可在 `[input_method]` 设置 `auto_english=false`。`Enter` 选择 Python / Rscript / Bash 执行，分别运行 `python 文件绝对路径`、`Rscript 文件绝对路径`、`bash -- 文件绝对路径`，工作目录为脚本所在目录；当前终端执行结束后按 Enter 返回；也可把 `.py` / `.r` / `.sh` 默认方式改为 `python` / `rscript` / `bash`。Python、编辑器、Node 和浏览器是外部程序。没有 `python` 命令但存在 `python3` 时，Python 执行项自动使用 `python3`。

`.md` / `.markdown` 默认先复制绝对路径，再用相邻 [toolkit](../toolkit/README.md) 渲染并在 Firefox 新标签页打开，保存后自动更新。状态栏显示预览就绪 URL，启动失败会显示原因。需要 Node.js 22+、Firefox，以及在 toolkit 目录完成 `npm install`；本机已具备。预览使用随机本地端口，每个文件独立，不占用 toolkit 原有的 4177 端口。没有预览连接且无请求 30 分钟后自动关闭对应渲染进程。Markdown 默认打开项可在配置中覆盖，`command` 内的 `{toolkit}` 可改为其他 toolkit 的绝对目录。`.js`、`.mjs` 默认金色 `#FFD700`；`.r`、`.cpp`、`.sh`、`.mp3`、`.mp4` 也有默认颜色，大小写不敏感。

个人配置位于 `~/.config/fileX/filex.conf`（遵循 XDG_CONFIG_HOME），本机已创建。项目只保留 [filex.conf](filex.conf) 作为主配置的软链接入口，重复的 `fileX.conf` 已移除；编辑并保存后，TUI 按 `,` 重载，下次启动也会读取更新。独立默认样本保留为 [fileX.example.conf](fileX.example.conf)，供 `--init-config` 使用；初始化不会覆盖已有配置。用 `--config PATH` 可选择其他配置。颜色必须写成 `"#RRGGBB"`，例如 `".js" = "#FFD700"`，并放在 `[extension_colors]` 下；`#ffd700` 同样有效。非空的 `NO_COLOR` 环境变量会关闭 TUI 颜色；需要彩色时先 `unset NO_COLOR`。

配置可以定制：隐藏文件、排序、目录显示 / 缓存 / 展开上限、搜索上限、节点间距、标签宽度、行距、Dashboard 宽度、预览大小 / 开头行数、链接扫描上限、单字符、Ctrl+字母、Ctrl+Shift+字母或 Ctrl+Alt+字母快捷键、打开程序和参数、扩展名默认打开方式、扩展名颜色、按目录名设置颜色（含隐藏目录）、祖先路径高亮、启动时的分组树设置，以及 tmux 右侧打开开关、TUI 自动英文输入法。例如：

```toml
[terminal]
mode = "auto"       # auto / tty；--tty 也可强制
color_mode = "consistent" # 两边统一 16 色；也可 auto / 16 / 256 / truecolor / none；NO_COLOR 优先
charset = "auto"    # auto / ascii / unicode
cli_opener = "auto" # 或 nano / vim / nvim / 自定义 terminal=true 的 opener

[tmux]
open_in_right_pane = true

[open_defaults]
".py" = "vscodium"
".html" = "firefox"
".md" = "markdown"
# ".py" = "python" # 改为运行脚本时替换已有 .py 行
# ".r" = "rscript"
# ".sh" = "bash"

[input_method]
auto_english = true

[dashboard]
dashboard_width = 36 # 20..240 列；实际宽度给树至少保留 36 列

[layout]
grouped = false # 默认同级垂直罗列；v 将直属文件分组
highlight_path = true # 祖先路线高亮，其余分支稍暗

[keys]
toggle_grouped = "v"
reorder = "O"
hide_item = "z"
hidden_items = "Z"

[colors]
path_color = "#9cc5d1"

[folder_colors]
".git" = "#c678dd"
".github" = "#c678dd"
node_modules = "#98c379"
__pycache__ = "#e5c07b"
pycache = "#e5c07b"

[preview]
preview_lines = 5 # q / I 的文件开头行数；0 关闭

[extension_colors]
".js" = "#FFD700"
".mjs" = "#FFD700"
".py" = "#e5c07b"
".html" = "#e06c75"
```

上述配置与已有区段合并编辑，不重复添加同名 TOML 区段。扩展名大小写不敏感，最长后缀优先；`"*"` 为其他文件、`""` 为无扩展名、`"<directory>"` 用于目录打开方式。颜色应用于 TUI。`folder_colors` 精确匹配目录名，区分大小写；没有匹配时使用 `colors.folder_color`；隐藏目录名要加引号，如 `".git"`，按 `.` 显示隐藏项。颜色字符串的首尾空格会自动忽略。创建时间缺失时显示不可用；目录大小为目录本身的元数据大小，不含子文件总量。

安装 / 移除 Bash 入口和终端命令：

```bash
python3 scripts/install_alias.py          # Bash 函数及 TTY 登录加载入口，已在本机完成
./scripts/install.sh --command-only      # 只安装命令，不修改 shell 配置
./scripts/install.sh                     # 安装命令、Bash/Zsh 函数及 TTY 登录入口
./scripts/uninstall.sh                   # 移除命令及本项目 Bash 启动区块
python3 scripts/install_alias.py --remove # 只移除 Bash 函数及登录配置区块
unset -f filex                          # 同时卸载当前 shell 已加载的函数
```

tmux 当前 pane 中直接运行 `filex`，根目录为该 pane 的当前目录；也可运行 `tmux split-window -h -c "#{pane_current_path}" "$HOME/.local/bin/filex"` 在新右侧 pane 启动。现有 tmux 方向键配置无需更改。桌面模式的 Rust TUI 临时启用 tmux CSI-u 扩展键和焦点事件，切换回来时恢复原始输入模式及键盘编码，并释放当前 Konsole 窗口占用的 Ctrl+Shift+N；正常退出恢复原状态。Konsole 适配需要 qt6-tools 的 qdbus6，可用 `[terminal] konsole_shortcuts=false` / `tmux_extended_keys=false` 关闭。

保留源码目录，移动后重新安装入口。卸载保留配置和用户文件。默认文件管理器保持原设置。

`[terminal]` 的模式、颜色和字符集在启动时决定，修改后重新启动；其他现有配置仍可用 `,` 重载。TTY 模式不调用 Fcitx / Konsole，也不改 tmux 扩展键和焦点设置。正常退出、Ctrl+C、SIGTERM / SIGHUP 恢复终端设置。原生控制台字体仍限制文件名字符显示，ASCII 模式不转写中文文件名。

TTY 登录的 Bash 自动读取已有的第一个 `.bash_profile` / `.bash_login` / `.profile`；安装器在其中追加本项目管理区块，交互登录自动加载 `.bashrc`（函数已加载则跳过），同时补上 `~/.local/bin`。没有登录配置时创建 `.bash_profile`。本机已安装；新登录直接运行 `filex`，已经打开的 TTY 可重新登录或运行一次 `exec bash -l`，之后不需要 `source ~/.bashrc`。自定义 `--prefix` 或 `--command-only` 不修改个人 shell；`install_alias.py --rc PATH --profile PATH` 可管理其他 Bash 配置。

颜色默认使用 `[terminal] color_mode="consistent"`：读取 Linux VT 的 16 色默认表，配置颜色取最近颜色；暗淡分支和加粗链接也映射到同一表。只控制 filex 自身输出，不改变 Konsole 主题或系统 / VT 调色板。自定义原始 `#RRGGBB` 配置保留，超过 16 种颜色会合并为近似色；若改回 `auto`，两类终端恢复各自能力的颜色模式。已单独用 OSC 自定义过某个 VT 的色表可能仍与系统默认表不同。修改颜色模式后重启 filex 生效。

新默认文件类型配色覆盖 100 多个后缀，并保持同色相的亮 / 暗两档，收起其他分支时不会丢掉类别区别：

| 类别 | 颜色 | 例子 |
|---|---|---|
| 代码 | 黄（JS / MJS 仍为 `#FFD700`） | py、r、rs、cpp、js、ts、go |
| Shell / 启动程序 | 绿 | sh、bash、zsh、fish、AppImage |
| 配置 / 数据 / 标记 | 青 | json、toml、conf、yaml、csv、html、css |
| 文本 / 文档 | 白 | txt、md、pdf、docx、epub |
| 图像 | 紫 | png、jpg、webp、svg |
| 音频 / 视频 | 蓝 | mp3、mp4、flac、mkv |
| 压缩包 / 镜像 / 安装包 | 红 | tar.gz、zip、7z、iso、deb、rpm |

其他文件灰色；分类按后缀，不自动执行程序。默认定义见 [themes/terminal-16.conf](themes/terminal-16.conf)，Rust 在构建时嵌入；修改自己的 `[extension_colors]` 后按 `,` 重载即可。此次已更新本机文件后缀配色，保留目录名颜色、打开规则及其他自定义设置。

安装器自动识别 Bash / Zsh；可 `python3 scripts/install_alias.py --shell zsh` 显式选择。Zsh 维护 `.zshrc` / `.zprofile` 并遵循 `ZDOTDIR`，交互登录只加载一次 `.zshrc`；Bash 的现有加载方式保留。其他 shell 可用 `--command-only`，`x` 使用已说明的独立 shell 退回方式。

跨发行版复测：有 bubblewrap 和 Python 3.12+ 时运行 `python3 scripts/distro-smoke.py --install-tools`。它下载 Ubuntu / Fedora / Kali 官方 OCI 镜像，在用户缓存中的隔离根目录安装测试依赖，不修改主机软件包或登录文件；核对安装、Python3、TTY 文件操作和 Zsh `x`。结果写入 [distro-validation.json](distro-validation.json)。这些是共享主机内核的用户空间 / PTY 检查，不代替实机 VT 或完整发行版启动验证。

ZIP：在 `.zip` 上 Enter 选择 **Bash unzip / 解压 ZIP**（或 `o` 使用默认项），通过 Bash 调用 `unzip`，解压到压缩包旁全新的 `*.unpacked.随机后缀` 目录；原 ZIP 保留，不覆盖既有文件。需 Bash、unzip 和 Python 3（仅用于提取前校验 ZIP 路径／链接）；Windows 可用 Git Bash，并把 Python 加入 PATH。ZIP 内含越界路径或符号链接时会拒绝解压；失败保留部分输出目录以供检查。

多选方框默认为亮洋红、加粗显示，当前光标所在项继续反色；可在 `[colors]` 设置 `mark_color = "#FF55FF"`。TTY 使用 ASCII 方框，16 色终端映射到亮洋红。

打开菜单会隐藏不适用的专用程序：Python 对 `.py/.pyw`、Rscript 对 `.r`、Bash 对 `.sh/.bash`、unzip 对 `.zip`、Toolkit Markdown 对 `.md/.markdown`；Windows PowerShell 执行仅对 `.ps1` 显示。普通编辑器／系统关联仍通用。自定义 opener 可设置 `extensions = [".foo", ".bar"]`（大小写不敏感）；未设置或空列表表示通用。内置专用项即使旧配置没有 extensions 也会自动限制。

目录顺序可在配置中设置；置顶／沉底优先于“目录在前”，列表顺序也会保留：

```toml
[order]
top = ["index.html"]
bottom = ["node_modules", "__pycache__"]

[directory_order."~/Documents/site"]
top = ["index.html", "package.json"]
bottom = ["<other-folders>", "node_modules"]
```

`directory_order` 可用实际绝对目录路径（Windows 使用 TOML 单引号，例如 `[directory_order.'C:\work\site']`），或用目录名匹配所有同名目录；绝对路径规则优先，只作用于该目录直接子项。省略字段继承全局，显式 `[]` 清空继承。修改后按 `,` 重读配置。

Git 仓库展开后，树节点箭头附加状态标记并着色：`?` 未跟踪、`M` 未暂存修改、`S` 已暂存、`!` 冲突、`C` 已跟踪且干净但未配置上游、`P` 干净且与本地记录的上游一致、`↑/↓/↕` 分别为领先／落后／分叉（TTY 用 `^/v/D`）。文件名颜色保留，目录汇总子项状态，底部说明当前项目状态。颜色用于提示，字符标记在无色模式也保留。

Git 后台只读扫描，不自动 commit/push/fetch；`P` 不代表已实时查询服务器。F5／Ctrl+F5 更新本地快照；没有 Git 或查询失败时保持普通箭头。`[git] enabled = false` 可关闭，`max_entries` 默认 4000，每个窗口至多缓存 4 个已展开仓库；超出扫描范围不猜测状态。

排序列表中的 `"<other-folders>"` 匹配未在当前有效 top/bottom 中点名的所有直接子目录（不含目录软链接）。上述示例将 index.html、package.json 置顶，普通文件居中，其余子目录沉底，node_modules 最后；也可把该标记放入 top。显式名称优先，同组目录沿用 sort，隐藏目录需开启显示。

复制／移动：按 `c`／`m` 后可用光标选目标目录，或直接选目标文件进行替换；按 `t` 可输入目标地址，Enter 提交。单项支持目录或完整的新文件路径，多选要求已有目标目录；相对路径以源所在目录为基准，支持 `~`。已有普通文件会逐一提示，再按 Enter 确认覆盖，Esc 返回选择状态；全部覆盖确认完成后才开始本批操作。目录／链接冲突不覆盖。`g`／Ctrl+L 仍可输入目录并切换视图，再 Enter 执行。

选中文件夹后按 `!`，输入 `npm start` 等命令并 Enter，即以该目录为工作目录后台运行；Esc 取消输入。启动后显示 PID 和日志路径，关闭 filex 不停止命令。`[keys] run_background = "!"` 可改键。Linux 使用 `$SHELL -c`（默认 `/bin/sh`），日志在 `$XDG_STATE_HOME/filex/jobs` 或 `~/.local/state/filex/jobs`；Windows 使用 CMD，日志在 `%LOCALAPPDATA%\win-filex\jobs`。标准输入关闭，适用于不需要交互输入的命令；退出后仍可从日志看输出，依赖与 PATH 沿用启动 filex 的环境。

Linux 后台任务可用 `kill -- -PID` 停止对应进程组（PID 使用 filex 启动提示中的值）。日志需自行清理；服务能否启动仍取决于该项目依赖和命令自身。

`G` 打开当前选中项所属仓库的 Git 菜单：一键 `add ALL → commit → push`、只暂存当前文件／目录、提交全部已暂存内容、单独推送。菜单选择后输入提交信息（需要时），再 Enter 确认；Esc 取消。组合流程暂存整个仓库的新增／修改／删除，包含当前视图外的文件；只暂存当前项不受 Space 多选影响。执行时使用当前终端，支持认证和签名提示，完成或失败后 Enter 返回；失败即停止后续步骤，已完成步骤保留。不自动 pull、force push、设置上游或初始化仓库，push 遵循 Git 的 remote/refspec 配置；首次推送可先在终端设置 upstream。`[keys] git_menu = "G"` 可修改。

复制／移动模式下，目录末尾的虚拟 `+` 是放入位置：选中它按 Enter，直接将待操作项目复制／移动到其所在目录；按 `+` 键也会放入当前目标目录（选中文件时使用其父目录）。`A`／配置的新建文件夹键仍可创建子目录，`t` 仍可输入目标地址。同名普通文件沿用 Enter 覆盖确认。


名称通配与完整顺序（文件和目录都适用，`%` 匹配任意长度，`-` 是普通字符）：

```toml
[directory_order."~/Documents/site"]
sequence = ["README.md", "d-%", "%-d", "%"]
# 首次匹配生效：README.md → d- 开头 → -d 结尾 → 所有剩余项。
# d% 匹配所有 d 开头的名称；=100%.txt 强制按字面匹配百分号。
```

`sequence` 优先于 top/bottom；未匹配项回退到原规则。top/bottom 同样支持 `%`，并保留显式名称高于通配的优先级；同一通配组内沿用原有 sort。分组树会分别在目录列、文件列体现该顺序，普通树可显示完整混合顺序。

`O` 编辑当前选中项的同级列表（根目录上则编辑其直接子项），↑↓ 选择，**K/J** 调整位置，Enter 保存为该目录专用 sequence；Esc 放弃修改。保存当前已加载、可见的项，并用末尾 `%` 接收未列出的项目；隐藏／未加载项不会被永久排除。保留配置其他设置与注释，并沿用配置软链接。

`z` 隐藏选中项，仅在 filex 的树和搜索中不可见，不重命名或删除；目录包含的整个子树同时隐藏。路径写入当前配置 `[browser] excluded_paths`，下次启动继续生效。`.` 只切换点文件，不能恢复这些排除项；`Z` 打开列表，选中项目 Enter 恢复。隐藏规则按绝对路径限定当前位置，不影响其他目录里的同名项；根目录本身不可隐藏。移动／重命名后按新路径处理，旧规则可在 Z 中移除。

Ctrl+F（或 `/`）输入关键词并 Enter 后进入搜索模式：↑／↓ 在匹配项目之间循环，可跨目录定位；n/N 仍可切换匹配。单个结果保持选中，零结果不移动。Esc 退出搜索模式，保留当前光标及展开状态；随后 ↑／↓ 恢复同级树导航。搜索模式中首次 Esc 不清空多选，也不取消待执行的复制／移动；再次 Esc 按原规则处理。

在 Arch / KDE / Wayland 下可使用相邻 [桌面整合](../integrate-toolkit-arch/README.md)：光标选中文件后按 **T**，进入原生竖向 Toolkit 工具菜单，↑↓／Enter 选方法，再选择保存目录与结果文件名。`[keys] toolkit = "T"` 可改键；`FILEX_TOOLKIT_DESKTOP` 可指定其他启动器。原文件保留，已有结果名称不覆盖。

**F 临时聚焦**：选中文件／文件夹按 F，仅在该项的直接父目录内隐藏其他同级项，保留该项及其展开内容；父目录及更高祖先的其他分支照常显示和导航，↑↓ 跳过被隐藏项；各父目录可分别聚焦并叠加；在已聚焦项上再按 F，恢复该级并清除其全部子级聚焦，其他分支的聚焦保留。保留原根目录、展开状态、多选及排序，不修改 excluded_paths，不写入配置；退出重启即恢复。搜索跳过被隐藏的同级项及其子树，其他分支仍可查找，更换根目录或聚焦项被删除后自动退出。快捷键可用 `[keys] temporary_focus = "F"` 修改，小写 f 仍为设为根目录。

选中文件夹后按 **@（Shift+2）**，输入 `unzip archive.zip` 等命令并 Enter，在当前终端前台执行，显示输出和错误，支持覆盖提示等交互输入。命令结束后 Enter 返回并刷新树，Ctrl+C 可中断命令。`!` 仍在后台执行并写日志；两者都使用所选目录作为工作目录。`[keys] run_foreground = "@"` 可自定义。普通终端不能单独区分 ! 与 Shift+!，因此使用 @ 作为可兼容 TTY 的前台入口。

本次验证：两产品各 55 项共享 Rust 测试、80 项 Linux Python / PTY 检查全通过。临时聚焦覆盖 folder/file、自定义快捷键、保留父链与展开状态、原指针恢复、搜索范围、目标删除、根变更、批量展开收起不影响其他分支和传输期间恢复；前台命令覆盖当前目录、真实 unzip、读输入、输出/错误、非零退出、向 parent/child 发送 SIGINT 后返回界面并新建文件。最终 Linux release 的 8 项内存场景均小于 15 MB，最高 VmHWM 7.786 MB；原始记录及二进制 SHA256 见 fileX/memory-validation.json。Windows release 和测试程序已交叉编译，尚未实机验证。

2026-10-25 局部聚焦调整：F 仅隐藏所选项的直接同级及其子树，父级和更高祖先的其他分支保持显示、导航与搜索。两产品各 56 项 Rust 测试通过；Linux 81 项 Python / PTY 检查重跑全通过（首次一项窗口缩放时序失败，单项及整套重跑均通过）。最新 release 的 8 项内存场景峰值 7.309 MB；Windows release / 测试程序交叉编译通过，未实机验证。

2026-10-25 默认布局调整：内置值、示例配置及本机主配置使用 `[layout] grouped = false`，同级目录／文件默认垂直罗列；v 开启直属文件独立分组，再按恢复。F 仍只聚焦直接父目录内的分支。两产品各 56 项 Rust 测试及 Linux 26 项配置／导航／TTY 检查通过；Linux、Windows release 已重新编译，Windows 未实机验证。
