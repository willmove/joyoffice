# 三产品共享界面主题验证

2026-11-11，macOS ARM64。起点为 main `4c7a951b73e62fcc3b71bbb7acc83abc99efa291`，任务分支 `feat/shared-theme-presets`。[主题分析、名称和兼容规则](../../THEMES.md)

## 实现与本机界面

Word、Deck、Sheet 均提供青岚、素雅、晴空、暖砂的亮色与暗色，共八个预设。三个应用使用同一主题目录与 Palette，偏好继续由产品分别保存。文档模型、内容颜色、格式文件和导出不受界面选择影响。

本轮通过当前 Debug 构建及隔离配置实际打开三个 GPUI 应用，确认预览、名称和选中状态可见，点击后立即切换，退出进程并重启后恢复所选预设。设置页按亮暗配对排列；辅助功能树能读出当前预设的 on/off 状态。Deck 从暖砂亮色窗口新建另一个原生窗口后，保留完整暖砂风格，明暗快捷按钮切到同风格暗色。二进制指纹见 [binaries.json](binaries.json)。

| 实屏 | 观察结果 |
| --- | --- |
| [Word 晴空暗色](gui/word-azure-dark.png) | 选项页和导航栏使用深蓝配色，八项预览可见；重启恢复晴空暗色 |
| [Sheet 暖砂暗色](gui/sheet-sandstone-dark.png) | 设置、功能区、行列标题、工作表标签和滚动条使用暖色配色；单元格保持文档原色 |
| [Deck 晴空暗色](gui/deck-azure-dark.png) | 文件选项及导航栏跟随深蓝配色，选中状态可见；重启恢复晴空暗色 |
| [Deck 暖砂亮色](gui/deck-sandstone-light.png) | 文件页、按钮和预览的亮色切换生效 |
| [Deck 新窗口暖砂暗色](gui/deck-new-window-sandstone-dark.png) | 新建窗口继承完整预设，快捷切换保留暖砂风格；幻灯片继续显示原色白底 |

## 自动验证

- Sheet 产品工程检查：1,404 通过、0 失败、1 个原有计时测试忽略；包含 fmt、严格 all-targets Clippy。[日志](joysheet-check-final.log)
- Deck 产品工程检查：975 通过、0 失败；包含 fmt、严格 all-targets Clippy、291 项来源指纹、依赖及文档门禁。[日志](joydeck-check.log)
- Word 主题及直接使用方：129 通过、0 失败，覆盖共享库、shell 和 GPUI 适配器。新增保存失败回滚、旧暗色配置恢复、精确预设保存与同风格明暗切换回归通过。[日志](word-theme-tests.log)
- 三产品联合生产库／二进制严格 Clippy 通过，采用产品工程入口对 Word 规定的 `--lib --bins` 范围，带 `joydeck/validation,joysheet/automation`。[日志](joint-clippy-production.log)
- Sheet 生产原生控制接口专项：八个预设、非法 ID／类型和冲突模式的原子拒绝、旧 dark 切换保留风格、保存和重启、同工作簿两个原生窗口的完整主题同步通过；切换前后文档 ID、修订和脏状态一致。[结果](sheet-theme-control.json)
- 全部预设 × 三种产品的文字、次要文字及按钮配色对比度回归通过；没有新增 registry 依赖或升级既有依赖。

联合单元及集成测试 **6,001 通过、0 失败、3 项既有测试忽略**，[完整测试段](joint-unit-integration.log)。没有排除语言规则测试，该测试在本次完整复跑中通过。联合文档测试已在停止并行构建后单独重跑并通过，[日志](joint-doc-test.log)。首次联合命令在文档阶段遇到共享构建缓存的 E0463 依赖产物错误，不能把首次命令本身记为成功；上述单元／集成结果与单独文档重跑分别记录。

三个产品均已通过自己的 `cargo xtask build` 普通构建（含所属 CLI）：[Word](build-word-production.log)、[Deck](build-deck-production.log)、[Sheet](build-sheet-production.log)。[结构化检查汇总](checks.json) · [源码输入指纹](source-inputs.json)

## 发现的既有问题与验收边界

Sheet 的完整 `smoke-native.py` 在主题分支与未修改 main 的独立 worktree 上，均于公式剪贴板场景得到相同失败（[main 原始结果](sheet-smoke-main.json)、[本分支原始结果](sheet-smoke-branch.json)）：先复制 `B1:C1`，把目标 B6 改为 7 后，C6 保持 40，脚本预期为 14。原生窗口、Unicode／emoji、重算、撤销重做与合成组合态等前置场景通过。本轮不把该全流程记为通过，也不在主题任务中修改剪贴板语义。

Word [首轮完整产品测试](word-check-initial.log)在 `swiss_german_and_portugal_portuguese_keep_their_languages_rules` 失败；同一测试单独运行和未修改 main 的 1,166 项 engine 回归均通过。保留初次失败，最终联合复跑结果单独列出，不通过放宽断言或修改该测试绕过。

额外直接执行全 workspace 的 all-targets Clippy 会命中 Word 既有测试夹具的 unwrap／panic 限制；现行产品工程入口已明确 Word 只 lint 生产库与二进制，Sheet／Deck 仍 lint 全部 targets。本轮没有降低 lint 级别或修改测试夹具。

本轮是 macOS 1× 原生窗口与程序控制验证。完整键盘遍历时未观察到预期的 Tab 焦点移动，不能据控件的键盘事件处理代码宣称其已通过实机键盘验收；读屏语音、全部 DPI、Windows／Linux 实屏及完整输入法矩阵仍需后续补验。原生控制中的草稿创建／取消有结果检查，内联编辑绘制和物理输入仍保留人工验收边界。
