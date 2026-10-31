# 构建与分发

`cargo xtask dist` 先构建 Release，再收集目标依赖许可证、来源与图标，产出便携 ZIP、源码 ZIP 和平台安装/分发包。`--no-build` 只用于已经验证过的同一 Release 二进制。阶段标签 M8-preview3 表示 Windows 可测增量；其他平台的原生验收和签名未通过前不标为发布候选完成。

## Windows

```powershell
pwsh -NoProfile -File scripts/dev.ps1 xtask ci
pwsh -NoProfile -File scripts/dev.ps1 xtask smoke --stage M8
pwsh -NoProfile -File scripts/dev.ps1 xtask dist
pwsh -NoProfile -File scripts/installer-smoke.ps1 -Installer dist/joysheet-0.1.0-M8-preview3-windows-x86_64-setup.exe
```

安装器使用 NSIS 3.11（zlib），本机工具解压到 artifacts/tools/nsis-3.11；也可设置 `JOYSHEET_NSIS` 指向 makensis.exe。三端 CI 的 Windows 步骤安装锁定版本编译器。工具不随源码检查点分发。官方 [NSIS 下载](https://nsis.sourceforge.io/Download)、[版本与哈希数据](https://nsis-dev.github.io/release-data/versions.json)。使用前校验 Windows ZIP SHA256：`c7d27f780ddb6cffb4730138cd1591e841f4b7edb155856901cdf5f214394fa1`。本次 SourceForge 镜像返回 HTML，改从 MacPorts 南京大学镜像下载相同文件并核对官方哈希，未执行错误响应。

用户级安装到 `%LOCALAPPDATA%/Programs/Joysheet`，含开始菜单和卸载器。“打开方式”组件可选，不更改默认应用。卸载按安装清单删除已安装文件，保留额外文件及用户配置/恢复目录。`/TEST` 隔离测试禁用快捷方式和注册表写入，但执行同一文件解包和卸载代码；相关系统集成不能据此记为实际运行通过。

可用的 VC++ app-local runtime DLL 从本机 VS Redist 目录随包复制。Windows 原生打印对话框仍需要 PowerShell 7；工作簿查看、编辑、PDF 导出不需要终端。设置 `JOYSHEET_WINDOWS_SIGN_THUMBPRINT` 可接入已安装的发行者证书和 signtool；没有证书的本次预览包保持未签名。

## macOS Apple Silicon

在 macOS 15+ arm64 主机运行相同 dist 入口，创建 `Joysheet.app`、Info.plist（文件类型、15.0 基线与高 DPI）、ICNS、CLI、许可证和 DMG。默认仅 ad-hoc 签名，不能当作发行者签名。发行时设置 `JOYSHEET_MAC_SIGN_IDENTITY`；已有 notarytool keychain profile 时可设置 `JOYSHEET_NOTARY_PROFILE`，命令失败即停止打包。尚无本地 macOS 主机证据。

## Ubuntu 24.04 x64

生成带 AppRun 的 AppDir tar.gz 和 `.deb`（应用、CLI、图标、桌面/MIME 入口、依赖声明、许可证）。需要 dpkg-deb、tar；PDF 预览依赖 poppler-utils，系统打印通过默认 PDF 查看器。CI 用 ldd 检查实际动态依赖并检查 deb。尚无本地 Linux 原生证据；不将 Windows 上的打包源代码检查当作 Linux 成功。

## 发行依赖与更新

分发目录 licenses/inventory.json 保留版本、SPDX 声明、原始仓库与可获得的许可证正文；包含构建/测试依赖的超集，不是法律意见。所有来源记录和 Cargo.lock 随源码包保留。当前未提供正式发布远端、更新 feed、发行者证书或 Apple 公证凭据；自动更新发布、发行者签名和公证不宣称已完成。预览包采用人工下载更新，保留用户配置。

本次 Windows 主机清单为 508 项（含构建/测试依赖），25 项使用附带上游来源补充，工作区核心使用原项目根许可证。mac、seahash 的发布包缺少独立正文，分别保留上游声明、作者及标准条款，DECLARATION.md 明确来源限制；不将标准文本补充冒充原始版权文件。完整供应链/许可复核仍是正式发行门槛。


