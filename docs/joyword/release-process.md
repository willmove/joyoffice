# GitHub 发布流程（Release Process）

本流程是向 GitHub 发布 JoyWord 版本的规范步骤，参照 Termior 的
`docs/process/release-process.md` 并结合本仓库约定（提交信息用英文、第三方
action 固定完整提交 SHA、CI 全绿才打标签）。发布完成的判定：标签工作流
（`.github/workflows/release.yml`）全部成功、各平台产物与 `.sha256` 校验文件
已挂到 Release、Release 带有人工整理的双语说明、仓库与 GitHub 同步。

## 1. 默认约定与前置检查

- 从 `main` 发布，工作区干净，本地分支与 `origin/main` 同步。
- 版本号以维护者明确给出的为准；未给出时从已有最高 `vMAJOR.MINOR.PATCH`
  标签递增 PATCH，不自行推断 major/minor/prerelease。
- 技术预览阶段默认发布为 **prerelease**，除非维护者另有要求。
- 每个 Release 必须带人工整理的双语发布说明（英文在前、中文在后）；工作流
  只负责上传产物，不生成、不覆盖发布说明。
- 公开标签一经推送不得删除、强移或重建（禁止强推历史）。
- 所需工具：stable Rust/Cargo、Git、`gh` CLI 且有推送与发布权限。

发布前检查：

```bash
gh auth status
git fetch --tags origin
git status --short --branch
git tag --sort=-version:refname
git log --oneline --decorate -10
gh run list --branch main --limit 3   # main CI 全绿；红灯必须先查清并记录
git tag --list vX.Y.Z
git ls-remote --tags origin vX.Y.Z
gh release view vX.Y.Z                # 新版本应返回 not-found
```

## 2. 同步版本元数据

版本真值是根 `Cargo.toml` 的 `[workspace.package].version`，所有 crate 继承。
改成 `X.Y.Z` 后让 Cargo 刷新锁文件（不要手工替换 `Cargo.lock` 里的版本字符串）：

```bash
# 修改 [workspace.package].version 后：
cargo update --workspace
cargo metadata --no-deps --format-version 1 | python3 -c \
  "import json,sys; print(sorted({p['version'] for p in json.load(sys.stdin)['packages']}))"
```

期望 diff：`Cargo.toml` 一行 + `Cargo.lock` 中工作区成员版本行。根锁文件之外，
`experiments/*/Cargo.lock`（path 依赖工作区成员的独立 crate）与
`scripts/package_macos_dev.py` 的 bundle 版本号也要同步。发布工作流会校验标签
版本与 `Cargo.toml` 一致并用 `--locked` 构建，锁文件过期会直接失败。

## 3. 发布前验证

```bash
python3 scripts/check_docs.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build -p joyword --release --locked
python3 scripts/package_release.py --binary target/release/joyword \
  --version X.Y.Z --platform <macos|windows|linux> --arch <x86_64|aarch64> --out dist
```

## 4. 提交、打标签、推送

```bash
git add -- Cargo.toml Cargo.lock experiments scripts/package_macos_dev.py
git commit -m "chore(release): bump workspace version to X.Y.Z"
git tag -a vX.Y.Z -m "JoyWord vX.Y.Z"
git push origin main vX.Y.Z
```

推送 `v*` 标签即触发 Release 工作流；这是权威发布运行。

## 5. 监视标签工作流

```bash
gh run list --workflow release.yml --limit 5 --json databaseId,status,conclusion,url
gh run watch <run-id> --exit-status --interval 30
```

以下作业必须全部成功：

- `Package (linux x86_64)` / `Package (linux aarch64)` — `tar.gz` + `.sha256`
- `Package (macos aarch64)` — `JoyWord.app` 的 `zip` + `.sha256`
- `Package (windows x86_64)` — `joyword.exe` 的 `zip` + `.sha256`
- `Publish GitHub release` — 把产物挂到已存在的 Release

产物为未签名预览构建：macOS 需右键「打开」绕过 Gatekeeper，Windows 的
SmartScreen 可能需要「更多信息 → 仍要运行」，Linux 需预装 GPUI 系统依赖
（Vulkan/X11/Wayland 等，见 README）。失败时用
`gh run view <run-id> --log-failed` 排查；公开标签已存在则保留标签，向前修复。

## 6. 整理发布说明

先用 `gh release create vX.Y.Z --prerelease --notes-file <草稿>` 建 Release（或在
打标签前建好），再触发工作流挂产物。草稿写在版本库之外（例如 `/tmp`），结构与
语言对照 `docs/joyword/` 现有文档；至少包含：一句话总结、亮点、边界与已知限制、
下载清单（每个产物的文件名与 `.sha256`）、验证证据（工作流结果与 main CI 状态）、
完整对比链接 `https://github.com/willmove/joyoffice/compare/<上一标签>...vX.Y.Z`。

发布说明规则：只写标签间 diff 能支撑的用户可见变化；覆盖直接提交与合并的 PR；
不宣称未验证过的测试、平台构建或功能。

## 7. 收尾核验

```bash
gh release view vX.Y.Z --json name,tagName,isDraft,isPrerelease,publishedAt,url,assets
git status --short --branch
git log -1 --oneline --decorate
git ls-remote --heads --tags origin main vX.Y.Z
```

确认：标题为 `JoyWord vX.Y.Z`、标签为 `vX.Y.Z`；Release 已发布且为预期
prerelease 状态；双语说明与对比链接在位；资产为 4 个平台产物各带 `.sha256`
共 8 个文件；标签工作流全绿；本地 `main`、`origin/main` 与标签指向预期提交。
全部通过后才算发布完成。
