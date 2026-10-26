# MadB Lab 案例与报告规范

案例是具有独立预期的测试说明，而非对实现的复述。场景中的动作引用测试入口，断言引用契约和外部可观测行为。

## 必需字段

| 字段 | 含义 |
|---|---|
| schema / id / kind | 格式版本、稳定用例编号及 template/instance 类别 |
| problem_ids / source_ids | 关联公共问题与历史研究编号；审查来源使用独立 REVIEW-20261024-* 编号，AUD-* 为审查场景索引 |
| selector | 路线、profile_id/profile_digest、方向与运行目标；模板为 null，运行实例必须全部解析 |
| requires | 生成器及补丁、运行库、构建系统、分发形式、宿主/IDE、目标 SDK、设备、插桩和功能条件 |
| fixture / setup | 初始对象、测试入口和状态 |
| steps | 调用、barrier、取消、环境事件和清理 |
| assertions | 输出、错误、事件顺序、线程、资源与后条件 |
| expected_result | 正常通过或明确负例期望 |
| cleanup | 无论成功失败都执行的收尾 |
| timeout_policy | 挂起判定和隔离恢复方式 |

[样本](../examples/case.example.json)为计划定义，不是已实现用例。样本遵循 `case.schema.json`，使用 `kind: template` 与 `selector: null` 等待实例化；它不是可执行实例。fixture 的零摘要、planned 环境版本和示意入口均为占位，5000 毫秒只是模板预算。正式案例改为 `kind: instance`，固定完整 selector、真实 fixture 文件引用、环境、测试入口及适用预算，并提供独立断言；runner 拒绝未解析模板，不能填入 host-native 等模糊目标后生成通过报告。

## 示例场景

针对 LIFE-004 和 THR-004：注册观察，制造一个已获准但暂停的回调，同时请求停止。停止等待必须在回调退出后完成。另一个子用例从回调中发起停止请求，验证不出现自等待。停止完成后再触发源事件，消费者计数保持不变，context 最终释放一次。

P03 的补充场景：先让 foreign future 成功完成回调发生，再由测试钩子注入可识别异常。断言错误完成没有再次调用，完成计数为一，结果和引用按契约仅交接/释放一次；随后分别加入取消、同步完成和迟到结果交错。旧模板对照在隔离测试构建中验证检测能力，不进入发布候选。上游崩溃的根因推断不等于 MadB 的实测结论。

不同阶段的先后由 barrier 和逻辑事件序列证明，不能仅比较低精度墙钟时间。测试实现不能通过调用同一生产状态机计算期望序列。

## 应用性矩阵

案例可以参数化 profile、target 和 direction，但每个实例都有独立结果。适用性枚举为 required、optional、not-applicable；不适用必须有原因。平台特例保留专门子用例，例如 Swift Sendable、ArkTS env 退出和 JNI R8，不能被抽象成一个通用“线程测试”后丢失。SKIE 的不可调用占位、SwiftPM 的调用方向限制及 KMP 根 publication 冲突同样需要专门的编译/消费负例。

## 报告身份

运行报告包含 schema、run_id、工具版本、Lab revision、candidate digest、profile、目标、SDK/编译器、生成器补丁身份、构建系统、分发 profile、时间、案例结果和附件摘要。运行 ID 与时间属于证据，不参与代码生成的确定性摘要。

结果包含 actual、expected、status、evidence_level、duration、resource_observations、diagnostics。分类统一使用[测试策略](01-testing-strategy.md)：证据不完整或必需环境缺失为 error，受其阻塞的下游为 blocked；主动不执行为 skipped，能力不支持为 unsupported，后条件不满足为 failed。发布所需案例只接受完整且适用的 passed。

## 新增问题流程

先写问题和失败场景，再写预期；实现修复后同时运行正例、负例和受影响路线。来自线上或用户报告的案例记录可公开的最小复现，去除业务数据和凭证。重命名保留旧 ID 对应关系，不覆盖历史结果。

## 子用例与超时的可执行性

每条断言都要有对应 setup/steps；回调中 requestStop 与回调外 awaitStopped 是不同场景，不能只在断言中写“重入子用例通过”却没有执行它。样本仅覆盖明确列出的停止竞态，重入由独立实例执行。

watchdog 超时默认是 error，保留挂起位置和清理结果；通过 barrier 已证明后条件违例才判 failed。可以隔离执行会 abort 的负例，但必须匹配指定阶段与退出原因，任意崩溃不是通过。关键用例的独立 oracle 见[场景规范](08-critical-scenarios.md)。

## 设计准入与案例关系

DEC-01 至 DEC-12 指向规范和责任任务，CRIT 场景展开实际交错；二者均不是执行结果。用例实例在运行前固定适用性和独立预期，不能由被测后端把失败场景改成 not-applicable。架构/验证维护者共同审核适用性，缺设备仍是证据缺口。

报告同时绑定输入、工具、profile、候选、Lab revision 和案例定义摘要。包装、库或预期定义变化后，原记录可追溯但不能作为新配置通过；行为兼容必须重新验证。
