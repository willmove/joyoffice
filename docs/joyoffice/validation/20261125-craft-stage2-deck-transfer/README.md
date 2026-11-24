# 阶段 2 · JoyDeck 媒体引用和跨文稿复制

本轮先提交撤销历史 `205fc433`，再替换媒体引用遍历和对象剪贴板的资源映射。兼容性对照沿用冻结版 `14cbb296` 的 Windows 桌面和 CLI，核对了两个二进制的冻结散列。

## 实现

- 媒体查找和重绑共用一份有类型的字段清单，覆盖组子对象、图片原图、母版与版式、主题填充、文本项目符号和链接声音等引用。对象粘贴与富文字资源落盘共用嵌入资源映射，按内容类型和精确字节匹配已有资源。资源检查和 ID 分配在写入前完成，重复 ID、资源缺失和分配失败均拒绝事务。
- 普通组可复制为同一棵对象树。组外框执行 18 点偏移，子树保留内部坐标，连接线跟随新对象 ID。跨文稿保留有效网页链接，去除源文稿的页内跳转。同一编辑会话的有效页内目标继续可用。
- 对象复制解析源幻灯片、版式和母版中的有效坐标、图片填充、颜色及字体。目标版式、母版与主题继续按其文件关系读取。PPTX 保留保存会重新绑定新增对象整棵子树的嵌入图片关系。[ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/) · [组与子对象坐标](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.transformgroup?view=openxml-3.0.1) · [版式颜色映射](https://learn.microsoft.com/en-us/office/open-xml/presentation/working-with-slide-layouts) · [嵌入图片关系](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.blip?view=openxml-3.0.1)

## 冻结对照与 Windows 实测

| 场景 | 结果 |
| --- | --- |
| CLI 普通图片重复粘贴 | 两版均插入两张图片，复用同一 PNG，分别通过撤销、重做、PPTX 保留写入和网页链接检查。原生文件两写两读共 4 组通过。 |
| CLI 组对象 | 冻结版拒绝，候选版保留 3 个子对象和连接线两端。组的撤销、重做通过，候选版写出的原生和 PPTX 文件各用两版读回，共 4 组通过。 |
| Windows 跨文稿 | 冻结版原生窗口拒绝复制该组。候选版用系统剪贴板粘贴整组，一次撤销移除整组，重做恢复组与选区。来源页内跳转被移除，图片与文字上的网页链接均保留。 |
| Windows 版式和母版 | 粘贴源占位符后保持版式坐标（外框按规则偏移）、图片填充及母版 Arial、`#A15721` 字色。原图、组内图与占位符填充均指向同一 PNG。 |
| Windows 保存重开 | 原生另存 `.joydeck` 和 `.pptx`，新进程重开均可看到组与占位符。两份文件各由两版 CLI 独立读回，共 4 组通过。PPTX 的目标版式、母版和主题原有部件逐字节一致，新增关系指向原 PNG。 |
| Windows 同文稿 | 同一来源窗口复制和粘贴组，原组与新组的页内链接指向同一有效页面，文件仍复用一张 PNG。另存原生文稿后两版 CLI 读回通过。 |

`cargo xtask check joydeck` 通过 **1,042 项测试**，包括严格 Clippy、格式、来源和依赖检查。Windows 桌面与 CLI 构建通过。R2 回归在冻结版和候选版各通过 20 条命令，覆盖保留写入、覆盖拒绝和崩溃恢复。[结果与散列](transfer-verification.json) · [CLI 对照脚本](../../../../scripts/joydeck/check-stage2-transfer.py)

验证范围覆盖受支持的对象与普通组。整页跨文稿、带未知扩展或 SmartArt 的组，以及其他平台的原生剪贴板仍待各自的验证。本轮复杂对象拒绝路径保留，形状、富文本和主题模型的其余替换继续按阶段 2 跟进。
