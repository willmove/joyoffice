# W04 证据索引

测试源码 2d4093b885eaf5928929f972ca25c48bd330082e，采集日期 2026-10-14（Asia/Shanghai）。

- [环境](environment.json)：系统/GPU/工具链、Metal 缺口、字体身份、已检测 Office。
- [独立 DOCX 文字提取](textutil.json)：五个原创样例输入散列、命令、退出码与原文。
- [开发构建](app-build.log)：dev-runtime-shaders 构建，不是发行/GUI 证明。
- [核心测试](core-tests.log)、[clippy](clippy.log)：集成源码复验。
- [回归修复前](docx-regressions-before.log)、[修复后](docx-regressions-after.log)：未声明 MCE 前缀和非法 XML 控制字符的具体失败/通过。
- [场景生成](unified-scene.log)、[PDFKit 读回](pdf-readback.log)、[期望场景](unified/expected.json)、[pypdf 读回](unified/pypdf-readback.json)。两种读回均限文字，PDFKit 输出的零几何偏差不能解释为实测几何。

本机诊断 PDF/PNG 在 W04 worktree 的 target/w04-validation 中；含系统字体子集，未作为公开仓库资产提交。GUI/Office 对照和跨平台真机尚未执行，详见[报告](../../reports/W04.md)。
