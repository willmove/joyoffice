# 2026-11-23 UOS 重构进展与实机复验

## 源码与环境

本轮验证源码为 `22036e0209a38156748c5c6c61682f6c23519360`，统一版本 0.2.0。开始时工作区干净，当前重构分支 `feat/refactor-native-roadmap` 与远端一致，包含最新 `origin/main`（`32562303`）并领先 16 个提交。验证分支为 `feat/uos-refactor-validation-20261123`，没有改写既有提交。

实际主机为 UOS Desktop 25 Professional（snipe）、x86_64、AMD Ryzen AI 9 HX 370 / Radeon 890M，内核 `6.18.57-061857-generic`，Rust/Cargo 1.98.1、Python 3.12.13。已登录 X11 / DDE 桌面，显示器 2560×1600、144 Hz；DDE 配置 `scale-factor=1.0`、`window-scale=1`，但 Xft 为 120 DPI，应用报告 `scaleFactor=1.25`，本轮按实际 125% 记录。输入法为现有 Fcitx5 搜狗 `com.sogou.ime.ng.fcitx5.uos`。

Codex 应用运行时的 `/etc/os-release` 为 Deepin 25；构建和桌面测试经用户会话的 `systemd-run --user --wait --pipe --collect --working-directory=/home/Wilmer/Coding/joyoffice /bin/bash -lc 'COMMAND'` 在 UOS 宿主执行。没有使用容器、CI 或 QEMU 的结果替代实机记录。使用 Debug 构建，Deck 开启 `validation`、Sheet 开启 `automation`；本轮不提供 Release 性能结论。

## 重构进展判断

| 范围 | 当前实现 | 尚未关闭的范围 |
| --- | --- | --- |
| Monorepo 与公共桌面基础 | 三产品共享 workspace、统一版本、GPUI Kit 0.9.1 / Pre 0.3.12；共享保存提示、OPC/ZIP 预算读取、字体遍历、部分 DrawingML/CFB/OMML 和 Ribbon 呈现 | 产品语义仍各自维护；完整 UI 等价、安装关联与各平台资格分别验收 |
| Word | 分节、跨段编辑、字段、注释等已接入有界局部重排，超预算或出口不确定时完整分页；校对库与语言包存储纳入第一方维护，四个 `craft-*` 运行时依赖退役 | 复杂分页回退路径继续保留；第一方维护和算法替换不等于撤销原有来源与词典数据许可；旧功能、完整桌面/发布矩阵继续开放 |
| Deck | 49 种 PresentationML 转场已有第一方生成器；183 项预设图形及文件连接线已有独立实现 | 转场替换完成不等于 DeckCraft/PowerPoint 全功能和界面对齐完成；旧格式只读迁移桥仍保留 |
| Sheet | 文本/日期登记名、条件聚合、矩阵、复数/贝塞尔/折旧等已迁移；三种 Excel 格式逐项共用视图、尺寸、打印、保护和超链接映射 | 概率分布、ETS、证券/货币时间价值、工程单位换算等公式长尾，以及其余文件对象映射仍待推进；Excel 实时重算与设备打印不得由缓存或保存往返替代 |

Deck 的当前任务账本共有 98 项：完成 41、进行中 50、已实现待验收 4、待开始 3。R0–R2 的 28 项均完成；R4–R8 仍有大量未关闭项。该统计仅描述任务账本，不作为自研代码比例或平台完成率。本轮实机切片通过也不改变整项跨平台条件。

进展以 [重构路线图](../../REFACTOR-ROADMAP.md)、[Word 状态](../../../joyword/r8/STATUS.md)、[Deck 任务](../../../joydeck/TASKS.md)、[界面对照](../../../joydeck/UI-PARITY.md) 和 [Sheet 迁移矩阵](../../../joysheet/MIGRATION.md) 为依据；旧 M* 及迁移前证据保留其历史范围。

## 自动检查与构建

| 当前产品工程命令 | 通过 | 失败 | 既有忽略 | fmt / 严格 Clippy |
| --- | ---: | ---: | ---: | --- |
| `cargo xtask check joyword` | 4,211 | 0 | 2 | 通过 |
| `cargo xtask check joydeck` | 1,041 | 0 | 0 | 通过 |
| `cargo xtask check joysheet` | 1,524 | 0 | 1 | 通过 |

