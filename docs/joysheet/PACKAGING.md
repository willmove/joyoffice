# 构建与分发

JoyOffice 中使用 `cargo xtask package joysheet --release` 生成普通预览包；增加 `--validation` 生成允许自动化 GPU 截图的独立验证包。输出位于 `dist/joysheet/`，包括便携 ZIP、源码 ZIP 和当前平台原生分发包。打包始终从锁定依赖构建，不接受旧的 `dist --no-build` 跳过构建入口。

```powershell
pwsh -NoProfile -File scripts/joysheet/dev.ps1 xtask check joysheet --validation
pwsh -NoProfile -File scripts/joysheet/dev.ps1 xtask smoke joysheet --stage M8
pwsh -NoProfile -File scripts/joysheet/dev.ps1 xtask package joysheet --release --validation
pwsh -NoProfile -File scripts/joysheet/installer-smoke.ps1 -Installer dist/joysheet/joysheet-0.1.0-windows-x86_64-release-validation-setup.exe
```

包名按产品版本、宿主系统、架构、profile 和可选 validation 后缀生成。GUI/CLI 版本必须一致，Windows 版本资源与安装器读取产品版本。构建前后比较实际源码输入哈希；已有输出目录不能覆盖。资源、脚本、开发文档、版权原文随包携带；正式发布路由为 `joysheet-v*`，本次迁移不创建发布或标签。

## Windows

安装器使用 NSIS 3.11（zlib），本机路径为 `artifacts/joysheet/tools/nsis-3.11`；也可设置 `JOYSHEET_NSIS` 指向 makensis.exe。CI 安装精确版本。官方 [下载](https://nsis.sourceforge.io/Download)及[版本/哈希数据](https://nsis-dev.github.io/release-data/versions.json)给出的 Windows ZIP SHA256 为 `c7d27f780ddb6cffb4730138cd1591e841f4b7edb155856901cdf5f214394fa1`，工具不随源码检查点分发。

用户级安装到 `%LOCALAPPDATA%/Programs/Joysheet`，含开始菜单和卸载器。“打开方式”组件可选，保留默认应用。卸载按清单删除包文件，保留额外文件及用户配置/恢复目录。`/TEST` 隔离模式执行同一文件解包和卸载代码，禁用快捷方式和注册表写入；测试结果不能替代这些系统集成的实际验收。自动安装后 GUI 检查使用验证版包。

可用的 VC++ app-local runtime DLL 从 VS Redist 目录复制。Windows 原生打印对话框需要 PowerShell 7；工作簿查看、编辑、PDF 导出不需要终端。设置 `JOYSHEET_WINDOWS_SIGN_THUMBPRINT` 可接入发行者证书和 signtool；未配置证书的预览包保持未签名。

## macOS Apple Silicon

在 macOS 15+ arm64 主机运行相同入口，生成 `Joysheet.app`、Info.plist、ICNS、配套 CLI 和 DMG。默认仅 ad-hoc 签名；发行者签名使用 `JOYSHEET_MAC_SIGN_IDENTITY`，公证使用已有 `JOYSHEET_NOTARY_PROFILE`。命令失败即停止打包。构建 CI 与实际原生 GUI 验收分别记录。

## Ubuntu 24.04 x64 / ARM64

生成 AppDir tar.gz 和 deb（应用、CLI、图标、桌面入口、依赖声明及许可）。需要 dpkg-deb 和 tar；PDF 预览依赖 poppler-utils，打印通过系统 PDF 查看器。Linux 运行依赖由 deb 显式声明，原生 GUI/设备验收另列。

## 源码、依赖和更新

`SBOM.spdx.json` 与 `dependency-licenses/index.json` 只记录当前宿主、功能和 GUI/CLI 两个入口实际使用的普通/构建依赖闭包。依赖版本、SPDX 声明、源地址及原始许可正文随包保留，其他产品运行时代码不进入清单。

`SOURCE-HASHES.json`、`BUILD.json`、`BUILD-INFO.json` 和 `PAYLOAD-SHA256.json` 分别记录源码、构建、功能和载荷。源码 ZIP 包含 Sheet 专属 workspace；仅裁剪成员与锁文件，验证依赖版本/校验和未升级，并在包中记录原始及派生输入。完整 Monorepo 工程门禁仍在 JoyOffice 仓库运行。

mac、seahash 的正文补充保留来源限制，详见 `third-party/` 声明；不能冒充原始版权文件。发行者签名、公证、更新 feed 与完整供应链复核仍是正式发行开放项。当前预览采用人工下载更新并保留用户配置。迁移前包和超集许可清单见[历史产物](LEGACY-ARTIFACTS.md)，不作为新包证据。
