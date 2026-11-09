# JoyOffice v0.1.2 Windows P1 续验

2026-11-09，Windows x64，从 `50c5ff4d` 继续。使用测试副本和隔离配置。上一批的分栏、页脚添加／更新和 Excel 双向复制结果见 [Windows v0.1.2 验证](../20261109-v012/README.md#p1)。

| 任务 | 本轮证据 | 仍开放 |
| --- | --- | --- |
| JW-515 认证桌面控制 | 对活动文档请求字面查找 `English summary`，返回一处匹配。原生画布显示高亮，单页适配时状态栏显示 53%，页宽适配后显示 173%。正文、文档标识与修订号一致。TCP 回归覆盖缺失或错误密钥、过期版本、文件操作拒绝、无效查找输入和查找参数隔离。MCP 从同一桌面读回测试编辑。 | 通用界面操作、更广的命令与平台验收。 |
| JD-305、JD-602、JD-608 页脚与 PPTX 保留写回 | 从原生插入功能区取消清空，原页脚保留。再次确认后清空已有文字并保留占位框，Ctrl+Z 恢复，Ctrl+Y 重做。独立检查三页 DrawingML，页脚文字为空，各有一个页脚占位框，其他正文保留。发现并修复改动页遗漏形状树非可视组属性、把扩展放在对象前的问题。使用结构完整的三页语料重做，CLI 和 PowerPoint 都能直接重开。 | 已缺失结构的旧文件仍需修复。继承页脚、标题页可见性、复杂页脚、完整页眉／日期 UI 和完整交换矩阵继续开放。 |
| JD-307 Windows Office 富剪贴板 | 在 Microsoft Word 复制合成粗体、红字和斜体，JoyDeck 原生 Ctrl+V 在画布创建可编辑文本框，单步撤销和重做通过。另存的 PPTX 中独立确认 `b="1"`、`C42B1C` 和 `i="1"`。PowerPoint 直接打开三页，保留可见格式。 | 原位／表格的 Office 富内容、仅 RTF 的 Windows 提供方、内联图片、完整 Office 对象、Wayland 与物理 Linux 桌面。 |
| M1.4 加密工作簿 | 启动参数和文件选择器均显示掩码输入。取消后保留空白工作簿，错误值被拒绝，重试输入清空，正确值打开，原生另存为独立 XLSX。ZIP/XML 读回保留整数 `12500` 和小数 `3.5`。引擎回归覆盖相对别名、普通保存、PDF 导出及打印文件输出绕过、原件散列不变。恢复服务跳过尚无保存路径的解密工作簿。 | 安装关联、Explorer 拖放、多文件与其他平台验收。 |
| M4.4 Windows 多窗口 | M3 和 M4 桌面冒烟共 23 项通过，涵盖独立视图、后台草稿定向提交、按表恢复滚动、锁定文件的保存失败和重试。 | 真实多屏、混合 DPI、负坐标与休眠恢复。 |
| M6.4、W8 系统打印 | 从 JoySheet 生成一页预览并打开系统打印对话框。窗口控制仅捕获到部分打印框，取消回传和队列输出未确认。 | Microsoft Print to PDF、实体队列作业与纸面结果。 |

加密源文件 SHA-256 为 `a95ec421f7a985cd7fa8b87c4eec369a01819dd13fd5c245af39ae3802511985`，独立 XLSX 副本为 `cf2a426b9479e65f59af37efd26308ab7cc6407c87f7437983f00b70e358393e`。

初测的历史三页输入已有两页缺少 `p:nvGrpSpPr`，输入及其输出都触发 PowerPoint 修复提示。用 `tests/fixtures/joydeck/r1-preview.pptx` 加入三页普通页脚复测。结构完整的输入、清空输出、富文本输出的 SHA-256 分别为 `83edc31e42eb01f2eb01640f6bd080fb661087ab78ff30d46e95f23a5ac878c7`、`e2cea004477e499c23b1e75a5ebb89aa33f62f45e650c968194e4ed01650a3f7` 和 `330bb0615ff25c460f507430aba218726e554a913423ce377350e51aee6edbbb`。三页的非可视属性、组属性和页脚占位框均由独立 XML 读回确认。

## 自动验证

| Windows x64 检查 | 结果 |
| --- | --- |
| JoyWord 产品检查及严格 Clippy | 3,597 项通过，2 项既有忽略 |
| JoyDeck 产品检查、验证功能及严格 Clippy | 973 项通过 |
| JoySheet 产品检查、自动化功能及严格 Clippy | 1,398 项通过，1 项既有忽略 |
| 全工作区，启用 `joydeck/validation,joysheet/automation,joyword/agent` | 5,965 项通过，3 项既有忽略 |
| JoySheet `desktop-smoke.ps1 -Stage M3 -M4Task 4` | 23 项通过，使用生产 GPUI 窗口和应用内测试事件 |
| JoyWord 回退脚本单测与本机控制复验 | 13 项中 9 项通过，4 项按测试条件跳过。另启动生产桌面运行 TCP 和 MCP 实测 |
| 格式、文档、仓库门禁及 Deck 来源／依赖隔离 | 通过 |

未列入本轮验收的 P1 仍按各产品台账跟踪。可继续开发的范围包括 JoyWord 的格式交换、公共格式服务和性能预算，JoyDeck 的旧协议、旧格式和高级编辑能力，以及 JoySheet 的富表格互操作。设备或发行验收包括 Agent Provider、系统 IME／读屏、打印设备、安装关联和目标平台发布包。 [JoyWord 状态](../../../joyword/r8/STATUS.md) · [JoyDeck 任务](../../../joydeck/TASKS.md) · [JoySheet 迁移矩阵](../../../joysheet/MIGRATION.md)
