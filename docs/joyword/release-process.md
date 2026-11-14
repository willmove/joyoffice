# GitHub 产品发布流程

JoyWord、JoyDeck 和 JoySheet 共享根 `Cargo.toml` 的工作区版本，使用独立的 `joyword-v*`、`joydeck-v*`、`joysheet-v*` 标签与 Release；历史 `v*` 标签仅兼容 JoyWord。技术预览保持 prerelease，不声称正式签名或完整平台人工验收。

## 版本与说明

- 未指定版本时，多产品同批发布以本批产品最高已有版本递增 PATCH，选择均未占用的共同版本；公开标签不得移动、删除或重建。
- 更新工作区版本及第一方精确 path 依赖版本，通过 `cargo metadata --offline --format-version 1` 同步锁文件，保留所有外部依赖版本和 checksum。桌面与 CLI 必须一致。
- 同步当前下载页、产品状态与交接文档；历史发布说明及证据不重写。
- `docs/releases/<tag>.md` 使用 `## English` 和 `## 中文`，说明本版变化、下载、校验、已知限制和验证范围。

## 验证、提交与发布

1. 获取远端与标签，核对干净状态和最新 main，确认版本未占用。
2. 运行三产品 `cargo xtask check PRODUCT`、`python scripts/check_docs.py`、工程与打包 Python 测试。版本及公共锁文件变更须跨产品回归。
3. 构建并打包本机 release，实际启动产物验证编辑、保存重开及本版界面变更，记录哈希和未覆盖项。
4. 提交并合入 main，推送后等待同一提交的三产品 CI 全绿。
5. 对每个产品运行 `gh workflow run release.yml --ref main -f product=PRODUCT -f version=VERSION -f create_tag=true`。也可在已验证提交推送带注释的产品标签。

## 自动构建和发布

`.github/workflows/release.yml` 在六种原生 runner 构建桌面和 CLI：Windows、macOS、Linux 各 x86_64 与 ARM64。JoyWord 提供 ZIP 或 TAR.GZ，JoyDeck 提供 ZIP，三个产品均另含 Windows Setup EXE；JoySheet 在 macOS/Linux 还提供 DMG/DEB。Windows 安装器共用 NSIS 3.11，实现、原生安装检查及已有版本的安装器补充分发见[三产品 Windows 安装器](../joyoffice/WINDOWS-INSTALLERS.md)。

所有平台完成后，工作流验证完整资产矩阵、归档成员、架构、版本、构建信息、SHA-256 和 SBOM。自动建标签模式只允许从 main 创建指向当前提交的新标签。草稿上传后再次核对 GitHub 资产集合及摘要，全部通过才公开预览版，不覆盖已有公开 Release。

失败时检查 `gh run view RUN_ID --log-failed`，临时问题重跑原任务。标签代码有误则向前修复并采用新版本，禁止移动旧标签。未公开的草稿不算完成发布。

## 发布后检查

核对三条工作流成功、三个 Release 公开且为 prerelease、各六平台的包及校验文件齐全、GitHub 摘要与本地验证一致，并确认标签指向预期提交。CI/QEMU 不代替对应系统与硬件的人工验收；信创真机和其他未实测项明确保留。
