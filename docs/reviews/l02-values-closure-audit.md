# L02 收包只读审核

导航副本：仅将目录及带行号路径链接改为文本，规则和结论不变；原始受审字节 SHA-256 为 `80d2373fdc07a66deb7f054cf0b6c8126720dedc53340bb20b48f0843876d4c2`，原稿保持于[持久归档](l02-values-evidence-archive.json)。采用或复核仍绑定该原稿。

**当前不能收包：新有限值配方的 Linux x86_64 出口尚未完成。** 已检查的旧诊断/explain、Mac 新值配方、同根变更事务及独审范围没有新增阻断。待下面列出的 Linux 实际检查和证据收尾完成，按当前已批准范围可将 L02 收为 implemented / passed；无需增加 L04/L05/L06、工程 I 链、C09/C10 或 L07 整包前置，也无需重新审批 ADR-0022。

## 审核输入与边界

实际产品根 `/Users/changyue/Desktop/MadB/madb`，读取 HEAD 为 `e0f74a5fa5a9a6e73400ced70bc81ba06751227f`，`git status --short` 无输出。catalog 当前 L02 为 `in_progress / missing`，依赖仅 C05、L01，二者均 `implemented / passed`。独立 Lab 固定为 `2d06cbefa0f49187f163ce937c8c3bd01a062eb1`。

实际读取 L02 五条 acceptance 与七份 documents 的适用部分，重点为 architecture/15 普通入口、声明生成与诊断边界，validation/07 的 DX-01/02/10/11，architecture/08 的 DEC-07，delivery/04 的首链准入，architecture/17 的唯一阶段责任，已批准 CLI 归属及 ADR-0022。Swift/ArkTS、宏/UI、C++ RAII、IDE/工程完整配置和公开分发的额外出口仍按原工作包负责；ADR-0022 的第二业务明确是有限同步 Rust→C 值配方，没有新资源/async/回调或新目标承诺。

本审核读取实际证据、重算相关源码摘要并核对报告条目；未重跑 SDK、编译器或 Linux 容器，未改产品/Lab 源码、catalog、历史报告或正式候选。

## acceptance 逐条对账

| L02 现行验收 | 已有证据 | 尚未完成的本包出口 |
| --- | --- | --- |
| **1. 复用后端声明和来源映射；普通入口可独立消费** | round6 既有 cbindgen/Clang/受检 IR 来源、真实 reader→explain 和 checked constructor；旧 Mac/Linux 各5个正式 L02 案例。round7 自然 Rust 是唯一作者结构，syn/rustc 见证→shim→实际 cbindgen/Clang→受检 IR→Source/Build/Candidate。Mac clean producer 的7个正式候选已独立消费；普通 C 使用公开头/库与自动身份入口，不填内部摘要，不依赖 MadB CLI/核心或作者缓存。 | Linux 新7候选的实际产出、外部 pins 核验和正式29案例消费尚不能记通过。旧 Linux 五配置不能替代新值配方。 |
| **2. 关闭诊断仍保留资源/线程/状态机保护** | round6 两目标 `disabled-protection` 正式案例 passed，原始 off/self-wait/exporter 观察有精确谓词；日志/计数关闭不关闭准入、关闭等待、重入拒绝。旧实现相关源码从 fbb0a49 至当前未变。round7 Mac 新生成旧配置又有5例 L02回归与19例C05回归。 | 旧诊断子范围无新增必需出口；不把此结论扩大为任意线程/裸指针安全或全部资源计数。新值配方没有另造诊断/资源机制。 |
| **3. 首链普通资源/回调与生成诊断先贯通；诊断、静态属性、运行检查分别声明覆盖** | ADR-0021 的64槽 configure/drain、7/9普通/诊断导出与伴随配置保持；两目标真实候选消费覆盖开关、容量/FIFO、隔离、重入/争用与故障；C05原资源/回调及正常关闭单独有正式报告。静态 surface/属性支持、运行保护、sanitizer/平台限制分别记录，没有将静态属性称为内存安全证明。 | 不需要把 L06 的 RAII/move-only、Swift OSSignposter接入、ArkUI真实页面或未来插桩矩阵移入L02。已计划的 Linux 新值原生专项须实际留证，尤其 allocation/cleanup 与可恢复 unwind；它们与 Lab helper-only 案例分列。 |
| **4. DEC-07：每符号阶段责任；普通/便利入口共用机制，不重复转换/订阅** | 旧7/9角色由受检IR/政策建立27/33行表并核心比较，checked helper仅委派原bootstrap/create。新值配方从冻结作者、完整SourceKey/派生ID、精确类型/政策重建 required/provided；两项门禁缺口已修。Source实际表字节、Package保留及 omission/换reader负例有验证；普通值wrapper只做身份/协议核对，raw转换/作者调用各一次，无额外资源owner。 | Linux新候选自身责任表、源输入、生成/构建/候选血缘仍应与实际 pins 一起核对。不能仅复制Mac表或给纯值补造订阅/排水责任。 |
| **5. explain：码义、触发、声明来源、修复入口；未知码/机器输出/退出正反例；只读** | ADR-0020 的51词条、21个真实CLI场景、14项受控读取分类/零读取检查、26个格式向量，以及2类真实reader→10次文本/JSON查询已有独审；未知码明确失败，源位置绑定实际字节，无隐式探测/构建/网络。旧explain相关实现未变。 | 无新增已知阻断。退出0/2/3有真实CLI场景；退出4由受控ReadWorkspace重检分类、格式向量与CLI映射覆盖，不写成真实文件系统竞态已复现。 |

