# GitHub 产品发布流程

JoyWord 和 JoyDeck 独立版本、独立标签与 Release；历史 `v*` 标签仅兼容 JoyWord。两个产品的发布工作流均使用 `.github/workflows/release.yml`，从已验证的 `main` 提交构建桌面和 CLI 程序。技术预览默认标为 prerelease，不声称正式签名或完整平台人工验收。

## 版本与说明

- 分别以 `apps/joyword/Cargo.toml` 和 `apps/joydeck/Cargo.toml` 的版本为真值；同产品的 `-cli` manifest 必须与桌面应用一致。单产品发布不必更改另一产品或 workspace 版本。
- 没有指定版本时，在对应产品已有最高标签上递增 PATCH；JoyWord 同时考虑历史 `v*` 标签。首次发布的产品使用当前应用版本，除非它已被占用。
- 在 `docs/releases/<tag>.md` 编写人工整理的发布说明，以 `## English` 和 `## 中文` 分节。包含用户可见变化、已知限制、各平台文件名、校验方法及版本范围；不声称未经验证的事项。
- 更新应用及 CLI manifest 后，用 `cargo metadata --format-version 1` 更新根锁文件。公共锁文件变更须对两个产品做回归。公开标签不得删除、强移或重建。

## 验证、提交与标签

1. 检查 `git status`、获取远端和标签，确认目标标签及 Release 均不存在。
2. 运行 `python3 scripts/check_docs.py`、产品工程检查与相关 Python 测试；验证本机 release 构建及打包。涉及锁文件或发布构建规则时，检查两个产品。
3. 将版本、发布说明和必要的流程修复提交并合入 `main`，推送后等待同一提交的 JoyWord 与 JoyDeck CI 全绿。
4. 为每个产品分别在该提交创建并推送带注释的标签 `joyword-vX.Y.Z` 或 `joydeck-vX.Y.Z`。推送标签会启动对应的 Release 工作流；只发布一个产品时只推对应标签。

## 自动构建和发布

工作流在原生 runner 上分别构建 Linux x86_64、Linux ARM64、macOS Apple Silicon 和 Windows x86_64 的桌面与 CLI 程序。打包器记录构建来源、依赖 SBOM、目标、版本及 SHA-256。

所有构建成功后，发布作业必须验证四个归档和四个 SHA-256 文件齐全、哈希正确、包含目标架构的桌面和 CLI 可执行文件，并检查构建信息。随后创建或继续与标签说明一致的预览草稿，上传产物，核对 GitHub 返回的每个资产摘要；全部通过后才公开 Release。现有已公开 Release 不会被工作流覆盖。手动 `workflow_dispatch` 应选择对应标签作为 ref，产品及版本必须与标签、应用 manifest 和 CLI 一致。

标签工作流失败时，检查 `gh run view <run-id> --log-failed`；临时问题可重跑原工作流，草稿会保留且可重试。若标签中的代码或工作流本身有误，在 `main` 向前修复并使用新版本和新标签，不移动旧标签。失败的草稿不是已完成的发布。

## 发布后检查

```bash
gh run list --workflow release.yml --limit 10
gh release view <tag> --json name,tagName,isDraft,isPrerelease,url,body,assets
git status --short --branch
git ls-remote --heads --tags origin main <tag>
```

确认两种语言的说明、各自四个平台压缩包和四个校验文件均存在，资产摘要与文件相符，标签工作流成功，标签指向预期提交。下载的校验文件只能检查传输损坏，不能证明发布者身份。CI、交叉构建或 QEMU 不替代相应系统和硬件上的人工验收；未实测项在说明中保留。
