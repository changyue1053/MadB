# C06 Kotlin / Swift / Rust 四路线验收

任务 C06 · 规范基线 0.9 · 更新于 2026-10-25。起点为 MadB `cee264bbe93544d9700282ab444a93cc7202af4c` 与 MadB-Lab `13f0159da92d34746b096c2c07427a897967fe48`；当前任务状态以[实施目录](../catalog/implementation-plan.json)为准。

## 范围与结论

本包为固定 SampleEngine 接口提供四个独立 macOS Arm64、未插桩 Release 候选，并由独立 MadB-Lab 从公开文件编译和消费：

| 配置 | 后端与公开交付 |
|---|---|
| `kotlin-jvm-rust` | UniFFI 0.32.2 与受管补丁；JAR、依赖、头文件及动态库 |
| `kotlin-native-rust` | Gobley 0.3.7 / UniFFI 0.29.4 与受管补丁；KLIB、依赖、头文件及动态库 |
| `swift-rust` | UniFFI 0.32.2 与受管补丁；Swift 源码、头文件及动态库 |
| `swift-kotlin` | Objective-C export + SKIE 0.10.15；可分发 framework 及 Swift 并发门面 |

**C06 实现与本机验收已交付；远端整包验证按用户安排留到下一轮。** 固定干净源为 `b19d48f5f6fda584a6c47f2e0dc7059f5a080d01`，Lab 为 `c53a64ef2c160fa7ca9b8e23d6365b84d882fc41`。Lab 92 项 harness 检查通过；54 个案例中，51 项是目标运行，3 项是预期编译/链接拒绝，不能写成54个运行成功。当前源码触发的基础检查和 C05 回归共四个远端作业成功。用户于 2026-10-25 决定本轮不再等待 C06 远端作业，下一工作包完成后统一验证；本轮远端作业已取消。目录登记为 `implemented / missing`：实现已交付，整包验收尚未通过。

工作包验收只覆盖上述选定 profile。它不表示任意接口都可安全生成，也不覆盖 Android、iOS、CPF/OHOS、其他 Apple 工具组合、SKIE + NativeCoroutines、UI 便利层、`madb init` 或正式发布渠道。deployment target 14.0 是编译参数，不是最低系统实跑证明；C09/C10 全矩阵继续按各自任务收口。任务验收不自动更新全路线认证登记。

使用入口：[公开复现指南](../examples/managed-routes/README.md)。工具来源、完整身份和依赖以实际候选 profile/lock 为准；候选不能通过修改内容或沿用旧工具安装冒充当前源。

## 八项验收

下表记录本轮已确认的本地结果。证据绑定下方四个候选及报告；远端 C06 留待下一轮统一验证，本地通过不授予整包验收通过状态。

| # | 要求与实际验证落点 | 本轮结果 |
|---|---|---|
| 1 | JVM、Native、Swift 从公开 JAR/KLIB/framework/Swift 源独立编译；四 profile、同次工具 observation、实际装载及完整工具/依赖/补丁身份 | 本机四候选、12份构建证据及四路 identity/distribution 均通过；非provisional，统一tool digest见下方 |
| 2 | 类型、空值、整数、字节、变体、错误、对象、异步完成、取消方向、迟到结果及显式关闭；Swift 6 strict、actor 前值复制、关闭/排水后引用释放 | 四路 data/errors/objects/async/cancel/late-callback/shutdown 本机通过；GC 最终回收时刻不作保证 |
| 3 | 后端负责既定 ABI/类型与协程绑定；受检 IR 生成薄入口；runtime 负责预算/所有权/停止；Swift facade 只负责入口与回调排水 | 读取正反例纳入已成功的基础检查，四路独立真实消费通过；适配面仍限 SampleEngine |
| 4 | Gobley foreign future 旧模板双完成复现及修复；回调抛错、取消/迟到结果只交接/释放一次；0.29/0.32 各自保留补丁与回归证据 | Native公开 foreign-future-once/callback-throw/cancel-late-result 三项本机通过；0.32旧探针范围见后文 |
| 5 | SKIE 可分发 framework；真实 Swift Flow/suspend/enum/sealed 调用；custom cinterop 来源和泛型 lambda 在指定编译阶段准确拒绝 | enhanced-api目标运行及missing-cinterop-origin/generic-lambda编译拒绝均通过；不含NativeCoroutines组合 |
| 6 | DEC-08/CRIT-11：1024 载荷字节、4096 保留字节、8 events、1 state slot、32 waiters、每引擎1工作线程；满额、合并、stop/drain/ACK 和资源归零 | 四路 capacity/late-callback/shutdown 本机通过；退出限活运行时的实例owner |
| 7 | DEC-10/CRIT-12：预期 profile 独立于库自报值；后端 checksum/bootstrap；错库/错 ABI/profile 在业务资源创建前按声明阶段拒绝；工具库存拒绝篡改与逃逸 | 四路wrong-profile及三Rust路线wrong-abi在load拒绝，SKIE wrong-abi在link拒绝；本机全部匹配预期，非超时/崩溃 |
| 8 | DEC-11：四个最低完整 profile，精确 12+15+12+15=54；P01/P02 独立、P03/P04 按组合；C05 回归独立保留 | 本机54/54通过；当前源触发的baseline/C05四项远端成功；远端 C06 下一轮补验。P05/CPF 不借此通过 |

