# R2 基础编辑闭环验证

2026-11-02，JoyDeck 0.3.0，Windows x86_64/MSVC，Rust 1.98.1、GPUI Kit 0.8.1 / pre 0.3.10。新 Windows 开发包路径与 SHA-256 见 [package.json](package.json)，全部源快照及许可随包提供。R0/R1 包保留。

## 可运行切片

1. 纯命令引擎/历史与有界文件服务：独立编译 engine/shell/CLI，真实 PPTX 文字、变换、插入和页面操作经同一事务入口；保留 DeckCraft 的 copy-on-write 历史估算；未迁入 Session 的 IO/PDF/媒体服务。
2. 保存/项目/恢复闭环：PPTX 保留式 writer、.joydeck v2、prepare/sync/指纹提交、revision 保存语义、租约恢复。15 项新模型集成测试见 [日志](logs/editing-tests.txt)，包括 Windows 目标文件禁止替换的真实失败场景，以及文件已发布但恢复清理失败仍返回保存成功的语义。
3. GPUI 基础编辑器：真实对象选择、框选、多选/拖动、取消、工具栏、文字草稿应用、undo/redo、保存重开、关闭提示和恢复列表。坐标命中透明层已在实际窗口中修正为页面左上角锚定。

## 自动检查

产品与无界面门禁日志分别为 [完整产品](logs/product-check.txt)、[无界面](logs/headless-check.txt)，包括来源/依赖/文档、fmt、严格 Clippy、Rust 测试及 5 项 Python 工程测试。来源共 209 项（含独立字体资产）；engine 的 IO/导出服务隔离、新增 sha2/getrandom 宿主精确版本及无 egui/epaint 闭包均有门禁。

最终完整产品门禁通过 **839 项 Rust 测试 + 5 项 Python 测试**；无界面门禁通过 **838 项 Rust 测试 + 5 项 Python 测试**，失败均为 0。摘要见 [verification.json](verification.json)。这是两个配置的同一批测试，不累加为互不重复的测试数。

CLI 真实执行检查见 [开发 CLI](cli-verification.json)、[最终包 CLI](cli-release-verification.json)。每轮 20 项调用，覆盖两种格式保存/重开、公开 OOXML 文字/几何检查、原包未触及成员字节相同、原始输入不变、PNG、旧格式/未来能力拒绝、新文稿、已有输出保护和强制终止后的恢复。探针完成快照落盘后由另一个进程强制终止，并在新进程恢复，未用正常析构模拟崩溃。

## 最终 Windows 开发包

- 包：`dist/joydeck/joydeck-0.3.0-r2-windows-x86_64-release-be894dc194.zip`。
- ZIP SHA-256：`2bd0afaf916614bb2606800a9382fcda652465ca1bcdefb0cfa9a859ca51765e`。
- 源文件指纹：`be894dc19412e6eaff0123da3b3d7d88ca5dec81de35ddfcfb92b3949ed25074`；最终核查与当前生产源码一致，文档和运行证据不参与该指纹。
- GUI SHA-256：`8779c17f23b422bb7ba8ff92f19584853051a65fe52787a14f78a8844f501647`。
- CLI SHA-256：`3d02bf63ec90120e9d465ea14d70cf22464fdb470bbc9040cc2a69dc4950f8e7`。
- [最终构建日志](logs/package-final.txt)、[包元数据](package.json)、[仓库外启动及 758 个文件校验记录](standalone-launch.json)。`logs/package.txt` 是保留的前一切片包日志，以本节最终包为准。

## 实际桌面验证

最终 ZIP 解压到仓库外 `%LOCALAPPDATA%/Temp/joydeck-r2-final-kyv887ym/中文 空格路径/`，以该包目录为工作目录运行，不依赖工作区资源。使用独立工具 python-pptx 1.0.2 生成的公开 OOXML 文稿 `independent.pptx`，未通过私有 JSON 冒充 PPTX。

1. [打开真实 PPTX](01-release-editor-pptx.png)，在文字草稿框将标题改为 `JoyDeck R2 · 编辑成功` 并 [应用到实际模型](02-release-text-applied.png)。
2. 鼠标拖动绿色矩形产生一个事务；[撤销拖动](03-release-undo-drag.png)保留标题编辑，[Ctrl+Y 重做](04-release-redo.png)恢复位置。日志 revision 1→2→3→4。
3. [Ctrl+S 另存 PPTX](05-release-saved-pptx.png)后 dirty 清除，并在 [桌面重新打开](06-release-reopened-pptx.png)。[包内 CLI](gui-saved.inspect.json) 和 [独立 python-pptx](gui-independent-readback.json) 均确认 3 页、中文标题和横向位置 50.4→156.391732 pt，原输入 SHA 不变。
4. 重开后插入一个矩形，验证 [未保存关闭提示](07-release-close-guard.png)，选择“取消”后仍可继续编辑。等待 revision 1 恢复快照落盘，强制终止桌面进程 **18320**，不经过正常关闭和析构。
5. 新进程 **19736** 的 [恢复列表](08-release-crash-recovery-list.png)发现该记录，[恢复出未保存矩形](09-release-restored-unsaved-edit.png)。恢复后另存路径为空，要求新路径保存；保存为 `.joydeck` 后再次 [桌面重开](10-release-reopened-recovered-joydeck.png)，中文标题、已移动矩形和新增矩形均存在。
6. [恢复结果与进程记录](gui-crash-recovery.json)、[原生文件 inspect](gui-recovered.inspect.json)、[CLI 转回 PPTX](gui-recovered-converted.inspect.json)确认 3 页、首面 5 个对象；独立 python-pptx 可读取转换结果。最终桌面进程正常退出，运行日志已关闭。

[保存/编辑运行日志](logs/package-runtime.txt)、[重启恢复运行日志](logs/package-recovery-runtime.txt)及 [保留的实际输出](outputs/) 可用于复核；[恢复后首面 PNG](recovered-render/slide-0001.png)由包内 CLI 生成。界面图片异步绘制，最早的即时截图可能只显示白色页面；重复重开并等待 1.5 秒后已确认首屏和缩略图，未据加载中帧判断文件内容。系统文件选择器的本轮选择操作未覆盖，已验证路径输入和启动参数打开。

## 范围

本轮迁移基础命令子集，不宣称上游 329 条编辑命令、旧 engine 依赖的 tests-upstream 集成测试、完整多窗口、复杂路径/手势、原位富文本/IME 或原生对象渲染已完成。撤销历史限本次会话；重开/恢复建立新历史。输入框的草稿和文稿事务分开，整段替换使用首文字样式。

PPTX 保存只支持已验证的对象/页面变更，更新前会内存重开并检查页面/对象数量、文字和几何；不支持的签名/结构/复杂对象写回拒绝，原文件不变，可另存 .joydeck。未触及 ZIP 成员字节保留，修改页可重新序列化，未承诺 PowerPoint 全保真。恢复约 250 ms 后安排后台落盘，预算或 IO 失败会显示；无法保证落盘前或断电瞬间的修改。

Windows 为本轮原生环境；macOS/Linux 未执行。开发包未签名，SBOM 中 21 个 registry crate 未自带完整许可文本，明确清单在 package.json；正式对外分发审计仍待 R8。文件原子提交与两次指纹检查不等于 OS 级 compare-and-swap。