旧证据的逐文件对账见 [l02-prior-evidence-audit.md](/private/tmp/madb-chief-round7/l02-prior-evidence-audit.md)。round6 权威版本化入口为 [冻结候选记录](/Users/changyue/Desktop/MadB/madb/docs/reviews/2026-10-26-l02-frozen-candidate-validation.md)及其 final-macos/final-linux 执行、独审与持久归档索引。两个目标各84项旧观察准确分为56次真实SDK进程、24次helper stub、4个导出检查，不称84次SDK运行。

## DX-01/02/11 第二业务与同根事务

- 五份自然作者依次完成：author-0 基线 normalize；author-1 增 offset/Result；author-2 增 quality；author-3 改 canonicalize；author-4 删 offset/错误enum。作者不写额外字段IDL或逐方法表，不修改MadB私有业务白名单或生成文件。
- 最终 clean producer `e0f74a5` 已重新构建 host/reader 并复跑两项真实 live 测试，均通过：[最终执行记录](/private/tmp/madb-chief-round7/final-clean-live-001-evidence/result.json)、[实际日志](/private/tmp/madb-chief-round7/final-clean-live-001-evidence/live.log)、[五步结果](/private/tmp/madb-chief-round7/final-clean-live-001/live-result.json)。前后 producer/Lab 均 clean且 revision一致，source/三个执行镜像/全部工具摘要不变；本审核又读取当前实际镜像字节核对，没有差异。这是当前提交的同根证据，不再仅依赖早期开发运行。
- [live-e结果](/private/tmp/madb-chief-round7/values-live-transaction-e/live-result.json)与[实际测试日志](/private/tmp/madb-chief-round7/values-live-transaction-e.log)记录同一 managed/domain、固定 boundary/profile ID/target/namespace 的五次真实 metadata→generate→compile/link/run→package。Result所需features变化会合法改变profile digest；不称五步使用同一不可变profile。
- author-2 后实际拒绝责任表遗漏、编译隐式输入/环境、冻结作者变化、未知政策selector、冲突政策、错误生成selector及不支持声明；每次旧current和完整旧candidate字节保留。author-4检查旧normalize/offset无残留普通头/原生导出。测试只更新recipe输入，既有事务域保留。
- 本次将 live-e managed 中13份相关生成器/流水线/reader源码与实际产品逐字节核对，全部相等。live-e保留为较早开发同根证据，最终clean复跑承担当前提交的同根证明；独立正式消费者另验证新7候选，二者职责不同。七个fresh目录本身不证明Linux同根增量，也不冒充IDE操作录像；当前验收不据此新增每目标IDE专项。
- Mac正式29例由7份report/1组成，全部passed：**18个真实SDK案例、5个helper-only运行案例、5个static案例、1个compiled删除API拒绝**。23个target-run条目包含5个helper-only；混候选案例有两次真实进程，因此有19次native进程，但仍是18个SDK案例，不能混计。
- [集成后复验](/private/tmp/madb-chief-round7/integrated-macos-values-001.json)再次对同一7个候选运行29例通过，不增加独立候选数，也不把重复执行算作58个不同案例。所有观察仍明确 `full_l02_acceptance=false`，保留该局部原义，由主协调合并各子范围证据作catalog判断。

