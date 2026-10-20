# filex 1.5

## 当前范围与版本

2026-10-11：在原有文件管理功能基础上增加 q 信息面板、Ctrl+F 递归名称搜索、分支全部展开 / 收起、可配置打开程序和扩展名配色、保留树状态的上级根目录导航。随后按用户要求把默认 TUI 重构为 Rust，2026-10-12 按最新要求放弃 GUI，移除入口、源码、专用测试、截图与内存基线，产品只保留 Rust TUI。当前 TUI 的实测目标为应用 release 进程 RSS <15 MB。

## 导航与 tmux

单栏与双栏左侧：↑/↓ 与 j/k 在当前树的所有可见分支中选择相同深度的节点，可跨父目录；→ / l 展开 / 进入子级，← 返回所选节点的实际父目录。最上方 ↑ 跳到同层最下方，最下方 ↓ 回到同层最上方；PgUp/PgDn 也按该顺序循环。已展开目录的更深层后代不会被同层移动选中，未展开分支中的隐藏后代不会被自动打开；根节点所在层只有它自己时保持不动。同层扫描只遍历已有可见树，不读取新的目录、不收起分支。返回父级键直接选择父节点，不收起任何分支；在根节点使用它时，将原根节点挂到新父节点下并选中新根，旧分支的节点身份与展开状态保留。h 仅收起选中的目录。f / keys.focus_root 将选中目录（文件时为其父目录）设为根，并重新读取目录；保留该子树展开状态、清空搜索、重置视口。Rust 会释放当前根以外的缓存，之后可用返回父级键重新引入上级目录。旧 parent=u 配置自动迁移为 Left。

TUI 只保留横向树，标题为 filex。r 和 Shift+R 重命名，Ctrl+R / F5 / Ctrl+F5 刷新；旧配置 refresh=r / rename=R 自动迁移为新默认键。Ctrl+Alt+C 复制当前 file/folder 的绝对路径，不解析选中符号链接的目标、不添加换行。Rust / 旧 Python TUI 通过 wl-copy 标准输入复制到 Wayland 剪贴板；不经过 shell。传统终端的 Esc + Ctrl+C 和增强 CSI-u 编码、隔离 tmux 的组合键均有集成验证。单独 Ctrl+C 仍退出 TUI。旧 layout.orientation / keys.toggle_tree 配置仅被忽略，运行时没有对应状态或动作。

Rust TUI 打开 terminal=true 程序时，tmux.open_in_right_pane=true 会检查当前 window 中位于右侧、垂直范围有交集的 pane，优先最近者。仅复用常见 shell（bash/zsh/fish/sh/dash/ksh）的普通屏幕：shell 必须在休眠等待、占有前台进程组且没有子进程；光标行必须是没有输入草稿的常规提示符（末尾 $/#/%/>）。复制模式、程序运行中、有草稿或无法识别的自定义提示符均回退当前终端。pane 状态检查到发送之间仍有极短的竞态窗口；这是对已存在交互 shell 的复用，不创建或终止 pane。

路由通过 tmux send-keys 排队带单引号转义的字面参数，使用 env -C（文件所在目录）和解析后的绝对可执行路径，避免 shell alias / function 和文件名中的命令替换；包含换行或非 UTF-8 参数时回退直接执行。成功后聚焦目标 pane，filex 留在原 pane 并保留树状态。可用 [tmux] open_in_right_pane=false 关闭路由。

Bash alias 已升级为同名 Bash 函数；另安装 ~/.local/bin/filex，让 tmux split-window / new-window 和非交互 shell 可以调用，不依赖 alias 展开。启动脚本保持调用者当前目录；现有 ~/.config/tmux/tmux.conf 已将普通方向键传给应用，未写入用户 tmux 配置文件；运行时键盘选项按下一节临时适配并恢复。

## v1.5 创建、链接与退出目录

Ctrl+N 固定新建文件，Ctrl+Shift+N 固定新建目录；原 a/A 保留。CSI-u 保留 Shift 修饰键，Rust TUI 进入时请求增强键盘编码。Konsole 默认 Ctrl+Shift+N 的 new-window 动作通过 qdbus6 在当前宿主窗口暂时禁用；多个 filex 共享带 flock 的运行时租约，最后一个正常退出时恢复原启用状态。tmux 临时设置 extended-keys=on（原 off 时）及 extended-keys-format=csi-u，同样以租约恢复原值。只影响运行期，不写 Konsole / tmux 配置文件；强制杀进程无法立即执行清理，下一实例会清除死亡 PID。terminal.konsole_shortcuts / tmux_extended_keys 可分别关闭。

x / keys.exit_here 退出并让 Bash 进入当前 folder（文件使用父目录）。scripts/install_alias.py 用同名函数替代旧 alias；函数以 mktemp 创建回传文件，传 FILEX_CWD_FILE，应用写绝对路径加 NUL，函数用 read -d 和 builtin cd，不使用 eval，支持引号、换行、中文。Q 退出不会写回目录；独立启动没有父 shell 回传机制时，x 先释放 TUI / 宿主终端 / 输入法租约，再以 exec 在选中目录启动交互式 $SHELL -i（缺省 /bin/bash）。直接入口无法改变原父进程的目录，exit 该 shell 后回到原 shell；新 Bash 自动读取 .bashrc，此后的函数调用直接改变其目录。外部打开程序不继承回传环境变量。

节点使用 lstat.nlink：软链接显示 a------b，------b 以 colors.link_color（默认 #4B0082）加粗显示，即使选中也保持该样式；无法读取目标时保留 ↗ 标记。目标显示记录路径的末级名称，q / I 展示完整路径。两个名称共同受 max_label_width 限制，目标保留为同一行的独立选择位置，不加入缓存树，也不自动遍历目标目录。选中软链接后 → / l 显式解析目标并选中右侧名称，名称单独反色，连接线与目标保留 link_color 加粗；← 先返回源链接。x 使用解析后的目标：文件进入其父目录，目录进入自身；q / I / p / Enter / o / Ctrl+Alt+C 使用目标路径，f 将目标目录设为根。目标模式中的新建 / 重命名 / 复制 / 移动 / 删除提示先 f 进入目标目录或 ← 选回链接，避免对视觉选中目标却操作源链接。上下同层导航、搜索、刷新、换根和 Esc 清除目标选择。只在显式选择时 canonicalize，支持相对 / 绝对 / 连续软链接，断开及循环链接显示错误并保留 TUI；没有目录递归加载。目标路径在刷新前保留为选择快照，x 再检查目标是否仍存在。非目录文件 nlink>1 显示 H×N，长名称截断仍保留标记。q Dashboard / I 可滚动信息页显示软链接原目标、绝对目标及有效 / 断开状态。硬链接按设备号和 inode 流式扫描当前根内的其他名称，包含隐藏项，不跟随软链接，深度最多 128，默认 links.scan_limit=10000 / max_results=32；范围外及超过上限的名称不会全部列出，真实 nlink 仍显示。目录链接计数来自 . 与子目录 ..，不会当作文件硬链接关系。

q / I 默认显示普通文本文件前 5 行，preview.preview_lines 可设 0..1000，同时受 preview_bytes 限制；软链接、特殊文件不读取，二进制使用原预览提示。窄屏 q 只显示可见部分，I 可滚动完整信息。

新增 openers.rscript=[Rscript, {path}] 与 openers.bash=[bash, --, {path}]，terminal=true、pause_on_exit=true；Enter 选择执行，工作目录为脚本父目录。open_defaults 的 .r / .sh 可映射为 rscript / bash，目前仍保留系统默认打开，执行行为由用户选择。当前终端执行后 Enter 返回；右侧 tmux pane 留下输出和提示符。

.js / .mjs 内置颜色 #FFD700，TUI 使用。2026-10-12 配置排查：默认进程未指定 --config，读取 ~/.config/fileX/fileX.conf；用户编辑的项目 fileX.conf 只是样本。最初 .js=ffd700 缺少 # 会被拒绝；随后 #ffd700 有效但不影响个人配置。已同步两份配置，并验证大小写后缀和十六进制大小写。编辑个人配置后逗号重载，Ctrl+R / F5 仅刷新目录；可用 --config ./fileX.conf 显式选样本。

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

首次测量峰值：项目目录 3.469 MB，20000 个文件的单目录 3.809 MB，7200 个文件的多分支目录 6.410 MB。历史 v1.3 测量峰值分别为 3.998 / 3.94 / 6.275 MB。v1.4 最终 release 峰值分别为 3.781 / 3.822 / 6.324 MB，三个默认配置场景均低于 15 MB；这些是历史测量，当前 v1.5 二进制 SHA256 和操作序列以 JSON 中的记录为准。

## 信息面板宽度与目录名颜色

q 面板默认 36 列，dashboard.dashboard_width 接受 20..240。[ / ]（keys.dashboard_narrow / dashboard_widen）每次减少 / 增加 4 列，自动显示面板；至少给树保留 36 列和分隔空隙，终端不足 70 列时提示而不显示面板。面板不再限制为终端的一半。快捷键调整只影响当前进程，逗号重载恢复配置宽度；持久设置直接编辑 filex.conf。

folder_colors 按目录 basename 精确匹配，区分大小写，可配置任意合法目录名，颜色必须是 #RRGGBB，首尾空格自动忽略。隐藏目录完全适用，TOML 名称含点要加引号，如 `".git"="#c678dd"`；按 . 显示隐藏项。默认 .git=#c678dd、node_modules=#98c379、__pycache__ / pycache=#e5c07b；未匹配使用 colors.folder_color。规则只用于真正目录（包括当前根目录），不把同名普通文件当目录，不跟随软链接。名字不支持路径或通配符解释。

## 文件信息与打开方式

