# W03：单字体横向段落断行切片

分配：DeepSeek Harness Desktop；分支 codex/w03-line-layout。T105 的限定算法切片，不依赖 W02 的图形实验完成。

## 目标

复用 LoadedFont、FontKey、shape、ShapedRun 和 PageScene，做无 UI、单字体、从左到右的横向段落在指定 EMU 宽度下断行。现有字体/整形/PDF 链路不变。

## 本轮工作

1. 新 line.rs 定义增量接口、行源范围/几何与诊断，区分 UTF-8 byte、scalar 和 glyph cluster。
2. 优先接受调用者提供的合法断点集合，或清楚声明基础中文/空格子集；不凭空承诺完整 UAX #14/Word 规则。新增 Unicode 依赖先提案。
3. 不拆 UTF-8 字符或整形 cluster，不按字符数估宽；候选行使用已有 shape 重新整形并核对宽度，不把整段字形切片当成跨行整形等价。
4. 明确处理空段、连续空白、组合字符、emoji、超长无断点、单 cluster 超宽、零/负宽度；有界诊断，不死循环、不丢字、不静默截断。
5. 输出能转换为既有 GlyphRun/PageScene，保留原文范围、EMU 几何与字体身份。页面安排仅示例，正式分页/快照/命中后续接入。
6. 测试错断/丢字/越界/推进；纯算法尽量无字体依赖，字体集成记录来源/散列/face，缺字体明确 skipped，不把全跳过当端到端通过。

不做 GUI/IME、复杂双向/竖排、表格、fallback、整体样式解析或 Word 行为猜测。

## 文件边界

- crates/joyword-layout/src/line.rs；src/lib.rs 仅增量导出。
- 该 crate 的 tests/line*.rs、examples/line*.rs。
- 该 crate Cargo.toml 仅引用已有 workspace 依赖；不新增外部版本。
- docs/coordination/reports/W03.md。

根 Cargo 文件和现有 font.rs/shape.rs/scene.rs、PDF/model/engine 不改。若确实需变动锁文件，报告原因由协调者接入。

## 验证与交接

运行 cargo test -p joyword-layout --locked、cargo test -p joyword-pdf --locked、cargo fmt --all -- --check、cargo clippy -p joyword-layout --all-targets --locked -- -D warnings、python3 scripts/check_docs.py。

报告公开签名、断点契约、源/字形范围映射、子集和字体测试实际执行量及完整 T105 缺口。提交/推送分支，等待审查。
