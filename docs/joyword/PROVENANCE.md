# 独立实现、依赖与资产来源

版本 0.2 · 2026-10-13。对应 QUALITY-002。仓库代码许可证延续根目录 [MIT LICENSE](../../LICENSE)。本文件是工程来源规则，不把公开规范的可访问性解释为对所有知识产权的统一授权。

## 1. 独立实现规则

允许：公开格式规范、Unicode／字体标准、产品使用说明、公开 API 文档、经授权的黑盒操作和原创样例。规范中的元素名称与行为可用于实现；代码注释需标规范版本和条款。微软 Open Specification Promise 覆盖 ECMA-376、MS-DOCX、MS-OI29500、MS-DOC、MS-OFFCRYPTO、MS-CFB 等规范的必要专利主张，各规范另附实现用版权许可；这不涉及商标、字体或产品资产。

禁止：读取、复制或翻译微软 Office 与其他 Office 产品实现源码；禁止：读取、复制或翻译微软 Office 与其他 Office 产品实现源码；将 Open XML SDK、LibreOffice、ONLYOFFICE、Pandoc、Typst 或 Word 自动化作为产品内核；把公开产品截图、商标、图标和模板直接打包为 JoyWord 资产。

通用基础库可以复用、阅读和必要时修补；这包括 Rust UI、字体、Unicode、容器、图片和密码算法库。阅读 GPUI 的构建脚本及输入 API 不属于阅读 Office 实现。平台公开 API 和通用绑定不属于排除的 Office 实现。

## 2. 来源证据

每项关键实现记录：需求 ID、规范 ID／版本／条款、独立推导说明、原创测试、黑盒观察环境。行为无法从规范确定时，记录被观察软件的版本、操作输入、结果和备选解释，不把猜测变成规范事实。

研究 Office 开源项目限于 README、用户手册、公开能力矩阵、许可与发布说明。不要把其内部架构或代码逐段迁移。若来源不符合边界，先隔离对应实现并重新从规范和原创样例推导，任务不得标为完成。

## 3. 依赖登记策略

每个实际使用的直接依赖记录精确版本、来源、许可证、feature、用途、是否含原生代码及维护风险。Cargo.lock 锁定完整传递依赖；升级需执行相关语料回归。发布前生成包含传递依赖的 SBOM 和 THIRD_PARTY_NOTICES，不将本文件的候选表当作完整许可清单。

