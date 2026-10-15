# C07：真实输入、选择与 IME 组合态

- 执行者：当前 Codex 协调者，单人执行。
- 基线：cb219b31c1d2ed4c8ba56b0df80be4815bc3ce09（C06，含 EBDT 回归修正）。
- 分支：codex/coordination-text-input。
- 独立目录：/Users/willm/.codex/worktrees/joyoffice-coordination/joyoffice。
- 对应：T102，EDIT-002／EDIT-003／EDIT-004／EDIT-005／PLATFORM-004；T003 的输入接口与自动验证。

## 交付

以现有 engine 为唯一提交入口，将平台 UTF-16 范围与正文段落／标量位置严格互转，支持普通文本替换、换段、跨段选择、字素簇移动／删除、撤销／重做。组合预编辑仅存在临时显示快照，提交一次事务、一次撤销；取消／失焦不提交，迟到或越界／半 surrogate 范围不伪造合法位置。读取范围、标记／选择范围、候选框与命中位置对接 GPUI 的公开 EntityInputHandler。

正式画布接入输入、光标、组合下划线及选择反馈，后台重排／CPU 呈现按显示请求身份验证，不以旧场景映射新文本。编辑命令继续经同一注册表；无保存能力时显式提示暂存和关闭丢弃／取消，不开放伪保存。GUI 已由用户暂缓，自动验证、接口接入与真实 IME 人工验收分别记录，不标 T102 done。

## 边界裁决

协调者批准 shell 增量 input 模块、DOCX 适配（保留 backing／只读限制）、app 输入与画布、命令和文案、相关测试／公共记录／CI；render 的请求门增加默认类型参数以支持排版前的显示修订键，保留原默认 API；文案检查器扫描全部 app Rust 模块。现有 model／OPC／字体／整形／PageScene API 不变；engine 保持已有 apply 行为，仅新增 apply_with_mapped_selection 入口，分开事务前选择与需要映射至提交后的选择，供正确 undo／redo；另增只读 history 预览用于在撤销提交前检验输入预算，不重复实现事务或撤销。使用锁文件已有精确 unicode-segmentation 1.13.3，输入实现无外部包新增／升级，GPUI 来源不变。后续 CI 命中同日公布的 RUSTSEC-2026-0346–0349，协调者另批准仅将间接 rustls 0.23.45 精确更新为 0.23.46（锁文件版本／校验和），其他包和依赖边不变，不新增 advisory ignore。

## 验证

组合多次更新／部分替换／取消／提交／unmark 与一次撤销，非法 UTF-16 与候选选择、跨段／空段、emoji ZWJ／组合／旗帜／连字，版本竞争、只读和 locked DOCX、超预算与失败原子性；独立字符串参考模型的随机替换与撤销，真实 engine 及布局／CPU 图像链路，陈旧画布与选择拒绝、关闭提示逻辑。运行相关及全默认成员／release、应用无窗口测试、fmt／clippy、文档／工具、交叉检查和绑定源码的远程 CI。GUI、候选框实际定位、系统拼音和焦点行为必须留待人工验证。
