# 第十二批：工程持续使用、Dart独立扩展与ArkTS生命周期

承接[第十一批](2026-10-27-chief-architect-eleventh-batch.md)。任务状态只维护在catalog；本文记录本批范围、决定、实现及实际证据，不另建任务系统。

## 基线、隔离与责任

实际MadB为`2fac2f7437e7bc235392fcca8c5d7af86b510e22`，实际Lab为`7a252092a0b50850c961f7d617facce20fbc57a2`，均为干净的`codex/c07-arkts`。分别核对1426/540项tracked字节和模式，与复用的开发副本完全一致。产品旧C07 detached工作树保持`c8e7e01c31a7bc23abf6716abcc85cb47d47f418`。初始快照在`/private/tmp/madb-chief-round12/baseline.json`；其他列出的MadB聊天非活动，既有子任务已停写，仅观察到已有hdc服务，没有活动编译器。此观察不保证后续无其他写入，集成前仍须全量基线复核。

复用`/private/tmp/madb-chief-round10/repo`与`/private/tmp/madb-chief-round10/lab`，分别切换到`codex/round12-engineering-dart`和`codex/round12-arkts-lifecycle`。本轮证据使用新的`/private/tmp/madb-chief-round12`，不写回前批归档。实际两仓根只由主协调在复核后集成；共享schema、注册/导出、catalog、规范和集成单一写入，子任务先申明精确文件范围。

## 本批子范围和退出条件

- **E04 / P10**：C05、E01、E02、E03前置均已通过。以一个真实目标的Dart Native→C ABI同步子集开始，核正式工具和公开输入，先通过类型、错误、显式资源关闭的可行性探针，再交付公开组件组合的独立包。依当前E04四项验收及适用EXT检查收口；手工互调、生成或编译成功均不代替真实独立消费、禁用隔离和最终冻结复验。当前Dart示例为设计样例，不能当作已安装清单。未冻结的普通Dart API、所有权映射或其他新增公开语义先形成具体提案与独审；决定不影响的探针与内部接线继续进行。
- **I01 / I02 / I03**：从已采用ADR-0019/0023/0024/0025及当前0.1.1配方出发，定位可直接实现的正常工程入口、Wrapper与P09四类基线。I01负责已有工程受检差异、文件所有权、基准复核和恢复契约，I02负责应用编排，I03负责Wrapper及原生构建接线；已明确接口的子范围可提前实施，I03整包仍等待I02。新命令、wire或兼容承诺按既有决定流程定稿，不重复审批已有init/doctor格式，也不以doctor静态预检替代构建运行。
- **L05**：基于旧固定C ordinary公开候选及真实页面消费者，优先补后台到恢复、页面订阅与共享owner的实际行为；先核现有公开入口能否控制关闭失败重试及已入队迟到回调。不可控制的窗口保留缺口，不能以宿主替身或排水后发布冒充Native交错。Lab独立定义预期，只使用公开候选和源补充；已有hdc/设备串行协调，不杀共享服务。原生生命周期与并发改动须独审。

E04作为本轮优先争取整包收口的目标，只有实际验收齐备才更新catalog。I/L线按既有完整条件逐项推进，不预先承诺整包通过。第十一批Swift GUI006及C06/C07历史结论保留；L03、C08/L06、C09/C10联合队列不因本轮有限切片被省略。

依据为各任务catalog的documents、当前实施计划及决策风险入口。实施使用现有端口和有限配方，不增加万能插件层、第二套构建机制或Agent调度框架；适用第三方资料以正式版本和固定源码核对。

## 验证与交付

先记录受影响行为的正反例，再实施、独审、冻结候选并从独立Lab消费；实际根集成后重新验证对应候选。错误、取消、编译、运行、独立验收与发布分别记录，失败原件保持。文档从catalog/源模板生成HTML后执行定向门禁。

阶段GitHub推送仅使用`[skip ci]`提交并核对精确远端HEAD和Actions运行记录；不创建PR、标签、发布或dispatch。Actions未执行不能写成通过。

## L05：后台与恢复的首个冻结候选

Lab新增独立`l05_background_lifecycle`消费者、执行器、观察解析和测试，共十个文件；主协调另在`scripts/check.py`登记测试。普通C HAR仍为`sha256:7fce7dc8b8befbb985694ca08dfecff0b769fef5d0443608b61cc79edf1d921a`，生产者`18256fcc4b26cf3cd1e2e79c39b86c2512e012d9`。公开Observation源码补充沿用既有固定字节，不将源码补充写成新发布SDK。

消费者由真实UIAbility回调处理后台/恢复，同一页面和应用owner保持Runtime/Engine及普通观察者。后台先同步请求停止页面订阅，实际await排水后发布；普通观察者应收到后台值，页面模型应保留前台值；恢复等待上一关闭任务完成才重新attach。三轮后依次关闭页面、普通观察者、Engine、Runtime。独立oracle固定21个有序frame与9个UI阶段；每阶段四个字段必须来自同一张真实hierarchy。它不控制已排队迟到回调，也不等于故障关闭重试或整个L05通过。

