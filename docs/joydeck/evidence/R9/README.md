# R9 JoyOffice 替换与联合验证

2026-11-03。用户明确授权备份 JoyOffice 并用 0.8.0 重构替换旧 JoyDeck；当前开发入口和后续事项见 [交接](../../../joyoffice/JOYDECK-R9-HANDOFF.md)。工程替换不表示所有旧功能或完整 UI 一致性已经完成。

## 备份与输入

完整备份、bundle、逐文件校验与恢复演练见 [备份状态](backup-state.json) 和交接。旧基线 a973f75、重构输入 526ebf0；[导入输入清单](import.json) 保留进入演练前的原始指纹，后续宿主适配由 Git 与来源台账记录。先在备份旁独立 worktree 演练，通过编译/回归后才切换日常工作区。

## 当前宿主交付与验证

日常工作区已切换为 `D:\Coding\EditorProjects\joyoffice`，分支 `codex/joydeck-r9-integration`，[非草稿 PR #120](https://github.com/willmove/joyoffice/pull/120)。导入提交 05e81e2、保留原 9 个重构提交的合并 15554e1、兼容修复 f487efc；原 main a973f75 仍保留。原 joydeck-new 仅增加 AGENTS.md 停用指引，原有两份未提交日志未覆盖，不再在那里构建或继续开发。

- **Deck 当前完整产品检查：873 通过，0 失败。** fmt、严格 Clippy、来源/依赖/文档检查均通过。[宿主产品检查](host-deck-check.txt)。
- **最终联合 validation/automation：3,573 通过，0 失败，1 忽略。** 258 组测试；新增两项旧公共 PPTX 自定义部件路径回归也通过。[汇总](final-test-summary.json)、[最终联合测试](joint-final-tests.txt)、[最终 Clippy](joint-final-clippy.txt)。唯一忽略项仍是原有 Sheet release 计时测试。
- 最终再次运行 151 项通用 Python、14 项产品工程、11 项交付工具检查，全部通过；全仓 fmt、IO/分层/panic 门禁及原反例检查通过。[Python](final-python-tests.txt)、[工具](final-tool-tests.txt)、[交付工具](final-delivery-tests.txt)、[门禁](final-gates.txt)。反例输出中的错误是预期拒绝，不是放宽规则。
- 所有 268 条来源记录通过工作区校验，也逐项校验了 Git index blob，避免仅在本机 CRLF 下通过。依赖版本/来源/checksum、公共库和独立产品版本保持下述演练范围。[来源检查](../../../joyoffice/JOYDECK-R9-GATE-AUDIT.md)。

## Windows 开发包

当前 JoyDeck 包为 `dist/joydeck/joydeck-0.8.0-r9-windows-x86_64-release-dd8add7cf4.zip`，在 JoyOffice 主工作区从实际源码构建，未签名，不是正式发行。包含 GUI、CLI、三个 Windows helper、样例、来源清单、许可台账及 SBOM。

| 指纹 | SHA-256 |
| --- | --- |
| ZIP | `4b3b87d1432384e8dac01b3c3280a1280eb8856db0dfe35256b5fe09ae4dc271` |
| 源码快照 | `dd8add7cf47351bf069de289a56065b4569afac963775faa1b0bca58973ff60b` |
| GUI | `876c0a08c520eebd196830b11669e6dd95200cde58a32f4af6386c69dfd70e3d` |
| CLI | `8c1b2edfe691f832e63d5b86aad637edc8630d082b8bf5a2be584d83ff5563a8` |

[构建](package-final-build.txt)、[包元数据](package.json)、[独立解压检查](host-verification.json)。ZIP CRC、包内 **1,753 项 SHA-256** 全部通过；包在仓库外中文/空格路径解压运行，GUI 使用默认 **1 MiB PE 栈**。宿主 Windows 单归档元数据/可执行文件检查也通过：[归档契约](archive-contract.txt)。没有把单 Windows 包称为四平台正式 release 矩阵。

Word debug ZIP 与 Sheet debug ZIP/源码 ZIP/NSIS 安装器也已从 JoyOffice 构建；版本仍为 Word 0.1.1、Sheet 0.1.0。独立解压后的两产品 CLI 读取已保存 DOCX/XLSX 通过，GUI 重新打开正确；Sheet 1,243 项 payload 校验通过。Sheet 初次因缺 NSIS 失败，随后使用 [官方便携 NSIS 3.11](https://sourceforge.net/projects/nsis/files/NSIS%203/3.11/nsis-3.11.zip/download)（SHA 经校验）成功构建，未安装到系统，也未运行生成的安装器。[包记录](other-packages.json)、[Word 构建](package-word.txt)、[Sheet 成功重试](package-sheet-retry.txt)、[工具来源](nsis-tooling.json)。这些是联合回归开发包，不是三产品同时发布。

开发包的许可缺项仍如 [package.json](package.json) 中 missing_license_texts 所列；cargo-deny 通过不能替代许可全文齐备、签名/公证和正式再分发审阅，归属 JD-807/808。

## 最终包 CLI、旧文件与原生窗口

以下全部使用最终包解压后的 CLI，未复用 R8 或首轮 R9 二进制作为新证据：

| 回归 | 实际结果 | 证据 |
| --- | --- | --- |
| R1 读取/渲染/错误拒绝 | 28 次进程调用通过 | [R1](cli-r1.json) |
| R2 编辑/保存/崩溃恢复 | 20 次命令通过 | [R2](cli-r2.json) |
| R3 文字/表格/备注/独立公共读取 | 20 次调用通过 | [R3](cli-r3.json) |
| R4/R5 场景/缓存/主题图片图表/交换 | 23 次调用通过 | [R4/R5](cli-r4-r5.json) |
| R6/R7 迁移/输出/放映/控制 | 32 次 CLI、41 个 MCP 响应通过 | [R6/R7](cli-r6-r7.json) |
| 原宿主语料 | native-20 迁移后 20 页，原 .joydeck 附件逐字节一致；五个旧公共/PowerPoint 文稿读取及首页渲染；原件 hash 不变 | [宿主语料](host-verification.json)、[可复现实验](verify-host.py) |

本轮语料回归发现并修复了旧公共 PPTX 的 `slideset/main.xml` 等非惯用部件路径问题：读取和保留式写回均沿 OPC 根 officeDocument 关系定位；未改母版时不要求最简文稿具有母版关系；复制页面重定位内部关系；合法的无资源页面不要求 .rels，损坏的已有 .rels 仍拒绝。[专门回归](r9-custom-path-tests.txt)。

Windows 原生验收记录在 [汇总](native-summary.json) 和 gui/ 中；界面通过 Computer Use 实际操作，截图同时登记包与 EXE 指纹：

- 最终 JoyDeck 的 [载入](gui/12-deck-final-loaded.jpg)、[原位中文文字](gui/13-deck-final-inline.jpg)、[撤销](gui/14-deck-final-undo.jpg)、[重做](gui/15-deck-final-redo.jpg)、[原生另存完成](gui/16-deck-final-saved.jpg)、[退出后重开](gui/17-deck-final-reopened.jpg)、[F5 放映](gui/18-deck-final-show.jpg)、[翻页](gui/19-deck-final-show-next.jpg)及 Esc 返回。文字和额外 Ctrl+Enter 换段分别撤销，重做文字后另存；没有把它记成一次撤销动作。
- 独立 python-pptx 重读桌面另存结果：三页、标题准确，只有 `ppt/slides/slide1.xml` 改变，其他原始成员内容逐字节一致，原稿未变。[公共读取](gui-deck-public-read.json)、[PPTX 产物](artifacts/desktop-saved.pptx)。这是本次文本输入验证，没有重新宣称系统 IME 组合输入、多屏或全部设备资格已通过。
- Word 实际 DOCX 输入、撤销和保护提示下另存副本；独立文本与原件相同，包内重开 [85 字原创测试正文](gui/09-word-package.jpg)。便携重开通过 JOYWORD_RECOVERY_DIR 隔离测试恢复目录；首次误拾共享恢复日志的窗口不作为重开验收。另存副本不清除原会话 dirty，未确认覆盖原件。
- Sheet B1 输入 `=1+2` 得 3，撤销/重做、原生另存 XLSX；独立 XML 确认公式/值及两幅图表，便携包 [重开](gui/10-sheet-package.jpg) 正确。[Word/Sheet 文件检查](host-verification.json)、[保存产物](artifacts/)。

## 可运行回退与备份恢复

除完整仓库恢复演练，另从原 bundle 的干净 clone 构建了旧 JoyDeck debug 包：

`D:\Coding\EditorProjects\a-backups\joyoffice-before-joydeck-r9-20261103-100321\rollback-runtime\joydeck-pre-r9-a973f75-windows-amd64-debug.zip`

SHA-256：`f01230240370144a0dbc58cfde40b7b357b38ec02c490be968a151ab065d7453`。包内 965 项 hash、CLI 原格式读取、仓库外 [旧桌面打开 20 页文稿](gui/11-old-deck-rollback.jpg)通过。[构建](rollback-package-build.txt)、[包记录](other-packages.json)、[复现实验](verify-other-packages.py)。未改备份的 repository/；baseline-runtime-build 仅是额外的恢复构建 clone。

回退使用原件或明确导出的公共副本；新 v2 文稿不能交给旧 reader 自动降级覆盖。迁移包中的原始附件与警告保留，旧软件包的存在不表示新软件已经继承所有旧功能。

## PR、CI 与后续

PR #120 是非草稿；最终状态以 [实时检查](https://github.com/willmove/joyoffice/pull/120/checks)为准，由用户/协调者合并，未自动改远端 main。提交本报告时多平台 CI 正在运行，不能把本机 Windows 验收写成远端全部通过。

初次 CI/打包的失败记录保留：跨平台换行指纹、非 Windows helper 的 unused_mut、离线检查前未 fetch、Linux 缺 CJK 字形，以及 QEMU 子进程未继承 sysroot。已分别改为精确 LF、cfg 内可变绑定、锁定图预取、安装 Noto CJK 和 QEMU_LD_PREFIX；没有禁用相关测试/断言或放宽门禁。QEMU 只是模拟器，不代表信创真机资格。

后续 Word CI 的产品测试通过后，独立实验在 `--locked` 阶段发现已有锁漂移；在原 a973f75 的额外 clone 上同样复现。两个实验锁仅补齐原已存在的 joyword-rtf / joyword-textfmt 本地路径依赖，所有 registry 版本/来源/checksum 不变。[锁审阅](experiment-locks.json)、[恢复实验](experiment-recovery-tests.txt)、[严格 Clippy](experiment-recovery-clippy.txt)、[DOCX 实验](experiment-robustness-tests.txt)、[严格 Clippy](experiment-robustness-clippy.txt)、[驱动工具](experiment-robustness-python.txt)、[固定语料](experiment-robustness-smoke.txt)、[200 次变异](experiment-robustness-fuzz.txt)。它们没有进入 Deck 二进制的构建输入，当前包源码指纹仍完全匹配。

独立实验合计 10 项 Rust 测试通过；驱动工具 23 通过、3 项 POSIX 专用检查在 Windows 跳过；固定矩阵执行 81 项，变异执行 200 项。驱动没有 Windows Job Object 内存限制后端，不能把此次变异称为该平台内存/完整安全资格通过。

JD-900–904 工程条件已完成；JD-905/907 已实现待验收；JD-906 的完整旧功能/命令兼容与 JD-908 最终 PR/CI/合入仍开放。下一会话只在 JoyOffice 继续，先读 [交接](../../../joyoffice/JOYDECK-R9-HANDOFF.md)，优先 UI/日常编辑与旧命令契约缺项。完整 UI/功能一致、全部旧功能无回退、实际 GPU/RSS 性能、其他平台真机、安装升级/签名等，不由本次替换冒充完成。

## 替换前演练检查（历史过程）

- Deck headless 869 项、桌面 870 项通过；随后新增的 R9 旧命令拒绝/能力协商回归已在联合测试通过。
- Word 1,316 项通过；Sheet 1,396 项通过、1 项原有计时测试忽略。分产品检查包括 fmt 与严格 Clippy，见 [产品汇总](product-tests.json)。
- 全 workspace 的 validation/automation 联合 Clippy 通过，Rust **3,571 通过、0 失败、1 忽略**，258 组；忽略项为 Sheet recalc_with_range_formulas_scales_linearly，要求 release 中单独计时。AI feature 启动不发 provider 请求的测试通过。[联合汇总](joint-summary.json)、[联合测试](joint-tests.txt)、[Clippy](joint-clippy.txt)。
- 六个桌面/CLI 二进制独立构建成功。[构建](three-products-build.txt)。
- 151 项通用 Python、14 项产品工程工具、11 项交付工具通过；原门禁反例仍被拒绝。
- cargo-deny 0.20.2 官方资产 sidecar 校验后运行，advisories/bans/licenses/sources 均通过；既有忽略规则未新增。[依赖审计](dependency-audit.txt)、[工具来源](tooling.json)、[锁差异](lock-delta.json)。许可检查通过不等于正式再分发资格或全部许可全文齐备。
- 来源与依赖/CLI 隔离、宿主及产品文档检查通过。[分层/IO/断言审阅](../../../joyoffice/JOYDECK-R9-GATE-AUDIT.md)。

上述 869/870/3,571 数字保留演练当时的含义；新增兼容回归后的当前宿主结果为 873/3,573，以本报告前文为准。
