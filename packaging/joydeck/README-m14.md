# JoyDeck M13/M14 macOS ARM64 开发包

双击 JoyDeck.app，或运行 ./joydeck --authoring-demo。JoyDeck.app 自包含，可单独移动到带空格/中文的目录；保留原版本以便回退。本包未正式签名或公证，完整三平台 Beta 与 M14 功能池尚未验收。

维护：保存窗口大小、缩放、格式栏和备注布局；最近文件最多12项，打开成功/保存后更新，支持清空。偏好版本2，旧版本1迁移，坏配置保留副本后重置，未来版本拒绝覆盖。macOS文件菜单及本地文件打开事件可用；同进程再次打开同一已保存原生文稿会激活现有窗口；其他进程独立编辑，保存冲突保护。

比较：无稳定ID的旧文稿须先在编辑器打开并另存为原生文稿。输入参考文稿完整路径，比较页面/对象/顺序/设计/审阅等。逐对象接受新增、删除或内容变更，可撤销。使用“接受并复制所需图片”可将目标缺少的图片资源与对象一起接受，失败整次回滚、单次撤销；CLI/MCP对应document.acceptObject的includeResources=true。版式/母版/主题等依赖尚不自动迁移。比较后任一文稿改变需重新比较。页面/设计/审阅差异只报告，不是实时协作或完整合并。

输出：现有PDF/PNG/JPEG/大纲/RTF/GIF/POTX/PPSX与macOS打印继续可用。新增“静态页MP4”：960宽、15fps、每页2秒、H.264 Main，无音轨/动画/转场/嵌入媒体或录制。≤1800帧、10分钟编码时间预算，可取消，失败不覆盖目标。页面标题WebVTT字幕使用同一时间轴，不输出私密备注。CLI convert ... mp4 必须加 --accept-losses；VTT格式为vtt。

CLI/MCP：命令/认证/roots等用法与M12相同，新增 compare /absolute/base /absolute/reference、diagnostics、document.compare/acceptObject。MCP仍锁2025-11-25同步stdio；无异步jobs/运行取消通知。diagnostics只包含版本/平台/能力，不包含文稿正文、路径、token或环境值。

领域命令：engine.commands现在列出全部16个已注册编辑命令及必需参数。command.execute执行，command.validate只检查草稿且不修改文稿/历史/ID游标；可选expectedRevision用于拒绝陈旧修改。transaction.batch整体成功或整体回滚，一次撤销。表格/设计action使用type/value结构；索引从0开始。不同CLI调用自动生成独立操作ID；需要在结果不确定后重试时，须自行提供相同operationId。scripts/verify-command-catalog.py可验证本地/远程MCP的全目录、校验、重放和undo/redo。

依赖与校验：BUILD-INFO.json记录target/toolchain/lock/features/源码hash；SBOM.spdx.json为目标平台Cargo正常/构建依赖图；dependency-licenses包含缓存中可获得的原始许可文本及索引，系统字体不随包分发。SHA256.json覆盖所有包文件（自身除外），不是签名。

离线维护工具：python3 scripts/release-tool.py verify /absolute/package。install需要独立可信渠道取得的 --expected-sha256，并指定 --team-id 校验正式mac签名；开发包可显式 --allow-unsigned-development。仅校验、展开到独立版本目录并原子更新active.json指针，不下载或运行程序；--stage-only只暂存，rollback --root ...选择保留的上一版本。不要用与ZIP来自同一不可信位置的hash当作身份认证。本机没有有效签名身份，未声明正式签名验证完成。

验证需Python3.11+、Pillow/numpy/pypdf/python-pptx、Poppler、macOS系统输入/录屏权限及未锁屏桌面。scripts/verify-m13-m14.py验证真实窗口、对象比较undo、静态MP4全部帧、再导入实际播放、偏好、24次窗口开关和120次编辑/undo。原生系统拼音与前序GUI脚本仍在scripts中。--native-only不算真实窗口验证。

128份新增语料为8组原创变体，并非100份独立外部Office语料。十分钟seeded引擎/检查点稳定性与部分性能数据是本机证据，不代表完整性能/硬件/读屏矩阵。仍缺外部授权语料、三平台干净安装、正式签名公证、真实多显示器/音频设备、完整无障碍、前序开发缺口及M14其他功能。详见源代码STATUS和36/37实施报告。writer schema1.10/minReader11，保留旧文件和M12回退包。
