# JoyDeck 0.4.0 · R3 Windows 开发包

解压后运行 `joydeck.exe`，或 `joydeck.exe "samples\independent.pptx"`。可输入本地 `.pptx`、`.joydeck` v2、旧 `.deckcraft` 路径。Rust 1.98.1、GPUI Kit 0.8.1 / GPUI pre 0.3.10 与 JoyOffice 保持一致，未使用 egui。Windows MSVC 桌面二进制主线程栈预留 8 MiB，以容纳调试模式 GPUI 视图树；不改变系统设置。

## 文字和系统输入法

双击文字对象或单元格，或选中后点击“原位文字”。画布原位显示光标、选择区和组合下划线；文字、命中和候选锚点共用 CPU 参考布局。支持普通水平且未旋转的顶层文字与表格单元格；不支持的旋转、竖排、WordArt、动态字段/公式会明确提示，原内容保留。原生 GPUI 对象绘制和字形提交属于 R4。

支持鼠标设置/拖动选择、Shift 扩展选择、方向键、Home/End、Backspace/Delete、Enter 段落、Shift+Enter 软换行。Ctrl+A 全选；选区 Ctrl+B/Ctrl+I 修改粗体/斜体。格式 UI 当前只提供选区粗体/斜体；未宣称完整字符/段落、列表或超链接编辑。文本框长内容仍受页面裁剪，完整文字滚动和旋转适配待后续。

输入法预编辑保存在独立草稿，候选提交才进入命令历史和恢复快照。Esc 先取消组合，再按一次退出文字模式。切页/退出/窗口失活会结束原位草稿；已提交文字保留，未提交组合丢弃。组合过程中保存或文稿编辑命令会提示先完成/取消。Ctrl+Z、Ctrl+Y/Ctrl+Shift+Z 操作文稿历史，输入框内部仍使用自己的草稿历史。候选框由操作系统显示，位置源于实际文字布局。

## 表格、备注和剪贴板

双击单元格输入；Tab/Shift+Tab 在未覆盖单元格间导航，合并覆盖单元格跳过。单击表格单元格后，工具栏提供行列增删、与右格合并、拆分和底色；可插入 3×3 表格。CLI 支持任意矩形合并；本轮 UI 不提供矩形多格选择。表格编辑上限 100×100，保留至少一行一列，越界和覆盖格粘贴拒绝且不产生部分修改。

原位选区 Ctrl+C/X/V 使用系统剪贴板；内部文字片段带版本及 TextBody 元数据以保留选区格式，外部文字使用 Unicode 文本。未进入原位输入时，“复制文字/表格”复制选中对象的文字或整个表格 TSV；“粘贴”/Ctrl+V 从当前单元格开始填入 TSV 区域，否则插入文字对象。区域须放得进现有表格，合并覆盖格不接受区域粘贴。整次区域粘贴是一个撤销步骤。图片、跨文稿对象/媒体依赖和 HTML/RTF 富剪贴板尚未接入。

“备注”使用与 JoyOffice 相同的 Textarea 输入控件，支持多行和系统输入法；“应用备注”才提交事务，未应用草稿可关闭对话框。已有 PPTX 演讲备注占位符可保留式写回；缺少可复用备注部件、带超链接的文字写回等受限情况请另存 `.joydeck`。不会默默将未应用备注写入文件。

## 保存与恢复

路径框选择新 `.pptx` / `.joydeck` 文件名后 Ctrl+S 另存；默认原位保存使用读取时的大小/SHA-256 指纹检测外部变化。另存拒绝覆盖已有输出；同目录 prepare/sync 后提交。保存前以内存重开核对页面、对象、文字/几何、表格单元格/合并/底色和备注，无法稳定读回时拒绝发布，可另存 v2 项目。

未触及 ZIP 成员保持未压缩字节相同。表格普通文字/底色修改保留周围原 XML；行列变更仅支持简单表格 XML 子集，扩展/未知属性等拒绝 PPTX 写回并可保存项目。签名、复杂对象和节/自定义放映等仍沿用 R2 限制。不保证 PowerPoint 全保真，修改内容的 XML 可重新序列化。

约 250 ms 后安排后台恢复快照，包含已提交模型及原始包；未提交 IME 和备注草稿不在恢复中。恢复目录为 `%LOCALAPPDATA%\joyoffice\joydeck\recovery-v2`，应按源文稿访问范围保管。恢复需另选新路径，撤销历史不跨重启保存。读取/输出、解压、页/对象/像素及 128 步历史预算延续 R2。

```powershell
.\joydeck-cli.exe commands
.\joydeck-cli.exe inspect ".\samples\independent.pptx"
.\joydeck-cli.exe edit ".\samples\independent.pptx" --script ".\samples\r3-edit.json" --output ".\r3-edited.pptx"
.\joydeck-cli.exe render ".\r3-edited.pptx" --output ".\rendered" --all
```

CLI 文稿页码从 1 开始；表格行列从 0 开始；edit-text 的 start/end 为 UTF-16 单位，不能拆分代理对。新 set-text/edit-text/insert-table/table 与原 UI 同一命令入口，inspect 包含 TextBody 和表格结构。批量脚本最多 1 MiB/1000 步，单文字主体和文字剪贴板最多 64 KiB，富片段元数据最多 1 MiB。

本包为未签名 Windows x86_64/MSVC 开发构建，macOS/Linux 未进行本轮原生验收。许可、source.json、sbom.json、SHA256SUMS 随包提供；正式分发的安装/更新及完整许可文本审计仍属后续。
