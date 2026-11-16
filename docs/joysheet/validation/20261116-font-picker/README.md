# 2026-11-16 开始选项卡字体组合框

本轮落实用户截图中的字体控件需求，范围限于“开始”中的字体名称输入、字体列表和最近字体。沿用高级 Ribbon 续验的实现，不将本轮结果扩大为所有 Excel 控件完全一致。

## 实现

- 字体名称改为真实输入框，支持中英文名称。Enter 应用到进入控件时的选区，Escape 取消；不会把字体名称输入到单元格。保留已有字号、加粗等样式和单元格值，支持撤销/重做。
- 输入名称时筛选展开的列表；修剪首尾空格并统一已安装字体名称的大小写。允许保存未安装字体名称，供其他机器使用；本机显示由已有字体回退机制处理。
- 列表右侧显示滚动条，可拖拽滑块浏览本机完整字体目录；不再截断到前 2,048 项。字体条目改为紧凑、左对齐排列，下拉箭头位于输入框右端。
- 顶部按“主题字体 → 最近使用的字体 → 所有字体”排列。主题项读取当前工作簿的标题/正文字体；最近字体去重并最多保留 8 项，重启后恢复。只有格式命令成功后才更新历史，受保护工作表拒绝格式修改不会留下错误记录。
- 格式应用前校验工作簿和工作表仍与进入控件时一致，避免将暂存输入错误应用到切换后的工作簿。

## 验证

最终测试构建 SHA256：`e7ff86d69718e239e0d9ff40c059cca551811817a1585f124b34f1d1d2b2b16c`。

- 完整 Sheet 产品检查：fmt、严格 Clippy、**1,435 项测试通过，1 项既有忽略**。本机日志：`artifacts/joysheet/font-picker-check-final-20261116.log`。
- 字体生产 GPUI 指针/键盘回归 **6 组通过**：名称输入/规范化、选区与混合样式保留、撤销重做、中文名称、Escape 取消、列表筛选、主题字体选择、滑块拖拽并选择后段字体、保护拒绝及历史重启恢复。见 [font-regression.json](font-regression.json)。
- 原有高级图库/向导 **11 组**、对话框 **7 组**、Ribbon/对象/分页 **35 组**回归使用同一构建，结果分别见 [advanced-regression.json](advanced-regression.json)、[dialog-regression.json](dialog-regression.json)、[ribbon-regression.json](ribbon-regression.json)。

Windows 原生鼠标/键盘检查与上述生产 GPUI 合成输入分开记录：实际聚焦字体名称、输入 Arial 并按 Enter；点击右端箭头确认顶部字体分组和滚动条；拖动滑块显示 W/Y 及中文字体；点击 Yu Gothic 后，下拉框关闭且字体名称更新。另用只读引擎查询确认 A1 的实际字体为 Yu Gothic。见 [native-regression.json](native-regression.json)。

| 原生窗口证据 | 截图 |
| --- | --- |
| 输入 Arial 时筛选列表 | [输入与筛选](typed-filter.png) |
| 主题/最近字体位于顶部，右侧滑块可见 | [分组与滚动条](theme-recent-scrollbar.png) |
| 原生鼠标拖动滑块后的列表后段 | [拖拽结果](dragged-later-fonts.png) |
| 点击后段字体，输入框显示 Yu Gothic | [选择结果](selected-later-font.png) |

本轮未验证真实中文 IME 候选窗、其他平台、多显示器和不同物理 DPI。字体列表使用统一界面字体显示名称，未实现 Excel 逐字体字形预览；主题字体选择保存当前名称，不新增随未来主题变化自动重映射的字体 scheme 语义。

## 复跑

```powershell
pwsh -File scripts/joysheet/dev.ps1 xtask check joysheet --validation
pwsh -File scripts/joysheet/dev.ps1 xtask build joysheet --validation
python scripts/joysheet/font-picker-smoke.py --output artifacts/joysheet/font-picker-fresh
python scripts/check_docs.py
```

自动检查使用一次性配置目录和工作簿，不修改用户原始文件或日常设置。
