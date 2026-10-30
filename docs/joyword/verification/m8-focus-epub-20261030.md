# 焦点、窗口关闭与 EPUB 第一切片（2026-10-30）

本轮从最新 main `0a6a5fdfd0c7261058b61376f2f6d4e386f38cb1` 建立 `codex/m8-focus-lifecycle-20261030`，保留前轮全部未提交修改。用户要求优先修复焦点角色和窗口句柄诊断，同时继续 M8。当前全仓 **15 done／38 in_progress／13 planned**，M8 **1 done／1 in_progress／7 planned**。T807 保持 done，T806 从 planned 进入 in_progress；M7、M8 与 1.0 未整体验收。

## 已实现

文件菜单可复现 GPUI FocusId(4v1) 缺少角色；现在菜单、下拉框、查找／替换和格式字段发布稳定标识、名称及真实控件角色。缩放支持有边界的原生数值设置，文本字段支持有界单行替换；拒绝非法值时不改状态。补齐菜单键盘激活与控件位置锚定。正文 TextPattern／选择接口保留，不能据此宣称完整读屏已通过。

Windows 原生关闭曾让系统先销毁 HWND，随后 GPUI 再处理激活／拖放注销和销毁，产生 window not found 与无效句柄。现在 Windows should-close 阻止系统先销毁，转入现有保存／取消／退出流程；最后一个窗口通过应用退出完成一次关闭。日志增加相对时间和源文件／行号，保留错误，不过滤诊断。使用的是精确固定的 registry gpui-pre 0.3.9。

