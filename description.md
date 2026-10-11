# filex 1.3

## 当前范围与版本

2026-10-11：在原有文件管理功能基础上增加 q 信息面板、Ctrl+F 递归名称搜索、分支全部展开 / 收起、可配置打开程序和扩展名配色、保留树状态的上级根目录导航。随后按用户要求把默认 TUI 重构为 Rust，保留现有 Python/PySide6 GUI；GUI 的 <50 MB 目标留待单独优化。当前 TUI 的实测目标为应用 release 进程 RSS <15 MB。

## 导航、布局与 tmux

方向键随布局旋转：横向 ↑/↓ 选择同级、→ 展开 / 进入子级、← 返回父级；纵向 ←/→ 选择同级、↓ 展开 / 进入子级、↑ 返回父级。j/k 仍是同级前后选择，l 仍是展开 / 进入。同级选择只在 parent.children 中移动，越界时停在同级首末项；根节点无兄弟时不移动，已展开目录也不会让同级选择进入后代。返回父级键直接选择父节点，不收起任何分支；在根节点使用它时，将原根节点挂到新父节点下并选中新根，旧分支的节点身份与展开状态保留。h 仅收起选中的目录。f / keys.focus_root 将选中目录（文件时为其父目录）设为根，并重新读取目录；保留该子树展开状态、清空搜索、重置视口。Rust 会释放当前根以外的缓存，之后可用返回父级键重新引入上级目录。旧 parent=u 配置自动迁移为 Left；该配置名称沿用横向基准，纵向的固定父级方向键为 Up。

v / keys.toggle_tree 切换横向与纵向布局，配置 layout.orientation 可设默认 horizontal / vertical；临时切换不自动写回配置。两种界面共用同一节点树，纵向布局为母节点在上、子节点在下并排的树图。自底向上计算各子树宽度，给不同分支预留独立区域；各层间留连接线行，同级子节点左右排列，母节点尽量居于首末子节点的中间，长标签受所在子树范围约束。GUI 提供「纵向树」按钮。TUI 标题与 GUI 窗口标题均为 filex。

Rust TUI 打开 terminal=true 程序时，tmux.open_in_right_pane=true 会检查当前 window 中位于右侧、垂直范围有交集的 pane，优先最近者。仅复用常见 shell（bash/zsh/fish/sh/dash/ksh）的普通屏幕：shell 必须在休眠等待、占有前台进程组且没有子进程；光标行必须是没有输入草稿的常规提示符（末尾 $/#/%/>）。复制模式、程序运行中、有草稿或无法识别的自定义提示符均回退当前终端。pane 状态检查到发送之间仍有极短的竞态窗口；这是对已存在交互 shell 的复用，不创建或终止 pane。

路由通过 tmux send-keys 排队带单引号转义的字面参数，使用 env 和解析后的绝对可执行路径，避免 shell alias / function 和文件名中的命令替换；包含换行或非 UTF-8 参数时回退直接执行。成功后聚焦目标 pane，filex 留在原 pane 并保留树状态。GUI 的终端程序继续使用 Konsole。可用 [tmux] open_in_right_pane=false 关闭路由。

Bash alias 保留；另安装 ~/.local/bin/filex，让 tmux split-window / new-window 和非交互 shell 可以调用，不依赖 alias 展开。启动脚本保持调用者当前目录；现有 ~/.config/tmux/tmux.conf 已将普通方向键传给应用，未改动用户 tmux 配置或活跃会话。

## Rust TUI 与内存设计

- 不启动 Python 后台进程。Rust + Crossterm 直接处理终端事件和输出；程序无常驻网络服务。
- 目录读取使用有上限的 top-K heap，不把一个目录的全部条目常驻内存。默认显示 1000 项。
- 默认最多缓存 6000 个目录 / 文件节点。空间不足时释放已经收起的分支内部缓存；已展开分支不会因普通选择或搜索定位而自动收起。缓存仍满时提示未显示数量，需收起部分分支或调高 `max_loaded_nodes`。引入上级目录需额外的祖先节点。
- 全部分支展开受 `max_expand_nodes=5000` 的访问预算约束，不跟随符号链接。
- 搜索流式遍历目录，默认扫描最多 100000 项，保存最多 500 个结果，不受每目录显示上限影响。最多遍历 128 层，跳过不可读目录与链接目标，并提示上限 / 跳过情况。显示隐藏文件开关也控制搜索范围。
- 搜索定位只打开结果的祖先路径；其他已经展开的分支保持展开。根节点处的 `←` 把已有根节点挂在新父节点下，保留分支并选中新根；`g` 显式换根则重建 Rust 缓存。
- 预览默认读取最多 65536 bytes，不读取特殊文件。显示区域只绘制可见字符格。