计数包含各产品重复运行的共享库测试，不能相加当作独立用例数。Word 忽略的是恢复体积测量与需本地授权 `kaiu.ttf` 的轮廓测试；Sheet 保留原有 Release 时序测试忽略。Word Clippy 沿用工程命令的 lib/bins 范围，Deck/Sheet 为 all-targets。

三个产品的桌面和 CLI 构建均通过；Deck 开启 `--validation`，Sheet 开启 `--validation`（映射到 `automation`）。另外运行校对 485、规则 12、语言包存储 22 项定向测试，均通过；它们与上述产品检查重叠。工程脚本单元测试运行 211 项，209 通过、2 跳过。Deck 的 292 份来源指纹、精确依赖/CLI 隔离检查和根工程门禁通过。[工程命令及日志摘要](engineering.json) · [完整结果索引](verification.json) · [原始日志散列](logs.json)

从当前 `origin/main=32562303` 的 Git 归档构建了三个旧 CLI，与当前六个应用分别保存二进制 SHA-256。六应用 `cargo tree --locked --offline -e normal,build` 均无 `craft-*` 依赖边；本轮没有重新制作 SBOM 或作新发行许可合规结论。[两版二进制](binaries.json) · [基线构建](baseline.json) · [依赖闭包](dependency-closure.json)

## UOS 实机自动化

本节在已登录的真实 X11 桌面运行生产窗口和实际 GPU。操作分别使用 X11 系统键鼠事件或显式启用的产品控制协议；文件另由 CLI、ZIP/XML 或 PDF 栅格读回。它们是实机自动化证据，不等于用户人工签署、物理键盘认证或完整平台验收。截图由 Agent 目视核对。

### Word

- 分节边界及浮动绘图重锚：14 步、956 页场景比较、2,868 次命中比较，0 次整篇回退。保留 Debug 原始采样，不据此宣称 Release 性能提升。[完整分页对照](section-oracle.json)
- 原生窗口插入跨段正文、撤销、重做均生成局部布局补丁；四类分节符经 MCP 写入并保存。两版对同一 69 页文件生成的 PDF 栅格一致，DOCX 再存散列一致，原始语料未变。[桌面与两版读回](section-splice.json)
- 合成语言包安装、替换、卸载、关闭网络同意时拒绝刷新、篡改归档拒绝安装均通过。桌面与 CLI 页数一致，安装整词例外后 20→24 页，改为音节例外及卸载后回到 20 页。固定发行索引的 75 项清单及荷兰语包的散列、许可、词典和断词数据另经实际下载、安装与卸载核验。[语言包实测](language-pack.json)

### Deck

| 实际 Vulkan / CPU 比较 | 帧数 | 最大通道差 | 差值 >1 的像素采样数 | 差值 >2 | 起止帧差异 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 64×36 | 925 | 2 | 73 | 0 | 0 |
| 193×109 | 925 | 2 | 491 | 0 | 0 |

后端确认为 `Vulkan / AMD Radeon 890M Graphics (RADV GFX1150)`。每种尺寸另做 48 次缺失来源检查，最大通道差 1；所有结果满足既有最大差 2 的门槛。该结果不同于 macOS 原记录中的最大差 1，不能写成中间帧逐字节相等。[小尺寸逐帧](gpu-small.json) · [非整数比例逐帧](gpu-large.json)

剩余 17 类、48 个文件选项经两版 CLI 写入、再分别读回与重新生成，共 192 组，核对转场元素、方向、图案、反弹、黑场等属性及持续时间，原件散列未变。这里验证文件契约，没有在 UOS 启动 PowerPoint 重新导出参照。[完整矩阵](remaining-transition-exchange.json) · [可复跑脚本](scripts/transition-exchange.py)

真实放映窗口分别以 Vulkan 和 CPU 回退运行 `cube/l`、`orbit/u`、`glitter/r`、`vortex/l`，暂停后按实际时钟与 CLI CPU 参考取样比较，再确认终态和关闭。四种效果通过，不冒充全部 49 类都在全屏窗口逐一人工验收。[立方体与轨道](deck-prisms.json) · [闪光与漩涡](deck-new-transitions.json) · [实际放映画面](deck-cube.png)

### Sheet

