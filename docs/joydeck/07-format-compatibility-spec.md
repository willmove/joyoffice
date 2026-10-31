# 文件格式与兼容性规格

本章定义完整目标架构；M1 当前读取/预览子集及更小预算以 [18](18-m1-viewer-spec.md) 为准，实际支持和验证见 [19](19-m1-implementation-report.md)。M7已实现受限源包保留/新建导出，M8–M10扩展表格、图表、图示和动画；实际支持范围以末尾扩展及实施记录为准。数据保全优先于扩大格式数量。

## 1 文件类型与版本范围

| 类型 | 输入 | 输出 | 阶段与边界 |
|---|---|---|---|
| `.joydeck` | v1 最小只读子集 | M2 默认保存 | M1 读取；详见 18；M2 补充写入与扩展 |
| `.pptx` | 常用对象及未知内容保留 | 常用可编辑内容与受支持保留 | M1 只读，M7 正式写回 |
| `.potx` / `.ppsx` | 按真实 content type 识别 | 保存模板/放映变体须写正确 content type | M7 基础评估，M12 扩展验收后开放 |
| `.pptm/.potm/.ppsm` | 后续可保留宏载荷 | 不执行宏；能否原样输出单独验证 | M14，首版给明确不支持提示 |
| 二进制 PPT、ODP、加密文件 | 不列首版支持 | 不列首版支持 | M14 独立项目 |
| PNG/JPEG、PDF、outline/RTF、GIF | 图片/大纲依各功能导入 | 按功能阶段开放 | M5 图片，M7 PDF/图片，M12 其他 |
| MP4 视频 | M11 经验证的媒体导入 | 需编码器和统一时间轴 | M14 |

文件后缀不能代替内容嗅探。ZIP 内 PPTX、JoyDeck、其他格式和 CFB 加密容器分别识别；后缀与内容不符时提示，并按正确格式打开/拒绝。

## 2 新原生容器

目标 `.joydeck` 为 ZIP 容器，保存可编辑模型、原始资源及兼容 sidecar。首个格式 schemaMajor=1，结构由本项目规格独立定义。

| 成员 | 内容与约束 |
|---|---|
| mimetype | JoyDeck 独立格式身份，初始暂定 `application/vnd.joydeck.presentation+zip`；仅内部标识，不声称已注册 IANA |
| manifest.json | format、schemaMajor、schemaMinor、minReader、requiredFeatures、generator、文档入口、资源索引、校验信息 |
| document.json | 领域数据、稳定 ID、显式覆盖/继承、文本、主题、动画、评论、阅读顺序；不含 UI/窗口/缓存 |
| resources/ | 以内容标识管理的原图片、音视频、可合法保留字体；索引给出 MIME、长度和 digest |
| preserved/ | 来自 PPTX 的未知 parts/关系/owner 映射及图表/SmartArt/OLE sidecar，带原 content type 与来源 ID |
| preview/ | 可选缩略图，派生缓存，缺少或损坏不影响打开文档；后续实现时生成 |

M1 独立定义最小只读 schema；M2 增加写入、资源索引及明确的 schema 升级和未知字段策略；不把任意 Rust struct 的 serde 输出永久当作无版本 API。外部媒体 bytes 不以内嵌 base64 重复塞进 JSON。

### 版本与未知数据

- 同 major 的新增可选字段有默认值，未知字段需要保存在扩展 bag 并能重写；普通 serde 忽略未知字段会丢数据，不能称“无损向前兼容”。
- unknown requiredFeature 或 minReader 超限时只读或拒绝编辑；major 不兼容时不尝试猜测性覆盖保存。
- 迁移在内存完成，原文件保持不动；首次保存可以建议另存并记录原版本。
- UI 设置另存平台配置；恢复数据另存应用目录，不写进最终 `.joydeck`。
- stable IDs 唯一、所有引用有效、资源存在、数值有限、几何/文本范围合法，是载入和提交后的校验不变量。

## 3 PPTX 的公开结构与实现依据

