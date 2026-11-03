# JoyDeck R0 原生技术预览

启动 `joydeck.exe`（Windows）或 `joydeck`（Linux/macOS）；macOS 亦可打开 JoyDeck.app。

本版本提供原创示例的幻灯片浏览、缩放、明暗主题、属性面板和临时备注输入。文件导入、内容编辑、保存、PPTX/PDF 尚在迁移；相关按钮保持禁用。备注仅属于当前窗口，退出即清除。

- 用缩略图、上一页/下一页或 PageUp/PageDown 切换页面。
- 状态栏调整缩放；Ctrl+0 恢复适应窗口。
- 点击“深色/浅色”切换主题，或 Ctrl+Shift+T。
- 点击“备注”展开输入，支持系统文字输入。
- `joydeck-cli diagnostics` 输出版本与当前能力；`joydeck-cli sample` 检查示例内容。

这是未签名的本机开发包。`source.json` 记录构建源码指纹，`sbom.json` 记录本目标实际 normal/build 依赖，`SHA256SUMS` 校验包内容。系统字体不随包分发；没有可用字体/图形驱动的机器需要先配置其原生环境。
