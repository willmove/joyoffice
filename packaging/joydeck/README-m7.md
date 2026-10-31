# JoyDeck M7 文件交付开发预览

macOS ARM64本机release，未签名/公证。运行 `./joydeck` 或Launch.command。顶部“导出与放映”打开右侧交付面板。全部导出默认新建，已有文件/图片文件夹拒绝覆盖；后台使用不可变快照，不将导出成功记为原生文稿已保存。

路径框可填写PPTX/原生输入、目标.pptx/.pdf，或尚不存在的图片文件夹。导入PPTX会在当前窗口建立新原生编辑会话；有未保存修改时拒绝替换，请用新文稿窗口导入。兼容性诊断显示在交付面板。原生保存继续使用顶部独立路径栏。

两种PPTX输出：
- 保留源包写回：保留全部原部件/关系/未知字节，当前仅允许能够唯一匹配的单run单段落单行文字修改。改页结构/位置/样式/图片等会拒绝；不把不支持的修改静默丢掉。多run/多段落/重复文本不支持保留写回编辑，原样保存可以。仅新建目标。
- 新建PPTX：导出常见形状、文字runs/颜色/字号/粗斜体、图片/crop/opacity/rotation、隐藏页与备注；确认损失后新建。公开内容是解析后的静态版面，继承/字段/节等会固化，未知原包部件不携带。附带受公共部件哈希约束的JoyDeck专有快照供未被外部改写的文件完整重开；哈希不符时走受限公共导入，绝不使用陈旧快照覆盖外部修改。本机PowerPoint只读打开样例已验证，不代表任意PPTX往返或PowerPoint编辑保存通过。

PNG/JPEG文件夹输出：页范围空白=所有可见页，或1-3,5；DPI36–600，PNG可透明，JPEG不透明。当前界面排除隐藏页，领域API可显式包含。单图边≤8192、≤16M像素、每批≤100页、输出总编码≤64MiB；超限报错，请降低DPI。带正确DPI标签，文件名按原页号slide-NNN。

PDF：可导出幻灯片或含备注页面；可见页面为栅格图，附不可见ActualText行映射供Unicode文字提取、页面书签。不是矢量文字、精准逐glyph选择或Tagged PDF；尚无链接/讲义/打印。含备注页面按字符簇折行并限40行，超限整次拒绝。仍受DPI/尺寸/文件预算限制。

静态放映：从首页/当前页打开不可变ShowSnapshot全屏窗口，不显示备注与编辑UI、跳过隐藏页。方向键/空格/Enter/PageUp/PageDown、Home/End、左/右键点击翻页，Esc关闭放映回到编辑器。源文稿后续编辑不会改变当前放映。尚非动画/双屏演讲者模式。

原生schema1.5/minReader6，新增pptx-source-v1，可读旧1.0–1.4；旧版本拒绝新文件。源PPTX及文字映射以独立source部件随原生/恢复快照保存。现有模型/历史的JSON估算仍包含内存源包，较大源包可能更早触发8MiB模型预算；完整unknown-field bag/新版本迁移尚未完成。保留旧文稿副本。

完整M7/MVP尚未完成：三平台、≥5授权外部样本、完整PowerPoint编辑往返、复杂原包对象映射/复制删除、所有native未知字段保留、完整C2语料、PDF精准选择/链接/讲义及全部AT-10/11/12静态/14/15，M0–M6开放条件继续保留。

验证：`scripts/smoke-native.py M0 M1 M2 M3 M4 --binary ./joydeck`；macOS带Pillow/numpy/python-pptx/pypdf/Poppler的Python运行 `scripts/verify-m5.py`、`scripts/verify-m6.py`、`scripts/verify-m7.py --binary ./joydeck --poppler <Poppler bin>`；系统拼音用 `scripts/smoke-macos-ime.py`。需要可交互桌面。QA会暂时使用并恢复剪贴板/输入源，异常中断不保证恢复。SHA256.json包含包文件校验值。
