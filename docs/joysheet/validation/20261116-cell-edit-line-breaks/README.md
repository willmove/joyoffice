# 单元格编辑保留换行

2026-11-16，分支 `fix/joysheet-preserve-cell-line-breaks`，起点 `14da6f22c1b1605382a776755437cda78ede2f15`。

## 问题与修复

原单元格编辑器和公式栏使用 GPUI Kit 的单行 `InputState`。双击或 F2 载入含 LF/CRLF 的文本时，控件过滤换行，草稿观察器把过滤结果当成实际编辑；随后确认会把删掉换行的文本写回。修复前的本机程序已复现：`首行😀\n第二行` 进入编辑后变成 `首行😀第二行`。

两个入口改用 `TextareaState` / `Textarea`，同步原始文本并保持原有末尾光标位置。拦截多行控件的 Enter、Tab 和反向导航动作，继续使用工作表提交、导航和公式补全逻辑。编辑内容与原文一致时，沿用不提交事务的路径。变更局限于 Sheet UI，不修改工作簿模型、格式、导入导出或公共依赖。

## 本机自动验证

- `cargo xtask build joysheet --validation` 通过。
- `cargo xtask check joysheet --validation` 通过：产品及实际使用的公共库 fmt、所有 target 严格 Clippy，44 组 Rust 测试套件共 **1,427 通过、0 失败、1 项既有忽略**。
- `python3 scripts/joysheet/smoke-native.py --binary target/debug/joysheet --output target/joysheet-native/cell-lines-acceptance-20261116` 通过 **13 组桌面检查**，保留原有 Unicode、公式补全、剪贴板、组合态保护和生命周期回归。
- 新增 `scripts/joysheet/cell_edit_cases.py`，由上述原生冒烟入口持续执行。合成 XLSX 包含 LF、CRLF、空行和纯自动换行四种文本，带自动换行、字体、填充、垂直对齐和文本数字格式。
- 检查双击后原文、Enter/Tab/Shift 导航、不改内容时 revision/dirty 不变、F2/Escape 取消、公式栏确认、两个编辑入口追加中文/emoji、撤销重做及完整样式一致。保存后独立解析 OOXML（包含 `_x000D_` 文本解码），关闭并重新导入，再比较文本和样式。
- `python3 -m py_compile scripts/joysheet/cell_edit_cases.py scripts/joysheet/smoke-native.py` 和 `python3 scripts/check_docs.py` 通过。

本机运行于 Linux x86_64 / UOS 桌面 X11，缩放 125%。隔离 profile 和测试进程，应用结束后关闭测试窗口。本会话终端使用宿主的编译器、系统库和截图工具；没有更改仓库依赖或构建配置。

二进制、输入/输出 XLSX、截图及日志哈希见 [验证摘要](summary.json)。完整运行产物留在本机 `target/joysheet-native/cell-lines-acceptance-20261116/`，Rust 日志留在 `/tmp/joysheet-cell-lines-check-complete.log`。截图来自 X11 实际窗口像素；未标记为 Linux 不支持的 GPUI GPU 读回。

## 验证边界

已查看本机编辑态和保存重开后的窗口截图，确认多行及空行显示。桌面事件来自生产 GPUI 合成输入；本轮没有人工物理键鼠、系统 IME 或读屏验收，没有 Windows/macOS 实测，也没有实际 Excel 应用对照或用户原始工作簿。合成 XLSX 自动验证不替代这些事项，不发布新版本或关闭既有平台验收项。