| 公式切片 | 工作簿 | 每份公式缓存数 | 原生联动变化格数 |
| --- | ---: | ---: | ---: |
| 条件聚合 | 2 | 104 | 17 |
| 矩阵与经典文本 | 2 | 59 | 22 |
| 现代文本与日历日期 | 2 | 59 | 21 |
| 工程、折旧与特殊函数 | 2 | 80 | 30 |

八份规范工作簿共 604 个公式缓存，当前版与规范期望零差异，并完成两版交叉读回。原生窗口各自完成编辑、撤销、重做、保存及新进程重开。缓存/规范对照与真实 Excel 实时重算分开记录，后者本轮未运行。[条件聚合](conditional-aggregates.json) · [矩阵与经典文本](matrix-classic-text.json) · [现代文本与日期](modern-text-date.json) · [工程折旧](engineering-depreciation.json)

| 三格式导入切片 | 原件数 | 两版读写组合 | 原生验证 |
| --- | ---: | ---: | --- |
| 冻结窗格、选区、滚动原点 | 12 | 48 | 编辑、历史、多区选择、保存、重开 |
| 显示、视图、缩放、汇总方向 | 9 | 36 | 修改显示/缩放、历史、保存、重开 |
| 默认尺寸、隐藏与分级大纲 | 6 | 24 | 分组、历史、保存、重开 |
| 打印设置、页边距及分页符 | 6 | 24 | 页面设置、历史、保存、重开 |
| 保护、密码及超链接 | 6 | 24 | 错密拒绝、解锁/加链/加锁、历史、保存、重开 |

共 39 份 XLS/XLSB/XLSX 输入、156 组读写组合。输出通过 XLSX 保存并由两代 CLI 读回，不表示新增了 XLS/XLSB 写出能力。原件散列均保持。打印设置验证不替代物理打印；透视表预算边界由当前 crate 自动测试覆盖，没有新增真实大透视表 UI 场景。[窗格](sheet-panes.json) · [显示](sheet-display.json) · [尺寸](sheet-dims.json) · [打印设置](sheet-print.json) · [保护与链接](sheet-protection-links.json)

既有 13 组原生冒烟也通过，覆盖公式、历史、格式、系统剪贴板、多行编辑、合成组合态、取消关闭及 XLSX 保存重开。Linux 截图来自实际 X11 窗口，不标成 GPUI GPU 读回。[冒烟明细](sheet-smoke.json) · [保存重开窗口](sheet-roundtrip.png)

## 输入法、原生另存与首测问题

默认 Fcitx5 搜狗最初处于内部英文模式，三个产品直接收到 `zhongwen`；最小 GPUI 输入框同样失败。最小控件仅临时按 Shift 切换模式即可选中“中文”，无需重启服务。三产品随后用 `--sogou-shift --server-preedit` 均通过选词、Esc 取消第二段、撤销、重做、保存与新进程读回，并恢复原模式、输入源和激活状态。服务端预编辑期间控件内容保持未提交值；本轮不据截图认证完整候选框定位矩阵。[最小控件 A/B 与首测](investigation.json) · [Word](ime-word.json) · [Deck](ime-deck.json) · [Sheet](ime-sheet.json) · [备注中的搜狗候选](ime-deck.png)

`Ctrl+Shift+S` 首测被本机 Kimi Code 全局快捷键截获，前台切到了 `kimi-code`，未打开产品保存面板。没有修改该应用或系统快捷键配置。根据当前 1800×1125 窗口截图核对文件页按钮中心后，Sheet/Deck 通过真实 X11 鼠标操作打开原生保存面板，验证取消后重试、选择期间编辑使旧返回结果被拒绝、有效副本保存、独立包读取及新进程重开；原件散列保持。本机该快捷键仍存在外部冲突，不记为通过。[Sheet 保存事务](save-sheet.json) · [Deck 保存事务](save-deck.json) · [Sheet 文件页及本机坐标依据](sheet-file-menu.png)

验证脚本同步改进：

- `verify_uos_native_ime.py` 增加显式搜狗模式开关；模式恢复不额外发送会丢弃 Sheet 草稿的 Escape。
- `verify_native_save_target.py` 增加经当前截图核对的 X11 文件页按钮坐标入口。宿主缺少 Atspi typelib，未把自动可访问树操作记为通过。
- `check-transition-window.py` 的终态颜色误差由 3 收紧到 1，与最后的跨后端检查一致。原宽松判定曾在 CPU 轨道转场进度 0.9179 时提前接受近终态；修正后复跑通过，没有放宽门槛。
- Deck 驱动的 GUI、CLI 参考和交换子进程统一使用实际支持的 `XDG_CONFIG_HOME` / `XDG_STATE_HOME`，替换未被产品读取的 `JOYDECK_CONFIG_DIR` / `JOYDECK_RECOVERY_DIR`。已清理本轮临时 MRU 条目并恢复快照中的其他条目，随后隔离复跑时共享最近文件散列不变；没有批量清理用户恢复目录。[隔离核对](isolation.json)

