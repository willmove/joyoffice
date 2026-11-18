# 桌面共享组件验证

## 保存更改提示框

同一份 Rust 源码在 Apple M4 macOS 27.2.1 与 UOS 25 x86_64 X11／DDE 构建，工具链为 Rust 1.98.1。Word 和 Sheet 的正常入口均使用公共面板。分别以独立配置及临时文档实际启动桌面，并保存程序指纹。Word 发送系统键盘事件，Sheet 用生产窗口的 GPUI 指针事件点击实测坐标处的按钮。

| 平台 | Word | Sheet | Deck | 公共 UI 与三个产品 UI 专项 |
| --- | --- | --- | --- | --- |
| macOS | 3,655 通过，2 忽略 | 1,440 通过，1 忽略 | 984 通过 | 75 通过 |
| UOS | 2,999 通过，5 失败，2 忽略 | 1,440 通过，1 忽略 | 989 通过 | 79 通过 |

Word 核对 Tab、反向 Tab、Esc 保留修改和原件，随后在弹框确认保存和替换。独立解析 DOCX，并在新进程重开。Sheet 在另一个工作簿窗口仍打开时核对取消、失败保留草稿、重试、只关闭目标、XLSX 重开和放弃后的原文件摘要。UOS 全量 Word 的五个失败测试名称逐项与已登记的排版基线对应。[本次机器记录](dialog.json) · [UOS 排版基线](../20261116-gpui-kit-091/baseline-layout.log)

复跑时使用未被占用的输出路径，在已解锁的图形桌面执行。macOS 需允许系统键盘事件，UOS 使用已登录的 X11 会话。

```sh
cargo build --locked -p joyword -p joysheet --features joysheet/automation
python3 scripts/verify_save_changes.py word target/debug/joyword target/save-dialog-word
python3 scripts/verify_save_changes.py sheet target/debug/joysheet target/save-dialog-sheet
cargo xtask check joyword
cargo xtask check joysheet
cargo xtask check joydeck
```

## WMF／EMF 有界读取

从保存提示框提交构建基线，再从共享读取实现构建候选。两台机器上分别以同一字体、工具和语料运行四个文档的产品 CLI。每份文档提取一页场景 PNG、导出 PDF，并用 Poppler 将 PDF 解码为 RGB。脚本先确认每张图包含指定颜色的不对称三角形，再对同机基线与候选比较全部 RGB 字节。两个平台各 8 项一致。此处比较的是应用页面场景与 PDF 光栅。[像素、测试计数和失败项](meta.json)

```sh
python3 scripts/verify_shared_metafile.py /path/to/baseline/debug /tmp/joyoffice-old-images
python3 scripts/verify_shared_metafile.py /path/to/candidate/debug /tmp/joyoffice-new-images --baseline /tmp/joyoffice-old-images/pixels.json
cargo test --locked -p joyoffice-metafile -p joyword-metafile -p joydeck-metafile
```

两份私有实现已改为导入公共读取器，输入预检、裁剪和 EMF+ 的调用路径与原件相同。新库覆盖负坐标、未对齐字段、记录截断和长度溢出。Word／Deck 的 GUI 与 CLI 普通依赖均可达该库，三个 CLI 的闭包均无 GPUI。可在单一产品恢复改动前的 `bytes.rs` 并用同一语料重放。

## 原生另存目标

Sheet 的两个保存入口在原生面板打开前捕获窗口、工作簿及修订号。单写者在写入前核验三者，面板从已有文件的目录打开。macOS 和 UOS 的真实另存面板均通过取消后重试。面板打开时修改 A1，接受的过时文件名没有产生文件，草稿仍在。再次选择中文带空格文件名后，独立解析 XLSX 并在新桌面进程读回。单写者专项另验文档已关闭、同修订号的其他文档以及切换活动窗口。Deck 的取消、失败和成功回调均绑定文稿代次和读取请求，接受时另核对修订号。两端 Deck 均取消后再次另存，并在新进程读回三页演示文稿。[结果和产品回归](save-target.json)

```sh
python3 scripts/verify_native_save_target.py sheet target/debug/joysheet /tmp/joyoffice-sheet-panel
python3 scripts/verify_native_save_target.py deck target/debug/joydeck /tmp/joyoffice-deck-panel
```


### UOS 中文输入

