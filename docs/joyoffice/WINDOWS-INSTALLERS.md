# 三产品 Windows 安装器

JoyWord、JoyDeck、JoySheet 均提供 Windows x64 和 ARM64 的用户级 Setup EXE，便携 ZIP 继续保留。NSIS 3.11 共用实现位于 `scripts/package_windows.py` 和 `scripts/windows/joyoffice.nsi`；Sheet 保留原有安装目录与注册表身份，由 `scripts/joysheet/package-windows.ps1` 收集 app-local CRT 后调用共用实现。

安装器默认写入 `%LOCALAPPDATA%/Programs/JoyWord`、`JoyDeck` 或 `Joysheet`，提供开始菜单和系统卸载入口。可选“打开方式”只登记本产品的 ProgID，不切换默认应用。卸载仅删除载荷清单中的文件和属于当前安装路径的注册项，保留额外文件、用户设置和恢复目录。ARM64 安装器拒绝在 x64 Windows 安装。

## 构建与验证

正式发布工作流为三个产品生成安装器、SHA256，并在两个原生 Windows runner 上检查安装、逐文件匹配、CLI 和卸载。Word/Deck 从刚构建的 Release ZIP 生成；Sheet 的产品打包命令仍直接生成安装器。

对已有发布补充安装器时使用 `Windows installer validation` 工作流（`.github/workflows/windows-installers.yml`）。输入现有版本，下载 GitHub 公开 ZIP 与 sidecar，先核对 GitHub 资产摘要，再验证归档路径、版本、架构及源码快照标记。它在三个产品的 x64/ARM64 原生 runner 上生成并测试安装器，上传工作流资产，**不会改动产品标签或公开 Release**。协调者在全部通过后追加缺失的 EXE/sidecar，禁止覆盖已有公开资产；更新发布说明并核对原资产 ID/摘要保持原值。

本机构建示例（NSIS 精确为 3.11，或设置 `JOYOFFICE_NSIS`）：

```powershell
python scripts/package_windows.py --product joyword --version 0.1.4 --arch x86_64 --archive dist/joyword/joyword-0.1.4-windows-x86_64.zip --output dist/joyword/joyword-0.1.4-windows-x86_64-setup.exe
pwsh -File scripts/windows/installer-smoke.ps1 -Product joyword -Installer dist/joyword/joyword-0.1.4-windows-x86_64-setup.exe -Archive dist/joyword/joyword-0.1.4-windows-x86_64.zip
```

安装器包含 `INSTALLER-INFO.json`，记录载荷 ZIP SHA256（从目录构建时为空）、目标架构、编译器版本和安装器源码指纹。默认 smoke 使用 `/S /TEST` 隔离目录，跳过快捷方式和注册表；`-SystemIntegration` 只在没有既有产品注册项/快捷方式时验证真实当前用户安装登记。它不代表 GUI、可选关联、升级回滚或所有系统集成场景已验收。

## 预览边界

安装器当前未作发行者签名。ARM64 runner 自动验证不代替人工实机验收；签名、完整升级/回滚、并行架构切换、可选关联的 Explorer 行为及产品原有差距仍分别跟踪。2026-11-14 对 v0.1.4 的补充沿用原应用二进制，不能当成应用功能更新或移动原标签。

NSIS 行为按[官方脚本手册](https://nsis.sourceforge.io/Docs/Chapter4.html)与随 3.11 工具分发的 `Include/x64.nsh` 实现；安装器保留 NSIS COPYING。当前版本的实测结果见[安装器验证](validation/20261114-windows-installers/README.md)。
