# JoyDeck 阶段 4 合成、图元与输出验收

以冻结的 Windows CLI `14cbb296` 及其两种尺寸的转场像素清单为参照。候选版从 `018f30a5` 继续。验证覆盖 CLI、PDF 独立读回和 Windows 原生打印回调。[测试明细](verification.json) · [排版与放映基线](../20261126-craft-stage4-deck-runtime/README.md)

- CPU 路径通过同一预乘混合内核绘制纹理、实色和带透明度的三角形。匹配原像素采样点与半开裁剪区间。64×36 和 193×109 共 1,850 帧的 CPU 清单与冻结版完全一致，Intel Arc Vulkan 与 CPU 最大通道差 1，首尾帧完全一致。既有 48 个选项完成 192 组文件互读。[合成](../../../../crates/joydeck-render/src/transition.rs)
- WMF 的字长与 EMF 的字节长度分别校验，EMF+ 的注释记录独立限制负载。绘制按放置顺序回放路径、纹样、位图与文字，各次绘制按记录的区域裁剪。两份冻结 PPTX 的 WMF／EMF 画布与 PDF 栅格逐像素相等。产品测试覆盖路径、裁剪、文字、位图、WMF 映射与 EMF+ 记录。[记录与画布](../../../../crates/joydeck-metafile/src/records.rs) · [WMF 规范](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmf/33c95db8-eff2-44d3-b2ce-23d0ebdca5c2) · [EMF 规范](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emf/) · [EMF+ 规范](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/06811d6d-2727-42e2-b45e-6adecbeb0c14)
- PDF 共比较 7 种输出和 14 页。独立读回页框、混排文字、嵌入字体流、链接和书签，Poppler 复核光栅。重复选页时，三页和六页讲义的首张页脚由 2 修正为 1，像素差限于页脚。其余页面内容与冻结版一致。[PDF 实现](../../../../crates/joydeck-pdf/src/lib.rs) · [复跑脚本](../../../../scripts/joydeck/check-stage4-output.py)
- 打印快照按显式选页顺序创建，全部页面就绪后发布清单。Windows 上用生产适配器的同一分页回调和 Microsoft Print to PDF 输出本地测试副本，独立读回 3 张纸及 3、1、2 的页面内容顺序。自动化启动时未能定位独立的打印对话框，虚拟打印验证直接使用系统驱动。[打印适配器](../../../../crates/joydeck-shell/native/print-windows.cs) · [GDI 分页](https://learn.microsoft.com/en-us/windows/win32/printdocs/gdi-print-api-functions)

剩余的阶段 4 开发为功能区呈现，估 2–4 人周。macOS Metal、UOS Vulkan 的同配置实机复验留在平台待验项。[替换计划](../../CRAFT-REPLACEMENT-PLAN.md)