新增只读 `joyword-epub` 和 shell 共用输出层，GUI／CLI 共用 EPUB 3.3 计划；包含重排正文、标题导航、基础样式、表格与 PNG／JPEG。差异预检、显式接受、不覆盖输出、临时读回、取消和原件保护已实现，无新增第三方依赖。输入预算、拒绝边界与命令见 [指南](../guide/epub.md)。依据 [W3C EPUB 3.3](https://www.w3.org/TR/epub-33/) 独立实现，不声明 EPUB 无障碍或 PDF/A 合规。

## 自动与独立验证

核心新增 9 项 EPUB 测试、1 项 shell 发布测试和 2 项 CLI 子进程测试，分别检查容器、Unicode、导航、图片、元数据、预算、危险链接与拒绝路径，以及零写入计划、显式接受、确定性和原件散列。另补原生缩放、字段替换及键盘上下文隔离三项回归。

首轮 workspace **1249 项通过（含应用 89 项）**；随后 GUI 发现菜单 Enter 串入正文，补充回归并修复工具栏键盘上下文后，最终应用 **90 项通过**。AI shell **291 项**、Python **116 项**、恢复实验 **15 项**、稳健性实验 **3 项**、强制独立 qpdf 导航读取 **15 项**通过；fmt、workspace／AI clippy、无默认 feature 构建、cargo-deny 0.20.2、文档／验收／架构门禁和 release 构建通过。未将重复运行累计成新增测试。

EPUB 新依赖边使两个独立实验锁文件需要补登记本地 crate，所有第三方版本和 checksum 保持原值。稳健性在干净私有快照执行 **81 smoke＋200 次变异**，0 崩溃／超时，分别为 43 ok／38 rejected 和 50 ok／150 rejected；Windows 没有 Job Object 内存上限后端，该活动不是覆盖引导 fuzz 或完整安全证明。见 [smoke](../validation/20261030-focus/robustness/smoke-summary.json)、[变异](../validation/20261030-focus/robustness/fuzz-summary.json)。

最终命令、原始日志和散列在 [检查汇总](../validation/20261030-focus/checks-summary.json)。首次 Python／panic 门禁失败由测试错误使用 unwrap 引起，已改为传播解析错误，未放宽 baseline；cargo-deny PATH 缺失及两次错误 feature 名命令均保留失败记录，最后改用缓存工具和实际 navigation 测试目标通过。旧进程仍占用 exe 时一次构建失败，关闭测试会话后重建通过；这些尝试没有被改写成成功。

独立 EPUBCheck 5.4.0 对实际 CLI 和 GUI 导出均返回 **0 fatal／0 error／0 warning**，见 [CLI 原始报告](../validation/20261030-focus/epub/epubcheck-cli.json) 和 [GUI 原始报告](../validation/20261030-focus/epub/epubcheck-gui.json)。使用本机既有 OpenJDK 24.0.1；官方工具 ZIP SHA-256 为 `33350c61038e71dfb3d45a76aed04bf5481e6d5500cb780f6e98db8bbd15a28c`，与 GitHub release digest 相符，仅用于本地验证，不随产品打包。来源：[官方发布](https://github.com/w3c/epubcheck/releases/tag/v5.4.0)。

独立标准库 ZIP／XML 读回逐段比较 DOCX 与 XHTML 的 Unicode 和软换行，检查两个目录目标、表格阅读顺序和一张 PNG 原字节；GUI 四段信件、源文件 SHA-256 及零覆盖同样通过。见 [读回](../validation/20261030-focus/epub-readback.json)。这两份文件的零错误不等于全格式输入或完整 EPUB 无障碍合规。

## GUI 与证据

[原生观察与截图](../validation/20261030-focus/gui/observations.jsonl) 包含 **23 条关键观察**。基线点击 File 可复现缺失角色；增加日志位置后，原生关闭错误分别定位到 GPUI 激活回调和 Windows 拖放注销／窗口销毁。基线二进制散列未在覆盖前捕获，不把它当成已知散列的发布构建。

第一 release 候选 `fd21d07e…` 验证发现菜单 Enter 使文档 dirty，观察 07 当时的预期说明被观察 08 明确纠正，失败保留。修复根因是根视图的 JoyWordInput 上下文也匹配工具栏；现在工具栏加入 JoyWordControl，正文绑定显式排除此上下文。最终候选 09 复验 Enter 后四段原文、干净状态与禁用撤销保持。

最终候选实际完成 EPUB 预检取消／新文件保存、Unicode 查找 ValuePattern、缩放 RangeValue 110 驱动画面、字体字段 ValuePattern=16 后取消、正文测试编辑、Alt+F4 未保存提示→取消→精确文本保留→撤销→应用关闭命令。独立解包候选干净启动后 Alt+F4 退出；两个实际进程均消失，最终 [应用日志](../validation/20261030-focus/gui/application-stderr.log) 与 [解包日志](../validation/20261030-focus/gui/unpacked-stderr.log) 无缺失角色、window not found 或无效句柄错误，详见 [生命周期读回](../validation/20261030-focus/gui/lifecycle.json)。tiny-skia 空路径警告保留，未过滤。撤销后会话保守地保持 dirty，本轮没有把它声称为干净状态。

复杂 DOCX 编辑会话仍可能以 UnsupportedBody 拒绝，原画布不被替换；CLI 的表格／图片导出验证不能替代桌面对应路径。未发现已安装独立 EPUB 阅读器，未将 EPUBCheck 当成阅读系统 GUI 验收。完整实际读屏与所有焦点宣读行为仍未认定通过。

最终 release 二进制 SHA-256：JoyWord `451a58bdd2205b47469965b29366a242c306eb648f13312ddf6b64a47aaf2aea`；CLI `0cfd654c374b13e6a8fa38b98dbd77bf342ab08d36976947b98fdd003fc34288`。

## 交付与源码身份

最终私有验证快照 `26e148f394e0dcaabd88bf8ee05dc7e5a58389df`，crates tree `535d7a01ce245a48d0c8ae8a7780fee8d67aee47`、apps tree `632b8d73c2fd005b64e161b4caf211648359dbdb`。它是本地证据锚点，未推送或作为交付提交；主工作区保留原有修改，主索引不变。文档和本轮证据在快照后收尾，最终源码树仍 [逐项匹配](../validation/20261030-focus/source-match.json)。

生成两次字节相同的未签名便携包，33,390,748 字节，SHA-256 `3e2f550f8706babab99b5363d036c436445a03f731b6e56f9f339e5ca4869863`。全部 599 成员散列、SPDX 2.3 官方 schema（382 包／919 关系）及解包 CLI 计划零写入通过，解包 GUI 原生关闭也已实测。见 [包读回](../validation/20261030-focus/package-readback.json)。18 项依赖 NOTICE 与 54 个发布门槛仍待完成，stable_release_approved=false；确定性打包不等于独立可重复构建或签名发布。

证据文件清单及 SHA-256 见 [manifest](../validation/20261030-focus/manifest.json)，先前 M7 与邮件合并证据均保留。

## 仍未完成

T806 的 PDF/A、独立 EPUB 阅读系统、辅助技术阅读顺序与检索验收仍未完成。三端完整读屏／焦点、真实打印、完整平台性能矩阵、签名安装及依赖 NOTICE 审计继续登记在 M7；信创真机非阻塞待补验。其他七个 M8 任务仍 planned，未以一次切片覆盖全部任务。
