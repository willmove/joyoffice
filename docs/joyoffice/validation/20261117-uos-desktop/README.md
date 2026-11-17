# UOS 25 三产品桌面实机验证

2026-11-17 至 18，任务分支 `docs/uos-desktop-validation-20261117`，验证源码为 `f6895dde66cefb8af9e1e2559d79829d5d667ff2`。本轮直接在本机桌面执行当前 JoyWord、JoyDeck、JoySheet Debug 构建，使用隔离配置和一次性测试文件；没有修改产品源码、依赖或用户文档。**结果只覆盖下述场景，不表示各产品阶段或完整平台任务已完成。**

## 实际环境

- UOS Desktop 25 Professional，Linux `6.18.57-061857-generic`，x86_64；X11／DDE，2560 × 1600，应用报告缩放 1.25。
- AMD Ryzen AI 9 HX 370／Radeon 890M；GLX 报告 `radeonsi, gfx1150, ACO`、Mesa 24.3.0、direct rendering=yes。**这不是国产 CPU／GPU、ARM64 或 LoongArch 验收。**
- Rust／Cargo 1.98.1，根 `Cargo.lock` 未改。Word／Deck／Sheet 桌面 SHA-256 分别为 `2ea82988…bfbf`、`972753c5…cfa6`、`2fdfecf0…3b39e`；完整值见各报告和 verification.json。
- 系统输入法为运行中的搜狗／Fcitx5 `com.sogou.ime.ng.fcitx5.uos`。普通回归需要 ASCII 路径或避免候选窗口时，临时用 `fcitx5-remote -c` 停用，结束后用 `-o` 恢复；最终已恢复为活动状态 2。真实 IME 探针始终单独执行，不把停用输入法后的结果记作 IME 通过。
- X11 窗口像素来自 ImageMagick `import`；键盘／鼠标来自 `xdotool` 的操作系统合成事件。JoySheet 主回归另使用生产 GPUI 合成事件，JoyWord／JoyDeck 的控制接口用于读回实际会话。**这些都不等同于物理键盘、系统 IME、读屏或 GPU 呈现延迟认证。**

## 结果总览

| 产品 | 已通过的限定桌面场景 | 明确未通过 | 证据 |
| --- | --- | --- | --- |
| JoyWord | 5 组基础打开／编辑／查找／普通保存重开；3 组中文模板；3 组居中未保存弹框 | DDE 原生另存点击保存后 60 秒仍无文件；系统拼音直通 | [基础](word-native.json) · [模板](word-template.json) · [弹框](word-save-dialog.json) · [另存失败](word-native-save-failure.json) · [IME 失败](word-ime.json) |
| JoyDeck | 5 组当前 R9 打开／复制页／历史／保存／深色重开；原生 DDE 另存及三页 PPTX 独立读回 | 备注中的系统拼音直通 | [基础](deck-native.json) · [原生另存](deck-native-save.json) · [IME 失败](deck-ime.json) |
| JoySheet | 13 组原生回归，含公式、历史、合成组合态、剪贴板、格式、关闭取消、多行编辑和 XLSX 重开 | 系统拼音直通，无组合态 | [13 组报告](sheet-native.json) · [IME 失败](sheet-ime.json) |

