# 第二轮接口与集成约定

本文件由 C08 维护。基于 C07 的现有 API 是并行开发的稳定起点；不预先创建返回成功的占位函数。下述新增签名是本轮协调者批准的实现目标，执行者可以实现；确需改签名或新增依赖时在自己的报告列出提案，由用户转交 C08 裁决后再改。C08 接入新增 API 以实际交付为准。

## W05 → C08／W07

继续使用 `joyword_docx::plan_export`、不可变 `ExportPlan` 的现有签名和 `bytes/version/target/changed_parts/preserved_parts/warnings`。扩大可证明安全的结构子集，不更改现有调用方式或枚举穷尽匹配契约。新结构导出仍通过原 OPC 预算重新导入并比对语义，不能让调用方用伪造 editability 解除保护。

W07 使用 C07 的真实 ExportPlan 测试存储，完全不依赖 W05 是否已完成拆分／合并。W05 若需要新的顶层导出、importer 变更或源引用规则，先在 W05 报告提出，不能越界改 model／engine／OPC／shell。

## W06 → C08

保持 `layout_document`、SingleFontProfile、LayoutSnapshot 的已有签名和 PageScene 不变；现有 `ensure_current/caret/hit_test/selection_rects` 在新分页后继续有效。输入来自不可变 snapshot，返回几何仍是 EMU；不另建屏幕专用分页。新增统计／诊断优先使用内部类型；确需公共类型先提案。W06 不接管字体回退、四槽字体或混合 run 格式，后续另派切片。

## W07 → C08

所有新增存储 API 放在已公开的 `joyword_shell::storage` 下，避免与 C08 的 shell/lib.rs、docx.rs 冲突。原 `save_export_copy(&ExportPlan, &Path)` 及现有回执／错误签名保持不变。

首份交付目标接口（具体私有字段由 W07 实现；不是已有接口声明）：

```rust
pub fn open_source(path: &std::path::Path, budget: &joyoffice_opc::Budget)
    -> Result<OpenedSource, SaveOriginalError>;
pub fn check_source(source: &SourceStamp) -> Result<(), SaveOriginalError>;
pub fn save_original(plan: &joyword_docx::ExportPlan, source: &SourceStamp)
    -> Result<SaveOriginalReceipt, SaveOriginalError>;
```

- `OpenedSource::bytes(&self) -> &[u8]` 与 `stamp(&self) -> &SourceStamp` 提供同一次有界读取所得的字节及源身份；来源令牌私有不可伪造。C08 用这些字节调用 DocxSession::open，解析失败不安装会话。源身份含文件系统对象／路径解析及字节基线信息，不能只比较大小和 mtime；不得无界读取大文件。
- `check_source` 仅作冲突预检。预检后到替换仍有竞争窗口；save_original 必须自己重新验证并有可解释的同步／平台策略，不能把两个函数顺序调用当作原子 compare-and-swap。
- 新回执至少提供 `path()`、`version()`，以及有类型的持久化状态；替换后失败的错误必须携带已发布回执。SourceStamp 刷新须来自成功落盘并读回的实际对象，旧 stamp 不可悄悄复用。
- plan 与来源必须有可靠绑定，版本号相同不能证明是同一文档／原包；不能仅凭调用者传入两个独立对象就允许覆盖。若 C07 ExportPlan 缺少可验证的来源绑定，W07 完成打开、预检及安全拒绝，向 C08 提交精确增量接口方案，不能放宽此契约。
- 符号／硬链接、ACL、扩展属性、来源标记等若无法在现有依赖及 unsafe 禁止约束下保证，save_original 必须在改写前明确返回不支持。以原有新副本能力交付用户可用退路，原件替换列为阻塞；不创建伪成功后端。不执行自动“另存”或静默丢弃元数据。
- 平台实现细节可放 `storage/` 子目录。W07 提交带实际结果的能力矩阵；未验证平台不得报告已支持。仅检测 Office `~$` 文件不足以获得独占，锁存在／状态不明保守拒绝；锁过期也不能擅自删除。

## 协调者接入边界

C08 独占文件打开／会话生命周期、docx.rs 适配、app 命令／文案以及保存版本跟踪。保存任务绑定会话身份＋请求身份＋文档版本；只有持久化结果与仍打开的对应会话相符才更新保存版本；更新后再次编辑仍 dirty，关闭后的迟到回执不得复活会话。导出副本不改当前文件来源、不清 dirty；组合预编辑不直接当持久文档导出。W07 不修改 session/backing/history。

W05／W07 的第一份报告可作为接口审查点；需要裁决时先完成不依赖提案的测试和局部交付。C08 不直接在执行者目录实现提案。集成冲突在协调者的集成 checkout 处理，重新跑受影响 crate 与全默认成员、应用无窗口测试／开发构建、fmt／clippy、文档／工具检查；未满足的 GUI／Office／平台门槛继续列明。
