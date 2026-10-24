# Kotlin ↔ C/C++ 跨语言边界工程清单

对照基线：MadB 0.9；修订日期：2026-10-24。本文为研究阅读版，保留原问题编号和历史样本；当前实施以权威规范、任务和目标证据为准。表内项目事实沿用原研究快照，本次没有重新验证旧仓库的现状，也不把风险推演当作已发生事故。

JVM/Android 主线使用 JNI，Kotlin/Native 使用 cinterop 与显式 C 可见边界；四条路线分别验收。直接 C 调用的 JNA/FFM 属于 X04 扩展。

当前选型、关键更新与实施关联见[Kotlin ↔ C/C++ 研究整理](Kotlin_C_CPP_架构总结与新版_mad-boundary_设计输入.md)；精确版本只维护在[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)。原文件、本机路径、旧行号及旧来源映射保存在[更新前快照](../snapshots/2026-10-24-before-refresh.zip)，原始记录不被改写。

本页共 25 项，稳定来源身份为 `Kotlin_C_CPP:序号`。[source-coverage.json](../../../catalog/source-coverage.json)记录修订阅读版的路径、行号、摘要及公共问题映射；原始映射可从快照中的 source-coverage.json 复核。

## 一 接口与数据表达

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **1. C++ 类型与调用路线** | JVM/Android 主线为 JNI C/C++ adapter；Native 为 cinterop + 显式 C 边界；按方向和目标分别验证，JNA/FFM 为 X04 扩展 | JNI、受限签名生成、cinterop、C adapter；JavaCPP 仅为生态研究选项 | 原研究：官方 cinterop 接入 C/Objective-C；JavaCPP 通过 JNI 接入 C++。[cinterop](https://kotlinlang.org/docs/native-definition-file.html)、[JavaCPP](https://github.com/bytedeco/javacpp) | 不把所有 Kotlin 调用归为 JNI；不假设 cinterop 可导入任意 C++ 模板；不强制所有 JNI 实现先增加 C 门面 |
| **2. Kotlin 源码与 JVM 签名** | 为 native 设置有限的桥接接口；从编译产物读取或校验方法描述符；把 native 反向访问纳入接口清单 | javap -s、ASM、RegisterNatives、描述符生成与编译探针 | 原研究：JVM 依据编译后的方法描述符匹配接口，不能直接照 Kotlin 源码猜测。[JNI 规范](https://docs.oracle.com/en/java/javase/24/docs/specs/jni/design.html) | 覆盖 static/实例、伴生对象、重载、值类及 suspend 降低后的形状；签名漂移在构建期失败 |
| **3. 数值 空值与枚举** | 声明宽度、符号、单位、合法范围和未知枚举策略；收窄前检查；空值与有效零值分开 | 固定宽度 C 类型、受检转换、可空包装、边界值夹具 | 原研究：HUKS 接口使用显式长度和输出参数，Kotlin 侧仍要处理 UInt 到 Int 的范围。 | size_t/UInt 到 Int 不静默截断；缺失 ID 不用 0 冒充；convert 或强转本身不被当作范围验证 |
| **4. 字符串编码与二进制** | 明确 UTF-8/UTF-16、长度单位、NUL 规则与错误输入策略；二进制按字节和长度处理 | 编码转换器、长度受限 API、Unicode 测试 | 原研究：JNI 的 Modified UTF-8 与普通 UTF-8 不同，不能直接当作原生库约定的 UTF-8。[Android JNI 字符串规则](https://developer.android.com/ndk/guides/jni-tips) | 覆盖空串、内嵌 NUL、补充平面字符、非法编码；不对任意二进制使用 strlen；转换失败可诊断 |
| **5. 结构体布局与编译条件** | 读取目标平台实际头文件、宏和编译参数；优先隐藏复杂布局；公开布局按目标验证 | Clang AST、cinterop、sizeof/alignof/offsetof 探针、static_assert | 原研究：MadDemo 原生事件约定固定布局并有断言，该快照中的由 Rust 消费，可复用其 ABI 验证方法。 | 覆盖指针宽度、对齐、packing、条件编译和 C++ 标准库；不能只验证开发机上的一个布局 |
| **6. 错误与异常边界** | 包装层捕获并映射 C++ 异常；JNI 调用 Kotlin 后检查异常状态；反向回调失败也使用明确协议 | 状态码与错误目录、JNI ExceptionCheck、C++ try/catch、fbjni | 原研究：fbjni 提供 JNI 引用及异常处理辅助；C 状态码仍需领域映射。[fbjni](https://github.com/facebookincubator/fbjni) | 不允许未处理异常意外越过边界；不把失败包装成空成功；有待处理 JVM 异常时不继续任意 JNI 操作；失败输出只按契约读取 |

## 二 内存与所有权

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **7. 创建与释放配对** | 每类资源声明创建方、持有方、转移条件与释放入口；复用后端已有 owner；资源需要确定性终止时提供 close | RAII、专用 deleter、资源包装、所有权策略 | 原研究：K/N 导出 C 接口含编译器提供的对象及字符串释放服务，不能任意 free。[导出服务](https://kotlinlang.org/docs/native-dynamic-libraries.html) | malloc/free、new/delete、库专用释放器不混用；错误及取消路径配对；两个语言包装不竞争销毁同一资源 |
| **8. 临时指针与异步保留** | 同步借用只在有效作用域内使用；原生异步保存输入时，复制或建立明确的长期存储 | memScoped、usePinned、native owner、操作保活 | 原研究：MadDemo 在 usePinned 作用域中同步调用 SecRandomCopyBytes。 | 指针不逃逸作用域；只保留 ByteArray/StableRef 不视为固定数组地址；取消等待不提前释放原生仍读取的输入 |
| **9. 跨运行时引用与回调保活** | JNI 区分 LocalRef/GlobalRef；JNA 回调保留强引用；K/N 显式管理 StableRef；注销和清理采用同一协议 | 引用包装、回调注册对象、全局引用计数、StableRef | 原研究：JNA 文档明确原生保存回调时，Java 侧必须保持其可达性。[JNA 回调](https://github.com/java-native-access/jna/blob/master/www/CallbacksAndClosures.md) | native 方法返回后不保存失效 LocalRef；注册失败清理；未来与在途回调均不再访问后才释放；强引用环能够断开 |
| **10. close 与在途调用竞争** | 获取使用许可后再访问资源；关闭先阻止新调用，再等待原生工作与在途使用结束，最后释放 | Open/Closing/Closed 状态机、使用计数、停止确认、awaitClosed | 原研究：MadDemo 原生接口区分 stop 与 free，后者承担线程退出后的回收。 | 不能只检查 closed 再无保护地调用；重复关闭安全；回调内关闭不等待自己；超时不授权强行销毁仍在使用的对象 |
| **11. 空缓冲 长度与输出参数** | 区分长度和容量；先判断成功，再读取契约允许的输出；收窄前验证实际长度；零长度使用允许的表示 | 生成的前后置检查、空输入策略、受检算术、故障注入 | 原研究：旧 mad HUKS 已用 null + 0 表示空输入；其输出读取顺序和长度校验可补强。 | 不对空数组 addressOf(0)；不读未保证初始化的 out 参数；0 ≤ written ≤ capacity；输入尺寸计算不溢出 |
| **12. 零拷贝 视图与可变别名** | 将地址与 owner、容量、可写性、失效条件绑定；默认安全复制，测量后再引入受控借用 | DirectByteBuffer、受限缓冲包装、FFM MemorySegment/Arena、pin 配对 | 原研究：FFM 的 Arena 管理内存作用域；错误的 native 使用仍需要原生协议约束。[FFM](https://docs.oracle.com/en/java/javase/22/docs/api/java.base/java/lang/foreign/package-summary.html) | 视图在 owner 释放或容器重分配后不可用；DirectByteBuffer 不自动拥有任意 native 地址；保持对象存活不等于地址稳定 |

## 三 线程 协程与回调

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **13. 原生线程接入 JVM** | 共享 JavaVM 而非 JNIEnv；各线程取得自身环境；只由负责 attach 的一方承担相应 detach；管理循环中的局部引用 | GetEnv、AttachCurrentThread、线程接入包装、JNI 引用包装 | 原研究：Android 文档区分 JavaVM、JNIEnv 与原生线程接入。[JNI 线程](https://developer.android.com/ndk/guides/jni-tips) | 跨线程不传 JNIEnv/LocalRef；不得误 detach 借用的 JVM 线程；原生长期线程不持续积累局部引用 |
| **14. 线程亲和性与阻塞** | 分开声明可并发、串行、固定线程和主线程；阻塞调用进入容量受控的执行设施 | 专用线程、Executor、Dispatcher 适配、线程断言、主线程耗时预算 | 原研究：图形句柄等要求固定线程的原生 API 不能仅靠串行协程调度满足；MadDemo 的 native free 可能等待线程退出。 | suspend 不等于不阻塞；普通 Dispatcher 不等于固定 OS 线程；创建、使用、回调与销毁分别符合要求 |
| **15. 共享状态 重入与锁顺序** | 明确哪一侧负责同步；避免持业务锁调用外部代码；宿主与原生的嵌套调用按重入协议处理 | Mutex、原子变量、锁顺序、隔离 owner、交错测试 | 原研究：K/N 新内存管理器允许共享堆访问，但不会自动消除数据竞争。[内存管理](https://kotlinlang.org/docs/native-memory-manager.html) | 不把 GC、shared_ptr 或 StableRef 当作线程安全；不默认需要 freeze；回调再次调用或关闭原生对象不死锁 |
| **16. 取消与实际停止** | 分开定义停止等待、停止观察、停止工作与撤销副作用；保留真实原生停止确认和后台清理责任 | 原生 cancel/stop、操作状态、取消适配、最终清理任务 | 原研究：MadDemo 网络请求明确等待取消不等于工作停止，可复用于直接原生边界的语义设计。 | 取消不得虚报原生已停；迟到结果有 owner；取消处理不提前销毁 callback context；超时和业务撤销分别验证 |
| **17. 取消与结果交接竞争** | 区分 native 完成、等待者恢复和资源成功交付；交付失败必须执行对应清理 | suspendCancellableCoroutine、resume 的 onCancellation、资源转移状态 | 原研究：协程的及时取消保证允许 resume 后、调用方恢复前取消；返回资源需要专门清理。[资源交接](https://kotlinlang.org/api/kotlinx.coroutines/kotlinx-coroutines-core/kotlinx.coroutines/suspend-cancellable-coroutine.html) | 按 CRIT-02 验证 resume 后未恢复即取消；结果交付失败不泄漏；清理符合资源线程要求，取消等待不提前释放 native 输入/context |
| **18. 同步 重复与迟到回调** | 先登记操作再启动；支持启动期间同步完成；统一终态认领；注销、关闭与在途回调协调 | 一次完成门闩、操作代次、回调注册表、屏障测试 | 原研究：MadDemo 相邻边界采用先登记等待者再发送的顺序。 | 不以 isActive 检查替代原子认领；重复回调不重复 resume；旧操作结果不完成新操作；重复通知的资源所有权按原生协议处理 |
| **19. Flow 背压与观察生命周期** | 明确状态/事件、容量、丢弃、顺序、多观察者和恢复；注销与订阅作用域绑定；需要时向原生反馈压力 | callbackFlow、awaitClose、buffer、共享订阅、有界分发、序号 | 原研究：callbackFlow 每次收集可重新注册，取消可与注册/回调并发。[官方 API](https://kotlinlang.org/api/kotlinx.coroutines/kotlinx-coroutines-core/kotlinx.coroutines.flow/callback-flow.html) | trySend 失败有明确处理；有限缓冲不自动减慢原生生产；关键事件不静默丢弃；页面停止观察与关闭账户级资源分开 |

## 四 构建 分发与验证

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **20. Release 与 R8 反向访问** | 从导出/回调清单生成精确保留规则；覆盖 native 访问的方法、字段、构造器与描述符类型 | consumer rules、描述符清单、混淆后消费者测试 | 原研究：默认 native 方法规则不等于保留所有 native 反向调用。[R8 JNI 示例](https://developer.android.com/topic/performance/app-optimization/keep-rule-examples) | Debug 通过不能代替 Release；不依赖全包 keep 掩盖缺失声明；改名与裁剪后真实回调仍成功 |
| **21. 库加载与 ClassLoader** | 明确加载入口、类加载器和缓存作用域；在正确上下文解析类；反向入口与库寿命绑定 | System.loadLibrary、显式 ClassLoader、加载器封装、符号检查 | 原研究：MadDemo 鸿蒙 C++ 壳以 dlsym 查找 Kotlin 入口，并保留需要继续使用的库。 | native 新线程不盲目 FindClass；不缓存失效类引用；仍有函数指针、析构或工作线程时不卸载库 |
| **22. 绑定与二进制漂移** | 锁定头文件、生成配置、工具链、绑定与库的配套身份；API、ABI、行为分别比较 | 再生成检查、manifest、ABI 探针、旧消费者、编译产物校验 | 原研究：MadDemo 编译与 cinterop 前验证候选文件，JVM/Native 使用不同生成源。 | 新绑定不混用旧库；@CName 与 dlsym 类型一致；签名不变但取消/线程协议变化也能被审查 |
| **23. 多架构与传递依赖** | 固定目标与原生依赖；KMP 库使用既定 Android-KMP 插件，app 独立；预编译 so 用 jniLibs，需要 externalNativeBuild 的库用独立 Android library | Gradle、CMake、cinterop 配置、nm/readelf、依赖清单、干净环境消费 | 原研究：MadDemo Android 按实际内核产物配置 ABI；JNA 的 jnidispatch 也是原生依赖。 | 检查最终 AAR/APK 及全部传递 so、最低系统和实际消费；官方/CPF cinterop 分别验证；组合内核不重复静态链接 |
| **24. Android 16 KB 页兼容** | 验证 ELF 和打包对齐，并检查代码对运行时页大小的假设；覆盖所有传递库 | zipalign、llvm-readelf、16 KB 环境测试、依赖升级检查 | 原研究：MadDemo 脚本检查 APK 和每个 .so，并包含第三方库。[Android 指南](https://developer.android.com/guide/practices/page-sizes) | 主库合格不能代替所有库合格；对齐通过不能代替实际运行；不依赖硬编码 4096 的错误假设 |
| **25. 性能与故障诊断** | 测量真实载荷的跨界次数、复制量、延迟和峰值内存；验证正反调用、失败、竞态并保留匹配符号 | Profiler、Perfetto、适用的 ASan/HWASan/TSan/UBSan、模糊测试、资源计数、符号化 | 原研究：小粒度 JNI 调用应减少跨界与搬运；mad-boundary 的 Verifier 关联受测制品身份。[JNI 工程建议](https://developer.android.com/ndk/guides/jni-tips)、 | 批量前后用真实业务基准比较；Sanitizer 按目标能力启用；符号匹配真实二进制；未运行、失败与不支持分开记录 |

## 本路线补充

- JNI Modified UTF-8 与普通 UTF-8 分开；C 的 NULL+0 与 Kotlin 临时 pinning 也需要按接口具体处理。

## 使用方式

按选定 profile 和本次任务读取相关条目；验收采用[公共问题目录](../../../validation/04-problem-catalog.md)、[关键交错场景](../../../validation/08-critical-scenarios.md)与任务 acceptance。本研究不维护独立进度、支持矩阵或第二套模块架构。共同约束与资料优先级见[归档入口](../README.md)。
