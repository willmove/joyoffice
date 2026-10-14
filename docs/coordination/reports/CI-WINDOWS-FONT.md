# Windows 连字测试输入修正（2026-10-14）

初版集成提交 88b5739 的 CI [38518995032](https://github.com/willmove/joyoffice/actions/runs/38518995032) 中，Windows 的 ligatures_are_atomic_and_every_output_line_is_reshaped 失败；其他七项作业通过。日志记录 Segoe UI face 0、SHA-256 ba32a222b23d727267cf1aba4e5296fe84ce99b9d910915103fc085d7931bc88，在开启 standard_ligatures 后 fi 仍形成两个 glyph，测试要求一个。随后 c1a7831 的运行也出现同一 Windows 失败。

这是测试字体前提不成立，不能据此判定行拟合代码有误，也不能通过跳过/删除连字断言解决。macOS 先前的 Avenir 与 Linux 的 DejaVu 并未提供跨系统固定输入。

## 修正

- 从 google/fonts 固定提交的 Noto Serif 派生 OFL-1.1 静态最小子集，改名为 JoyOffice Test Serif；仅 4,164 bytes，保留 fi GSUB、AV GPOS、版权和许可，散列 cbc12abc87271f04aa0cf107214a49bad3b865f2dd01a4dec80fed818c032ec3。
- 三端默认拉丁测试均读取同一仓库 fixture；显式字体覆盖保留。固定 fixture 缺失必须失败，不允许静默跳过。CJK 测试仍按现有环境选择并遵守强制字体开关。
- 所有连字、字距、原子 cluster 和重整形断言不变。没有修改产品字体选择、行拟合算法或 Cargo 依赖。
- 上游固定 URL、源/输出散列、OFL 全文、再生成脚本见 tests/fixtures/fonts；FontTools 仅用于再生成，不是 CI/用户运行依赖。连续两次再生成得到相同输出散列。

## 本机验证与远程限制

固定子集实测支持 fi 连字与 AV 字距；使用它的 8 项字体集成测试及 layout 单元测试通过，固定字体实际读取两次；CJK 强制为 Songti face 0。全默认成员测试、相关 clippy、fmt 与规格检查再次执行，见对应日志。

最终 Windows runner 结果必须来自包含本修正的新提交；初版失败记录不会被改写为通过。草稿 PR #1 保留待新一轮 CI 完成后再判定跨平台构建门槛。

## 后续远程核对（2026-10-14）

固定字体修正提交 100bf6b26ea80d37159a040a670a30b2beede279 的两次 CI [38525056320](https://github.com/willmove/joyoffice/actions/runs/38525056320) 与 [38525061210](https://github.com/willmove/joyoffice/actions/runs/38525061210) 均全部通过，包括 Windows 作业；PR #1 已合并，main 为 2b9d58c2866703f4cd10b7f5e0fc56c961c23c57。

main 的后续运行 [38529091352](https://github.com/willmove/joyoffice/actions/runs/38529091352) 标记失败，但 8 个作业均未执行任何步骤。GitHub check-run 注释原文为：`The job was not started because recent account payments have failed or your spending limit needs to be increased. Please check the 'Billing & plans' section in your settings`。这是账户付款／消费额度阻塞；不能视为代码回归，也不能将该次 main CI 记录为通过。账户设置恢复后需重新执行。
