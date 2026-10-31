# 需求与实施追踪矩阵

基线：稳定 FR/NFR、父任务与增量子项映射至20组基础验收流程；M0 Windows 预览已实施；M1 基础只读浏览已实现，完整阶段待验收；M2功能实现补齐，Windows自动/原生验证通过，完整退出待验收；macOS 最终包 M0–M3 原生 adapter 回归、系统拼音与 35 项测试通过（见[24](24-m3-macos-verification.md)）；M3 文字预览已交付，完整 M3 未通过（遗留见[23](23-m3-implementation-report.md)）；M4 核心 Scene/GPUI 切片已实现，45 项测试及本机 release 验证见[26](26-m4-implementation-report.md)，完整 M4 仍待验收；M5图片创作增量见 [27](27-m5-implementation-report.md)，完整M5未验收；M6 文稿设计增量见 [28](28-m6-implementation-report.md)，完整M6未验收；M7 文件交付增量见 [29](29-m7-implementation-report.md)，完整M7/MVP未验收；M8表格与工作区改造本机受支持范围done，79项测试、GUI/系统拼音/PowerPoint编辑往返及最终包见 [30](30-m8-implementation-report.md)；M9基础图表/图示本机受支持范围done，见 [31](31-m9-implementation-report.md)；M10软件子项与单屏双窗口验证done，完整双屏/硬件/AT-12门禁开放，见 [32](32-m10-implementation-report.md)；M11/M12已交付本机开发切片，完整阶段仍开放，见[33](33-m11-implementation-report.md)、[34](34-m12-implementation-report.md)。状态以实施记录及本表更新为准。需求原文见 [功能规格](03-functional-spec.md)，任务见 [清单](11-task-backlog.md)，验收见 [手册](12-acceptance-playbook.md)。

每项至少关联一组具体任务和验收。M14的P2项先映射立项任务与基础回归，其新增专项验收在单项立项时补齐；基础AT不能作为尚未设计的高级功能已通过证据。阶段/优先级以03为准。

## 需求映射

