# ADR-0011 ArkTS 原生路线复用一个协议实现

状态：设计采用，C07 实施前的责任分配；未形成完整 profile 或认证。日期：2026-10-25。适用基线：0.9。关联 DEC-07/08、C07；不新增工作包、公共 feature 或 wire schema。

## 问题与依据

C07 的临时最小 profile 草案曾要求 C、C++、Rust 各自实现全部排队、票据、预算和关闭协议，并仅允许共享投递原语。这是草案额外施加的实现限制。[运行时设计](../architecture/08-runtime-and-idiomatic-api.md)要求共同语义、按需原语和每阶段唯一责任，明确不要求各语言重复类层级或引用计数；C07 的验收要求真实语言调用和独立证据，没有要求重复实现同一 SDK 机制。

## 决定

C、C++、Rust 三条 ArkTS 路线复用私有 **C++17 原生 SDK 协议实现＋窄 C ABI**。共享源代码和协议单元测试；每个运行时实例仍有显式 owner、env/generation 和有界状态，不建立隐式进程全局业务单例。

| 责任 | 实际 owner |
|---|---|
| 操作准入、队列、终态认领、source/ticket/bytes 台账、取消、停止、排水与实例 worker | 私有 native SDK 协议组件 |
| 环境引用、Promise/deferred、TSFN、异步 cleanup 与最终释放确认 | 该组件内独立的 env delivery/cleanup owner；具体平台原语须先验证 |
| 业务模型、算法、模型分配与配对释放 | 各路线真实 C、C++、Rust provider；不以同一语言实现伪装多路线 |
| 源声明读取、绑定生成、参数与结果转换 | 对应既定前端/后端与受检适配；Rust 保留真实 napi-ohos 元数据、宏导出、模块入口和转换 |
| Kotlin 路线 | 保持 Kotlin 业务 owner、CPF/akinterop 和唯一 managed registrar，不接入第二套原生业务 owner |

共享组件持有 opaque payload lease，通过明确的 provider 调用、借用视图、克隆和配对释放端口操作业务值。它不解释外部 C++/Rust 对象布局，不跨分配器 free，也不把业务算法放入公共协议。窄 C ABI 不传递 STL、Rust Box 或异常；线程、panic/异常、借用长度和函数指针/DSO 保活前提均须显式检查和验证。

同一协议源码静态进入不同 DSO，不等于它们共享一个实例。独立实例分别归属 route/profile/env；需要组合共用实例时必须由一个明确 owner 装配，其余模块只持引用，不能把重复实例称为唯一 owner。

## 依赖与实现范围

采用 C++17 是为了复用已有 RAII/move 和 Node-API 原生机制，减少手工回滚与重复并发状态机。纯 C 可减少 C++运行库依赖，但当前没有零 C++依赖的产品要求；统一采用 Rust 则会把 Rust 构建/运行库闭包扩散到 C/C++，本次不选择。

Rust 现有基础探针只依赖平台 ace_napi 与 libc。接入 C++组件会新增 Clang、公共 ABI/原语源码、静态库以及实际 libc++/libuv 等依赖；这不是零成本内部改动。必须重新锁定、检查 DT_NEEDED、符号/ABI、许可、唯一运行库、HAR→HAP 成员与实际装载，不沿用旧 Rust 无 C++依赖的证据。

实现放在 `sdk/arkts/native-runtime`，语言 provider 放在相应语言 SDK 目录，前端、工具链和打包继续归适配器。工具核心不导入具体平台后端，也不成为应用运行时。当前决定只确定责任和复用边界；完整 API、预算、Promise/deferred 弃置、平台清理时序和 profile 摘要仍按探针与冻结清单完成后实施。

## 不变的验收条件

每路线仍独立证明作者声明→实际前端/生成→真实 provider 编译语言→模型创建/配对释放→异步操作与反调→CRIT-11/12 正反场景→公开候选与 Release/HAR 消费。协议单元测试可复用；跨分配器交接、线程入口、真实桥接和目标结果不能借用其他路线的结论。

C05/C06 既有行为与候选身份不变，Kotlin main-env 限制和受管修补身份独立。共享层不能绕过后端生成、错误类型检查、bootstrap 或任何必需验收，也不能把 P05/P06 机制探针标成完整 SDK。