三产品构建均通过。各产品标准检查结果见[自动检查](#自动检查)；GUI 通过不能抵消产品测试中的既有失败。

## JoyWord 实测详情

1. 中文／空格路径 DOCX 打开，系统合成键盘输入、撤销重做和查找均进入实际编辑会话；普通 `Ctrl+S` 的元数据保护确认后，独立 OOXML 读回包含编辑标记，新进程重开一致，仓库原语料散列不变。[重开截图](gui/word-reopened.png)
2. 首次启动显示中文欢迎文档；真实鼠标进入文件首页和完整九项新建页，点击创建「项目计划书（示例）」，中文正文和进度分工表显示正确，活动会话读回标题及表格文本。[九项模板](gui/word-gallery.png) · [项目计划书](gui/word-project-plan.png)
3. 未保存新建提示为居中三按钮弹框。Esc 取消保留修改；Enter 保存并再次确认替换原文件后继续新建；另一次点击「不保存」进入空白文档。独立 XML 含应保存的 `UOS save dialog 20261117`，不含丢弃文字。[保存提示](gui/word-save-changes.png) · [替换确认](gui/word-replace-original.png)
4. **原生另存失败**：F12 能打开 DDE 文件对话框，中文／空格文件名可输入；点击「保存」后控件持续禁用，20 秒首次复现、60 秒复核仍无目标文件。进程采样时 JoyWord 位于 `ep_poll`，DDE 对话框位于 `poll_schedule_timeout`。普通保存正常，JoyDeck 的同类 DDE 另存正常，因此不能把此异常泛化为整个 DDE 不可用，也不据此推断根因。[失败截图](gui/word-native-save-failed.png) · [精简报告](word-native-save-failure.json)
5. **系统 IME 失败**：输入 `zhongwen` 后正文直接显示拼音，未出现候选；按 `1` 后成为 `zhongwen1`。[失败截图](gui/word-ime-failed.png)

旧 `scripts/smoke_word_linux.py` 面向应用内路径输入框，不能直接驱动当前 DDE 原生另存窗口；其三轮路径超时和首次误用 Enter 确认顶部元数据条的记录只作为脚本适配问题保留在本机 `target/uos-validation-20261117/`，不计为额外产品失败。原生窗口由本轮专用脚本另行验证。

## JoyDeck 实测详情

1. 当前 R9 桌面 stdio 报告 `attachedDesktop=true`、schema 2。打开中文／空格路径 PPTX；`command.validate` 不改文稿，复制幻灯片后系统键盘撤销／重做恢复页数；保存后独立检查 `ppt/presentation.xml`，深色新进程重开一致。[打开](gui/deck-open.png) · [深色重开](gui/deck-dark-reopened.png)
2. 「文件 → 另存为」打开真实 `dde-file-dialog`，输入中文文件名并点击保存，生成三页 PPTX 且独立 ZIP／XML 读回通过。[原生另存](gui/deck-native-save.png)
3. **系统 IME 失败**：备注输入 `zhongwen` 后无候选，按 `1` 后成为 `zhongwen1`；桌面 stdio 同时确认拼音进入实际备注模型。[失败截图](gui/deck-ime-failed.png)

快捷键／坐标的探索性未触发记录不另计产品缺陷；最终原生另存从实际文件按钮完成。完整旧命令、UI 一致性和其他 Linux 显示协议不在本轮通过范围。

## JoySheet 实测详情

- 最终 13 组全部通过：Linux 不支持 GPUI `render_to_image` 的能力声明与拒绝路径、销售图表窗口、Unicode／emoji、依赖公式重算、撤销重做、合成组合态、系统剪贴板相对公式、函数补全、冻结／125% 缩放／格式表单、关闭取消、中文／空格路径 XLSX 保存关闭重开，以及多行 LF／CRLF／空行、追加编辑和完整样式保留。[销售图表](gui/sheet-sales.png) · [格式表单](gui/sheet-format-dialog.png) · [XLSX 重开](gui/sheet-roundtrip.png) · [多行重开](gui/sheet-multiline-roundtrip.png)
- 首轮在格式表单截图时报告测试进程失去前台焦点，此前 7 组通过，整轮不计通过。临时停用输入法后完整复跑 13 组通过；只记录现象，不把焦点丢失归因于输入法。[首轮报告](sheet-focus-failure.json) · [最终报告](sheet-native.json)
- **系统 IME 失败**：生产控制读回编辑器为 `zhongwen`、`composing=false`；按 `1` 后为 `zhongwen1`，提交单元格仍是该 ASCII 文本。[失败截图](gui/sheet-ime-failed.png)

## 自动检查

| 检查 | 结果 |
| --- | --- |
| `cargo xtask build joyword` | 通过 |
| `cargo xtask build joydeck --validation` | 通过 |
| `cargo xtask build joysheet --validation` | 通过 |
| `cargo xtask check joydeck` | **988 通过、0 失败**；fmt、all-targets 严格 Clippy 通过，退出码 0 |
| `cargo xtask check joysheet` | **1,439 通过、0 失败、1 项既有忽略**；fmt、all-targets 严格 Clippy 通过，退出码 0 |
| `cargo xtask check joyword` | **1,937 通过、1 失败**；fmt、严格 Clippy 通过，在 `joyword-engine` 的 `page_down_and_up_never_stay_on_a_line_in_the_continuous_views` 失败后停止，退出码 1 |
| `cargo test --locked -p joyword-ui-gpui` | **31 通过、0 失败**；用于补验本次产品 check 未运行到的桌面 UI 模块 |

构建日志为 [build.log](build.log)，产品日志为 [Deck](joydeck-check.log)、[Sheet](joysheet-check.log)、[Word](joyword-check.log)，[Word UI 补验](joyword-ui-tests.log)及[退出码摘要](check-summary.log)分别保留。Word 的失败与 2026-11-16 已登记的既有引擎导航失败同名；本次未修改该模块、断言或忽略列表，也不把 product check 未运行到的其他 Word 模块记作全量通过。

## 复跑入口

在仓库根目录、解锁的 X11 桌面中执行。`scripts/` 下是本轮现场快照，输出路径写死为 `target/uos-validation-20261117/...` 且拒绝覆盖；复跑需改用新输出名。坐标对应当前 125% 窗口，不是通用 DPI 工具。

```sh
cargo xtask build joyword
cargo xtask build joydeck --validation
cargo xtask build joysheet --validation
python3 docs/joyoffice/validation/20261117-uos-desktop/scripts/word-smoke.py
python3 docs/joyoffice/validation/20261117-uos-desktop/scripts/deck-smoke.py
python3 scripts/joysheet/smoke-native.py --binary target/debug/joysheet --output target/uos-validation-20261117/sheet-new-run
```

真实 IME 探针分别使用 `word-ime-probe.py`、`deck-ime-probe.py`、`sheet-ime-probe.py`，原生另存使用 `word-native-save.py`、`deck-native-save.py`。模板与未保存弹框使用对应快照。需要 ASCII 路径的脚本可临时停用输入法，但必须恢复原状态；IME 探针不得预先停用。

## 未关闭范围

- **UOS-IME-001 继续开放并扩展到三产品实测**：搜狗／Fcitx5 对 JoyWord、JoyDeck、JoySheet 均为拼音直通、无候选／无组合态。合成组合态和直接 Unicode 输入通过不能替代此失败项，根因与生产修复不在本轮文档任务范围。
- **JoyWord 原生另存异常开放**：普通保存与 JoyDeck 原生另存不能代替 JoyWord 的保存副本／导出对话框验收。
- Wayland、真实读屏、物理键盘／触控板、多显示器与 DPI 热切换、实体打印、完整 Office 互操作、故障与性能认证、Release 包／安装／升级／签名均未覆盖。
- Windows／macOS 本轮未复跑；ARM64、LoongArch、麒麟、国产 CPU／GPU 和其他 UOS 配置未验证。现有父任务、里程碑和发布资格保持独立状态。