| 依赖 | 定位 | 许可／来源依据 | 注意事项 |
| --- | --- | --- | --- |
| GPUI：crates.io gpui-pre =0.3.9（19 个 gpui-pre* 包） | UI 框架 | [上游许可](https://zed.dev/software-overview)，Apache-2.0；Zed 主干 93d745668805029476e6d7fbb19652116fed51de 的快照，crates.io 所有者为个人（huacnlee），发布包无 .cargo_vcs_info.json | 编辑器主体许可不同；仅依赖框架；记录传递依赖；来源按 ADR-010，CI 逐文件比对 Zed 提交，见下文“gpui-pre 来源逐文件校验” |
| sha2 0.10.9 | joyword-model 资源内容散列（直接依赖） | RustCrypto，MIT OR Apache-2.0（crate 元数据） | 与 GPUI 间接锁定的版本相同；只用于身份散列，不用于密码学保护 |
| Ropey 1.6.1 | 仅 T100 文本存储基准的 dev 依赖 | [上游](https://github.com/cessen/ropey)，MIT | 不进入产品构建；基准结论见 IMPLEMENTATION |
| HarfRust 0.8.4 | joyword-layout 字体整形（直接依赖） | [上游](https://github.com/harfbuzz/harfrust)，MIT | 选此版本以与 krilla 共用 read-fonts 0.39；特性逐项显式开关；不根据库名称声称兼容 Word |
| read-fonts 0.39.2 | joyword-layout 字体表读取（直接依赖） | MIT OR Apache-2.0（crate 元数据） | 与 HarfRust、krilla 的 skrifa 0.42.1 共用同一版本 |
| Fontique／ICU4X | 后续字体和 Unicode 基础 | 各选定发行包的 LICENSE 与元数据 | 使用时登记，而非默认视作已加入 |
| Skrifa 0.42.1 | CPU 字形轮廓直接依赖 | [上游](https://github.com/googlefonts/fontations)，MIT OR Apache-2.0 | 与现有 PDF 共用 read-fonts 0.39.2；只重用原锁文件版本 |
| tiny-skia 0.11.4 | CPU 单色轮廓光栅直接依赖 | [上游](https://github.com/RazrFalcon/tiny-skia)，BSD-3-Clause | 重用原锁文件，默认 SIMD／PNG 特性；保留许可通知 |
| image 0.25.10 | app 的内存页面到 GPUI 图像适配；joyword-model 对 PNG／JPEG／GIF／BMP／TIFF 的预算内接纳与解码 | [上游](https://github.com/image-rs/image)，MIT OR Apache-2.0 | 复用原锁文件 0.25.10；默认特性关闭。模型只打开 png、jpeg、gif、bmp、tiff |
| quick-xml 0.41.0 | joyoffice-opc 包级 XML 解析（直接依赖） | MIT（crate 元数据） | 拒绝 DOCTYPE，只展开预定义实体与字符引用；与 GPUI 间接锁定的版本相同 |
| flate2 1.1.10（rust_backend）／crc32fast 1.5.2 | joyoffice-opc 的 deflate 编解码与 CRC-32；joyword-pdf 结构检查的单层 FlateDecode（直接依赖，同一钉扎） | MIT OR Apache-2.0（crate 元数据）；后端 miniz_oxide 为纯 Rust | ZIP 容器结构按公开的 PKWARE APPNOTE 自行实现，未使用 zip crate。PDF 结构检查只复用这一已钉扎版本做 inflate，没有升级，也没有引入 qpdf 或另一套 PDF 库 |
| unicode-linebreak 0.1.5 | layout 的默认 Unicode 候选断点（直接依赖） | [上游 API](https://docs.rs/unicode-linebreak/0.1.5/unicode_linebreak/)，Apache-2.0（crate 元数据） | 精确复用锁文件版本，Unicode 15.0；不据此宣称 Word 中文规则或词典分词 |
| unicode-segmentation 1.13.3 | shell 输入的默认扩展字素与词边界，以及引擎查找的字素对齐匹配（直接依赖） | [上游 API](https://docs.rs/unicode-segmentation/1.13.3/unicode_segmentation/)，MIT OR Apache-2.0（crate 元数据） | Unicode 17.0；复用已有锁文件包，引擎与 shell 共用同一钉扎，无新版本；不等同于中文词典分词或字体整形簇 |
| tempfile 3.27.0 | shell 安全创建同目录私有临时文件（直接依赖） | [上游 API](https://docs.rs/tempfile/3.27.0/tempfile/struct.NamedTempFile.html)，MIT OR Apache-2.0（crate 元数据） | 复用原锁文件版本；不使用可覆盖 persist，发布采用 std::fs::hard_link；无新外部包或升级 |
| winsafe 0.0.19（kernel，仅 Windows） | shell 的 Win32 文件身份观测与 `ReplaceFileW` 原件替换（直接依赖） | [上游](https://github.com/rodrigocfd/winsafe)，MIT；[ReplaceFileW 公开文档](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew) | 安全通用绑定，不在仓库写 unsafe；不使用 IGNORE_MERGE／ACL 标志，不把 Win32 成功码代替读回验证 |
| krilla 0.8.2（default-features = false） | joyword-pdf 的 PDF 写出、定位字形和场景图片（直接依赖） | [上游](https://github.com/LaurenzV/krilla)，MIT OR Apache-2.0；传递依赖含 pdf-writer、subsetter、xmp-writer（MIT OR Apache-2.0）与 tiny-skia-path（BSD-3-Clause） | 关闭 simple-text（其 rustybuzz 整形）。joyword-pdf 打开 raster-images，把已接纳的场景图片画进 PDF；版本仍是 0.8.2。写出时 `xmp_metadata` 为 false，且不设置 creation date，避免时钟和实例标识进入字节。传递依赖仍含 pdf-writer、subsetter、xmp-writer 与 tiny-skia-path |

MIT 主项目不能删去第三方 Apache 等许可证的通知。当前锁文件已含 MPL-2.0（cbindgen、option-ext，以及 Windows 字体用的 dwrote）、Unicode-3.0、BSD、ISC、Zlib 和 CC0-1.0 等许可，均须进入通知清单。根目录 deny.toml 由 cargo-deny 在 CI 中检查许可白名单、RustSec 公告、通配版本和注册表来源；经 GPUI 间接引入的“停止维护”公告逐条登记理由，迁到 gpui-pre 0.3.9 后由 7 条减为 3 条（paste、rustybuzz、ttf-parser），每次升级 GPUI 快照时复查，JoyWord 直接依赖不得列入忽略。是否允许特定依赖应按实际许可与分发方式检查，不笼统地把全部开源代码认定为可用或不可用。

## 4. 字体、图标、模板与样例

- 样例文字由项目原创或明确授权；不得默认把用户合同、邮件或公司资料提交到仓库。
- 字体记录来源、版本／散列、许可证、face index 和嵌入限制。系统字体可以用于本地测试，不据此获得重新分发权。
- 持续集成优先使用许可明确、可重新分发的固定测试字体；生产版使用系统字体与可选字体包。
- 当前 CI 的 T003 作业在运行时从 Ubuntu 软件源安装 fonts-noto-cjk（SIL OFL 1.1），记录软件包版本与文件散列，不放入仓库；本机测试可用系统字体（如 macOS 宋体），只作本地证据，不分发。
- Windows 中文字体（宋体、黑体、仿宋、楷体、微软雅黑、等线）和方正等商业字体不随产品分发；测试与回退字体优先选 OFL：思源宋体／黑体、Noto CJK、霞鹜文楷、朱雀仿宋（技术预览），以及西文度量兼容的 Carlito、Caladea、Liberation。
- 图标和模板原创或使用许可明确的资源；JoyWord 品牌与其他产品标识分开。
- 参考软件生成的测试文件记录生成者、环境和样例内容来源；不将专有字体嵌入公开语料。

## 5. 本轮来源记录

2026-10-13 的产品和生态调研依据 [RESEARCH](RESEARCH.md)。文档中的工程方案是 JoyWord 的规格决策，不是对竞争产品实现的转述。实际依赖及构建结果登记在 [IMPLEMENTATION](IMPLEMENTATION.md)。

任何后续扩展的来源登记随代码一起评审；发布前验证 LICENSE、NOTICE、字体／资源授权和依赖清单齐全。


### 保守 DOCX 文字导出（C02）

- FORMAT-002／FORMAT-003／QUALITY-005；依据 [ECMA-376 第五版 Part 1](https://ecma-international.org/publications-and-standards/standards/ecma-376/) §17.3.1、§17.3.2、§17.3.3.23（tab）、§17.3.3.31（t）与 Part 2（OPC）。空白处理交叉核对微软公开 [TextType.Space API 规范说明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.texttype.space?view=openxml-3.0.1)，未读取 SDK 实现源码。
- 独立实现采用原始 XML 叶子范围补丁，写出 xml:space="preserve" 与 XML 预定义实体／字符引用；tab 使用独立元素。不能证明安全的节点修改拒绝。原 OPC writer 仅重建 ZIP 容器；其元数据变化在 ExportPlan 显式披露。
- 原创测试覆盖中英／emoji／组合字符、XML 转义、tab、CDATA、空段落、属性／未知部件／外链保留、保护与签名拒绝、版本竞争、撤销重做和资源预算；未把自导入一致视为 Word/WPS 往返或排版证明。详细环境与命令见 [C02 报告](../coordination/reports/C02-DOCX-EXPORT.md)。

### 已验证导出副本落盘（C03）

- FORMAT-004／QUALITY-004；依据 Rust 标准库 [hard_link](https://doc.rust-lang.org/std/fs/fn.hard_link.html) 的既有目标拒绝语义及 [File::sync_all](https://doc.rust-lang.org/std/fs/struct.File.html#method.sync_all)，使用固定 tempfile 3.27.0 的同目录私有临时文件 API。只复用通用文件库，不涉及 Office 实现。
- 独立策略为写入→sync→分块回读→不覆盖发布→清理→目录 sync；发布后的失败携带已发布收据，不删除用户可能已读取或继续修改的输出。没有借用库 API 名称宣称所有平台、文件系统与断电场景均通过。
- 原创故障注入、并发目标、符号链接和真实 DOCX 重开验证见 [C03 报告](../coordination/reports/C03-DOCX-COPY.md)。

### 自动 Unicode 默认换行（C04）

- LAYOUT-002／LAYOUT-003／LAYOUT-009；依据 [Unicode 15.0 UAX #14 rev. 49](https://www.unicode.org/reports/tr14/tr14-49.html) §1、§6、§8 及 unicode-linebreak 0.1.5 的公开 API。候选规则复用通用 Unicode 库，字体身份、簇安全、预算、行拟合与重整形仍使用项目既有独立实现。
- 默认 Unicode 禁则不是固定 Word profile；需要词典的分词、语言定制、连字符生成、混合字体与分页不在此切片内。不能正确显示 discretionary soft hyphen 时明确拒绝，不静默遗漏可见字形。原创真实字体测试及环境见 [C04 报告](../coordination/reports/C04-UNICODE-WRAP.md)。

### 文档分页快照与命中几何（C05）

- LAYOUT-003／LAYOUT-009；页框与段前分页语义参考 ECMA-376 Part 1 §17.6 及微软公开规范说明 [PageSize](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.pagesize?view=openxml-3.0.1)、[PageBreakBefore](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.pagebreakbefore?view=openxml-3.0.1)。只读取规范／API 文档，未读取 Open XML SDK 实现，不由 Office 引擎承担内核。
- 基于现有 model、engine、真实字体与 C04 断行独立实现固定行盒预览分页、源位置几何和内容键。累加段间距、固定基线和 cluster 边界命中为显式预览选择，不等同于 Word 的段间距折叠、页顶规则或字体布局 profile。
- 原创连续文本与事务测试，PDF 原样消费 PageScene；Poppler／pdfplumber 为独立验证工具，不进入 Rust 产品依赖。证据与未覆盖项见 [C05 报告](../coordination/reports/C05-LAYOUT-SNAPSHOT.md)。

### CPU 页面光栅与正式只读画布（C06）

- LAYOUT-001／LAYOUT-009；依据既有 EMU 场景契约、微软公开 [OpenType glyf 说明](https://learn.microsoft.com/en-us/typography/opentype/spec/glyf)，以及固定 Skrifa 0.42.1 的 outline::DrawSettings／OutlinePen、tiny-skia 0.11.4 的 Pixmap／PathBuilder 公共 API。精确版本的 crate 本地 rustdoc 和源码已核对；浏览器未能访问相应 docs.rs 版本页，未以 latest API 代替固定版本。
- 坐标变换、预算、请求版本、缓存复杂度计费与独立像素核对器为项目原创实现；轮廓解析和光栅算法复用通用字体／图形库。GPUI 的公开图像元素接收明确转换的 BGRA，不借用 Zed 编辑器实现，也不改字体元数据来迁就名字解析。
- 20 段原创种子在 CPU 和 PDF 共用 PageScene，Poppler 是独立参考光栅器；同场景比较不是 Word 黑盒参考或原生窗口验收，具体结果见 [C06 报告](../coordination/reports/C06-CPU-CANVAS.md)。

- C06 Windows 回归补充：依据微软公开 [EBDT](https://learn.microsoft.com/en-us/typography/opentype/spec/ebdt)／[EBLC](https://learn.microsoft.com/en-us/typography/opentype/spec/eblc)，传统单色／灰度 bitmap strike 可与可缩放轮廓共存。显式轮廓 profile 使用后者，彩色绘制和缺轮廓的纯 bitmap 字体仍不支持，见 C06 报告的失败与修正记录。

### 纯文本输入与组合态（C07）

- EDIT-002／EDIT-003／EDIT-004／EDIT-005／PLATFORM-004；采用 [Unicode 17.0 UAX #29 rev. 47](https://www.unicode.org/reports/tr29/tr29-47.html) §3 默认扩展字素、§4 默认词边界，精确复用 unicode-segmentation 1.13.3（本地 tables.rs 版本已核对）。字素适用于移动／删除，默认词边界不宣称中文词典分词；不做文本 Unicode 规范化。
- 平台范围契约依据固定 GPUI 0.2.2 的公开 EntityInputHandler／InputHandler／UTF16Selection 与 AppKit [NSTextInputClient](https://developer.apple.com/documentation/appkit/nstextinputclient)、[unmarkText](https://developer.apple.com/documentation/appkit/nstextinputclient/unmarktext()) 说明。读取通用工具包公开 API 及平台事件路由核对接口，不读取／迁移 Zed 编辑器实现；平台说明不等于系统拼音已实际验收。
- 组合态临时快照、原事务范围折叠、单事务确认、显示修订门、关闭提示版本检查与原创独立 String 随机模型为项目自行实现。输入实现无新外部包；后续 CI 检出新公告后单独升级间接 Rustls，见下述安全修正。详细自动测试及未覆盖项见 [C07 报告](../coordination/reports/C07-TEXT-INPUT.md)。

- C07 依赖审查补充（2026-10-15）：[Rustls 0.23.46 上游发布说明](https://github.com/rustls/rustls/releases/tag/v%2F0.23.46) 列出四项安全修正，对应 RustSec RUSTSEC-2026-0346、0347、0348、0349。现有 GPUI→gpui_http_client→zed-reqwest 的间接 0.23.45 被新数据库拒绝；仅锁定为修复版 0.23.46，不更改 feature、GPUI 来源或审查规则。未把所有功能条件均描述为产品已触发的漏洞。

### gpui-pre 来源逐文件校验（ADR-010 补偿措施）

- 依据 ADR-010 对第三方发布的供应链审查要求。gpui-pre 系列由个人发布，发布包没有 .cargo_vcs_info.json，只在 `[package.metadata.gpui-pre]` 中声明 zed-crate 与 zed-rev。
- 方法：scripts/verify_gpui_pre_provenance.py 先用 Cargo.lock 校验和验证 .crate，再要求声明的提交在 zed-industries/zed 的 main 历史中，在 Zed 工作区成员中定位 crate 目录，按 git blob ID 逐文件比较。只允许 cargo 打包生成的文件差异、符号链接解析和嵌套包排除。CI 的 provenance 作业用 `--expected-zed-rev` 固定提交，并带审查偏差文件运行。
- 结果（2026-10-16）：19 个包中 16 个与 Zed 93d7456 逐文件一致。gpui-pre、gpui-pre-apple、gpui-pre-macros 共有 9 处发布适配：`actions!` 宏改用 `$crate::Action`；构建脚本改读 vendor/gpui，并附带 5 个与 Zed 逐字节相同的副本；新增 gpui-kit 门面改写器及其 proc-macro-crate 依赖。这些改动经项目负责人接受，按内容钉死在 scripts/gpui_pre_reviewed_deviations.json。JoyWord 不依赖 gpui-kit，门面改写器实际为空操作。
- 边界：Cargo.toml 的内容（依赖、features）不在逐文件比对范围内，由 Cargo.lock 校验和与 cargo-deny 覆盖；本校验不评价 Zed 代码本身。升级快照时须先不带偏差文件运行，并重新审查。详见[来源校验报告](../coordination/reports/GPUI-PRE-PROVENANCE.md)。

### 应用日志与崩溃诊断（T211）

- QUALITY-007。不新增外部 crate，不复制 Word、WPS、Zed 或 WordCraft 的日志实现。日志文件、轮转、脱敏和 panic hook 为项目自行实现。
- 依据 Rust 标准库文档：[std::panic::set_hook](https://doc.rust-lang.org/std/panic/fn.set_hook.html)、[PanicHookInfo](https://doc.rust-lang.org/std/panic/struct.PanicHookInfo.html)、[Backtrace::force_capture](https://doc.rust-lang.org/std/backtrace/struct.Backtrace.html#method.force_capture)、[File::create_new](https://doc.rust-lang.org/std/fs/struct.File.html#method.create_new)、[File::sync_all](https://doc.rust-lang.org/std/fs/struct.File.html#method.sync_all)。只使用这些公开 API 的语义：新建文件不截断已有文件，hook 替换默认 hook，回溯在捕获时生成。目录项本身不做目录 sync，不把文件 sync 说成断电后目录项一定可见。

### 样式继承与主题解析（T200 样式基础）

- STYLE-002；依据 [ECMA-376 第五版 Part 1](https://ecma-international.org/publications-and-standards/standards/ecma-376/) §17.7.2（样式层级：文档默认、`basedOn`、直接格式）、§17.7.3（toggle：未设置继承、显式关闭、显式打开翻转）、§17.7.4.6（`w:link` 配对段落样式与字符样式，解析时不作为第二层属性），以及 §14.2.7、§20.1.4（DrawingML `a:fontScheme`／`a:clrScheme`）。只读公开规范条款，未读取 Word、WPS、LibreOffice 或 Open XML SDK 的样式实现。
- 独立实现把 latin／ea／cs 映射到 ascii+hAnsi、eastAsia、bidi，并把已知的 `w:themeColor` 长名解析为方案中的 sRGB。themeTint／themeShade、表格条件格式和编号样式不在本切片解析。没有新增外部 crate。

### 多级编号与制表位（T200 编号切片）

- STYLE-003；依据 [ECMA-376 第五版 Part 1](https://ecma-international.org/publications-and-standards/standards/ecma-376/) §17.9（`abstractNum`、`num`、`lvl`、`lvlOverride`、`startOverride`、`lvlRestart`、`lvlText`、`suff`）、§17.18.59（`ST_NumberFormat`）、§17.3.1.37–38（`tabs`／`tab`）与 §17.15.1.25（`defaultTabStop`）。`ST_NumberFormat` 的公开枚举名对照 [docx4j 转载的 ECMA-376 条款页](https://webapp.docx4java.org/OnlineDemo/ecma376/WordML/ST_NumberFormat.html)。只读公开规范，未读取 Word、WPS、LibreOffice 或 Open XML SDK 的编号实现。
- 中文计数与简体大写按条款示例的位值写法独立实现（十百千万；零；10–19 写作十／拾而不是一十）。`chineseLegalSimplified` 用零壹贰叁肆伍陆柒捌玖与拾佰仟万。规范没有规定上限；超过 99999 记为不能绘制，不猜测。甲乙丙丁戊己庚辛壬癸只用于 1..=10。字母只用于 1..=26，罗马数字只用于 1..=3999。MS-OI29500 记载 Word 对零和“万”使用不同码位；本切片不照搬该偏差。
- 不支持的 `numFmt` 保留原 token 并给出诊断，不用 `lvlText` 里的静态符号代替。`numbering.xml` 不从模型回写。没有新增外部 crate。

### 页面、分页符与主页眉页脚（T202 第一切片）

- LAYOUT-004／LAYOUT-012；依据 [ECMA-376 第五版 Part 1](https://ecma-international.org/publications-and-standards/standards/ecma-376/) §17.6.11（`pgMar`：页边距、页眉页脚距离、装订线）、§17.6.13（`pgSz`：`w`／`h` 为页面实际尺寸，`orient` 为方向标记）、§17.6.17（`sectPr`）、§17.3.3.1（`br` 的 `type="page"`）、§17.10.2（页眉页脚引用；省略 `headerReference` 时沿用前一节，即链接到前一节）、§17.16.1（`PAGE`、`NUMPAGES` 简单域）。只读公开规范，未读取 Word、WPS、LibreOffice 或 Open XML SDK 的分页实现。
- 排版不因 `orient` 交换宽高。页眉页脚画在边距内，不重排正文。首页不同与奇偶页不同只保存并给出诊断，不猜测另一套内容。widowControl 不实现，依据仓库内 layout_rules R5／R8 对 Word 2024 的负结果。没有新增外部 crate。

### 功能区图标、标尺与字数（T205 第一切片）

- PLATFORM-003／EDIT-009。功能区图标是 `crates/joyword-shell/src/chrome.rs` 中的 16×16 原创格子图，许可 MIT，逐项登记在 [feature_catalog.json](feature_catalog.json)（`origin` 为 `drawn-in-code`）。不打包 Office、WPS 或其他产品的图标、截图或图标字体。检查器拒绝 `apps/joyword` 与 `crates/joyword-shell` 下未登记的 png、svg、ico 等二进制资产。
- 标尺读取当前段经样式解析后的 `w:ind`（ECMA-376 Part 1 §17.3.1.12）和 `w:tabs`（§17.3.1.37–38）。默认制表位在设置省略时为 720 twips（§17.15.1.25）。字符单位与行单位不折算，标尺标为不能换算。本切片只显示，不拖拽。
- 字数按 EDIT-009 自行分类：汉字（含扩展与兼容表意文字）、假名和谚文各计一字，连续西文字母计一词，连续十进制数字计一词，标点和空格分开计数，分页符对象标记不计入。这不是 Word 字数对话框的对照结果。没有新增外部 crate。

### 字体与段落对话框（T205 第二切片）

- PLATFORM-003。中文字号对照 [OriginLab FAQ-850](https://www.originlab.com/doc/en/Origin-Help/FAQ-850) 所录的 Microsoft Office 号与磅值，以及 [W3C clreq issue 142](https://github.com/w3c/clreq/issues/142) 讨论中的 Word 列（二号 22 pt、三号 16 pt、四号 14 pt、六号 7.5 pt、七号 5.5 pt）。历史字号的其他列不采用。半点是 ECMA-376 `ST_HpsMeasure`。字号框范围按 Word 的 1–1638 pt，即半点 2 到 3276。
- 缩进、间距和制表位依据 [ECMA-376 第五版 Part 1](https://ecma-international.org/publications-and-standards/standards/ecma-376/) §17.3.1.12（`ind`）、§17.3.1.33（`spacing`）、§17.3.1.37–38（`tabs`／`tab`）。只读公开规范，未读取 Word、WPS、LibreOffice 或 Open XML SDK 的对话框实现。Auto 倍数在 n 不等于 240 时按本仓库 profile 行距做整数比例，不把它说成 Word 的行网格。没有新增外部 crate。

### 首页不同与奇偶页页眉页脚（T202 第二切片）

- LAYOUT-004；依据 [ECMA-376 第五版 Part 1](https://ecma-international.org/publications-and-standards/standards/ecma-376/) §17.10.1（`evenAndOddHeaders`：按节的页码起点区分奇偶，省略的类型继承前一节，不用另一种代替）、§17.10.5（`headerReference`：省略的首页／偶数页／奇数页引用分别继承前一节的同类型，否则空白）、§17.10.2（`footerReference`，规则与页眉相同）、§17.10.6（`titlePg`：节的首页用 First；关闭时按页码用奇或偶，并忽略已保存的 First）。条款文字对照 [ECMA-376 中文摘录](https://hellowac.github.io/ecma-376-zh-cn/ecma-part1/chapter17/10headeres/)。只读公开规范，未读取 Word、WPS、LibreOffice 或 Open XML SDK 的页眉实现。
- 本切片不读 `pgNumType/@w:start`，奇偶与 `PAGE` 共用从 1 起的文档页序号。显式空 story 是引用，截断继承。没有新增外部 crate。

### 页码域、分节与等宽分栏（T202 第三切片）

- LAYOUT-004／LAYOUT-012；依据 [ECMA-376 第五版 Part 1](https://ecma-international.org/publications-and-standards/standards/ecma-376/) §17.6.12（`pgNumType`：`fmt` 与 `start`；省略 `fmt` 为 decimal，省略 `start` 继续前一节）、§17.6.22（`w:type`：`nextPage` 为默认，另有 `continuous`、`evenPage`、`oddPage`；`nextColumn` 不在本切片）、§17.6.4（`cols`：等宽与 `space`）、§17.6.17（`sectPr`）、§17.3.3.1（`br`：`type="column"` 进入下一栏，没有多栏时进入下一页）、§17.10.1（`evenAndOddHeaders` 按节的显示页码区分奇偶）、§17.16.1（`PAGE`、`NUMPAGES`、`SECTIONPAGES`）。只读公开规范，未读取 Word、WPS、LibreOffice 或 Open XML SDK 的分页或域实现。
- `PAGE` 使用该节的 `fmt`。`NUMPAGES` 与 `SECTIONPAGES` 用十进制计数。不能格式化时使用文件里的缓存结果，不把缓存当成普通正文。没有新增外部 crate。

### 超链接、HTML／RTF 粘贴与图片同行文字（T203 第二切片）

- 依据 [ECMA-376 第五版 Part 1](https://ecma-international.org/publications-and-standards/standards/ecma-376/) §17.16.22（`w:hyperlink` 的 `r:id`、`w:anchor`、`w:tooltip`）。关系目标按 OPC 外部关系存储，不解析、不请求。打开允许列表（`http`、`https`、`mailto`）是 JoyWord 的安全选择，不是对 Word 的测量。只读公开规范，未读取 Word、WPS、LibreOffice 或 Open XML SDK 的超链接实现。
- HTML 子集对照 [HTML Living Standard](https://html.spec.whatwg.org/) 的元素与字符引用：保留 `b`／`strong`、`i`／`em`、`u`、`p`／`div`／`br`、`ul`／`ol`／`li`、`h1`–`h3` 的文本结构，丢弃 `script`、`style` 与嵌入内容，不取远程资源。RTF 对照 [Rich Text Format (RTF) Specification 1.9.1](https://www.loc.gov/preservation/digital/formats/fdd/fdd000473.shtml) 的控制字 `\b`、`\i`、`\ul`、`\par`、`\line`、`\tab`、`\u`、`\'`、`\field`、`\fldrslt`、`\object`、`\pict`。解析器是手写子集，不复制 Word、WPS 或 LibreOffice 的粘贴实现，不新增外部 crate。
- 列表只在文档已有编号实例时套用，不生成 `numbering.xml`。图片后的同行文字是排版规则，不是 Word 观察。插入图片按钮调用已有插入路径。没有新增外部 crate。

### 统一 PDF 与打印到文件（T204 第一切片）

- LAYOUT-009／FORMAT-008／PLATFORM-007。页面几何沿用已有 PageScene，不另写分页器。文档信息字典的 Title 与 Producer 对照 [ISO 32000-1](https://www.iso.org/standard/51502.html) §14.3.3；本切片不写 CreationDate 与 ModDate。ToUnicode 对照 §9.10，字体对照 §9.6，图像 XObject 对照 §8.9，书签大纲对照 §12.3.3，页面树与 MediaBox 对照 §7.7.3。结构读取器按 §7.5.4 经典交叉引用表与 §7.5.8 交叉引用流、§7.5.7 对象流、§7.4.4 单层 FlateDecode 自行实现，与 krilla 不共享对象。只读公开规范，未读取 Word、WPS、LibreOffice、Poppler、qpdf 或其他 PDF 库的实现源码。
- 标题取 OPC core properties 的 Dublin Core `dc:title`（[ECMA-376 第五版 Part 2](https://ecma-international.org/publications-and-standards/standards/ecma-376/) Core Properties；命名空间 `http://purl.org/dc/elements/1.1/`）。书签只在样式规范名为 heading 1–9 时生成（Part 1 §17.7.4），级别优先用已解析的 `outlineLvl`（§17.3.1.20，0 表示第一级）。没有放置行的标题不猜测页码。
- 落盘复用 shell 已有的写入、sync、回读与 `hard_link`，不覆盖已有路径。系统打印对话框只按操作系统 cfg 返回未验证，不调用 AppKit、Win32 或 CUPS。FlateDecode 复用已钉扎的 flate2 1.1.10（rust_backend），没有新的外部包版本。krilla 仍是 0.8.2。