增加缓存、显示、结果数量或预览上限会增加内存使用。测量证明下述工作负载达标，不能将其视为所有配置、任意深度或异常长路径下的绝对上限。

[内存测量记录](memory-validation.json) 使用真实 PTY 的 Linux `/proc/<pid>/status` `VmHWM`，覆盖浏览、全部展开、收起、刷新、Dashboard、搜索、预览和上级根导航。计入 release 应用进程的 resident memory；不计终端、测量脚本、编译器和用户选择打开的外部程序。MB 使用 1000000 bytes，报告也列出 MiB。

首次测量峰值：项目目录 3.469 MB，20000 个文件的单目录 3.809 MB，7200 个文件的多分支目录 6.410 MB。v1.3 测量峰值分别为 3.998 / 3.94 / 6.275 MB，包含纵向布局切换；最终构建的精确数值以 JSON 中的记录为准。

## 文件信息与打开方式

Rust Dashboard 使用 Linux statx 的 BTIME 和文件元数据；创建时间未提供时显示不可用，不用 ctime 冒充。GUI 使用现有元数据功能，调用 GNU stat 获取真正 birth time。修改时间为 mtime。普通文件显示字节数，目录显示自身元数据大小，符号链接显示链接自身大小，均有标识。

`[openers.NAME]` 定义 label、command 参数列表和 terminal 布尔值，`{path}` 替换完整文件路径，普通直接打开使用参数数组；复用 tmux 空闲 shell 时按上述转义规则发送命令。Enter 显示打开方式菜单，按 `[open_defaults]` 的最长扩展名后缀排序默认项；`o` 直接打开。目录菜单另有 fileX 展开 / 收起选项。TUI 的 terminal=true 程序先尝试 tmux 右侧 pane；在当前终端运行时恢复终端模式，退出后回到原树；GUI 使用 Konsole。已加入 Nano：openers.nano 的 command=["nano", "--", "{path}"]、terminal=true；Enter 菜单可选择，也可在 open_defaults 中把指定后缀映射为 nano。未安装的程序明确提示。

`[extension_colors]` 使用 #RRGGBB，大小写不敏感，支持 .tar.gz 等复合后缀。Rust TUI 直接发出终端 RGB 颜色，Konsole 已支持；终端自身的颜色能力会影响实际显示。GUI 使用相同颜色。默认 .py 用 VSCodium，.html / .htm 用 Firefox，其余使用系统默认程序；所有内置默认可覆盖，其他内置打开程序仍保留在菜单中。

## 文件操作边界

新建、复制不覆盖已有目标。重命名和同设备移动用 Linux renameat2(RENAME_NOREPLACE)。跨设备移动先复制成功，再删除源；中途失败可能留有部分副本，源删除阶段失败可能留有部分源目录。目录复制保留符号链接，不允许复制 / 移动到自身内部；特殊文件拒绝复制，复制目录最多 128 层。Rust 复制普通文件保留权限和访问 / 修改时间，不保证所有 ACL、扩展属性与目录时间元数据。

TUI 的 Delete 键与 D 均调用 gio trash，输入 YES 才移到回收站；Esc 或其他输入取消，不会退回永久删除。根目录的重命名 / 移动 / 删除需先引入其父目录。本机 /tmp 等内部挂载可能拒绝回收站操作，实际错误会显示。恢复在 Dolphin 回收站完成。

## 配置区段

| 区段 | 可自定义内容 |
|---|---|
| browser | show_hidden, directories_first, sort, max_entries, max_expand_nodes, max_loaded_nodes（Rust TUI） |
| layout | orientation, column_gap, max_label_width, row_spacing |
| tmux | open_in_right_pane（Rust TUI） |
| dashboard | dashboard_width（TUI） |
| search | search_max_results, search_scan_limit |
| preview | preview_bytes |
| gui | gui_font_size |
| colors | GUI 主题、文件夹 / 文件 / 连接线颜色 |
| extension_colors | 扩展名 → #RRGGBB；两种界面适用 |
| openers | 程序名称 → label / command / terminal |
| open_defaults | 扩展名 → 已定义的程序名称 |
| keys | 单字符 / Ctrl+字母快捷键，parent 支持 Left，toggle_tree 默认 v |

