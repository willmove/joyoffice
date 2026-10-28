# T602 旧版 MS-DOC (.doc) 导入自动化证据（2026-10-28）

## 交付范围
1. **MS-DOC 二进制结构解析（基于 [MS-DOC] 与 [MS-CFB]）**：
   - 基于自研零依赖 `joyoffice-cfb` 解析 WordDocument、0Table、1Table 等核心流。
   - FIB（File Information Block）解析：校验魔数 `0xA5EC`、产品版本 `nFib`（Word 97–2003）、表流选择标志位 `fWhichTblStm` 与加密位 `fEncrypted`。
   - 分段表与分块描述符解析：遍历 Clx、Prc 规则并解析 PlcPcd（Piece Descriptor Table），支持 8 位压缩字符（ANSI/Latin-1）与 16 位非压缩 UTF-16LE 字符解码。
2. **段落、表格与内容模型映射**：
   - 提取主文档字符流并转换为 `joyword_model::Document`。
   - 识别段落结束符 `\r` 与表格单元格分隔符 `\x07`，结构化生成段落与表格行/单元格块。
   - 异常输入与资源预算控制（`DocBudget`）：最大文件字节数、文本字符上限与 Piece 描述符数量上限。
3. **安全与限制契约**：
   - 加密 DOC 探测到 `fEncrypted` 标志时显式报错（`DocError::EncryptedDocument`），拒绝静默读取。
   - 识别并向 `LossReport` 报告未处理的嵌入图片/OLE 锚点（`0x08`）与二进制域标记（`0x13`/`0x15`）。

## 自动测试命令与结果
```bash
cargo test -p joyword-doc
```
5 项综合测试全部通过：
- `test_doc_basic_import`：标准 DOC 导入、FIB 校验、PlcPcd 文本提取与段落划分测试通过。
- `test_table_parsing_and_loss_report`：包含单元格分隔符 `\x07` 的表格解析与损失报告测试通过。
- `test_encrypted_rejection`：加密文档明确拒绝测试通过。
- `test_malformed_and_budget`：畸形容器与资源预算超限拦截测试通过。

## 未验证项
- 使用真实 Microsoft Word 生成的原生 .doc 文件进行人工语义与版面外观对照未做。
