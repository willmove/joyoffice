# Word shell 全目标 Clippy 跟进

基于整合提交 `b61bcfe29c54ed88183dedd609c9270963e52da1`，远程 main 仍为 `aac15f2f`。本批处理的是此前失败命令中的 `joyword-shell --all-targets`，不代表 Word 全工作区所有 crate 的全目标扫描。

原始失败的 248 处诊断由两部分构成：246 处位于测试模块（unwrap/unwrap_err 和两处 panic，作为构造失败与断言失败），以及 `r8_render_goldens` 示例的两处 f64 到 f64 多余转换。没有发现该批诊断指向生产函数中的 panic/unwrap。

修改仅为删除多余转换，并在 12 个已有 `#[cfg(test)]` 模块声明带原因的 `clippy::unwrap_used` 局部例外，其中两个确有 panic 断言的测试模块同时声明 `clippy::panic` 例外。这与已存在的 legacy_recovery 测试及 Deck 测试的局部规则一致。保留 Cargo.toml 的生产 deny 规则，不使用 crate 级 allow，不删除断言，不批量重写测试，不修改产品级 CI/lint 范围。

验证：

- `cargo clippy --locked -p joyword-shell --all-targets -- -D warnings` 通过（库、单元测试和示例）。
- `cargo test --locked -p joyword-shell`：73 通过、0 失败、0 忽略。
- rustfmt、1452 Rust 文件工程门禁、291 文件来源门禁和文档检查通过。
- 没有新增生产功能，本批未重复整套跨产品测试；此前 863 项 Rust 与 179 项 Python 整合结果仍作为前一阶段记录。

本批使用同一 Linux 工具链，未运行示例的真实渲染黄金输出，也未将 lint 通过视为跨平台或桌面验收。

## 仍需外部条件的验收

| 阻碍 | 需要什么 | 当前可确认的边界 |
| --- | --- | --- |
| Word 平台恢复、磁盘写入故障、系统偏好与真实文件关联 | 对应 Windows/macOS 环境及可控故障场景 | shell 自动测试已通过；本轮不代表系统级故障演练 |
| Deck 系统剪贴板、输入法/AltGr、窗口焦点与放映 | 对应桌面会话、真实键盘/输入法；Linux 还需实际 X11/Wayland 会话 | 解析器、适配器 harness 与按键逻辑测试不能替代实屏 |
| 签名、公证与更新/回退的真实发布链 | 对应平台工具链、授权签名身份及受控更新环境 | 本地门禁通过不等于正式签名或发布；本轮不具备发行资格 |
| `mac@0.1.1`、`block@0.1.6`、`malloc_buf@0.0.6` 正文 | 精确版本绑定的权威许可全文 | 已核对标签，仅有声明，不能自拟正文 |
| `pathfinder_geometry@0.5.1` 正文 | 与包内提交对应、可验证的权威来源 | 记录提交不可取，当前分支版本号相同不足以替代 |
| `seahash@4.1.0` 正文 | 可访问且与精确版本绑定的权威来源 | 原站拒绝访问，未绕过或使用未验证镜像 |

上述来源限制使严格许可阶段仍拒绝：Linux 缺 3 项、Windows 缺 2 项、Linux 主机选择的 macOS 图缺 5 项。哈希与许可标识不构成法律合规结论。

仍有不依赖设备的未完成开发，例如新增缺失页脚的安全模型/命令、完整页脚 UI 接入前的模型工作，以及发布归档资源预算/全载荷校验评估；不能将项目全部剩余工作归为“等待设备”。本轮没有扩大到这些任务。

所有原分支保留；修改继续位于本地 `fix/joyoffice-safety-integration`。未推送、未创建 PR、未发布。
