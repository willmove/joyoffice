# T603 PDF 导航、结构标签与页面图片输出自动化证据（2026-10-28）

## 交付范围
1. **Tagged PDF 与逻辑结构标签 (ISO 32000-1 §14.7)**：
   - 结构树支持：`PdfExport` 增加 `tagged: bool` 与 `lang: Option<String>` 开关。
   - 标记内容序列：通过 `krilla::tagging` 输出 `/StructTreeRoot`、`/Lang` 属性与标准逻辑结构元素（Part, Span, Other）。
   - 饰品与非语义内容分离：粗体阴影轮廓、页面背景矩形与边框标记为 `/Artifact`，保证屏幕阅读器与检索时不重复读取。
2. **PDF 结构独立检测器扩展**：
   - `crates/joyword-pdf/src/check.rs` 扩展解析 `/Catalog` 下的 `/MarkInfo`（验证 `Marked: true`）与 `/StructTreeRoot` 引用，并纳入 `PdfReport`（`is_tagged`, `has_struct_tree`）。
3. **分级书签与链接导航**：
   - 模型书签到 Named Destinations、页面内部 GoTo 跳转动作与合规 URI 外链导出及独立 xref 回读校验。
4. **页面图片输出**：
   - 基于统一 `PageScene` CPU 光栅器（`joyword-render`）输出确定性 PNG 图片（`joyword-cli render`），支持多 DPI 规格化输出与尺寸校验。

## 自动测试命令与结果
```bash
cargo test -p joyword-pdf
```
42 项测试全部通过，包括新增的 `test_tagged_pdf_structure`（检验 `/MarkInfo`、`/StructTreeRoot` 存在且经独立 `pdftotext` 文本抽取验证一致）。

```bash
cargo test -p joyword-cli --test visual
```
页面图片输出重复性、尺寸正确性与防覆盖机制全部通过。

## 未验证项
- 真实辅助技术屏幕阅读器（NVDA、Orca、VoiceOver）人工朗读验收未做，标未验证。
- 两个以上外部阅读器（Acrobat Reader、Foxit Reader）人工视觉与标签树交互检查未做，标未验证。