OOXML 不等于“一个 XML 文件”。公开结构包含 presentation、slides、masters、layouts、themes、notes、media 等 parts，通过 relationships 组成包图；DrawingML 进一步定义形状、文字、表格与图表数据。[PresentationML 结构](https://learn.microsoft.com/en-us/office/open-xml/presentation/structure-of-a-presentationml-document)

以 ECMA-376 的 Markup、OPC、Markup Compatibility 和 Transitional 部分为格式基础，遇到 Microsoft 扩展再查对应公开规范。解析器与写入器依据公开规范独立实现，不依赖本机 Office。[ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/)、[Office 实现补充](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376)

Strict/Transitional namespace 支持须单独测试。M7 不默认承诺所有 Strict 文稿可编辑；识别到未支持 namespace 时给报告或只读，不能写成错误混合 namespace。

## 4 解析与中间表示

流程：嗅探 → 限额 ZIP 读取 → content types/relationships 解析 → 包图建立 → presentation/theme/master/layout → slides/notes/resources → 继承解析与模型校验 → CompatibilityReport → 发布 DocumentSnapshot。

解析告警是结构化数据，含 code、severity、part、slide/object、行为和保留状态。不能仅记日志后静默丢掉 part。允许受控容错，但必须区分可恢复修正、降级显示和不可继续的结构损坏。

源格式信息保留为 SourcePackageSidecar：规范化名称与原名、content type、owner、关系类型/ID、目标、是否 external、raw bytes digest、是否已由模型接管。复制、删除对象或另存时更新引用闭包。

### 模型化与保留的边界

| 内容 | 处理要求 |
|---|---|
| 已完整建模 | 写回根据模型生成；删除对象不再保留其过期 duplicate XML |
| 部分建模 | 已识别字段可修改，未识别属性保留；记录 ownership，防止新旧字段重复冲突 |
| Opaque 对象 | 保留 XML、预览、完整关系和所需 parts；UI 显示不可编辑语义，允许移动等操作须验证 transform 合成 |
| SmartArt/OLE/嵌入工作簿 | 保留关联部件闭包；不执行嵌入程序；未知依赖不随意去重重命名 |
| 宏/主动内容 | 首版不执行；未来保留能力独立开放 |
| 外部关系 | 默认不 fetch、不自动打开；保留目标文本并标注外部依赖 |
| 数字签名/IRM | 修改会影响有效性；不伪造有效状态，未支持时只读/另存并说明 |

未知数据保留不是 ZIP 全文件无差别复制：损坏/危险路径、无 owner 的垃圾、过期签名和冲突关系须有策略。反过来，也不能只留下 XML 而删除其 rId 对应二进制 part。

## 5 兼容级别与能力矩阵

每种对象分别记录 Import、Render、Edit、RoundTrip 四列，状态为 full/partial/preserve-only/unsupported，附样本。再给当前文档整体级别：

| 等级 | 承诺 | 保存策略 |
|---|---|---|
| C0 可安全查看 | 能显示部分内容或预览，其他有占位/报告 | 默认只读，不以“已完整打开”呈现 |
| C1 可编辑子集 | 常见图文可编辑，未支持内容有保留/降级说明 | 默认另存，展示当前损失/风险清单 |
| C2 已验证往返 | 对指定受支持特性及语料，语义、关系和视觉达标 | 可按用户选择覆盖，仍提示当前文档实际损失 |
| C3 专项高保真 | 某些主题、图表、文字/动画细节经专项对照 | 逐特性标注，不能宣称任意 PPTX 完全无损 |

模型读回相同但视觉不同，或截图相同但对象不可编辑，都不是完整 C2。C2 针对明确特性/语料版本，不是全格式全球承诺。

## 6 保存与关闭事务

保存输入是 DocumentId、revision R、不可变快照、目标格式/路径和损失策略。后台序列化与校验，写同目录唯一临时文件，flush/sync，按平台可靠替换，再报告成功。必要时同步父目录；网络盘/文件系统差异需单独测试，不承诺所有介质断电安全。

完成时将 R 标为已保存；若已有 R+1，不清 dirty。同一目标串行保存或明确覆盖排序，不能让慢的旧 revision 最后完成覆盖新文件。用户取消只有在替换提交前保证“不写目标”；提交后展示已保存结果，不能伪装取消成功。

覆盖前检查外部修改 fingerprint（路径身份、大小、mtime，必要时 hash）；检测到冲突重新确认用户意图。首次另存及有损另存不会隐式改变用户原始 PPTX，除非明确选择成为活动保存路径。

关闭/退出统一调用生命周期服务。恢复文件存在不构成允许丢弃未保存修改的理由；只有用户选择放弃或相应自动化策略授权后，才能关闭。

## 7 自动恢复

建议默认编辑后 30 秒 debounce、最迟 60 秒触发一次，均可配置；后台保存不可变稳定快照，不含尚未提交的手势或 IME 预编辑。大型文档要避免每次全量复制媒体，采用内容去重与快照引用。

恢复记录包含文档身份、源路径、生成时间、schema、revision、源 fingerprint 和资源完整性信息。分开处理：异常退出的有效恢复、新版本不支持、损坏、另一个实例仍占用和原文件外部变化。

恢复配额建议总计 1 GiB、每文档最近 3 份，M2 实测调整。用户从恢复列表恢复时创建 dirty 文档并建议另存；只在明确保存/放弃后清理对应记录。过期清理不能删除正在另一个实例写入的文件。

## 8 输入与资源预算

以下是首版默认工程预算，实施时根据语料在 ADR 中调整。解析预算与常规性能预算分开。

| 资源 | 默认上限/策略 |
|---|---|
| 单文件压缩体积 | 普通模式 256 MiB；更大文件需显式大型文档模式或拒绝 |
| 解压合计 | 512 MiB，含失败/丢弃项实际已解压字节；不只统计成功保留部分 |
| 单 binary part / XML part | 64 MiB / 16 MiB；流式路径可经专项验证提升 |
| ZIP 项数 | 20,000；重复名称、规范化后冲突、绝对路径和逃逸路径拒绝 |
| XML | 深度 128，元素与文本累计配额；禁用 DTD/外部实体；组/关系递归也有界 |
| 图片 | 单图最多 64 MP，decoded RGBA 需先计入全局预算；优先缩略解码 |
| 模型数量 | 默认 5,000 页、单页 10,000 对象，总对象/文字字节另有累计预算；超限只读/拒绝 |
| GPU | 根据设备能力与 09 的 cache 预算裁决；不依据文件声明分配巨大纹理 |
| 控制请求 | 默认 8 MiB；大二进制使用受控文件/资源句柄，避免大 base64 JSON |

预算检查要在分配前和流式过程中同时进行。文件大小不大但宣称巨图、深度组、超长路径、非有限坐标和极大滤镜半径也要处理。严格默认值可能拒绝合法大文稿，应明确说明超限项及安全替代，不静默裁掉内容后继续覆盖保存。

## 9 输出规格

### 图片

支持当前页/页范围/全部页，明确是否包含隐藏页、背景、透明度和输出尺寸。PNG 保留 alpha，JPEG 要求指定背景。像素尺寸/scale 与 DPI 标签一起确定；备注、选择框和编辑提示不进入输出。

### PDF 与打印

M7 可实现栅格页加可选择文本、链接和书签，输出 UI 明确质量选项。M12 基于统一 Scene 输出矢量路径和字体文本，对透明滤镜/不支持效果做局部 raster，并复核字体嵌入与 text extraction。

文本层不能与可见文字重复显示或阅读顺序错乱。可选择文字不等于 Tagged PDF/完整文档无障碍；Tagged PDF 是单独后续能力，不能仅因有文字层宣称已实现。

讲义和备注布局不修改幻灯片大小；页范围、纸张、方向、页边距、隐藏页和每页数量在预览与最终 PDF/打印一致。打印通过 OS adapter 消费完成的输出，不等待 UI 绘制截图。

### GIF 与视频

M12 先保留静态逐页 GIF，明确固定页时长、循环、调色板和尺寸；带转场/构建的 GIF 使用确定性时间轴另验收。MP4 要求编码器、音视频同步和容器封装，是 M14 项；解码支持不能替代编码与封装工作。

## 10 兼容性语料与验收

未来建立经许可的 corpus，每个样本附来源、授权、特性标签、字体环境、预期语义、PowerPoint 版本/平台和参考渲染条件。M1 使用本项目自建语料。可先用本项目自行生成的样本，获得授权的真实文稿再私有化扩充。

M7 最低集合：基础图文 20 份、母版/主题 10 份、复杂保留内容 10 份，至少 5 份经授权的非本项目生成文稿。若真实语料不可取得，只能报告合成语料结果，不据此标成实用 C2。M13 目标不少于 100 份，按特性覆盖而非只凑数量。

执行链：原包检查 → JoyDeck 打开 → 无编辑另存 → 重开 → 修改指定属性 → 再保存重开 → PowerPoint 打开检查/参考渲染。结构验证查关系闭包、对象/文本/媒体计数、原始保留 payload hash；语义验证查样式、动画和字段；视觉验证查缺失、换行、位置与效果。

临时 part 名、XML 顺序和 ZIP 压缩差异不作为失败；未知部件丢失、文本变化、媒体引用悬空、PowerPoint 提示修复属于失败。所有声明 C2 的样本不允许静默内容损失。

## M3 开发预览原生扩展（2026-10-30）

当前 writer 使用 schemaMajor=1、schemaMinor=2、minReader=3，必需特性 object-transform-v1 与 rich-text-v1。reader 继续接收 1.0/1.1，旧 reader 会因 minReader/requiredFeatures 拒绝新文件。文本对象可附 runs 数组：start/end 是 UTF-8 字节偏移；非空 runs 从 0 起连续覆盖全文，不允许重叠、缺口、半个 codepoint、非法字号/颜色/字体名。style 包含 family、size、bold、italic、color。空 runs 按旧对象 fontSize/fill 继承。尚无完整 paragraph/list 数据契约；本扩展不代表全部 M3 格式工作完成。详情见 [23](23-m3-implementation-report.md)。

## M5 图片开发预览原生扩展（2026-10-30）

当前writer为schemaMajor1/schemaMinor3/minReader4，必需特性增加image-resource-v1。空图片文档也写新身份；继续兼容读取1.0–1.2，旧reader会拒绝新文件。`manifest.resources`列出独立PNG部件`resources/<sha256>.png`，document.json不内嵌资源字节；图片对象kind=image，image.resource为SHA-256，image.crop为归一化[x,y,w,h]，有限/正面积/区域不得越界。规范化PNG应用EXIF方向；不保留原JPEG字节或元数据。读取检验资源hash、格式、尺寸和预算，缺失/重复/非法crop拒绝；编辑器拒绝未知额外部件。导入/替换/裁剪/删除/undo/redo与恢复快照保有资源引用，删除只回收当前快照未引用资源；历史与独立传输快照自持资源。预算和未实现格式见 [27](27-m5-implementation-report.md)。

## M6 文稿设计预览原生扩展（2026-10-30）

writer升级schema1.4/minReader5，新增必需design-v1；旧1.0–1.3继续读取，旧reader明确拒绝新文件。Deck.design包含themes/masters/layouts的独立ID表，版式引用母版、母版引用主题；placeholder以kind/index匹配，Layer保存可选frame/fill/size/family，颜色/字体以`@slot`保留主题引用。Slide.layout与inherit_background声明继承，文字Object.binding保存kind/index及direct覆盖，解析优先级为对象基值→母版→版式→direct。普通对象保持直接格式；富文本runs作为局部直接格式保留。删除造成悬空关系或缺失主题槽位时事务拒绝。

Slide增加notes及section引用，Deck.organization保存author/subject与节ID名称表；fieldDate是固定日期上下文，文字占位符field=number/date按目标页序/文档日期解析。未知字段仍由编辑器/嵌套类型拒绝，以免另存丢失。当前没有unknown-bag/完整PPTX母版主题保留；这些属于后续工作。原生图片部件继续使用M5的SHA-256规范。

跨文档PageTransfer v1自持文稿子集/设计/资源并重映射IDs。目标主题缺槽整次拒绝；不同页尺寸做显式几何/字号缩放，不宣称任意非均匀变换完全保真。普通Object Transfer v1把源占位符外观固化为普通对象，解除继承和动态字段引用。详情、预算、证据和回退见 [28](28-m6-implementation-report.md)。

## M7 文件交付预览扩展（2026-10-30）

当前writer为schema1.5/minReader6、必需pptx-source-v1，读取1.0–1.4。Deck.source在内存自持原包与基线/唯一文字映射；持久化时从document.json剥离，manifest.source=true，source/metadata.json存不含parts的元数据，source/package/*存原始部件。native读取核验关系闭包/预算/映射及无递归baseline，恢复同样保留源包。未知新requiredFeatures继续拒绝；完整unknown-field bag尚未实现，不能宣称任意未知原生字段都能往返。

源包写回只允许可唯一匹配单run单段落单行文字patch，其余修改拒绝；未知XML/二进制/关系raw bytes保留。新建PPTX为有损静态子集，需确认且不覆盖目标；customXml/joydeck.xml的专有快照绑定所有公共部件hash，哈希不符即回退公共受限导入，避免陈旧模型覆盖外部修改。公开模型和有效快照的能力必须分列。PNG/JPEG/PDF、预算、源包历史估算及实际PowerPoint验证范围见 [29](29-m7-implementation-report.md)。


## M8 表格扩展（2026-10-30）

当前writer升级schema1.6/minReader7/table-v1，向后读1.0–1.5。table为独立对象，保存行/列尺寸、主格span、空覆盖格、文字runs、全表主题样式与直接覆盖；所有span拓扑与预算先校验。新Object.textAlign缺省left。公开a:tbl merged cell/统一边框读写与PowerPoint实际编辑重读通过，未依赖有效专有快照。PDF文字层含单元格。表格/文稿预算、公开子集限制、兼容矩阵和新包见 [30](30-m8-implementation-report.md)；M7的原包表格修改写回仍拒绝。


## M9/M10 图表、图示与动画扩展（2026-10-30）

当前writer为schema1.8/minReader9，新增charts-v1和motion-v1；reader向后接收1.0–1.7。Object可带chart/diagram/morph_key，Deck.presentation保存Timing/Effect/顺序/循环/内部页链接，播放进度不入模型。旧reader拒绝新身份，旧包/源码保留回退。

七类基础图表公开输出DrawingML chart和嵌入XLSX，缓存与工作簿同源生成；导入有界解包并核对支持的数值引用。未知复杂图表给出诊断并保留源包；不是全量Excel计算引擎。图示读取受支持dataModel文本/拓扑时明确简化，新输出普通形状组。基础动画/转场以PresentationML timing及Morph扩展输出，受支持参数公共重读；未知树仍在源包。内部链接/自定义放映顺序/循环未输出公共PPTX，导出说明损失。

源包写回仍仅唯一单run/单段/单行文字patch；图表/图示/动画修改走有损新建PPTX。专有快照只在全部公开parts哈希一致时恢复，PowerPoint修改不会被旧私有模型盖回。预算、真实Office正负例、公开能力及未知部件边界见 [31](31-m9-implementation-report.md)、[32](32-m10-implementation-report.md)。


## M11/M12 native增量（2026-10-30）

当前writer schema1.10/minReader11，新增review-v1/media-v1，保留原有requiredFeatures，旧程序拒绝新文件。review储存原生评论/alt/装饰/独立阅读顺序；playback元数据在document.json，bytes在media/<SHA-256>.bin，manifest索引受校验；每资源≤8MiB、文稿媒体≤32MiB。公共PPTX尚不生成这些评论或媒体；源包未知闭包保留不能替代公开可编辑/播放往返。

M12提供实际矢量路径/文字/subset字体PDF（复杂filter允许局部raster），内部页链接/书签；无Tagged/独立PDF阅读顺序。大纲/RTF为文字子集，GIF为静态页序列；POTX/PPSX使用真实主content type且沿用PPTX有损范围。验证及开放条件见34。
