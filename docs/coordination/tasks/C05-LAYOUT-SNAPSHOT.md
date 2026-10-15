# C05：文档排版快照、单节分页与命中几何

- 执行者：当前 Codex 协调者，单人执行。
- 基线：01e4b6d28682c02ce55dd10c33ce9e6447f1c8c3（C04）。
- 分支：codex/coordination-layout-snapshot。
- 独立目录：/Users/willm/.codex/worktrees/joyoffice-coordination/joyoffice。
- 对应：T105，LAYOUT-002、LAYOUT-003、LAYOUT-009；为 T102 的位置输入提供真实数据。

## 交付

将 engine 可提供的 DocumentSnapshot 接入现有实际字体／自动断行链路，读取单节物理页框，生成持有版本、内容指纹、实际 FontKey 和排版 profile 的不可变 LayoutSnapshot。跨页 PageScene、源段落／行／cluster 范围、光标、命中与选区几何均使用 EMU，缩放只在输出端转换。

命中返回 model::Position，并覆盖 wrap 边界亲和性、空段落与跨段选区。不可表示的 cluster 内位置明确拒绝；未知节点、对象、额外 story／section、尚未解析的字体／样式和布局属性不得被静默忽略。长段、页面／行／字形／整形总工作量、诊断、选区输出均有预算，取消或失败不返回成功的半份快照。旧版本、内容、字体或 profile 不匹配必须拒绝使用。

## 边界

新增 layout 的 document 模块、增量导出、测试与示例；line 内部增加候选间取消检查而不改变既有入口行为。协调者批准 layout 新增现有 workspace model 依赖；测试可接现有 engine，PDF 的集成示例可接现有 model／engine／DOCX crate。Cargo.lock 仅内部边，不增加或升级外部包。model／engine／OPC／字体／整形／PageScene 公共 API 保持不变。协调者可增加独立 PDF 读回核对器及其故障检测测试，将真实多页示例接入既有 CI；公共实施／任务文档仅由当前协调者维护。

当前先实现单实际字体、固定显式度量和能完整表达的正文流；未实现的混合字体／样式、Word 兼容规则与对象不伪装成完成。目标仍是完整 T105/T102，不以此切片重定义其验收。

## 验证

真实字体、DocumentSnapshot→多页场景→PDF 的连续内容与页面几何；命中→engine 事务→重新布局；空段落、wrap 亲和性、emoji／组合／连字 cluster、跨页选区、逆范围与未知锚点；版本／字体／profile 陈旧拒绝；预算、取消、极端尺寸及失败原子性。运行相关与完整默认成员、release、fmt、clippy、文档和 Python 工具测试。GUI／Word／WPS 与物理断电不是自动测试替代项。
