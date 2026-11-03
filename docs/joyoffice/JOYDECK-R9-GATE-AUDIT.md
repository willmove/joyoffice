# JoyDeck R9 工程门禁审阅

R9 替换了旧产品目录。Word/Sheet 的分层规则保留；新的 Deck crate 只允许清单中的实际内部依赖。GUI 权限仅扩至 `joydeck-ui-gpui`；CLI、模型、引擎、格式、渲染与私有旧格式迁移桥不含 GPUI。

IO/进程按 `scripts/check_gates.py` 的 `DECK_FS_REVIEW` 逐文件登记：CLI 显式文稿/场景输出；字体 build.rs 的注册输入与 OUT_DIR、fontdb 的系统字体只读发现；shell 的文稿、存储、发布、输出、控制、媒体与 owned helper。没有生产目录级豁免，renderer/model/engine/format/pptx/legacy 没有获得文件或进程权限。

静态 panic 扫描算法保持原样。已移除旧 Deck/已替换 OPC 代码的陈旧台账；新条目逐个绑定文件与代码文本，见 [审阅](../joydeck/evidence/R9/panic-audit.json) 与 [移除项](../joydeck/evidence/R9/removed-stale-panic-sites.json)。其中只有一处生产不变量：编译进程序、SHA 固定的 Ubuntu last-resort 字体解析，`last_resort_face_parses` 每次测试覆盖；它不读取用户文稿。其余条目都位于测试模块/测试函数或明确 cfg(test) 的外部文件。未把全部导入代码加入忽略列表。

未适配的 DeckCraft tests-upstream 从 crate 移至 [只读档案](../joydeck/upstream-tests/README.md)，保持字节及来源映射；不编译、不计作已通过测试。现行及反例门禁、Python 工具测试继续执行。

公共 OPC 只把几个已校验字节长度/绝对路径不变量的 expect 改为错误返回/安全分支；接口与预算不变。共享 UI 实现没有内容变化。锁文件只新增新渲染器需要的精确依赖，既有 registry 包版本/来源/checksum 未变。完整回归与当前限制见 [R9 验证](../joydeck/evidence/R9/README.md)。
