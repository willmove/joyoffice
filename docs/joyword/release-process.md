# GitHub 发布流程（Release Process）

本流程是向 GitHub 发布 JoyWord 版本的规范步骤，参照 Termior 的
`docs/process/release-process.md` 并结合本仓库约定（提交信息用英文、第三方
action 固定完整提交 SHA、CI 全绿才打标签）。发布完成的判定：标签工作流
（`.github/workflows/release.yml`）全部成功、各平台产物与 `.sha256` 校验文件
已挂到 Release、Release 带有人工整理的双语说明、仓库与 GitHub 同步。

## 1. 默认约定与前置检查

- 从 `main` 发布，工作区干净，本地分支与 `origin/main` 同步。
- 版本号以维护者明确给出的为准；未给出时从 JoyWord 已有最高 `joyword-vMAJOR.MINOR.PATCH`（兼容历史 `v*`）
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
git tag --list joyword-vX.Y.Z
git ls-remote --tags origin joyword-vX.Y.Z
gh release view joyword-vX.Y.Z                # 新版本应返回 not-found
```

## 2. 同步版本元数据

版本真值是 `apps/joyword/Cargo.toml` 的 `[package].version`，`apps/joyword-cli/Cargo.toml` 与其同步。根 workspace 版本与其它产品版本独立。
改成 `X.Y.Z` 后让 Cargo 刷新锁文件（不要手工替换 `Cargo.lock` 里的版本字符串）：

```bash
# 修改 apps/joyword 与 apps/joyword-cli 的版本后：
cargo update --workspace
cargo metadata --no-deps --format-version 1 | python3 -c \
  "import json,sys; print(sorted({p['version'] for p in json.load(sys.stdin)['packages']}))"
```

期望 diff：两个应用 manifest 与根锁文件中对应包版本。若修改内部库版本，按需刷新使用它的独立实验锁文件。打包工具从应用 manifest 读取 bundle 版本。发布工作流校验标签与应用版本，使用 `--locked` 构建；锁文件过期直接失败。

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
git add -- apps/joyword/Cargo.toml apps/joyword-cli/Cargo.toml Cargo.lock
git commit -m "chore(release): bump JoyWord version to X.Y.Z"
git tag -a joyword-vX.Y.Z -m "JoyWord vX.Y.Z"
git push origin main joyword-vX.Y.Z
```

推送 `joyword-v*` 标签（`v*` 仅保留 JoyWord 历史兼容）即触发 Release 工作流；这是权威发布运行。

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

先用 `gh release create joyword-vX.Y.Z --prerelease --notes-file <草稿>` 建 Release（或在
打标签前建好），再触发工作流挂产物。草稿写在版本库之外（例如 `/tmp`），结构与
语言对照 `docs/joyword/` 现有文档；至少包含：一句话总结、亮点、边界与已知限制、
下载清单（每个产物的文件名与 `.sha256`）、验证证据（工作流结果与 main CI 状态）、
完整对比链接 `https://github.com/willmove/joyoffice/compare/<上一标签>...joyword-vX.Y.Z`。

发布说明规则：只写标签间 diff 能支撑的用户可见变化；覆盖直接提交与合并的 PR；
不宣称未验证过的测试、平台构建或功能。

## 7. 收尾核验

```bash
gh release view joyword-vX.Y.Z --json name,tagName,isDraft,isPrerelease,publishedAt,url,assets
git status --short --branch
git log -1 --oneline --decorate
git ls-remote --heads --tags origin main joyword-vX.Y.Z
```

确认：标题为 `JoyWord vX.Y.Z`、标签为 `joyword-vX.Y.Z`；Release 已发布且为预期
prerelease 状态；双语说明与对比链接在位；资产为 4 个平台产物各带 `.sha256`
共 8 个文件；标签工作流全绿；本地 `main`、`origin/main` 与标签指向预期提交。
全部通过后才算发布完成。
