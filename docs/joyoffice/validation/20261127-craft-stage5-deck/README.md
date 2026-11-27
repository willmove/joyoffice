# JoyDeck 阶段 5 生产审计与回退验收

2026-11-27，在 Windows 11 x64 上从 `5e2bfb86` 开始验证。冻结参照仍为 `14cbb29616d7c0ababbf40a8f6c0218b996e7ae8`，回退演练使用与冻结记录散列相同的二进制。[冻结记录](../20261124-craft-pilots/verification.json) · [验证数据](regression.json)

## Windows 独立打印

直接启动由产品脚本构建的 WinForms 打印助手。Windows 打印选择窗显示 Microsoft Print to PDF 和横向纸张。取消选择窗返回 `cancelled` 和退出码 2。点击打印能进入虚拟驱动自己的保存窗口。原助手在取消这个窗口后仍返回 `submitted`，探针确认 `EndPrint.Cancel` 为真且没有页面回调。修复后在相同窗口取消也返回 `cancelled` 和退出码 2。[Windows 助手](../../../../crates/joydeck-shell/native/print-windows.cs)

打包 CLI 的正常打印命令也能打开独立选择窗，取消后 CLI 收到 `cancelled`。本轮用生产助手的分页回调和 Microsoft Print to PDF 在隔离目录产出 3 张 A4 横向页面。Poppler 栅格与阶段 4 的既有虚拟打印件逐页配对，匹配页像素完全相同，页序符合本轮输入。驱动保存窗的文件名控件无法由本机窗口自动化直接接管，成功写盘的分页验证使用显式输出路径。macOS Metal 和 UOS Vulkan 待可用实机补验。[阶段 4 打印记录](../20261126-craft-stage4-deck-output/README.md)

## 生产中的 DeckCraft 来源

将 195 条 DeckCraft 来源记录与 Windows 桌面、CLI 默认构建的 Rust dep-info 交叉。桌面编译 119 个带来源记录的产品及共享 Rust 文件，CLI 编译其中 118 个，另有一个两入口共用的 Cargo 构建脚本。多个文件已经部分或全部替换，文件归属和实际调用核对同时保留。桌面的 `chrome.rs` 合并了三条上游记录，按一个文件计数。[来源账本](../../../../licenses/deckcraft/file-map.json) · [逐文件审计](audit.json)

| 当前生产链路 | 带来源记录的模块和保留分支 |
| --- | --- |
| 模型、形状、主题 | `joydeck-model` 的演示、母版、版式及形状 DTO，图表与趋势线计算。 `joydeck-color` 的颜色与主题枚举兼容表示。 `joydeck-geom` 的路径命中和历史预设、文字变形适配。 |
| 字体与文本 | `joydeck-fonts` 的字体面、脚本分类和嵌入字体输入适配。 `joydeck-text` 的行距、制表位、缩排及行内样式配置。桌面字形命中、CLI 光栅和输出均经过这些模块。 |
| 动画与画布 | `joydeck-anim` 的运动路径、效果选项和网格支持。 `joydeck-render` 的对象绘制、图表、SmartArt、3D、内嵌图像和文本适配。GPU 转场和 CPU 回退调用同一演示模型。 |
| WMF／EMF | `joydeck-metafile` 的记录解释、设备坐标、位图、画笔、文字和裁剪。Word 与 Deck 共用 `joyoffice-metafile` 中适配的定长字段读取。 |
| 原生与 PPTX 文件 | `joydeck-format` 的原生模型读取与大纲等入口。 `joydeck-pptx` 的剩余图表、过渡、母版占位符、表格、媒体关系和保留路径分支。桌面与 CLI 运行同一读写实现。 |
| PDF 与打印 | `joydeck-pdf` 的备注与讲义框、继承文本样式、链接等适配分支。PDF 后端使用固定的 Krilla 0.8.2。Windows 打印通过共享幻灯片场景生成页面位图，交给系统驱动。 |
| 桌面 | `joydeck-ui-gpui/src/chrome.rs` 的界面框架和控件呈现仍有来源映射，仅由桌面入口编译。 |

`joydeck-fonts/build.rs` 的 Craft 来源单列为构建输入。入口显式拒绝 `CRAFT_FONTS_DIR`。生产内嵌字体为已登记的 Ubuntu Light，Hack Regular 用于测试。`joydeck-legacy` 通过 JoyOffice 来源记录追踪，其调用限于旧文稿只读迁移。映射中同时保留已退役条目的历史散列、上游测试存档与清单。[产品署名](../../../../THIRD_PARTY-JOYDECK.md) · [JoyOffice 来源](../../../../licenses/joyoffice/source-map.json) · [字体记录](../../../../licenses/fonts/sources.json)

## 依赖和许可

对 Windows x64、macOS arm64、Linux x64／arm64 的桌面及 CLI 共八张锁定依赖图核对普通／构建边，Craft crate 数均为 0。固定依赖包括 GPUI Kit 0.9.1、桌面 GPUI pre 0.3.12、Linux 本地 GPUI 补丁和注册表 Krilla 0.8.2。Windows Debug 开发包的 SPDX 包含 491 项依赖，校验 1,975 个 ZIP 成员、原生助手源码和二进制指纹、DeckCraft 原 MIT／Apache 正文、NOTICE 与归属账本。macOS arm64 源码依赖图含 498 项，Linux x64／arm64 分别 575／574 项，均以本地锁文件生成许可清单。[依赖及包审计](audit.json) · [复跑脚本](audit.py)

