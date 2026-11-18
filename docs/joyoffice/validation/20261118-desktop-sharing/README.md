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
