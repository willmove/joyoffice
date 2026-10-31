# 历史融合准备记录

当前执行计划与实际进度参见 [Monorepo 迁移计划](../joyoffice/MONOREPO-MIGRATION.md)及[迁移结果](../joyoffice/MONOREPO-RESULT.md)。以下是导入前独立目录的准备记录。

# JoyOffice 融合准备

2026-10-31。本目录没有 Git 元数据；本轮未导入 JoyOffice workspace，也未改写既有 M0–M14 验收结论。

依赖保持 `gpui-kit =0.8.1`、锁定 `gpui-pre 0.3.10`、`image =0.25.9`。JoyOffice 本轮向这些版本对齐。Kit 默认 component/assets 与本产品保持一致；尚未统一其余全部传递依赖。

本目录新增 `scripts/check-repository.py`，显式登记 `apps/joydeck` 与 `xtask`，拒绝未登记包及未经审阅的内部／path 依赖，要求包继承 workspace `unsafe_code = forbid`，检查 Actions 完整 SHA。使用 Python 3.11+；CI 新增 repository 作业，Python 固定 3.12，native 作业依赖它。执行：

```sh
python3 scripts/check-repository.py
python3 -m unittest discover -s scripts/tests
cargo xtask check
cargo test --locked --no-default-features
```

checkout v4、rust-cache v2、upload-artifact v4 已从远端 tag 解析并固定提交；dtolnay 的 master 引用改为固定 Rust 1.98.1 的 rustup 命令。当前两包都继承 workspace lints。crate 登记与 SHA 检查有反例测试。

这只是独立目录的准备门禁，不能替代 JoyOffice 的 LAYERS / DEV_LAYERS、逐文件 IO 白名单与 panic baseline。以 JoyOffice 当前扫描器检查此目录：joydeck 未登记，84 处文件系统／进程访问、407 处生产范围 panic-family 命中（含应用内 smoke / bench）。这些是扫描命中，不是运行时故障次数。当前不自动接纳新 baseline、不添加目录级豁免。

建议先将现有无界面库提取为真实的过渡 joydeck-core，GUI 与 CLI 独立入口后并入一份 workspace / lock / CI；再断开 document 与 pptx/preview、table 与 edit 等循环，将纯模型、编辑事务、格式、scene、导出、shell 拆为产品 crate。OPC / DrawingML 等标准层逐步复用 joyoffice-*，不依赖 joyword-*。现有序列化 schema、未知部件保留、输入预算和回归用例持续验证。

完整依赖来源与目标映射记录在 JoyOffice 仓库 `docs/joyoffice/JOYDECK-INTEGRATION.md`。本次本机自动检查结果随该报告记录；远端 CI、三平台 GUI、信创真机与架构拆分仍未完成。
