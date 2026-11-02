# 导出禁用原因与兼容诊断

2026-11-02。基线 `95f632dee778d53bee67d9391c6c1b402861bca3`，分支 `codex/joyword-1-0-completion`。本轮继续保留工作区已有修改。

用户反馈私有复杂 DOCX 在文件页仅 DOCX 可用。只读诊断确认 `style_approximation_required=true`、`read_only=false`、`partial=true`；文档有表格和图片，以及原文保留的 XML 片段。实际阻止导出的是既有 `has_style_approximations` 门禁，不是格式按钮未绑定。私有文件、正文、文件名及路径不进入仓库测试语料或截图；原件逐字节保持不变。

PDF／PDF/A 和打印不能把仅用于显示的近似页面作为完整场景输出。文本格式的生产适配器当前只处理声明的普通正文子集，复杂表格／图片／额外 story 等仍须独立实现。DOCX 副本则可使用保留原包的作业，不能把这个能力解释为已经完成格式转换。

## 修改

- 文件页在版式兼容门禁生效时显示原因说明，在每个禁用格式下显示对应的版式、转换、兼容预览、只读、布局未稳定或作业忙原因。按钮状态和原因从同一个可用性结果取得。
- CLI `inspect`／`text` 新增可选 `--compatibility`，输出 `compatibility.style_approximation_required`；不带该参数时已有 JSON 金样保持不变。可与 `--max-text-bytes 0` 合用，只输出元数据；退出码 3 沿用文本截断契约。
- 新增原创异常运行属性样例的端到端回归，验证模型标记、默认 JSON 不变、零正文输出、重复参数拒绝与源件不变。

## 验证范围

Word 分产品构建通过；fmt、严格 Clippy 和 1316 项测试通过，零失败／忽略；文档、架构和原子验收元数据检查通过。默认 inspect 金样未变化，可选兼容诊断回归通过。

原生界面使用原创合成 DOCX 复现同一版式兼容条件。完整命令、自动检查与截图见 [证据索引](../validation/20261102-export-availability/verification.json)。此次没有解除导出保护或实现复杂格式转换；M0–M7 父任务计数保持不变。其他平台 GUI 和完整辅助技术验收未执行。
