# MACOS-MENU-ORDER 交接报告

- 任务 ID：MACOS-MENU-ORDER（macOS 菜单栏布局修正；由 gpui-pre 迁移会话在 GUI 冒烟后派出）
- 执行者/软件版本/模型：Claude 桌面应用 Code 会话，Claude Opus 5.5（`claude-opus-5-5`）
- 主机/系统/架构：macOS 27.0.1（26A434），arm64；Rust 1.98.1
- 代码基线：`claude/gpui-pre-migration` @ `616441f5194c756a10b1ad08da2e40a0c3d9cd32`（开工时尚未合入 main；main 为 `0dcc0f0`）
- 启动时实际 HEAD：同上
- 分支：`claude/macos-menu-order`（独立 worktree `.claude/worktrees/focused-kowalevski-8b9860`）
- 状态：完成本轮切片（代码、单元测试、lint、macOS GUI 读菜单栏）
- interface_ready：true。`Registry::menus(platform)` 签名不变，新增 `MenuKind::display_order`、`MenuKind::on_platform` 两个公开函数（增量）。
- implementation_verified：true。无窗口测试与 macOS 开发构建的菜单栏读取均通过，见“验证”与“GUI 读菜单栏（macOS）”。
- acceptance_complete：false。macOS 菜单栏已人工读取确认；Windows、Linux 菜单只有单元测试，没有人工运行。

## 问题与原因

macOS 菜单栏原为：粗体应用菜单（内容却是“打开…”“另存为新副本…”）、“JoyWord”（只有“退出 JoyWord”）、编辑、视图。`MenuKind` 声明顺序为 File、App、Edit、View，`Registry::menus` 用以 `MenuKind` 为键的 `BTreeMap` 输出，结果按枚举序排列；gpui-pre 的 macOS 后端（`gpui-pre-macos-0.3.9/src/platform.rs` `create_menu_bar`）把菜单按顺序放入 `NSApp.mainMenu`，系统把第一个子菜单当作应用菜单，于是“文件”占了应用菜单的位置。gpui 0.2.2 基线相同，属既有问题。

## 实际变更与支持范围

| 路径 | 变更 |
| --- | --- |
| `crates/joyword-shell/src/command.rs` | `MenuKind` 声明顺序改为 App、File、Edit、View；新增 `MenuKind::display_order(Platform)`（各平台顶层菜单顺序）与 `MenuKind::on_platform(Platform)`（某菜单的命令在该平台显示于哪个菜单、是否折叠）。`Registry::menus` 改为按 `display_order` 输出，不再依赖枚举派生序；分组键改为 `(折叠, group)`，折叠进来的分组总在目标菜单自有分组之后。新增 4 项单元测试 |
| `crates/joyword-shell/tests/c09_open_session.rs` | `file_menu_offers_open_and_save_copy_but_not_save` 原断言 Linux 有独立 App 菜单，改为断言 Linux 的“文件”为 打开、另存为新副本、分隔线、退出，且没有 App 菜单；“没有 file.save”的原意不变 |
| `apps/joyword/src/main.rs` | 仅给 `menus()` 加文档注释，说明它只做转换、顺序与平台位置由 `Registry::menus` 决定；逻辑未改 |

各平台结果（内置注册表）：

| 平台 | 顶层菜单顺序与内容 |
| --- | --- |
| macOS | 应用菜单 JoyWord（退出 JoyWord）→ 文件（打开…、另存为新副本…）→ 编辑 → 视图 |
| Windows / Linux | 文件（打开…、另存为新副本…、分隔线、退出 JoyWord）→ 编辑 → 视图；不输出 App 菜单 |

其他规则：

- macOS 上即使注册表没有 App 命令，也在首位输出（空的）App 菜单，避免其他菜单再被系统当作应用菜单。
- 命令 ID、快捷键、命令标签与 `menu.*` 本地化键均未改动；`validate()` 与现有的菜单/快捷键/名称同源测试照常通过。
- `MenuKind` 的派生 `Ord` 随声明顺序改变；仓库内除 `Registry::menus` 外没有按它排序的调用方，`menus` 也已不再依赖它。

