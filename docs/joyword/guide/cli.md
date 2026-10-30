# 无头命令行

程序是 `joyword-cli`。帮助文本在 [main.rs](../../../apps/joyword-cli/src/main.rs) 的 `HELP`。不改输入文件；邮件合并在独立目录创建信件，见 [使用指南](mail-merge.md)。

## 使用者

```text
joyword-cli inspect|text [--max-text-bytes N] [--] FILE
joyword-cli render FILE --font PATH:FACE --output DIR [--dpi N] [--page N]
joyword-cli convert FILE --to pdf --font PATH:FACE --output FILE.pdf
joyword-cli merge --template FILE.docx --data FILE.csv [--map FIELD=COLUMN] (--preview-row N | --output-parent DIR)
```

选项可以写在文件名后面。`--` 结束选项解析，用来传入以 `-` 开头的路径。

| 子命令 | 做什么 |
| --- | --- |
| `inspect` | 标准输出一份 schema 1 的 JSON。`partial` 和 `truncated` 彼此独立。 |
| `text` | 标准输出 UTF-8 正文。投影、是否部分、是否截断的 JSON 在标准错误。 |
| `render` | 把统一页面场景画成确定性的 CPU PNG，写到目录。 |
| `convert` | 用同一场景写出可检索 PDF。`--to` 不是 `pdf` 就拒绝。 |
| `merge` | 显式映射本地 CSV 和 DOCX 文字模板，预览或批量生成信件；schema 1 JSON，错误退出 2。 |

退出码：

| 码 | 含义 |
| --- | --- |
| 0 | 在声明的投影里完整读完 |
| 2 | 错误（打不开、超预算、不支持的命令、坏字体、坏 DPI） |
| 3 | 正文因字节上限被截断 |

投影：导入后的主文档正文，包含隐藏文字；删除修订、批注和其他故事不出现；插入和 moveTo 出现。不支持的 XML 由导入保留，但不进入这段文字。不透明块跳过、计数，不占页面几何。

预算（与帮助文本和 `visual.rs` 常量一致）：

| 项 | 值 |
| --- | --- |
| 输入文件 | 32 MiB |
| 正文字节 `--max-text-bytes` | 0 到 1048576，默认 1048576 |
| 字体文件 | 128 MiB |
| 页面 | 最多 64 页 |
| 输出 | 256 MiB |
| DPI | 默认 144，允许 1 到 1200 |
| `--page` | 从 1 起算 |

打开包时的 OPC 预算：最多 1024 个部件，单部件 16 MiB，合计 64 MiB，XML 深度 128，节点 1000000，关系 4096。错误信息不回显包内路径。

字体写成 `路径:face序号`。没有系统字体回退。仓库不附带字体。

## 开发者

文本路径在 `execute_text`。光栅和 PDF 在 [visual.rs](../../../apps/joyword-cli/src/visual.rs)，调用与窗口相同的 `layout_document` 和页面场景，不做另一套分页。`render`／`convert` 的 JSON 也是 schema 1，并报告 `omitted_blocks`。

帮助文本里的投影句子就是上表。测试覆盖五个原创样例的稳定 JSON 与 PNG，同一输入两次运行逐字节相同；见任务 T210 的完成证据，本页不另列未在此核对的命令输出。
另有两个示例工具不参与上表的命令：`feature_scenes` 生成页面场景样例（CI 的 unified-pdf 作业用它），`chapter_probe` 报告一个 DOCX 导入后的章节页码设置，用来区分「`w:pgNumType` 已类型化导入」与「退回保留通道」。两者的用法见 [第二波验证手册](../verification/wave2-manual-verification.md)。

## 限制

- 没有编辑、保存或打印子命令。
- 不读页眉、页脚、脚注、批注和删除修订。
- 隐藏文字会计入正文。这和「最终」视图不同。
- 超过 64 页、输出超过 256 MiB，或字体超过 128 MiB，命令失败。
- 图形界面上的窗口画面与 `render` PNG 是否一致，要靠人工看；Windows 上对 DOCX-MIN-001 有过对照，本环境没有重做。

## JoyWord 假定，没有 Word 观察

- 这段投影是导入正文，不是 Word 的最终视图或修订视图。源文件模块注释写明这是有意的。

## EPUB 导出

`epub` 提供计划与新文件输出，语法、预算、差异接受及拒绝范围见 [EPUB 指南](epub.md)。同一输入和固定元数据可逐字节重现；不改写 DOCX。

PDF/A-2b 使用 `convert --to pdfa-2b --archive-year YEAR`，完整语法与边界见 [归档指南](pdfa.md)。
