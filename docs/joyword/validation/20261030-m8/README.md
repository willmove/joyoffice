# M8 机器与 GUI 证据

2026-10-30。范围为 T807 本地邮件合并与当前 Windows 候选回归；任务结论见 [验收](../../acceptance/T807.md) 和 [报告](../../verification/m8-20261030.md)。原生操作使用 [Computer Use 技能](C:/Users/willm/.codex/plugins/cache/openai-bundled/computer-use/26.1027.30824/skills/computer-use/SKILL.md)。

- [原生观察和截图索引](gui/observations.jsonl)：包含逐条说明、窗口、原生可访问树和对应 JPEG 名称。
- [取消后磁盘读回](gui/cancel-readback.json)：输出目录为空，输入字节不变。
- [真实桌面批量读回](gui/batch-readback.json)：三封信件 SHA-256、精确正文、粗体／居中／软换行断言。
- [交付候选读回](gui/delivery-readback.json)：最终候选与此前三批九封逐字节一致，输入及旧输出未覆盖。
- [千封规模与上限](scale.json)：1000 封逐封正文一致，第 1001 条拒绝且无输出。
- [自动检查命令和原始日志](checks.json)、[81 格健壮性矩阵](robustness-smoke-summary.json)、[200 次行为反馈 fuzz](robustness-fuzz-summary.json)。
- [便携预览包验证](package.json)：两次字节相同，成员与 SPDX schema 读回、解包 CLI、最终 GUI 二进制身份一致。
- [发布审计](release-audit.json)：54 项门槛未通过、0 条无效记录，完整发布保持未批准。
- [应用原始日志](gui/application-stderr.log)：仍含焦点角色和窗口句柄诊断，T700／T701 保留未完成，未声称零错误退出或完整读屏通过。

输出在仓库 `target/` 下，不提交运行 profile、锁文件、用户 MRU、原始 Office 作者属性或系统字体。完整信创真机、其他系统原生 GUI、签名安装和真实打印没有在本轮通过。[SHA-256 证据清单](manifest.json) 覆盖本目录所有文件（清单自身除外）。
