# 第三轮接口与集成约定

本文件由 C09 维护。基于 main（Wave 02 已合入）与 PR #10 文档台账。不预先创建返回成功的占位函数。下述签名是本轮批准的实现目标；确需改签名或新增依赖时在报告提案，由用户转交 C09 裁决。

## 前置：W07 已交付、仍阻塞原件写入的契约

已有（勿破坏）：

```rust
// joyword_shell::storage
pub fn open_source(path: &Path, budget: &Budget) -> Result<OpenedSource, SaveOriginalError>;
pub fn check_source(source: &SourceStamp) -> Result<(), SaveOriginalError>;
pub fn save_original(plan: &ExportPlan, source: &SourceStamp)
    -> Result<SaveOriginalReceipt, SaveOriginalError>;
pub fn save_export_copy(plan: &ExportPlan, dest: &Path)
    -> Result<SaveCopyReceipt, SaveCopyError>;
```

`save_original` 今日在写入前因 `OriginBindingUnproven` 等 blocker **一律拒绝**。`save_export_copy` 仍是已验证的新副本退路。

## W10 → C09／W11：ExportPlan 来源绑定

实现 [W07 报告提案 1](../reports/W07.md)（允许精确细化，须保持可证明性）：

- `plan_export` **不改**公开签名。
- `ExportPlan` 私有增加来源字节绑定（建议 `origin: Option<Arc<[u8]>>`，值为 `backing.original_bytes()` 的廉价共享），并提供只读访问器，例如：

```rust
impl ExportPlan {
    /// Bytes of the package backing this plan was derived from.
    /// `None` only if the plan cannot prove origin (then save_original must refuse).
    pub fn origin_bytes(&self) -> Option<&[u8]>;
}
```

- 调用方不得伪造 origin：无公共构造器把任意字节塞进计划；只由 `plan_export` 从 `PackageBacking` 填入。
- 版本号相同 **不能** 代替来源相等。W11 的 `save_original` 在冲突复查后要求 `plan.origin_bytes() == source` 所绑定的精确字节，否则拒绝。
- 既有 `bytes/version/mode/target/changed_parts/preserved_parts/warnings` 语义不变；穷尽匹配 `ExportMode`／错误枚举若新增变体须在报告列出并对调用方无破坏，或保持不变。

W10 可只交付 docx 侧绑定与测试；**不**改 shell storage 写入逻辑（那是 W11），**不**改 app。

## C09-A：`open_source` → `DocxSession`

- 打开路径：`open_source` → 取 `OpenedSource::bytes()`／`stamp()` → `DocxSession::open(bytes, budget)`；解析失败不安装会话、不丢 stamp 诊断。
- 会话须能在后续导出时自然带上同一 backing 的 origin，使 W11 相等检查成立。
- C09 独占 `docx.rs`／`documents.rs`／`lib.rs`／命令与文案；W10／W11 不得改这些文件。
- JoyWord 锁文件的**创建／释放／所有者**归 C09 会话生命周期；W11 只检测（W07 已登记名 `.<文件名>.joyword-lock`）。

## C09-B：应用外壳（T106 切片）与保存语义

| 用户动作 | 允许的实现 | 禁止 |
| --- | --- | --- |
| 打开 | 文件对话框 → `open_source` → `DocxSession`；失败可解释 | 无适配器的“通用 open_file”塞进可编辑会话 |
| 另存为／导出副本 | 仅 `save_export_copy` 到**新路径**；成功可更新“当前路径”策略须在报告写清，且不得默示原件已替换 | 覆盖已存在目标；把副本成功显示成“已保存原件” |
| 保存（原件） | 仅当 W11 使 `save_original` 在该平台可用；绑定会话身份＋请求身份＋文档版本 | 在仍 `ReplacementUnavailable` 时显示保存成功或静默改走另存 |
| dirty／关闭 | 沿用 C07 未保存关闭；导出副本默认**不**清 dirty、不改来源，除非 C09 文档化的明确产品规则 | 迟到回执复活已关闭会话；晚到计划清除新编辑 dirty |

保存任务绑定：只有持久化结果与仍打开的对应会话相符才更新保存版本；更新后再次编辑仍 dirty。

## W11 → C09：`save_original` 发布策略

在 W10 绑定可用后，按 [W07 提案 2](../reports/W07.md) 实现（可微调，须保留安全性质）：

1. 自行重跑完整冲突验证（不信任先前的 `check_source`）。
2. 要求 `origin_bytes` 与 stamp 字节精确相等；包类型与扩展一致。
3. 能力矩阵：无法保证的元数据 → 写入前拒绝（不静默丢 ACL／xattr／来源标记等）。
4. 发布草图：同目录临时文件 → 写 → sync → 逐字节回读 → 将当前目标硬链接为私有备份并回读确认与 stamp 一致 → rename 发布；失败语义对齐 C03（发布前保留原件；发布后错误携带已发布回执）。
5. `save_export_copy` 回归不得破坏。

未验证平台不得宣称“原件保存已支持”。Unix 权限位恢复若实现须测试；所有权／ACL／xattr 等仍可能保持拒绝直至 C09 批准平台依赖。

## W12 → C09：带格式段落结构导出

基于 [W05 提案 3(a)](../reports/W05.md)：

- 继续使用现有 `plan_export`／`ExportPlan` 调用方式。
- 范围：完全映射、无保留片段、无“不能证明安全”的命名空间声明的 **pPr／rPr**，在拆分／合并时**原样复制**源属性字节到新段落／run；默认格式路径（W05）不退化。
- 仍拒绝：未知属性／未映射内容、对象、节边界结构编辑、表格／SDT、保护／签名等既有拒绝类。
- 往返：同预算重新导入；语义＋节边界位置＋保留片段多重集合检查保持。
- 不改 importer／model／engine；不宣称 Word／WPS 验收。

与 W10 同时改 `export/**` 时：开发可并行，**集成顺序 W10 → W12**。

## W13：W09 重锚定

- 只改 `experiments/docx-robustness/**` 与证据。
- 新 `TARGET_SHA` 必须是用户／C09 核准的生产 tip（建议 PR #10 合并后、或本轮集成后的明确提交），并更新 build.rs／driver／README／证据摘要。
- 固定种子变异 ≠ 覆盖引导 fuzz；不安装 nightly／cargo-fuzz。

## W14：恢复日志

- 本轮默认**设计／提案 only**。生产接入点（会话生命周期、持久化时机、与 dirty／保存版本关系、恢复 UI）须等 C09-B 打开／另存语义写清。
- 不得改 W11 storage 职责边界；engine 撤销仍是唯一实现。

## 协调者接入边界（重申）

C09 不在执行者目录直接实现提案。集成冲突在协调者 checkout 处理。远程 CI 绑定集成 SHA。GUI／Office／IME／断电证据另列，不因本轮测试通过而勾选 TASKS done。
