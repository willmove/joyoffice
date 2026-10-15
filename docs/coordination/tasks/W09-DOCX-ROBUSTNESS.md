# W09：DOCX 安全回归、变异测试与 fuzz 工程

- 预留负责人：Windows 11 x64 主机的 Codex；环境已确认，尚未启动。
- 分支／目录：`codex/w09-docx-robustness`／`joyoffice-w09`。
- 代码基线：C07 `a88217136a570a8f224d92b808bc0b8840ab26ca`；另记录新版协作包完整 SHA。
- 对应：T103／T107 的不可信输入与往返验证；先读 AGENTS.md、第二轮 README／INTERFACES、SPEC FORMAT-001–004、ARCHITECTURE §5、QUALITY §2／3／6、C01/C02/C03/C07 报告。

这是独立的大型验证工程，包含可运行驱动、结构化样例、故障最小化和真实链路不变量；不是只跑一次现有测试。分三批连续推进，不修改被测生产实现，不等待 W05 或 W07 完成。

## 交付批次

1. **独立 harness 与样例矩阵。** 在独立 workspace 实现 OPC 解包、DOCX 导入、受限导入→engine 编辑→导出→重导入三类目标。输入从 stdin／文件读取且先限长；每类目标有确定预算、退出状态、case ID 与结果 JSON。样例覆盖 ZIP 声明／实际大小、重复／重叠／异常路径、CRC、压缩比、XML 深度／实体／namespace／兼容分支、关系循环／外链、保护／修订／签名／未知部件。原创种子，或只读使用仓库已登记语料并记录散列；不访问外部关系。
2. **有界变异与可复现最小化。** 实现固定种子的字节变异和 ZIP/XML 结构感知变异、受限编辑序列生成、进程级超时／内存限制能力检测、崩溃／超时／不变量失败分类与最小化。输出种子、输入 SHA-256、目标完整源码 SHA、预算、OS、退出码和重现命令；最小化必须保持相同失败类别。将异常转成最小回归测试，不捕获 panic 后报告成功。没有真正覆盖反馈的驱动必须写“变异测试”，不能冒充覆盖引导 fuzz。
3. **端到端证据与接力。** 验证确定性、未编辑原包复制、支持子集重导入语义、未知部件字节／关系保留、保护拒绝及失败会话不变。配置一次有明确运行数／时间上限的 campaign 并保留摘要、最小反例和资源峰值可测范围；不建立后台自动监控。提供外部 fuzz 驱动接入规范及一键回放入口。若环境能以固定 Rust 1.98.1 支持实际覆盖引导驱动，先提交具体工具版本／依赖／许可方案给 C08；未安装或未获接口裁决不更换根工具链，报告清楚未覆盖，继续完成前两批与端到端证据。

W05/W07 交付后由 C08 再提供批准的集成 SHA，W09 可在自己的独立目录复测；不得自动追踪移动分支或合并执行者在途代码。发现生产 bug，只在自己的报告给反例、预期／实际、建议修复文件；由 C08 分回对应负责人，W09 不越界修生产文件。

## 独占可写路径

- `experiments/docx-robustness/**`（独立 manifest／锁文件、harness、驱动、测试、原创语料、来源说明）
- `docs/coordination/evidence/W09/**`（小型重现输入、结构化结果、命令与环境；不提交巨型随机输出／构建产物）
- `docs/coordination/reports/W09.md`

其余只读，尤其 OPC／DOCX／engine／shell 生产代码、共享 fixture 清单与生成器、CI、根 Cargo。自己的 manifest 声明 `[workspace]`、edition 2024、`rust-version = "1.98"`、`unsafe_code = "forbid"`，使用固定 Rust 1.98.1。C08 授权使用 Python 标准库、std、现有本地 crates 的 path 依赖及精确 `crc32fast = "=1.5.2"`、`flate2 = "=1.1.10"`（default-features=false, rust_backend）、`sha2 = "=0.10.9"`、`tempfile = "=3.27.0"`；按需使用，不升级依赖。其他工具／依赖列方案；独立 Cargo.lock 提交后全部验证 --locked。

## 验证与验收

```sh
cargo test --manifest-path experiments/docx-robustness/Cargo.toml --locked
cargo test --manifest-path experiments/docx-robustness/Cargo.toml --release --locked
cargo fmt --manifest-path experiments/docx-robustness/Cargo.toml --all -- --check
cargo clippy --manifest-path experiments/docx-robustness/Cargo.toml --all-targets --locked -- -D warnings
python3 scripts/check_docs.py
git diff --check
```

README 必须提供实际可执行的 smoke／campaign／replay／minimize 命令；测试驱动本身的超时、错误分类、固定种子、最小化与日志 schema。报告运行次数、时长、成功／拒绝／异常数、未执行目标、全部故障及其回放；无崩溃不等于安全证明。性能或内存指标无法取得时写未测，不从输入长度推算实测峰值。每批真实提交与结果经用户交给 C08；不宣称 Word/WPS／GUI／M1 完整验收完成。

## Windows 11 x64 验证补充

先用原生 MSVC／Rust 1.98.1 跑 harness、变异／回放／最小化与进程故障测试；文档检查用 `py -3 scripts/check_docs.py`。Python 驱动使用参数列表、二进制 stdin 和明确超时，测试含空格／中文路径、非零退出与进程结束后的输出收集。不要依赖 Unix signal、resource/rlimit 或 shell 命令在 Windows 上可用；超时后确认自己创建的子进程树已退出，无法控制内存限额时单列能力缺口，不能宣称有 OS 级硬内存隔离。实际覆盖引导驱动的 Windows 支持／工具链约束先实测；WSL campaign 单列 Linux 证据，不替代原生 Windows。只操作本任务测试进程，不关闭其他 Agent 或桌面程序。
