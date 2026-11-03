# R9 JoyOffice 替换与联合验证

2026-11-03。用户明确授权备份 JoyOffice 并用 0.8.0 重构替换旧 JoyDeck；当前开发入口和后续事项见 [交接](../../../joyoffice/JOYDECK-R9-HANDOFF.md)。工程替换不表示所有旧功能或完整 UI 一致性已经完成。

## 备份与输入

完整备份、bundle、逐文件校验与恢复演练见 [备份状态](backup-state.json) 和交接。旧基线 a973f75、重构输入 526ebf0；[导入输入清单](import.json) 保留进入演练前的原始指纹，后续宿主适配由 Git 与来源台账记录。先在备份旁独立 worktree 演练，通过编译/回归后才切换日常工作区。

## 已完成的演练检查

- Deck headless 869 项、桌面 870 项通过；随后新增的 R9 旧命令拒绝/能力协商回归已在联合测试通过。
- Word 1,316 项通过；Sheet 1,396 项通过、1 项原有计时测试忽略。分产品检查包括 fmt 与严格 Clippy，见 [产品汇总](product-tests.json)。
- 全 workspace 的 validation/automation 联合 Clippy 通过，Rust **3,571 通过、0 失败、1 忽略**，258 组；忽略项为 Sheet recalc_with_range_formulas_scales_linearly，要求 release 中单独计时。AI feature 启动不发 provider 请求的测试通过。[联合汇总](joint-summary.json)、[联合测试](joint-tests.txt)、[Clippy](joint-clippy.txt)。
- 六个桌面/CLI 二进制独立构建成功。[构建](three-products-build.txt)。
- 151 项通用 Python、14 项产品工程工具、11 项交付工具通过；原门禁反例仍被拒绝。
- cargo-deny 0.20.2 官方资产 sidecar 校验后运行，advisories/bans/licenses/sources 均通过；既有忽略规则未新增。[依赖审计](dependency-audit.txt)、[工具来源](tooling.json)、[锁差异](lock-delta.json)。许可检查通过不等于正式再分发资格或全部许可全文齐备。
- 来源与依赖/CLI 隔离、宿主及产品文档检查通过。[分层/IO/断言审阅](../../../joyoffice/JOYDECK-R9-GATE-AUDIT.md)。

本轮继续在日常 JoyOffice 目录完成最终包、CLI/旧文件回归及 Windows 桌面验收；最终产物与结果在本报告补录。完整旧功能无回退（JD-906）、UI/命令完整一致及多平台真机资格保持开放，不用上述检查替代。
