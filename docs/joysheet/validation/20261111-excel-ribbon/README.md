# 2026-11-11 Windows Excel Ribbon 比对验证

本页保留第一轮的截图和验收快照。第二轮对象、分页、追踪箭头与细线调整见[后续记录](../../STRUCTURED-OBJECTS-AND-PAGING.md)，下方“仍禁用”等描述仅指第一轮二进制。

范围和未实现项见 [Ribbon 比对记录](../../EXCEL-RIBBON-PARITY.md)。本机参照为中文 Excel 16.0.20527.20150；源码分支 `feat/joysheet-excel-ribbon`，产品版本保持 0.1.3。本次未提交、推送或打包发布。

## 自动验证

- `pwsh -File scripts/joysheet/dev.ps1 xtask check joysheet --validation`：fmt、严格 Clippy、产品所属包及实际公共依赖测试通过，**1,401 项通过、1 项忽略**。
- `pwsh -File scripts/joysheet/dev.ps1 xtask build joysheet --validation`：Windows 开发验证二进制构建通过。
- `python scripts/joysheet/ribbon-smoke.py --output <new-directory>`：**20 组检查通过**，详见 [原生报告](native-report.json)。事件由生产 GPUI 路径处理，截图来自原生 GPU 读回，配置和工作簿隔离。不是物理键鼠、真实 IME 或读屏验收。
- 回归包括十个编辑器选项卡、文件后台与 Esc、加粗/格式刷、功能区操作后的 Ctrl+Z/Ctrl+Y、菜单、Ctrl+F1 折叠、分类搜索后插入 SUM 草稿、隐藏标题后的 B2 命中与编辑、墨迹绘制/擦除/撤消/重做，以及磁盘 XLSX 独立重开。重开使用磁盘文件字节创建新文档，避免“已打开同路径”只激活现有文档造成假阳性。
- 最终文字换行和焦点修正后，另行通过 UI crate 的 17 项测试和严格 Clippy。额外单测覆盖粘贴值与粘贴格式的不同语义和撤消、页面设置、符号插入、数据表单转换，以及已启用 Ribbon 叶命令的注册和参数契约。

## 原生界面比对

直接操作本机 Excel 和最终 JoySheet 窗口，将两者置于相同的 2048 像素逻辑窗口宽度，比较空白工作簿和相同选项卡。另保留 1440 宽窗口的原生回归截图，检查窄窗口布局与滚动。下列图片仅裁剪掉工作区和账户标题，不改变 Ribbon 的比例或内容。

| 选项卡 | 对照截图 |
| --- | --- |
| 开始 | [Excel / JoySheet](home.jpg) |
| 插入 | [Excel / JoySheet](insert.jpg) |
| 绘图 | [Excel / JoySheet](draw.jpg) |
| 页面布局 | [Excel / JoySheet](page-layout.jpg) |
| 公式 | [Excel / JoySheet](formulas.jpg) |
| 数据 | [Excel / JoySheet](data.jpg) |
| 审阅 | [Excel / JoySheet](review.jpg) |
| 视图 | [Excel / JoySheet](view.jpg) |
| 帮助 | [Excel / JoySheet](help.jpg) |

![开始选项卡对照](home.jpg)

宽屏开始页增加两行样式图库；窄屏使用下拉菜单。实际点击图库“好”并读取单元格样式、撤消复原（[图库报告](gallery-report.json)）；其余核心交互由上述原生回归覆盖。全部截图已作视觉检查，未据此宣称像素级一致或完整可访问性合规。

## 保留的限制

SmartArt、WordArt、日程表、嵌入对象和独立公式对象仍禁用。Excel 在线功能、全部图库/弹窗、分页编辑/标尺、公式追踪箭头、独立滚动拆分及持久对象组合等差异仍开放。图标采用本产品已有的 Lucide 资源，不是 Excel 图标。真实 IME、读屏、多显示器/物理 DPI、设备打印、其他平台和发行包均不在本轮新验收范围。

完整原始截图、可访问树、原生回归失败轮次及最终检查日志保留在本机忽略目录 `artifacts/joysheet/excel-ribbon-20261111/`，原生回归的各轮目录分别保留。Excel 文件后台含最近文件的原始截图未提交。
