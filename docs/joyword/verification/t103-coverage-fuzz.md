# T103 代码覆盖引导 fuzz（2026-10-20）

本文件是 DOCX 读取器 harness 的代码覆盖引导 fuzz 证据。生产 crate 树仍钉在 `9026d230e7e4d34349c3c4f5567f231b9f4d4c52`，本轮没有改 `crates/**`，也没有改生产行为。T103 仍是 `in_progress`：WPS 生成的样例和 Word／WPS 人工交换都不在本轮。

## 结论

覆盖引导已经接上，反馈来自插桩二进制的 LLVM 源码区域计数器。覆盖模式的摘要里 `coverage_feedback` 为 true，`feedback_signal` 为 `llvm-source-regions`。固定种子 `20261020`、60 次变异、27 个种子的 import 基线，一共读到 87 份 profile，`crates/` 下覆盖 3594／44812 个源码区域，新增区域事件 5 次，语料从 27 个种子长到 32 个。分类只有 `ok` 和 `rejected`，没有 crash、timeout、invariant 或 protocol。调度器累计的区域数与最终 `llvm-profdata` 合并结果一致（`coverage_merge_matches_scheduler` 为 true）。

稳定版 Rust 1.98.1 的 `-C instrument-coverage` 写出源码区域，不写出分支记录。本轮 `branches_total` 为 0。分支数组在导出里出现时会计入，但本工具链没有分支计数器，因此不能把本轮叫成分支覆盖引导。

## 工具审查

`llvm-profdata` 和 `llvm-cov` 来自 Rust 工具链的 `llvm-tools` 组件，安装命令是 `rustup component add llvm-tools --toolchain 1.98.1`。本轮版本是 LLVM 22.1.8-rust-1.98.1-stable，`rustc` 1.98.1（48a229cea，2026-09-01）。该组件是 Rust 项目随工具链分发的 LLVM，许可为 Apache-2.0 WITH LLVM-exception。它没有被收进仓库，没有成为 Cargo 或 Python 依赖，驱动只调用它合并和读取 profile。`rust-toolchain.toml` 没有改。没有使用 nightly，也没有搬入其他项目的 fuzzer 源码。

## 命令

构建把插桩 build script 的 profile 写到 git 忽略的 `target/` 下，避免在 `crates/` 旁留下 `*.profraw`：

```sh
rustup component add llvm-tools --toolchain 1.98.1
profile_dir="$PWD/experiments/docx-robustness/target/cov/build-profiles"
mkdir -p "$profile_dir"
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=experiments/docx-robustness/target/cov \
  RUSTFLAGS="-C instrument-coverage" \
  LLVM_PROFILE_FILE="$profile_dir/%p-%m.profraw" \
  cargo build --manifest-path experiments/docx-robustness/Cargo.toml --locked
python3 experiments/docx-robustness/driver.py fuzz --feedback coverage \
  --binary experiments/docx-robustness/target/cov/debug/joyword-docx-robustness \
  --out experiments/docx-robustness/target/coverage-20261020 \
  --seed 20261020 --iterations 60 --seconds 300 --timeout 8 --memory-mib 512 --max-corpus 128
```

同一调度、同一预算下的行为特征对照：

```sh
python3 experiments/docx-robustness/driver.py fuzz \
  --binary experiments/docx-robustness/target/debug/joyword-docx-robustness \
  --out experiments/docx-robustness/target/behavior-20261020 \
  --seed 20261020 --iterations 60 --seconds 300 --timeout 8 --memory-mib 512 --max-corpus 128
```

保留语料再在插桩二进制上按 import 各跑一遍，只为比较区域，不再变异：

```sh
python3 experiments/docx-robustness/driver.py cover \
  --binary experiments/docx-robustness/target/cov/debug/joyword-docx-robustness \
  --inputs experiments/docx-robustness/target/coverage-20261020/corpus \
  --out experiments/docx-robustness/target/coverage-corpus-regions \
  --timeout 8 --memory-mib 512
python3 experiments/docx-robustness/driver.py cover \
  --binary experiments/docx-robustness/target/cov/debug/joyword-docx-robustness \
  --inputs experiments/docx-robustness/target/behavior-20261020/corpus \
  --out experiments/docx-robustness/target/behavior-corpus-regions \
  --timeout 8 --memory-mib 512
```

既有 CI 行为特征命令（种子 20261019，200 次，60 秒，语料上限 128）另跑了一遍，退出码 0。

## 数字

脱敏摘要在 [coverage-20261020.json](../../../experiments/docx-robustness/evidence/coverage-20261020.json)。其中没有文档正文、文件系统路径或部件散列。`harness_binary_sha256` 是插桩 harness 二进制的散列，`target_sha` 是生产树锚点，都不是部件散列。活动当时的 `harness_git_sha` 是 `2860bd68ddea4b89aa8fa7ee1b0e6952e707b5fd`，工作区对 harness 目录是干净的。

| 项目 | 覆盖引导 | 行为特征 |
| --- | --- | --- |
| 种子 | 20261020 | 20261020 |
| 变异次数 | 60 | 60 |
| 反馈 | llvm-source-regions | harness-features |
| coverage_feedback | true | false |
| 新奇事件 | 5（新区域） | 10（新行为特征） |
| 语料大小 | 32 | 37 |
| 发现的特征数 | 不适用 | 27 |
| crates/ 区域 | 3594／44812 | 活动本身不采集 |
| 分支 | 0／0 | 不适用 |
| 异常 | 0 | 0 |
| 分类 | ok 10，rejected 50；基线 ok 14，rejected 13 | ok 11，rejected 49 |
| 耗时 | 95.9 秒 | 0.23 秒 |
| 地址空间上限 | 512 MiB，探测 enforced=true | 同左 |

覆盖活动还包含 27 次 import 基线。基线只建立已有种子的区域，不算新奇事件。变异仍按 opc、import、roundtrip 轮转，调度仍是新语料优先的平方偏置。

把两边留下的语料只按 import 再放进插桩二进制：覆盖语料 32 个输入覆盖 3588／44812 个区域，行为特征语料 37 个输入覆盖 3582／44812 个区域，两边都是 0 异常。覆盖活动总数 3594 比「只重放覆盖语料的 import」多 6 个区域，因为活动里还有 opc／roundtrip 和未留在语料里的输入。

种子 20261019 的既有行为特征命令：执行 200，新奇 18，语料 45，特征 42，ok 46，rejected 154，异常 0，`coverage_feedback` 为 false。

## 预算

沿用原驱动的硬边界：墙钟 300 秒、迭代 60、单次超时 8 秒、`RLIMIT_AS` 512 MiB（本机探测确认生效）、输入 1 MiB、保留输出 64 KiB、语料个数 128、语料字节 32 MiB、不访问网络、失败样本攒到 8 个即停。本轮没有触发失败停止。

## 未覆盖

没有 Word、WPS、图形界面或信创真机。没有 WPS 生成的固定样例，因此 T103 不能标完成。没有发现生产缺陷，所以没有改生产代码，也没有加已知缺口的忽略测试。夜间工作流见仓库 `.github/workflows/fuzz-nightly.yml`，动作 SHA 与 `ci.yml` 相同。驱动说明见 [harness README](../../../experiments/docx-robustness/README.md)。夜间读取器 fuzz 的要求见 [QUALITY.md](../QUALITY.md)，读取器 fuzz 的需求条目是 SPEC 的 QUALITY-005，见 [SPEC.md](../SPEC.md)。
