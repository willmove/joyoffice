# 文本格式交换预览

2026-11-02。T600／T601 的生产 CLI 与桌面导出入口。CLI 支持明确指定格式的 DOCX／TXT／Markdown／HTML／RTF 输入和输出；桌面支持将当前正文导出为 Markdown／HTML／RTF／TXT。当前范围为普通正文段落及粗体、斜体、删除线（TXT 不保留格式）；完整 T600／T601 仍为 `in_progress`。

## 桌面导出

打开可编辑文档，点击“文件”打开左侧导航页面，在“导出”Tab 选择目标格式，核对转换差异，再选择新文件路径。格式列表可用“返回文档”或 Esc 退出，差异确认和路径选择也可取消；未确认前不创建输出。TXT 的确认提示单独说明所有格式都会丢失。

导出取当前已提交的编辑快照，包含尚未保存到 DOCX 的输入，不包含尚未提交的输入法组合。组合期间、只读预览和样式近似预览禁用入口；隐藏文字、审阅、批注和超出子集的复杂内容明确拒绝。选择路径期间文档变化会要求重新导出；后台发布期间的新修改继续保持未保存状态。导出不改变来源路径、不清除 dirty 状态、不替换源件或已有目标。成功后可继续编辑并正常保存原文档。

CLI 和桌面共用不可变转换计划与取消感知的发布器；桌面转换和文件发布在后台执行，窗口关闭时请求取消。发布已经完成后不会把结果误报为“取消且未生成”。

### 为什么有些文档只能导出 DOCX

格式列表表示已经接入的导出入口，并不表示任意 Word 文档都能完整转换。若导入内容需要“版式兼容显示”，当前统一门禁会暂停 PDF／PDF/A、RTF／HTML／Markdown／TXT 和 EPUB；DOCX 副本仍使用原始包保留及验证路径。仅能显示或保留某个对象，不等于已经实现该对象在目标格式中的转换。

导出页会显示兼容范围说明，并在每个置灰按钮下显示禁用原因。版式限制、兼容预览、只读、布局尚未稳定以及已有文本导出作业分别提示，不通过简单放开按钮将不完整场景变成正式导出。复杂表格、图片、页眉页脚等也不属于当前文本交换的普通正文子集。

开发者可用 `joyword-cli inspect FILE.docx --compatibility --max-text-bytes 0` 只读获取兼容标记，`compatibility.style_approximation_required=true` 对应上述版式兼容门禁。零正文预算使 `text` 为空，并按既有契约返回退出码 3；这表示请求的文本预算产生截断，不表示文件导入失败。未带 `--compatibility` 时原 JSON 格式保持不变。

## 手工验证桌面导出

下面的步骤适用于当前已构建的 JoyWord 桌面程序，先将 [原创测试 DOCX](../validation/20261102-exchange-ui/samples/source.docx) 复制到独立测试目录，避免修改验证记录中的样例。该样例包含两段文字、中文、emoji、斜体、粗体和字面标点。

1. 用 JoyWord 打开测试目录中的 DOCX 副本。在正文末尾输入一段独有标记，暂不保存，确认窗口显示未保存状态。
2. 依次选择“文件 → 导出 → Markdown”，阅读“检查转换差异”，选择“导出新副本”，将输出保存为测试目录中尚不存在的 `test.md`。重复操作生成 `test.html`、`test.rtf` 和 `test.txt`；文件名须包含与所选格式匹配的扩展名。
3. 分别用 Markdown 编辑器／预览、浏览器、Word／WPS 和文本编辑器打开四份输出，核对段落、中文、字面标点和刚输入的标记。Markdown、HTML 和 RTF 应保留样例中的粗体与斜体；TXT 的确认提示应说明它不保留格式。
4. 导出后回到 JoyWord，确认文档仍显示未保存；如需进一步核对原件未改变，可在打开副本前后分别计算 DOCX 的 SHA-256 并比较。
5. 在格式列表按 Esc 返回、差异确认点击取消、选择保存路径点击取消，确认不新增目标文件、正文和未保存状态保持不变。
6. 在测试目录预先创建一个含已知文字的 `keep.txt`，再尝试选择它作为 TXT 输出。若系统对话框询问是否替换，仅对该测试文件选择继续；JoyWord 应提示无法生成副本，原文件文字保持不变。

