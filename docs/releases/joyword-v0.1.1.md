# JoyWord v0.1.1 · Preview / 预览版

## English

JoyWord 0.1.1 is an unsigned technology preview of the JoyOffice word processor. It includes the desktop application and the `joyword-cli` command-line tool.

### Changes since v0.1.0

- JoyWord now shares a Rust workspace with JoyDeck while retaining separate product crates, builds, versions, and release packages. The desktop and CLI packages include build information, source hashes, and a dependency SBOM.
- The GPUI and shared dependency versions were aligned for the monorepo. Cargo dependencies remain locked for the release build.
- The release pipeline builds both JoyWord executables on each platform, checks the archive contents and SHA-256 files, and publishes only after all packages have been uploaded and verified.

### Downloads

- macOS Apple Silicon: `joyword-0.1.1-macos-aarch64.zip` (`JoyWord.app` and CLI)
- Windows x86_64: `joyword-0.1.1-windows-x86_64.zip` (desktop executable and CLI)
- Linux x86_64: `joyword-0.1.1-linux-x86_64.tar.gz` (desktop executable and CLI)
- Linux ARM64: `joyword-0.1.1-linux-aarch64.tar.gz` (desktop executable and CLI)

Each archive has a separate `.sha256` file. For example, run `shasum -a 256 -c joyword-0.1.1-macos-aarch64.zip.sha256` from the download directory. These checksums detect accidental corruption; they are not a publisher signature.

### Preview limitations

JoyWord supports a declared subset of DOCX and remains a preview; consult the [status and specifications](https://github.com/willmove/joyoffice/tree/joyword-v0.1.1/docs/joyword) for supported behavior and outstanding acceptance work. Packages are unsigned: macOS may require opening the app via the context menu, and Windows may show a SmartScreen warning. Linux requires the relevant Vulkan, font, X11/Wayland and other desktop libraries. Build and automated-test results do not establish GUI acceptance on every target machine.

[Changes since v0.1.0](https://github.com/willmove/joyoffice/compare/v0.1.0...joyword-v0.1.1)

## 中文

JoyWord 0.1.1 是 JoyOffice 文字处理软件的未签名技术预览版，包含桌面应用和 `joyword-cli` 命令行工具。

### 相比 v0.1.0 的变化

- JoyWord 与 JoyDeck 现已共用 Rust 工作区，同时保留独立的产品模块、构建、版本和发布包。桌面与命令行包包含构建信息、源码哈希和依赖 SBOM。
- 为适配 Monorepo，对齐了 GPUI 与共享依赖版本；发布构建仍使用锁定依赖。
- 发布流程在各平台构建 JoyWord 桌面和命令行程序，检查压缩包内容与 SHA-256，并在全部包上传和校验成功后公开。

### 下载

- macOS Apple Silicon：`joyword-0.1.1-macos-aarch64.zip`（`JoyWord.app` 与命令行工具）
- Windows x86_64：`joyword-0.1.1-windows-x86_64.zip`（桌面程序与命令行工具）
- Linux x86_64：`joyword-0.1.1-linux-x86_64.tar.gz`（桌面程序与命令行工具）
- Linux ARM64：`joyword-0.1.1-linux-aarch64.tar.gz`（桌面程序与命令行工具）

每个压缩包均附带 `.sha256` 文件。例如，在下载目录运行 `shasum -a 256 -c joyword-0.1.1-macos-aarch64.zip.sha256`。校验和可发现意外损坏，不能替代发布者签名。

### 预览版限制

JoyWord 仅支持已声明的 DOCX 子集，仍处于预览阶段；支持范围与待完成验收见[状态和规格文档](https://github.com/willmove/joyoffice/tree/joyword-v0.1.1/docs/joyword)。发布包未签名：macOS 可能需要从右键菜单打开，Windows 可能显示 SmartScreen 警告。Linux 需要安装相应的 Vulkan、字体、X11/Wayland 等桌面依赖。构建和自动测试成功不代表各目标机器上的图形界面已完成人工验收。

[与 v0.1.0 的完整变更](https://github.com/willmove/joyoffice/compare/v0.1.0...joyword-v0.1.1)
