# JoyWord 调研与选型

基线日期：2026-10-13（v0.2 补充 GPUI 来源、中文默认文档、WPS 与法规调研）。方法：查阅官方规范、产品帮助、项目 README 和通用基础库公开 API；不读取 Office 产品实现源码，不把宣传或 README 的性能声明当实测。工具链验证见 [IMPLEMENTATION](IMPLEMENTATION.md)。

## 1. 证据分级

- **规范事实**：公开标准或官方格式规范定义的行为；记录版本与章节后用于具体实现。
- **公开声明**：厂商帮助或项目文档所述能力；不代表本项目已验证其正确性。
- **工程判断**：JoyWord 根据目标作出的选型与优先级。
- **实测**：必须附命令／操作、环境、样本和结果；本轮没有 Office 兼容实测。

没有排名、市场份额或未限定样本的兼容率结论。若文档之间冲突，记录版本和适用范围，并通过固定版本产品验证，不拼接成不存在的统一行为。

## 2. 商业主流和开源产品

| 产品 | 已查阅的公开能力 | JoyWord 对照重点与限制 | 来源 |
| --- | --- | --- | --- |
| Microsoft Word 桌面版 | 高级排版、比较合并、引用、修订和批注；与 Web 功能存在差异 | 主要交互与行为基准，参考版本为 Office LTSC 2024；冻结版本、系统和字体，网页版不能替代桌面验证 | [功能比较](https://support.microsoft.com/en-au/word/word-features-comparison-word-for-the-web-vs-desktop)、[审阅](https://support.microsoft.com/en-us/word/give-and-receive-feedback-in-word) |
| WPS Writer | DOC／DOCX／RTF 处理、模板、PDF 转换及文档 AI | 中文文档互操作的并列基准；其 DOCX 含 wpsCustomData 命名空间、customXml 和数字样式 ID；原生 .wps 为附加 WpsCustomData 流的 DOC 式复合文件，无公开规范，只能按 MS-DOC 与固定版本 WPS 的黑盒样例实现；“兼容”声明需样本验证 | [Writer](https://www.wps.com/office/writer/) |
| Google Docs | 建议、批注及 Office 修订互转 | 协作和审阅操作；不作为 Word 分页的主要标准 | [建议模式](https://support.google.com/docs/answer/6033474) |
| Apple Pages | 修订跟踪、接受／拒绝和多种标记显示 | Mac 原生体验；不能据此推断全部 Word 审阅元素往返无损 | [Mac 修订](https://support.apple.com/guide/pages/track-changes-tan2685b84ff/mac) |
| TextMaker | DOCX 默认格式、修订、批注回复与解决、多格式导出 | 跨平台桌面日常流程；公开能力与质量分别验证 | [产品说明](https://www.softmaker.com/en/products/softmaker-office/textmaker) |
| LibreOffice Writer | 样式、索引、长文档、邮件合并、多栏、绘图与转换 | ODF 和复杂文档第二对照；不嵌入其引擎 | [功能](https://help.libreoffice.org/latest/en-US/text/swriter/main0503.html)、[许可](https://www.libreoffice.org/licenses/) |
| ONLYOFFICE | 审阅、比较、评论、版本和共同编辑 | OOXML 工作流及后续协作；开源与商业许可并存，产品能力不等于可直接复用 | [审阅](https://helpcenter.onlyoffice.com/docs/userguides/document_editor/ReviewTab.aspx)、[许可](https://www.onlyoffice.com/legalterms) |
| Collabora | 多人文档中的文字及格式修订 | 补充协作审阅场景 | [修订说明](https://www.collaboraonline.com/blog/how-to-track-and-manage-changes-in-collabora-online/) |
| Apache OpenOffice | 传统文字处理和 PDF 输出 | 历史交互及开放格式补充对照 | [Writer](https://www2.openoffice.org/product/writer.html) |
| Calligra Words | 带桌面出版能力的文字处理组件 | 对象布局与套件组件边界参考 | [组件说明](https://calligra.org/components/) |
| AbiWord | 官方入口本轮未能访问 | 仅列历史轻量路线补充项；不对当前维护和兼容状态下结论 | [官方入口](https://www.abisource.com/) |

工程结论：功能区和快捷键只是可见层；字体、分页、样式、修订、批注以及未知数据的往返决定实际替代能力。采用 Word 桌面主要行为基准，同时保留 WPS／LibreOffice 差异测试。

## 3. Rust 文档生态

| 项目 | 定位与公开声明 | 取舍 |
| --- | --- | --- |
| [WordCraft](https://github.com/storytold/wordcraft) | 2026-10-07 创建的 Rust Word 风格应用，egui＋vello_cpu 渲染、krilla 输出 PDF，README 自评 alpha、约 60% | 产品级对照；只读 README，不验证其覆盖比例或性能，不阅读或移植实现 |
| [docx-rs](https://github.com/bokuweb/docx-rs) | Rust DOCX 读写，公开列出多种元素，并说明 OOXML 支持不穷尽 | 作为功能边界研究；不作为 JoyWord 自研格式内核 |
| [Shiva](https://github.com/igumnoff/shiva) | 共用文档模型和多格式解析／生成；矩阵列出具体元素缺口 | 广泛格式覆盖不能推导逐项无损；不作为中间语义模型 |
| [Stemma](https://github.com/stemma-sh/stemma) | 面向 Agent 和自动化的 DOCX 原生修订工具 | 对照安全修订、歧义拒绝及 Word 交换工作流；不作为编辑内核 |
| [Typst](https://typst.app/docs) | Rust 排版系统，语言、编译器与编辑服务分层 | 对照输出质量及增量计算；不承担 Word 编辑和分页语义 |
| [GPUI](https://gpui.rs/examples/) | 原生 GPU UI；crates.io 最新官方版 0.2.2（2025-10）后停更，主干与社区 gpui-pre 增加 AccessKit、公开 GlyphId 和 Linux wgpu；无打印与富剪贴板 | 用户指定的 UI 框架；来源见 ADR-010；不自带完整 Word 文档布局 |
| [Parley](https://github.com/linebender/parley) | 富文本排版栈；0.12 依赖 harfrust ^0.12、skrifa ^0.44，并新增可自定义断行的 parley_engine | 通用文本基础设施研究；与 HarfRust／Skrifa 版本对齐后再引入；自研 Word 断行与分页规则 |

没有从这次调研得出“Rust 已存在可直接作为完整 Word 内核的库”的结论，也不声称不存在其他项目。格式读写、排版库、编辑器和完整文字处理器是不同类别。

## 4. 基础库选型与风险

| 能力 | 选型 | 决策边界 |
| --- | --- | --- |
| UI | GPUI，按 ADR-010 选定来源并精确固定 | 只复用 Apache-2.0 框架；不复用 GPL Zed 编辑器主体 |
| 文本存储 | T100 基准选定自研持久化 B 树分块文本；Ropey 1.6 仅作基准对照（dev 依赖） | 实测每个 rope 约 1 KiB，短段落为选定方案的 4.4 倍；Word 结构、修订和位置映射独立实现 |
| 字体发现／回退 | Fontique | 必须确认实际 face，不能只按字体名称比较 |
| 字体整形 | HarfRust | 本文档正文整形基线；通过版本与固定字体回归升级 |
| 字体解析／字形 | Skrifa | 与同一字体身份绑定；不把不同 face 的 glyph ID 混用 |
| Unicode | ICU4X | 分析和边界基础；不自动规范化用户正文 |
| XML／ZIP | quick-xml／zip，实际引入时固定版本 | 只负责通用容器和语法，不采用其上层 Office 映射 |
| PDF | krilla 的定位字形和绘图接口 | 自研分页；关闭其自动整形路径，避免第二套排版 |

[rustybuzz](https://github.com/harfbuzz/rustybuzz) 的公开仓库已归档并推荐迁移到 [HarfRust](https://github.com/harfbuzz/harfrust)。这影响新建的正文整形栈选型。GPUI 等上游可能仍间接依赖 rustybuzz；必须在锁文件和许可清单中如实记录，不用“正文采用 HarfRust”掩盖传递依赖。

[krilla](https://github.com/LaurenzV/krilla) 提供定位字形、字体子集、图像、标签等 PDF 能力，明确不负责段落、表格和分页。实际选用时禁用不需要的默认 feature。GPUI 的 [许可说明](https://zed.dev/software-overview) 区分可复用框架与编辑器主体，不能因为同仓库就假设许可证统一。

## 5. 规范来源登记

| 标识 | 来源 | 使用范围 |
| --- | --- | --- |
| N-OOXML | [ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/) | Part 1 标记、Part 2 OPC、Part 3 兼容扩展、Part 4 Transitional；各部分版本分别记录 |
| N-DOCX | [MS-DOCX](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/) | Word 扩展；访问时发布表列 2026-08-18、23.0，具体实现引用仍须固定章节 |
| N-OI | [MS-OI29500](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/) | Office 实现差异和兼容配置 |
| N-DOC | [MS-DOC](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-doc/) | 旧版二进制 Word 文档 |
| N-FORMATS | [Office 文件格式规范入口](https://learn.microsoft.com/en-us/openspecs/office_file_formats/) | 相关 CFB、共享对象和加密规范索引 |
| N-ODF | [ODF 1.4 标准目录](https://docs.oasis-open.org/office/OpenDocument/v1.4/os/) | ODF 1.3 基础交换后扩展 1.4；不混用候选稿与标准 |
| N-SEGMENT | [UAX #29](https://www.unicode.org/reports/tr29/) | 字素簇和文本分段 |
| N-BREAK | [UAX #14](https://www.unicode.org/reports/tr14/) | Unicode 换行基础 |
| N-BIDI | [UAX #9](https://www.unicode.org/reports/tr9/) | 双向算法及关联版本 |
| N-CJK | [W3C 中文排版需求](https://www.w3.org/TR/clreq/) | 中文排版指导性要求，区别于 Word 的具体兼容行为 |
| N-EAW | [UAX #11](https://www.unicode.org/reports/tr11/) | 东亚宽度，参与四槽字体与间距判定 |
| N-SCRIPT | [UAX #24](https://www.unicode.org/reports/tr24/) | 文字脚本属性，参与字体槽选择 |
| N-VERTICAL | [UAX #50](https://www.unicode.org/reports/tr50/) | 纵排方向 |
| N-GONGWEN | [GB/T 9704-2012](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=F3CC9BEF482524C895FDA7A08BB4A70E) | 党政机关公文格式，现行 |
| N-AIGC | [人工智能生成合成内容标识办法](https://www.cac.gov.cn/2025-03/14/c_1743654684782215.htm) | 与 GB 45438-2025 同日施行的生成内容标识要求 |

规范首页链接不是充分的实现证据。每项格式测试需要补充规范版本、条款／元素路径以及预期行为。不要引用微软 SDK 的实现来代替规范解释。

## 6. 尚须通过实验消除的风险

1. GPUI 来源（ADR-010）与外部整形结果的映射：0.2.2 的 GlyphId 不可构造，按字节加载字体不返回 FontId；M0 未通过前不认定三端输出一致。
2. 中文 IME 的组合态、范围转换、候选框与撤销；不能只测试英文键盘输入。
3. 字体缺失、字体版本与 Word 兼容配置导致的分页差异。
4. 修订与批注扩展跨软件接受／拒绝后的语义；单纯 XML 合法不充分。
5. 未知 XML、资源关系和用户编辑相互影响时的安全保留边界。
6. 长文档增量分页、脚注／浮动对象迭代和性能预算。
7. 三端可访问性、Linux 两套窗口协议和原生打印。
8. 无 GPU、虚拟机和国产 GPU 环境的渲染退路（0.2.2 在 Linux 只支持 Vulkan；国产 GPU 多只提供 OpenGL）。
9. 中文默认文档的网格、字符单位与禁则行为：规范未定义字符单位，需以固定 Word 黑盒确认。
10. 信创：LoongArch 新旧世界 ABI 并存，Rust 的 loongarch64 目标只支持新世界（glibc 2.36），统信 UOS 与银河麒麟当前版本的 ABI 和 GPU 驱动需在 T005 实测。

以上风险均对应 TASKS 的验证任务，不在调研阶段虚构测试结果。

