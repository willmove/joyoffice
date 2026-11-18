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
| Deck | 七种预设各用默认、0、50,000、100,000，28 个形状在新旧程序内重新生成和读回，每台机器的 960 × 540 页面有 0 个差异像素。混合透明扫描行覆盖全部 256 种输入 alpha。富媒体页背景撤销与重作交替进行，20 次采样的实际呈现 P95 为 macOS 92.57 ms、UOS 55.09 ms | 该检查点尚余其他预设和透视层。CPU 透视层后续已完成 925 帧对照，旧文件由只读迁移桥处理。[形状](deck-presets.json) · [Release 显示帧](deck-media.json) |
| Sheet | 上版与本版各自核对七个工作簿的结构和检查结果，该检查点的两个版本均与 16 个独立公式预期值吻合，后续扩至 24 个。同一发布程序默认和开启回退各运行 20 次一万行编辑，每次检查 10,001 个公式并验证撤销、重做和 XLSX 读回 | 其他函数、通用依赖调度与格式适配。[七个工作簿及公式](version-rollback.json) · [同程序回退](scalar-rollback.json) |

Word 和 Deck 共用目录遍历及文件修订识别，塑形和回退顺序由产品维护。每端五个新进程分别记录首次和第二次扫描，字体面数保持相同。Word 的重复扫描中位数在 macOS 为 25.66 → 8.45 ms，在 UOS 为 6.81 → 1.88 ms。Deck 在 UOS 为 4.51 → 2.49 ms，在 macOS 为 17.18 → 18.46 ms，首次扫描在 macOS 也有波动（60.63 → 69.94 ms）。这些是分批观测值。系统字体来自本机安装；仓库保留字体资源和词典原许可。运行 `python3 scripts/measure-font-catalog.py OUTPUT.json` 可重测。[逐次结果](font-catalog.json)

发行清单从各平台的普通／构建依赖生成，三个产品均包含桌面和 CLI。Word 旧的 `craft-font-catalog` 已退出，macOS 和 UOS 的 Word 闭包分别为 512 和 601 包。语言包存储、词典引擎和原子写入仍分别使用 `craft-font-store`、`craft-proof` 和 `craft-atomic`。六份清单均找到依赖许可全文。重新启用 font-store 的 `catalog` 特性可恢复注册目录刷新接口；宿主包目录沿用既有路径。[各产品发行清单](distribution.json) · [上游来源及本地修改](../../../../licenses/craft-libs/SOURCE.json)


本批 macOS／UOS 产品检查分别为 Word 3,672／3,676、Deck 995／1,000、Sheet 1,452／1,452，全部通过。UI 专项为 75／79，字体专项为 159／161，与产品检查部分重叠。独立的语言包存储库分别以原默认目录功能和宿主关闭目录功能测试，均为 32 项通过、1 项按库原配置忽略。复现时对三个产品分别使用 `cargo xtask check PRODUCT`；存储库在 `third-party/craft-libs/font-store/Cargo.toml` 的独立工作区内执行 `cargo test --locked -p craft-font-store` 以及加上 `--no-default-features` 的同一命令。[计数及日志摘要](retirement-regression.json)


## 后续替换的验证范围

