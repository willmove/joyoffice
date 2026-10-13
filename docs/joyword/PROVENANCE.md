# 独立实现、依赖与资产来源

版本 0.2 · 2026-10-13。对应 QUALITY-002。仓库代码许可证延续根目录 [MIT LICENSE](../../LICENSE)。本文件是工程来源规则，不把公开规范的可访问性解释为对所有知识产权的统一授权。

## 1. 独立实现规则

允许：公开格式规范、Unicode／字体标准、产品使用说明、公开 API 文档、经授权的黑盒操作和原创样例。规范中的元素名称与行为可用于实现；代码注释需标规范版本和条款。微软 Open Specification Promise 覆盖 ECMA-376、MS-DOCX、MS-OI29500、MS-DOC、MS-OFFCRYPTO、MS-CFB 等规范的必要专利主张，各规范另附实现用版权许可；这不涉及商标、字体或产品资产。

禁止：读取、复制或翻译微软 Office 与其他 Office 产品实现源码；将 Open XML SDK、LibreOffice、ONLYOFFICE、Pandoc、Typst 或 Word 自动化作为产品内核；把公开产品截图、商标、图标和模板直接打包为 JoyWord 资产。

通用基础库可以复用、阅读和必要时修补；这包括 Rust UI、字体、Unicode、容器、图片和密码算法库。阅读 GPUI 的构建脚本及输入 API 不属于阅读 Office 实现。平台公开 API 和通用绑定不属于排除的 Office 实现。

## 2. 来源证据

每项关键实现记录：需求 ID、规范 ID／版本／条款、独立推导说明、原创测试、黑盒观察环境。行为无法从规范确定时，记录被观察软件的版本、操作输入、结果和备选解释，不把猜测变成规范事实。

研究 Office 开源项目限于 README、用户手册、公开能力矩阵、许可与发布说明。不要把其内部架构或代码逐段迁移。若来源不符合边界，先隔离对应实现并重新从规范和原创样例推导，任务不得标为完成。

## 3. 依赖登记策略

每个实际使用的直接依赖记录精确版本、来源、许可证、feature、用途、是否含原生代码及维护风险。Cargo.lock 锁定完整传递依赖；升级需执行相关语料回归。发布前生成包含传递依赖的 SBOM 和 THIRD_PARTY_NOTICES，不将本文件的候选表当作完整许可清单。

| 依赖 | 定位 | 许可／来源依据 | 注意事项 |
| --- | --- | --- | --- |
| GPUI | UI 框架 | [上游许可](https://zed.dev/software-overview)，Apache-2.0 | 编辑器主体许可不同；仅依赖框架；记录传递依赖；来源按 ADR-010，第三方发布需审查发布者与构建来源 |
| sha2 0.10.9 | joyword-model 资源内容散列（直接依赖） | RustCrypto，MIT OR Apache-2.0（crate 元数据） | 与 GPUI 间接锁定的版本相同；只用于身份散列，不用于密码学保护 |
| Ropey 1.6.1 | 仅 T100 文本存储基准的 dev 依赖 | [上游](https://github.com/cessen/ropey)，MIT | 不进入产品构建；基准结论见 IMPLEMENTATION |
| HarfRust | 字体整形 | [上游](https://github.com/harfbuzz/harfrust)，MIT | 记录版本、字体和特性；不根据库名称声称兼容 Word |
| Fontique／Skrifa／ICU4X | 字体和 Unicode 基础 | 各选定发行包的 LICENSE 与元数据 | 使用时登记，而非默认视作已加入 |
| quick-xml／zip | 通用格式基础 | 各选定发行包的 LICENSE 与元数据 | 自研上层映射，关闭不需要的能力 |
| krilla | PDF 绘图与定位字形 | [上游](https://github.com/LaurenzV/krilla)，MIT OR Apache-2.0 | 精确记录 feature；避免依赖自动整形作为第二排版路径 |

MIT 主项目不能删去第三方 Apache 等许可证的通知。当前锁文件已含 MPL-2.0（cbindgen、option-ext，以及 Windows 字体用的 dwrote）、Unicode-3.0、BSD、ISC、Zlib、CC0-1.0 和 NCSA 等许可，均须进入通知清单。根目录 deny.toml 由 cargo-deny 在 CI 中检查许可白名单、RustSec 公告、通配版本和注册表来源；经 GPUI 0.2.2 间接引入的“停止维护”公告逐条登记理由，待 ADR-010 选定来源时复查，JoyWord 直接依赖不得列入忽略。是否允许特定依赖应按实际许可与分发方式检查，不笼统地把全部开源代码认定为可用或不可用。

## 4. 字体、图标、模板与样例

- 样例文字由项目原创或明确授权；不得默认把用户合同、邮件或公司资料提交到仓库。
- 字体记录来源、版本／散列、许可证、face index 和嵌入限制。系统字体可以用于本地测试，不据此获得重新分发权。
- 持续集成优先使用许可明确、可重新分发的固定测试字体；生产版使用系统字体与可选字体包。
- Windows 中文字体（宋体、黑体、仿宋、楷体、微软雅黑、等线）和方正等商业字体不随产品分发；测试与回退字体优先选 OFL：思源宋体／黑体、Noto CJK、霞鹜文楷、朱雀仿宋（技术预览），以及西文度量兼容的 Carlito、Caladea、Liberation。
- 图标和模板原创或使用许可明确的资源；JoyWord 品牌与其他产品标识分开。
- 参考软件生成的测试文件记录生成者、环境和样例内容来源；不将专有字体嵌入公开语料。

## 5. 本轮来源记录

2026-10-13 的产品和生态调研依据 [RESEARCH](RESEARCH.md)。文档中的工程方案是 JoyWord 的规格决策，不是对竞争产品实现的转述。实际依赖及构建结果登记在 [IMPLEMENTATION](IMPLEMENTATION.md)。

任何后续扩展的来源登记随代码一起评审；发布前验证 LICENSE、NOTICE、字体／资源授权和依赖清单齐全。

