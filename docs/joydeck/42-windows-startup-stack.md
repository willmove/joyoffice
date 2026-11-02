# Windows debug 启动栈溢出修复

2026-11-01，在 Windows x86_64 / Rust 1.98.1 上复现并验证。从 `origin/main` 的 `d318492ab2170738f167364bb3318becaf8412f6` 建立修复分支。验证时的工作区基线为 `c4b1cf0c25cba9657cb7ad39cc1bb5a2faa522ab`，已有 Word 提交保留，本轮仅修改 Deck 代码与记录。

## 根因证据

`cargo xtask build joydeck` 构建成功，但直接启动 `target/debug/joydeck.exe` 立即报 `thread 'main' has overflowed its stack`，退出码为 `0xC00000FD`；`--version` 正常退出。

在原生栈溢出异常处通过 Windows 调试事件与 DbgHelp 抓取调用栈，关键路径为：

```text
__chkstk
editor::ui::large_tool
Editor::ribbon_content (ribbon.rs，开始工具栏的按钮构造)
Editor::render_workspace (chrome.rs)
Editor::render
GPUI AnyView / Window::draw
GPUI App::open_window
editor::open_session
main
```

该路径没有递归循环。读取本机二进制的 PE 头及 x64 unwind 信息，主线程预留栈为 1,048,576 字节；未优化的链式 GPUI builder 为大量临时值分配栈空间。同一函数不同工具栏/表单分支的临时值一起放大了函数栈帧，首帧调用按钮构造时耗尽剩余栈。

| 函数/表单 | 修复前栈帧（字节） | 修复后（字节） |
| --- | --- | --- |
| 原 `ribbon_content` / 拆分后的开始工具栏 `home_ribbon` | 605,224 | 248,792 |
| 原 `inspector` / 拆分后的对象表单 `object_inspector` | 501,784 | 214,056 |
| 工作区 `render_workspace` | 298,936 | 298,936 |

前两行比较原有大型函数与其拆分后最大的子表单，调用分派还有独立的小栈帧，不代表整条调用栈的总量。数值来自本机 debug 二进制，并非跨编译器的固定 ABI 或性能承诺。

## 实现

- 工具栏按插入、表格、视图、文件、开始拆为独立函数，再装入原有容器。
- 属性面板保留原容器、标题、滚动区和控件顺序，按文件、设计、表格、空选择、对象表单分派构造。
- 未改变依赖、优化配置或链接器栈大小。最终默认 debug 二进制仍预留 1 MiB 栈。
- 增加仅 `validation` 启用的 `--startup-smoke` 路由，直接在真实原生进程的 UI 主线程调用 `Window::draw`，释放 entity 借用后绘制并按 GPUI 契约清理元素 arena。
- 新增原生进程回归测试，使用私有偏好与恢复目录，检查进程正常退出及完整绘制标记；带超时终止。测试需要可用原生桌面/显卡，因此常规 CI 中显式 ignored，本机单独运行。

## 本机验证

```powershell
cargo xtask check joydeck --validation
cargo test -p joydeck --features validation --test recovery_process native_startup_and_all_ribbons_fit_the_default_ui_thread_stack -- --ignored --nocapture
cargo xtask build joydeck
python scripts/check_gates.py
python scripts/check_docs.py
```

- Deck 产品检查：fmt、Clippy `-D warnings` 通过；194 项测试通过，原生桌面测试按设计 ignored。
- 单独执行原生桌面回归通过：1280×800 初始布局、980×800 恢复属性/备注布局，各运行空选择、预设圆角矩形、表格选择三组；每组切换 16 个工具栏并绘制，共 96 个场景。包括标准属性面板、主题/版式预览和窄窗口开始工具栏。
- 用 computer-use 实际启动普通 debug 程序，确认主编辑窗口、开始工具栏和画布显示；实际点击设计工具栏后确认右侧属性面板正常显示。通过原生关闭操作退出。
- 最后重新构建默认功能的 GUI 与 CLI，避免交付仅能在 validation 构建运行的程序；普通编辑器、`--viewer`、`--diagnostics` 启动后均持续运行至少 4 秒，未出现栈溢出；`--version` 退出码 0，普通构建拒绝 `--startup-smoke` 并返回 2。
- 最终默认 `target/debug/joydeck.exe` 的 SHA-256 为 `c59188319c32f5a848dccdb7594458f09bfc3da22b2bac8c08255ccf3894855e`。
- 工程门禁通过，扫描 927 个 Rust 文件；文档检查通过。

## 范围与未覆盖项

本轮实测为 Windows debug，不替代 macOS/Linux、release 包、复杂文稿、真实输入法、辅助技术或完整产品验收。未修改公共库、锁文件与构建规则，未扩大其它产品的验收状态。

常规工具栏与属性面板已覆盖；高级设计/导出折叠表单不计入上述 96 个场景。扩展试验曾同时开启高级设计表单和备注区，发现原有重复 a11y node ID 断言，与本次栈溢出的调用栈不同。该布局存在同一输入实体在多处显示的情况，但具体重复节点尚未单独定位，保留为独立问题，未将该组合标记通过。

2026-11-02 后续：该组合已单独定位并修复，新增明确要求 a11y 激活的回归；见 [高级设计与备注的节点重复修复](43-design-notes-a11y.md)。本报告的原验证范围与结果保持原义。
