# 2026-11-27 UOS 重构进展与实机复验

本轮从当前重构源码 `d68bb22b2c39738906a0d767ff723930eed9b952` 构建和验证，工作区原先干净；验证脚本的修正发生在产品构建之后，不改变产品二进制。测试分支从最新的 `origin/main` 建立，再快进到上述重构源码。机器是 **UOS Desktop 25 Professional、x86_64、X11／DDE**，AMD Ryzen AI 9 HX 370／Radeon 890M，屏幕为 2560×1600，Xft DPI 为 120，Rust 为 1.98.1。Codex 应用本身处于显示为 Deepin 25 的玲珑容器中；本轮构建、命令和桌面窗口均由 UOS 宿主用户会话运行。源码、锁文件和六个 Debug 可执行文件的 SHA-256 见[环境记录](environment.json)。

桌面检查使用真实 UOS 会话、系统输入法、X11 合成键鼠事件或显式开启的产品测试接口，并以独立 XML／ZIP 及新进程读回核对；截图经目视检查。这是实机自动化验证，不等同于用户人工验收、物理键盘认证或完整平台资格。所有样本、配置和输出位于独立目录。[汇总数据](verification.json)

## 当前重构进度

| 产品／范围 | 当前状态与剩余边界 |
| --- | --- |
| JoyDeck | 替换计划的阶段 1–3 已按各自记录验收，阶段 4 开发清单和 Windows 验证已收口；本轮补验下述 UOS Vulkan、放映、输入及文件场景。阶段 5 的生产来源、许可原件及更多平台和图元场景仍开放，不能将阶段 5 或全部旧功能兼容标为完成。 |
| JoyWord | 校对规则、语言包索引和部分分页／共享基础已分批切换；截至路线图记录，75 个语言包中 62 个独立托管，剩余包的来源、通知和托管仍待核查。复杂分页、旧功能、正式打印和发行验收继续按任务划分。 |
| JoySheet | 公式和金融／ETS 等切片已分批替换，通用计算内核、格式对象和其他公式族仍需按范围迁移与验收；当前 UOS 窗口回归不等于 Excel 实际重算对照。 |
| 共享与整体 | 三产品使用同一宿主公共库，实际产品检查分别覆盖使用方。替换计划中 2026-11-26 的 **181–305 人周**是当时的剩余估算，本次没有重新估算，也没有足够依据折算成全项目完成百分比。 |

范围、先前的冻结基线和详细验收条件见 [Craft 替换计划](../../CRAFT-REPLACEMENT-PLAN.md)、[JoyDeck R9 交接](../../JOYDECK-R9-HANDOFF.md) 和各产品状态。此前 Windows 的 `14cbb296` 冻结参照不在本机使用；本轮跨版本对照明确使用 UOS 上留存的 `22036e02` 二进制。

## 本机构建与检查

三产品 GUI／CLI 均使用当前源码和锁文件重新构建；Deck 启用 `validation`，Sheet 启用 `automation`。下表的产品检查包括对应产品的 fmt、严格 Clippy 和 Rust 测试；共享 crate 会在不同产品的检查中重复出现，数字不能相加当作不重复测试数。

| 检查 | 通过 | 失败 | 忽略 |
| --- | ---: | ---: | ---: |
| `cargo xtask check joydeck --validation` | 1,107 | 0 | 0 |
| `cargo xtask check joysheet --validation` | 1,534 | 0 | 1 |
| `cargo xtask check joyword` | 4,222 | 0 | 2 |

JoyDeck R1–R7 的可执行文件验证合计 124 次 CLI 调用及 41 次 MCP 交互通过，覆盖旧文稿迁移、格式交换、导出、恢复和控制协议等脚本既定范围；MCP 交互次数包含异步任务轮询，会随运行变化。R6/R7 首次在 `media-inspect` 停止：脚本仍要求 Linux 报不支持，而当前 UOS 通过 FFmpeg 返回可用的视频、音频及正时长。已按实际后端和工具可用性修正脚本；原始失败保留在本机日志，修正后全脚本复跑通过。此处的媒体探测不代表已验证声音设备或视频放映窗口。[脚本修正](../../../../scripts/joydeck/verify-r6-r7.py) · [汇总](verification.json)

## UOS 桌面与输入