UOS 25 x86_64 的 DDE 用户会话、物理显示器 `:0`、系统 Fcitx5 和发行版拼音引擎。专用 XIM 名称、D-Bus 和临时用户配置使本次测试与桌面用户的搜狗服务分别运行。测试使用 X11 系统按键，生成中的拼音由 Fcitx 候选窗口显示。截图来自被测窗口范围。独立拼音使用 XIM 的 on-the-spot 设置；首测时默认搜狗服务的 selection 所有者为 0；后续按用户授权重启。Fcitx 按 XMODIFIERS 注册 XIM 名称。[Fcitx XIM 源码](https://github.com/fcitx/fcitx5/blob/5.1.12/src/frontend/xim/xim.cpp)

同一桌面、同一拼音，改动前 Word 和 Deck 通过，Sheet 提前发送 XIM_SET_IC_VALUES，连 XIM_OPEN 尚未完成，键入后单元格为 `zhongwen`。修复后最小控件收到 marked text。三个应用选词“中文”，Esc 取消第二段，切换焦点、撤销、重做后保存，独立读取文件包并用新进程核对编辑结果。每个产品记录二进制、输出文件和候选截图 SHA-256。[机器结果](uos-ime.json) · [Word 正文](uos-word-candidate.png) · [Deck 备注](uos-deck-candidate.png) · [Sheet 单元格](uos-sheet-candidate.png)

安装或从 UOS 发行版软件包解出拼音插件和词库后，在已登录桌面的环境中运行 `cargo build -p joyoffice-ui --example ime_probe` 以及三产品开发构建（Deck 启用 validation，Sheet 启用 automation），再执行 `sh scripts/verify_uos_pinyin.sh /abs/path/to/target/debug /tmp/new-ime-report`。该脚本创建独立的 Fcitx 配置与进程，退出时释放其 selection。要检查现有输入法，可分别执行 `python3 scripts/verify_uos_native_ime.py word /path/to/joyword /tmp/new-word-report --method pinyin` 等命令。回退 Linux 兼容补丁时同时恢复 Cargo.toml 的 registry 依赖和 Cargo.lock 校验值，再用相同输入法复跑。

**默认搜狗复验。** 重启 DDE 管理的用户 Fcitx5 服务后，默认 `@server=fcitx` 的所有者由 0 变为有效窗口，XOpenIM 从失败转为成功。默认配置由服务端显示预编辑，候选期间控件内容维持原值，按空格才收到“中文”。在该配置下最小控件和 Word、Deck、Sheet 均通过选择、取消与撤销；三产品另验重做、落盘和新进程读回。测试结束后恢复了原来的键盘输入源与未激活状态。复测命令加 `--method com.sogou.ime.ng.fcitx5.uos --server-preedit`。[默认服务记录](uos-sogou.json) · [Word](uos-sogou-word.png) · [Deck](uos-sogou-deck.png) · [Sheet](uos-sogou-sheet.png)

## 包读取与字体

普通 Word 与 Deck 入口将已验证的包成员移交给各自的格式 reader。成员名、长度和 CRC 不符时回到原解压路径，格式层继续执行自己的预算检查。Word 专项的重复解压从 120,016 字节降到 0，改坏一个成员缓存只重新解压该成员。两代保存和旧版读回覆盖批注／脚注、OMML、OLE、图表和 SmartArt 语料，按脚本实际列出的对象对照。[逐项结果](complex-docx.json)

同机交替执行普通 CLI 打开六轮，每轮保留峰值 RSS 和输出文本摘要。12 MiB 不透明部件样例的最高 RSS 在 macOS 上由 72.88 降至 62.70 MiB，在 UOS 上由 58.83 降至 56.84 MiB。对应打开时间中位数分别为 170.14 → 177.23 ms 和 87.81 → 83.36 ms。两端的页面场景及 PDF 各八项像素与同机基线匹配。复跑入口为 `scripts/joyword/measure-opc-open.py`、`scripts/joyword/check-complex-corpus.py` 与 `scripts/verify_shared_metafile.py`。[macOS](macos-opc.json) · [UOS](uos-opc.json) · [固定字体输出](font-pixels.json)

## Sheet 工作簿与重算

两份用户授权工作簿各自用默认与候选 CLI 验证往返，原始工作表没有公式。脚本在临时副本中按实际数值位置加入算式和汇总，生成独立期望缓存，逐格核对两种实现及往返结果。两端分别匹配 916/916 和 16/16。报告只记录摘要、计数和计时。复跑使用 `scripts/joysheet/verify-business-workbooks.py --help`。桌面扇出测量另用 `scripts/joysheet/bench-fanout.py`，每组 20 次编辑，验证撤销、重做和 XLSX 重开。[业务结果](business-workbooks.json) · [历史基线与候选](recalculation.json)

## 分页、媒体与形状

运行 `cargo run --locked --release -p joyword-engine --example chapter_reflow` 可重建 1,000 页语料并交替插入和删除四个会换行的短句。两端各执行 20 次计时。比较的第一个版本持有暖机的完整布局缓存；候选计时包括改段落和生成新布局。前两次改动另逐页比较所有放置项、行区间与命中，核对三页修改区及相邻两页的光栅和 PDF 字节。[逐次结果](chapter-reflow.json)

`joydeck-cli scene-bench tests/fixtures/joydeck/r6-r7/delivery-media.pptx --page 1 --iterations 25 --output NEW_DIR` 记录媒体哈希量、CPU 全页渲染、缓存构建和几何缓存大小。标题编辑和同 ID 换图的正确性另由 `joydeck-render` 的缓存用例验证。`scripts/joydeck/check-preset-shapes.py OLD_CLI NEW_CLI NEW_DIR` 为七种形状各生成四种调整设置，核对 XML，再让两个版本渲染原件和新版本重新生成的文件。设计资料取自 ECMA-376 第一部分 §20.1.9 及附带的预设几何 XML（SHA-256 为 `2f7c868d857c1e3c4b5a6068759fe0e07d77ad58377a6618d1b02ba3507b6939`）。[场景结果](deck-media.json) · [几何结果](deck-presets.json) · [ECMA 第五版第一部分](https://ecma-international.org/wp-content/uploads/ECMA-376-1_5th_edition_december_2016.zip)

## Sheet 默认采用与原生显示帧

两个 Release 构建来自同一份源码，分别关闭和启用标量批处理。按基线、候选、候选、基线的顺序测试，两个顺序均有每配置 20 次有效编辑。UOS 计时期间停止编译。下表为合并 40 个样本的最近秩 P95。RSS 为两轮中较高的进程生命周期峰值，包含保存、撤销和重开。真实工作簿结果另见上节。

| 平台与行数 | 引擎 P95 毫秒 | 提交到 CPU 绘制 P95 毫秒 | 请求到显示 P95 毫秒 | 峰值 RSS MiB |
| --- | --- | --- | --- | --- |
| macOS 100 | 0.208 → 0.206 | 21.60 → 18.04 | 81.16 → 81.21 | 142.13 → 142.14 |
| macOS 100,000 | 71.23 → 50.63 | 81.79 → 62.50 | 144.51 → 116.06 | 1111.20 → 1115.69 |
| UOS 100 | 0.558 → 0.538 | 15.80 → 15.22 | 47.59 → 42.51 | 140.60 → 141.21 |
| UOS 100,000 | 205.17 → 155.91 | 217.40 → 168.61 | 203.97 → 138.72 | 1062.42 → 1062.80 |

CPU 与显示来自独立运行。显示用例在 A1 交替提交 17 和 88，逐次读回 B1，按 B1:B3 的两个已校准像素摘要匹配帧。时间从发送本机编辑请求前算起，包含传输与调度。macOS 采用 ScreenCaptureKit 的 WindowServer 显示时间，UOS 采用 X Present 的 pixmap 完成事件并在收到事件时采样同一窗口。编译观察器可用 `swiftc -parse-as-library scripts/desktop_frames/window.swift -o /tmp/window-frames`，X11 可用 `cc -O2 scripts/desktop_frames/xpresent.c -o /tmp/window-frames -lX11 -lXpresent`。运行 `python3 scripts/joysheet/measure-presentation.py APP /tmp/window-frames NEW_DIR --rows 100000`。两端的引擎收益均超过 15%，小表 P95 与 RSS 均满足预算。已将合格子集设为默认，新进程可用 `JOYSHEET_SCALAR_RECALC=0` 恢复通用路径。[逐次引擎数据](same-source-recalculation.json) · [逐次显示数据](desktop-presentation.json) · [Apple 显示时间定义](https://developer.apple.com/documentation/screencapturekit/scstreamframeinfo/displaytime)

## 功能区原生验收

用 `python3 scripts/verify_shared_ribbon.py PRODUCT APP NEW_DIR` 启动独立配置的产品窗口。macOS 读取 AX 树，UOS 在 DDE 实机桌面上读取 AT-SPI 树。两端分别请求 1180、740 逻辑像素窗宽，实际宽度记录在报告中。逐窗验证分组可达、键盘折叠后消失并可恢复。Word、Sheet 覆盖中英文，Deck 覆盖当前提供的中文功能区。[20 组实测](ribbon.json)


Word 和 Sheet 的 16 个字体组用例还通过原生可访问按钮打开相应命令，核对 Word 字体对话框或 Sheet 单元格格式的字体页后关闭；Deck 的四个用例验证它已有的分组与折叠。Deck 用两个实际窗口打开同一文稿，在第二个窗口的文件选择器显示期间由第一个窗口新增一页。接受后核对旧路径不存在，重试后按 PPTX 关系找到四页并在新进程重开，且原件哈希一致。复现可加 `--other-view`。UOS DDE 原生选择器测试使用真实桌面会话；窗口位置因缩放改变时，`--x11-view-buttons` 接收实机量得的两个按钮中心。[功能区](ribbon.json) · [跨窗另存](save-target.json)

## 旧组件和旧版读回

| 产品 | 直接验证 | 仍有生产调用的部分 |
| --- | --- | --- |
| Word | 自然分页与完整分页逐页比较；另在 96 页文档第 49 页编辑表格，核验行／格路径与命中索引。前版 CLI 读回本版生成的含脚注、公式、OLE、图表和 SmartArt 的五类 DOCX | 多语词典与安装器、复杂跨页传播和未迁移的格式语义。[分页用例](../../../../crates/joyword-layout/src/incremental.rs) · [DOCX 两代保存](complex-docx.json) |
| Deck | 七种预设各用默认、0、50,000、100,000，28 个形状在新旧程序内重新生成和读回，每台机器的 960 × 540 页面有 0 个差异像素。混合透明扫描行覆盖全部 256 种输入 alpha。富媒体页背景撤销与重作交替进行，20 次采样的实际呈现 P95 为 macOS 92.57 ms、UOS 55.09 ms | 其他预设和透视层；只读迁移桥负责旧文件。[形状](deck-presets.json) · [Release 显示帧](deck-media.json) |
| Sheet | 上版与本版各自核对七个工作簿的结构和检查结果，两个版本均与 16 个独立公式预期值吻合。同一发布程序默认和开启回退各运行 20 次一万行编辑，每次检查 10,001 个公式并验证撤销、重做和 XLSX 读回 | 其他函数、通用依赖调度与格式适配。[七个工作簿及公式](version-rollback.json) · [同程序回退](scalar-rollback.json) |

Word 和 Deck 共用目录遍历及文件修订识别，塑形和回退顺序由产品维护。每端五个新进程分别记录首次和第二次扫描，字体面数保持相同。Word 的重复扫描中位数在 macOS 为 25.66 → 8.45 ms，在 UOS 为 6.81 → 1.88 ms。Deck 在 UOS 为 4.51 → 2.49 ms，在 macOS 为 17.18 → 18.46 ms，首次扫描在 macOS 也有波动（60.63 → 69.94 ms）。这些是分批观测值。系统字体来自本机安装；仓库保留字体资源和词典原许可。运行 `python3 scripts/measure-font-catalog.py OUTPUT.json` 可重测。[逐次结果](font-catalog.json)

发行清单从各平台的普通／构建依赖生成，三个产品均包含桌面和 CLI。Word 旧的 `craft-font-catalog` 已退出，macOS 和 UOS 的 Word 闭包分别为 512 和 601 包。语言包存储、词典引擎和原子写入仍分别使用 `craft-font-store`、`craft-proof` 和 `craft-atomic`。六份清单均找到依赖许可全文。重新启用 font-store 的 `catalog` 特性可恢复注册目录刷新接口；宿主包目录沿用既有路径。[各产品发行清单](distribution.json) · [上游来源及本地修改](../../../../licenses/craft-libs/SOURCE.json)


本批 macOS／UOS 产品检查分别为 Word 3,672／3,676、Deck 995／1,000、Sheet 1,452／1,452，全部通过。UI 专项为 75／79，字体专项为 159／161，与产品检查部分重叠。独立的语言包存储库分别以原默认目录功能和宿主关闭目录功能测试，均为 32 项通过、1 项按库原配置忽略。复现时对三个产品分别使用 `cargo xtask check PRODUCT`；存储库在 `third-party/craft-libs/font-store/Cargo.toml` 的独立工作区内执行 `cargo test --locked -p craft-font-store` 以及加上 `--no-default-features` 的同一命令。[计数及日志摘要](retirement-regression.json)