| 要求 | 优先级与首次阶段 | 关联任务 | 验收 | 领域规格 | 状态 |
|---|---|---|---|---|---|
| FR-APP-01 | P0 / M0 | T-M0-01、T-M0-03、T-M0-07 | AT-01 | [04](04-architecture-spec.md)、[05](05-gpui-ui-spec.md)、[09](09-quality-platform-release.md) | Windows已实现；跨平台待验收 |
| FR-APP-02 | P0 / M1 | T-M1-05 | AT-02 | [05](05-gpui-ui-spec.md) | Windows只读切片已验证；完整M1待验收 |
| FR-APP-03 | P0 / M2 | T-M2-01、T-M2-05 | AT-03、AT-05 | [04](04-architecture-spec.md)、[05](05-gpui-ui-spec.md) | Windows实现与自动验证完成；完整验收见22，仍待补齐 |
| FR-APP-04 | P1 / M6 | T-M6-01 | AT-09、AT-14 | [04](04-architecture-spec.md)、[05](05-gpui-ui-spec.md) | 同文稿共享Session/undo、独立选择/当前页和独立文稿窗口已实施；完整多文档/关闭恢复矩阵仍待，见28 |
| FR-APP-05 | P1 / M0→M6 | T-M0-03、T-M6-09 | AT-01、AT-05 | [05](05-gpui-ui-spec.md) | M0切换已实现；M6待实施 |
| FR-APP-06 | P0 / M3 | T-M3-04 | AT-04、AT-05 | [05](05-gpui-ui-spec.md) | 画布/属性守卫及macOS系统拼音/原生回归通过；完整焦点/读屏待验收，见24 |
| FR-APP-07 | P1 / M6 | T-M6-09、T-M13-04 | AT-05、AT-19、AT-20 | [05](05-gpui-ui-spec.md)、[09](09-quality-platform-release.md) | 未开始 |
| FR-APP-08 | P1 / M13 | T-M13-06 | AT-20 | [05](05-gpui-ui-spec.md)、[09](09-quality-platform-release.md) | 未开始 |
| FR-DOC-01 | P0 / M2 | T-M2-03 | AT-03 | [04](04-architecture-spec.md)、[07](07-format-compatibility-spec.md) | Windows实现与自动验证完成；完整验收见22，仍待补齐 |
| FR-DOC-02 | P0 / M1 | T-M1-03、T-M1-06 | AT-02、AT-15 | [07](07-format-compatibility-spec.md) | v1/PPTX子集已验证；进度细化/DocumentStore待补 |
| FR-DOC-03 | P0 / M2 | T-M2-07 | AT-06、AT-14 | [04](04-architecture-spec.md)、[07](07-format-compatibility-spec.md) | Windows实现与自动验证完成；完整验收见22，仍待补齐 |
| FR-DOC-04 | P0 / M2 | T-M2-07 | AT-06 | [04](04-architecture-spec.md)、[07](07-format-compatibility-spec.md) | Windows实现与自动验证完成；完整验收见22，仍待补齐 |
| FR-DOC-05 | P0 / M2 | T-M2-02、T-M2-09 | AT-03、AT-04 | [04](04-architecture-spec.md) | Windows实现与自动验证完成；完整验收见22，仍待补齐 |
| FR-DOC-06 | P0 / M2 | T-M2-08 | AT-06 | [07](07-format-compatibility-spec.md) | Windows实现与自动验证完成；完整验收见22，仍待补齐 |
| FR-DOC-07 | P0 / M7 | T-M7-03 | AT-06、AT-14 | [07](07-format-compatibility-spec.md) | 沿用文件指纹保护，新增导出目标不覆盖/外部修改冲突与取消清理；完整重载/明确覆盖及故障/竞争矩阵仍待，见29 |
| FR-DOC-08 | P1 / M6 | T-M6-07 | AT-09 | [04](04-architecture-spec.md)、[07](07-format-compatibility-spec.md) | 标题/作者/说明、页备注和节已原生往返；完整属性/工作区仍待，见28 |
| FR-SLD-01 | P0 / M2 | T-M2-03 | AT-03 | [05](05-gpui-ui-spec.md) | Windows实现与自动验证完成；完整验收见22，仍待补齐 |
| FR-SLD-02 | P0 / M2 | T-M2-03 | AT-03 | [05](05-gpui-ui-spec.md) | Windows实现与自动验证完成；完整验收见22，仍待补齐 |
| FR-SLD-03 | P1 / M6 | T-M6-07 | AT-09 | [05](05-gpui-ui-spec.md) | 节创建/重命名/归页/删除保留页已实施；折叠/整体移动仍待，见28 |
| FR-SLD-04 | P1 / M6 | T-M6-07 | AT-09 | [05](05-gpui-ui-spec.md) | 大纲导航与多行备注编辑已接入；完整排序/备注专用视图仍待，见28 |
| FR-SLD-05 | P0 / M1→M4 | T-M1-05、T-M4-06 | AT-02、AT-07 | [05](05-gpui-ui-spec.md)、[06](06-render-text-spec.md) | M1翻页/fit/缩放/滚动及 M4 Scene 绘制已实现；指针锚点/标尺仍开放，见26 |
| FR-SLD-06 | P1 / M6 | T-M6-08 | AT-09、AT-10 | [04](04-architecture-spec.md)、[07](07-format-compatibility-spec.md) | 独立PageTransfer重映射设计/节/媒体IDs、源格式/目标主题入口已实施；完整交换矩阵仍待，见28 |
| FR-GEO-01 | P0 / M2 | T-M2-04、T-M5-05 | AT-03、AT-08 | [05](05-gpui-ui-spec.md)、[06](06-render-text-spec.md) | Windows实现与自动验证完成；完整验收见22，仍待补齐 |
| FR-GEO-02 | P0 / M2 | T-M2-04 | AT-03 | [04](04-architecture-spec.md)、[05](05-gpui-ui-spec.md) | Windows实现与自动验证完成；完整验收见22，仍待补齐 |
| FR-GEO-03 | P0 / M2→M5 | T-M2-05、T-M5-06 | AT-03、AT-08 | [05](05-gpui-ui-spec.md) | 六向对齐/横纵分布已实现；组/锁定/翻转仍待，见27 |
| FR-GEO-04 | P1 / M5 | T-M5-06 | AT-08 | [05](05-gpui-ui-spec.md)、[06](06-render-text-spec.md) | 未开始 |
| FR-GEO-05 | P1 / M5 | T-M5-07 | AT-08 | [04](04-architecture-spec.md)、[06](06-render-text-spec.md) | 未开始 |
| FR-GEO-06 | P1 / M5 | T-M5-05 | AT-08、AT-15 | [05](05-gpui-ui-spec.md)、[06](06-render-text-spec.md) | 未开始 |
| FR-GEO-07 | P0 / M2→M6 | T-M2-05、T-M6-04、T-M6-05 | AT-03、AT-07、AT-09 | [05](05-gpui-ui-spec.md)、[06](06-render-text-spec.md) | 未开始 |
| FR-GEO-08 | P1 / M5 | T-M5-07 | AT-08、AT-19 | [05](05-gpui-ui-spec.md) | 未开始 |
| FR-TXT-01 | P0 / M3 | T-M3-01、T-M3-02 | AT-04 | [05](05-gpui-ui-spec.md)、[06](06-render-text-spec.md) | 画布文字输入/字素/选区预览已实现；统一布局、视觉行/逐词导航待完成，见23 |
| FR-TXT-02 | P0 / M3 | T-M3-05 | AT-04 | [06](06-render-text-spec.md) | 局部粗斜体/字体/颜色和 runs 往返已实现；混合字号及完整装饰待完成，见23 |
| FR-TXT-03 | P0 / M3 | T-M3-05 | AT-04 | [06](06-render-text-spec.md) | 未开始 |
| FR-TXT-04 | P0 / M3 | T-M3-03、T-M3-04、T-M3-07 | AT-04、AT-05 | [05](05-gpui-ui-spec.md)、[06](06-render-text-spec.md) | macOS系统拼音预编辑/提交/取消/单次撤销通过；其余输入法/平台待验收，见24 |
| FR-TXT-05 | P0 / M3 | T-M1-02、T-M3-06 | AT-04、AT-15 | [06](06-render-text-spec.md) | 按页/对象缺字体提示已实现；逐字覆盖/替代列表/字体失效待完成，见23 |
| FR-TXT-06 | P1 / M6 | T-M6-06 | AT-04、AT-09 | [06](06-render-text-spec.md) | 未开始 |
| FR-TXT-07 | P1 / M6 | T-M6-06 | AT-09、AT-10、AT-11 | [06](06-render-text-spec.md)、[07](07-format-compatibility-spec.md) | 页码/固定日期占位符按共同上下文解析；链接与完整字段尚未完成，见28 |
| FR-TXT-08 | P1 / M6 | T-M6-06 | AT-09 | [05](05-gpui-ui-spec.md)、[06](06-render-text-spec.md) | 文字查找定位/全局替换/字体替换和单次undo已实施，领域/本机验证见28；完整平台人工验收仍待 |
| FR-TXT-09 | P1 / M13 | T-M13-04 | AT-04、AT-05、AT-20 | [06](06-render-text-spec.md)、[09](09-quality-platform-release.md) | 未开始 |
| FR-TXT-10 | P2 / M14 | T-M14-03 | 后续专项；基础04 | [06](06-render-text-spec.md) | 未开始 |
| FR-IMG-01 | P0 / M5 | T-M5-01 | AT-08、AT-15 | [06](06-render-text-spec.md)、[07](07-format-compatibility-spec.md) | PNG/JPEG有界导入/EXIF/去重/持久化及本机窗口通过；完整资源/平台仍待，见27 |
| FR-IMG-02 | P0 / M5 | T-M5-02 | AT-08 | [05](05-gpui-ui-spec.md)、[06](06-render-text-spec.md) | 非破坏crop/reset/undo/重开通过；preview/cancel/aspect仍待，见27 |
| FR-IMG-03 | P0 / M5 | T-M5-03、T-M5-08 | AT-08 | [04](04-architecture-spec.md)、[05](05-gpui-ui-spec.md) | 图片/文字fallback、内部对象资源快照已实现，macOS adapter通过；外部rich/其余平台仍待，见27 |
| FR-IMG-04 | P1 / M5 | T-M5-04 | AT-08、AT-15 | [06](06-render-text-spec.md)、[07](07-format-compatibility-spec.md) | 未开始 |
| FR-IMG-05 | P1 / M6 | T-M6-05 | AT-08、AT-09 | [06](06-render-text-spec.md) | 未开始 |
| FR-TBL-01 | P1 / M8 | T-M8-01、T-M8-02、T-M8-03 | AT-16 | [05](05-gpui-ui-spec.md)、[06](06-render-text-spec.md)、[07](07-format-compatibility-spec.md) | 受支持表格范围本机done；native/公开PPTX、系统拼音与Office编辑闭环见30；外部复杂格式/三平台仍开放 |
| FR-TBL-02 | P1 / M8 | T-M8-01、T-M8-03 | AT-16 | [05](05-gpui-ui-spec.md)、[06](06-render-text-spec.md) | 受支持表格范围本机done；native/公开PPTX、系统拼音与Office编辑闭环见30；外部复杂格式/三平台仍开放 |
| FR-TBL-03 | P1 / M8 | T-M8-04 | AT-16 | [06](06-render-text-spec.md)、[07](07-format-compatibility-spec.md) | 受支持表格范围本机done；native/公开PPTX、系统拼音与Office编辑闭环见30；外部复杂格式/三平台仍开放 |
| FR-TBL-04 | P1 / M8 | T-M8-05 | AT-16、AT-10 | [07](07-format-compatibility-spec.md) | 受支持表格范围本机done；native/公开PPTX、系统拼音与Office编辑闭环见30；外部复杂格式/三平台仍开放 |
| FR-CHT-01 | P1 / M9 | T-M9-01 | AT-17 | [06](06-render-text-spec.md)、[07](07-format-compatibility-spec.md) | 七类基础图表/Scene/CPU/PDF已实施；面积非堆积、环形50%内径，完整类型/样式仍受31边界限制 |
| FR-CHT-02 | P1 / M9 | T-M9-02、T-M9-04 | AT-17、AT-10 | [05](05-gpui-ui-spec.md)、[07](07-format-compatibility-spec.md) | 网格/TSV/undo、原子预算与缓存/XLSX一致输出及导入核对；PowerPoint实际保存133重读，见31 |
| FR-CHT-03 | P1 / M9 | T-M9-03、T-M9-06 | AT-17、AT-10 | [06](06-render-text-spec.md)、[07](07-format-compatibility-spec.md) | 标题/图例/轴/标签/网格/调色板及未知复杂类型诊断/原包保留，见31 |
| FR-CHT-04 | P1 / M9 | T-M9-05、T-M9-06 | AT-17、AT-10 | [05](05-gpui-ui-spec.md)、[07](07-format-compatibility-spec.md) | 列表/流程/层级文本与布局、导入拓扑简化提示、普通形状转换确认/undo，非完整SmartArt引擎，见31 |
| FR-DSN-01 | P0 / M6 | T-M6-02 | AT-09 | [04](04-architecture-spec.md)、[06](06-render-text-spec.md) | 基础Layer继承/主题槽位/direct与CPU/Scene共源已实施；完整属性及工作区仍待，见28 |
| FR-DSN-02 | P1 / M6 | T-M6-03 | AT-09 | [04](04-architecture-spec.md)、[05](05-gpui-ui-spec.md) | 母版/版式复制重命名、引用保护、领域版式替代已实施；完整画布编辑器仍待，见28 |
| FR-DSN-03 | P0 / M6 | T-M6-02、T-M6-03 | AT-09 | [04](04-architecture-spec.md)、[06](06-render-text-spec.md) | 文字占位符kind/idx与应用/reset保留内容/ID已验证；非文字占位符及完整样本仍待，见28 |
| FR-DSN-04 | P1 / M6 | T-M6-04 | AT-09、AT-10 | [05](05-gpui-ui-spec.md)、[07](07-format-compatibility-spec.md) | 主题颜色/字体/纯色背景及页码/固定日期已实施；其他背景/页脚/模板仍待，见28 |
| FR-DSN-05 | P2 / M14 | T-M14-02 | 后续专项；基础09 11 | [07](07-format-compatibility-spec.md) | 未开始 |
| FR-ANM-01 | P1 / M10 | T-M10-02 | AT-12 | [08](08-show-media-automation-spec.md) | 7类基础效果/触发/延迟/重复/rewind确定性采样；受限直线路径和显式段落，见32 |
| FR-ANM-02 | P1 / M10 | T-M10-03 | AT-12 | [05](05-gpui-ui-spec.md)、[08](08-show-media-automation-spec.md) | 目标/排序/时间/段落面板、同采样器预览与停止还原已验证，见32 |
| FR-ANM-03 | P1 / M10 | T-M10-04 | AT-12 | [06](06-render-text-spec.md)、[08](08-show-media-automation-spec.md) | none/fade/push/wipe、缓存/CPU回退、公共参数往返与未知树保留，见32 |
| FR-ANM-04 | P1 / M10 | T-M10-05 | AT-12 | [06](06-render-text-spec.md)、[08](08-show-media-automation-spec.md) | 显式key、冲突拒绝、几何/opacity/同资源crop插值；内容变化降级，路径/逐字未支持，见32 |
| FR-ANM-05 | P2 / M14 | T-M14-02 | 后续专项；基础12 | [08](08-show-media-automation-spec.md) | 未开始 |
| FR-SHW-01 | P0 / M7 | T-M7-06 | AT-12静态 | [08](08-show-media-automation-spec.md) | 不可变快照全屏静态放映、隐藏过滤、导航/Esc与截图验证通过；跨平台/跨屏/物理输入仍待，见29 |
| FR-SHW-02 | P1 / M10 | T-M10-06 | AT-12 | [08](08-show-media-automation-spec.md) | 单屏下演示者/观众双窗口、当前实际采样/下一页/独立备注/计时导航通过；真实双屏待验，见32 |
| FR-SHW-03 | P1 / M10 | T-M10-07 | AT-12 | [08](08-show-media-automation-spec.md)、[09](09-quality-platform-release.md) | 选择/交换/丢失暂停软件路径已实现；真实双屏/DPI/镜像/热插拔未验收，见32 |
| FR-SHW-04 | P1 / M10 | T-M10-08 | AT-12 | [08](08-show-media-automation-spec.md) | 黑白屏/激光/墨迹另存副本或放弃真实GUI通过，原稿不变，见32 |
| FR-SHW-05 | P1 / M10 | T-M10-01、T-M10-08 | AT-12 | [08](08-show-media-automation-spec.md) | 原生内部链接/自定义顺序/循环/排练另存/自动翻页与返回状态已验证；PPTX链接/自定义顺序不输出，见32 |
| FR-SHW-06 | P1 / M10 | T-M10-01、T-M10-09 | AT-12、AT-14 | [08](08-show-media-automation-spec.md) | 单一monotonic时钟/暂停/代次移交已验证，长间隔暂停；真实睡眠/双屏仍待，见32 |
| FR-MED-01 | P1 / M11 | T-M11-01、T-M11-03 | AT-13 | [08](08-show-media-automation-spec.md)、[09](09-quality-platform-release.md) | 本机开发切片已实施，完整验收开放；见[33](33-m11-implementation-report.md) |
| FR-MED-02 | P1 / M11 | T-M11-04、T-M11-05 | AT-13、AT-12 | [08](08-show-media-automation-spec.md) | 本机开发切片已实施，完整验收开放；见[33](33-m11-implementation-report.md) |
| FR-MED-03 | P1 / M11 | T-M11-02、T-M11-05、T-M11-06 | AT-13、AT-15 | [08](08-show-media-automation-spec.md)、[09](09-quality-platform-release.md) | 本机开发切片已实施，完整验收开放；见[33](33-m11-implementation-report.md) |
| FR-MED-04 | P2 / M14 | T-M14-04、T-M14-05 | 后续专项；基础13 | [08](08-show-media-automation-spec.md) | 未开始 |
| FR-IO-01 | P0 / M7 | T-M2-06、T-M7-02、T-M7-04 | AT-06、AT-10 | [07](07-format-compatibility-spec.md) | PPTX原包保留与单行文字patch、静态独立输出/专有快照已实施；完整公开往返仍待，见29 |
| FR-IO-02 | P0 / M1→M7 | T-M1-06、T-M7-01、T-M7-03、T-M7-07 | AT-02、AT-10 | [07](07-format-compatibility-spec.md) | native与PPTX受限导入、源包持久化/关系闭包已实施；完整保留/公开模型仍待，见29 |
| FR-IO-03 | P0 / M7 | T-M7-05 | AT-11 | [07](07-format-compatibility-spec.md) | PNG/JPEG页范围/DPI/alpha及预算已实施；完整输出/平台验收仍待，见29 |
| FR-IO-04 | P0 / M7 | T-M7-05 | AT-11 | [07](07-format-compatibility-spec.md) | 栅格PDF含Unicode ActualText行映射/书签/备注已验证；精准选择/链接/讲义等仍待，见29 |
| FR-IO-05 | P1 / M12 | T-M12-10 | AT-11 | [07](07-format-compatibility-spec.md)、[09](09-quality-platform-release.md) | 本机切片已实施，原生/协议/格式验证通过，完整验收开放；见[34](34-m12-implementation-report.md) |
| FR-IO-06 | P1 / M12 | T-M12-09 | AT-11 | [06](06-render-text-spec.md)、[07](07-format-compatibility-spec.md) | 本机切片已实施，原生/协议/格式验证通过，完整验收开放；见[34](34-m12-implementation-report.md) |
| FR-IO-07 | P1 / M12 | T-M12-11 | AT-11 | [07](07-format-compatibility-spec.md)、[08](08-show-media-automation-spec.md) | 静态GIF/大纲/变体受支持范围done；见[34](34-m12-implementation-report.md) |
| FR-IO-08 | P2 / M14 | T-M14-01、T-M14-04 | 后续专项；基础10 11 | [07](07-format-compatibility-spec.md)、[08](08-show-media-automation-spec.md) | 未开始 |
| FR-REV-01 | P1 / M12 | T-M12-01 | AT-19 | [07](07-format-compatibility-spec.md)、[08](08-show-media-automation-spec.md) | 本机切片已实施，原生/协议/格式验证通过，完整验收开放；见[34](34-m12-implementation-report.md) |
| FR-REV-02 | P1 / M12 | T-M12-02 | AT-19、AT-05 | [05](05-gpui-ui-spec.md)、[07](07-format-compatibility-spec.md) | 本机切片已实施，原生/协议/格式验证通过，完整验收开放；见[34](34-m12-implementation-report.md) |
| FR-REV-03 | P2 / M14 | T-M14-06 | 后续专项；基础19 | [04](04-architecture-spec.md)、[07](07-format-compatibility-spec.md) | 未开始 |
| FR-REV-04 | P2 / M14 | T-M14-06 | 后续专项；基础14 18 | [04](04-architecture-spec.md) | 未开始 |
| FR-AUT-01 | P1 / M12 | T-M12-05 | AT-18 | [08](08-show-media-automation-spec.md) | 本机切片已实施，原生/协议/格式验证通过，完整验收开放；见[34](34-m12-implementation-report.md) |
| FR-AUT-02 | P1 / M12 | T-M12-04、T-M12-06 | AT-18、AT-14 | [08](08-show-media-automation-spec.md) | 本机切片已实施，原生/协议/格式验证通过，完整验收开放；见[34](34-m12-implementation-report.md) |
| FR-AUT-03 | P1 / M12 | T-M12-07 | AT-18 | [08](08-show-media-automation-spec.md) | 本机切片已实施，原生/协议/格式验证通过，完整验收开放；见[34](34-m12-implementation-report.md) |
| FR-AUT-04 | P1 / M12 | T-M12-06、T-M12-08 | AT-18、AT-06、AT-14 | [07](07-format-compatibility-spec.md)、[08](08-show-media-automation-spec.md) | 本机切片已实施，原生/协议/格式验证通过，完整验收开放；见[34](34-m12-implementation-report.md) |
| NFR-01 | 全程；M13终验 | T-M0-02、T-M1-01、T-M1-02、T-M12-04 | AT-01、AT-18、AT-20 | [04](04-architecture-spec.md)、[09](09-quality-platform-release.md) | 核心模块无GPUI类型；持续验收 |
| NFR-02 | 全程；M13终验 | T-M1-03、T-M4-06、T-M11-03 | AT-02、AT-07、AT-13、AT-15 | [04](04-architecture-spec.md)、[09](09-quality-platform-release.md) | M1后台解析/预览及取消、M4单任务合并/取消/有界缓存已验证；全局/长期预算仍待验收，见26 |
| NFR-03 | 全程；M13终验 | T-M2-02、T-M2-07、T-M7-03 | AT-03、AT-06、AT-14 | [04](04-architecture-spec.md)、[07](07-format-compatibility-spec.md) | 未开始 |
| NFR-04 | 全程；M13终验 | T-M0-07、T-M7-08、T-M13-07 | AT-01、AT-20 | [09](09-quality-platform-release.md) | 未开始 |
| NFR-05 | 全程；M13终验 | T-M4-01、T-M4-08、T-M13-03 | AT-07、AT-13、AT-15、AT-20 | [06](06-render-text-spec.md)、[09](09-quality-platform-release.md) | M4 本机 EDIT/EFFECT release 反馈、画质与 RSS 门槛通过；完整输入/平台/数据集验收开放，见26 |
| NFR-06 | 全程；M13终验 | T-M1-03、T-M5-04、T-M12-06 | AT-02、AT-15、AT-18 | [07](07-format-compatibility-spec.md)、[08](08-show-media-automation-spec.md) | M1 ZIP/XML/版本/故障回归通过；完整门槛待验收 |
| NFR-07 | 全程；M13终验 | T-M3-01、T-M3-03、T-M4-04 | AT-04、AT-07、AT-11 | [05](05-gpui-ui-spec.md)、[06](06-render-text-spec.md) | 静态 CPU/Scene 共享 SVG glyph，macOS输入桥已验证；活动编辑统一布局仍开放，见23/26 |
| NFR-08 | 全程；M13终验 | T-M0-04、T-M6-09、T-M13-04 | AT-04、AT-05、AT-20 | [05](05-gpui-ui-spec.md)、[09](09-quality-platform-release.md) | 未开始 |
| NFR-09 | 全程；M13终验 | T-M0-02、T-M1-02、T-M13-09 | AT-20 | [09](09-quality-platform-release.md)、[15](15-evidence-references.md) | 通用依赖来源已登记；全量发布审核待M13 |
| NFR-10 | 全程；M13终验 | T-M0-08、T-M7-09、T-M13-10 | AT-01、AT-20及各阶段AT | [10](10-roadmap.md)、[12](12-acceptance-playbook.md) | M0/M1本地包与运行证据已提供；完整阶段待验收 |

