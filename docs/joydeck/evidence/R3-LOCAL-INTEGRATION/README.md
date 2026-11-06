# JoyDeck R3 本地整合与页脚接入评估

2026-11-06，Linux x86_64。基于主干 `7b37ad4c884706087983d352d4a11148040de02a`，在本地 `feat/joydeck-r3-integration` 非快进合并以下分支；不推送、不创建 PR、不合入 main。

| 切片 | 来源提交 |
| --- | --- |
| 富剪贴板隐藏内容及表格回退保护 | `6ab3f94aca9fb414016c0d1e2bb315180d856980` |
| 字符输入优先级 | `0a75f6f33a5d8ea606f89d75ffc9c78d63c79708` |
| 清除字符格式保留链接和校对语义 | `de1ffe92871c55300754295268d2b51f7342f4d4` |
| 页眉／自动日期导入摘要 | `d28049dc419031d255fa0fac3e059725d4cdb6a9` |
| 批量更新已有普通页脚命令 | `522ccc1134b3ca3056b83144c150a703ddf61af4` |

五个来源提交均已验证为整合 HEAD 的祖先。合并冲突涉及 STATUS/TASKS 文档和 Shell 文件末尾的两组独立新增测试，按任务内容保留两侧；未以任一侧覆盖另一侧。TASKS 的 98 个任务行无重复。源码中按键分流和剪贴板入口自动合并，组合检查验证同一版本。

## 页脚 UI 接入边界

当前可接入的能力应准确命名为“更新已有页脚文字”，对应真实 `set-existing-footer-text` 命令。界面应展示作用范围及未支持对象，先走 `Session::validate`，再一次提交；不能把它标为“为所有页面添加页脚”。缺少页脚、复杂／锁定页脚和预算超限必须显示引擎实际错误。真实界面还需复验当前页、选择、单步撤销与保存重开。

仓库 [UI-PARITY](../../UI-PARITY.md) 要求：“后续 UI 改动须先观察真实产品，再实现、构建、通过真实窗口操作并对照截图”。当前环境没有本批真实窗口／参照产品验收，因此本批未新增按钮或对话框；此前后端测试不能冒充 UI 完成。

## 新增缺失页脚的安全条件

已检查 `Shape::ph`、`Presentation::master_for`、`resolve::parents` 和 `resolve::xfrm`。占位符可继承版式／母版样式，但不能直接复制来源形状：其链接、标签、扩展 XML 等可能含来源部件的关系。现有 `xfrm` 在缺少有效父级时还有通用几何回退，该回退不适合作为自动新增页脚的放置依据。

此外 `HeaderFooter` 是导入摘要，当前没有完整表达 `p:hf` 可见性继承和每页隐藏策略。补建会改变此前没有页脚的页面；必须先明确目标页和标题页策略，汇总标志不能判断哪些页面需要新增页脚。

下一实现切片应满足：明确目标页列表；已有页脚完全不改；缺失页只接受唯一且有效的版式页脚来源；拒绝缺失／歧义／分组／隐藏或未能可靠解析的继承；不复制外来关系或未知 XML；使用新 ID 并纳入单次事务；验证版式继承前后、源包无关部件、撤销和公开 PPTX 重开。完整支持前不使用猜测坐标或通用页脚模板替代来源布局。

本批未新增“补建页脚”命令。现有命令、模型摘要修复和所有其他 R3 切片已整合，真实设备验收仍开放。

## 组合验证结果

- `cargo test -p joydeck-engine -p joydeck-shell -p joydeck-format -p joydeck-pptx`：**203 通过**，0 失败、0 忽略。
- 直接包含生产 Linux 剪贴板模块的轻量 Cargo 测试宿主：**5 通过**；重新编译并包含生产按键分流模块的 Rust 测试宿主：**3 通过**。两者均为无桌面测试，不计作系统输入验收。
- `RUST_FONTCONFIG_DLOPEN=1 cargo clippy -p joydeck-engine -p joydeck-shell -p joydeck-format -p joydeck-pptx -p joydeck-ui-gpui --all-targets -- -D warnings`、`cargo fmt --all --check`、`python3 scripts/check_docs.py` 通过。
- 测试和检查关闭 debug 信息／增量编译；Fontconfig 动态加载配置仅用于缺少开发文件的环境编译检查。验证源码提交及日志摘要见 [verification.json](verification.json)。所有分支均保留，JoyWord 整合分支未改动。