生命周期独审发现并关闭两项问题：UI匹配不能拼接不同快照；只有全部public close成功之后，清理产生的合法生命周期回调才可无操作返回。十文件hash与独审逐项相符，完整Lab仓库检查通过（520项harness测试及其余各组21/18/17/6/28/28/20/12/9项），冻结clean候选`732b30f52199ef6e3be5ac09d3498972cb1edc96`。证据为`/private/tmp/madb-chief-round12/l05-freeze.json`和`l05-background-code-review.md`；这些检查不认证SDK。

首次正式`l05/formal001`重新构建Release并在既有API23 Arm64模拟器执行：编译通过，第一轮真实后台排水、普通观察者收值及同PID恢复已出现。但恢复日志带有系统HiTrace前缀，当前解析器要求协议marker紧接日志头，因此准确返回失败并停止；没有放宽业务预期。该报告的target/native/ui均为failed、runtime_close为not_confirmed，限定清理passed不能替代public close。原报告SHA256为`9e015c805fa36135716788156a7430c057e33ebfee6d73d38312780837c4e873`，失败原件保留；下一步核实固定平台日志格式后作窄修、独审和新候选复验。

## 工程入口的实际观察

`engineering/p09-003/result.json`记录当前0.1.1冷init实际run通过；同根直接CMake的无变化、Rust实现修改与公开字段改名均报告无需构建并继续输出旧`7:x`。这与当前CMake只导入冻结SDK、init只接受全新native根的设计一致，不能当成正常作者增量链已接通。P09仍缺完整嵌套进程计数和GUI证据；原样本不作性能承诺。拟议的日常重建入口、Wrapper和新格式须在具体提案及独审后由所有者决定，旧init/doctor/模板语义保留。

## L05切片收束与阶段交付

按官方OpenHarmony 6.1固定源码`84928bb7fd12d954c06fc4c0e6cdfc64b0f54459`及当前SDK的60/26/26位HiTrace定义，仅窄修新parser及其测试。包络只能在消息起点出现一次，PID/tag/nonce、数值和帧序保持；20项定向测试与[delta独审](twelfth-batch-l05-trace-review.md)通过。旧037原件复读只有7帧incomplete，不能补成完整运行。新Lab clean冻结为`f6f1d1bf071616d68cf68441adf9ded85e64b91b`。

正式002以新Release HAP完成原三轮场景，21帧/9同快照UI阶段和最终public close全部成立，原件独审通过。随后主协调对实际两仓原始字节、HEAD、分支和工作树复核，实际Lab快进至同一提交，550项tracked文件与开发副本精确相同；实际产品根与旧工作树不变。集成后003再次从实际Lab根新编译和运行，通过相同预期及独立原件审查，见[集成后独审](twelfth-batch-l05-integrated-review.md)和[实际观察摘要](twelfth-batch-l05-integrated-validation.json)。003的report SHA256为`fd1074c945a685c6b92771d1fe4beb1e42b810a10d99b7a1d065381f7837b5b9`；88条实际命令、21帧、9阶段，public close发生在限定OS清理之前。001失败、002通过和003通过分别保存，不改历史结果。

本L05证据已冻结归档到`/Users/changyue/Desktop/MadB/.local/evidence/round12-engineering-dart-arkts/l05`；2017项、352732660文件字节、25个不跟随的符号链接，复制前后全量核对。相邻`l05-manifest.json`的SHA256为`d5124afb26d68a2062b89d7566b2bf07b8784b1daef8763314036ccd28314a1b`。该归档只含已停写L05及基线/集成证据，不冒充仍在实施的Dart/工程线已经归档。

按已授权阶段交付，Lab的`732b30f`和`f6f1d1b`两条`[skip ci]`提交已推送至既有`codex/c07-arkts`。远端精确HEAD为上述f6f1d1b全SHA，逐提交Actions查询均为0；原件在本批`delivery/lab-stage-push.json`及对应Actions响应。没有PR、标签、发布或dispatch；Actions未执行不是通过。L05仍按catalog保持整包未验收。

## 两项新决定及实施责任

项目所有者已分别采用[ADR-0026](../adr/0026-dart-sync-extension-contract.md)和[ADR-0027](../adr/0027-project-rebuild-contract.md)绑定的最终稿，不重复询问既有授权。

E04分为三段并行工作：reader/generator及公开会话组合根；`sdk/dart/ffi`运行资产；独立Lab公开消费者。运行资产与生成器通过同一Dart library的私有接口协作，文件域单一写入。Source阶段只交已知源码，bridge二进制及与其绑定的安装清单/pins须由声明完整输出的Build阶段产生；Package只消费同批受检输出，不回写冻结Source。实际候选、完整生成链和Lab验证仍须完成后才能收包。

