# JoyDeck v0.1.0 · First Preview / 首个预览版

## English

This is the first public JoyDeck preview in the JoyOffice monorepo. Each package contains the desktop presentation application and the `joydeck-cli` command-line tool.

### Highlights

- Create and edit slides, text, images, tables, charts and other supported objects; native documents retain stable object identities and editing history supports undo and redo.
- Compare native presentations and accept individual object changes. An explicit option can copy required PNG image resources as part of the same undoable transaction.
- Use the registered editing commands through the automation interface, including validation without changing the document and optional revision checks.
- Export supported formats. On macOS, the preview also supports static-slide MP4 and title-only WebVTT; video does not include sound, transitions, animations, or recordings.

### Downloads

- macOS Apple Silicon: `joydeck-0.1.0-macos-aarch64-release.zip` (`JoyDeck.app` and CLI)
- Windows x86_64: `joydeck-0.1.0-windows-x86_64-release.zip` (desktop executable and CLI)
- Linux x86_64: `joydeck-0.1.0-linux-x86_64-release.zip` (desktop executable and CLI)
- Linux ARM64: `joydeck-0.1.0-linux-aarch64-release.zip` (desktop executable and CLI)

Each archive has a separate `.sha256` file. For example, run `shasum -a 256 -c joydeck-0.1.0-macos-aarch64-release.zip.sha256` from the download directory. These checksums detect accidental corruption; they are not a publisher signature.

### Preview limitations

This is not a complete three-platform beta. Full accessibility, external Office compatibility, clean installation, and physical multi-display testing remain open. Comparison is not real-time collaboration or a complete three-way merge; themes, layouts and other dependencies are not automatically migrated. The native writer uses schema 1.10/minimum reader 11: keep backups when exchanging files with older builds. Packages are unsigned; macOS may require opening the app via the context menu, Windows may show SmartScreen, and Linux needs the relevant Vulkan, font and X11/Wayland desktop libraries. Platform builds and automated tests do not establish GUI acceptance on every target machine. See [JoyDeck status](https://github.com/willmove/joyoffice/blob/joydeck-v0.1.0/docs/joydeck/STATUS.md) for details.

## 中文

这是 JoyOffice Monorepo 中首次公开的 JoyDeck 预览版。每个平台包均包含演示文稿桌面应用和 `joydeck-cli` 命令行工具。

### 主要功能

- 创建和编辑幻灯片、文字、图片、表格、图表及其他受支持对象；原生文稿保留稳定对象标识，编辑历史支持撤销和重做。
- 比较原生演示文稿并逐对象接受变化；可显式选择在同一个可撤销事务中复制所需 PNG 图片资源。
- 通过自动化接口使用已注册的编辑命令，支持不修改文稿的预先校验以及可选的修订号检查。
- 导出受支持的格式。macOS 预览版还支持静态幻灯片 MP4 和仅含标题的 WebVTT；视频不包含声音、转场、动画或录制。

### 下载

- macOS Apple Silicon：`joydeck-0.1.0-macos-aarch64-release.zip`（`JoyDeck.app` 与命令行工具）
- Windows x86_64：`joydeck-0.1.0-windows-x86_64-release.zip`（桌面程序与命令行工具）
- Linux x86_64：`joydeck-0.1.0-linux-x86_64-release.zip`（桌面程序与命令行工具）
- Linux ARM64：`joydeck-0.1.0-linux-aarch64-release.zip`（桌面程序与命令行工具）

每个压缩包均附带 `.sha256` 文件。例如，在下载目录运行 `shasum -a 256 -c joydeck-0.1.0-macos-aarch64-release.zip.sha256`。校验和可发现意外损坏，不能替代发布者签名。

### 预览版限制

本版尚未完成完整三平台 Beta 验收；完整无障碍、外部 Office 兼容、干净安装和物理多显示器测试仍待完成。比较功能并非实时协作或完整三方合并；主题、版式及其他依赖不会自动迁移。原生写入采用 schema 1.10／最低读取版本 11：与旧版交换文件时请保留备份。发布包未签名；macOS 可能需要从右键菜单打开，Windows 可能显示 SmartScreen，Linux 需要安装相应的 Vulkan、字体及 X11/Wayland 桌面依赖。平台构建和自动测试成功不代表各目标机器上的图形界面已完成人工验收。详情见 [JoyDeck 状态](https://github.com/willmove/joyoffice/blob/joydeck-v0.1.0/docs/joydeck/STATUS.md)。