正式生产记录：[7个值候选](/private/tmp/madb-chief-round7/formal-native-values-macos-r7/producer.json)、[5个旧配方回归候选](/private/tmp/madb-chief-round7/formal-first-route-regression-macos-r7/producer.json)。正式Lab原结果在 values目录（原路径：`/private/tmp/madb-chief-round7/formal-macos-lab-r7/values`）和l02目录（原路径：`/private/tmp/madb-chief-round7/formal-macos-lab-r7/l02`），独立身份/附件审查见 [values-l02-audit.json](/private/tmp/madb-chief-round7/macos-review/values-l02-audit.json)。

## 最终源码与独审的串联

- [原生转换独审及修复后追加](/private/tmp/madb-chief-round7/native-independent-review.md)：payload析构二次panic问题已修；可恢复路径返回8并清零、作者/析构次数与分配余额有真实证据。再次清理失败明确abort，不声称abort/OOM abort可恢复。其5份最终被审文件摘要与当前产品全部相等。
- [pipeline独审](/private/tmp/madb-chief-round7/pipeline-independent-review.md)：实际读取/生成、物理ABI、出包原子末检、名称冲突与同根预备目录已核；12份被审文件摘要与当前产品全部相等。
- 最终reader的[primitive/std追加独审](/private/tmp/madb-chief-round7/reader-primitive-independent-review.json)有50向量，5份文件与当前产品相等；其中13个新增用例有实际rustc见证，旧35个仅parser复放，保持该分层。
- [责任门禁独审](/private/tmp/madb-chief-round7/value-ownership-independent-review.md)的主gate/接线仍同字节，但其早期names.rs pin与最终std准入修订不同。因此不宣称旧报告全部依赖等于最终提交；以最终reader追加独审和[集成后ownership 8项](/private/tmp/madb-chief-round7/integrated-product-ownership.log)、[reader 17项](/private/tmp/madb-chief-round7/integrated-product-reader.log)闭环。既有报告不回写。
- Lab实现独审、clean `2d06cbe` 与最终候选/报告的独立核验分开；helper坏provider不能替代真实Rust作者计数或try_reserve故障证明。

## 剩余出口与收包条件

当前新Linux正式出口仍为未完成，具体应沿已准备的固定执行计划完成：

1. 从clean producer `e0f74a5` / clean Lab `2d06cbe`实际生成全部7候选；校验source、profile、target、工具、required_cases及完整资产，独立确认外部candidate pins。单有构建exit0或候选目录不能当运行通过。
2. 以固定Linux x86_64/QEMU环境和既定Clang配置实际运行7份report/1的完整29案例，冻结前后核对身份、用例、工具、输出及清理；分类保持18 SDK / 5 helper-only / 5 static / 1 compiled，不用旧round6 Linux结果替代。
3. 完成已经安排的Linux native codegen/client专项并绑定实际生成shim及源码，记录分配失败、临时值/payload清理与预期abort的真实阶段；它不是Lab report/1，不把helper-provider计数当SDK计数。若某项失败/超时/缺失则保留并处理，不删除检查或降低预算。
4. 对Linux最终产物/报告/日志做独立复核，与当前Mac证据共同固化持久归档和版本化收口记录；保留失败历史，更新catalog唯一状态，按现有文档流程改源、生成HTML并执行适用文档门禁。

在以上出口完成且没有新的源码变更或新增失败后，**可按现有范围关闭L02**。这是对当前两目标、两个有限配方、已批准explain/诊断及普通C消费的判断，不授予Swift/ArkTS完整平台能力、C++、真实页面、所有工具链、物理x86性能或公开发布资格。Actions按用户既定决定在项目完成后独立补跑；未执行仍记未执行，不设为本次临时追加前置。

## 必须保留的证据限制

- Mac首次C05因SDKROOT缺失失败仍保留；补充Clang/SDKROOT执行19 passed，其中17 target-run、1 compiled、1 static。两个Rust错配案例的Cargo build原始日志被同名运行日志覆盖，现有运行/锁/metadata/编译归档及固定runner支持结果，但不得称全部构建日志完整留存。该限制见[C05独审](/private/tmp/madb-chief-round7/macos-review/c05-audit.md)，不改写旧C05已验收范围。
- round6 Linux原GNU cc失败不因后续固定Clang成功变为通过；QEMU目标不等于物理硬件或性能认证。
- 原始local observation、provisional开发候选、正式report/1、源码独审和新候选重跑保持分层；不以编译替代目标运行、不修改历史结论补齐身份。
- 本审核是既有证据审查，没有执行新的SDK/编译/容器检查；Linux状态以审核启动时尚在构建的真实快照为界，后续完成需要追加真实记录，不能由本文件预授通过。
