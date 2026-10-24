# MadB 各语言编码与边界规范

公共语义一致，具体 API 遵循语言生态。工具版本在实施时固定；以下工具是候选检查手段，不表示已安装或所有目标均支持。

| 语言 | 编码重点 | 候选自动检查 |
|---|---|---|
| Rust | 纯模型、typed error、小范围 unsafe、显式 Send/Sync 前提 | rustfmt、Clippy、编译失败用例、适用范围内的 Miri |
| Kotlin | 不可变公开模型、结构化并发、source set 隔离 | Kotlin formatter、Detekt、编译测试 |
| Swift | 访问控制、actor 隔离、值/引用语义、明确 async 清理 | swift-format、SwiftLint、严格并发编译模式 |
| ArkTS | 遵守目标 SDK 的语言子集、类型化边界、env 归属 | SDK 自带检查、真实 ArkTS/HAP 构建 |
| C/C++ | 受限公开接口、RAII、调用约定、异常和别名规则 | clang-format、clang-tidy、警告、适用的 Sanitizer |

## Rust

unsafe 块限定到最小可信边界，每处写出有效地址、长度、对齐、别名、所有权、线程等所需前提，并指向建立这些前提的调用或契约。安全函数不能把调用方无法满足或未被检查的要求藏起来。

不从任意外部指针构造 Box/Vec/CString；from_raw 必须匹配来源和唯一回收责任。切片构造验证空值、长度计算与分配范围，但这些检查不证明未知指针真实有效。外部类型的 Send/Sync 实现需要独立安全论证，Arc 本身不能提供该论证。

## Kotlin

公开 suspend/Flow 的取消行为需与原生协议一致。continuation 恢复后的及时取消和资源交接必须有清理处理。业务 scope 由调用方或明确 owner 提供，平台绑定放在对应 source set。

JVM JNI 的方法签名、反射/反向调用和 R8 单独验证；Native 的 StableRef、临时 C 指针与回调生命周期单独管理。两者不能以同一个“native”工具类掩盖差异。

官方 Kotlin 与 CPF 的处理器、生成目录和依赖坐标分别管理。AGP 9 的 KMP library、Android app 与需要 externalNativeBuild 的模块遵循[初始化配方](../architecture/14-project-initialization.md)；预编译 `.so` 包装不等于使用 Kotlin/Native Android target。

## Swift

尽量让编译器表达隔离和传递约束。`@unchecked Sendable` 需要证明，不作为去除警告的快捷方式。MainActor 表达 Swift 隔离意图，不证明任意外部对象内部线程安全。

Data/Array/String 与外部缓冲之间的借用限制在合法闭包或 owner 生命周期内。deinit 不承担必须等待的异步排水。独立快照的字段也要满足目标隔离与值语义，不只更换顶层类型名称。

公开 SDK 的 C++ interop 要求、SKIE 分发设置和 custom cinterop framework 来源都进入构建配置。测试实际调用公开增强接口，不能以可导入 module 或可编译 framework 代替接口可用性。普通 Swift 外观是否隐藏 C++ 构建条件由真实消费者验证。

## ArkTS

Node.js/TypeScript 能编译或运行不能替代目标 ArkTS 编译与运行。JSON 输入做真实校验，类型断言不增加运行安全。长期持有 JS 值使用目标允许的引用机制，并随 env 清理；不能跨 Worker 直接复用环境引用。

Promise 的完成与取消需要独立协议。ArkUI 状态装饰器只按实际 SDK 支持使用，UI 观察不拥有共享原生资源的关闭责任。

当前语言资料允许自定义装饰器，但不因此允许照搬依赖动态增删字段/原型的 TypeScript 方案。`@ObservedV2` / `@Trace` 用于状态观测；调用追踪使用诊断模块，不复用这两个名字表达另一套语义。

## C/C++

C 导出面使用固定且可验证的布局、不透明句柄和配对释放函数。错误、字符串长度和输出参数语义明确。C++ 非平凡对象不按裸字节复制，构造、移动和析构由正确语言完成。

是否启用异常、RTTI 和标准库 ABI 是 profile 属性；出口遵循声明的异常协议。持锁调用外部回调需特别审查，固定线程对象的析构回到合法执行位置。

## 使用手段的边界

Sanitizer、Miri、静态检查和并发编译模式各有目标和覆盖限制；测试计划记录启用范围与遗漏。安全结论综合编译约束、代码评审和真实边界用例。

## 注解 宏与属性封装

具体选择以[语言特性设计](../architecture/15-developer-ergonomics.md)为准。Kotlin 优先 KSP 声明处理，不假定能够改写函数体；Swift 与 Rust 的宏保持薄层，复用普通 API 和单一规则实现。Dart 扩展使用 build_runner/ffigen 等构建期生成，不依赖尚不受当前编译器支持的语言宏。

属性读取不能隐式发起阻塞 FFI、创建线程或重复订阅。普通 API 与便利入口共用资源协议；use、Drop、析构只能承担其实际支持的同步清理，需要等待的关闭保留显式异步入口。宏和处理器错误应定位原始声明，生成代码必须能检查、调试和复现。

诊断性断言与产品正确性保护分别命名和配置。关闭日志、指标或追踪时，不能同时删掉资源准入、所有权交接和必要线程检查。

参考：[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)、[C++ Core Guidelines](https://isocpp.github.io/CppCoreGuidelines/CppCoreGuidelines)、[Kotlin 互操作](https://kotlinlang.org/docs/native-c-interop.html)。规范采用其适用思想，具体规则以 MadB 的公开契约为准。
