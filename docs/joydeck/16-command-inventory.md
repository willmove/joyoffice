# JoyDeck 行为与动作目录

从产品 FR 推导动作；不继承外部命令名或数量。M1 使用下列 GPUI Action 名称；M2以后为拟定行为键，尚未开放机器 API，具体 schema 在实现时固定。

| ID | 用户行为 | 阶段 | 副作用与验证 |
|---|---|---|---|
| viewer::Open | 选择原生文稿/PPTX | M1 | 后台读取；失败保留当前页；Ctrl+O |
| viewer::Cancel | 取消当前读取 | M1 | 使请求失效；迟到结果丢弃；Escape |
| viewer::Previous / Next | 上一页/下一页 | M1 | 页码限界；PageUp/PageDown、方向键 |
| viewer::First / Last | 首尾页 | M1 | Home/End |
| viewer::ZoomIn / ZoomOut | 放大/缩小 | M1 | 25–400%，越界限幅 |
| viewer::Fit | 适应可用视口 | M1 | 随窗口大小重新计算；Ctrl+0 |
| viewer::Report | 查看兼容报告 | M1 | 页/部件/原因可见 |
| document.edit | 领域修改事务 | M2起 | 参数校验、undo/redo、文档与视图目标隔离 |
| document.save | 保存稳定快照 | M2起 | 原子替换；revision一致；失败保留旧文件 |

M1 动作不提供内容写入、保存、格式化和导出。编辑动作必须在对应阶段有 FR、schema、enabled 条件、测试与用户界面后才能开放。

## M2领域注册目录

当前注册元数据位于edit.rs的COMMANDS，参数使用类型化Command并在提交前验证；enabled查询不发布模型，文档身份明确。

| ID | 行为 | 历史 |
|---|---|---|
| page.add / page.copy / page.delete | 页面新增、批量复制/删除 | 单事务 |
| page.reorder / page.visibility / page.size | 页面重排、隐藏、尺寸 | 单事务 |
| object.add / object.update / object.delete | 基础对象与批量属性 | 单事务 |
| object.reorder | 多选图层重排 | 单事务 |
| transaction.batch | 复合修改及失败回滚 | 单事务 |

保存、打开、恢复和关闭仍属服务，不进入文档undo。JSON/CLI/MCP外部schema在M12定义，当前目录不宣称外部机器接口已开放。

## M6 实施增量（2026-10-30）

新增已注册undoable dispatcher ID `document.design`：Action覆盖SetDesign、ApplyLayout、DeleteLayout、Notes、Properties、Section/DeleteSection、ReplaceAll/ReplaceFont、PastePages。所有修改先在独立draft验证后提交，一次历史事务；非法引用/主题槽位/范围/预算整次拒绝。查找定位、大纲导航、选择/当前页和新窗口属于view操作，不进入文档历史。UI只开放当前有实现的属性，完整能力限制见 [28](28-m6-implementation-report.md)。

## M7 已实现入口（2026-10-30）

交付面板提供导入、源包保留写回、新建有损PPTX、PNG/透明PNG/JPEG、PDF/备注PDF、取消任务和首/当前页静态放映。导出/ViewSession操作不进入文档历史，也不改变dirty；导入建立新Session并检查旧身份/revision。源包可修改子集和输出限制见 [29](29-m7-implementation-report.md)。放映键为左右/上下/Space/Enter/PageUp/PageDown、Home/End、Esc及鼠标左右键，M10已补充动画/演示者入口，其当前能力以末节和32为准。


## M8 已实现入口（2026-10-30）

注册 `table.edit`，Action包含SetCell/Paste、Insert/DeleteRow/Column、Merge/Split、RowHeight/ColumnWidth、DistributeRows/Columns、Style/CellStyle。结构/尺寸/矩形粘贴均为单事务，预编辑与拖动为视图草稿。CellNext/CellPrevious在Editor且非Input上下文绑定Tab/Shift-Tab，避免框架遍历抢占；OpenFile、SaveAs接系统选择器。原生存储更新为table-v1，跨文稿对象/页复制复用既有dispatcher，外部CLI/MCP仍留在M12。验证见 [30](30-m8-implementation-report.md)。


## M9/M10 已实现入口（2026-10-30）

图表网格/TSV/类型/样式、基础图示文本/布局通过现有object.update事务；转换图示使用transaction.batch，确认损失后删除原对象/添加形状，一步undo。注册presentation.motion持久化时间线/转场/Morph匹配/顺序/链接/自动翻页，先验证并清理已删除目标；复制页面重映射动画目标/链接，粘贴对象清除冲突匹配名。

动画预览、从头/当前页、演示者视图、排练、屏幕选择/交换、pause/seek、黑白屏/激光/墨迹属于ViewSession/Controller，播放位置不入undo。墨迹与排练只另存新副本，拒绝覆盖已有目标。F5/Shift+F5、Cmd+N/Cmd+Shift+N及放映快捷键见 [32](32-m10-implementation-report.md)。这仍不是外部CLI/MCP已开放的声明。