初次缺参命令、诊断包装器和新增语料命名空间问题均保留原因及后续通过记录，不把脚本/环境失败直接归为产品缺陷，也不删除首测来制造全绿结论。[问题分类](investigation.json)

## 复跑入口

先在仓库根目录使用产品命令构建当前源码并运行检查。两版 CLI 需来自明确提交和独立保留的路径；所有输出目录必须是新目录，文件页坐标须按当前窗口重新核对。

```sh
cargo xtask check joyword
cargo xtask check joydeck
cargo xtask check joysheet
cargo xtask build joyword
cargo xtask build joydeck --validation
cargo xtask build joysheet --validation
cargo run --locked --offline -p joyword-engine --example chapter_reflow -- --section-splice
cargo run --locked --offline -p joydeck-render --features transition-gpu --example transition_pixels -- --gpu 64 36
cargo run --locked --offline -p joydeck-render --features transition-gpu --example transition_pixels -- --gpu 193 109
python3 scripts/joyword/check-installed-language-pack.py NEW_WORD_CLI NEW_WORD_DESKTOP NEW_DIR --published
python3 scripts/joyword/check-section-splice.py NEW_WORD_DESKTOP OLD_WORD_CLI NEW_WORD_CLI NEW_DIR
python3 docs/joyoffice/validation/20261123-uos-refactor/scripts/transition-exchange.py OLD_DECK_CLI NEW_DECK_CLI NEW_DIR
python3 scripts/joydeck/check-transition-window.py NEW_DECK_DESKTOP NEW_DIR --reference-cli OLD_DECK_CLI --prisms
python3 scripts/verify_uos_native_ime.py sheet NEW_SHEET_DESKTOP NEW_DIR --method com.sogou.ime.ng.fcitx5.uos --server-preedit --sogou-shift
python3 scripts/verify_native_save_target.py sheet NEW_SHEET_DESKTOP NEW_DIR --x11-save-buttons 42 56 100 391
python3 scripts/verify_native_save_target.py deck NEW_DECK_DESKTOP NEW_DIR --x11-save-buttons 29 58 95 653
python3 scripts/check_docs.py
```

Sheet 的九个增量脚本均使用 `python3 scripts/joysheet/check-NAME.py OLD_SHEET_CLI NEW_SHEET_CLI NEW_DIR --desktop NEW_SHEET_DESKTOP`；名称与上表 JSON 相同。三产品搜狗复验分别将 `sheet` 与应用参数替换为 `word`、`deck`。`--sogou-shift` 仅用于已确认当前搜狗处于英文模式的 A/B 条件，不能对未知状态盲目切换。

原始全量日志和临时 DOCX/PPTX/XLSX/PDF 留在本机 `target/uos-refactor-20261123/`，仓库保留摘要、原始逐帧数据、报告和截图，不提交控制 token、用户配置快照或系统字体。

## 仍开放的事项

1. Sheet 其余公式族与文件对象长尾；Deck 完整工作区/界面和旧自动化/功能矩阵；Word 旧功能与复杂场景继续按路线图逐项推进。
2. 本轮未运行 Excel/PowerPoint/WPS 的 UOS 端交互对照、Release 性能与 GPU/物理输入延迟专项、真实读屏、多屏/DPI 切换、拖放和设备打印。
3. Wayland、ARM64/LoongArch、麒麟等未实测平台保留未验证。其他信创真机可按条件后补，不阻塞开发、任务依赖、阶段验收或发布。
4. 本轮没有生成安装包、发布标签、签名/公证、安装关联或升级回滚验收，也没有新的远端 CI 或合并结果。验证修改按用户要求提交于 `feat/uos-refactor-validation-20261123`，未推送或合并。

本轮仅把有直接证据的 12 批 UOS 待验项更新为当前机器、当前 Debug 构建的通过，不将 R8/R9/M8 或全项目状态整体改为完成。
