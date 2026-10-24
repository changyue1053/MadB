# ADR-0009 实施契约与阶段准入

状态：设计采用。日期：2026-10-24。适用基线：0.9。具体规则归下表引用的规范，实际实现和认证仍由任务及证据记录。

## 决定

将会影响模块接口、资源协议、构建一致性和发布承诺的决定落实到十二项准入检查。宏名称、内部类布局等可以随原型收敛；公开语义、默认行为、输出身份和失败规则在对应配置正式集成前明确。

| 决定 | 完成节点 | 规则权威 | 工作包 |
|---|---|---|---|
| DEC-01 符号身份与策略解析 | 首链集成前 | [03-contract-and-ir.md](../architecture/03-contract-and-ir.md) | C02、C04 |
| DEC-02 格式权威与语义摘要 | 首链集成前 | [03-contract-and-ir.md](../architecture/03-contract-and-ir.md) | C02、E01、E02、E03 |
| DEC-03 句柄、关闭与最终释放 | 首链集成前 | [05-lifecycle-and-concurrency.md](../architecture/05-lifecycle-and-concurrency.md) | C02、C05、C09 |
| DEC-04 生成、构建与候选的任务图 | 首链集成前 | [07-code-generation.md](../architecture/07-code-generation.md) | C04、C05、I03 |
| DEC-05 并发提交与崩溃恢复 | 首链集成前 | [07-code-generation.md](../architecture/07-code-generation.md) | C04、I03、C09 |
| DEC-06 依赖追踪与缓存失效 | 首链集成前 | [07-code-generation.md](../architecture/07-code-generation.md) | C04、I03、C09 |
| DEC-07 后端与便利层的唯一责任 | 对应配置集成前 | [08-runtime-and-idiomatic-api.md](../architecture/08-runtime-and-idiomatic-api.md) | L01、L02、L03、L04、L05、L06、X01 |
| DEC-08 执行器、容量与退出 | 涉及调度的配置集成前 | [08-runtime-and-idiomatic-api.md](../architecture/08-runtime-and-idiomatic-api.md) | C05、C06、C07、C08、C09、L03、L04、L05 |
| DEC-09 共享内核的物理装配 | 组合模板集成前 | [09-build-packaging-release.md](../architecture/09-build-packaging-release.md) | I05、C09、C10 |
| DEC-10 消费制品与装载匹配 | 对应 SDK 独立消费前 | [09-build-packaging-release.md](../architecture/09-build-packaging-release.md) | C05、C06、C07、C08、C10 |
| DEC-11 最低完整配置与阻断范围 | 各路线集成及发布前 | [16-technology-stack-and-versions.md](../architecture/16-technology-stack-and-versions.md) | C03、C06、C07、C08、C11、I06 |
| DEC-12 开源安装、更新与责任 | 公开仓库及分发前 | [02-open-source-and-contributing.md](../delivery/02-open-source-and-contributing.md) | C01、C10、C11 |

映射以[实施目录的 decision_gates](../catalog/implementation-plan.json)为维护入口，HTML 直接生成其阅读视图。DEC 是设计准入索引，不增加工作包、不创建新的进度或认证枚举。关联任务完成不能单独证明某项配置已认证，必须核对相应候选与案例。

## 取舍与验证

采用精确选择器和显式默认来源，牺牲少量配置简写以防策略误绑定；采用不可变批次和短提交锁，以有限磁盘占用换取 IDE/CLI 一致性；优先复用后端运行机制，以每阶段唯一责任避免重复状态机。平台差异保留在 profile 与适配器，不合并为通用线程或句柄实现。

首链自有 C ABI 的登记令牌不改变第三方 ABI；GitHub Releases 和 Apache-2.0 是本项目的交付设计，不改变第三方许可。首次公开之前仍需实际仓库身份、许可文件、依赖核对和发布证据。

符合性按[关键场景](../validation/08-critical-scenarios.md)执行。Dart E04 只验证同步子集；P03/P05 提前做风险探针，阻断范围限于受影响配置，Rust/C 首链使用自己的 P01/P02 锁。十六条主线、四十一工作包与 S5 完整交付范围保持不变。
