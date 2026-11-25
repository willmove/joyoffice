# 阶段 2 · JoyDeck 形状坐标和变换

媒体引用和跨文稿粘贴先提交为 `1c98bcb1`。形状变换沿用 `14cbb296` 的 Windows 冻结 CLI，以独立生成的 DrawingML 双层组做兼容对照。

## 实现

- 形状和组使用同一套坐标实现。组内坐标先根据子空间偏移和尺寸映射到组外框，再沿各层组的中心执行翻转与旋转。画布绘制、形状定位和连接点求解共用这套变换。[ECMA-376 Part 1](https://ecma-international.org/publications-and-standards/standards/ecma-376/) · [DrawingML 组变换](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.transformgroup?view=openxml-3.0.1)
- 编辑时把绑定目标的连接点换算到连接线所在的父级坐标。预设连接线和目标形状一起进入撤销记录，未解析的端点保留原作者位置。PPTX 保留写入仅更新相关坐标字段，嵌套连接线的扩展属性和节点随原 XML 保留。[DrawingML 起点和终点](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.connectionshape?view=openxml-3.0.1)
- 直接坐标命令与画布手柄使用同一事务入口。多选编辑在全部形状通过尺寸和坐标检查后生效。

## 验证结果

| 验证 | 结果 |
| --- | --- |
| 嵌套组兼容性 | 双层组包含非零子坐标原点、各层旋转、横翻和纵翻。原稿在冻结与候选 CLI 下的渲染逐像素一致。直接编辑和多选非等比缩放均通过坐标验算。 |
| 连接端点 | 四条预设连接线覆盖组内、跨组及无法解析的端点。冻结版的跨组端点最多偏移约 270.15 点，候选版全部端点误差低于 0.00014 点，验收阈值为 0.001 点。6 张渲染图均命中各自预期的端点像素。 |
| 撤销、重做和保存 | 两版 CLI、两种格式、四个编辑状态共 32 组交叉读回通过。16 次撤销／重做比较中，重新打开的完整模型与相应编辑状态完全一致。8 份 PPTX 保留写入中的原始扩展和其余包部件均通过比较。 |
| Windows 桌面 | 在含翻转的双层组上拖动旋转手柄和缩放角点，执行两次撤销和两次重做。保存 `.pptx` 和另存 `.joydeck` 后分别在新进程重开。两版 CLI 对两份桌面保存结果的 4 组读回均通过，端点最大误差低于 0.00018 点。 |
| 产品回归 | `cargo xtask check joydeck` 通过 1,045 项测试、严格 Clippy、格式、依赖和来源检查。R2 在冻结和候选各通过 20 条命令，上一批媒体和跨文稿回归通过。 |

横翻和纵翻通过带翻转标志的文件验证。自定义几何连接点和连接线接到连接线的情形沿用文件原有端点。桌面交互验证覆盖 Windows。

[验证结果与散列](verification.json) · [冻结版对照脚本](../../../../scripts/joydeck/check-stage2-coordinates.py) · [媒体和跨文稿回归](../../../../scripts/joydeck/check-stage2-transfer.py)
