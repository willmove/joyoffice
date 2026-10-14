# C02：保守 DOCX 编辑导出计划

- 执行者：当前 Codex 协调者，单人执行；不重新启动 W01–W04。
- 基线：main 的 b48ca9b426b80c220f0b2518d1e0ad363e07afea。
- 分支：codex/coordination-docx-export。
- 对应任务：T103、T106；需求 FORMAT-002、FORMAT-003、QUALITY-005。
- 优先级：先补齐已接入会话的编辑导出，再推进文件系统保存；M0 阶段门槛不变。

## 交付与边界

在 joyword-docx 增量实现不可变 ExportPlan，记录源版本、目标类型、变更部件、原样保留部件与支持限制。对现有正文段落、现有字符格式边界内的纯文本修改进行 XML 字节补丁；不能证明保真的结构／格式／对象改动明确拒绝。源 XML、未知属性、未知部件和关系不能静默删除。数字签名包的编辑导出拒绝，未编辑时可精确复制。

shell 的 DocxSession 持有原预算，导出器从 backing 重建可信基线，通过只读接口生成计划；计划绑定快照，迟到结果不得代表新版本。仍保留原未编辑复制接口的版本契约。新增接口不改变 model、engine、OPC、字体／场景／PDF 公共 API，不增加依赖。

## 验证

真实 importer → engine → ExportPlan → importer 往返；空段落输入、混合语言、emoji、组合字符、XML 转义、空白、tab、重复导出、撤销／重做；未知内容与属性精确保留；保护、数字签名、格式／结构变化与预算拒绝；失败不改会话或原包。执行相关 crate 和全默认 workspace 测试、fmt、clippy、文档及 Python 工具检查。

此任务不宣称文件系统原子保存、元数据保留、文件锁、GUI、Word/WPS 或 M1 验收完成。
