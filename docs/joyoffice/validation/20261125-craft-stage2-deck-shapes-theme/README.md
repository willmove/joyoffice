# 阶段 2 · JoyDeck 形状和主题色

形状树使用统一的有界遍历执行查找、父组定位、同层编辑和 ID 分配。预设替换清理失效的调整值。占位符外观依次读取幻灯片、版式和母版的属性，样式矩阵解析填充、描边、效果和背景。原生文件继续使用已有形状记录和序列化名称，沿用来源账本登记兼容字段。[形状模型](../../../../crates/joydeck-model/src/shape.rs) · [继承解析](../../../../crates/joydeck-model/src/resolve.rs) · [来源账本](../../../../licenses/deckcraft/file-map.json)

主题色解析先按幻灯片、版式、母版的优先次序选择适用映射，再从原始调色板取色。直接引用的深浅色槽位使用原始色值，背景、文字、强调色和链接色使用映射。填充矩阵的 `phClr` 先解析样式引用的颜色，再按文件顺序执行变换。编辑区的字体配色和表格样式预览共用这一解析路径。[主题映射](../../../../crates/joydeck-color/src/palette.rs) · [ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/) · [Microsoft 幻灯片母版](https://learn.microsoft.com/en-us/office/open-xml/presentation/working-with-slide-masters) · [DrawingML 填充引用](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.fillreference?view=openxml-3.0.1)

## 回归结果

| 范围 | 实测结果 |
| --- | --- |
| 冻结版 `14cbb296` 与候选版的文件互读 | 56 次 `.joydeck`／PPTX 读回通过，其中 24 次核对撤销或重做快照。页序、对象 ID、版式绑定、预设与调整值、嵌套组、自定义路径均通过。独立检查 8 份 PPTX 的主题槽位、覆盖映射和扩展 XML，4 次无效编辑按预期拒绝。 |
| 主题色和继承 | 83 个渲染探针覆盖映射优先次序、背景、占位符位置及填充／描边继承、`phClr`、透明度和变换顺序。对照用的两页中性样本与冻结版逐像素相同。 |
| PowerPoint 16.0.20625.20108 | 两页导出中的强调色、链接色、背景和占位符探针与候选版一致。18 个反相、灰度、RGB 调制及组合探针逐通道一致。组合淡化后加深、亮度和饱和度在两页各有 3 个探针相差至多 1 个通道值，保留冻结版的逐步量化。PowerPoint 打开并回存候选版编辑、重排后的两份 PPTX，JoyDeck 成功重开；原页 ID 256 和椭圆形状可定位。 |
| Windows 桌面 | 在第二个视图修改母版强调色，第一个视图同步更新。从第一个视图撤销、第二个视图重做，两个视图的颜色和编辑状态随之更新。保存后用最终构建冷启动打开同一 PPTX，核对修改后的颜色和三个页面。 |
| 阶段 2 既有专项 | 坐标与连接端点回归 32 次互读，富文本回归 28 次互读，跨文稿复跑三组资源样本和候选版组对象读回。R2 在两版各跑 20 条控制命令，覆盖历史、过时修订、PPTX 写入及强制崩溃恢复。R4/R5 通过 23 次 CLI 调用及独立 PPTX 读回。 |
| 工程门禁 | `cargo xtask check joydeck` 通过 1,050 项测试、格式、严格 Clippy、来源和依赖检查。Windows 产品构建和文档检查通过。双窗口真机验收覆盖 Windows。 |

反相和 RGB 通道调制使用线性 RGB，灰度使用编码后 RGB 的亮度权重。PowerPoint 16 实测确认这三个路径比冻结版更接近其输出，样本使用相同的操作顺序和透明度。色相转换参考 HSL 的公开公式，线性百分色和 sRGB 共用传递函数。[颜色变换](../../../../crates/joydeck-color/src/transform.rs) · [CSS Color 4 的 HSL 转换](https://www.w3.org/TR/css-color-4/#hsl-to-rgb) · [DrawingML 线性 RGB](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.rgbcolormodelpercentage) · [DrawingML 灰度](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.gray?view=openxml-3.0.1)

[二进制散列与测试计数](verification.json) · [样本生成与冻结版回放脚本](../../../../scripts/joydeck/check-stage2-shapes-theme.py)

复跑脚本的第一个参数使用新目录，`--frozen` 和 `--candidate` 指向对应的 CLI。PowerPoint 导出、真实桌面操作保存的样本、产品门禁日志和其它专项结果归档于本机 `backups/craft-stage2-deck-shapes-theme-20261125/`。macOS 和 UOS 本轮未执行真机回归。