Rust Dashboard 使用 Linux statx 的 BTIME 和文件元数据；创建时间未提供时显示不可用，不用 ctime 冒充。修改时间为 mtime。普通文件显示字节数，目录显示自身元数据大小，符号链接显示链接自身大小，均有标识。

`[openers.NAME]` 定义 label、command 参数列表、terminal 与可选 copy_path / pause_on_exit 布尔值，`{path}` 替换完整文件路径，{filex} / {toolkit} 分别替换为本产品目录 / 相邻 toolkit 目录，普通直接打开使用参数数组；复用 tmux 空闲 shell 时按上述转义规则发送命令。普通文件 Enter 或任意节点 w 显示打开方式菜单，folder Enter 直接展开子树且保留光标；按 `[open_defaults]` 的最长扩展名后缀排序默认项；`o` 直接打开。目录菜单另有 fileX 展开 / 收起选项。TUI 的 terminal=true 程序先尝试 tmux 右侧 pane；在当前终端运行时恢复终端模式，退出后回到原树。已加入 Nano：openers.nano 的 command=["nano", "--", "{path}"]、terminal=true；Enter 菜单可选择，也可在 open_defaults 中把指定后缀映射为 nano。未安装的程序明确提示。

`[extension_colors]` 使用 #RRGGBB，大小写不敏感，支持 .tar.gz 等复合后缀。Rust TUI 默认 consistent 共用 Linux 16 色；auto 按能力发出 16 / 256 / RGB，truecolor 保留原始 RGB，可在 terminal.color_mode 中覆盖；终端自身的颜色能力会影响实际显示。非空 NO_COLOR 环境变量使 Crossterm 禁用颜色，需要彩色时 unset NO_COLOR；不修改用户环境。默认 .js / .mjs 为 #FFD700，并有 .r / .cpp / .sh / .mp3 / .mp4 颜色。默认 .py 用 VSCodium，.html / .htm 用 Firefox， .md / .markdown 使用 toolkit 渲染后在 Firefox 打开，其余使用系统默认程序；所有内置默认可覆盖，其他内置打开程序仍保留在菜单中。


Python 执行项使用 command=["python", "{path}"]、terminal=true、pause_on_exit=true。当前 TUI 终端执行结束后等待 Enter / Esc，tmux pane 留下输出与 shell 提示符。Enter 可选「Python 执行」；把 open_defaults 的 .py 改为 python 后 o 直接运行。终端程序的工作目录为选中文件所在目录（选中目录时为目录本身）；tmux 复用也应用此目录，但不修改该 pane shell 的工作目录。Python、编辑器与浏览器为外部程序，其内存不计入 TUI 的 15 MB 目标。

Markdown 默认 opener 为 markdown：copy_path=true 先复制文件绝对路径，再运行 [scripts/open_markdown.mjs](scripts/open_markdown.mjs)。适配器动态引用相邻 [toolkit](../toolkit/README.md) 的 createToolkit，给目标文件分配 loopback 随机端口，并执行 Firefox --new-tab 本地 URL。已有 toolkit 服务、页面及 4177 端口保持独立；原文件不被适配器修改。保存后渲染更新，PDF 仍需在 toolkit 页面明确导出。每次打开有独立 Node 渲染进程；有预览事件连接时保持运行，最后连接断开后无请求 30 分钟自动退出，没有页面连接时也按该超时退出。TUI 退出不关闭已打开的预览。错误、资源上限与格式支持遵循 toolkit 文档。需要 Node 22+、Firefox 和 toolkit 已安装的 npm 依赖；Mermaid / PDF 还需要 Chromium。command 中 --toolkit 后可改为自定义绝对目录，--browser 后可改为其他兼容 --new-tab 的浏览器。

## TUI 英文输入法

本机使用 Fcitx5，EnglishGuard 仅在桌面会话的真实 TUI 启动时查询当前 method 并切到 keyboard-us；没有 Fcitx5 服务或工具时继续启动。已有英文键盘时不重复切换；配置 [input_method] auto_english=false 可关闭。正常退出、Ctrl+C 退出或 Rust 错误返回时恢复进入前的 method。只调整当前 Fcitx 状态，不改 profile、系统语言或快捷键，也不应用于 --print-tree。SIGKILL / 强制终止无法执行恢复；其他输入法框架目前未接入。

## 文件操作边界

新建、复制不覆盖已有目标。重命名和同设备移动用 Linux renameat2(RENAME_NOREPLACE)。跨设备移动先复制成功，再删除源；中途失败可能留有部分副本，源删除阶段失败可能留有部分源目录。目录复制保留符号链接，不允许复制 / 移动到自身内部；特殊文件拒绝复制，复制目录最多 128 层。Rust 复制普通文件保留权限和访问 / 修改时间，不保证所有 ACL、扩展属性与目录时间元数据。

TUI 的 Delete 键与 D 均调用 gio trash，再次按 Enter 才移到回收站；Esc / Ctrl+C 取消，其他输入不执行操作，不会退回永久删除。根目录的重命名 / 移动 / 删除需先引入其父目录。本机 /tmp 等内部挂载可能拒绝回收站操作，实际错误会显示。恢复在 Dolphin 回收站完成。

## 配置区段

| 区段 | 可自定义内容 |
|---|---|
| browser | show_hidden, directories_first, sort, max_entries, max_expand_nodes, max_loaded_nodes（Rust TUI） |
| layout | column_gap, max_label_width, row_spacing, highlight_path, dual_pane（默认启动模式） |
| tmux | open_in_right_pane（Rust TUI） |
| input_method | auto_english（Fcitx5 TUI） |
| dashboard | dashboard_width：20..240 列；[ / ] 运行时调整 |
| search | search_max_results, search_scan_limit |
| preview | preview_bytes, preview_lines |
| colors | folder_color, file_color, line_color, link_color, path_color；软链接连接线和目标固定加粗，旧背景项兼容忽略 |
| extension_colors | 扩展名 → #RRGGBB；TUI 适用 |
| folder_colors | 目录名 → #RRGGBB；精确、区分大小写 |
| openers | 程序名称 → label / command / terminal / copy_path / pause_on_exit |
| open_defaults | 扩展名 → 已定义的程序名称 |
| keys | 单字符 / Ctrl+字母 / Ctrl+Shift+字母 / Ctrl+Alt+字母快捷键，parent 支持 Left；switch_pane 支持 Tab |
| links | scan_limit, max_results |
| terminal | konsole_shortcuts, tmux_extended_keys（Rust TUI） |

默认个人配置路径是 XDG_CONFIG_HOME/fileX/filex.conf；独立 [fileX.example.conf](fileX.example.conf) 给出完整默认样本。类型、范围、未知区段 / 字段、程序引用、颜色、快捷键重复会被拒绝，重载失败保留旧配置。旧配置 quit=q 且未定义 dashboard 时自动迁移为 quit=Q。Ctrl+F 与 / 固定搜索，Ctrl+L 固定路径输入；导航字符和终端控制组合保留。

## 本机配置软链接

2026-10-12：先将项目 filex.conf 与 fileX.conf 同时软链接到主配置，随后按要求合并，只保留 filex.conf。主配置已原样重命名为 /home/a/.config/fileX/filex.conf，项目 filex.conf 软链接至它，旧 fileX.conf 已移除。原项目样本保留为 fileX.example.conf；Rust include_bytes 与历史 Python --init-config 都引用独立样本，构建不读取个人配置。编辑软链接后保存，运行中的 TUI 用逗号重载，后续启动直接读取主配置。主配置内容及权限保留，未生成第二份可编辑配置。已用隔离 XDG_CONFIG_HOME 验证 Rust / Python 默认启动优先读取小写配置、只存在旧路径时保留原设置、--init-config 创建小写文件并拒绝覆盖。默认路径统一为 fileX/filex.conf；其他旧安装仅有 fileX.conf 时兼容读取旧路径，两个路径都有时优先小写文件，不创建额外别名。

这个绝对软链接用于本机；换用户 / 移动环境 / 更改 XDG_CONFIG_HOME 时需要重新指向相应主配置。显式 --config PATH 仍读取指定文件。若编辑器使用原子保存，请启用保留软链接的保存方式，避免把链接替换为普通文件；可用 readlink -f filex.conf 检查目标。此前项目样本与个人配置独立的描述属于历史行为，已由本节替代。

2026-10-12：增加软链接的 a------b 关系显示，连接线与目标名称为靛蓝色加粗，源名称保留其扩展名颜色。硬链接没有唯一的源 / 目标方向，继续使用 H×N 与 q / I 中的同 inode 路径列表。读取目标只调用 read_link，不增加节点或扫描链接目标；默认视图上限和同级选择规则保持原有行为。

Ctrl+F5 固定刷新目录，与 F5 / 默认 Ctrl+R 相同；接受终端 CSI 15;5~ 编码，真实 PTY 与隔离 tmux C-F5 用外部新增文件验证刷新。keys.refresh 也可配置为 Ctrl+F5，该组合保留给刷新。

## 文件与维护

