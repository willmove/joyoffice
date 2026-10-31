# JoyDeck M8 表格与工作区预览

macOS ARM64 release。双击 JoyDeck.app 或 Launch.command，也可运行 `./joydeck`。未正式签名/公证，不代表三平台 Beta。用 `./joydeck --table-demo` 查看原创季度回顾样例。

顶部功能区分为文件、开始、插入、设计、表格、视图；左侧为幻灯片缩略图，中央为可缩放画布，右侧为上下文格式面板。底部可展开备注。文件→打开/另存为/PowerPoint/PDF、插入→图片均使用系统选择器。Cmd/Ctrl+O 打开，Cmd/Ctrl+S 保存，Cmd/Ctrl+Shift+S 另存为；原生保存继续检查外部修改/覆盖冲突。导出到新目标，已有目标拒绝覆盖。

插入→设置行列数→插入表格。双击单元格输入；支持中文系统输入法、富文本粗斜体/字体/颜色、显式换行。Tab/Shift+Tab 导航，末格 Tab 新增行；未编辑时方向键移动，Shift+点击/拖动选择范围。表格功能区提供插删行列、合并拆分、均分、标题行与条纹行。右侧设置边框、内边距、字号、对齐和局部填充；恢复继承清除局部覆盖。拖动分隔线调尺寸，拖动外框控制点缩放，Alt 拖动整个表格；操作可撤销/重做。多格粘贴 TSV，非矩形/超限/与合并区冲突整次拒绝。

范围：每表1–128行、1–64列、最多4096格，全文稿16384格；导入行列≥8pt，分隔线拖动≥24pt；每格文字≤40000字节，每表≤100000字符，渲染另受总文字预算限制。合并文字按行优先串接并保留已解析的富文本。拆分保留主格内容，其余格为空。

原生 schema1.6 / minReader7 / table-v1，兼容读取1.0–1.5；旧程序拒绝新格式，请保留旧文稿。公开 PPTX 输出真实 a:tbl、单元格文字/runs、合并、统一单元格边框和直接颜色。PowerPoint16.114.1已打开、编辑并另存原创表格，JoyDeck公共导入重读修改和合并结构。专有快照仅在所有公共部件哈希匹配时使用。公开导入仍是子集：不完整支持主题表格样式ID、四边不同边框、嵌套对象、复杂文字排版；外部字体替换会改变度量。当前长文本使用显式换行，旋转表格需设为0再编辑文字。表格绘制可使用局部栅格fallback，不声称纯矢量GPU表格。

M7能力沿用：源PPTX包保留写回仅允许唯一单run/单段落/单行文字修改，表格修改请新建PPTX。PNG/JPEG、DPI/页范围/透明PNG、静态全屏放映可用；PDF可见内容为栅格，另有ActualText行映射和书签，包含表格文字；无矢量文字/精准glyph选择/Tagged PDF/系统打印/动画双屏。新建PPTX会固化继承/字段/节，不携带原包未知内容。

M8本机受支持范围的AT-16完成；M0–M7完整父阶段中三平台、硬件、外部授权语料及尚未实现功能保持开放，见源码 STATUS.md 与 docs/30-m8-implementation-report.md。CI配置不等于远端已运行。

源码验证：`cargo xtask check`、`cargo xtask stage M8`。包回归：`scripts/smoke-native.py M0 M1 M2 M3 M4 --binary ./joydeck`；macOS带Pillow/numpy/python-pptx/pypdf/Poppler的Python执行 verify-m5.py、verify-m6.py、verify-m7.py、verify-m8.py（均 `--binary ./joydeck`）。系统拼音：smoke-macos-ime.py `--table --binary ./joydeck`，普通文字去掉 `--table`。需要交互桌面；自动化使用并恢复剪贴板/输入源，不等于物理键盘或其他平台验证。SHA256.json列出包内文件校验值。