默认个人配置路径是 XDG_CONFIG_HOME/fileX/fileX.conf；源 [fileX.conf](fileX.conf) 给出完整样本。类型、范围、未知区段 / 字段、程序引用、颜色、快捷键重复会被拒绝，重载失败保留旧配置。旧配置 quit=q 且未定义 dashboard 时自动迁移为 quit=Q。Ctrl+F 与 / 固定搜索，Ctrl+L 固定路径输入；导航字符和终端控制组合保留。

## 文件与维护

- `Cargo.toml` / `Cargo.lock`：Rust 依赖与锁定版本，release 使用 LTO / stripped binary。
- `src/config.rs`、`model.rs`、`files.rs`、`tui.rs`、`main.rs`、`tmux.rs`、`tests.rs`：配置、有限缓存树与布局、文件操作 / statx / 打开方式、终端界面、CLI、tmux 右侧 pane 路由、Rust 测试。
- `filex/gui.py`：Qt GUI 主窗口与画布，`filex --gui` 启动；`filex/layout.py` 负责两种树布局。
- `filex/`：保留的 Python GUI 与对应模型 / 配置 / 操作，`details.py` 和 `opening.py` 提供元数据与可配置启动器。原 Python TUI 为历史兼容实现，默认启动器已使用 Rust。
- `filex.sh`：保留调用者工作目录，默认 exec Rust release 程序，--gui exec 现有 Qt GUI。缺少 release 时自动编译。
- `scripts/build.sh`、`measure_memory.py`：更新 release 程序、验证应用内存。
- `scripts/install_alias.py`：只维护 .bashrc 中本项目的标记区块，拒绝覆盖无关 alias / function；本机已安装 alias。已有 shell 需要 source ~/.bashrc，新交互 shell 自动生效。
- `scripts/install.sh` / `uninstall.sh`：--command-only 只装用户命令；默认也安装 KDE 菜单入口，alias 单独安装 / 移除。
- `tests/`：Python GUI 回归、Rust CLI 与真实 PTY 的集成检查、Wayland 窗口截图和 GUI 内存基线。

```bash
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
./scripts/build.sh
python -m unittest discover -s tests -v
python scripts/measure_memory.py
QT_QPA_PLATFORM=wayland python tests/wayland_smoke.py
```

## 验证与后续 GUI 目标

Rust 14 项单元检查与 31 项集成 / GUI 回归检查通过。实际 Nano 通过真实 PTY 打开 / Ctrl+X 返回，f 重新设根后进入文件的完整流程也通过。真实 Rust PTY 检查涵盖文件操作、q Dashboard、Ctrl+F 找到未展开目录中的文件、打开方式选择、当前终端运行编辑器并返回、向上引入根目录、Delete 取消 / 确认 / 实际回收站内容恢复证据和缩小终端后的操作。Rust Clippy 无 warning，release 已构建并接入 filex alias。隔离 tmux server 实测覆盖 alias 当前目录、同级方向键、横纵切换、右侧空闲路由、繁忙回退、输入草稿保留、含引号 / shell 符号的文件名和 resize。实际 Wayland GUI 两种布局已验证，截图为 [preview.png](preview.png) 与 [preview-vertical.png](preview-vertical.png)。

GUI 当前仍使用 Python/PySide6，<50 MB 尚未完成；[GUI 实测基线](gui-memory-baseline.json) 单独记录一个实际 Wayland 窗口的内存。后续需评估 Rust GUI 与原生 Wayland 渲染方案，并在选择库后实测，不能只凭语言迁移保证达标。

## 参考

