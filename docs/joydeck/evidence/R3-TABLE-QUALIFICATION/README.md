# JD-306 表格桌面验收

2026-11-05，Apple M4／macOS 27.2，1× 实屏。沿用同一批仓库外解压的 ARM64 Release 验收包，归档 `joydeck-0.8.0-r3-text-fields.zip` 的 1,805 项载荷散列一致。验证采用真实系统键鼠、原生拼音、文件面板与独立桌面进程。[包指纹与产品检查](../R3-TEXT-FIELDS/verification.json) · [原创两页样例](tables.pptx) · [实测摘要](verification.json)

| 场景 | 实测结果 |
| --- | --- |
| 插入与行列 | 在第二张空白页从 Ribbon 插入 3×3 表格，零列输入时文稿保持干净。随后插入并删除一行、一列，逐步核对网格尺寸和修订，每个动作只提交一次。[无效尺寸](01-invalid-table-size.png) · [行列编辑](02-row-column-history.png) |
| 选区与合并 | Shift+点击将首行相邻两格纳入矩形选区。「合并选区」只合并这两格。模型核对跨列宽度和覆盖格，第三格仍独立。[矩形选区](03-pointer-range.png) · [合并结果](04-merged.png) |
| 原生输入和导航 | 系统简体拼音在合并格中显示候选，候选期间模型返回忙态，确认「你」只提交一次。Tab 跳到首行第三格，Shift+Tab 回到合并锚格。再写入字母，模型核对锚格为 `你a`，第三格为 `b`。[系统候选](05-native-pinyin-candidate.png) · [键盘结果](06-merged-cell-navigation.png) |
| 拆分、撤销与格式 | 拆分只解除合并。撤销恢复跨度、重做解除，原有输入保留。给第二行相邻两格设置浅黄色底色，再应用 Light Style 1 的主题色样式。第三格保持独立底色。[样式与底色](07-style-and-fill.png) |
| 另存与重开 | 通过原生另存面板分别保存 PPTX 和 `.joydeck`，用包内 CLI 和独立桌面进程重开两份。python-pptx 核对公开 PPTX 的文字和两处 `FFF2CC` 底色。第一张原有表格所在 XML 及项目内嵌的原始包逐字节保持。[PPTX](08-reopened-pptx.png) · [JoyDeck](09-reopened-native.png) |

相关实现见 [表格 Ribbon](../../../../crates/joydeck-ui-gpui/src/table_ui.rs)、[单元格导航](../../../../crates/joydeck-ui-gpui/src/inline_text.rs) 和 [事务边界回归](../../../../crates/joydeck-engine/tests/r9_formatting.rs)。同一包的产品检查 919 项通过，R3 CLI 复验覆盖表格 TSV、结构、底色、未知属性保留及不安全结构写回拒绝。旧 Windows 基础表格实测记录仍可单独复核。[Windows R3 基线](../R3/README.md)

命令矩阵只登记本次实测入口。表格绘制／擦除、完整样式选项、其他表格命令和跨平台对照继续按 JD-508、JD-509 与 R8 追踪。