- 三产品分别在测试文档中使用系统搜狗输入法完成“中文”选词、第二段取消、撤销／重做、保存及新进程重开；Word 和 Deck 分别独立检查 DOCX／PPTX 内的文字，Sheet 检查 XLSX。测试前最小 GPUI 控件显示搜狗处于内部英文模式，按一次 Shift 后中文候选通过；每次测试恢复原模式、输入源和激活状态。输入过程采用服务端预编辑；没有宣称完整候选框定位或所有输入法组合通过。[Word](word-ime.png) · [Deck](deck-ime.png) · [Sheet](sheet-ime.png)
- 三产品从真实 DDE 另存面板取消后重试并保存副本。Word 验证不支持的扩展名被拒绝而草稿保留，独立读回 DOCX 且原件散列未变。Deck 和 Sheet 在面板打开时修改文档，确认过时结果不创建文件，再保存有效副本，由独立 ZIP/XML 和新进程读回，原件散列保持。文件页点击坐标在当次窗口截图上核对。Word 的普通保存与新进程重开另外由输入法场景覆盖。[另存结果](verification.json)
- JoySheet 的 13 组原生窗口冒烟通过，包括 Unicode／emoji、公式联动、系统剪贴板、撤销／重做、格式面板、关闭取消、多行输入和中文空格路径 XLSX 保存重开。该脚本部分使用生产窗口的合成输入；Linux 截图为 X11 窗口像素，不是 GPU 图像读回。[工作簿窗口](sheet-roundtrip.png)

## JoyDeck 放映、GPU 与图元

Radeon 890M 上的实际 Vulkan 计算与 CPU 参考对照 49 类效果、185 个选项，每个尺寸 925 帧：

| 尺寸 | 帧数 | 最大通道差 | 差值大于 1 的像素采样 | 首尾帧差异 | 缺失来源检查 |
| --- | ---: | ---: | ---: | ---: | --- |
| 64×36 | 925 | 1 | 0 | 0 | 48 帧，最大差 0 |
| 193×109 | 925 | 1 | 0 | 0 | 48 帧，最大差 1 |

两个尺寸均满足既有 UOS 最大通道差 2 的门槛；不能据此断言全部中间帧逐字节相等。[逐帧小尺寸](gpu-small.json) · [逐帧非整数比例](gpu-large.json)

真实观众窗口分别以 Vulkan 和 CPU 回退运行立方体、轨道、风吹和魔术升幕，采样暂停中间画面并与 CPU 参考、最终画面及退出核对；四种效果通过。每组还用旧版 `22036e02` 与当前 CLI 交叉读回写出的转场文稿。窗口截图是实际 X11 显示像素。[立方体](deck-cube.png) · [风吹](deck-wind.png) · [量化结果](verification.json)

共享图元脚本在同机用旧版和当前 Word／Deck CLI 各渲染 WMF 与 EMF 的合成三角形，四组页面场景及对应四页 PDF 栅格散列一致；两组包含设备变换、裁剪和文字的 Deck WMF／EMF 场景也与旧版逐像素一致。这些是固定样本的 CLI 场景／PDF 比较，不是所有 EMF+ 滤镜、字体或设备的覆盖。[图元结果](verification.json)

## 复跑与未覆盖项

从仓库根目录在已登录的 UOS X11 会话运行；输入法开关应先用 `scripts/probe_native_ime.py` 确认当前模式，不能对未知状态盲目切换。各输出目录必须不存在。桌面与 CLI 的临时配置须隔离；实际按钮坐标应按当前窗口重新核对。R6/R7 脚本需要含 Pillow 和 python-pptx 的 Python 环境，本轮使用 Codex 随附的 Python。

```sh
cargo xtask build joydeck --validation
cargo xtask build joysheet --validation
cargo xtask build joyword
cargo xtask check joydeck --validation
cargo xtask check joysheet --validation
cargo xtask check joyword
cargo run --locked --offline -p joydeck-render --features transition-gpu --example transition_pixels -- --gpu 64 36
cargo run --locked --offline -p joydeck-render --features transition-gpu --example transition_pixels -- --gpu 193 109
python3 scripts/verify_uos_native_ime.py deck target/debug/joydeck NEW_DIR --method com.sogou.ime.ng.fcitx5.uos --server-preedit --sogou-shift
python3 scripts/joydeck/check-transition-window.py target/debug/joydeck NEW_DIR --reference-cli OLD_UOS_CLI --prisms
python3 scripts/verify_native_save_target.py deck target/debug/joydeck NEW_DIR --x11-save-buttons 29 58 95 653
python3 scripts/joyword/probe-dde-save.py target/debug/joyword NEW_DIR
python3 scripts/joysheet/smoke-native.py --binary target/debug/joysheet --output NEW_DIR
python3 scripts/joydeck/verify-r6-r7.py --cli target/debug/joydeck-cli --evidence NEW_REPORT.json
python3 scripts/check_docs.py
```

原始日志、临时样本及未提交的配置留在本机 `target/uos-native-validation-20261127/`；仓库保存本报告、指标、逐帧数据和选定截图，不保存控制密钥或系统字体。

本轮未验证 Wayland、ARM64／LoongArch、麒麟、其他 GPU／显示器、读屏、物理打印和安装升级；未重新测试完整 UI／旧自动化矩阵、全部 49 种转场的实屏交互、复杂图元与纹理字形、Office/WPS 互读及 Release 性能。也未建立完整的阶段 4 同配置字体／Morph 对照或重新签署阶段 5 来源／许可验收。相关父任务继续开放，不能用本轮限定场景推断整个平台或重构项目完成。