工程线分为app/contracts/CLI及有限模板、独立Host捕获/owner/提交/安装两段；主协调单写schema、摘要域、共享导出和catalog。已将批准的两个schema原字节注册为ProjectBuild/ProjectBuildResult及三个独立摘要域，18个结构向量与10个已知摘要接入Rust/Python真实解码检查；protocol 29项及Python contract 13项检查通过。首轮fixture索引由主协调误标schema错误为FMT001，按原解码器定义修正为FMT004，失败索引保存；批准schema与正反预期未改。上述属于格式层证据，不能代替真实重建、并发/取消、完整结果发布或IDE验收。

## 当前实现与受检接线

Dart已从公开C声明读取走通真实reader/generator两个AuthorSession及Source提交。固定ffigen23.0.0的AOT默认格式化入口会在查找Dart路径时停留根目录，首失败保持；改用该正式版本公开Output(format:false)后继续执行实际Dart分析和编译，不放宽超时。随后缺失文件来源和错误checked-helper字段均在真实门禁被拒；修复源模板后创建新的运行记录，没有回写失败结果。

既有profile/1的构建及打包入口保持原检查；新增显式profile/2适配先检查真实组合输入、合同、策略和固定责任，再复用原BuildTask、完成策略、父Source/Build、Package端口及字节捕获。固定包映射在执行前同时检查manifest与资产路径；33项构建/打包及旧适配定向向量通过。根新增GraphSequence只在同一既有GraphGuard间隔中串联后绑定的静态图，使真实编译产物字节确定后再构造link/run输入；失败、取消与panic粘滞，累计64节点/256MiB预算及原提交门保留。39项图测试（含6项新增）及完整app149项通过，并发语义独审无确定阻断；这些是受控模型/纯适配检查，不代替真实进程候选。

SDK的bootstrap失败输出零值防线已经补齐并独审。两个仅有限值模块使用的UTF8纯辅助函数从资源运行资产拆出，函数体逐字移动；资源生成不包含未用函数，不关闭分析警告。SDK分阶段原始检查及独审保留在本批e04-sdk-runtime-001/002/003，真实正常路径与注入状态模型分别标记。

独立Lab按公开候选编写五个资源案例，JIT/AOT共16次实际调用是有限首片要求。当前模型测试及schema检查不能替代这些实际调用。首轮独审指出packageUri实际导入目录、FileRef.schema及超时后无界排水三处缺口，修正与复核进行中；未登记SDK已通过。

工程新模板和CLI已接入现有C04执行及独立工程结果CAS。app/contracts最终独审闭合结果容量与接受后事实保存；Host16项定向测试及独审闭合工具安装接受后IO失败仍保留installed事实。根CLI初读发现初始化跨install/build取消间隙，修订为保留唯一外层listener和粘滞请求，每步仍独立封口；28项CLI测试与取消delta独审通过。非Unix仅补固定unavailable类型入口，不能外推新宿主支持。

为真实运行而另存的首个clean本地源码候选为`0007f8a55510037d055f59411ba9e879b421a02c`，路径`/private/tmp/madb-chief-round12/engineering-candidate-001/repo`。完整1548源文件包含当时全部未提交输入，复制前后及候选逐字/模式核对，未从HEAD遗漏首片；该候选只用于工程运行，不是实际仓集成、整包验收或发布。其后Dart新manifest登记不反向纳入这个已冻结来源。实际产品根保持原HEAD与字节，Lab已交付L05保持不变，工程真实重建及Dart正式包消费继续验证。

## 首片代码冻结前的补充复核

独立Lab上述三项P2均已修复，41项定向检查与完整仓库723项检查通过，最终独审SHA为`b1cef473369d8fea817fd6650060bd15c6341ef60470ade8d6edaf9cb3a5ee4b`；clean本地源码冻结为`189e1517feeb1be811bb030aaadb1dedc48a4c63`。此时尚未集成实际Lab根，亦未运行正式Dart候选。

Dart开发链008按最终源码冻结002完成真实Source/Build/Package、JIT/AOT正常资源调用和五项工具/SDK拒绝门禁；其126文件候选仍明确provisional。最终装配链独审SHA为`8632cbf16b8ba2dfeb0a44b222146d034d723a1764fc1de41544aba5c027da0d`。五项门禁核实际工具字节、profile/lock元组和SDK与Clang参数关系，不以辅助器自报代替Host最终复核；旧失败和旧候选不改结论。下一步从完整clean源码构建正式工具并重放所需检查，再由独立Lab消费。

工程clean候选0007的真实冷init与无变化Wrapper重建已通过；Rust实现变化触发旧consumer拒绝，更新独立预期后实际输出8:x。兼容新增普通入口twice由实际C调用返回12，合同、头文件和原生导出均有对应证据；真实双CLI竞争只有一个发布，另一个准确拒绝并保留attempt。缺失工具同字节恢复、错误工具拒绝覆盖及实际活动子进程SIGINT已有各自原件。字段改名使旧C在consumer-link准确失败并保全旧current，源码捕获后漂移和间接头变化仍在执行；以上不等于I链、P09或IDE整包验收。

完整产品门禁002在旧有Unix socket测试被沙箱拒绝创建socket而失败，原件保留；003在允许该本地测试能力的环境执行同一门禁。没有删除测试、放宽断言或将环境失败记为通过。