载荷预算按 UTF-8 字符串与字节数组的逻辑字节计算，不等于堆内存或序列化峰值；每引擎一个业务 worker 不等于进程只有一个线程。同步外国回调可请求停止，但须先返回，再由独立 owner 等待排水；取消等待不保证中断同步外国代码或撤销副作用。

## 当前身份与报告

| 项目 | 真实身份或结果 |
|---|---|
| MadB 分支源码 | `b19d48f5f6fda584a6c47f2e0dc7059f5a080d01`；正式源码身份以 candidate 的 `producer_revision` 为准 |
| MadB-Lab | `c53a64ef2c160fa7ca9b8e23d6365b84d882fc41` |
| 本机正式构建 | `/private/tmp/madb-c06-formal-b19d48f` 的四个非provisional候选；12份build-evidence已复核。四报告统一tool digest为 `sha256:464448c7d4b307867e1a0ea4aa76aa8b157d7bd7b6e36964d8890c132d5992c6` |
| 本机独立 Lab | `/private/tmp/madb-c06-lab-formal-b19d48f`：92项harness、54/54案例通过；4candidate/12build-evidence/4report/54case的schema及162个附件引用校验通过。汇总 `/private/tmp/madb-c06-local-b19d48f-summary.json` 的SHA为 `02931af4c01681c9590f31df354010a0285024ab7d0b17c88661aa0645e9f7c5` |
| 远端 C06 | [本轮运行 40157655879](https://github.com/changyue1053/MadB/actions/runs/40157655879)按用户安排已取消，结果不计通过。远端 JVM、Native 两份正式候选及其公共准入通过，producer 均为实际 PR merge `e45ed94048c2898d7f6795f116eb9da5d3197fe3`，其 tree 与 b19 相同；Swift/Rust 构建时取消，SKIE 未开始，远端 Lab 未执行（0 个案例） |
| 当前源基础检查/C05 | [baseline PR](https://github.com/changyue1053/MadB/actions/runs/40157655799)、[baseline push](https://github.com/changyue1053/MadB/actions/runs/40157650688)、[C05 PR](https://github.com/changyue1053/MadB/actions/runs/40157655823)、[C05 push](https://github.com/changyue1053/MadB/actions/runs/40157650680)均为SUCCESS；四run触发head均是b19，两个push实际checkout也是b19；两PR日志实际checkout为 `e45ed94048c2898d7f6795f116eb9da5d3197fe3`，tree `0254e1befe3397f0975d12c8f545aaaea115522a` 与b19相同；配套摘要见 `/private/tmp/madb-c06-ci-b19d48f-supporting-checks.json`，不代替C06 artifact producer核对 |
| 文档收口提交 | 本记录所在提交仅收口说明、任务目录和派生 HTML；功能验收仍绑定 b19。142个冻结作者/schema文件与 b19 一致，集合摘要 `sha256:a5ee73f95fbc852bc0a7de47b35f1e360ae8640d9807abdf298d00ca5c370482`；完整 diff 及提交后闭包对比单独复核，不改写旧候选来源 |

| 本机 profile | candidate digest（SHA-256） | 同名 `.report.json` 的SHA-256 | Lab 必需案例 |
|---|---|---|---|
| `kotlin-jvm-rust` | `4281673f425f80cd327bf5daf7de5db11e71da8f038dd779e3da39d8e02588ea` | `423cfee0d8c745b7121d678427bf5cdf3e5a766043040f2d43d63896b7caa09e` | 12/12，均target-run |
| `kotlin-native-rust` | `e8bbe497f97380a07b55d9f8023cef9dd1c36192d4ee096059d086255734c6e4` | `347a75910e593f091d40d7218fcd8fbf689f0d5ed811d6886c0a08795d360e59` | 15/15，均target-run |
| `swift-rust` | `cc1651d76baa491fccd04691a8fadd1cc5c5ec0533cf22eee674eb7eafd2ce9a` | `48fbf4ff1c8f82dd82c469fae13b3bb57adae6f8635709e03239c1eb2c8d1871` | 12/12，均target-run |
| `swift-kotlin` | `872d0cbd49bee21d442f24be6536334a7cc8dff3a525839f2a889a7776ac908c` | `6012e18eb3a54dcb143d94fb6b746b6b02de967f5322fd44e23d46b7cf9c5040` | 15/15：12 target-run，3 compiled |

本机完整日志 `/private/tmp/madb-c06-lab-formal-b19d48f.log` 的SHA-256为 `c00887fbf1914045f0d8d8fe7a0bd471a120700ccc20c69d55c85825cc90ff5c`；独立摘要清单 `/private/tmp/madb-c06-local-b19d48f-report-hashes.json` 记录上述四报告、日志与汇总摘要，已逐项重新计算核对。

取消前保存的远端 JVM、Native candidate digest 分别为 `sha256:cffbfa8ce09881b01b05034adf2ca3133e5f12ec047f81e48ac3946227f939c5`、`sha256:2c2fca383962c6b19a7c4700ed7524626d7e5cee8a6fb8bff6b0142eb7c74268`；run 的 preflight/candidates artifact ID 为 `12586547568` / `12586817319`。两份候选的构建及准入证据不替代尚未执行的远端消费者验收。

远端候选记录其自身真实 producer、工具镜像和候选摘要；不同宿主身份不得拼成同一个 profile。PR 交付于 [MadB #12](https://github.com/changyue1053/MadB/pull/12) 与 [MadB-Lab #3](https://github.com/changyue1053/MadB-Lab/pull/3)，草稿/合入状态以交付时 GitHub 为准，不从测试通过推定合并。

## 审查落实的设计与缺陷修复

| 责任 | 最终设计与验证要求 |
|---|---|
| 共同契约 | 显式 `throws` 与 `backend-handle` 进入受检模型，旧文档摘要兼容、新能力显式协商；见[契约规范](../architecture/03-contract-and-ir.md#c06-的错误通道与后端句柄) |
| 所有权与异步 | Rust ready 结果在领取前保留 Rust 所有权；foreign 结果单一接收守卫；Gobley 完成通知不进入业务错误二次完成路径；Swift cancel/free 同步，各版本补丁分别验证 |
| 并发与关闭 | 外国调用、waker 和最后引用析构均在内部锁外；cleanup ticket 占用准入额度；Kotlin 独立清理 ACK；多名关闭等待者共享完成，取消等待不取消关闭；Swift 不增加第二生产队列、不普遍授予 Kotlin 对象 Sendable |
| 可复现构建 | 保留正常 UUID，不改写二进制；SKIE 保留原有 DWARF，Rust 沿用既定 Release 配置，本包不承诺交付 dSYM。Rust/SKIE/Native 私有 smoke 采用输入身份绑定的受管冷目录及独占所有权校验；SKIE 私有 smoke 单独关闭 chained fixups，Native 私有 smoke 固定中间目录；不是构建缓存或公开 SDK 语义降级 |
| 进程与工具身份 | 进程组 leader 保持可等待，停止后才清理标记；Darwin libproc/Linux proc 观察受管组，范围不含主动逃逸进程。Python 解释器/库、Swift 实际镜像/别名、源码解析器与补丁配方均锁定；Python.org 内部缺失链接保留目标与缺失状态 |
| 预算与产物准入 | C04 每次阶段端口执行900秒、终止宽限6000毫秒，内部编译与运行保留各自有界预算，远端整包240分钟；没有削减必需案例。runtime protocol 在选定路线的 SDK 生成和编译前检查 schema/版本/预算形状；共享工具观察和声明 helper 准备仍先执行，Lab 独立复核。符号链接、特殊文件、逃逸或未声明输出均拒绝 |

## 阶段失败与证据边界

| 阶段 | 保留的事实与处理 |
|---|---|
| `14d0c53` | Native 私有 smoke UUID/签名字节不一致，由 C04 拒绝；[远端失败](https://github.com/changyue1053/MadB/actions/runs/40106593797)实际 PR 检出 `cf991ebeb9e381d9caa32ef9fa02ca365b506c91`，tree 与14d相同。固定中间目录后的专项5冷构建/24产物一致与 Swift/Rust5冷链接分别保留，后由527完整构建验证 |
| `527cb23` | 四路正式构建成功；两份 Rust 作者 runtime protocol 缺 schema，Lab 在准入阶段拒绝：92项harness通过、0个SDK案例，不能写54通过。证据 `/private/tmp/madb-c06-lab-formal-527cb23/admission-summary.json`；[baseline](https://github.com/changyue1053/MadB/actions/runs/40155539333)与[C05](https://github.com/changyue1053/MadB/actions/runs/40155539358)通过；旧C06远端请求停止不计通过 |
| `b19d48f` | 补齐两份作者schema、生产者早期严格校验及23项c06_project测试；v9经公开installer重建。四路重新正式构建，本机92harness与54/54案例通过；远端 C06 按用户安排下一轮补验。没有沿用527候选修改字段后认证 |
| C05 独立回归 | 本机 producer `4d3e8710343dabc528a24ac1de6eedca825476c3`、Lab `b7d3a50aa8107a1f397140b2da67511019e8b946`；18路线+1包装案例、87harness通过，三候选非provisional。该证据保留原身份，不能冒充当前源码重跑 |

UniFFI 0.32.2 复用的是已有 v7 后端/探针子闭包证据：原始失败/修复通过、Swift 6/JVM P03 和1000次Swift取消竞态。只读复核确认 v7/v9 原始源码、284个后端源文件、五补丁、五探针及11份原日志摘要一致；19项作者inputrefs中仅 Gobley 专用条件模块与 runtime protocol schema 新增。前者未进入0.32探针编译，后者已通过本轮新候选的本机 Lab 准入及54案例，远端验收另行补齐。工具二进制在新安装中身份不同，各自与receipt匹配；不能称v9全量探针重跑或旧测试覆盖新交付。

已知限制保留两条：旧 C04 出现过5秒消费者启动超时，后续原配置68项通过仍不足以确定根因；远端0.29 metadata helper的rust-objcopy缺libLLVM造成debug-info清理warning，Cargo仍成功且helper真实字节被锁，未观察为当前功能阻断，但未证明已修复。不得据现有日志宣称全部公开provider构建均无此warning。

详细本机证据保留：`/private/tmp/madb-c06-c05-regression-j8yq5lg5/summary.json`、`/private/tmp/madb-c06-skie-cancel-public-proof.json`、`/private/tmp/madb-c06-gobley/unsigned-final-probe/observations.json`、`/private/tmp/madb-c06-uniffi-v7-regressions/report.json`、`/private/tmp/madb-c06-uniffi-v7-source-integrity/report.json`、`/private/tmp/madb-c04-package-targeted-capture/report.json`、`/private/tmp/madb-native-fixed-workspace-proof/report.json`、`/private/tmp/madb-c06-swift-rust-link-probe-01/report.json`、`/private/tmp/madb-c06-rust-objcopy-assessment.md`。它们是本次审查追踪，不是公开SDK构建依赖；本机新报告已绑定上表，远端完整证据由下一轮补齐。

## 下一轮统一验证

下一核心工作包 C07（ArkTS 四路线）完成后，统一运行适用的完整基础检查、C05 回归、C06 四路线冷构建与独立 Lab 54 项，以及 C07 的必需验收。使用届时的固定生产者和 Lab 提交，重新绑定实际候选与工具身份；本轮本机报告保留原提交，不改写成下一轮证据。C06 只有剩余必需验收完成后才将 `verification` 改为 `passed`；两份 PR 当前保持草稿、未合并。

本轮文档门禁：静态链接/格式/生成一致性、21 项文档契约测试及隔离脚本逻辑测试通过。浏览器安全策略拒绝本地 `file://` 预览，未执行视觉验证；隔离 DOM 测试不作为浏览器实测。