## 阶段验收入口

| 阶段 | 关键AT | 必须交付 |
|---|---|---|
| M0 | 01 | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |
| M1 | 02、15 | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |
| M2 | 03、06、14 | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |
| M3 | 04、05 | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |
| M4 | 07、15 | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |
| M5 | 08、03 | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |
| M6 | 09、05、14 | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |
| M7 | 10、11、12静态、14、15 | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |
| M8 | 16、10 | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |
| M9 | 17、10 | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |
| M10 | 12完整、07 | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |
| M11 | 13、15 | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |
| M12 | 18、19、11扩展 | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |
| M13 | 20及全部适用AT | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |
| M14 | 单项新AT及基础回归 | 可运行包、真实演示、自动检查结果、平台状态、已知限制及回退点 |

## 命令与需求的关系

[动作目录](16-command-inventory.md) 从 FR 推导。用户功能完成率以任务和验收证据为准，不按代码行数或动作数量计算。

状态规则：未开始 → 实施中 → 待验收 → 已验收；不适用/移除必须给决策记录。所有引用的任务完成但AT失败，FR仍不能标已验收。跨平台部分通过时保留平台子状态，不用一个全局勾掩盖。

风险和条件见 [13](13-decisions-risks.md)；本次交付边界见 [STATUS](../STATUS.md)。

2026-10-30解锁后补充：最终包的真实审阅/媒体/输出面板、视频预览/全屏放映、打印设置与取消及前序GUI/系统拼音回归已补齐；本机子项状态和证据见[35](35-m11-m12-gui-verification.md)，完整阶段的开发与平台缺口保持开放。

2026-10-31：M13本机资格化、偏好/文件事件、自包含包、SBOM与离线更新回退；M14静态MP4/标题VTT及离线比较/对象接受已交付。子项与原父任务完整验收范围分别记账；见[36](36-m13-implementation-report.md)、[37](37-m14-implementation-report.md)及[最终索引](evidence/m14/final-verification.json)。