- `Cargo.toml` / `Cargo.lock`：Rust 依赖与锁定版本，release 使用 LTO / stripped binary。
- `src/config.rs`、`model.rs`、`files.rs`、`tui.rs`、`main.rs`、`tmux.rs`、`input.rs`、`links.rs`、`terminal_host.rs`、`terminal_profile.rs`、`tests.rs`：配置、有限缓存树与布局、文件操作 / statx / 打开方式、终端界面、CLI、tmux 右侧 pane 路由、英文输入法恢复、链接扫描、宿主终端快捷键租约、Rust 测试。
- `filex/`：保留的 Python 历史 TUI 与对应模型 / 配置 / 操作，`details.py` 和 `opening.py` 提供元数据与可配置启动器。原 Python TUI 为历史兼容实现，默认启动器已使用 Rust。
- `filex.sh`：保留调用者工作目录，exec Rust release 程序。缺少 release 时自动编译。
- `scripts/build.sh`、`measure_memory.py`：更新 release 程序、验证应用内存。
- `scripts/install_alias.py`：只维护 .bashrc 中本项目的标记区块，拒绝覆盖无关 alias / function；本机已安装 Bash 函数。新交互 Bash 自动生效；已有 shell 直接使用命令链接，x 会进入选中目录的交互 shell，首次切换也不必手动 source。
- `scripts/install.sh` / `uninstall.sh`：只安装终端命令，清理本项目旧图形菜单入口；--command-only 为兼容参数，Bash 函数单独安装 / 移除。
- `tests/`：Python 核心回归、Rust CLI 与真实 PTY / 隔离 tmux 的集成检查。

```bash
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
./scripts/build.sh
python -m unittest discover -s tests -v
python scripts/measure_memory.py
```

## 验证

2026-10-12：Rust 25 项单元检查、Python 44 项核心 / CLI / 真实 PTY / tmux 集成检查（合计 69 项）全部通过；Clippy 无 warning，release 已构建。验证新增跨父目录同深度导航、首尾循环、q 面板 [ / ] 实际调整宽度、目录名颜色 RGB 输出和合法 / 非法配置；覆盖 Ctrl+N / Ctrl+Shift+N、真实 tmux CSI-u 与退出目录、链接标记与有界扫描、q / I 文件开头配置、Python / Rscript / Bash 真实执行、默认颜色与非法配置处理、GUI 入口拒绝和旧菜单安装清理。真实 PTY 另外验证软链接连接线和目标输出 RGB(75,0,130) 加粗，选中链接时保持该样式；.js / .MJS 输出 RGB(255,215,0)，逗号重载改变 RGB，非法值保留先前颜色。测试就绪条件由固定延时改为等待 TUI 页脚，避免启动期间输入被终端提前转换。

Konsole 当前窗口 new-window 动作停用已在本机独立终端实测；最后一个实例退出恢复原启用 / 禁用状态由隔离 DBus CLI 加两个真实 TUI 实例验证。本机 Konsole 禁用 sendText 敏感 DBus API，因此未用该接口验证真实窗口中的 Q 退出；没有更改这项设置。tmux 恢复原 extended-keys / format 已在隔离真实 server 完成验证。

2026-10-15 当前 release 的项目目录 / 20000 文件 / 7200 多分支文件 / 2400 硬链接 / 7200 软链接场景峰值分别为 5.022 / 4.121 / 6.939 / 4.018 / 7.078 MB，Konsole 模式双栏场景为 8.237 MB；TTY 7200 文件单 / 双栏分别为 6.930 / 8.114 MB。八个场景均低于 15 MB。consistent 模式两边共用 16 色与同色相亮暗档；外部解释器与浏览器不计入应用进程。详情和二进制 SHA256 见 [memory-validation.json](memory-validation.json)。源配置与本机个人配置都通过 Rust / Python 解析验证；filex Bash 函数及命令链接已安装。GUI 已移除，不再设置 GUI 内存目标。

## 2026-10-12 运行修复与祖先路径高亮

`layout.highlight_path=true` 默认启用祖先路线：光标到根目录之间的实际母子连接使用 `colors.path_color`（默认 #9cc5d1），祖先节点保持各自名称 / 扩展名颜色，其他连接及节点使用终端 Dim 属性。选中节点仍反色，软链接目标仍按 link_color 加粗；根目录标题和 Dashboard 保持可读。设为 false 关闭路线区分。渲染只计算当前可见树及路线，没有额外目录扫描；路径单元检查验证不包含兄弟分支，并保持连接交叉的原有字形。

本机真实 wl-copy 会后台化，剪贴板拥有者继承 stderr；原 wait_with_output / Python capture_output 在父进程成功退出后仍等错误管道 EOF，直至剪贴板被替换。Markdown 的 copy_path=true 因而在启动渲染器前卡住。现在只等待 wl-copy 父进程（最多 3 秒），失败时非阻塞读取有限错误信息，成功后直接关闭启动管道。集成测试使用持有 stderr 的后台剪贴板模拟进程复现该条件。

内置 Markdown 适配器等待服务器及浏览器启动后输出 URL，Rust 非阻塞收集最多 16 KiB 启动信息并等待最多 10 秒。成功显示就绪 URL；启动失败或超时显示具体错误，仍可继续导航 / 创建文件。Node 处理启动管道关闭后的 EPIPE，已打开页面独立存活。真实 toolkit / Chromium 检查覆盖渲染、保存更新、独立页面及 Firefox 参数；缺失 toolkit 的失败和后续文件操作也已验证。本机另以真实 wl-copy、Firefox 和相邻 toolkit 完成 native TUI 启动、精确路径复制、HTML 渲染以及预览启动后的文件新建检查；没有用浏览器自动化验证 Firefox 内的视觉效果。

