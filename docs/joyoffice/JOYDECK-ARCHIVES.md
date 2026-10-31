# JoyDeck 旧开发包与原始证据存档

旧独立目录已在迁移工作区的 `backups/joydeck-migration-20261031/` 本机存档。交付时同一存档会放入日常开发目录 `~/Coding/joyoffice/backups/joydeck-migration-20261031/`；构建不读取它，Git 检出不自动包含历史二进制。

- `original-source-and-evidence.tar.gz`：完整旧源码、全部原始证据、日志与导出结果，排除可重建 target 和单独存放的 dist。
- `dist/`：原目录全部历史开发包、源代码回退包、包内校验材料。原名保留，不覆盖旧产物。
- [旧产物文件清单与 SHA-256](validation/joydeck-legacy-artifacts.json)：用报告中原来的 `dist/...` 路径查找本机 `archivePath`。
- [原始文件清单](validation/joydeck-source-inventory-20261031.json)：包含源快照归档的 SHA-256。

历史报告里的“最新包”仅指报告当时的产物；其功能、签名、平台验收边界保持原记录。迁移后的新代码必须重新构建和验证，不能将旧包或旧 hash 作为迁移通过的证据。

Git 中的 `docs/joydeck/evidence` 是报告及最终索引显式引用的关键证据。JSON 的旧路径由迁移映射解释，原记录字节保持不变。完整中间截图、原始媒体输出等在源快照归档中；恢复审计材料时解到新的临时目录，避免覆盖现有开发树。
