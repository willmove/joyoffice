# T503 语言工具与视图自动化证据（2026-10-28）

## 交付范围
1. **多视图投影与不可变视图切换（`ViewMode`，PLATFORM-003）**：
   - 在 `joyword-layout` 中引入 `ViewMode` 枚举（`Print` 页面视图、`Draft` 草稿视图、`Outline` 大纲视图、`Reading` 阅读视图）。
   - 视图参数嵌入 `ViewOptions` 与 `LayoutKey` 参与缓存键计算与增量重排。
   - 大纲视图（`ViewMode::Outline`）实现标题驱动投影：保留 Heading 样式段落，正文主体段落折叠投影为空文本，实现大纲层级视图切换。
   - 视图切换核心契约验证：不同视图间切换绝不修改底层 `Document` 树，不递增版本，不产生伪修订。
2. **离线拼写检查与中英文校对工具（`joyword-engine::spell`，EDIT-008）**：
   - 纯本地自研拼写检查器（`OfflineSpellChecker`），内置核心标准词汇表，支持用户自定义字典（`add_word`）与会话忽略（`ignore_word`）。
   - 基于 Levenshtein 动态规划编辑距离算法的离线拼写建议推荐（`suggest`）。
   - 中文标点规范化与校对规则（连续重复标点检测如 `，，`、`。。`，未闭合书名号 `《` 与括号配对平衡检查）。
   - 严格隐私契约：完全离线执行，无任何网络依赖，无自动上传，AI 关闭时语言能力依然完整可用。

## 自动测试命令与结果
```bash
cargo test -p joyword-engine --test spell_tests
cargo test -p joyword-layout --test langview_tests
```
3 项综合测试全部通过：
- `test_offline_spell_check_and_suggestions`：英文单词离线纠错、候选建议、用户字典及段落级错误定位测试通过。
- `test_chinese_proofing_rules`：中文重复标点与未配对书名号规则拦截测试通过。
- `test_view_mode_switching_does_not_mutate_document`：Print/Draft/Outline/Reading 四视图切换与大纲投影有效性，及文档状态完全不变性测试通过。

## 未验证项
- 人工在图形界面（GUI）中点击切换草稿视图、大纲视图和阅读视图并人工核对视觉效果尚未执行。
- 扩展语言（如德文、法文复合词与音符变音）词典尚未载入。