TUI 请求焦点事件；tmux focus-events 通过单独的 flock / PID 租约临时启用，最后一个正常退出实例恢复原值。获得焦点时重新应用 termios raw、恢复输出流、以替换模式恢复增强键盘参数，并按配置重新切换 Fcitx 英文；不重复压入键盘状态栈，提示框已有文字保留。参考 [Kitty 键盘协议](https://sw.kovidgoyal.net/kitty/keyboard-protocol/)。隔离真实 tmux 客户端反复切换窗口后验证导航、新建及退出目录；真实 PTY 另验证提示框焦点往返、组合键和原有操作。不写用户 tmux 配置。

原行为记录：此前 README 要求已有终端 source ~/.bashrc，并且独立入口 x 提示不能改变父 shell、保持 TUI；Delete 要求输入 YES。以上行为已由本节和当前 README 替代。个人配置自定义的 .git / .github 颜色继续使用；本次发现 pycache 颜色的开头空格后统一容忍颜色首尾空格，不改写个人颜色。

2026-10-12：增加软链接右侧目标的独立选择。Rust 检查相对 / 绝对 / 连续 / 目录 / 断开 / 循环解析；真实 PTY 检查目标名称反色、x 的 NUL 路径回传、← 返回源链接及同层移动清除目标选择，目标位于原根目录外且包含中文和引号。

## 中轴镜像双目录树与跨栏文件操作

v / keys.toggle_dual 开关同一界面双栏，Tab / Shift+Tab / keys.switch_pane 切换操作侧；不是新 tmux pane。左树正常向右展开，右树沿树区域中轴镜像向左展开，文字不反转，连接角及分支字形镜像，折叠目录图标指向左。两侧分别维护 Browser、根目录、光标、展开分支、搜索与滚动位置、链接目标选择；活动侧用 ● 标识，只有活动侧显示反色光标。新开的另一侧以当前选中目录或文件父目录为根；关闭双栏释放非活动侧，当前侧恢复单栏正常方向。

右侧物理 ← 映射为展开 / 进入，→ 映射为实际父级；左右的同级 ↑↓ 循环规则相同。右侧软链接在 ← 时进入行内目标、→ 返回源链接，x 仍进入目标文件父目录（目录进入自身）；h 收起、l 逻辑展开仍维持原绑定，H/L 横移方向在右侧反转。f / g / 搜索 / 新建等只操作活动侧；q / I / 预览 / 打开程序也读取活动侧。双栏最少 75×8，缩窄时仍能用双栏开关键恢复单栏或退出。q 双栏最少 110 列并给两树各保留至少 36 列，剩余树区域保持镜像中轴；[ / ] 宽度上限随可用空间收紧。

双栏 c/m 预填另一侧所选目录加源文件名；另一侧选中文件时取父目录，行内链接目标选择使用解析后的目标路径。完整目标路径可编辑，Enter 执行、Esc 取消。继续调用已有 files.transfer，不经过 shell、不覆盖同名项、拒绝目录复制到自身内部；目录递归复制，符号链接复制链接本身，跨文件系统移动走已有复制后删除源流程。失败保留既有错误处理。成功刷新两侧，目标位于已展开目录时强制保留新条目显示，目的侧光标和其他展开状态保留。根目录移动限制保持，先返回其父级再选目录操作。

配置 `layout.dual_pane=false` 是启动默认模式，逗号重载颜色 / 热键等不自动开关现有双栏；运行时开关由 v 控制。`keys.toggle_dual="v"`、`switch_pane="Tab"` 可改为其他允许绑定；Tab 仅用于切换栏，Shift+Tab 同样切换。已有配置把 v 分配给其他动作且未定义 toggle_dual 时，双栏默认键自动让位到 V / Ctrl+Alt+v。旧 toggle_tree / orientation 继续忽略，v 的新含义是双栏，不恢复垂直视图。Python 历史配置解析接受这些字段，双栏实现只在 Rust TUI。

缓存方案：每侧沿用 browser.max_loaded_nodes（默认 6000），双栏总缓存节点至多约两倍单栏上限，两侧布局依次生成后释放。这样一侧的展开不会挤掉另一侧树或使其不能继续浏览；关闭双栏释放另一侧缓存。两个 Browser 在同一个 Rust 进程，终端 / runtime / 代码页共享；仍以该进程的 VmHWM 实测 <15 MB，不用节点数量推断 RSS。六个场景的当前数据、操作序列及二进制 SHA256 见 memory-validation.json；增大缓存、标签、搜索上限仍可能超过目标。

验证覆盖普通 / 镜像文本及中文位置、连接字形翻转、视口边界裁切、独立光标和缓存上限；真实 PTY 覆盖文件 / 目录 / 软链接复制、移动后源消失、目标重名保护、取消、目的侧光标和展开状态、右侧父子导航、配置启动双栏、镜像链接目标 x、可自定义开关 / 切换键及小终端恢复。隔离真实 tmux 额外验证 v 开关及 Tab 往返；未操作用户活跃 tmux 会话。

## 参考

- [Crossterm 事件](https://docs.rs/crossterm/latest/crossterm/event/index.html)
- [Linux statx](https://man7.org/linux/man-pages/man2/statx.2.html)
- [Konsole 26.08.2 默认键盘协议](https://github.com/KDE/konsole/blob/v26.08.2/src/profile/Profile.cpp)
- [Konsole 26.08.2 new-window 快捷键](https://github.com/KDE/konsole/blob/v26.08.2/src/MainWindow.cpp)


## v1.4 验证存档

以下为 GUI 移除前的历史记录，旧 GUI 命令、依赖和目标已撤销，相关源码与图片已移除。

### 历史验证

v1.4 验证涵盖横向同级导航及分支保留、两种重命名键、Ctrl+R / F5、Ctrl+Alt+C 的传统 / CSI-u / tmux 编码、GUI 剪贴板、Python 真实执行与工作目录、Fcitx 英文切换和恢复、Delete 回收站取消 / 确认。Markdown 集成使用相邻真实 toolkit 与真实 Chromium 页面，验证路径加载、HTML 渲染、保存更新、独立预览、Firefox --new-tab 参数和服务空闲退出；另验证 Rust TUI 自动复制路径和启动适配器。Rust 15 项单元检查与 Python 34 项集成 / GUI / CLI 检查（合计 49 项）全部通过，Clippy 无 warning。最终 release 已构建，filex alias 与命令链接均指向当前启动器；本机个人配置与源配置均通过两种实现验证。最终应用进程峰值为 3.781 / 3.822 / 6.324 MB。实际 Wayland 横向 GUI 截图为 `preview.png`（已移除）。

GUI 当前仍使用 Python/PySide6，<50 MB 尚未完成；GUI 旧实测基线（已移除） 单独记录一个实际 Wayland 窗口的内存。后续需评估 Rust GUI 与原生 Wayland 渲染方案，并在选择库后实测，不能只凭语言迁移保证达标。

## v1.0 文档存档

以下保存第一版 README 与详细说明，供历史追溯；旧的 q 退出、仅搜索可见节点、Python 为 TUI 前置条件等描述已由上述 v1.2 行为取代。

### What is it?

fileX 1.0 是面向 Arch Linux / KDE Wayland 的横向树文件管理器。母子目录和文件由线连接，可同时展开多个分支。默认启动 TUI，也提供原生 Qt GUI；支持浏览、搜索、预览、新建、重命名、复制、移动和回收站删除。

设计、限制、源码说明和验证记录见 [description.md](description.md)。GUI 预览（已移除）。

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

默认路径 `${XDG_CONFIG_HOME:-~/.config}/fileX/fileX.conf`，可用 `--config PATH` 替换。`--init-config` 只在目标不存在时写入样本。源码中的 `fileX.conf`（已合并为 filex.conf） 是完整可编辑样本。

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
- `tests/wayland_smoke.py`：本机 Wayland 窗口验证与 `preview.png`（已移除） 生成。

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

配置、架构、内存测量、限制和历史记录见 [description.md](description.md)。GUI 预览（已移除）。

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

个人配置位于 `~/.config/fileX/fileX.conf`（遵循 XDG_CONFIG_HOME），本机已创建。源码 `fileX.conf`（已合并为 filex.conf） 是完整样本；用 `--config PATH` 可选择其他配置，缺少个人配置时 `filex --init-config` 创建且不覆盖已有文件。修改后按 `,` 或 GUI「读取配置」。

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


## v1.3 文档存档

以下保留变更前的功能、实现与验证记录；垂直视图及其截图已按用户要求移除。当前操作与配置以上方 v1.5 及 README 为准。

### v1.3 详细说明

#### 当前范围与版本

2026-10-11：在原有文件管理功能基础上增加 q 信息面板、Ctrl+F 递归名称搜索、分支全部展开 / 收起、可配置打开程序和扩展名配色、保留树状态的上级根目录导航。随后按用户要求把默认 TUI 重构为 Rust，保留现有 Python/PySide6 GUI；GUI 的 <50 MB 目标留待单独优化。当前 TUI 的实测目标为应用 release 进程 RSS <15 MB。

#### 导航、布局与 tmux

方向键随布局旋转：横向 ↑/↓ 选择同级、→ 展开 / 进入子级、← 返回父级；纵向 ←/→ 选择同级、↓ 展开 / 进入子级、↑ 返回父级。j/k 仍是同级前后选择，l 仍是展开 / 进入。同级选择只在 parent.children 中移动，越界时停在同级首末项；根节点无兄弟时不移动，已展开目录也不会让同级选择进入后代。返回父级键直接选择父节点，不收起任何分支；在根节点使用它时，将原根节点挂到新父节点下并选中新根，旧分支的节点身份与展开状态保留。h 仅收起选中的目录。f / keys.focus_root 将选中目录（文件时为其父目录）设为根，并重新读取目录；保留该子树展开状态、清空搜索、重置视口。Rust 会释放当前根以外的缓存，之后可用返回父级键重新引入上级目录。旧 parent=u 配置自动迁移为 Left；该配置名称沿用横向基准，纵向的固定父级方向键为 Up。

v / keys.toggle_tree 切换横向与纵向布局，配置 layout.orientation 可设默认 horizontal / vertical；临时切换不自动写回配置。两种界面共用同一节点树，纵向布局为母节点在上、子节点在下并排的树图。自底向上计算各子树宽度，给不同分支预留独立区域；各层间留连接线行，同级子节点左右排列，母节点尽量居于首末子节点的中间，长标签受所在子树范围约束。GUI 提供「纵向树」按钮。TUI 标题与 GUI 窗口标题均为 filex。

Rust TUI 打开 terminal=true 程序时，tmux.open_in_right_pane=true 会检查当前 window 中位于右侧、垂直范围有交集的 pane，优先最近者。仅复用常见 shell（bash/zsh/fish/sh/dash/ksh）的普通屏幕：shell 必须在休眠等待、占有前台进程组且没有子进程；光标行必须是没有输入草稿的常规提示符（末尾 $/#/%/>）。复制模式、程序运行中、有草稿或无法识别的自定义提示符均回退当前终端。pane 状态检查到发送之间仍有极短的竞态窗口；这是对已存在交互 shell 的复用，不创建或终止 pane。

路由通过 tmux send-keys 排队带单引号转义的字面参数，使用 env 和解析后的绝对可执行路径，避免 shell alias / function 和文件名中的命令替换；包含换行或非 UTF-8 参数时回退直接执行。成功后聚焦目标 pane，filex 留在原 pane 并保留树状态。GUI 的终端程序继续使用 Konsole。可用 [tmux] open_in_right_pane=false 关闭路由。

Bash alias 保留；另安装 ~/.local/bin/filex，让 tmux split-window / new-window 和非交互 shell 可以调用，不依赖 alias 展开。启动脚本保持调用者当前目录；现有 ~/.config/tmux/tmux.conf 已将普通方向键传给应用，未改动用户 tmux 配置或活跃会话。

#### Rust TUI 与内存设计

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

#### 文件信息与打开方式

Rust Dashboard 使用 Linux statx 的 BTIME 和文件元数据；创建时间未提供时显示不可用，不用 ctime 冒充。GUI 使用现有元数据功能，调用 GNU stat 获取真正 birth time。修改时间为 mtime。普通文件显示字节数，目录显示自身元数据大小，符号链接显示链接自身大小，均有标识。

`[openers.NAME]` 定义 label、command 参数列表和 terminal 布尔值，`{path}` 替换完整文件路径，普通直接打开使用参数数组；复用 tmux 空闲 shell 时按上述转义规则发送命令。Enter 显示打开方式菜单，按 `[open_defaults]` 的最长扩展名后缀排序默认项；`o` 直接打开。目录菜单另有 fileX 展开 / 收起选项。TUI 的 terminal=true 程序先尝试 tmux 右侧 pane；在当前终端运行时恢复终端模式，退出后回到原树；GUI 使用 Konsole。已加入 Nano：openers.nano 的 command=["nano", "--", "{path}"]、terminal=true；Enter 菜单可选择，也可在 open_defaults 中把指定后缀映射为 nano。未安装的程序明确提示。

`[extension_colors]` 使用 #RRGGBB，大小写不敏感，支持 .tar.gz 等复合后缀。Rust TUI 默认 consistent 共用 Linux 16 色；auto 按能力发出 16 / 256 / RGB，truecolor 保留原始 RGB，可在 terminal.color_mode 中覆盖；终端自身的颜色能力会影响实际显示。GUI 使用相同颜色。默认 .py 用 VSCodium，.html / .htm 用 Firefox，其余使用系统默认程序；所有内置默认可覆盖，其他内置打开程序仍保留在菜单中。

#### 文件操作边界

新建、复制不覆盖已有目标。重命名和同设备移动用 Linux renameat2(RENAME_NOREPLACE)。跨设备移动先复制成功，再删除源；中途失败可能留有部分副本，源删除阶段失败可能留有部分源目录。目录复制保留符号链接，不允许复制 / 移动到自身内部；特殊文件拒绝复制，复制目录最多 128 层。Rust 复制普通文件保留权限和访问 / 修改时间，不保证所有 ACL、扩展属性与目录时间元数据。

TUI 的 Delete 键与 D 均调用 gio trash，输入 YES 才移到回收站；Esc 或其他输入取消，不会退回永久删除。根目录的重命名 / 移动 / 删除需先引入其父目录。本机 /tmp 等内部挂载可能拒绝回收站操作，实际错误会显示。恢复在 Dolphin 回收站完成。

#### 配置区段

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

默认个人配置路径是 XDG_CONFIG_HOME/fileX/fileX.conf；源 `fileX.conf`（已合并为 filex.conf） 给出完整样本。类型、范围、未知区段 / 字段、程序引用、颜色、快捷键重复会被拒绝，重载失败保留旧配置。旧配置 quit=q 且未定义 dashboard 时自动迁移为 quit=Q。Ctrl+F 与 / 固定搜索，Ctrl+L 固定路径输入；导航字符和终端控制组合保留。

#### 文件与维护

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

#### 验证与后续 GUI 目标

Rust 14 项单元检查与 31 项集成 / GUI 回归检查通过。实际 Nano 通过真实 PTY 打开 / Ctrl+X 返回，f 重新设根后进入文件的完整流程也通过。真实 Rust PTY 检查涵盖文件操作、q Dashboard、Ctrl+F 找到未展开目录中的文件、打开方式选择、当前终端运行编辑器并返回、向上引入根目录、Delete 取消 / 确认 / 实际回收站内容恢复证据和缩小终端后的操作。Rust Clippy 无 warning，release 已构建并接入 filex alias。隔离 tmux server 实测覆盖 alias 当前目录、同级方向键、横纵切换、右侧空闲路由、繁忙回退、输入草稿保留、含引号 / shell 符号的文件名和 resize。实际 Wayland GUI 两种布局已验证，截图为 `preview.png`（已移除） 与 `preview-vertical.png`（此旧布局截图随 v1.4 移除）。

GUI 当前仍使用 Python/PySide6，<50 MB 尚未完成；GUI 旧实测基线（已移除） 单独记录一个实际 Wayland 窗口的内存。后续需评估 Rust GUI 与原生 Wayland 渲染方案，并在选择库后实测，不能只凭语言迁移保证达标。

#### 参考

- [Crossterm 事件](https://docs.rs/crossterm/latest/crossterm/event/index.html)
- [Linux statx](https://man7.org/linux/man-pages/man2/statx.2.html)
- [Qt QGraphicsView](https://doc.qt.io/qtforpython-6/PySide6/QtWidgets/QGraphicsView.html)


### v1.3 README

#### What is it?

filex 1.3 是面向 Arch Linux / KDE Wayland 的横向 / 纵向树文件管理器。默认 TUI 已用 Rust 重构，母子目录和文件由线连接，支持多分支展开、搜索定位、文件信息和常用文件操作。现有 Qt GUI 保留，通过 `--gui` 启动。

配置、架构、内存测量、限制和历史记录见 [description.md](description.md)。GUI 预览（已移除）。

#### How to use

本机已经编译好 TUI，并安装 `filex` Bash alias 和 `~/.local/bin/filex` 命令，可在 tmux 内使用。已有终端先执行一次：

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

启动脚本在缺少 release 程序时自动编译。VSCodium、Vim、Neovim、Nano、Firefox 是可选打开程序，使用前需已安装。

| 默认键 | 操作 |
|---|---|
| 横向 `↑/↓`，纵向 `←/→`；或 `k/j` | 只选择同级兄弟节点，跳过展开目录的后代 |
| 横向 `→`，纵向 `↓`；或 `l` | 展开选中目录；已展开时进入其第一个子节点 |
| 横向 `←`，纵向 `↑` | 返回父节点；在根节点时引入上级根目录并选中它，保留原树展开状态 |
| `h` | 只收起选中目录，不移动到父节点 |
| `v` | 横向 / 纵向树切换：纵向为母节点在上、子节点在下并排；保留树状态 |
| `f` | 当前 folder 设为根并刷新；光标在文件上时使用其所在 folder，保留新根内的展开状态 |
| `q` | 切换右侧 Dashboard：创建时间、修改时间、大小；随选中节点更新 |
| `Ctrl+F` 或 `/` | 递归搜寻根目录下文件 / 目录名，含未展开目录；Enter 定位 |
| `n`，`N` | 下一个 / 上一个搜索结果，保留其他展开分支 |
| `.` | 显示 / 隐藏隐藏文件和目录 |
| `E`，`C` | 展开 / 收起选中目录下的全部分支（受配置上限约束） |
| `Enter` 或 `w` | 选择打开方式：系统默认、VSCodium、Vim、Neovim、Nano、Firefox 或配置中的程序 |
| `o` | 直接使用该扩展名的默认打开方式 |
| `g` / `Ctrl+L` | 输入其他根目录；显式更换根目录时 Rust TUI 重建树缓存 |
| `p` | 文本预览，Esc 返回 |
| `a`，`A` | 新建空文件 / 目录，位置为选中目录或文件的父目录 |
| `R`，`c`，`m` | 重命名、复制、移动 |
| `Delete` 或 `D` | 移到回收站；输入 `YES` 确认，可在 Dolphin 回收站恢复 |
| `r`，`,` | 刷新目录、重载配置 |
| `?`，`Q` | 帮助、退出（小写 q 用于 Dashboard） |

`H/L` 横移，PgUp/PgDn 同级翻页，Home/End 选择首末节点；输入时 Enter 确认、Esc 取消、Ctrl+U 清空。复制 / 移动填写**完整目标路径，包含目标名称**；相对路径以源项目的父目录为基准，不覆盖已有目标，目标父目录必须存在。文件树不自动遍历符号链接。

Enter 选中目录时还可选择在 fileX 展开 / 收起。TUI 的终端编辑器（如 Vim/Neovim/Nano）优先复用同一 tmux window 内右侧空闲 pane，并切换焦点过去；没有合适 pane、pane 繁忙或已有未提交命令时，在当前终端运行，退出后返回原树。GUI 在 Konsole 打开终端编辑器。GUI 单击选择，双击目录展开 / 折叠，双击文件选择打开方式；拖动空白处平移，Ctrl+滚轮缩放，工具栏和右键菜单提供操作。GUI 主程序位于 `filex/gui.py`（已移除），通过 `filex --gui` 启动。画布焦点下快捷键与 TUI 相同，F5 刷新、F2 重命名、Delete 回收站。

个人配置位于 `~/.config/fileX/fileX.conf`（遵循 XDG_CONFIG_HOME），本机已创建。源码 `fileX.conf`（已合并为 filex.conf） 是完整样本；用 `--config PATH` 可选择其他配置，缺少个人配置时 `filex --init-config` 创建且不覆盖已有文件。修改后按 `,` 或 GUI「读取配置」。

配置可以定制：隐藏文件、排序、目录显示 / 缓存 / 展开上限、搜索上限、默认横向 / 纵向布局、节点间距、标签宽度、行距、Dashboard 宽度、预览大小、GUI 字号 / 主题、单字符或 Ctrl+字母快捷键、打开程序和参数、扩展名默认打开方式、扩展名颜色，以及 tmux 右侧打开开关。例如：

```toml
[layout]
orientation = "horizontal" # 或 "vertical"；v 临时切换

[tmux]
open_in_right_pane = true

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
./scripts/install.sh --command-only      # 只安装命令，已在本机完成
./scripts/install.sh                     # 可选命令与 KDE 菜单入口
./scripts/uninstall.sh                   # 移除命令与菜单入口
python scripts/install_alias.py --remove # 移除本项目的 alias
```

tmux 当前 pane 中直接运行 `filex`，根目录为该 pane 的当前目录；也可运行 `tmux split-window -h -c "#{pane_current_path}" "$HOME/.local/bin/filex"` 在新右侧 pane 启动。现有 tmux 方向键配置无需更改。

保留源码目录，移动后重新安装入口。卸载保留配置和用户文件。默认文件管理器保持原设置。



## 2026-10-13：纯 CLI、SSH 与 Linux TTY 适配

新增 `src/terminal_profile.rs`，启动时一次性选择终端能力，不发送能力查询等待终端回复。通过 TERM=linux 与 KDGETMODE ioctl 检测原生控制台；`--tty` / terminal.mode="tty" 强制控制台模式，即使继承 DISPLAY / WAYLAND_DISPLAY 也不启用桌面打开、Fcitx、Konsole。SSH_CONNECTION / SSH_TTY 或缺少桌面显示变量时采用 CLI 打开方式。tmux 下没有桌面时同样走 CLI 路径，右侧空闲 pane 路由保留，不改变其 extended-keys / focus-events 设置。

控制台不发送 Kitty CSI-u、modifyOtherKeys、焦点报告、1049 alternate screen 控制；仅使用基础光标 / 清屏 / 属性指令，退出清理屏幕并恢复 termios。其他 CLI 终端可用备用屏幕，扩展键协议只在桌面现代终端启用。`terminal.color_mode=auto`：原生 / 强制控制台 16 色；CLI 按 COLORTERM 和 TERM 选择真彩色 / 256 色 / 16 色；现代 KDE 桌面保留 RGB。16 / 256 模式将配置中的 RGB 映射到最近的标准调色板颜色，实际色值还受用户终端调色板影响。NO_COLOR 非空始终优先关闭颜色。参照 [Linux console_codes(4)](https://man7.org/linux/man-pages/man4/console_codes.4.html) 和 [Crossterm 0.29 event API](https://docs.rs/crossterm/0.29.0/crossterm/event/index.html)。

`terminal.charset=auto`：控制台 / 无桌面默认 ASCII 装饰与英文主要交互提示；桌面沿用 Unicode 线条和中文。ascii / unicode 或 `--ascii` 可覆盖。只改变连线、折叠图标和提示，不改文件名 / 文件内容；原生控制台字体仍限制中文文件名显示。不改变系统字体、键盘映射或登录会话。传统控制台不区分所有组合键，a/A 是文件 / 目录创建的可用替代；Linux F5 的 ESC[[E、Ctrl+R 刷新和普通方向键均有真实 PTY 检查。

CLI 的桌面默认 opener 自动退到 terminal.cli_opener，auto 按 nano/nvim/vim 中已定义为 terminal=true 且可执行的程序选择；没有编辑器则用有界内置预览。用户显式配置的终端脚本方法保留，Python / Rscript / Bash 不被作为自动编辑器候选。Enter 菜单过滤 desktop opener，目录仍可展开 / 收起；显式终端程序可以打开目录。terminal=true、copy_path=true 的自定义程序在 CLI 不调用 wl-copy。工具存在检查改为普通文件且有执行权限。GUI Markdown 集成保持原有行为，CLI 中使用文本编辑 / 预览。

复制绝对路径增加 portable `y` 后备（未被配置占用时），Ctrl+Alt+C 保留；路径始终存入当前 Ui 的共享内部缓冲，可跨双栏在输入框 Ctrl+V 粘贴，Ctrl+U 清空已有输入。桌面同时写 Wayland 剪贴板；CLI 不要求系统剪贴板，不向 SSH 客户端发送 OSC52。软链接目标选中时复制目标路径，源链接选中时复制源名称。内部缓冲在进程退出时释放。非 UTF-8 名称在交互输入中仍受原有 String / lossy 显示限制；不把该缓冲宣称为系统剪贴板。

stdin / stdout 非交互、TERM=dumb 自动输出当前目录的一层静态树；--print-tree 仍可显式使用，--ascii 适合脚本。遇到关闭的管道（例如 head 提前退出）正常结束，不出现 Broken pipe panic。静态模式不进入 raw、不获取桌面或 tmux 租约、不启动外部打开程序。TUI 的 SIGTERM / SIGHUP / SIGINT 标记由主循环以 200ms poll 响应并正常释放 Guard，已有提示框也可取消；正在等待外部终端程序时清理须待该程序返回，SIGKILL 无法恢复。基本 Ctrl+C / Q 退出不影响已有目录切换语义。

`[terminal]` 新增 mode=auto/tty、color_mode=auto/16/256/truecolor/none、charset=auto/ascii/unicode、cli_opener=auto/terminal opener 名称；严格校验，个人配置只添加这些缺失项并保留颜色 / 打开规则。模式 / 色彩 / 字符集是启动能力，改后重启；cli_opener 可通过逗号重载。默认模板与 Python 历史配置解析器同步字段；新终端渲染行为只由 Rust 默认 TUI 实现。

新增 tests/test_tty.py 覆盖管道 / dumb / 断管、TTY 16 色与协议排除、a/A / Ctrl+N、重命名、内部路径粘贴、Linux F5、终端 opener 工作目录与返回后新建、无编辑器预览、Bash 执行和等待返回、双栏复制 / 移动 / 镜像导航 / x 回传、删除 Enter 确认与 Esc 取消 / gio 真实回收站、无桌面 tmux 右侧空闲 pane、SIGTERM / SIGHUP termios 恢复、NO_COLOR、配置合法 / 非法值。实际虚拟控制台登录会话未被切换；PTY 验证覆盖 TERM=linux 的输入 / 输出协议，不等同于真实显示硬件 / console 字体视觉验证。

2026-10-13 验证结果：Rust 26 项 + Python 54 项（其中新 CLI / TTY 10 项）全部通过，Clippy -D warnings 通过，release 已更新。内存八场景最大 8.176 MB（TTY 双栏 8.151 MB），均 <15 MB；报告 SHA256 与当前 release 一致。主配置、默认模板均通过 Rust / Python 解析；README 保持 What is it? / How to use 两节。


## 2026-10-13：Enter 直接展开文件夹

用户确认目标是 folder 的子目录／文件树。Rust 主树中 Enter 直接展开选中的普通目录，不出现打开方式菜单，不改变选中节点；已展开时保持原树和光标，不自动进入子项，也不收起其他分支。子级选择仍用左右箭头，右侧镜像树方向保持向左。w / keys.open_with 显式选择打开方式，o 用默认程序；普通文件 Enter 仍显示打开方式菜单。菜单、创建 / 复制等输入框中的 Enter 确认语义保持。符号链接仍遵循显式目标选择与不自动遍历的边界。Python 历史 TUI 的普通目录 Enter 同步行为。

新增 Rust 行为检查覆盖重复 Enter 的光标 / 缓存保留、两个同层目录同时展开、镜像右侧展开且另一侧分支保留；真实 TTY PTY 检查 Enter 后创建文件确实落在选中的 folder、上下跳过展开内容切换同层、w 菜单仍能打开 / 取消。

Enter 行为更新后的验证：Rust 27 项 + Python 55 项全部通过（合计 82 项），Clippy 无 warning，release 已重建。八场景内存重测最大 8.188 MB，报告 SHA256 对应当前二进制。


## 2026-10-14：TTY 登录自动启动与一致的 16 色

本机原来没有 ~/.bash_profile、~/.bash_login、~/.profile；filex 函数和 ~/.local/bin 的 PATH 仅在 ~/.bashrc，因此原生 TTY 的 Bash 登录需要手动 source。scripts/install_alias.py 现在按 Bash 登录优先级维护第一个已有配置；全部不存在则创建 .bash_profile。管理区块为登录环境追加 ~/.local/bin，并仅在交互 Bash 且 filex 函数尚未加载时 source 所选 bashrc；已有配置提前加载 bashrc 时不重复执行。POSIX 外层保护使 .profile 被其他 shell 使用时不会调用 Bash 的 declare。保留其他配置、权限和软链接，写入使用临时文件 / 原子替换。

默认 scripts/install.sh 同时维护命令、函数和登录入口；--command-only 或 --prefix 只维护命令。uninstall.sh 对称移除本项目区块；独立 install_alias.py --remove 同样清理登录区块，只删除由本安装器新建且移除后为空的登录文件。--rc 自定义时不改真实 home 的登录文件，--profile 可显式指定配套登录配置。实际主机已运行安装器，并确认新 login shell 可解析命令。新登录无需 source；已有 shell 的环境不能由子进程修改，需要重新登录或 exec bash -l 一次。

用户选择 TTY 与 Konsole 共用 16 色。新增 terminal.color_mode="consistent"，作为 Rust / Python 配置默认和样本默认，个人配置仅改变这一项，原始扩展名 / 目录颜色保持。shared_palette 读取 /sys/module/vt/parameters/default_red、default_grn、default_blu（各 16 项、严格 u8 校验）；缺失时使用 Linux 标准默认色表。TTY 输出 ANSI 颜色槽，Konsole 输出同一槽对应的 RGB，避免用户 Konsole ANSI 主题与控制台默认色表不同。不发送 OSC palette 改写或 PIO_CMAP ioctl，不改变其他终端 / 虚拟控制台。

Linux 的 SGR dim 会把色彩改为 half-bright，SGR bold 会置高亮位，因此统一模式不用 SGR dim：暗淡色先按 65% RGB 计算再量化；加粗色统一选择对应亮色槽，两边按同一结果显示。无颜色参数的 UI 文字也使用共有白色，画布使用共有黑色；正常退出恢复终端默认颜色。16 色内的精度受限，配置中的不同颜色可能合并；单独通过 OSC 设置过某个 VT 的独立色表无法由 sysfs 默认表读回，该情况下不能保证相同。auto / 16 / 256 / truecolor / none 仍可选择，NO_COLOR 保持优先；原始 RGB 输出的回归测试明确使用 truecolor。

依据：Bash 本机官方手册 /usr/share/doc/bash/bash.html 的 Bash Startup Files；[Linux vt.c](https://github.com/torvalds/linux/blob/master/drivers/tty/vt/vt.c) 中的 default_red/grn/blu、intensity 和 palette 行为。新增 tests/test_login.py 以临时 HOME 和真正的 Bash login + PTY 验证 PATH / 函数自动加载、x 同 shell 目录切换、已有登录文件优先级、去重和安装 / 移除保留用户内容；新增 PTY 色彩对比解码同一目录的 TTY ANSI 与 Konsole RGB，验证文件 / .git / node_modules / 金色 JS / 软链接、反色、加粗及暗淡映射一致。未切换实际 VT 登录会话做显示硬件视觉验证。

2026-10-14 验证：Rust 28 项、Python 58 项全部通过，Clippy -D warnings / fmt / diff 检查通过；release 与配置已更新并安装本机 Bash 登录入口。八场景内存最大 8.184 MB，当前报告 SHA256 对应 release；TTY 双栏 8.102 MB。


## 2026-10-15：文件类型语义配色与跨发行版终端适配

默认定义集中到 themes/terminal-16.conf，Rust include_str 嵌入，Python 历史配置从同一 TOML 读默认；fileX.example.conf 复制该默认定义。代码 / 脚本 / 数据配置 / 文档 / 图像 / 媒体 / 压缩镜像采用黄 / 绿 / 青 / 白 / 紫 / 蓝 / 红，100 多个后缀；JS / MJS #FFD700 保留。已按用户重设计请求替换个人 extension_colors 区段，保留全部目录名颜色、其他 colors、打开程序、快捷键和配置软链接。无后缀 / 未知项仍使用 file_color；分类不检测 MIME，不自动执行文件，也不因 chmod 执行位改变类别。

原 consistent 调暗算法按 65% RGB 缩放后取最近槽，将亮红调暗后落入棕色，压缩包与代码碰撞。现在先求正常颜色槽，再在相同色相的亮 / 暗银行切换；白落到浅灰、浅灰落到深灰、低银行彩色保持色相。仅 consistent 更改，truecolor / auto 原有语义保留。新增 Rust 验证七类正常与暗淡均七个不同槽，保留两类终端使用同一 RGB / ANSI 映射的 PTY 对比。

安装 / 移除脚本用 python3，避免 Ubuntu 不提供 /usr/bin/python。Rust Python opener 保留用户 command=[python,...]；只有 method=python 且 python 缺失、python3 可执行时才替代命令，不改参数和工作目录。Kali 的 Zsh 用同一支持 NUL 路径回传的函数；安装器 --shell auto/bash/zsh 按 SHELL / passwd 选择，--rc 自定义默认为 Bash，可用 --shell zsh 明确指定。Zsh 的 ZDOTDIR 与 .zshrc / .zprofile 维护，profile 仅补 PATH，由 Zsh 自己加载 rc，不重复 source；Bash 原有行为和管理标记保留。其他 shell 不写入 Bash 配置，提示 --command-only。

Cargo.toml 声明 Rust 1.88：edition 2024 中的 let-chain 从此版本稳定。build.sh 显式检查 rustc/Cargo，旧编译器或缺失工具给出可操作提示，filex.sh 首次构建复用该检查。check-system.py 只读 /etc/os-release，分 Arch、Fedora、Debian/Ubuntu/Kali 包管理器，列编译与可选打开依赖，不执行包安装、不修改仓库。Fedora 使用 glib2，Ubuntu/Kali 的 gio 为 libglib2.0-bin；建议在目标发行版源码编译，本机动态 GNU binary 依赖 GLIBC_2.39，不能宣称该二进制在所有旧 Linux 通用。

实际测试入口 scripts/distro-smoke.py 通过 Docker Hub 官方镜像的 amd64/arm64 Linux manifest 选平台、逐层 SHA256 校验、受约束的 tar 数据过滤及白出处理；绝对符号链接改为 root 内相对链接，拒绝逃出缓存根目录，跳过特殊设备。bubblewrap 创建临时用户命名空间，仅 /app 源码只读，主机 home 不绑定；工具依赖 apt/dnf 仅在隔离根安装，结束删除该测试根，镜像层保留用户缓存。apt 的 sandbox user=root 只用于 UID 单映射的隔离用户空间，避免 _apt setgroups 无法映射。未改主机 Docker 权限、用户组或系统包。Python 3.12+ / bubblewrap 仅是可选复测工具，不是应用运行依赖。

scripts/distro-tty-smoke.py 在隔离 root 真实 PTY 中验证 Python3 运行及工作目录、新建 / 重命名 / 复制 / 移动 / gio 回收站、终端设置恢复；随后实际 Zsh login 的 filex 函数与 x 改变调用 shell 的目录。跨发行版报告保留镜像 manifest digest、os-release、实际二进制 SHA256 和输出，不能等同各发行版编译器 / 实机 getty / framebuffer / Wayland 视觉验证。宿主是 Arch，本测试共享宿主内核。

来源：[Rust 1.88 发布说明](https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/)、[Fedora GLib 包](https://packages.fedoraproject.org/pkgs/glib2/glib2/)、[Ubuntu libglib2.0-bin](https://packages.ubuntu.com/noble/libglib2.0-bin)、[Kali 与 Debian](https://www.kali.org/docs/policy/kali-linux-relationship-with-debian/)、[Kali Zsh 默认 shell](https://www.kali.org/blog/kali-linux-2020-4-release/)、[Zsh builtin read](https://zsh.sourceforge.io/Doc/Release/Shell-Builtin-Commands.html)。

最终验证（2026-10-15）：29 Rust 单元 + 61 Python 核心 / CLI / PTY / shell 集成检查通过，Clippy -D warnings、fmt 与 diff 检查通过。本机新配置 / release / 安装入口已更新；八场景内存最大 8.237 MB。Ubuntu 24.04.5 LTS、Fedora 44、Kali Rolling 官方 OCI 用户空间全部通过安装、Bash/Zsh 登录、Python3 fallback、TTY 新建 / 重命名 / 复制 / 移动 / 回收站、termios 恢复、Zsh x 同 shell 目录切换。三组记录的二进制 SHA256 与当前 release 及内存报告相同。首次不可访问主机 Docker socket 后使用 rootless bubblewrap；没有 sudo 授权或改变主机 Docker 用户组。


### 已替换的旧版复制／移动交互

`H/L` 横移，PgUp/PgDn 同层级循环翻页，Home/End 选择首末节点；输入时 Enter 确认、Esc 取消、Ctrl+U 清空、Ctrl+V 粘贴 filex 内部路径缓冲。单栏复制 / 移动填写**完整目标路径，包含目标名称**；相对路径以源项目的父目录为基准，不覆盖已有目标，目标父目录必须存在。文件树不自动遍历符号链接。

跨栏复制 / 移动：左侧选中文件或文件夹，`v` 开启双栏，`Tab` 切到另一侧，用 `g` 指定目的目录并选中目标 folder，再 `Tab` 回源侧，按 `c` / `m`。输入框预填「另一侧选中目录 / 源名称」；另一侧选中文件时用其父目录，选中链接目标时用目标目录。可修改路径，按 Enter 执行、Esc 取消；同名目标不会覆盖。操作后两侧刷新并保留展开状态。双栏至少需要 75 列；`q` 仍显示当前侧信息，双栏下需至少 110 列。


## 2026-10-16 光标目标、多选、输入光标与 ZIP

- `Ui.pending` 保存源快照与复制／移动类型；Space 多选保存在有界显示／搜索路径集合中，`[*]` 为绘制标记。重叠父子源按路径层级去重；批内目标名称重复、已有目标及自身内部复制先统一预检，再逐项操作。运行中权限或 I/O 错误仍可能出现部分成功，保留未完成来源并报告数量，不宣称事务回滚。成功项目取消标记；回收站同样支持多选，重命名仍只处理光标项目。
- 目标模式在已展开目录下附加虚拟 `+` 节点，使用当前树和镜像导航；不创建名为 `+` 的实际文件，也不隐藏真实同名文件。选中 `+` 时 Enter 提示新目录名，创建后选中真实目录；再 Enter 执行。Esc 或成功退出模式会移除虚拟节点，保持其他展开分支。用户主动创建的目录不会因取消复制／移动而删除。
- `src/edit.rs` 为所有输入框提供 UTF-8 字符边界光标；Left/Right、Home/End、Backspace/Delete 与光标处插入／粘贴共用实现。字符组合与终端字体仍依赖 Unicode width，未实现复杂 grapheme 编辑。
- ZIP opener 使用独立参数数组 `bash -- {filex}/scripts/unzip.sh {path}`，不将文件名拼接进 shell 代码。Python 标准库先校验 central directory，拒绝绝对／驱动器／越界路径、特殊设备和链接条目；Bash 随后执行 unzip 校验与 -n 解压。新建随机输出目录，原 ZIP 不删除。Python 只为此可选解压功能所需，普通浏览／复制／移动无需 Python。
- 两个产品独立保留平台接口，共用的模型、输入与 TUI 逻辑同步更新。Windows 只能在本机交叉编译，不能据此宣称 Windows 原生交互测试通过。

验证：两版各 33 项 Rust 共用测试在 Linux 通过；fileX 的 64 项 Python／PTY 测试通过，包括新建 `+` 目录后的多选复制、中文名称中间编辑、TTY 与镜像双栏回归。Windows release 和测试 exe 已交叉编译；未运行 Windows 11 原生测试。记录：`operation-validation.json`。


## 2026-10-16 多选方框与打开方式筛选

多选项使用三行彩色矩形，布局为边框预留行距和列宽，镜像栏复用相同裁切规则；当前光标保留反色，多选边框加粗且不随其他分支变暗。`colors.mark_color` 默认 #FF55FF。ASCII／无色终端保留形状。`Opener.extensions` 限制菜单可见性，并在 open_command 再次验证，防止通过错误默认配置对不相关扩展名执行解释器。旧配置继续继承内置扩展名限制，自定义程序可以显式声明适用范围。

附加验证：两版各 35 项 Rust 共用测试在 Linux 通过，包含矩形行距、颜色配置及专用扩展名限制；真实 PTY 验证 ASCII 方框与亮洋红 ANSI 输出，以及 .txt 隐藏解释器、.py 显示 Python。Windows release 与测试 exe 再次交叉编译，尚无 Windows 原生验证。


## 2026-10-16 目录置顶／沉底与 Git 箭头

`order.top/bottom` 与 `directory_order` 按名称精确排序，置顶可跨过 directories_first；普通项继续按已有 name/size/modified 排序，沉底项最后显示。绝对路径（读取配置时规范化）优先于目录名规则。按列表索引排序，缓存上限选择使用同一排序键。重读配置后沿用既有刷新逻辑。

`src/git_status.rs` 在后台读取稳定的 porcelain v2 -z、ls-files -z 与 cached-upstream 对比，不解析面向人的 git 文本输出。rename 的旧路径单独消费，文件名空格／Unicode／换行不改变字段切分。状态仅保留有界映射，逐项读取未跟踪文件以避免把被忽略的子项误标为未跟踪，最多 4 个仓库；单个子进程最多 512 KiB 输出、1.5 秒执行预算，超限或失败不显示猜测标记。每次展开目录发现其最近 .git 仓库（含 worktree .git 文件）；后台完成通过原有 200ms 输入轮询唤醒一次重绘，无持续的逐帧 Git 调用。关闭分支不会主动扫描其内部嵌套仓库。

关闭 optional locks 与 fsmonitor，不联网、不执行 Git hooks 或外部 diff/textconv。F5 清理完成快照后按已展开分支重新查询。目录以冲突 > 未暂存修改 > 暂存 > 未跟踪 > 分叉/落后/领先 > 干净状态汇总；已配置上游时，文件只在其内容与 cached upstream 有差异时显示领先／落后／分叉，不给其他未改变文件错误标记“未推送”。上游状态为本地缓存，不能证明服务器当前状态。忽略项或超出映射上限的项目保留普通箭头。

来源：[Git status porcelain v2](https://git-scm.com/docs/git-status)、[Git ls-files](https://git-scm.com/docs/git-ls-files)。Windows 仍以交叉编译验证为界，未在 Windows 11 原生实测。

验证：两版各 38 项 Rust 测试在 Linux 通过；fileX 全量 66 项 Python／PTY 回归通过，最后的未跟踪忽略项改进另经 Git 单元测试及 3 项光标／菜单／Git 终端测试复核。临时 Git 仓库验证本地状态、Unicode 重命名、忽略项、上游四种同步状态、输出条目上限及索引字节不变。Windows release 与测试 exe 已交叉编译，未执行 Windows 原生测试。

### 未声明子目录的排序选择器

`top` / `bottom` 支持保留标记 `"<other-folders>"`，选择没有出现在当前有效 top/bottom 名称列表中的所有直接子目录；显式名称先匹配，再匹配目录组。组内沿用 sort 排序，其他普通项仍遵循 directories_first/sort。符号链接不归入目录组，隐藏目录在开启显示时参与；规则不递归。标记同时存在两组时 top 优先，与名称重复时的既有优先级一致。特殊标记为保留名称，不能用来按字面排序同名普通文件。Rust 和兼容 Python 排序共用语义，无新增逐项文件系统查询。

本次验证：每个项目 39 项 Rust 测试通过；新增未声明目录选择器覆盖目录范围、显式项优先及真实子项顺序；Linux Python 回归 67 项通过（配置生成检查在 release 构建完成后复测）。Windows release 和测试程序交叉编译通过，未做 Windows 实机运行验证。

### 复制／移动的显式覆盖与路径输入

当前传输模式新增 `t` 地址输入，单项可指定文件路径以同时改名，多项必须指定目录。光标亦可直接选择已有普通文件作为替换目标。整批先校验，每个已有文件再 Enter 确认；Esc 取消本次提交，保留待操作项。目录与符号链接不替换，拒绝自身及同 inode 的硬链接。覆盖采用在目标旁新建临时目录、完整复制、重命名替换的方式，不先截断旧内容；移动在替换成功后删除源。目标文件的其他硬链接仍指向旧内容。不是整批事务：运行中 I/O 失败时保留已完成结果并报告进度，剩余项待操作；移动若替换成功但源删除失败，会报告并保留源。大文件替换需要额外磁盘空间。Linux 使用 rename，Windows 使用 MoveFileExW(REPLACE_EXISTING)，Windows 实机行为待验证。旧历史中的不覆盖说明已由本节替代；底层默认 transfer 仍禁止覆盖，仅已确认的 TUI 传输使用替换接口。

覆盖功能验证：两个产品各 42 项 Rust 测试通过；69 项 Linux Python 回归（其中旧双栏冲突提示断言更新为确认提示后单独复测通过）。新增 PTY 验证按 Enter 前内容不变、Esc 取消、输入完整目标路径移动改名、直接光标选文件覆盖、多选最后一项取消时整批未开始写入。Windows release／测试程序已交叉编译，未做 Windows 实机运行。

### 文件夹后台命令

`run_background` 默认快捷键 `!`；只对当前选中文件夹运行，普通文件拒绝，复制／移动目标选择期间不可触发。用户明确输入 shell 命令后 Enter 执行，支持 shell 管道与重定向；不自动执行目录内脚本，不加载交互式 shell 配置。stdin=null，stdout/stderr 直接写独立日志，不占用终端或在内存缓存输出。Unix setsid，Windows DETACHED_PROCESS + CREATE_NEW_PROCESS_GROUP；启动失败显示原因，等待线程回收子进程并在日志追加退出状态。退出 filex 后子进程继续，若退出时任务仍在运行，其后不会追加监控退出状态，但进程输出仍写日志。日志不自动删除；日志目录见 README。后台任务自身的内存不计入 filex，长期服务需自行停止和管理。

后台命令验证：两个产品分别 43 项 Rust 测试通过；71 项 Linux Python 回归全通过，含目录带引号／中文、正确 cwd、输出日志、退出 filex 后完成、取消与普通文件拒绝、自定义热键。实际 Visualize-sheet 的只读树输出确认 launch.sh 在第一项；未启动该项目服务。Windows release 和测试程序交叉编译通过，尚未 Windows 实机验证。

### Git 写操作快捷菜单

`git_actions.rs` 提供显式 Git 操作，独立于只读状态缓存。选中项解析 worktree 根（支持嵌套仓库与 .git 文件）；目录软链接以链接本身所在仓库为准。命令用参数数组直接执行，提交信息不经 shell，暂存 pathspec 使用 literal。清除 GIT_DIR/WORK_TREE/INDEX_FILE/COMMON_DIR/PREFIX 等仓库覆盖变量，按用户选中项操作。组合流程为仓库根 git add -A -- .、git commit -m message、git push；任一步非零即停止，包括无内容可提交。独立 commit 不隐式 add，独立 push 不隐式 commit。执行遵循用户 Git hooks、签名与远程配置。命令在当前终端前台运行，接管期间退出 raw/alternate screen，等待 Enter 后恢复树并刷新 Git 快照；传输目标选择模式中不允许触发。不会自动回滚成功的 add/commit，也不会自动重试失败的 push。

Git 菜单验证：两项目各 45 项 Rust 测试、73 项 Linux Python 回归全通过。真实 PTY 在临时仓库与本地 bare remote 验证组合流程推送、确认取消不改 index/HEAD、仅暂存所选文件、提交信息含中文／引号／shell 字面量、pre-commit 失败跳过 push、返回后新建文件。测试未对用户工作仓库执行 add/commit/push，未访问外部远程服务。Windows release 和测试程序交叉编译通过，未做 Windows 实机与真实远程认证验证。

### 目录内 + 作为传输放入位置（2026-10-17）

复制／移动状态中，虚拟 + 节点的 path 是其所在目录，Enter 直接提交传输到该目录，不再弹出新建文件夹输入框。+ 键通过 Browser.target() 获取目录并提交；A（或用户自定义 new_folder 键）创建子目录、选中新目录后 Enter 放入。支持单项与多选，成功后清理两侧虚拟节点，复制保留源、移动移除源，目录布局和传输冲突确认沿用既有逻辑。此行为替代旧版“+ 创建目标文件夹”的说明；t 手动路径输入不变。

此改动验证：两产品各 46 项 Rust 测试、74 项 Linux Python 回归通过。新增真实 PTY 验证多选后选中 + Enter 直接复制、+ 键移动、源保留／移除；既有测试改用 A 新建子目录并继续放入，重命名与覆盖确认回归通过。Linux release 已重建，Windows release 与测试程序交叉编译通过；Windows 实机验证未执行。


### 2026-10-18：可编辑顺序、分组树、记忆排除项

移除双栏及其镜像坐标、左右反转和 Tab 切换；v 改为 grouped/classic，旧 dual_pane / toggle_dual / switch_pane 配置读取时忽略。单树保留源选择、跨目录多选、传输待操作状态及 t 地址输入。

ordering.rs 实现 Unicode 名称上的 % 通配（无正则依赖），sequence 首次匹配、legacy top/bottom 的精确名称例外、= 前缀字面量及 <other-folders>。匹配结果进入已有扫描堆和最终排序键，所以显示上限选取与最终显示一致。完整顺序编辑仅列出当前父目录已加载的实际节点；虚拟 + 不进入持久列表，写为 =字面名称 与末尾 %。grouped 物理分列优先，序列分别作用于目录列／文件列，classic 可以看到混合顺序。

分组 Layout 使用迭代后序计算子树矩形，目录子树纵向堆叠，直属文件列在完整目录子树右侧；两组在父目录同一水平线上连接。高度取两组最大值，标记方框沿用最小三行间距；边线和祖先路径高亮使用相同分组连接。不额外扫描磁盘，节点上限仍生效。展开深树会增加宽度，可自动跟随或 H/L 平移。

z/Z 将精确路径持久化到 browser.excluded_paths。扫描和搜索都跳过排除目录，独立于系统点文件显示；不修改源文件。隐藏项若被外部重命名或移动，旧路径规则仍保留，Z 可移除。显式从命令行／g 把某个目录设为根，仍可进入该目录；规则限制的是其父目录中的显示。

配置写回采用 [toml_edit](https://docs.rs/toml_edit/0.23.10/toml_edit/) 保留注释和其他键，解析并跟随主配置软链接，在同目录创建临时文件、校验原文件未在保存中改变后原子替换。拒绝写入非法 TOML；出错时不应用内存排序／隐藏，提示错误。不是跨进程的配置编辑锁；其他程序同时写同一字段时应先重载。依赖新增 toml_edit，仅执行偏好保存时解析编辑文档。

本次验证：两项目各 49 项共享 Rust 测试通过；74 项 Linux Python / PTY 回归通过，移除过时镜像测试并以分组布局、排序保存、隐藏／恢复、单树传输替代。最终构建另复测偏好 PTY 和配置兼容测试。覆盖 % 前后缀／多段／Unicode／字面量、完整序列、原子写回注释和软链接、每层分组的标签与线段互不穿越、排除项不参与点文件切换及搜索、重启记忆、窄终端和 tmux。Windows release／测试程序交叉编译，不代表 Windows 实机验证。

最终 Linux release 的 8 项真实 PTY 内存场景全部低于 15 MB，最高 VmHWM 为 7.27 MB；原始记录和二进制 SHA256 见 fileX/memory-validation.json。仅计应用进程，Windows、外部程序及终端不在此测量内。