- [Crossterm 事件](https://docs.rs/crossterm/latest/crossterm/event/index.html)
- [Linux statx](https://man7.org/linux/man-pages/man2/statx.2.html)
- [Qt QGraphicsView](https://doc.qt.io/qtforpython-6/PySide6/QtWidgets/QGraphicsView.html)

## v1.0 文档存档

以下保存第一版 README 与详细说明，供历史追溯；旧的 q 退出、仅搜索可见节点、Python 为 TUI 前置条件等描述已由上述 v1.2 行为取代。

### What is it?

fileX 1.0 是面向 Arch Linux / KDE Wayland 的横向树文件管理器。母子目录和文件由线连接，可同时展开多个分支。默认启动 TUI，也提供原生 Qt GUI；支持浏览、搜索、预览、新建、重命名、复制、移动和回收站删除。

设计、限制、源码说明和验证记录见 [description.md](description.md)。[GUI 预览](preview.png)。

### How to use

需要 Python 3.11+；TUI 使用 Python 自带 curses。GUI 需要 PySide6，默认程序打开需要 xdg-utils，回收站需要 glib2 的 `gio`。Arch 上缺少依赖时安装：

```bash
sudo pacman -S python pyside6 xdg-utils glib2
```

在 `fileX` 目录运行：

```bash
./filex.sh ~/Documents                 # TUI；省略路径则浏览当前工作目录
./filex.sh --gui ~/Documents           # GUI
./filex.sh --print-tree ~/Documents    # 输出第一层树
```

TUI 使用 `↑/↓` 或 `k/j` 选择节点，`→/l` 展开或进入子节点，`←/h` 折叠或选择父节点；Enter 展开目录或用默认程序打开文件。`H/L` 横向平移，PgUp/PgDn 翻页，`u` 将根目录上移一级，`g` 输入新根目录。

| 默认键 | 操作 |
|---|---|
| `p` | 文本 / 元数据预览；Esc 返回 |
| `/`，`n`，`N` | 搜索已展开的节点、下一个 / 上一个匹配 |
| `.`，`r` | 显示隐藏文件、刷新 |
| `a`，`A` | 新建空文件、新建目录；位置为选中目录或文件的父目录 |
| `R` | 重命名，输入单个新名称 |
| `c`，`m` | 复制、移动，输入**完整目标路径，包含新名称** |
| `D` | 送入回收站；输入 `YES` 确认，可在 Dolphin 回收站恢复 |
| `,` | 重新读取配置 |
| `?`，`q` | 帮助、退出 |

输入时 Enter 确认，Esc 取消，Ctrl+U 清空。复制 / 移动的相对路径以源项目的父目录为基准，例如选中 `~/Documents/a.txt` 后输入 `../Downloads/b.txt`。不覆盖已有目标，目标父目录必须已存在。

GUI 单击选择，双击目录展开 / 折叠，双击文件打开；拖动空白处平移，Ctrl+滚轮缩放。工具栏和右键菜单提供文件操作。画布获得焦点时可使用上述快捷键；Ctrl+L 编辑根路径，Ctrl+F 搜索，F5 刷新，F2 重命名，Delete 送入回收站（再次确认）。

创建并编辑个人配置：

```bash
./filex.sh --init-config
# 编辑 ~/.config/fileX/fileX.conf（遵循 XDG_CONFIG_HOME）
```

配置采用 TOML 格式，可调整隐藏文件、排序、目录显示上限、节点间距、标签宽度、预览大小、GUI 字号 / 配色和快捷键。直接使用项目中的样本也可以：

```bash
./filex.sh --config ./fileX.conf ~/Documents
./filex.sh --gui --config ./fileX.conf ~/Documents
```

修改后按 `,` 或点 GUI「读取配置」。错误配置会提示原因；重载失败时保留现有配置。配色只用于 GUI，TUI 使用终端基本色。

可选安装命令与 KDE 应用菜单入口（保留源码目录，不需要 sudo）：

```bash
./scripts/install.sh
# ~/.local/bin 位于 PATH 时：filex ~/Documents / filex --gui ~/Documents
./scripts/uninstall.sh
```

安装后源码目录不能随意移动；移动后应重新安装入口。卸载只移除本项目的入口，保留配置与用户文件。默认文件管理器保持原设置。

### v1.0 详细说明

#### 产品范围

第一版面向 Arch Linux、KDE Plasma、Wayland；优先完成无需额外 Python 库的 TUI，另提供 PySide6 Qt Widgets GUI。默认只展开根目录的一层，用户可同时展开多个分支。每个父节点与第一个子节点位于同一行，后续子分支往下排列；列宽随当前已展开节点的标签变化。长标签截断，完整路径显示于状态栏，GUI 节点也有悬浮提示。

```text
▾ folder(a)──┬───▾ folder(a1)──────· file(a1a)
             └───▾ folder(a2)──┬───· file(a2a)
                              ├───· file(a2b)
                              └───· image(a2c)
```

搜索匹配**当前已经展开 / 加载的可见节点的名称**，不递归扫描硬盘。折叠的目录、隐藏文件开关关闭时的隐藏文件、超过显示上限的条目不在搜索结果中。默认每目录上限 1000；选中该目录时提示未显示的数量，可增加 `max_entries` 后刷新。排序仍读取整个当前目录的条目信息，显示上限用于控制画布规模。

符号链接显示 `↗`，不自动遍历链接目标，避免循环。打开链接调用默认程序；需要在 fileX 中浏览目标时，可用根路径输入其目录路径。根路径本身会解析符号链接。文本预览最多读取配置的字节数，仅支持 UTF-8；二进制、图片、音视频显示元数据并可用默认程序打开。设备、FIFO、socket 不读取。名称和文本中的控制字符被替换，避免污染终端显示。

#### 文件操作语义

- 新建文件采用 exclusive create；已有文件不会被清空。目录新建拒绝同名目标。
- 重命名使用 Linux `renameat2(RENAME_NOREPLACE)`，原子拒绝覆盖。
- 同文件系统移动同样使用原子、不覆盖的重命名；跨文件系统移动先复制，成功后才删除源。
- 复制文件 / 符号链接独占创建目标；递归复制保留符号链接本身。不能将目录复制或移动到自身或其子目录。
- 操作不执行 shell 拼接命令，文件名不会被作为 shell 代码解释。
- 当前浏览根目录不能直接重命名、移动或送入回收站；先浏览其父目录再操作。
- 删除仅调用 `gio trash -- PATH`，两种界面均先确认；缺少 gio 或回收站拒绝操作时报告实际失败原因，不改为永久删除。恢复通过 Dolphin 回收站完成。本机 gio 拒绝 `/tmp` 等系统内部挂载的回收站操作；普通用户数据目录已实际验证。

没有覆盖、自动合并目录、批量多选、内部撤销或跨设备事务恢复功能。跨设备操作中途失败可能保留部分目标副本；源文件在复制失败时仍保留，用户应检查错误后的目标。跨设备目录删除阶段失败时可能留下部分源目录。移动大型文件时不要从其他程序同时修改源。复制采用 Python shutil 的标准 metadata 保留能力，不保证全部 ACL / 扩展属性。安装脚本不修改默认文件管理器。

#### 配置

默认路径 `${XDG_CONFIG_HOME:-~/.config}/fileX/fileX.conf`，可用 `--config PATH` 替换。`--init-config` 只在目标不存在时写入样本。源码中的 [fileX.conf](fileX.conf) 是完整可编辑样本。

| 区段 | 选项 | 说明 |
|---|---|---|
| browser | show_hidden, directories_first, sort, max_entries | sort 为 name / size / modified；后两者降序 |
| layout | column_gap, max_label_width, row_spacing | 母子列距、标签宽度、行距，以字符格表达 |
| preview | preview_bytes | 1024–1048576，默认 65536 |
| gui | gui_font_size | 8–32，默认 11 |
| colors | background, foreground, folder_color, file_color, selected_color, line_color | `#RRGGBB`；只应用于 GUI |
| keys | 见样本 | 可打印单字符，不得重复；保留导航字符不能占用 |

配置严格校验类型、范围、区段、选项、颜色和键冲突。启动时错误配置阻止启动，运行中重载错误保留旧配置。运行中的隐藏文件开关不会自动改写配置；需要持久默认值时手动编辑。无需常驻后台服务或外部网络。

#### 架构与文件

- `filex/core.py`：目录模型、排序、按需读取、导航、Unicode 标签、文件操作、预览和默认打开。
- `filex/layout.py`：两端共用的横向布局，TUI 字符连接线与非交互树输出。
- `filex/config.py`：TOML 配置读取与验证。
- `filex/actions.py`：两端共用的文件操作入口与根目录保护。
- `filex/tui.py`：curses 画布、视口跟随、键盘导航、输入、预览和帮助。
- `filex/gui.py`：原生 Qt 场景、节点、预览、缩放、菜单和操作输入。目录展开 / 刷新及文件操作在线程池执行；执行期间禁用交互，避免模型竞争，完成后在 GUI 主线程重绘。操作进行时阻止关闭窗口。
- `filex/__main__.py`：命令行选项；TUI 不导入 GUI 依赖。
- `filex.sh`：保留调用者当前目录的启动器，通过 PYTHONPATH 定位本项目。
- `scripts/install.sh` / `uninstall.sh`：本地用户命令和 KDE 菜单入口。默认写入 ~/.local/bin 和 XDG_DATA_HOME/applications；`--prefix PATH` 可安装到隔离目录。
- `tests/test_core.py`、`test_tui.py`、`test_gui.py`、`test_cli.py`：核心边界、真实伪终端交互、Qt 离屏交互、配置初始化和安装生命周期检查。
- `tests/wayland_smoke.py`：本机 Wayland 窗口验证与 [preview.png](preview.png) 生成。

#### 开发与维护

Python 3.11+（使用 tomllib），Linux glibc，GUI 使用 PySide6。Arch 依赖安装和使用见 README。所有工作位于 fileX 目录，没有修改其他产品。

```bash
python -m unittest discover -s tests -v
python -m compileall -q filex
QT_QPA_PLATFORM=wayland python tests/wayland_smoke.py
```

GUI 自动测试使用 `offscreen`；未安装 PySide6 时跳过 GUI 检查，仍能验证 TUI。Wayland 检查必须从桌面会话执行。大目录扫描 / 大型复制在 TUI 中会阻塞交互，GUI 操作期间禁用交互；本版没有进度百分比或中途取消。预览仍由界面线程读取有限字节，慢速网络挂载可能有延迟。刷新为手动，暂无文件系统 watcher。复杂 emoji 的终端字符宽度可能随字体与终端实现不同；中日韩文字和组合重音已覆盖测试。长树通过滚动 / 平移浏览。

#### 验证记录

2026-10-11，在本机 Python 3.14.7、PySide6 6.12.0、KDE Wayland 环境验证：

- 19 项自动检查通过：多分支连线、展开折叠、隐藏文件、配置类型 / 范围 / 按键冲突、符号链接循环和断链、中文 / 控制字符、有限 UTF-8 预览、特殊文件、目标不覆盖、目录自身内部操作拒绝、原子移动 inode 保留、根目录保护、GUI 线程池和无效配置重载。
- 真实 PTY 通过键盘完成中文新建、重命名、复制、移动、新目录、帮助、预览、搜索与缩小终端后的平移 / 刷新 / 退出。
- 实际 Qt Wayland 窗口成功显示 8 个示例节点，捕获并检查截图；与离屏 GUI 测试分开验证。
- gio 实际将项目内的独立测试文件送入隔离的 XDG 回收站，验证内容和 trashinfo；测试目录随后清理，没有触碰已有回收站内容。
- 在临时安装 prefix 中验证重复安装、从其他工作目录启动、卸载以及拒绝覆盖无关命令。实际跨设备分支以 EXDEV 故障注入验证，尚未在真实跨设备大目录上进行压力测试。

#### 来源与实现参考

- [Python curses](https://docs.python.org/3/library/curses.html)
- [Python tomllib](https://docs.python.org/3/library/tomllib.html)
- [Qt QGraphicsView](https://doc.qt.io/qtforpython-6/PySide6/QtWidgets/QGraphicsView.html) 与 [Graphics View](https://doc.qt.io/qtforpython-6/overviews/qtwidgets-graphicsview.html)：原生 GUI 场景与缩放参考。
- Linux renameat2 与 gio 的本机运行行为由实际测试验证。

## v1.2 README 存档

以下保留 v1.3 导航与 tmux 调整之前的使用说明；当前行为以上文和 README 为准。

### What is it?

fileX 1.2 是面向 Arch Linux / KDE Wayland 的横向树文件管理器。默认 TUI 已用 Rust 重构，母子目录和文件由线连接，支持多分支展开、搜索定位、文件信息和常用文件操作。现有 Qt GUI 保留，通过 `--gui` 启动。

配置、架构、内存测量、限制和历史记录见 [description.md](description.md)。[GUI 预览](preview.png)。

### How to use

本机已经编译好 TUI，并安装 `filex` Bash alias。已有终端先执行一次：

```bash
source ~/.bashrc
filex               # 以当前工作目录为根，启动 Rust TUI
filex ~/Documents   # 指定根目录
filex --gui         # 以当前工作目录为根，启动现有 Qt GUI
```

也可直接运行 `./filex.sh`，无须 alias。TUI 首次编译需要 Rust / Cargo；编译后运行不需要 Python。GUI 需要 Python 3.11+ / PySide6，文件打开需要 xdg-utils，回收站需要 glib2 的 gio；GUI 打开终端编辑器需要 Konsole。Arch 缺少依赖时安装：

```bash
sudo pacman -S rust python pyside6 xdg-utils glib2 konsole
./scripts/build.sh          # 构建 / 更新 release TUI
```

启动脚本在缺少 release 程序时自动编译。VSCodium、Vim、Neovim、Firefox 是可选打开程序，使用前需已安装。

| 默认键 | 操作 |
|---|---|
| `↑/↓` 或 `k/j` | 选择节点；其他分支保持展开 |
| `→/l`，`←/h` | 展开 / 进入子节点，折叠 / 返回父节点 |
| `q` | 切换右侧 Dashboard：创建时间、修改时间、大小；随选中节点更新 |
| `Ctrl+F` 或 `/` | 递归搜寻根目录下文件 / 目录名，含未展开目录；Enter 定位 |
| `n`，`N` | 下一个 / 上一个搜索结果，保留其他展开分支 |
| `.` | 显示 / 隐藏隐藏文件和目录 |
| `E`，`C` | 展开 / 收起选中目录下的全部分支（受配置上限约束） |
| `Enter` 或 `w` | 选择打开方式：系统默认、VSCodium、Vim、Neovim、Firefox 或配置中的程序 |
| `o` | 直接使用该扩展名的默认打开方式 |
| `u` | 向上引入新根目录，旧根目录成为子目录，保留展开状态和选中位置 |
| `g` / `Ctrl+L` | 输入其他根目录；显式更换根目录时 Rust TUI 重建树缓存 |
| `p` | 文本预览，Esc 返回 |
| `a`，`A` | 新建空文件 / 目录，位置为选中目录或文件的父目录 |
| `R`，`c`，`m` | 重命名、复制、移动 |
| `D` | 送入回收站；输入 `YES` 确认，可在 Dolphin 回收站恢复 |
| `r`，`,` | 刷新目录、重载配置 |
| `?`，`Q` | 帮助、退出（小写 q 用于 Dashboard） |

`H/L` 横移，PgUp/PgDn 翻页，Home/End 选择首末节点；输入时 Enter 确认、Esc 取消、Ctrl+U 清空。复制 / 移动填写**完整目标路径，包含目标名称**；相对路径以源项目的父目录为基准，不覆盖已有目标，目标父目录必须存在。文件树不自动遍历符号链接。

Enter 选中目录时还可选择在 fileX 展开 / 收起。TUI 使用 Vim/Neovim 时在当前终端运行，退出编辑器后返回原树；GUI 在 Konsole 打开终端编辑器。GUI 单击选择，双击目录展开 / 折叠，双击文件选择打开方式；拖动空白处平移，Ctrl+滚轮缩放，工具栏和右键菜单提供操作。画布焦点下快捷键与 TUI 相同，F5 刷新、F2 重命名、Delete 回收站。

个人配置位于 `~/.config/fileX/fileX.conf`（遵循 XDG_CONFIG_HOME），本机已创建。源码 [fileX.conf](fileX.conf) 是完整样本；用 `--config PATH` 可选择其他配置，缺少个人配置时 `filex --init-config` 创建且不覆盖已有文件。修改后按 `,` 或 GUI「读取配置」。

配置可以定制：隐藏文件、排序、目录显示 / 缓存 / 展开上限、搜索上限、横向间距、标签宽度、行距、Dashboard 宽度、预览大小、GUI 字号 / 主题、单字符或 Ctrl+字母快捷键、打开程序和参数、扩展名默认打开方式、扩展名颜色。例如：

```toml
[open_defaults]
".py" = "vscodium"
".html" = "firefox"

[extension_colors]
".py" = "#e5c07b"
".html" = "#e06c75"
```

上述配置与已有区段合并编辑，不重复添加同名 TOML 区段。扩展名大小写不敏感，最长后缀优先；`"*"` 为其他文件、`""` 为无扩展名、`"<directory>"` 用于目录打开方式。颜色同时应用于 TUI 和 GUI。创建时间缺失时显示不可用；目录大小为目录本身的元数据大小，不含子文件总量。

安装 / 移除 alias 或可选 KDE 菜单入口：

```bash
python scripts/install_alias.py          # Bash alias，已在本机完成
./scripts/install.sh                     # 可选 ~/.local/bin 命令与 KDE 菜单入口
./scripts/uninstall.sh                   # 移除命令与菜单入口
python scripts/install_alias.py --remove # 移除本项目的 alias
```

保留源码目录，移动后重新安装入口。卸载保留配置和用户文件。默认文件管理器保持原设置。
