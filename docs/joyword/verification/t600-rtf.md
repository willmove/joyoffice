# T600 RTF 1.9 读写与交换自动化证据（2026-10-28）

## 交付范围
1. **RTF 1.9 双向词法分析与解析器**：
   - 组嵌套结构：支持标准 RTF 花括号作用域 `{ ... }`，维护格式继承与栈恢复。
   - 控制词与控制符：支持 `\rtf`, `\ansi`, `\b`, `\i`, `\strike`, `\par`, `\cell`, `\row`, `\trowd` 及可忽略目标标记 `\*`。
   - Unicode 编码与跳过机制：实现 `\uN?` 规范解析与 Unicode 替代字符跳过计数（`\uc`）。
   - 不可信输入资源预算：`RtfBudget` 限制最大组嵌套深度（默认 64）、最大控制词长度（默认 64）与最大文本字节数。
2. **RTF 1.9 生成器**：
   - 输出符合标准 RTF 1.9 规范的文档头、字体表（`\fonttbl`）及段落、运行、表格流。
   - 字符转义：安全转义 `{`, `}`, `\` 及不可见字符；高位字符统一转义为 `\uN?` 格式（兼容非 ASCII 字符与代理对）。
3. **转换损失汇报**：
   - 接入统一的 `LossReport` 机制，对图形对象 `\pict` 或不透明块保留并报告转换损失。

## 自动测试命令与结果
```bash
cargo test -p joyword-rtf
```
4 项综合测试全部通过：
- `test_rtf_roundtrip_basic`：基础段落、字体修饰与 Unicode 往返测试通过。
- `test_rtf_table`：表格行、单元格结构导入导出测试通过。
- `test_rtf_ignorable_destination`：可忽略目标 `\*` 跳过测试通过。
- `test_rtf_malformed_and_budget`：畸形头与资源预算超限拦截测试通过。

## 未验证项
- 人工使用 Microsoft Word / WPS 打开导出的 RTF 文件进行实际版面外观比对尚未验证。
- RTF 复杂绘图对象（VML/DrawingML 嵌入）未做。
