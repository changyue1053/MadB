# ADR-0027 有限工程日常重建与 Wrapper

项目所有者已确认“采用推荐契约，进入实现”。采用日期为2026-10-27（America/New_York）。本决定承接I01/I02/I03既定范围，子范围提前实施不改变整包依赖或验收条件。

采用[具体提案](../reviews/engineering-rebuild-proposal.md)的冻结v2及[最终独审](../reviews/engineering-rebuild-final-v2-review.md)。提案SHA-256为`ebd8e133eaac9898db37fb3ad4789e46b972d0e3a8adf46da95afba0e96f6b3e`，独审SHA-256为`a4af506efe821b713d3386e2126a0e579ecd58e342ee429765883c946a333835`，完整格式/向量manifest为`381a00bdcf104afec78a1c43a866d8374258894e7230b2e71bda1397fe89ae0c`。原提案和历史审查保持原结论；采用不计实现或运行验收。

- 显式新模板`org.madb.template/rust-c-sdk@0.2.0`与有限配方`madb.project.rust-c-values@1`，限定macOS Arm64 Rust→C同步值工程、Debug。旧0.1.0/0.1.1原包和默认0.1.1入口保留，未覆盖任意已有工程、其他平台或资源/异步配方。
- 新增`madb project build --root PATH --frozen [--json]`及`madb project install-tool --root PATH --frozen [--json]`；固定静态`./madbw build --frozen [--json]`作为CLI/后续IDE共同入口。Wrapper按提案固定系统工具、描述摘要与实际镜像准入，不经PATH找版本或下载；唯一launcher以0755创建。安装恢复只接纳同字节工具，无覆盖复制，不升级来源。
- 新增`madb.project-build/1`与`madb.project-build-result/1`，严格结构、完整输入/结果、显示路径、退出与预算按提案固定；统一1MiB解码上限保留。请求、输入及输出使用三个独立project-build摘要域及精确JCS投影，schema通过不能作为执行凭据。
- 每次捕获当前完整作者/consumer输入与目录集合，保守全量执行，记录`conservative-full-build`，不虚报缓存命中。共享模板/工具/SDK/profile来源保持冻结，每次作者摘要及C04锁只写新的受管尝试目录。CLI组合根复用既有C04 coordinator，app依赖窄端口；内部consumer CMake不递归调用Wrapper。
- 工程owner、初始来源和完整旧结果先准入；每次独立C04域和编译输出，最终短临界区以输入/旧索引基准CAS发布完整SDK+consumer结果。坏旧结果准确拒绝，保留历史批次，不套用通用recover/自动GC，不持锁启动外部编译器。
- CAS前失败或取消不替换current；成功替换后封口取消。持久化/证据尾窗错误保留真实`published=true`及result；仍不可判定则`published=null`并拒绝消费。install-tool按无覆盖安装或匹配已有镜像的最终接受点封口取消，后续错误保留实际installed_tool，不能误报130或删除已核对镜像。

实施须补真实同根修改重建、普通C消费、并发/取消、输入漂移、恢复与离线错误的独立证据；P09需真实进程计数及分阶段样本。Wrapper原型与格式向量不授予工程/IDE通过；导入、索引、Run/Debug、已有工程接入/升级/禁用、其他承诺配方及I链整包仍逐项验收。实时状态只在catalog维护。
