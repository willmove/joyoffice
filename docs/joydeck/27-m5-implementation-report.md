# M5 图片创作增量与前序回归（2026-10-30）

本轮交付为 **M5 图片创作开发预览**。完成 T-M5-01/02/03/06/08/09 的下述子范围；完整父任务、AT-08 和 M5 不标完成。M0–M4 原有开放条件仍保留，不能通过本轮回归替代完整收尾。

## 实现与格式

- `media.rs` 提供按内容嗅探的 PNG/JPEG 导入、EXIF 方向应用、解码前尺寸/像素/内存限制。原始图片转换为规范化 PNG；不保存 JPEG 元数据或原始编码字节。单文件/资源部件8 MiB、边4096、4M像素、解码分配32 MiB、资源总编码32 MiB。模型快照仍有8 MiB序列化预算，包含内存资源的JSON估算，因此复杂图片可能更早拒绝；不是宣称任意32 MiB文稿均可编辑。
- Deck 持有按 SHA-256 索引的共享资源；对象只引用 resource ID 与归一化 crop。容器部件 `resources/<sha256>.png`，manifest.resources 声明，hash/尺寸/缺失/重复验证。当前快照删除未引用资源，undo/redo 保有共享资源，历史计入估算；回收不破坏撤销或独立 transferable snapshot。
- writer 升级 schema1.3/minReader4/required image-resource-v1；旧1.0–1.2仍读写升级，旧reader拒绝新文件，避免丢图片。空图片文档也采用新writer身份。保留旧文件/源码/包作为回退，不提供降级转换。
- UI：独立图片路径、后台导入与替换、单文件拖放 adapter、系统图片粘贴；文档身份/revision/页变化时丢弃迟到结果。图片裁剪/重置为单次事务；非法数值整次拒绝。选中时同步裁剪值，既有平移/缩放/旋转/透明度可用。
- 普通文字及 PNG/JPEG 剪贴板 fallback；内部 v1 payload 包含对象、rich runs 和独立资源，可在源会话销毁后粘贴，重建对象ID、偏移16pt、单次undo。提供按钮及画布焦点 Cmd/Ctrl+C/V；活动文字继续使用原文字剪贴板处理。尚无外部 rich HTML/RTF。
- 六向对齐/横纵等间距分布按旋转后的视觉边界计算，分别要求≥2/≥3对象，单次事务；不足数目不修改文稿。
- CPU SVG 和 Scene 都消费同一 crop/旋转/opacity/image 数据。图片在 Scene 走**对象局部 raster fallback**，遵守当前缓存与可见场景预算；不是新建整页CPU栅格，也不是源图片直接上传路径。不能据此关闭 M4 原生 Image/Group/Hit 模型的全部集成。

## 验证与证据

`cargo xtask check`：fmt、all-targets Clippy、**50项测试通过**（原45项和5项媒体/对齐领域集成测试）。新覆盖：导入/去重/删除回收、非法crop回滚、undo/redo、源会话销毁后的复制粘贴、旧格式兼容、新格式资源往返、CPU/Scene像素、EXIF旋转、错误资源尺寸、恢复快照、旋转边界对齐及分布。

检查日志：[check](../logs/m5-check-full.log)、[stage M5](../logs/m5-stage.log)、[release打包](../logs/m5-package.log)。stage M5只执行链接与自动测试，不等同阶段验收。CI更新为M5，远端未执行。

调试构建的原生系统图片粘贴/内部对象粘贴去重/裁剪单次undo与redo/保存重开已通过。窗口像素复用M4既定阈值：meanRGB≤5、interiorRGB≤1、interior>16比例≤0.001、edge>128比例≤0.002。首轮分数像素高度造成crop 388行与reference 389行不匹配，并触发错误；将验证页面改为512pt高，使显示高度368整像素，复测通过，**未放宽阈值**。初次失败证据仍保留。该修正针对验证夹具；普通分数尺寸显示继续受采样误差影响，不据夹具覆盖所有DPI。

最终 [release包](../dist/joydeck-m5-preview-darwin-arm64.zip) 的M0–M4原生adapter回归全部通过；macOS系统拼音预编辑/提交/取消/一次撤销/重做/保存重开通过。M5系统图片/内部对象剪贴板、裁剪历史、保存重开与CPU/GPUI窗口对照均通过。最终GPUI meanRGB=0.27085、interiorRGB=0.03577、edge>128=0.000902；CPU meanRGB=0.07085、interiorRGB=0.00643、edge>128=0。详情及二进制/ZIP哈希见 [交付索引](evidence/m5/verification.json)、[窗口指标](evidence/m5/run-1793333271691298000/verification.json)、[真实窗口](evidence/m5/run-1793333271691298000/M5-GPUI-window.png)、[系统拼音](evidence/m3/pinyin-1793333277405576000/result.json)。[当前源码快照](../dist/joydeck-m5-preview-source.tar.gz)。未实测项目不登记通过。QA脚本暂时占用并尽量恢复原剪贴板；正常完成会恢复，异常中断不作保证。

## 开放事项及后续顺序

1. T-M5-01：增加格式/资源压力语料与三平台真实拖放、导入任务显式取消，优化资源验证重复解码及历史序列化成本。
2. T-M5-02：交互crop预览/取消、aspect/fit/fill、完整图片面板；本轮只交归一化crop、reset和既有变换。
3. T-M5-03/08：外部rich text、其他平台图片剪贴板、连续长期copy/delete/undo和全部缓存/资源压力验收。
4. T-M5-04/05：受限SVG/EMF/WMF fallback、freeform/Edit Points/布尔路径与高级预设仍未实现。
5. T-M5-06/07：嵌套组/组内选择、翻转/锁定、连接符glue/reroute/delete、名称/可见/阅读顺序的选择面板仍未实现。现有对象选择列表和图层按钮不算完整选择面板。
6. T-M5-09：完整“粘贴→裁剪→组→连接→对齐→保存重开”尚缺组/连接，AT-08父验收保持开放。
7. M3统一CPU/活动编辑布局、自动换行/段落列表/混合字号/bidi，以及M4真实device-lost/完整输入到present trace、tile/长会话、多窗口/跨屏/其余平台，以及M0–M2 LARGE/完整人工故障矩阵仍依照[23](23-m3-implementation-report.md)、[26](26-m4-implementation-report.md)开放。

本机目录无Git元数据，未创建提交。保留 [开发前源码](../dist/joydeck-before-m5-source.tar.gz)、M4 final包与新M5包；包未签名/公证，干净机器安装未验证。
