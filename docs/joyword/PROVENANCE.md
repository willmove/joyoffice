# 独立实现、依赖与资产来源

版本 0.2 · 2026-10-13。对应 QUALITY-002。仓库代码许可证延续根目录 [MIT LICENSE](../../LICENSE)。本文件是工程来源规则，不把公开规范的可访问性解释为对所有知识产权的统一授权。

## 1. 独立实现规则

允许：公开格式规范、Unicode／字体标准、产品使用说明、公开 API 文档、经授权的黑盒操作和原创样例。规范中的元素名称与行为可用于实现；代码注释需标规范版本和条款。微软 Open Specification Promise 覆盖 ECMA-376、MS-DOCX、MS-OI29500、MS-DOC、MS-OFFCRYPTO、MS-CFB 等规范的必要专利主张，各规范另附实现用版权许可；这不涉及商标、字体或产品资产。

禁止：读取、复制或翻译微软 Office 与其他 Office 产品实现源码；将 Open XML SDK、LibreOffice、ONLYOFFICE、Pandoc、Typst 或 Word 自动化作为产品内核；把公开产品截图、商标、图标和模板直接打包为 JoyWord 资产。

通用基础库可以复用、阅读和必要时修补；这包括 Rust UI、字体、Unicode、容器、图片和密码算法库。阅读 GPUI 的构建脚本及输入 API 不属于阅读 Office 实现。平台公开 API 和通用绑定不属于排除的 Office 实现。

## 2. 来源证据

每项关键实现记录：需求 ID、规范 ID／版本／条款、独立推导说明、原创测试、黑盒观察环境。行为无法从规范确定时，记录被观察软件的版本、操作输入、结果和备选解释，不把猜测变成规范事实。

研究 Office 开源项目限于 README、用户手册、公开能力矩阵、许可与发布说明。不要把其内部架构或代码逐段迁移。若来源不符合边界，先隔离对应实现并重新从规范和原创样例推导，任务不得标为完成。

## 3. 依赖登记策略

每个实际使用的直接依赖记录精确版本、来源、许可证、feature、用途、是否含原生代码及维护风险。Cargo.lock 锁定完整传递依赖；升级需执行相关语料回归。发布前生成包含传递依赖的 SBOM 和 THIRD_PARTY_NOTICES，不将本文件的候选表当作完整许可清单。

| 依赖 | 定位 | 许可／来源依据 | 注意事项 |
| --- | --- | --- | --- |
| GPUI | UI 框架 | [上游许可](https://zed.dev/software-overview)，Apache-2.0 | 编辑器主体许可不同；仅依赖框架；记录传递依赖；来源按 ADR-010，第三方发布需审查发布者与构建来源 |
| sha2 0.10.9 | joyword-model 资源内容散列（直接依赖） | RustCrypto，MIT OR Apache-2.0（crate 元数据） | 与 GPUI 间接锁定的版本相同；只用于身份散列，不用于密码学保护 |
| Ropey 1.6.1 | 仅 T100 文本存储基准的 dev 依赖 | [上游](https://github.com/cessen/ropey)，MIT | 不进入产品构建；基准结论见 IMPLEMENTATION |
| HarfRust 0.8.4 | joyword-layout 字体整形（直接依赖） | [上游](https://github.com/harfbuzz/harfrust)，MIT | 选此版本以与 krilla 共用 read-fonts 0.39；特性逐项显式开关；不根据库名称声称兼容 Word |
| read-fonts 0.39.2 | joyword-layout 字体表读取（直接依赖） | MIT OR Apache-2.0（crate 元数据） | 与 HarfRust、krilla 的 skrifa 0.42.1 共用同一版本 |
| Fontique／ICU4X | 后续字体和 Unicode 基础 | 各选定发行包的 LICENSE 与元数据 | 使用时登记，而非默认视作已加入 |
| Skrifa 0.42.1 | CPU 字形轮廓直接依赖 | [上游](https://github.com/googlefonts/fontations)，MIT OR Apache-2.0 | 与现有 PDF 共用 read-fonts 0.39.2；只重用原锁文件版本 |
| tiny-skia 0.11.4 | CPU 单色轮廓光栅直接依赖 | [上游](https://github.com/RazrFalcon/tiny-skia)，BSD-3-Clause | 重用原锁文件，默认 SIMD／PNG 特性；保留许可通知 |
| image 0.25.10 | app 的内存页面到 GPUI 图像适配 | [上游](https://github.com/image-rs/image)，MIT OR Apache-2.0 | 复用 GPUI 原锁文件依赖；未启用额外默认特性 |
| quick-xml 0.41.0 | joyoffice-opc 包级 XML 解析（直接依赖） | MIT（crate 元数据） | 拒绝 DOCTYPE，只展开预定义实体与字符引用；与 GPUI 间接锁定的版本相同 |
| flate2 1.1.10（rust_backend）／crc32fast 1.5.2 | joyoffice-opc 的 deflate 编解码与 CRC-32（直接依赖） | MIT OR Apache-2.0（crate 元数据）；后端 miniz_oxide 为纯 Rust | ZIP 容器结构按公开的 PKWARE APPNOTE 自行实现，未使用 zip crate，以便逐项执行预算与一致性检查 |
| unicode-linebreak 0.1.5 | layout 的默认 Unicode 候选断点（直接依赖） | [上游 API](https://docs.rs/unicode-linebreak/0.1.5/unicode_linebreak/)，Apache-2.0（crate 元数据） | 精确复用锁文件版本，Unicode 15.0；不据此宣称 Word 中文规则或词典分词 |
| tempfile 3.27.0 | shell 安全创建同目录私有临时文件（直接依赖） | [上游 API](https://docs.rs/tempfile/3.27.0/tempfile/struct.NamedTempFile.html)，MIT OR Apache-2.0（crate 元数据） | 复用原锁文件版本；不使用可覆盖 persist，发布采用 std::fs::hard_link；无新外部包或升级 |
| krilla 0.8.2（default-features = false） | joyword-pdf 的 PDF 写出与定位字形（直接依赖） | [上游](https://github.com/LaurenzV/krilla)，MIT OR Apache-2.0；传递依赖含 pdf-writer、subsetter、xmp-writer（MIT OR Apache-2.0）与 tiny-skia-path（BSD-3-Clause） | 关闭 simple-text（其 rustybuzz 整形）与 raster-images；只消费页面场景中的字形与位置 |

MIT 主项目不能删去第三方 Apache 等许可证的通知。当前锁文件已含 MPL-2.0（cbindgen、option-ext，以及 Windows 字体用的 dwrote）、Unicode-3.0、BSD、ISC、Zlib、CC0-1.0 和 NCSA 等许可，均须进入通知清单。根目录 deny.toml 由 cargo-deny 在 CI 中检查许可白名单、RustSec 公告、通配版本和注册表来源；经 GPUI 0.2.2 间接引入的“停止维护”公告逐条登记理由，待 ADR-010 选定来源时复查，JoyWord 直接依赖不得列入忽略。是否允许特定依赖应按实际许可与分发方式检查，不笼统地把全部开源代码认定为可用或不可用。

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
