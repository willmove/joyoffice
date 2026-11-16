# GPUI Kit 0.9.1 本机验证

2026-11-16，`feat/gpui-kit-0.9.1`，起点 `main@1c189c62f7eae382e4f095b49670090d1891280e`。本轮将当前三产品及公共 UI 的 Kit 家族统一为 0.9.1，Pre 快照统一为 0.3.12，保留默认后端和 AccessKit Windows 0.34.0 补丁。实现、依赖和来源审查见[升级记录](../../GPUI-KIT-0.9.1-UPGRADE.md)。

宿主为 UOS Desktop 25 Professional x86_64，X11，125% 显示缩放；执行环境为玲珑 Deepin 25，Rust 1.98.1。使用宿主现有 Git/GCC/CMake 和临时 sysroot 包装，不向仓库加入环境专用构建依赖。完整命令、退出码、日志 SHA-256、锁差异及二进制指纹见 [verification.json](verification.json)。

## 自动验证

| 范围 | 结果 |
| --- | --- |
| 全仓 fmt、联合 feature 生产 lib/bin 严格 Clippy | 通过 |
| Word/Deck/Sheet 桌面与 CLI 构建 | 全部通过；Deck validation、Sheet automation 构建通过 |
| 联合桌面 check 与 Word/Deck runtime-shaders check | 通过 |
| Deck 产品 check，含 all-targets Clippy | 988 项通过 |
| Sheet 产品 check，含 all-targets Clippy 和 automation | 1,439 项通过，一项既有忽略 |
| Word 产品 check | 生产 Clippy 通过；1,933 项通过后遇到下述引擎失败，后续模块由联合测试覆盖 |
| workspace 联合测试，含 Word Agent / Deck validation / Sheet automation，no-fail-fast | 6,057 通过、6 失败、3 既有忽略；三个 UI crate 的 69 项全部通过 |
| Python | 根脚本 196（两项平台跳过）、Deck 14、产品工具 16、Sheet 打包 7，均通过 |
| 架构/副作用/生产 panic、文档、来源指纹及版本门禁 | 通过；291 条导入来源指纹；三个 CLI 的 normal/build 闭包无 GPUI |
| GPUI Pre provenance | 22 个包通过；17 个相同、4 个快照包的 12 处发布适配已审、1 个 reqwest 改名参照包 |
| cargo-deny 0.20.2 | advisories / bans / licenses / sources 通过；既有 async-executor 1.14.0 yanked 警告仍在，无新增忽略 |

[逐文件来源结果](provenance.log)、[已审差异](source-review.diff)和[根锁差异](lock-delta.json)保留独立证据。已有 ICU 等版本保持，新增 segmenter 固定为上游声明的 2.3.0 基线；未运行无约束的全仓依赖升级。历史实验与迁移快照保持原版本。

## 六项基线失败

| crate | 测试 |
| --- | --- |
| joyword-engine | `page_down_and_up_never_stay_on_a_line_in_the_continuous_views` |
| joyword-layout | `a_symbol_bullet_sizes_its_line_by_the_symbol_font` |
| joyword-layout | `justified_lines_shrink_spaces_in_word_2013_mode` |
| joyword-layout | `last_rewrap_pass_keeps_early_floats_clear_of_text` |
| joyword-layout | `legacy_form_fields_are_drawn` |
| joyword-layout | `long_footnotes_continue_on_the_next_page` |

在升级前 `1c189c62` 的独立只读工作树中，使用原 Kit 0.8.1 / Pre 0.3.10 锁文件，分别执行引擎单项和完整 layout 单测；失败名称及断言与升级后相同。layout 为 580 通过、5 失败。见[引擎基线日志](baseline-engine.log)及[排版基线日志](baseline-layout.log)。临时工作树验证后已移除，日志及原锁指纹保留。

本轮未修改这些模块、降低断言或增加忽略；全量测试退出非零，不能记为全绿。更早文档中的其他分页失败不代替本次现场结果。

## 本机实际窗口

- Word：5 组通过。中文/空格路径打开、系统合成键盘输入及撤销重做、查找输入框、保存及独立 OOXML 读回、新进程重开。保存测试副本时出现既有元数据保护确认，首次脚本未完成该确认，未计为保存通过；最终显式确认后复验通过。原语料散列不变。[原生报告](word-native.json)、[现场脚本快照](word-smoke.py)、[查找框](gui/word-search.png)、[保存保护](gui/word-metadata-warning.png)、[重开](gui/word-reopened.png)。
- Deck：5 组通过。使用当前 R9 桌面 stdio 和 schema 2：验证不改文稿、复制幻灯片、系统键盘撤销重做、PPTX 保存及独立页数读回、深色新进程重开。没有使用已替换实现的 M0–M4 冒烟入口。[原生报告](deck-native.json)、[现场脚本快照](deck-smoke.py)、[浅色](gui/deck-light.png)、[深色重开](gui/deck-dark-reopened.png)。
- Sheet：13 组通过。执行现有 `scripts/joysheet/smoke-native.py`，覆盖销售图表、Unicode/emoji、公式、撤销重做、合成组合态、剪贴板、函数补全、格式输入框、取消关闭、XLSX 保存重开，以及多行 LF/CRLF/空行和样式保留。Linux 不支持原生 GPUI readback 的能力声明与拒绝路径同时通过；截图来自实际 X11 窗口像素。[原生报告](sheet-native.json)、[图表](gui/sheet-sales.png)、[格式输入框](gui/sheet-format-dialog.png)。

Word/Deck 现场脚本从仓库根目录执行，依赖 X11/xdotool/ImageMagick，输出 `target/gpui-kit-091/*-native` 且拒绝覆盖。Word 确认按钮坐标对应本次 125% 窗口配置；脚本是本次证据快照，不是通用 DPI 认证工具。原生报告中的系统合成键盘、生产 GPUI 合成事件及窗口像素来源分别保留；未将其称为物理输入或完整 IME 验收。

## 保留的边界

本轮验证为本机 Debug 构建，不生成发布安装包，不执行远端 CI、Windows/macOS 原生、真实中文 IME/读屏、多显示器、完整 DPI 矩阵、物理打印或呈现延迟测量。AccessKit Windows 补丁没有移除，未冒充新版本 Windows UIA 验收。信创真机留待有条件时补验；各产品原有功能、迁移和发行父任务保持独立验收状态。
