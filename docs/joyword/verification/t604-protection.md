# T604 加密、保护与活动内容自动化证据（2026-10-28）

## 交付范围
1. **密码学基础**：自研实现 FIPS 180-4 SHA-1、SHA-256、SHA-512，FIPS 197 AES-128/256-CBC 模式加解密，RFC 2104 / RFC 4231 HMAC-SHA256/512，以及常数时间比较（防侧信道时序攻击）。
2. **文档保护验证**：支持 `w:documentProtection` 的解析与密码哈希校验（ECMA-376 Part 1 §17.15.1.29），支持 SHA-1/256/512 与 spinCount 迭代哈希比对。
3. **活动内容与伪装防御**：
   - 阻断并诊断外部 UNC 路径（`\\server\share`）与本地协议（`file://`）。
   - 保真保留宏容器 `word/vbaProject.bin`、`word/vbaData.xml` 与 ActiveX 对象。
   - 文件扩展名与内容不符检测（PE/ELF 可执行文件伪装、ZIP 与 CFB 格式混淆）。
4. **数字签名篡改检测**：解析 `_xmlsignatures/` 下 XML-DSig 签名描述，重算被签部件摘要，检测并报告篡改。
5. **MS-OFFCRYPTO 加密包解析与解密**：基于 `joyoffice-cfb` 解析 Agile 加密描述符，解密 `EncryptedPackage`，实现往返测试。

## 自动测试命令与结果
```bash
cargo test -p joyword-protect
```
5 项测试全部通过（测试用例覆盖密码学基础、文档保护密码验证、活动内容审计与阻断、XML-DSig 签名篡改检测、Agile 加密包往返解密）。

## 未验证项
- 人工在 Word/WPS 中打开带密码受保护文档并编辑的交换验收未做。
- X.509 数字证书链信任体系验证不在范围内（仅做部件散列完整性校验）。
