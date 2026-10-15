# C06：统一页面 CPU 光栅与真实文档画布

- 执行者：当前 Codex 协调者，单人执行。
- 基线：2b29c84d6c51a8ecfffe14418d8b9325d7bb5dcc（C05）。
- 分支：codex/coordination-cpu-canvas。
- 独立目录：/Users/willm/.codex/worktrees/joyoffice-coordination/joyoffice。
- 对应：T003／T005／T105，LAYOUT-001／LAYOUT-009；为 T102 画布输入提供呈现基础。

## 交付

实现规格要求的 PageScene CPU 光栅消费者，以现有实际 FontKey 和 glyph id／位置读取同一字体字节，不再整形或断行；缩放／DPI 仅变换投影，保留统一页面几何。字体轮廓使用与 HarfRust／PDF 共用 read-fonts 的 Skrifa，通用光栅器用 tiny-skia。字体、像素、字形、轮廓与工作量有硬预算，错误／取消不返回半幅成功图，未知字体／字形及不支持的字体格式明确拒绝。

将真实 DocumentSnapshot→LayoutSnapshot→CPU 页面接到正式 GPUI 窗口，后台执行 IO／排版／光栅，结果绑定版本和投影请求，取消或陈旧结果丢弃；空闲不重复计算。页面图像按实际设备倍率生成，菜单／快捷键／语言切换保持同一注册表。明确只读预览与缺字体／错误状态，不宣称尚未实现的输入保存。完整输入、四槽字体和 Word profile 仍是原任务目标，不因本切片缩减。

## 边界与依赖裁决

协调者批准新增无界面 joyword-render crate、app 的场景桥接与既有资源文案，测试／示例和公共实施、CI、来源记录。模型／engine／OPC／字体／整形／PageScene 现有接口不变，不改 W02 实验及其 vendor。根 workspace 可直接使用锁文件已有精确 skrifa 0.42.1、tiny-skia 0.11.4、image 0.25.10，不增加或升级外部包；GPUI 仍为固定 0.2.2，不将其 Vulkan 窗口当作完整无 GPU 退路或 ADR-010 裁决。

## 验证

真实字体按场景绘制／位置平移与缩放、行页几何、像素通道、空轮廓／缺字、字体身份／collection face、无 GPU 渲染和 PNG 解码；预算／取消／非法输入／失败原子性；原场景和字体不修改。多页原创中英样例与独立 PDF 渲染作位置对照，并实际查看图像。新旧投影结果竞争、空闲不重复调度、字体缺失及状态恢复自动测试；相关／完整默认测试、release、fmt、clippy、文档／工具检查、应用构建、交叉检查与绑定提交的远程 CI。GUI 已由用户暂缓，不再使用桌面控制或把编译当人工验收。
