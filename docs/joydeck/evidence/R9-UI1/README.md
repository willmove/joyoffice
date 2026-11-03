# R9-UI1：格式控件与旧方法恢复

2026-11-03，只在 JoyOffice 工作；起点为 main `3691d4dea11df473c95f94af9718cc699cd984f8`。
用户已明确授权合入 PR #120；合入前 21 项检查通过，合并保留全部源历史，见 [合并记录](merged-pr120.json)。原始备份与回退包仍按 [R9 交接](../../../joyoffice/JOYDECK-R9-HANDOFF.md) 保留。

## 实现范围

- 现场观察本机 DeckCraft 0.14.1 的字体/颜色菜单与 PowerPoint 中文 Ribbon，见 reference/。不更新参照软件，不打开用户的私人文稿。
- Ribbon 字体使用宿主 GPUI Kit 的 Select/SearchableVec，搜索系统已安装字体，并保留文稿请求的当前字体；选择使用原有 format_runs 事务。
- 字号独立常用值、自定义 1–400 磅、增大/减小和 Ctrl+Shift+./,；颜色提供主题/标准色块、当前颜色指示与严格六位 RGB。主题色保存为 scheme 引用，支持主题继续演进；高级字体对话框保留。
- UI 保留原位文字选区、表格目标、已有撤销/恢复/保存链路，组合输入中禁用格式选择；不直接改模型。
- 恢复 command.validate、document.undo、document.redo 的方法名：预检执行 copy-on-write 引擎快照，包含实际预算/有效性/历史检查，不修改活文稿或历史；Undo/Redo 使用当前真实历史、修订校验和重放保护。状态增加 canUndo/canRedo。
- 方法参数仍为 schema 2：预检 params.command，顶层 target / expectedRevision。engine.commands 明确声明恢复范围。schema 1 请求继续拒绝，完整旧协议适配、比较/逐对象接受、桌面远程绑定仍未完成。

## 验证记录

- [Deck check](deck-check.txt)：875 通过、0 失败，含 strict Clippy 与全产品 crate 测试；新增真实预检、无效/过期/旧请求、历史和请求重放回归。
- [格式](format.txt)、[门禁](gates.txt)、[依赖](dependencies.txt)：已通过。Cargo.toml / Cargo.lock / Rust 和 GPUI pins 未变；公共库未修改。
- [联合 Clippy](joint-clippy.txt) 与 [联合测试](joint-tests.txt)：3,575 通过、0 失败、1 项既有计时测试忽略（258 组）；见 test-summary.json。
- [构建日志](package-build.txt)、package.json 登记本轮实际 Release 包；历史 R9/package.json 保留原字节。
- verify-compat.py 在最终包运行 control 与 MCP，两条传输使用公开 PPTX、预检、真实修改、Undo/Redo、重复请求和另存，再由 python-pptx 独立读回；原件 hash 必须不变。
- 候选包 5ac9c94f1d 的桌面证据为 gui/01–11 与 candidate-*.json，覆盖字体/字号/主题色、另存与重开；它发现字号快捷键问题。最终 8e70ec1350 的 1,753 项校验和、全套 CLI 与兼容回归通过，最终桌面专项正在等待 Windows 解锁；不把候选包截图视为最终二进制证明。

Windows 实测发现 OEM 标点归一化会清除 Shift：Ctrl+Shift+. 到达 GPUI 时为 Ctrl+>。已兼容两种表示并新增回归；末次重新通过完整 Deck 与联合检查。第一次修正打包在移动测试模块时触发源码快照保护并拒绝出包（package-source-guard.txt），最终使用冻结源码重新构建。

## 本轮开发包

Windows x86_64 Release 未签名开发包：`dist/joydeck/joydeck-0.8.0-r9-windows-x86_64-release-8e70ec1350.zip`。

- ZIP SHA-256：`110db0a349561c989d8e2ea38b34ba6b3f6a090ffcf8c2d38e57acafe420f99d`。
- 源码快照：`8e70ec13500373fe9196f85732bd1d3a4b4f4be8e30f42958c343428b0d72bfe`；GUI/CLI hash 与 1,753 项 payload 校验见 package-runtime.json。
- 最终包 R1/R2/R3/R4–R5/R6–R7 分别 28/20/20/23/32 次 CLI 调用通过；R6–R7 另有 42 条 MCP 响应验证；本轮兼容预检/历史独立 control+MCP 共 36 次交换通过。
- 候选包桌面字体搜索/选择、32pt、原位首字符主题色、单步 Undo/Redo、原生另存/重开成功；python-pptx 独立确认标题文本不变、全部标题 run 为 Arial/32pt、只有 J 是 accent2、原件 hash 不变、仅 ppt/slides/slide1.xml 改变，见 candidate-desktop-public-read.json。最终包的字号快捷键与保存重开专项待解锁后执行。
- 首次对话框 UIA set_value 虽改变显示文件名，返回仍是默认 .joydeck，记录在 save-dialog-observation.json；改用真实键盘输入完整 PPTX 路径并 Enter 后，实际文件与公开格式读回均通过，不把 UIA 显示值当保存成功。
- 尚未逐项原生验证自定义字号/RGB 的全部非法输入、完整字体/混合格式和平台读屏；字体列表行在当前 GPUI Kit 的 UIA 名称为空，搜索框/触发器可访问，完整读屏资格仍留 JD-804。Ctrl+Shift+Right 当前仍为字符选择，完整按词快捷键保留 JD-305/308。

## 继续开发

这次是 JD-208/305/308/508/706/906 的可运行切片，不能视为完整 R9 或完整 UI/旧功能兼容验收。下一优先继续字体主题分组/预览/重扫、混合格式状态、完整字符/段落和表格选择器、形状图库；随后最近文件/模板和比较/逐对象接受、旧命令参数映射。实际呈现延迟、复杂 OOXML 保真、多平台真机、正式安装升级/签名等仍按任务表保留。未签名 Windows 开发包不代表正式发行。