四种格式集中在“文件 → 导出”Tab 的格式列表中；点击格式直接进入转换确认，不再出现第二个格式选择弹窗；损失确认和取消不是另外的功能区按钮。界面底部状态栏显示导出或取消结果。当前桌面画布对部分字体的 emoji 可能显示缺字符号，可以同时检查导出文本和外部应用显示；不要将该现象记为已经通过字体显示验收。

## 先检查，再生成副本

```text
joyword-cli exchange input.docx --from docx --to markdown --plan
joyword-cli exchange input.docx --from docx --to markdown --output output.md --accept-losses
joyword-cli exchange input.md --from markdown --to rtf --output output.rtf --accept-losses
joyword-cli exchange input.md --from markdown --to docx --output output.docx --accept-losses
```

`--plan` 只输出 schema 1 JSON，列出目标字节数、段落数、损失和 `writes: 0`。指定输出但未确认损失时，输出同一预检报告、退出码为 2，不创建目标文件或临时文件。确认后在目标目录写临时文件、同步并独立读回，最后以不覆盖的硬链接发布；已有文件、原件路径和已有符号链接均不会被替换。生成后的报告写出失败会提示目标可能已经发布，不应盲目重试。

目标扩展名须匹配格式：`.docx`、`.txt`、`.md`／`.markdown`、`.html`／`.htm`、`.rtf`。文件系统必须支持同目录硬链接；失败时保留源件。当前没有认证目录同步或物理断电后的持久性。

## 当前支持边界

- 输入不超过 4 MiB；DOCX 解压后单部件 4 MiB、总计 16 MiB、1024 项和 100000 XML 节点。文本预检限制行数和语法标记数，模型限制 10000 段、20000 run；输出上限 64 MiB。
- 所有转换均明确报告版式、字体、样式、元数据及未建模语法的损失。转换不等于原格式保真保存，不覆盖来源。
- DOCX 使用生产导入器。受限或部分导入的 DOCX、表格、内容控件、不透明块、行内对象、注释、额外 story、未决修订、隐藏文字和无法解析的样式拒绝转换。
- TXT／HTML／Markdown 接受 UTF-8、UTF-8 BOM、UTF-16LE／BE BOM；非法编码和不支持的控制字符拒绝，不静默替换字符。HTML 当前没有 CSS 级联；带 style／class／hidden 属性、样式表和修订元素的输入拒绝。活动内容不执行、不访问外链。
- RTF 支持声明的文本控制词子集与 UTF-16 Unicode 转义。非 ASCII 原始代码页字节和高位十六进制转义尚无解码器，明确拒绝。隐藏内容和超出子集的控制词拒绝，不猜测未知目的地的含义。
- Markdown 是已声明子集，不宣称完整 CommonMark／GFM；正文标点先转义再生成格式标记。HTML 实体只解码一次，支持常用命名实体和有效非控制字符的数值引用。
- DOCX 输出是重新建立的 A4 Transitional 文档，compatibilityMode=15；只写出已披露的正文投影，经生产导入器复验。它不复用来源包，不能用于替代原件保存。
- 桌面尚未接入这些文本格式的打开／导入向导；可使用 CLI 显式转换为新的 DOCX 后打开。Strict profile、复杂审阅、脚注及表格的完整交换仍未实现。

自动及外部应用验证见 [本轮记录](../verification/m0-m7-completion-20261102.md)，桌面后续证据见 [导出验证](../verification/text-exchange-ui-20261102.md)。状态真值为 [TASKS](../TASKS.md)。

实现依据现有 RTF 公开规范记录及 [CommonMark 0.31.2 §2.4](https://spec.commonmark.org/0.31.2/#backslash-escapes)、[HTML 字符引用语法](https://html.spec.whatwg.org/multipage/syntax.html#character-references)。独立实现，无新增第三方库版本。
