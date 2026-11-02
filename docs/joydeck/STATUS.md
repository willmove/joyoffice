# JoyDeck 实施状态

2026-11-02，当前为 R0 Windows 原生技术预览。应用和 CLI 均由当前 workspace 编译，使用 JoyOffice 固定的 Rust 1.98.1、GPUI Kit 0.8.1、GPUI pre 0.3.10。

## 已实现

- 独立 workspace、无 GUI 的 shell/CLI、GPUI 视图 crate 和产品工程工具。
- 按固定 JoyOffice 提交复用公共 UI 与纯工作区布局，保留许可证和文件来源。
- 原创三页样例、页面导航、缩放/适应窗口、明暗主题、属性面板和临时备注输入；未实现动作保持禁用。
- 统一构建、fmt/严格 Clippy/测试、文档/依赖/来源检查，以及不覆盖既有产物的开发包装。
- 实际 GPUI 来源校验与独立依赖兼容实验；三平台 CI 配置及隔离 JoyOffice 宿主编译演练。

## 实际验证

Windows x86_64/MSVC 下，产品检查、无界面检查、release 构建与 ZIP 打包通过。实际窗口覆盖 1280 和 980 像素宽布局、翻页、主题、缩放、备注展开、系统拼音候选提交及撤销、关于对话框和退出。release 包在仓库外的中文空格路径运行，CLI 诊断和错误参数返回码通过。

当前 [开发包记录](evidence/R0/package.json)、[仓库外启动记录](evidence/R0/standalone-launch.json)、[宿主编译记录](evidence/R0/host-check.json)与 [R0 验证说明](evidence/R0/README.md)可追溯实际产物和范围。完整构建日志与窗口截图保存在同目录。

macOS/Linux 原生构建、窗口、IME 及远端三平台 CI 尚未执行；CI 配置不能替代这些结果。R0 没有文稿编辑器，临时备注不写入文件；输入法结果只覆盖本次普通备注控件的 Windows 子场景，不代表 R3 原位编辑已完成。

## 下一阶段

接入 DeckCraft 核心时，先完成依赖顺序迁移、字体 fallback 脱离 epaint、包读取预算与 CPU 参考渲染，再接入真实文稿打开/取消、页缩略图及 CLI inspect/render。对应 JD-100–JD-108。保留当前 R0 包作为可运行检查点。
