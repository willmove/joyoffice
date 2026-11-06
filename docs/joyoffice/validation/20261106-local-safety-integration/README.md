# 本地安全修复整合与审查清点

2026-11-06。已重新 fetch 并核对远程 `main@aac15f2f9078fa5a0ff784575d9f296fd3a35c9c`；本轮期间未变化。新本地分支 `fix/joyoffice-safety-integration` 从该主干建立，以三次非快进合并整合全部成果，保留所有原分支与提交，不重写已推送历史。未推送、未创建 PR、未发布。

## 三个审查组

| 组 | 原整合分支与快照 | 范围 | 本轮审查范围（提交间 diff） | 远程状态 |
| --- | --- | --- | --- | --- |
| Word P0 | `fix/joyword-p0-integration@854304ce` | 恢复身份与对象保留、偏好设置读写安全、兼容与回退验证 | `aac15f2f..7c9187c5` | 整合分支未推送；其 5 个切片已推送且与远程同名引用一致 |
| Deck R3 | `feat/joydeck-r3-integration@a86bd58c` | 隐藏内容粘贴边界、字符输入优先、清除格式保留语义、页脚导入与已有页脚命令 | `7c9187c5..49ec1b62` | 整合及全部切片未推送 |
| 发布安全 | `fix/joydeck-license-provenance-check@abb85451` | 来源哈希、6 项精确版本正文、跨目标构建依赖身份、ZIP/TAR 门禁 | `49ec1b62..f341f5f5` | 全部未推送 |

完整分支 SHA、远程同名 SHA 与整合覆盖状态见 [分支清单](branches.json)。清单中的新整合分支 SHA 是代码合并节点，随后仅追加本地适配记录及本报告。源分支的 behind/ahead 提示不能当作推送状态；远程状态来自实际 `git ls-remote --heads origin`。

Word 的 `editing.rs` 与最新主干自动合并，无代码冲突；唯一手动冲突是 `docs/joydeck/STATUS.md` 两组新增状态记录，已保留双方内容。来源门禁发现 4 个本地修改缺少适配指纹：`package-native.py`、`inline_text.rs`、PPTX `lib.rs` 与 `read.rs`。本轮逐项更新 `adapted_sha256` 和变更说明，保留原始 `source_sha256` 与上游身份。这两个来源映射文件是整合后的必要跟进，不能漏掉后直接将旧源分支视为可发布。

审查可沿以上三个 diff 分组进行，最终可构建/门禁基线使用新整合分支。若授权后拆成独立 PR，应分别带上相应的适配记录，并重新核对当时主干及 CI；本报告不代表已经创建或通过远程 CI。

## 组合验证

[结果与日志摘要](verification.json)：

- Rust 六个相关 crate 组合回归：**863 通过，0 失败，0 忽略**。
- Python 脚本：**179 项，跳过 2 项，无失败**；共享 JoySheet 打包回归 **7 项通过**。
- 当前源码的 Linux 剪贴板隔离 harness **5 项通过**；键盘分流纯逻辑 **3 项通过**。这些均不是系统剪贴板/实屏验收。
- Word shell 库严格 Clippy 与 Deck 五 crate（含桌面 UI）全目标严格 Clippy 通过；fmt、文档、依赖隔离、迁移文档、291 文件来源指纹及 1452 Rust 文件工程门禁通过。
- 曾额外尝试将 Word 测试和示例纳入 `--all-targets -D warnings`，未通过：测试代码 unwrap/panic 与 `r8_render_goldens` 示例转换报错。仓库 `tools/xtask/product.py` 明确对 Word 使用库/二进制 lint 范围。本轮遵循该规则对受影响 shell 库复验，不更改 lint 策略，也不把额外检查称为通过。
- 三目标依赖盘点在整合源码上重新执行，见 [清单](dependency-inventory.json)：Linux 559 包/3 项缺正文，Windows 452 包/2 项，Linux 主机的 macOS ARM64 图 463 包/5 项；严格许可门禁均拒绝且未产生严格 SBOM。Cargo.lock 与 main 一致。

此前额外 Word shell 全目标 Clippy 失败已在后续小范围修复并复验通过，见 [跟进及外部阻碍](word-clippy-followup.md)。上述初次运行记录保留，不将新结果追溯改写为当时已通过。

## 尚未验收

Word 真机磁盘故障/恢复与 Windows/macOS 场景；Deck 系统剪贴板、真实键盘焦点、完整页脚 UI 与新增缺失页脚；完整跨平台构建、签名、公证、真实设备和发布载荷验收仍开放。Linux 上选择其他目标的依赖图不等于对应设备构建或验收。

未解决正文仍是 `mac@0.1.1`、`pathfinder_geometry@0.5.1`、`seahash@4.1.0`，以及 macOS 图中的 `block@0.1.6`、`malloc_buf@0.0.6`。不绕过来源访问限制，不补造版权或法律结论。历史 CLI 导出和各切片证据继续保留，但未在本轮重复运行的检查不计入本次组合通过数。