## 验证

| 完整命令或人工步骤 | 实际结果 | 环境/输入/日志 |
| --- | --- | --- |
| `cargo fmt --check` | passed | — |
| `cargo test -p joyword-shell --locked` | passed，145 passed / 0 failed（库内 41 项，原 37 项） | 新增：`macos_leads_with_the_application_menu`、`windows_and_linux_lead_with_file_and_end_it_with_quit`、`every_menu_command_appears_exactly_once_on_each_platform`（含无首尾/连续分隔线、无重复菜单）、`folded_app_groups_follow_file_groups_and_macos_keeps_an_app_slot` |
| `cargo test -p joyword --features dev-runtime-shaders --locked` | passed，11 passed | 无窗口 |
| `cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings` | passed | 仅有迁移报告已记录的 `block 0.1.6` future-incompat 提示 |
| `python3 scripts/check_docs.py` | passed | — |
| macOS GUI 读菜单栏（见下节） | passed | 2026-10-16 经用户同意占用桌面；构建对应提交 `d861e94` |

## GUI 读菜单栏（macOS）

环境：同上主机；`cargo build -p joyword --features dev-runtime-shaders --locked`（对应提交 `d861e94`，复跑无重新编译）后 `python3 scripts/package_macos_dev.py` 包装为 `target/dev/JoyWord.app`，`open` 启动；系统语言为简体中文，应用界面为中文。启动前确认没有其他 JoyWord 进程。经 System Events 辅助功能接口只读取菜单栏和菜单项，另截取屏幕顶部菜单栏区域核对粗体应用菜单。

| 检查项 | 结果 |
| --- | --- |
| 应用激活 | `frontmost=true`，菜单栏属于 JoyWord |
| 菜单栏顺序 | Apple 菜单 → **JoyWord**（粗体应用菜单）→ 文件 → 编辑 → 视图 |
| JoyWord（应用菜单） | 退出 JoyWord（⌘Q） |
| 文件 | 打开…（⌘O）、另存为新副本…（⌘⇧S） |
| 编辑 | 撤销（⌘Z）、重做（⌘⇧Z）、分隔线、全选（⌘A） |
| 视图 | 放大（⌘=）、缩小（⌘-）、实际大小（⌘0）、分隔线、界面语言切换为 English |
| 应用菜单“退出 JoyWord” | 经辅助功能点击后进程退出（文档未修改，无确认框） |

快捷键一列：按键字符经 `AXMenuItemCmdChar` 实际读取（Q、O、S、Z、Z、A、=、-、0）；修饰键未逐项读取，按注册表标注。

与 gpui-pre 迁移冒烟记录对照：原先粗体应用菜单里是“打开…”“另存为新副本…”，“退出 JoyWord”在第二个菜单；现在应用菜单只有退出，“文件”排在第二位。快捷键字符与注册表一致。未检查英文界面下的菜单栏，也未检查语言切换后重建菜单时的顺序；两者都经同一个 `Registry::menus(Platform::MacOs)`，单元测试已覆盖其顺序。

## 接口或依赖提案

无新依赖。新增公开函数 `MenuKind::display_order(platform: Platform) -> &'static [MenuKind]` 与 `MenuKind::on_platform(self, platform: Platform) -> (MenuKind, bool)`，调用方无需改动。

## 阻塞与下一步

- Windows/Linux：菜单顺序只由单元测试覆盖，未在真机运行；CI 三端构建不等于人工验收。
- macOS：只读取了中文界面启动时的菜单栏；英文界面及切换语言后的菜单栏没有人工核对。
- 退出项的文字在三端都用 `command.app.quit`（“退出 JoyWord”/“Quit JoyWord”）。Windows 惯例是“退出”/“Exit”。本任务不改本地化键；是否增加按平台的标签键由协调者决定。
- macOS 应用菜单目前只有“退出”，缺少“关于”“隐藏”“服务”等系统惯例项；这些需要对应命令与实现，不在本任务范围。
- 公共文档（IMPLEMENTATION/TASKS 中 T106 的菜单描述）如需更新，由协调者处理。
