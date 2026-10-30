# 本地邮件合并

2026-10-30，EDIT-012／T807。选择本地 DOCX 模板和 UTF-8 CSV，逐字段映射、逐行预览，再批量生成独立的 DOCX 信件。处理不调用网络或 Provider，也不修改模板、CSV 或当前编辑窗口。

## 模板和数据

在普通可编辑段落或表格单元格中输入 `{{姓名}}`、`{{单位}}`、`{{编号}}` 等文字占位符。字段可重复、可跨文字 run；替换文字继承第一个左花括号的字符格式，段落格式保持。字段名内部首尾空白被去除，名称区分大小写。字段不能跨段落或行内对象。

CSV 第一条记录是非空、唯一的列名；每条数据记录须与列数一致。支持 UTF-8 BOM、CRLF／LF、逗号、双引号转义、引号内换行、空值和 Unicode。值按文字处理，`001` 保留前导零，`{{字段}}` 不会再次解释。引号内 CRLF／CR 归一为软换行；其他文本不做 Unicode 规范化。错误编码、引号、列数、缺失映射和超预算均明确拒绝。

可直接使用原创 [模板](../../../tests/fixtures/mail-merge/template.docx) 和 [CSV](../../../tests/fixtures/mail-merge/recipients.csv)。三条虚构数据分别覆盖逗号、双引号和换行。

## 桌面入口

1. 文件 → 邮件合并，依次选择已保存的模板和 CSV。
2. 每个字段默认选择同名列；“下一列”循环选择，“使用此列”确认。取消退出。
3. 预览窗口显示实际合并并通过 DOCX 导出校验后的文字，“下一条”循环检查记录。超过 3000 字符明确标注截断。
4. “生成全部”后选择输出父目录。完成对话框包含数量与新目录路径，状态栏显示简短成功提示；每封信件名为 `letter-0001.docx` 等。Windows 可能视觉缩略长路径，可访问文本保留完整值。

预览取消不写输出。生成期间再次选择邮件合并可请求取消，关闭窗口也会请求取消；在信件处理边界响应，取消或错误会清理本次拥有的目录，清理失败明确报告目录路径。不会删除父目录中的其他文件。当前画布的未保存修改不属于磁盘模板，先保存后再选择模板。

## 命令行

```text
joyword-cli merge --template tests/fixtures/mail-merge/template.docx --data tests/fixtures/mail-merge/recipients.csv --preview-row 3
joyword-cli merge --template template.docx --data recipients.csv --map 姓名=收件人 --output-parent output
```

输出目录必须已存在。预览行号从 1 开始，预览与批量输出互斥，预览不创建文件。标准输出为 schema 1 JSON，批量结果包含新目录、文件清单与总字节数；成功退出 0，参数、编码、映射、导出或 I/O 错误退出 2，诊断写标准错误。CLI 不弹框。

## 输出隔离和边界

每批在所选父目录内创建新的 `JoyWord-merge-*` 目录，文件使用 create-new、同步落盘并逐字节读回。不会覆盖已有信件。输出前通过原有 DOCX 导出计划重新导入和语义检查。模板原始内容的保留／拒绝契约继续生效。

默认硬上限：CSV 8 MiB、模板 32 MiB、1000 条数据、128 列、每值 16 KiB、2048 处占位符、每行最多 32768 条编辑命令、整批输出 128 MiB。运行中临时目录可能可见；断电、进程强杀后的目录回收和硬磁盘配额不作保证。

当前模板语法是文字 `{{字段}}`，不计算 Word 原生 `MERGEFIELD` 指令。保护、宏输出、未决文字／格式／表格属性修订、锁定内容控件、不透明块和含已有行内对象的字段段落拒绝。其他不可表示的 DOCX 编辑由导出计划拒绝，不静默降级。没有数据库、Excel 或在线数据源；本轮 Windows GUI 通过，macOS／Linux 原生向导未实测。

CSV 引号与记录规则从 [RFC 4180 §2](https://www.rfc-editor.org/rfc/rfc4180) 独立实现；UTF-8 BOM、LF 与制表符为本产品声明的扩展。实现入口：[共享任务](../../../crates/joyword-shell/src/mail_merge.rs)、[CSV](../../../crates/joyword-shell/src/mail_merge/csv.rs)、[桌面向导](../../../apps/joyword/src/mail_merge_ui.rs)、[CLI](../../../apps/joyword-cli/src/merge.rs)。验收见 [T807](../acceptance/T807.md)。