- **Word 校对和分页。** 拼写直接遍历原始 UTF-8 词区间。断词仍由安装的语言数据或内置规则提供候选，宿主按原始字符位置、每段首尾字母数和大小写统一筛选。十个语言标签 × 13 个词／混合输入 × 三组限制逐一与先前实现对照。词典和词形资源沿用原许可。含分节、两栏和实际脚注的长文另验正文编辑只更新脚注所在页、所有页与索引相同，同时改动脚注时自动回到完整排版。[Unicode 词界及语言定制](https://www.unicode.org/reports/tr29/#Word_Boundaries) · [语言数据及许可](../../../../third-party/craft-i18n/language-data/README.md)
- **Deck 转场。** 预乘透明图纹在入场页、退场页及分开的背景／内容层上参与投影变换。两端各用改动前后两个渲染器检查 49 种转场、185 个效果与选项组合，进度为 0、22%、50%、78% 和 100%。925 组 RGBA 图片的差异数均为零。基线与候选各运行 `cargo run --locked -p joydeck-render --example transition_pixels` 比较完整结果。该测试覆盖离屏 CPU 投影和合成，原生桌面呈现时间见富媒体页实测。[逐帧结果](transition-raster.json) · [源覆盖公式](https://www.w3.org/TR/compositing-1/#porterduffcompositingoperators_srcover)
- **Sheet 统计和文件。** MEDIAN、LARGE 和 SMALL 按秩选择，MODE、MODE.SNGL 和 MODE.MULT 按频次统计并保留首次出现次序，FREQUENCY 按有序阈值二分并写回原始桶位置。远距离清除阻塞格和溢出收缩仅查询相应工作表的潜在锚点。CFB 目录名在 Word 严格解码、Sheet 容错读取及加密检测中复用，仍按产品原有约定处理畸形数据。XLSX 的按需部件、Word 的可选部件和公共 OPC 的严格 ZIP 解压各自保留 CRC 与预算失败行为。[工作簿和公式](version-rollback.json) · [MS-CFB 目录项](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/60fe8611-66c3-496b-b70d-a504c94c9ace)

两端 Release 原生窗口打开同一份 1,000 页 DOCX，定位第 500 页，各项预热两次后采样 20 次。系统键盘输入数字 7 和退格，从发送原生事件到首次读回正确的整份正文，P95 为 macOS 51.41 ms、UOS 41.77 ms，包含读回及调度开销。通过同一窗口的编辑命令交替插入 240 个字符和撤销，操作应答 P95 为 36.11 ms、28.68 ms。每次均核对整份正文、插入位置及页数，原件 SHA-256 一致。复跑命令为 `python3 scripts/joyword/measure-desktop-edit.py RELEASE_JOYWORD NEW_DIR`。[逐次桌面测量](word-desktop-edit.json)

本批全产品工程检查在 macOS 通过 Word 3,675、Deck 996、Sheet 1,468 项，在 UOS 25 真机分别通过 3,679、1,001、1,468 项。每端 Word 原有两项、Sheet 原有一项按其测试配置忽略。校对库专项每端通过 474 项、忽略一项。两端各八份 WMF／EMF 页面及 PDF 光栅与同机旧版相符。另用最终 Release 重新读取两份授权工作簿及含公式的派生副本，原件摘要均保持一致，UOS 临时输入副本已清理。额外试跑字体库测试目标的全量 Clippy 拒绝了基线已有的测试 `unwrap`（`joyword-fonts/src/color.rs`）；仓库规定的三产品工程检查全部通过。[回归分项](extended-regression.json) · [原件与派生工作簿](business-workbooks.json) · [产品许可闭包](distribution.json)

## 安装词典、矩形族与循环重算

| 产品 | 两端验证方式 | 可复核的结果 |
| --- | --- | --- |
| Word | 在隔离的原有目录启动全新的桌面进程和 CLI。用同一荷兰语 DOCX 比较未安装、整词例外、按音节断词、卸载四种状态。再实际下载固定的正式索引和荷兰语包，检查词形、词表、断词与许可文件 | macOS 四次页数分别为 15、21、15、15，UOS 为 20、24、20、20。各平台每次 GUI 与 CLI 匹配，原件摘要相同。拒绝关闭网络同意时刷新，拒绝损坏的安装包。两端均核对正式索引中的 75 个条目、756,670 字节荷兰语包的 SHA-256 和授权。UOS 直连下载超时，随后经 SSH 转发到本机已配置的 HTTPS 代理，产品自行完成 HTTPS 下载、校验、安装与卸载。[语言包记录](language-packs.json) |
| Deck | 对画廊的九种矩形各生成五组参数，包括非对称角点。在新旧版分别渲染原件和重新生成的 PPTX，检查文字、预设名与调整值 | 每端 45 个实例，四次 960 × 540 画面的同机 RGB 摘要相同。调整手柄和兼容别名另有几何单测。[矩形对照](deck-rectangles.json) |
| Sheet | 按独立传递闭包枚举四顶点有向图，另测五万节点长链。经 CLI 打开、保存并重开四种实际迭代设置，逐个读取 XLSX 的公式、缓存和迭代属性 | 65,536 张图的成环节点和分组相符。轮数限制、变化阈值、文本与错误、关闭迭代四组各有四种新旧读写组合。两个版本的例子在三轮后为 3 与 6，再次打开重算为 6 与 12。两台机器的最终 Release 再次读回两份业务原件，带公式的派生副本分别匹配 916 和 16 个预期值，UOS 已清理临时业务文件。[循环与往返](version-rollback.json) · [业务工作簿](business-workbooks.json) |

可复跑的入口为 `python3 scripts/joyword/check-installed-language-pack.py CLI DESKTOP NEW_DIR --published`、`python3 scripts/joydeck/check-preset-shapes.py OLD_CLI NEW_CLI NEW_DIR --family rectangles` 和 `python3 scripts/joysheet/check-version-rollback.py --help`。每次 GUI 测试使用独立配置和新进程。荷兰语发行数据与许可按正式索引核对，词典内容保留原许可。[固定发行清单](https://github.com/storytold/craft-i18n/releases/tag/language-packs-2)

本批采用各消费产品工程命令，macOS 与 UOS 的 Word 分别通过 3,676 与 3,680 项，Deck 通过 997 与 1,002 项，Sheet 均通过 1,470 项。两台机器均另跑校对库的 475 个用例，原有一项忽略。公共 UI 另通过 75 与 79 项，部分用例已包含在产品工程检查中。两端均完成桌面和 CLI 联合构建，且六份产品发行清单均找到依赖许可文本。[产品与专项回归](cycle-regression.json) · [发行范围](distribution.json)

## Word 连续编辑和跨页重绘

页面只有在纸色、显示选项、字体版本、页面尺寸及两份显示列表都相同时才能复用。对正文、表格与脚注的文字页面，比较在栅格线程执行。带图、水印、高密度或未能核验的页继续重绘。相邻两个版本的缓存可以继承，中间曾失效的旧图保持待更新。专项还检查增加脚注高度后每页的真实光栅、切换深色、格式标记与纸色，以及页号越界。

两端在真实窗口打开完全相同的六页双栏 DOCX。插入八次重复短句及撤销后分别核对完整正文和六页总数。两次都观测到第 1 页重新生成位图，第 2 页沿用已有图像，磁盘原件的 SHA-256 一致。复跑入口为 `python3 scripts/joyword/check-page-repaint.py DESKTOP NEW_DIR`。macOS 和 UOS 的 Word 产品工程检查分别通过 3,677 和 3,681 项。两端同时复跑公共 UI 专项和六个桌面与 CLI 入口的联合构建。[原生及工程验证](page-repaint.json)

## Deck 的 PPTX 连接线

对六种画廊线条与连接线使用五组参数，对另外五种导入预设使用同样五组参数。每个测试对象使用 PresentationML 的 `p:cxnSp`，包括折线、曲线及升降对角线。先让新旧 CLI 渲染原件和本版重新生成的 PPTX，再将 ECMA-376 附件中对应的几何路径及坐标公式作为 `a:custGeom`，专由旧版的自定义几何读取器生成参照图。两端 55 个连接实例均与参照完全相符。画廊的 30 个实例与旧版也完全相符。另 25 个实例的 960 × 540 合成页与旧版相差 12,071 个像素，两端相同。新旧版均保留预设名与调整值，旧版读回时仍画简化路径。

复跑用 `python3 scripts/joydeck/check-preset-shapes.py OLD_CLI NEW_CLI NEW_DIR --family connectors --definitions PATH_TO_PRESET_XML`，第二组将族名换为 `file-connectors`。几何单测另核对所有控制点、两个端点以及五段连接线的三个拖动手柄。两端的 Deck 产品工程检查分别通过 998 与 1,003 项，六个入口联合构建与公共 UI 专项均通过。[合成页面摘要、规范指纹及回归](deck-connectors.json)

## Sheet 查找向量与双版本互读

六种查找函数直接读取原数组，保留首次／末次重复项、通配符、类型差异、二分查找及缺失结果。XLOOKUP 与 XMATCH 的正向及反向最邻近查找，在同一遍访问中记录候选并优先返回精确命中。两端分别对 4,096 行的真实桌面会话提交新键并撤销、重做，再直接检查 XLSX XML 的七个公式及其缓存值，最后在桌面重开。另有八个查找公式通过两版写入、两版重开的四种组合。六种查找、汇总与统计的独立 CLI 公式期望值合计 35 个。[原生验证](sheet-lookups.json) · [独立公式期望](version-rollback.json) · [Microsoft 查找语义](https://support.microsoft.com/en-gb/excel/functions/xlookup-function)

性能用例每次由同一个 Release CLI 打开刚保存的工作簿，交替提交 12 次查找键并逐格核验最终值。基线和候选各预热一次，随后按 ABBA 各采 20 次。进程启动、打开及编辑合计的 P95 为 macOS 14.43 → 13.74 ms，UOS 26.53 → 29.26 ms。随后消除扫描不匹配键时重复的排序比较，按每组 20 次交替采用 ABBA 和 BAAB，将同机旧版和最终版各测 100 次。macOS 的 P50 为 13.58 → 13.01 ms，P95 为 14.13 → 13.88 ms；UOS 的 P50 为 23.05 → 22.72 ms，P95 为 27.95 → 26.59 ms。两组数据均包含进程启动、打开和编辑，不能单独衡量函数的求值时间。复跑用 `python3 scripts/joysheet/check-lookup-edit.py DESKTOP NEW_DIR --compare-cli OLD_CLI NEW_CLI --timing-rounds 5`。两台机器的产品工程检查均通过 1,471 项并按原配置忽略一项。最终 Release 重验两份授权业务原件及派生副本，原件摘要无变化，UOS 临时业务副本已清理。[原始计时和工程门禁](sheet-lookups.json) · [业务文件汇总](business-workbooks.json)

## Word 脚注、连续分节与嵌套表格

原生窗口打开 70 节、69 页的 DOCX。第 35 页使用等宽双栏，之后的连续节与它共用一页。文档含三条连续编号的脚注，浮动表格的单元格内还有一张表。两端都执行了插入、撤销和重做，三次布局更新的起止页均为 35。保存后独立读取 DOCX 的正文、脚注及表格定位，再启动新进程重开。最初的文件摘要保持一致。工程专项另对脚注加长／删回、栏平衡变化和两种嵌套表格的单元格编辑进行完整分页对照，核对全部页、正文与脚注的光标命中、受影响页和两侧邻页的 RGB、PDF。续注超出组末时实测回到完整分页。[桌面与产品结果](section-reflow.json)

Release 在同一份原生测试文档的第二条脚注交替插入和删除，五个新进程各执行 20 次。macOS 完整分页与编辑加局部分页的 P50 为 0.585 → 0.411 ms，P95 为 0.648 → 0.559 ms。UOS P50 为 0.591 → 0.422 ms，P95 为 1.000 → 1.017 ms。两端中位数均下降，UOS 尾部有约 0.018 ms 回退。每台机器另完成 690 次逐页比较和 2,070 次命中检查。记录中保留了每次都重新核验旧布局的初版、加入章节缓存的中间版与最终版的计时。macOS 产品回归通过 3,679 项，UOS 通过 3,683 项。两端公共 UI 和桌面／CLI 联合构建也通过。[逐次计时](section-reflow.json)

复跑用 `python3 scripts/joyword/check-section-reflow.py DESKTOP NEW_DIR`，随后运行 `cargo run --release -p joyword-engine --example chapter_reflow -- NEW_DIR/原始分节.docx`。该例省略文件名时仍执行原有的千页中段增删基准。当时的局部计算限于原布局至少 64 页、连续节组最多 256 个正文块和 16 页。当时分节改变总页数、外部字段、复杂浮动绘图和无法验证的入口均使用完整分页。下文记录了新增的分节增减页支持。[生产入口](../../../../crates/joyword-layout/src/incremental/sections.rs)

## Word 多语拼写建议与原生审阅

JoyWord 用带宽受阈值限制的动态规划计算候选距离。相邻字符换位与普通编辑各计两分，词典 MAP 声明的近似字母替换计一分。优先队列按距离、首个不同字符的位置和候选原次序取出结果，直到大小写转换、去重后的建议数量足够。对照程序使用完整矩阵，对 121 个含重音字符的字符串两两比较，在八个阈值下核对了 117,128 次结果。带状求解的边界依据 Ukkonen 的编辑距离研究，字符权重及换位规则采用现有产品词典约定。[Ukkonen 原文](https://www.cs.helsinki.fi/u/ukkonen/InfCont85.PDF) · [实现](../../../../third-party/craft-libs/proof/src/affix_suggest/ranking.rs)

两个平台分别校验了固定索引和 75 个正式压缩包的 SHA-256，共 41,923,267 字节。每个语言标签选两个词干，另造换位和删字输入。通过真实 CLI 的审阅命令，每端对照新旧程序各 450 个输入，产生 298 个拼写问题，其中 293 个有建议。问题的文字范围、语言和完整建议列表在版本之间、在两台机器之间逐项相同。四种语言另各有 12 个独立样例，核对词形、重音、换位、整词拆分及无建议的情形。真实词典部分衡量与旧版的一致性，语言学正确率需要人工标注语料。[逐语言结果](spelling-ranking.json)

原生窗口从“审阅”打开“拼写和语法”，两端的辅助功能树均读回 `teh` 和依次排列的 `the, tens, tent, test, text`。关闭弹窗后输入第一项，验证只有选中的错词改变，撤销、重做，保存副本并在新进程重开。首轮发现弹窗正文没有可访问标签，现已补齐。两端校对库各通过 476 项并按原配置忽略一项，生产库 Clippy 通过。macOS 的首次并发产品回归有一项 1 MiB 长词布局超过 15 秒限值，改为两个测试线程重跑整套。额外对校对库测试代码开启所有 Clippy 告警时，已有测试中的 `unwrap` 等仍会报错。[双平台回归与原生纠错](spelling-ranking.json)

复跑用 `python3 scripts/joyword/check-spelling-order.py OLD_CLI NEW_CLI NEW_DIR --release-packs VERIFIED_PACKS --desktop NEW_DESKTOP`。词典目录含正式 `index.json` 及按小写语言标签命名的 `.tar.zst`。安装器仍以用户同意和应用固定的索引为入口。[验证脚本](../../../../scripts/joyword/check-spelling-order.py)

## Word 语言包下载、发布与中断恢复

Word 的桌面和 CLI 现在共用 JoyOffice 的词典传输与发布逻辑。下载正式索引时校验应用固定的 SHA-256，每个压缩包另验索引所列的长度和摘要。用户同意设置、网络超时、许可和安装目录沿用原格式。安装、替换和卸载在同一文件锁下提交目录和清单；进程中断后，下一次打开词典目录会恢复上一个已提交版本，同时保留其他语言。文件锁、HTTP、压缩和已有归档验证仍复用原库。[传输实现](../../../../third-party/craft-libs/font-store/src/language_packs/transfer.rs) · [目录事务](../../../../third-party/craft-libs/font-store/src/language_packs/transaction.rs)

macOS 与 UOS 真机各用固定索引的 75 个正式包运行旧、新两版 CLI。37 个由旧版首次安装，38 个由新版首次安装，再交换版本覆盖全部 75 个。每轮单独对照压缩包内的 569 个文件和安装清单，其中有 276 个许可文件。四个独立进程同时替换词典，其中两个写入同一语言，最后按两版分别卸载并检查目录与清单均空。整套使用本地镜像以便重放；另在两个原生桌面实际下载正式索引与荷兰语包，UOS 经 SSH 转发使用 HTTPS 代理。两端的 GUI 与 CLI 对安装前、替换、卸载及实际正式包的分页一致，原始文档摘要相同。[文件、网络与原生结果](pack-publication.json)

对创建、替换、移除分别构造提交前后的五种磁盘状态，再启动新存储会话恢复；另有八种替换／卸载中断快照经过真实 CLI 冷启动及旧版读回。传输中取消时，原词典和清单保持逐字节相同。这里验证进程中断恢复，尚无断电实测。两端存储库各通过 33 项、忽略原有一项，生产库 Clippy 通过。产品工程检查与共享 UI 和六入口联合构建通过。复跑用 `python3 scripts/joyword/check-pack-publication.py OLD_CLI NEW_CLI VERIFIED_PACKS NEW_DIR`，随后运行 `python3 scripts/joyword/check-installed-language-pack.py NEW_CLI NEW_DESKTOP OTHER_NEW_DIR --published`。[双版本复跑脚本](../../../../scripts/joyword/check-pack-publication.py)

## JoyDeck 的 29 种流程图与指针命中

29 种流程图现在从同一套图形定义生成屏幕、导出、文字框和有序连接点。移除了旧分派中的流程图分支。另在旋转 37° 的圆角流程框中复现了内部点击漏选，原因是曲线与直线接头附近的射线交点随坐标变换的舍入而消失。现在使用有界细分，共享相邻曲线端点，保留孔洞、奇偶填充和开放轮廓的隐含闭合边。[流程图](../../../../crates/joydeck-geom/src/preset/flowchart.rs) · [填充命中](../../../../crates/joydeck-geom/src/hit.rs)

macOS 与 UOS 真机均通过 87 组文字框和连接点坐标对照。独立参照来自 ECMA-376 第一部分的 DrawingML 预设附件，交由更改前的 CLI 以自定义几何渲染。文字单独放入附件定义的矩形。每端各渲染横、竖、方三种比例及各自带字和无字两种版本，共 174 个实例。逐次核对预设名，新旧两版各自重新打开生成的 PPTX 后，图像与各自打开原件时相同。[规范附件](https://ecma-international.org/wp-content/uploads/ECMA-376-1_5th_edition_december_2016.zip) · [复验脚本](../../../../scripts/joydeck/check-flowcharts.py)

相对独立参照，六张 960 × 540 合成页分别有 231 至 664 个像素的 RGB 存在差异。按白底、填充和描边的覆盖率作双向一像素邻域检查，超出一像素的取色点为零。竖版有四个边缘像素的通道强度超出邻域容差，数据中保留了原值。另从参照图提取 4,984 个纯色内部点及 947 个纯白外部点，在原角度和旋转后核对桌面指针处理，加上框外点，每端检查 12,210 次。原生窗口另对文档形状进行了横向／纵向移动、拉宽、增高、撤销、重做、保存副本和新进程重开。原件 SHA-256 相同。两个平台另复验 55 个连接线实例，与参照图逐像素相同。Deck 产品回归分别通过 1,001、1,006 项，四个 UI 库分别通过 76、80 项。[双平台原始摘要与回归](flowcharts.json)

## Word 发行索引、压缩帧与词典包内容

Word 的桌面、CLI、缓存索引和安装入口现共用宿主编写的索引与包内容读取器。索引保留发行顺序，一次遍历检查语言标签、摘要、容量、来源与重复项。安装器按 tar 成员名识别词典、模式与许可文件，在临时目录逐个写入并核对清单声明。现有 tar、TOML、JSON 和 Zstandard 库负责通用格式解码。多个 Zstandard 帧共享同一个解码工作区。压缩包 128 MiB、单成员 64 MiB、总内容 256 MiB 和条目数等预算继续生效，尾部 tar 填充也纳入解压预算。[实现](../../../../third-party/craft-libs/font-store/src/language_packs/contents.rs) · [RFC 8878 的帧结构](https://www.rfc-editor.org/rfc/rfc8878.html#section-3.1)

两个平台各用发行索引和全部 75 个包做新旧版交替安装、覆盖、读取和卸载。每端逐一对照 569 个文件的摘要，其中包含 276 个许可文件。真实桌面另下载荷兰语包，核对安装及卸载前后的分页和原始文档。专项回归覆盖 1,024 项索引的顺序与边界、跨帧读取、目录项、便携文件名、重复条目、保留未知清单字段及超过 1 MiB 的有效清单。正式档案字节和新旧版安装清单保持兼容。macOS 与 UOS 的 Word 产品回归分别通过 3,679 与 3,683 项，存储库各通过 36 项，原有一项忽略。恢复到上次发布器可直接读取相同目录和索引。[双平台结果](pack-contents.json)

## JoySheet 的动态筛选、多键排序与去重

FILTER、SORT、SORTBY、UNIQUE 现在直接读取输入行或列，按索引决定输出顺序，最后分配结果。按列排序也能直接读取原数组。去重键保留值类型、数值零、错误类型和大小写不敏感的文本语义，按首次出现的顺序输出。稳定的多键排序和逆序中空格的位置继续沿用产品比较规则。分组和透视汇总使用同一排序规则。[筛选](https://support.microsoft.com/en-us/excel/functions/filter-function) · [排序](https://support.microsoft.com/en-us/excel/functions/sort-function) · [按键排序](https://support.microsoft.com/en-us/excel/functions/sortby-function) · [去重](https://support.microsoft.com/en-us/excel/functions/unique-function)

七个公式分别由新旧 CLI 写入，两代都重开并读取 96 个单元格，还逐一核对 XLSX 中公式和溢出区域。两个原生桌面均从旧版工作簿开始，改变筛选阈值、撤销、重做、清空结果、放置阻塞单元格，读到 `#SPILL!`，移除阻塞后恢复，再保存副本并用新进程重开。两端各有八个完整的 96 格快照。另回归七份仓库工作簿与两个获准业务样本。原始业务文件均没有公式，派生副本分别核对了 916 和 16 个公式，UOS 临时副本已清理。[逐次结果与公式](array-selection.json) · [复跑脚本](../../../../scripts/joysheet/check-array-selection.py)

4,096 行的固定表上，两版发布程序交替启动，各取 40 个样本，每次打开工作簿、提交六次阈值编辑并核对四个独立计算结果。macOS 的 P50 从 34.25 降至 28.42 ms，P95 从 36.02 降至 29.54 ms。UOS 的 P50 从 44.14 降至 36.18 ms，P95 从 55.37 降至 46.40 ms。计时包含打开与六次计算。两端 Sheet 产品回归均通过 1,473 项。[发布程序指纹与原始计时](array-selection.json)

## Word 分节增减页、命中索引与旧尾页图像

在 69 页文档的第 35 页插入 11,100 个字符。macOS 原生窗口增至 73 页，UOS 原生窗口增至 74 页，两端撤销后均恢复 69 页，重做后恢复各自新增的页数。完整分页也得到各自平台的页数。旧尾页在撤销时一起失效。正文、三条脚注及浮动表格中的嵌套表格经独立 XML 检查；保存后用新进程重开，再用两代 CLI 各写一份，两版均逐字读回并重建相同的页面尺寸和页数。两端同页的小编辑也重新验证。[两端窗口、读回与回归](page-count-reflow.json)

新的排版路径在分节出口与后一节首段重新对齐后复用后续的页内容。后续逻辑页码随物理页增减，遇到显式重新编号则停止传播。回归还覆盖文档末节、重新从 7 开始编号，以及后方含奇数起始节时回到完整分页。每台机器用同一个 Release 程序交替插入、删除共 100 次。macOS 完整分页与编辑加增量分页的 P50 为 1.200 → 1.028 ms，P95 为 1.877 → 1.764 ms。UOS 的 P50 为 2.001 → 1.867 ms，P95 为 4.359 → 4.204 ms。两端分别比较了 710／715 个页面对象和 2,130／2,145 个命中点，并对受影响页和邻页核对 RGB 与 PDF。跨节续注、带字段的页眉页脚及奇偶页布局仍保留完整分页。[实现与回退](../../../../crates/joyword-layout/src/incremental/sections.rs)

GUI 复跑用 `python3 scripts/joyword/check-section-reflow.py DESKTOP NEW_DIR --grow --compare-cli OLD_CLI NEW_CLI`。Release 计时入口为 `cargo run --release -p joyword-engine --example chapter_reflow -- NEW_DIR/原始分节.docx --grow`。同一轮完整回归在 macOS 的 Word／Deck／Sheet 分别通过 3,681／1,001／1,473 项，在 UOS 分别通过 3,685／1,006／1,473 项。[原始采样](page-count-reflow.json)

## JoyDeck 的 12 种动作按钮与透明填充

“帮助”现在显示问号及圆点，“声音”显示喇叭与三道声波，“文档”分别绘制纸面和折角。12 个按钮按 DrawingML 附件分别构造底板、图标、阴影和描边。缩放时图标保持方形，边框填满整个按钮。四个连接点从右侧开始顺时针排列。在 45% 不透明度测试中发现，部分镜像图标与底板轮廓同向时，底板会多涂一次。现在按奇偶规则给图标留孔，双平台都通过了透明填充的独立图像对照。[实现](../../../../crates/joydeck-geom/src/preset/action_buttons.rs) · [DrawingML 预设附件](https://ecma-international.org/wp-content/uploads/ECMA-376-1_5th_edition_december_2016.zip)

macOS 与 UOS 各检查横、竖、方三种尺寸及普通、带文字、半透明、虚线四种呈现，共 12 张 960 × 540 页面、144 个形状实例。参照由旧版将标准附件作为自定义几何独立渲染。每版重新打开新版生成的 PPTX 后，结果与各自初次打开原件相同。相对独立参照，每张页面有 166 至 438 个像素的 RGB 不完全相同；逐点双向检查，一像素邻域之外的填充／背景／描边类别差异为零，通道强度超出邻域容差的点每页最多 25 个，出现在竖版虚线页。已有 29 个流程图的六张组合页也重验，两代结果逐字节相同。[双平台图像摘要、连接点及工程回归](action-buttons.json)

12 种按钮另验六种宽高组合，包含零宽、零高和双零。实机回归按独立参照取样，每端验证 5,240 次旋转前后的指针命中或排除，另在原生窗口实际移动、拉伸声音按钮、撤销、重做、保存副本并重开。两端均核实原件 SHA-256 相同。复跑用 `python3 scripts/joydeck/check-flowcharts.py OLD_CLI NEW_CLI PRESET_XML NEW_DIR --family buttons --desktop NEW_DESKTOP`。[复跑入口](../../../../scripts/joydeck/check-flowcharts.py)

## JoySheet 数组广播、条件函数与 MAP

本机 Microsoft Excel 16.115 实际重新计算并保存了五份合成工作簿。两份给出输入变化前后的 204 格参照，另外三份包含 69 个公式。旧 JoySheet 在其中 48 个公式的 87 格与 Excel 有差异。两平台的新版对这 69 个公式各检查 1,104 个位置，值、错误和空白均与 Excel 一致。例如 IFERROR 的替代数组可以扩大溢出范围，MAP 遇到某个参数没有对应坐标时会在该位置产生 `#N/A`。日期函数还区分首个引用错误与后续提升出的元素。[Excel 数组行为](https://support.microsoft.com/en-us/excel/dynamic-array-formulas-and-spilled-array-behavior) · [IFNA](https://support.microsoft.com/en-us/excel/functions/ifna-function) · [公式与期望](../../../../scripts/joysheet/tests/data/broadcast-conditions.json)

新旧版本相互打开对方保存的 XLSX，四种组合均保留公式。旧版重算后显示其原有结果。macOS 与 UOS 各在真实窗口修改 A1、撤销、重做、保存并用新进程重开，五次各核对 204 格。两份授权业务原件均为零公式，派生副本分别命中 916 和 16 个公式，七份仓库文件也通过旧版读回。两端 Sheet 产品回归各通过 1,475 项，原有一项忽略。[双平台结果与发行程序指纹](array-broadcast.json) · [桌面与双版本复跑](../../../../scripts/joysheet/check-broadcast.py) · [Excel 独立复跑](../../../../scripts/joysheet/check-broadcast-excel.py)

在 4,096 × 8 广播范围上交替运行两版发布程序，每次打开工作簿、改六次数值并核对三个结果。首批各 40 个样本，macOS 的 P95 为 38.25 → 38.00 ms，UOS 为 53.96 → 56.89 ms。单独再取各 120 个样本，macOS 的 P50／P95 分别为 35.84 → 35.57 和 36.99 → 36.73 ms，UOS 为 40.93 → 40.44 和 49.27 → 49.32 ms。这一项交付的效果是计算结果与 Excel 一致，UOS 尾延迟未见稳定改善。[全部原始采样](array-broadcast.json)

## JoyWord 拼写候选生成

建议引擎复用一块 UTF-8 缓冲生成替换、相邻键、增删字母、交换和缺失空格的候选。MAP 只组合具有可替换字母的位置，在 4,096 个候选的上限内保留候选顺序和费用。词干筛选逐字符统计相邻字母，在 48 个位置的优先队列中保留评分最高的词干。同分的现有词干保留原槽位顺序。新词干分数更高时，替换排序最靠前的最低分槽位。[字典格式](https://github.com/hunspell/hunspell/blob/master/man/hunspell.5) · [候选实现](../../../../third-party/craft-libs/proof/src/affix_suggest/edits.rs)

75 个正式包各取词干、换位、删字、插入、替换、重复、大小写及词典提供的混淆字符。每台机器共比较 1,499 个输入、1,196 处拼写标记，其中 1,179 处有建议。两版的词区间及完整建议顺序一致。macOS 和 UOS 的原生“审阅”窗口均显示了同序的五个建议，替换、撤销、重做、保存并重开成功。Word 产品各通过 3,681／3,685 项，单独运行词典库各通过 479 项，原有一项忽略。两端生产库严格 Clippy 通过。macOS 另对导入库的所有测试目标运行严格 Clippy，新旧版均有 476 处既有的测试断言用法诊断。[逐语言和原生验证](spelling-candidates.json) · [双版本复跑](../../../../scripts/joyword/check-spelling-order.py)

用两个提交的相同 Release 示例加载真实词典，每批查询荷兰语 21 词、波兰语 22 词、西班牙语 19 词，每版每语言交替取 40 个样本，逐个检查有序建议。macOS 的整批 P95 分别为 342.25 → 324.28、313.80 → 273.25、50.00 → 45.80 ms，UOS 分别为 369.69 → 345.47、592.95 → 542.70、73.03 → 65.29 ms。计时从词典加载完成后开始。语言包格式和已安装词典原样可供旧版打开。[原始采样](spelling-candidates.json) · [计时复跑](../../../../scripts/joyword/check-affix-candidates.py)

## JoyWord 前后缀匹配与配对环缀

词典读取器在内存中为词首、词尾分别构建压缩前缀树。每次查词只访问匹配的字节，保留空词缀及复合词的步数预算。前缀与两层后缀按词干和续接规则核对交叉许可，配对环缀可以识别 `neworkables`，也会拒绝多加一个 s 的 `neworkabless`。两台机器的实际审阅窗口都给出正确替换，并通过撤销、重做、保存和新进程重开。[词典格式](https://github.com/hunspell/hunspell/blob/master/man/hunspell.5) · [实现](../../../../third-party/craft-libs/proof/src/affix/edges.rs)

macOS 和 UOS 各自从官方源码构建了 Hunspell 1.7.5。对 256 种语法逐一检查六个词形，旧版与参照有 20 处差异，新版全部一致。UOS 自带的 1.7.2 在这 1,536 次判定中还接受了 32 个裸前缀实例，单独保留该版本的记录。75 个正式包共检查 15,528 个词形。西班牙语（乌拉圭）、亚美尼亚语、泰米尔语原词缀表有四处组头计数与实际规则数不符，外部参照报告加载失败。修正参照临时副本的这四个计数后，正式包的词形均一致；新旧 JoyWord 全程读取发行原件。另取 1,499 个输入核对两版拼写标记和完整建议顺序，全部一致。[独立参照发行版](https://github.com/hunspell/hunspell/releases/tag/v1.7.5) · [逐包结果与参照修正](affix-recognition.json) · [复跑脚本](../../../../scripts/joyword/check-morphology.py)

Word 产品回归在本机通过 3,681 项，在 UOS 通过 3,685 项。独立词典库在两端各通过 482 项，原先忽略的一项配对环缀测试已恢复。两端生产库严格 Clippy 通过。荷兰语、波兰语、西班牙语整批建议查询每版各取 40 个交替样本，P95 在 macOS 分别为 290.09 → 285.53、240.71 → 241.21、45.74 → 43.18 ms，在 UOS 分别为 338.18 → 312.58、526.89 → 532.57、79.04 → 55.90 ms。波兰语的尾延迟未改善。现有词典文件、压缩格式和旧版读回入口继续可用。[原始计时及回归摘要](affix-recognition.json)

## JoyDeck 折线标注与折点编辑

直线、单折和双折标注共有十二种边框与强调线组合。选择一个标注会显示二至四个折点，拖动时可预览引线。旋转和水平翻转后的框外折点也能选中和移动。编辑以一个可撤销的事务更新横、纵两个调整值。保存原 PPTX 时保留调整列表上的扩展属性、额外公式及未知子元素。遇到重复、计算得到或超出原读取预算的目标参数，保存会报告原因并保留原件。[折点几何](../../../../crates/joydeck-geom/src/preset/line_callout.rs) · [保存实现](../../../../crates/joydeck-pptx/src/preserve_content.rs)

在 macOS 与 UOS 25 上，每台分别渲染十二种标注的横、竖、方三种比例，覆盖实色、文字、透明填充、虚线、非对称调整和边界重合。每端 216 个实例与旧版 CLI 渲染的标准自定义几何逐像素一致。新旧版本分别重开新写出的 PPTX，画面均与各自打开原件时相同。两端还验证了零度和 37 度时的 5,772 处指针命中，单测另覆盖框外引线和无描边对象。[标准附件](https://ecma-international.org/wp-content/uploads/ECMA-376-1_5th_edition_december_2016.zip) · [图像与命中结果](line-callouts.json)

两端真实窗口对同一形状应用 19 度旋转和水平翻转，使用系统鼠标事件拖动第三个折点，分别核对撤销、重做、保存及新进程读回。旧版也能读到已移动的引线。macOS 首次复跑偶发缺少抬键事件，事件记录显示 GPUI 仍在接收拖动。改为同一 Quartz 事件源发送完整鼠标序列后，连续三次通过。UOS 使用 X11 鼠标事件通过。两端 Deck 产品回归分别通过 1,005 和 1,010 项。[实机结果和发行指纹](line-callouts.json) · [原生拖动复跑](../../../../scripts/joydeck/check-callout-drag.py)

## Word 与 Deck 的自定义几何

DOCX 和 PPTX 现通过同一个 DrawingML 内核求解 17 种坐标公式，将可顺逆时针绘制的椭圆弧转成三次贝塞尔曲线。两种产品分别构建自己的编辑模型。Deck 复用相同尺寸路径的指南值。Word 保留原始 XML 片段及其继承的命名空间和兼容性声明，改正文时仍能写回导引线、圆弧与厂商扩展。超出保留预算、不能重绑的关系和与原始曲线冲突的点编辑会阻止写出。[共享求值器](../../../../crates/joyoffice-drawingml/src/guide.rs) · [圆弧](../../../../crates/joyoffice-drawingml/src/arc.rs) · [Word 原始几何](../../../../crates/joyword-docx/src/geometry_source.rs)

macOS 和 UOS 25 各在 Word、Deck 中打开同一份含 64 条路径的图形，对照独立计算的数值路径。JoyWord 屏幕的 2,585,088 个颜色通道中仅两个相差 1，PDF 光栅逐像素一致。JoyDeck 屏幕和 PDF 均逐像素一致。两端分别在 Word 窗口改正文，在 Deck 窗口移动并缩放图形，均验证撤销、重做、保存、新进程重开及原件摘要。两代 CLI 都能读取写出的文件。旧版 Word 自身无法正确绘制这些命名坐标，回退时应保存原 DOCX，并用本次之前的可读副本继续处理。两种受保护 DOCX 均按预期拒绝生成目标文件。[逐端数据与回退](custom-geometry.json) · [复跑脚本](../../../../scripts/verify_shared_geometry.py)

## JoyDeck 标准预设与文本框

画廊 183 项中，74 种先前已逐族替换，108 种现在由 JoyOffice 在构建时把 ECMA-376 第五版附件编成只读几何表。最后一项是文本框。两台真实桌面均从编辑器插入中文文本框，执行撤销、重做、保存并由新进程重开。再生成的 PPTX 保留文本、矩形几何和文本框属性。标准附件原件的摘要、许可和编译器处理的两个勘误已登记。[来源及许可](../../../../licenses/ecma-376/NOTICE.md) · [双平台记录](standard-gallery.json)

参考图来自 Mac 上的 PowerPoint 16.115。12 份演示文稿各有三页及每页 27 个对象，覆盖宽、长、正方形三种尺寸和默认、非对称、边界参数。macOS 和 UOS 都核对了 972 个文字框及有序连接点集合、41,192 个普通或旋转的填充区点击结果。另测四种标注的框外尖端。两版 JoyDeck 对各自重开的 36 页均生成相同图像。独立 PDF 与两端的新图像在允许一个像素邻域后有 38 个轮廓和 11 个描边像素差异，旧版分别为 16,505 和 14,021。正常字号的星尖和细线仍留有少量栅格差异。[语料与几何检查](../../../../scripts/joydeck/check-standard-presets.py) · [导出对照](../../../../scripts/joydeck/compare-preset-exports.py)

复验时用该脚本的 generate 子命令生成演示文稿。用 PowerPoint 导出原件 PDF，再用 pdftoppm 以 72 DPI 为每页生成 PNG。verify 子命令接受语料目录、一个新输出目录和本地两版 CLI 路径，desktop 选项连接实际产品。导出对照读取参考 PNG 与 verify 产物。将生成的 geometry.txt 路径传给环境变量 JOYDECK_PRESET_GEOMETRY_REFERENCE 可执行全部 972 组；常规单测内置跨 108 种图形的取样。

### Word 派生词建议

JoyWord 现在沿词典实际授权的前后缀生成建议。对于分列的同形词，前缀和后缀必须由同一条词根记录或合法的后续规则授权。每个词根和每次建议请求都保留工作量上限。词干、单后缀和原有组合仍优先获得枚举配额，新支持的三重组合使用剩余配额。回退可用上一版 JoyWord 直接打开相同的词典目录和已保存的 DOCX。[实现](../../../../third-party/craft-libs/proof/src/affix/forms.rs)

macOS 和 UOS 对 4,608 种语法各做了 27,648 次独立 Hunspell 1.7.5 核对，旧版有 400 处差异，新版为零。75 个正式词典取样 883 个词根，共检查新旧输出的 100,859 个词根／词形组合。新版新增 11,084 个可识别的组合，旧版的 215 个无效候选已消除。移除项另有 305 个词可被整本词典识别。仅保留相应词根的所有同形记录再查，这 305 个均无法由该词根派生。三个包的外部参照沿用上节记录的组头计数修正。[词形回归](../../../../scripts/joyword/check-derived-forms.py) · [逐语种结果](derived-forms.json)

两端分别对 1,499 个拼写输入复查了问题位置、语言和候选顺序。亚美尼亚语、立陶宛语和巴西葡萄牙语共四个输入的建议有变，保留候选的相对顺序一致，新增建议全部经 Hunspell 识别。两端原生窗口都显示并应用了此前缺失的环缀建议，随后撤销、重做、保存和重开。UOS 用户的 DDE 会话当时处于锁屏，窗口键盘操作在同一台 UOS 机器的独立 X11／KWin 会话完成。两端各通过 484 项校对库测试；产品和共享 UI 也都通过。荷兰、波兰和西班牙语建议另各跑了 40 轮交替计时。低负载复测中，荷兰语批次 P95 在 macOS 由 266.9 增至 274.4 ms，在 UOS 由 303.1 降至 295.2 ms。原始并行构建期间的计时单列保留。[实际审阅复验](../../../../scripts/joyword/check-derived-review.py) · [计时与发行依赖](derived-forms.json)

### Sheet 动态引用、区域格式及重算

Excel 16.115 的英文和简体中文原生实例分别重算、保存了两份不同输入的工作簿，各含 138 个期望值。JoySheet 在两个系统上分别检查了 552 个缓存，旧版有 164 个差异，新版为零。语料覆盖 R1C1 的绝对、相对和整行整列坐标，含感叹号与单引号的表名，命名区域，溢出锚点，OFFSET 的负宽高、空参数和越界，以及间接引用第二参数的错误传播。区域语言决定字符串内的表格项目名；例如同一个表头在两种 Excel 中分别用 `#Headers` 和 `#标题`。XLSX 直接公式保持标准项目名。CLI 和桌面共用这条路径，进程区域优先采用 LC_ALL、LC_MESSAGES、LANG，缺省时读取系统设置。[微软结构化引用说明](https://support.microsoft.com/zh-cn/excel/using-structured-references-with-excel-tables) · [独立工作簿](../../../../scripts/joysheet/tests/data/dynamic-references) · [逐格结果](dynamic-references.json)

macOS 原生窗口和 UOS 真机的独立 X11／KWin 窗口各用两种区域设置编辑了引用地址和其新目标，再撤销、重做、保存并由新进程重开。每个平台执行 252 次桌面格值检查，两代 CLI 也互读了各自写出的副本并核对全部 138 个公式文本。回退可用旧版打开同一副本；重算后会恢复旧版在表中登记的差异。公开语料仅移除了 Excel 写入的本机绝对目录和作者元数据。[复验脚本](../../../../scripts/joysheet/check-dynamic-references.py)

同一脚本的 `--benchmark-only` 使用 8,192 条公式，改变被引用单元格，检查最后一个公式的结果，交替运行新旧桌面同配置的 CLI 各 20 次。耗时包括进程启动、打开文件和重算。在这次运行中，R1C1 工作簿 P95 在 macOS 由 154.68 降至 152.08 ms，UOS 由 121.63 降至 115.35 ms；普通 A1 对照分别为 147.00 → 147.70 和 106.56 → 104.25 ms。三产品工程回归和两端严格 SPDX 生成均通过。[回归、计时与依赖](dynamic-references.json)

### Word 奇偶起始页及镜像页边距

同一份 70 节 DOCX 含非对称镜像边距、装订线、奇偶分节、独立页码重起、三条编号脚注及浮动嵌套表格。macOS 原生窗口在第 35 页改正文后从 69 页增至 73 页，UOS 的独立 X11／KWin 窗口增至 75 页。两端均实际撤销到 69 页，重做及保存后由新进程重开。两代 CLI 在各平台互读，分别核对全部 73 页和 75 页的 PDF 文本，原文档摘要一致。平台之间沿用各自已有字体环境。[逐平台页数、命中及工程回归](facing-pages.json)

局部排版会记录进入章节前的物理页数及显示页码。若节末多出一页，可继续排到后面的奇偶分节或页码重起处，再核对下一节的真实首段。超过 16 页、256 个块或三个后续分节边界时使用完整分页。两端的差分回归逐页核对内容、命中索引和脏页两侧的位图与 PDF。用复验脚本加 `--facing` 可重跑窗口、保存及两代读回。[复验脚本](../../../../scripts/joyword/check-section-reflow.py)

### JoyDeck 放映转场的 GPU 合成

放映窗口用 Metal 或 Vulkan 合成旧页、新页、背景、透明内容、纯色和透视层。图像源、输出纹理和读回缓冲按一对幻灯片及实际设备像素尺寸复用。GPU 回读得到的帧继续交给媒体叠加和 GPUI 窗口。含动态 GIF 或形状动画的输入、逐对象 Morph、没有硬件适配器及超预算场景沿用 CPU。命令行渲染和既有确定性缓存接口保留原有结果。打开窗口前设置 `JOYDECK_GPU_TRANSITIONS=0` 即可在同一程序里切回 CPU。[合成器](../../../../crates/joydeck-render/src/transition_gpu.rs) · [放映接入](../../../../crates/joydeck-shell/src/show_render/transitions.rs)

Apple M4 和 UOS 真机的 Radeon 890M 各自运行了 49 种效果、185 组效果及选项、五个进度点。64×36 和 193×109 两个尺寸各 925 帧。每端共 740 个首尾帧与 CPU 逐字节相同。全部帧的最大 8 位通道误差在 Metal 为 1，在 Vulkan 为 2。另按两种尺寸各验证 48 个缺少旧页或背景、内容分层的组合。测试中特别校准了极薄的碎片边缘和奇数尺寸正中央的扇形接缝。[逐端数值](transition-gpu.json) · [像素复验入口](../../../../crates/joydeck-render/examples/transition_pixels.rs)

两端的真实窗口分别以 GPU 和 CPU 播放同一份 PPTX，暂停、恢复并退出。每个暂停画面取 3,864 个位置，与旧版 CLI 在同一时刻的独立渲染核对。Mac 截图先用显示器 ICC 配置转到 sRGB；UOS 的独立 X11／KWin 会话实际选中了 AMD Vulkan 硬件。两代 CLI 重写并互读了 PPTX，各自读回 4,800 ms 的时长，淡化和棋盘格中间帧在四种读写组合下相同。工程验证构建的窗口帧准备 P95，macOS 的淡化为 81.34 → 41.18 ms，棋盘格为 611.69 → 39.97 ms；UOS 分别为 52.00 → 36.70 ms 和 289.30 → 46.94 ms。这项计时包括工作线程和 GPU 回读，屏幕结果由暂停截图单独验证。[窗口、回退与文件互读脚本](../../../../scripts/joydeck/check-transition-window.py)

### Deck 基础转场和自动翻页

JoyDeck 现在用同一组归一化坐标描述推入、覆盖和揭开。PowerPoint 16.115 在 macOS 上导出的 25 页视频共 1,752 帧。验证器采样 24 种效果／方向组合各 15 个时刻，记录进场面积、重心和颜色。覆盖和揭开的轨迹只用向左、向左上两种方向校准，其余方向的 180 个样本平均位移误差从 2.37% 降至 0.97%，P95 从 7.41% 降至 4.86%。在另一个包含纹理和透明图层的 925 帧回归中，变化的 46 帧全部属于覆盖和揭开。端点 370 帧完全相同。回退到 `f5810bd0` 可直接打开新版写出的 PPTX。[原始测量](../../../../tests/fixtures/joydeck/slide-motion-reference.json) · [生成和复验脚本](../../../../scripts/joydeck/check-slide-motion.py) · [格式规范](https://ecma-international.org/publications-and-standards/standards/ecma-376/)

同一批参考文件暴露出自动翻页会截断转场。设置 1,600 ms 转场和 767 ms 停留的 22 页，在旧播放器中全部提前翻页；新版在 2.366 秒仍显示原页，2.368 秒已前进或到达结束画面。双平台另用真实窗口验证了自动播放、暂停和恢复，各用两代读写器交叉读回保存的 PPTX。macOS 的 Metal 与 CPU 均验证了相同时间的暂停画面。UOS 真机在登录界面时验证了自动和手动 CPU 回退。用户会话恢复后，同一程序在独立 KWin 窗口选中 Radeon 890M／Vulkan，推入和覆盖转场完成暂停、恢复及自动翻页。两种画布各 925 帧，最大通道差均为 2。开发用例强制使用 llvmpipe 时，另一个立体特效有一个通道相差 3 级，超出硬件对照采用的 2 级容差。[双平台结果](slide-motion.json) · [微软的自动翻页计时说明](https://support.microsoft.com/en-us/powerpoint/set-the-timing-and-speed-of-a-transition) · [上一轮硬件结果](transition-gpu.json)

## Sheet 名称依赖、数据库函数与文件读回

同一公式中的相对名称按所在单元格解析，工作表局部名称优先。运行时取得的范围可跟随 INDIRECT、OFFSET 和溢出目标移动；不完整的读取、迭代计算以及超出范围预算的情况使用保守调度。修改无关单元格时，Excel 易失函数仍会计算。通过 `JOYSHEET_REFERENCE_DEPENDENCIES=0` 可在同一程序中切回保守调度。[微软关于易失函数和重算的说明](https://learn.microsoft.com/en-us/office/vba/excel/concepts/excel-performance/excel-improving-calculation-performance)

Excel 16.115 分别保存三种输入状态。205 个公式覆盖数据库分类的全部 12 个函数，包括错误传播、单条／多条／零条匹配、公式返回的空字符串、计算条件的相对与绝对引用，以及名称、表格和溢出目标。每个平台比较 615 个带类型的缓存，旧版 111 处差异，新版为零。每份文件经过两代读写器的四种组合。真实编辑窗口还核对六个编辑、撤销、重做和重开状态，每个状态比较 38 格。桌面输入 `SUM(Sales)` 后，保存得到与 Excel 相同的 `SUM(Sales[])`。两份授权业务原件均无公式，另在副本验证 916 和 16 个计算结果。原件摘要保持一致。[双平台结果与重算计时](reference-dependencies.json) · [验证脚本](../../../../scripts/joysheet/check-reference-dependencies.py) · [DSUM 语义](https://support.microsoft.com/en-us/excel/functions/dsum-function)

## Word 页眉页脚、绘图锚点和跨节续注

70 节合成 DOCX 含奇偶页与页码重起、首尾链接的页眉页脚、双栏、浮动嵌套表格、以段落为锚点的环绕图片和延续到下一节的脚注。正文和五个运行页眉页脚分别打印页码、总页数、节序及节页数。旧版和新版的打印文本与每张 48 DPI 页图逐页相同。macOS 实际窗口在修改、撤销、重做后分别为 75、73、75 页；UOS 真机 Wilmer 的 DDE 桌面分别为 77、73、77 页。54 个续注段落在两端各跨三页。两台机器都从第一页起刷新总页数，并重开两代产品各自保存的文件。[双平台记录](section-stories.json) · [窗口与 PDF 复验](../../../../scripts/joyword/check-section-reflow.py)

对同一 DOCX 的每次编辑，另将增量结果与独立完整分页逐页比较，包括所有放置对象、段落索引以及正文、页眉和页脚的点击位置。模型回归覆盖页眉里的 PAGEREF、STYLEREF 和页脚的直接编辑。页脚增高侵入正文，或远端正文字段因总页数变化而影响换行时，编辑器自动完成整篇重排。两代读写器的四种组合均成功重开，保存副本仍包含字段、图片关系和表格。[逐页及命中检查](../../../../crates/joyword-shell/examples/verify_section_reflow.rs) · [边界回归](../../../../crates/joyword-layout/src/tests/section_checkpoint.rs)

## Deck 擦除和分割

在 PowerPoint 16.115 中把 13 页双色演示文稿导出为 912 帧视频，使用向右擦除的样本确定透明度曲线。其余 11 个方向留作验证，每个方向取 15 个时刻、每个时刻取 72 个像素。JoyDeck 在 macOS 和 UOS 上的平均覆盖率误差均由 1.94% 降至 0.39%，第 95 百分位由 7.17% 降至 1.18%。斜向擦除直接画移动带内的三角形，分割在两个半页上使用同一条曲线。[实测数据和双平台回归](soft-masks.json) · [可重建语料和参考采样](../../../../scripts/joydeck/check-soft-masks.py)

Apple M4 Metal 与 UOS Radeon 890M Vulkan 各对 1,850 帧执行像素比较。真实放映窗口分别用硬件与 CPU 播放斜擦和中心分割，核对暂停位置、恢复及结束颜色。PPTX 经新旧读写器的四种组合重开；恢复旧生成器可以直接播放新版本保存的演示文件。[窗口验证](../../../../scripts/joydeck/check-transition-window.py)

## Word 复合词、大小写和匈牙利语音节

新旧两代 JoyWord 在同一套词典上逐词与 Hunspell 1.7.5 的独立 C API 比较。两台机器各运行 11,385 个规则序列、7,092 个位置及词缀许可、312 个语言大小写和 3,080 个匈牙利语音节用例。75 个发行包另有 5,719 个结果，旧版两处差异在新版均已校正。92 个合成用例的词尾带有显式禁用的词根，JoyWord 旧版和新版均按原有词典契约拒绝，Hunspell 的组合规则接受；其余合成结果一致。程序继续限制字节、规则数、词缀搜索工作量和同形词数量。[逐项计数、耗时与双平台回归](compound-permissions.json) · [规则与语料](../../../../scripts/joyword/check-compound-permissions.py) · [Hunspell 1.7.5 格式](https://github.com/hunspell/hunspell/blob/v1.7.5/man/hunspell.5)

在两端的实际 JoyWord 校对窗口中，输入 `redsunenn` 可见 `redsunen` 和 `redsunens` 两项建议。已更正的复合词和土耳其语大写词通过撤销、重做、保存和全新进程重开。两代读写器的四种组合均保留了正文和大小写。同一发行目录仍可供旧版读取，构建时使用的语法与语言数据保留原有归属。[复合词实现](../../../../third-party/craft-libs/proof/src/affix/rule_program.rs) · [来源和许可](../../../../licenses/craft-libs/SOURCE.json)

## Sheet 工作日历和 Excel 二进制数值

JoySheet 现按周和排除日期的序号计算工作日。两端各对照了十二份 Excel 16.115 工作簿的 3,268 个缓存，旧版差异 691 处，新版为零。样本覆盖倒序区间、重复或连续假日、七日全休、零位移、日期上界、1904 纪元和英中文本日期。两次 Excel 原生编辑还提供了节假日变更的独立期望。JoySheet 的原生窗口在 1900／1904 样本分别跟踪到 27／24 个受影响的结果，撤销、重做、另存和新进程重开均通过。[平台数据及回退](workday-calendar.json) · [可重复验收](../../../../scripts/joysheet/check-workday-calendar.py)

Release 函数微基准中，连续排除 1,200 个工作日时，每次正向或反向选择的中位数在 macOS 从 425.97 降到 6.13 微秒，在 UOS 从 579.93 降到 10.95 微秒。12 个排除日分别从 0.218 降到 0.193、从 0.304 降到 0.253 微秒。两份授权业务表的原件哈希核对通过，两份临时副本的 916 和 16 个派生公式经两代程序读回。[基准源码](../../../../crates/joysheet-functions/examples/workday_calendar.rs)

XLS 与 XLSB 共用压缩数值和错误码读取，四份 Excel 导出各检查 70 个非空单元格。XLS 每份的 50 个公式经另存 XLSX 后仍可重算。XLSB 按现行导入契约保留类型和缓存，并在桌面及 CLI 提示功能范围。每个输入均完成新旧版写入与读回的四种组合。仓库样本只含构造的数据，已移除本机目录和作者资料。[二进制读取实现](../../../../crates/joysheet-xlsx/src/binary_scalar.rs) · [来源记录](../../../joysheet/UPSTREAM-PATCHES.md)

## Word 组合图形、链接文本框与远端尾注

70 节 DOCX 的画布含有链接到下一节的文本框，另有组合图形和四个尾注。两端真实窗口在第 35 页插入正文、撤销、重做后保存，用新进程重开。macOS 的页数为 70→75→70→75，UOS 为 70→77→70→77。另一组模型测试分别编辑正文、框内文字、分节尾注和文末尾注，十二次结果均核对完整分页的每页对象、位置索引和相关正文／注释的点击命中。[实测记录](drawing-stories.json) · [桌面回归程序](../../../../scripts/joyword/check-drawing-stories.py)

链接文本框按原文的字节位置续排。在 20 框交替窄宽的样本里，macOS 导出的 150 个位置过去只有 120 个不同的词，UOS 的 100 个位置过去只有 80 个；新版分别显示连续的 150、100 个词。宽框在前的 48 词样本和等宽对照都完整。两版读回双方保存的 DOCX；主样本在 macOS 的 75 页栅格相同。UOS 仅第 43 页校正续排的文字有 161 个像素不同，其余像素及未改文字坐标相同。原件摘要均保持一致。[框内行选择](../../../../crates/joyword-layout/src/textbox/frame.rs) · [来源与指纹](../../../../licenses/wordcraft/SOURCE.json)

超过段落和单元格重复排版预算的长链使用最窄框的行宽。超出有界章节或无法证明后续排版相同的编辑仍完整分页。回退到 `7de605a7` 可以重开本轮另存的 DOCX，旧版在宽窄交替样本上会重现重复词。

## Deck 图案与时钟转场

以 PowerPoint 16.115 实际导出的 912 帧视频为参照，两端按相同时间在 JoyDeck 的实际 CLI 放映入口取样。圆、菱形和十字使用等透明度带生成图层，时钟按幻灯片自身的归一化角度推进，扩大和缩入保留整页纹理的缩放。随机条和溶解使用固定排列生成阈值。11 个选项的覆盖率误差分别低于旧版，9,720 个几何像素的平均误差从 2.78% 降到 0.64%。随机效果分别比较整帧的覆盖率、行均值及列均值的变化，45 个覆盖率样本的平均误差从 6.15% 降到 2.14%。[按效果分列的结果](pattern-transitions.json) · [参考视频测量与回放](../../../../scripts/joydeck/check-pattern-transitions.py)

macOS 的 Metal 和 UOS 25 的 Radeon Vulkan 各运行 1,850 帧 GPU／CPU 对照。两端真实观众窗口分别用自动硬件与 CPU 模式暂停圆形和楔形转场，检查暂停时钟对应的画面和结束后的整页。UOS 验证字母 P 快捷键时临时切换为直接输入，结束后恢复原输入法。两代程序均读回双方另存的 PPTX 方向与时长。[窗口与交叉读回](../../../../scripts/joydeck/check-transition-window.py) · [来源记录](../../../../licenses/deckcraft/file-map.json)

## Word 重复词与拆词建议

九种语言的重复词检测已接入同一个原文字节范围遍历器。12 个语言配置、115 个段落覆盖 40 个预期重复词，同时核对制表符、不间断空格、标点、数字、大小写和西班牙语组合重音。印尼语保留连字符建议的顺序，土耳其语只选中第二个词。两台机器上，新旧版的消息、建议与选区逐条相同。另用 75 个正式语言包的 1,499 个输入比较了拼写结果和建议顺序。[机器记录](language-boundaries.json) · [独立期望与原生窗口回归](../../../../scripts/joyword/check-language-boundaries.py)

在测试词典同时接受两种切分时，旧版会把空格插进字母和组合重音之间。新版只在整个字形簇之后提供空格。macOS 和 UOS 的真实「审阅 → 拼写和语法」窗口均显示了捷克语重复词和荷兰语拆词的预期建议。两端均校验当前选区、替换、撤销、重做、保存和新进程重开，并由两代程序交叉读写更正后的 DOCX，核对正文和语言标记。回退到 `c81da92d` 可继续使用原词典目录及文档。[字形簇分词](../../../../third-party/craft-libs/proof/src/affix_suggest/space.rs) · [按语言检测重复词](../../../../third-party/craft-libs/proof/src/repeated.rs)

校对库在两端各通过 486 个单测，生产库通过严格 Clippy。UOS 单独对第三方校对库启用含测试目标的严格 Clippy 时，基线和新版均报告 475 条，诊断类型与数量相同。三产品工作区的正式检查均通过。

## Sheet 现金流、日期与收益率

同一组 37 个现金流计划在两个工作簿中只改变首笔付款。Microsoft Excel 16.115 实际重算并写出 1,804 个参考值，覆盖五个函数、多个可能的根、异号、乱序日期、闰年、日期上下限、空白、文本和错误值。两端旧版分别出现 298 处差异，新版逐格符合公式、结果类型和数值容差。两代读写器交叉保存后各自重算。收益率与 Excel 的迭代值按绝对或相对误差 10⁻⁸ 比较，其余数值按相对误差 2×10⁻¹² 或绝对误差 10⁻²¹ 比较。[实测数据](cashflows.json) · [复现脚本](../../../../scripts/joysheet/check-cashflows.py)

macOS 和 UOS 真机窗口将第一笔付款从 −100 改成 −120，每端在修改、撤销、重做和新进程重开时检查 21 个随之变化的结果及固定抽样，每种状态实际读取 52 格。保存文件的 902 个公式缓存也与 Excel 对照。两份授权业务原件本身没有公式；用副本构造的 916 和 16 个公式在两端回归通过。这里的现金流金融兼容数字来自独立构造的 Excel 工作簿。[公式实现](../../../../crates/joysheet-functions/src/financial/cashflow.rs) · [可重复的内核计时](../../../../crates/joysheet-functions/examples/cashflow_recalculation.rs)

发布优化构建对每种计划长度交替运行新旧程序 11 次。1,200 笔付款的单次 XIRR 中位耗时在macOS 从 129.5 降到 27.1 微秒，UOS 从 316.2 降到 70.6 微秒。各函数及短计划的原始测量见机器记录。

## Deck 棋盘、百叶、梳形与波纹

棋盘格使用能独立翻转的 35 张小面，百叶与梳形分别按条带开合或滑离，波纹按中心或四角至边界的距离计算混合与阴影。PowerPoint 16.115 各导出一份纯色和一份带四个位置标记的 912 帧视频。每份取 12 次转场、15 个时点、72 个位置。11 个不同选项在两种材料上分别优于旧版。纯色页的 25,920 个纹理权重平均误差从 11.09% 降为 2.33%，带标记页的 38,880 个 RGB 分量平均误差从 10.13% 降为 5.04%。剩余误差包括边缘阴影和波纹内部的纹理折射。[逐项误差与二进制指纹](panel-transitions.json) · [测量与回放](../../../../scripts/joydeck/check-panel-transitions.py)

Apple M4 的 Metal 和 UOS 25 Radeon 890M 的 Vulkan 各完成 1,850 帧 GPU／CPU 取样。最大通道差分别为 1 和 2，满值为 255。两端分别在真实观众窗口以硬件和 CPU 模式暂停水平百叶、角点波纹，并复验默认的淡化与垂直棋盘。百叶与波纹每张截图检查 3,864 个位置，四组窗口在暂停时的越界样本均为零。截图先按物理像素裁出幻灯片，再与当时的 CLI 帧对齐。两代写出的 PPTX 均经两代读回并核对选项、方向及持续时间。UOS 测试结束后恢复原搜狗输入法。按族回退可使用提交 `2e8a8846`。[窗口回放](../../../../scripts/joydeck/check-transition-window.py) · [实现与测量依据](../../../../crates/joydeck-anim/src/transition_panels.rs)

## Word 跨章节的正文页码字段

编辑第 35 节时，文档前文的总页数、后文的页码以及重起编号后的显示页码会受到不同影响。Word 现在能定位受影响的正文字段，分别重排它们所在的章节，核对下一节入口及页码，再复用其余页面。同一节的后续编辑可以复用刚验过的字段排版。16 页和 256 个正文块的上限适用于本次一起重排的所有章节。另用超过上限的文档检查了完整分页回退。[实现](../../../../crates/joyword-layout/src/incremental/sections.rs) · [边界与输出对照](../../../../crates/joyword-layout/src/tests/section_checkpoint.rs)

macOS 原生窗口在 73 页与 75 页间执行插入、撤销和重做，UOS 真机在 73 页与 77 页间执行同样操作。每次均实际显示四处字段所在的页。保存重开后，两个版本对每页输出的 PDF 栅格相同。原本 54 个续注段落及嵌套表格保留，另核对了四种新旧写入／读回组合、页码重起及未改动的源文件。[机器记录](distant-page-fields.json) · [窗口复验](../../../../scripts/joyword/check-section-reflow.py)

相同文件通过发布优化构建交替运行两代排版器，每个版本每端 60 次编辑。旧版 60 次触发完整分页，新版 60 次使用局部结果。正常编辑入口的中位耗时在 macOS 从 64.16 降到 2.80 ms，UOS 从 60.83 降到 3.10 ms。每轮第一次增页和删页另比较整篇的页面内容、命中坐标、光栅像素及固定布局 PDF。回退提交为 `577d86b5`。在旧版源码上运行同一计时程序时，传入 `--grow 73 --allow-full` 即可计入原回退开销。[可复现计时入口](../../../../crates/joyword-engine/examples/chapter_reflow.rs)

## Sheet 的进制转换和位运算

JoySheet 的工程函数和数学函数共用整数转换实现。Excel 16.115 对两份构造的工作簿分别计算并保存，每份含 8,322 个公式。覆盖十位补码、位移边界、零的补位、超长文本、数字字符集和两个错误实参的优先级。对数值使用零容差，对文本和错误码逐字核对。macOS 和 UOS 上旧版分别有 938 和 941 格与参考值不同，新版两份均为零。两代程序分别读取两种写回结果，再算并比较公式、类型和值。[双平台明细及构建指纹](integer-formulas.json) · [独立参考的复验入口](../../../../scripts/joysheet/check-integer-formulas.py)

两个实际窗口均在 A1 从 7 改为 12 时重算，经历初始、编辑、撤销、重做、新进程重开五种状态。每次检查 510 格，包含所有 236 个发生变化的结果。另存后核对全部 8,322 个缓存。复用了同一套读取和桌面工具复验现金流的 1,804 个独立缓存。两份授权业务原表都没有公式，派生副本的 916 和 16 个公式在两版之间无差异，原件摘要一致。

两代优化构建对六种调用各交替计时 11 轮，每轮 100,000 次。以数值调用 BIN2DEC 的中位耗时在 macOS 从 230.38 降至 10.34 纳秒，在 UOS 从 329.62 降至 32.80 纳秒。48 位移位在两端分别由 11.44 降至 9.17、20.89 降至 19.62 纳秒。回退到 `6136f1aa` 可重开本批另存的 XLSX，旧版重新计算时会重现上述结果差异。[计时程序](../../../../crates/joysheet-functions/examples/integer_conversion.rs) · [Excel BASE](https://support.microsoft.com/en-us/excel/functions/base-function) · [Excel DECIMAL](https://support.microsoft.com/en-us/excel/functions/decimal-function) · [Excel BITLSHIFT](https://support.microsoft.com/en-us/excel/functions/bitlshift-function)


## Word 多语种标点与句首大小写

十种语言的空白和标点、其中七种的句首大小写由 `joyword-proof-rules` 计算。区域适配层继续提供本地化消息、缩写及语言数据。17 个语言配置各检查 668 段原文，逐项核对 UTF-8 选区、消息顺序和建议。新旧结果有 6,526 条相同诊断。独立预期确认消除了 69 处误报，包括英文小数前的空格和意大利语省略号。两端另用 75 个正式词典包检查 1,499 个拼写输入和建议顺序。[规则](../../../../crates/joyword-proof-rules/src/lib.rs) · [复验脚本](../../../../scripts/joyword/check-editorial-rules.py) · [Unicode 文本边界参考](https://www.unicode.org/reports/tr29/)

在 macOS 与 UOS 25 的审阅窗口中，波兰语 `..` 提供句点及省略号，土耳其语 `izmir` 提供 `İzmir`。两端均实际应用更正、撤销、重做、保存，再由新进程重开，两代 CLI 交叉读回 DOCX。独立校对库各通过 485 项测试。回退至 `18424801` 可继续使用原件和原词典目录。许可检查覆盖 101 个随附来源文件；三产品的桌面与 CLI 中，Word 两个入口依赖这套规则。六种语言逐一计时。首轮 macOS 的土耳其语和 UOS 的捷克语耗时上升；macOS 复测与 UOS 固定单核复测中，六种语言的中位耗时均下降。机器记录保留初测及复测的全部样本。[输出、计时与依赖记录](editorial-rules.json) · [来源清单](../../../../licenses/craft-libs/SOURCE.json)

## Deck 透视转场与 UOS 放映快捷键

立方体、盒子、前景旋转和轨道共用可裁剪的投影面，每种效果均支持四个方向。PowerPoint 16.115 分别导出纯色页、带标记页和两种宽高比的标定视频。用前两份视频的 16 个选项、15 个时点和每帧 72 个位置验收。纯色页的 34,560 个纹理权重平均误差从 4.75% 降到 2.00%，带标记页的 51,840 个颜色分量从 8.53% 降到 5.07%。带标记的 16 个选项均改善。纯色页中仅旋转前景的 8 个选项没有前景，前后两版结果相同。局部边缘仍可有满量程通道差。两种材料均经两代程序各写两次，并核对每种写入结果在两个读回版本中产生的画面和方向。[逐效果明细与指纹](prism-transitions.json) · [实测回放](../../../../scripts/joydeck/check-panel-transitions.py) · [MS-PPTX 转场字段](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-pptx/79f003b6-811b-4225-a0f6-30a3b58d663d)

在 Apple M4 Metal 和 UOS 25 Radeon 890M Vulkan 上，各比对两种尺寸的 1,850 帧。两台机器也分别在真实放映窗口使用硬件和 CPU 暂停立方体及轨道转场，逐张截图的 3,864 个位置均在验收容差内。UOS 初次用默认搜狗输入法放映时，按 P 未能暂停。共享 X11 后端现将没有文字接收器的窗口按键送给应用；正文、备注和单元格仍通过 XIM 组合及选词。原桌面搜狗保持启用，真实窗口重测通过。另以独立 Fcitx5 拼音和 macOS 系统拼音复验三产品的选词、撤销、保存及重开。[窗口与读回](../../../../scripts/joydeck/check-transition-window.py) · [X11 来源与许可](../../../../licenses/gpui/NOTICE.md)

按族回退可从 `18424801` 恢复原转场分派。X11 输入路由可独立恢复同一提交中的客户端、窗口和来源记录。旧版本能重开新程序保存的 PPTX。放映中的硬件合成仍可通过 `JOYDECK_GPU_TRANSITIONS=0` 切换到 CPU。

## Sheet 信息函数与复选框读写

八个 IS 类型判断、奇偶判断、`TYPE`、`N`、`ERROR.TYPE` 和 `NA` 已接入 JoySheet 自研实现。普通文件公式的隐式交叉、动态数组和单格旧式数组公式保留各自的录入方式。两份由 Excel 16.115 独立重算的工作簿各有 867 个公式，每份的旧版结果中有 51 处差异，新版均为零。原来另存会改变 176 个 `TYPE` 公式中 47 个的录入方式，Excel 重开后有 34 个值随之改变；新版两项差异均为零。两份新文件又由 Excel 实际打开、重算、保存并逐格复核。[两代读写与 Excel 结果](value-information.json) · [类型函数](https://support.microsoft.com/en-us/excel/functions/type-function) · [隐式交叉](https://support.microsoft.com/en-us/excel/implicit-intersection-operator)

这次还定位了 Excel 打开旧版另存文件时弹出的修复提示。分别替换文件部件，再单独改变复选框映射表的名称或数量属性。补上 `XFComplementsMapperExtRef` 后 Excel 可正常打开，只移除数量属性时仍要求修复。新读取器逐级检查映射类型，并要求引用指向已经出现的条目，兼容只有一个无名映射的旧文件。[文件试验](value-information.json) · [MS-XLSX 特性包](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/8383a417-7cb1-4324-af63-99dd77ffebd5)

macOS 与 UOS 25 各通过 1,491 项产品回归，原有一项忽略。真实 Sheet 窗口把 A1 从 7 改为 12，再撤销、重做、保存、重开；五个状态各检查 91 格，发生变化的 63 格全部覆盖。19 个进制和位运算函数另复验 16,644 个缓存值。两份授权业务表原件均无公式，派生副本的 916 和 16 个公式在两端全部吻合。六种函数调用的每次中位数，macOS 均下降，UOS 的小幅变化和全部原始样本见记录。函数与格式映射可共同回退至 `18424801`；旧程序能读取两版文件，旧写入器会重现上述录入方式和复选框问题。[原生回归、计时和回退](value-information.json) · [重放脚本](../../../../scripts/joysheet/check-value-information.py)

## Word 远端字段引起的连续分页

章节里的页字段从两位数变成三位数时，字段自身换行可能让该节再多出一页，进而改变其他节的总页数。Word 现在通过原文块和节位置查找待更新的字段，重算直到已验证的边界稳定。每次编辑共用 16 页和 256 个块的预算。撤销时先试上一次已经验证的边界。超预算或无法核实入口、出口时，仍用完整分页。五种配置逐一比较插入、撤销、重做后的所有页面场景、命中和行索引。[算法与逐次数据](field-propagation.json)

普通样本由 68 页增至 71 页，镜像页边距与偶数页起节样本由 69 页增至 73 页。12 个远端字段分别从 99 变为 102，58 段脚注跨两页，末节页码仍从 7 起。验收中发现桌面仍按旧页高定位，跳转新增末页时看不到对应页面。现已随分页结果重建滚动索引。macOS 和 UOS 各在两个实际窗口完成编辑、撤销、重做、前后跳转、保存与重开。两代 CLI 交叉读写，最终 71 页和 73 页 PDF 在同机逐页光栅一致，六页样本的邻页在插入和撤销后均可复用。[桌面验收](field-propagation.json) · [重放脚本](../../../../scripts/joyword/check-field-propagation.py)

相同 Release 配置下交错执行三轮、每版各 60 次编辑。macOS 的编辑至排版完成中位数从 19.39 ms 降至 1.29 ms，UOS 从 14.54 ms 降至 0.86 ms。两个平台的完整分页回退均从 60 次降至 0 次。Word 产品回归分别通过 3,699 和 3,703 项，各有两项原有忽略。产品门禁、库级 Clippy 和发布依赖许可检查均通过。额外将全部排版测试目标的 Clippy 警告提升为错误，前后两版均报 1,087 条，按诊断消息统计相同。回退时一同恢复旧分节分页器和桌面页面索引，保存的 DOCX 可由前一版重开。[回归、许可和回退](field-propagation.json)

## Word 冠词、省音和复句诊断

英语冠词现在按后接词的常见读音处理，包括静音 h、以辅音开头的元音字母和部分首字母缩写。规则会避开有不同读法的词。西班牙语按分句查找问号和叹号的起点，意大利语区分省音与截尾。波兰语和俄语限定从句及介词适用的上下文，土耳其语在连接词前区分空格、引号和换行。六种语言共 12 个地区配置的 318 个标注段落逐条核对原文范围、消息和建议顺序。旧版的 84 项诊断保持一致，移除或替换 17 项，增加或纠正 65 项。原有 17 个地区配置的 11,356 段、6,526 项诊断，以及 75 个发行词典的 450 个独立输入均通过复验。[语言用例与来源](lexical-rules.json) · [重放脚本](../../../../scripts/joyword/check-lexical-rules.py)

两台机器各在真实审阅窗口选中西班牙语和意大利语问题，核对显示建议，改正选区并撤销、重做、保存和重开。两代写入器保存的文档均经两代读取器读回。macOS 与 UOS 的 Word 产品回归分别通过 3,701 和 3,705 项，各有两项原有忽略。独立校对库每端另通过 485 项。每种负载交错测量七对 Release 样本。英语冠词在 macOS 从每段 7.64 降至 6.90 微秒，西班牙语问句从 21.36 升至 25.05 微秒，俄语编辑段落从 129.98 升至 148.37 微秒。UOS 固定同一核心后，12 种负载中位数均下降。各地词表、消息和词典解析仍有原来源代码。回退时可从 `a0a7db3f` 恢复相应语言分派及来源账本，原词典和已保存文档继续可读。[原始计时、回归及回退](lexical-rules.json)
