# M3 文字编辑预览与 M0–M2 收尾记录

日期：2026-10-30。**这是可构建的开发预览；完整 M3 和此前全部收尾尚未完成。** 本次入口为已有 M2 源码，按用户要求继续 M3，同时保留以前阶段未测项；不将此前未通过的入口条件改写为通过。

## 已交付

对应 FR-APP-06、FR-TXT 系列、T-M3-01～06 的部分能力，以及 T-M0-07、T-M1-02、T-M2-07/08 的本机增量。

- 画布双击文本框或点“编辑文字”进入真实 EntityInputHandler；接通 surrounding text、UTF-16 selected/marked/replacement、候选位置与点击命中。旋转文字暂明确拒绝进入编辑。
- Unicode 字素导航/删除（含 ZWJ、旗帜、组合音标），方向/Home/End、范围拖选、双击词/三击段、反向 Shift 选区，纯文字 copy/cut/paste。上下导航当前是逻辑段边界，尚非保持横坐标的视觉行导航。
- 预编辑只存在 TextEdit 草稿；提交由 Session 单次 ReplaceObject 事务进入历史。Esc/空 marked text 取消并恢复原选区；保存、切页、关闭、失活取消尚未提交的预编辑。unmark 明确提交。属性输入拥有独立焦点；格式操作保留文字选区并回到画布。
- 局部粗体/斜体/字体/颜色，输入样式继承、run 拆分与合并、非法样式和范围拒绝，CPU 预览输出 tspan 样式。缺字体提示定位到页面/对象，原字体名保留；逐 glyph 覆盖和嵌入字体仍未完成。
- `.joydeck` schema 1.2，minReader 3，required feature `rich-text-v1`；保留 1.0/1.1 读取。旧程序拒绝新文件，避免保存丢失 runs。本文字模型尚未包含完整段落属性，不能声称完整富文本模型已结束。
- macOS 原生链接、Cmd 快捷键、持久恢复目录 `~/Library/Application Support/JoyDeck/recovery`；Linux 使用 XDG_STATE_HOME 或 ~/.local/state，Windows 保留 LOCALAPPDATA。
- 新增通用 native smoke、不可覆盖旧包的打包脚本、`cargo xtask stage M3`；check 扩展至 all-targets Clippy。CI 更新到 M3，但未远端执行。

## 自动验证

环境：macOS 27.0 / Darwin ARM64，Rust 1.98.1，Kit 0.8.1 / GPUI pre 0.3.10；debug 构建，不能作为 release 性能成绩。

- `cargo xtask check`：fmt、all-targets Clippy、**35 项测试通过**。见 [日志](evidence/legacy-logs/m3-unlocked-check.log)。
- `cargo xtask stage M3`：实际链接与 workspace tests；见 [日志](evidence/legacy-logs/m3-unlocked-stage.log)。
- 新增 8 项文字测试：UTF-16 surrogate/倒序/越界拒绝、ZWJ/旗帜/组合音标字素、预编辑提交/取消/失败原子性、局部样式 splice/保存重开/红色 CPU 像素、非法 run、反向选区、空标记取消、缺字体保留与定位。
- 新增 1 项 macOS 混排真实 glyph 回归；见 [24](24-m3-macos-verification.md)。
- 新增 2 项保存故障测试：目标已存在、目录占用、父路径缺失、Unix 父目录不可写；检查旧目标字节完整、dirty 保留、暂存清理。Unix 权限测试要求普通非 root 账户。
- 之前的 24 项测试继续通过，含真实异常退出子进程恢复、文件冲突、旧恢复副本隔离、ZIP/XML 预算与取消。

## 原生窗口验证的实际边界

首轮桌面不可交互时，M1 停在首帧，M3 返回 no_active_window，未将其记为通过。用户随后确认桌面已解锁。本轮已取得实际截图、定位并修复字体兜底与画布坐标问题；最终分发二进制的 M0/M1/M2/M3 adapter 回归已全部通过。系统拼音输入法集成也验证了预编辑、提交、取消、一次撤销和保存重开。最新证据与支持边界见 [24 macOS 验证记录](24-m3-macos-verification.md)。

系统拼音使用 OS 键盘事件驱动实际输入法，具备真实候选窗截图，不等同于物理键盘、多语言输入法、跨屏或其他平台验收。原先的 no_active_window 阻断已经解除，完整 M3 仍有下表中的软件缺口。

## 尚未完成的退出条件

| 阶段 | 实际剩余 | 后续验证方式 |
|---|---|---|
| M0 | 实际 IME/focus、多屏 DPI、完整双窗口人工流程、Linux native/远端 CI | AT-01，解锁桌面真实输入及其他平台环境 |
| M1 | 系统字体逐字缺字/嵌入字体、细粒度进度、完整 DocumentStore、完整丢失 part 定位、输出裁剪专项、LARGE 首屏/RSS/upload 基线、授权外部语料 | T-M1-02/03/04/06/07/08 与 AT-02/15；macOS native smoke 已重跑通过；其余门槛待补 |
| M2 | 完整人工演示、空间耗尽/写中途失败/平台文件占用矩阵、跨屏/DPI/Linux、断电边界 | T-M2-10、AT-03/06/14；新增测试未替代整套故障矩阵 |
| M3 | **统一 CPU/编辑排版**、自动换行、混合字号/段落/列表/缩进/段距、旋转输入、视觉行导航、完整 bidi affinity、字体 cluster fallback/嵌入许可诊断、下划线/删除线/高亮、rich clipboard、其余输入法/平台、读屏/跨屏 DPI | T-M3-01～08、AT-04/05；macOS 拼音已通过，其他输入法/平台待补；不能把软件缺口称为仅剩人工验收 |

活动编辑的 glyph/caret/selection 使用同一 ShapedLine，光标/选区移动不触发整页 raster；退出编辑的静态 CPU 预览依然走 SVG/usvg，尚不满足统一文字布局契约，这是明确的软件阻断项。没有把它移到 M4 后标 M3 完成。父任务保留未勾选，仅记录本轮已完成子项。

## 产物、复现和回退

- [macOS ARM64 开发包（旧归档：joydeck-m3-preview-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md)，内部有二进制、独立 fixtures、许可证与 SHA256.json；未签名，未验证干净机器安装。
- [当前源码快照（旧归档：joydeck-m3-preview-source.tar.gz）](../joyoffice/JOYDECK-ARCHIVES.md)。
- [开发前源码回退点（旧归档：joydeck-before-m3-source.tar.gz）](../joyoffice/JOYDECK-ARCHIVES.md)。当前目录无 Git 元数据，未创建虚构提交。当前环境没有上一轮 STATUS 指向的 Windows dist 文件，不声称那些二进制已在本机保留或重新验证。
- `cargo xtask check`；`cargo xtask stage M3`；`python3 scripts/smoke-native.py M0 M1 M2 M3`。
- `python3 scripts/package-native.py` 只打包当前原生平台，并拒绝覆盖已有同名目录。`--binary /绝对路径/joydeck` 可验证分发二进制；package 里附带 fixtures，已消除 viewer smoke 依赖编译机源码 fixtures 的问题。
