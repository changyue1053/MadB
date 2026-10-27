# ADR-0023 完整模板、初始化结果与有限值工程分片

项目所有者已选择“采用提案，进入格式实现（推荐）”。本决定承接 I01，采用日期为 2026-10-26（America/New_York）；不改变工作包阶段、依赖、验收条件或发布承诺。

## 已批准材料

精确规则采用[完整提案](../reviews/i01-final-format-proposal.md)及[最终独审追加](../reviews/i01-final-format-independent-review.md)绑定的原文，SHA-256 为 `09672664872c3d56fe1b81857692be87f4bd8222e38b012e9d5902055e207fa6`，45,441 字节。仓库中的提案是明确标注的导航副本，只调整无法通过仓库文档检查的目录/行号链接；原文及设计向量逐字节保存在第七批耐久归档。提案、早期审查和设计模型继续保留其当时“待决定、未实现”的结论，本 ADR 是随后采用的依据。

[ADR-0019](0019-project-init-incremental-contracts.md) 的四项增量继续有效，不重复审批。本次采用此前未定的五组规则：

1. `madb.project-template/1` 与 `madb.init-result/1` 的完整严格 envelope。模板 `files` 是除 manifest 自身之外的完整原包库存；结果新增原包 File、证据根定位以及初始文件计划的真实内容身份。共同文件引用、数字子集、退出类别保持既有规则。
2. 原包摘要绑定整个受限未压缩 POSIX USTAR 的原字节；可信内置目录分别锁归档、manifest 和 recipe/renderer 镜像。完整库存、未选文件、路径、元数据、padding、预算、重复/大小写冲突和重验都须检查。目录解包根与 manifest 父目录不能混淆，profile/SDK 原字节和内部引用不得改写。
3. 仅交付 copy、json-values、identifiers 三种有限 renderer。JSON 指针只替换既有标量/null；identifier 仅在已锁资产上下文内一次扫描合法标记，拒绝未知、未闭合、嵌套及注入。未知能力不静默降级。
4. `madb.init.rust-c-sdk.values@1` 是首个 macOS/aarch64 自然 Rust 值接口分片，复用既有生成/C04 链及普通 C 消费。CMake 4.4.4、Ninja 1.13.2、Rust/Apple 工具、SDK、MadB 和 recipe 的实际镜像与来源必须冻结后才准入对应执行；不得回退到本机其他版本。
5. 普通成功来自同根同计划的真实 Host no-replace CommitNew、独立必需检查、适用清理/open 及证据保存事实。外部 JSON 的 `created` 不能证明创建。dry-run 保存独立证据根和结果，但不创建工程、不下载、不启动工具；提交后失败保留工程，非零退出和恢复建议据实生成。

请求语义摘要使用 `madb.init-request/1` 域及提案 §7.1 的完整内部投影；这不是新增 InitRequest wire。摘要是否为空不决定错误类别。环境观察行精确复用现行 report 的七字段 items 规则，command/recovery argv 按最终独审追加的数量、字符串和首项约束，不获得任意命令执行权限。

## 实现责任与兼容边界

schema/严格解码归 protocol；跨文档转换归 contracts；模板领域模型、参数、模块/文件计划及检查依赖归 plan；确定性资产渲染与有界归档解析归 templates adapter；执行事实、成功判断和恢复编排归 app；真实文件与进程、无覆盖提交归 Host。保持六个核心 crate 与既有依赖方向，不把 wire 带入核心，也不让 templates adapter 取得提交权。

本次按已有 `adapters/templates` 职责启用一个实际 `madb-templates` 编译单元，以隔离 wire/JSON 字节渲染与纯计划、Host 副作用；仅依赖 model、plan、protocol 及已有 serde_json。contracts 增加对 plan 的显式转换依赖。CLI 组合根以开发依赖串接格式/计划/渲染回归；这不新增 init 命令。对应 manifest 与架构正反例共同维护，不新增插件内核或调度系统。

格式层、真实包/创建、首配方最终根执行分片分别实施和验证。当前 workspace 版本 `0.1.0-dev.0` 不满足提案的正式 `N.N.N` 准入值，不能伪装成 `0.1.0`；即使 schema/纯规则通过，也不授予当前开发构建正式模板执行资格。工具取得、可执行登记和真实结果各自留证。

首片不替代 C05 资源/异步接口、`rust-c-sdk` 最终承诺、IDE 导入/Run/调试、Wrapper、其他目标及组合验收。profile/2 可解码不等于该首 recipe 可执行，首 recipe 仍只准入 profile/1。C06/C07/L02 冻结候选和历史结论不外推到 init；状态只在 catalog 维护。

## 检查要求

两格式共用正反向量、严格 JSON 与现有 schema 注册路径；纯规则验证默认选择、参数/版本、DAG、路径/所有权、完整库存、渲染和结果引用。成功判断另用固定请求、受检计划、独立配方必需集合与 Host/证据事实验证，不能由空 checks 真空成功。

进入 Host 分片时另执行真实 no-replace 竞争、链接/路径拒绝、冻结后漂移、失败保留及取消检查，并独立复核。最终根生成/链接/运行与 Lab 消费绑定新候选、工具和环境；未执行不填通过。阶段提交仍按项目所有者要求跳过 Actions，不创建 PR、标签或发布。

## 后续版本决定

上述开发版本差异随后由项目所有者采用 [ADR-0024](0024-internal-tool-candidate-version.md)：重新构建真实 0.1.0 内部工具候选，继续拒绝开发版本，旧报告与候选身份不变。
