# T605 WPS 原生格式 (.wps/.wpt) 导入自动化证据（2026-10-28）

## 交付范围
1. **WPS 容器与变体识别**：
   - 识别基于 MS-CFB 复合文档容器的 `.wps`（文档）与 `.wpt`（模板）格式。
   - 探测 WPS 特有私有流（`KSOTree`、`WpsSummaryInformation`、`KsoProductInfo` 等），准确区分 DOC 兼容流与 WPS 私有流。
2. **私有流原样保留与哈希跟踪**：
   - 遍历并原样提取全部非核心流，计算其 SHA-256 散列并记录于 `DocMetadata.preserved_streams`，支持后续写回时不破坏专有数据。
3. **DOC 兼容流解析与模型导入**：
   - 复用 `joyword-doc` 引擎，解析 WordDocument 与 Table 流中的 FIB 与 Clx 结构，提取正文文字、段落与表格结构。

## 自动测试命令与结果
```bash
cargo test -p joyword-doc
```
测试通过：
- `test_wps_detection_and_preservation`：验证正确识别 WPS 原生格式（`DocumentFormat::Wps`），提取正文段落，并将私有流（如 `KSOTree`）原样保留且哈希比对一致。

## 未验证项
- 使用真实 WPS 软件生成含多重专有修饰的 .wps 文件并在 JoyWord 与 WPS 中进行人工比对未做。
- WPS 早期非 CFB 私有二进制变体未做（仅支持现代 CFB 容器格式）。