Windows 包的许可文件存在性检查无缺项。`mac@0.1.1` 和 `seahash@4.1.0` 仍缺可追溯的独立上游许可正文，随包记录保留发行声明和明确标记的 Apache-2.0／MIT 标准正文。复核发现两份本地声明的记录散列不符，已修正记录。另将 `seahash` 标准文本恢复为历史导入的确切字节，匹配其原 SHA-256。macOS 和 Linux 的 `pathfinder_geometry@0.5.1` 使用进 Git 的上游项目许可，来源提交与发布归档的 VCS 提交不同，仍保留这项精确版本限制。[mac 记录](../../../joysheet/third-party/mac/SOURCE.json) · [seahash 记录](../../../joysheet/third-party/seahash/SOURCE.json) · [Pathfinder 记录](../../../joysheet/third-party/pathfinder_geometry/0.5.1/SOURCE.json)

## 冻结回归

| 验收项 | 结果 |
| --- | --- |
| 工程与入口 | `cargo xtask build joydeck`、`cargo xtask check joydeck` 通过。1,073 项 Rust 测试通过，fmt 和严格 Clippy 通过。R1–R7 共 126 项 CLI 检查通过。共享脚本 200 项通过、13 项跳过，JoySheet 打包 10 项通过。清理了媒体引用重构留下的过期 panic 基线记录。 |
| 原生、PPTX 和转场交换 | 17 个文稿、100 份写出文件、234 次模型读回通过。签名／宏保留写回与 7 类不受支持输入的拒绝行为通过。48 个转场选项完成 192 次互读。 |
| PowerPoint 16 回存 | 形状与文本、对象动画两份文稿的读回模型一致。SmartArt 的右内边距差为 `2.84e-14`，页面栅格完全相同。3D 样本在冻结版读取为海报图，候选版读出模型、GLB 和海报。四份文件均由 PowerPoint 保存副本并重新打开。 |
| 字体、排版、放映 | 60 组排版、4,904 个字形命中与冻结参照逐项一致。放映分别重跑已跟踪的样本和额外 PowerPoint 16 文件，触发与状态一致。每次对照中 38 个画面有 35 个完全一致，3 个差异是阶段 4 已记录的 Morph 起始帧修正。 |
| GPU／CPU | Windows Intel Arc Vulkan 的 64×36 与 193×109 各 925 帧。CPU 散列与冻结版一致，GPU 最大通道差为 1，首尾帧完全一致。 |
| PDF 和图元 | 7 组输出、14 页。文字、字体、页框、链接、书签及 WMF／EMF 场景通过。三页与六页讲义保留阶段 4 已记录的两处首张页脚修正。 |

`67b9f3e7` 起可读取 M9 饼图的百分比标签。交换脚本在两个固定图表样本的 12 次比较中逐项检查这六个新增字段，其余内容仍精确比较。UTF-16 DTD 样本保持拒绝，候选版给出明确的 DTD 诊断。[PPTX 复跑](exchange.py) · [详细结果](regression.json)

## 回退操作

在独立目录解开并校验候选开发包，再将冻结版桌面和 CLI 二进制复制到另一个目录，核对阶段 0 的已知散列。用独立用户配置和中文空格路径下的测试副本，依次通过候选、冻结、候选 CLI 编辑同一标题并撤销重做，写出 `.joydeck → .pptx → .joydeck`。每个写出文件分别由两版重开核对对象 ID、页序、内容、媒体、母版、版式与未知 XML。随后用两版原生桌面分别打开三页测试副本，界面观察到三页标题且文件散列保持一致。冻结阶段归档提供二进制对，演练使用它作为文件级回退版本。[复跑脚本](rollback.py) · [回退数据](rollback.json)

回退时先归档原件和恢复目录，再用独立版本读取副本。冻结版读取的饼图模型缺少百分比标签属性。对未编辑的 M9 图表，冻结版保留另存和重新生成均保留原图表内容，候选版重新打开后读得百分比标签。3D 样本使用冻结版保留另存时，PPTX 部件逐字节一致。冻结版重新生成仅写出海报图，GLB 丢失。含 3D 模型的文稿回退应保留原件并使用保留另存。演练覆盖独立目录的二进制与文档换代。已安装版本、更新器和系统关联未参与演练。

阶段 5 继续开放的项目包括生产可达旧实现的进一步替换、两项独立许可原件和 Pathfinder 精确版本溯源，以及可用设备上的 macOS Metal 和 UOS Vulkan 补验。原始日志、构建产物与对照文件位于本机 `backups/craft-stage5-deck-20261127/`。Windows 包为未签名开发包。

## 复跑

以下命令在本轮仓库根目录和 PowerShell 7 下执行，回归使用 `14cbb296` 冻结归档。演练和交换脚本输出到各自的新目录。

```powershell
$checks = 'docs/joyoffice/validation/20261127-craft-stage5-deck'
$archive = 'dist/joydeck/joydeck-stage5-20261127-windows-x64-verified.zip'
$frozen = 'backups/craft-replacement-20261124/14cbb296'
$candidate = 'dist/joydeck/joydeck-stage5-20261127-windows-x64-verified/joydeck-cli.exe'
py -3 "$checks/audit.py" $archive target/x86_64-pc-windows-msvc/debug target/deck-audit.json
py -3 "$checks/rollback.py" $archive $frozen target/deck-rollback-recheck
py -3 "$checks/exchange.py" target/deck-exchange-recheck --frozen "$frozen/joydeck-cli.exe" --candidate $candidate
```
