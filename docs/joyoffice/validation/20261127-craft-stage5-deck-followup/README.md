# JoyDeck 阶段 5 打印补验、保留另存与图元替换

2026-11-27，Windows 11 x64。继续使用阶段 0 冻结版 `14cbb296` 和上轮阶段 5 审计。运行数据记录了两版 CLI 的 SHA-256。[上轮审计](../20261127-craft-stage5-deck/README.md) · [阶段 0 冻结指纹](../20261124-craft-pilots/verification.json) · [本轮结果](verification.json)

## Windows 打印实际写盘

用上轮验收包的 CLI 打印三页测试演示文稿。本轮打印助手源码与上轮相同。系统打印选择窗选用 Microsoft Print to PDF，用户在该驱动的真实保存窗口输入中文文件名并保存到隔离目录。CLI 返回 `submitted`，实际文件为 151,293 字节。pypdf 独立读回 3 张 A4 横向页面，Poppler 以 48 DPI 渲染复核原页序 1、2、3。逐页与上轮虚拟驱动参考相比，匹配页的像素差均为零。

## 生产切换

- **PPTX 保留另存。** 旧关系索引及目标路径解析从 DeckCraft 来源的 `opc.rs` 移出。独立关系模块处理原始目标、内部成员、外部 URL 和重新定位。复制到新目录时，克隆幻灯片、备注和生成资源的关系保留 URI 片段，字面空格、百分号和井号按目标路径编码。既有 Office 输入的反斜线和冗余路径读法继续兼容。整包放入独立回归，校验未改源部件、未知部件、外部 URL 和重开后的文稿。[关系模块](../../../../crates/joydeck-pptx/src/relationships.rs) · [保留另存](../../../../crates/joydeck-pptx/src/preserve.rs) · [ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/) · [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986)
- **WMF／EMF 位图。** 移除 Craft 派生的 `dib.rs`，WMF 与 EMF 生产入口使用独立的位图头解析、索引像素、通道掩码、逐行解码、源矩形裁切、镜像和混合前处理。EMF+ 的裁切和镜像亦共用该实现。保留旧位图预算、上下行方向、行填充以及低位深调色板回退。RGB565 回归核对三个像素宽的带填充行和负高度。历史指纹继续留在来源账本。[位图模块](../../../../crates/joydeck-metafile/src/bitmap.rs) · [来源账本](../../../../licenses/deckcraft/file-map.json) · [BITMAPINFOHEADER](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-bitmapinfoheader) · [MS-EMFPLUS](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/06811d6d-2727-42e2-b45e-6adecbeb0c14)

按候选构建的 Rust dep-info 复核，带 DeckCraft 来源记录且参与产品编译的文件现为桌面 118 个、CLI 117 个。另有一个共享构建脚本，来源账本保留 9 个已退役路径。

## 许可原件追溯

- `mac@0.1.1` 的锁定发布包和上游 0.1.1 标签对应树均无独立许可正文。继续随包保留发布声明、README 与明确标注的标准 Apache-2.0 文本。[上游固定文件树](https://api.github.com/repos/reem/rust-mac/git/trees/66afc663b68a65633ea165c742b5a9c6734581c2?recursive=1) · [产品许可记录](../../../joysheet/third-party/mac/SOURCE.json)
- `seahash@4.1.0` 的精确发布提交已在 Software Heritage 找到。归档树的 README 和原始 Cargo.toml 散列与锁定发布包相同。发布版本的根目录无独立许可正文。归档证据已补入记录，原发行声明及标准 MIT 文本继续随包。[精确发布提交](https://archive.softwareheritage.org/api/1/revision/94b632aeac099031c373599313d5b5f0acbbaec0/) · [产品许可记录](../../../joysheet/third-party/seahash/SOURCE.json)
- `pathfinder_geometry@0.5.1` 的 10 个原始发布文件与上游 `1cd5966d` 的 `geometry/` 逐字节匹配。该提交仓库根 MIT 与 Apache 正文的 Git blob 与现存许可副本相同。已将来源链接指向这套匹配内容的树。发布包中记录的 `a5e98fac` 仍无法从 GitHub 和 Software Heritage 取到，继续记录该提交标识上的限制。[匹配的上游代码与许可](https://github.com/servo/pathfinder/tree/1cd5966d3d7c3d395b6886d1ed63eb7b306f9be3) · [产品许可记录](../../../joysheet/third-party/pathfinder_geometry/0.5.1/SOURCE.json)

Windows x64 与 Linux x64 的源码依赖图重新生成 SPDX，分别核对 491 和 575 项。两项标准文本与发布声明的记录散列通过，Linux 包含的 Pathfinder 两份正文也通过 SHA-256 和 Git blob 复核。

## 冻结回归与 3D 回退

- `cargo xtask build joydeck`、`cargo xtask check joydeck` 和工程门禁通过。严格 Clippy、fmt 和 1,076 项 Rust 测试通过，R1–R7 共 126 项 CLI 检查通过。共享许可记录也通过 JoySheet 的 10 项打包测试。
- 原生与 PPTX 交换复跑 17 个文稿、100 份写出文件和 234 次模型读回。既定饼图字段差异继续单独核对。签名／宏保留写回与 7 类不支持输入持续按原契约拒绝。
- 7 组 PDF、14 页通过。两处已记录的讲义页脚修正保持。WMF 和 EMF 场景以及两份图元 PDF 光栅与冻结版匹配。
- PowerPoint 16 生成的 3D 文稿先由候选 CLI 保留另存，再由冻结 CLI 保留另存。两次写出的 44 个部件逐字节相同，候选版重开继续读取 GLB 和海报。两个副本均在 PowerPoint 16.0.20625.20108 保存并重新打开，仍识别为 3D 对象。冻结版明确重新生成仅保留海报。候选版的复杂 3D 幻灯片复制保持既有拒绝契约，拒绝时未产生文件。[3D 复跑脚本](verify-3d.py)

## 剩余工作

PPTX 通用 XML 与生成关系写出、图表和占位符的保留分支仍有来源记录。WMF／EMF 的向量记录解释、设备坐标、裁剪几何、文字和 EMF+ 图像解包仍待继续替换。其余模型、布局、放映及绘制分支沿用阶段 5 审计。两项独立许可正文与 Pathfinder 发布 VCS 的精确提交仍待可取得的原件。macOS Metal 和 UOS Vulkan 实机验收继续按设备可用性安排。[剩余路径](../20261127-craft-stage5-deck/README.md)

本机原始输出位于 `backups/craft-stage5-deck-followup-20261127/`。3D 复跑使用冻结 CLI、候选 CLI、上轮 PowerPoint 16 样本和一个新的输出目录。

```powershell
$report = 'docs/joyoffice/validation/20261127-craft-stage5-deck-followup'
$frozen = 'backups/craft-replacement-20261124/14cbb296/joydeck-cli.exe'
$source = 'backups/craft-stage5-deck-20261127/office16-other/01-07-powerpoint16-3d-powerpoint-powerpoint.pptx'
py -3 "$report/verify-3d.py" $frozen target/debug/joydeck-cli.exe $source target/deck-3d-recheck
```
