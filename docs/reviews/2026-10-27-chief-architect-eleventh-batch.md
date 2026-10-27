# 第十一批：扩展收口、doctor预检与原生SwiftUI首片

承接[第十批](2026-10-26-chief-architect-tenth-batch.md)。本记录只登记本批范围、实现与实际证据；任务状态仅在 catalog 维护。

## 输入及隔离

实际 MadB 为 `9a72a6417858b7ee1a141ce8b13fa47ba7ef2f94`，独立 Lab 为 `d270293f50c04ebdca31a4a1eaa7335b1e869cd1`，两仓均无未提交文件。完整核对1365/531项源码字节与模式，旧C07 detached工作树保留。复用 `/private/tmp/madb-chief-round10/repo` 干净隔离检出，在 `codex/round11-extensions` 分支开发；新证据写 `/private/tmp/madb-chief-round11`，不覆盖上批工具、候选和归档。实际产品根只由主协调在重核基线后集成。

## 本批范围与责任

- E03：按当前四条验收，对账模型版本、旧读取器拒绝、证据失效和按符号/阶段/实例的责任与能力前提。实施先复用现有模型、纯规则和已批准composition/1、profile/2；不以有限Source协议证据扩为全部SDK能力。
- E01：并行核对真实包注册、字节来源、依赖闭包/作用域、权限/锁和有限组合消费；具体缺口在实现前确定，独审不代替实现。
- I01/I02/I03：准备doctor具体输出契约及既有工程Wrapper/增量的最小可执行子范围。已批准责任不重问；新增公开字段、行为或兼容承诺先按既有决定流程处理。
- 主协调独占共享格式、导出、catalog、规范及集成；子任务修改边界另在本记录具体追加。原生与并发变更须独审。Swift/ArkTS沿现有L04/L05队列推进，设备和编译资源由主协调安排。

依据为各任务catalog的documents、ADR-0018/0019/0023/0024及共同工程规范。保留C06/C07/L01/L02/E02既有冻结范围，不改历史报告；不新增工作包或降低整包退出条件。

## 检查条件

先运行影响模块的真实正反例，再审查最终实现并冻结候选；涉及组合生成的改动须复验实际输入、生成、旧批次保持和独立消费。集成后重验受影响路径，缺环境或尚未执行明确记录。状态仅在全部适用验收齐备后更新catalog，HTML由源生成。阶段推送使用skip-ci，不创建PR/标签、发布或启动Actions。

## 已定位的准入差异与实施边界

独审使用第十批原冻结工具及新隔离输入，实际复现四项漏拒绝：普通配置清空依赖、增强配置将运行依赖改成验证用途、清单新增无映射toolchain依赖、清单新增无映射generator依赖，均进入双会话并提交Source。原工具、历史输入和报告保持原样。新增回归测试在修复前因“意外接受输入”真实失败，退出101。

主协调负责composition/pins、ownership与内部RecipeRequirements依赖接线：依赖用途由固定宿主配方给出，结构合法后仍须核对完整集合；复用composition/1已有六种scope，不新增wire。当前没有受信任映射的外部toolchain/generator依赖明确拒绝，锁里出现同名工具不能代替映射。该正确性修复会拒绝此前错误接受的缺依赖输入，历史候选身份及结论不重写。

E03实施者负责composition/semantics、新语义/演进检查和独立source输入组装；原CLI example仅接入推导调用。原@1配方保留其5/8项局部观察责任，新本地`source-semantics-{mode}@2`配方绑定16个实际调用及各自SDK机制来源。它复用已批准composition/1与profile/2，只覆盖Source配方声明的语义阶段；没有物理库装配执行者时不虚授该owner，不认证Native运行、HAR或页面。额外要求或责任必须准确拒绝。主协调负责配方选择和生成入口映射，测试生成输入与生产推导分别实现。

## 已批准doctor实现范围

项目所有者采用[ADR-0025](../adr/0025-doctor-preflight-contract.md)及其精确提案，进入I02现有冻结上下文的只读doctor实现。主协调负责schema、格式向量、决定和catalog；I02实施者负责app只读编排、contracts受检投影、Host部分事实观察及CLI组合，独占相关导出。现有执行安装凭据仍要求完整验证，部分观察不转换为执行许可。新增输出预算码DOCTOR001随现有explain目录维护。

Swift另在独立Lab的`consumers/swift/l04_native_ui`、专用harness及检查脚本推进macOS真实SKIE/SwiftUI页面。公开C06候选和独立Observation源导出逐项锁定；不修改C06案例目录或历史报告。新页面运行、生命周期独审与冻结Lab复验完成前，不登记运行通过。该子范围不替代iOS/UIKit、其他配置或L04整包验收。

## E01/E03冻结候选与验收

本批以14文件选择提交`fe75241697469292d7af06ff6bb4e1ce47cc4018`单独冻结E01/E03；并行doctor内容完整保留在开发检出，不混入该候选。正式检出`/private/tmp/madb-chief-round11/extensions-source`保持干净，1371项tracked字节/模式与构建前一致。13工具镜像收据SHA-256为`e3da7d727d7b3bcde226c5b52eee9c96212af96d110b294980de6760657605c1`。

新三配置完成真实读取/生成双会话、相同冻结IR传递、一次Source提交和独立公开消费。最终31项分为3个真实正例、25个相干重绑metadata/语义拒绝、1个重复责任key摘要前拒绝、2个真实reader拒绝；后两项明确未启动generator或提交Source。旧双会话三配置和保留单generator的9项矩阵分别复验，未把旧5/8项观察扩大为新责任保证。E01四项历史漏拒、独立两项同名工具依赖和七项共同准入反例均准确拒绝。

完整本地门禁002实际退出0，原001因沙箱禁止`UnixListener::bind`创建临时Unix socket失败；原日志保留，提权后按原断言完整复跑。没有删除检查或将远程Actions写成通过。新模块只读取并校验现有生命周期责任，没有修改SDK原生协议。

[最终独审](eleventh-batch-extension-final-independent-review.md)逐条确认E01六项、E03四项现行验收齐备。当前结论已按固定候选及有限范围登记catalog；[精确证据索引](eleventh-batch-extensions-validation.json)与[作者验证](eleventh-batch-e03-final-validation-001.md)保留分层结果。Source机制与消费不授予SDK/HAR、真实目标/页面或实验后端认证，后续L/I/C/X及E04承诺不因此省略，也不反向追加为本两包前置。实际产品根集成及本批其他改动后的复验继续单独记录。

## doctor正式首片与集成门禁

干净`e26a73b6bf8cbd9c6ffded3240ebb6f9e49a09c4`包含doctor实现、已批准格式及扩展变更；13工具收据SHA-256为`f312f465260147733d6cf3f802d72819f14b653bb2b60b817fa81c67e8a26977`。正式main的真实镜像摘要、32项实施清单与实际输入绑定见[doctor证据索引](eleventh-batch-doctor-validation.json)。

独立回放按15+2项有效原始证据通过，structure/build/run分别核对6/23/24个静态前提，始终为`preflight-only`。空/污染PATH、严格错误JSON、根/路径边界、缺snapshot的MetadataRequired和blocked关系，以及DOCTOR001/旧SEM007解释均实际核对。原复制夹具漏schema、错误选择不在原说明目录中的META001等审查脚本失败原样保留，不修改产品扩大码表或重写原退出1。输出预算/冲突、真实安装缺项/别名/同字节替换、只读能力边界与原query/init回归分别由定向检查和[最终独审](eleventh-batch-doctor-final-independent-review.md)覆盖，不能把观察树守恒说成OS全局trace。

集成候选完整本地门禁`combined-full-gate-001`实际退出0，1401项tracked字节/模式与构建前一致。核对两个实际仓库完整基线后，MadB已从`9a72a641`快进到该候选；独立Lab及旧detached工作树保持原状，实际产品根与受检开发检出全量一致。最初沙箱内fetch因`.git/FETCH_HEAD`写权限失败，获准的相同基线检查与快进随后成功；未删除或覆盖任何其他内容。

doctor首片不替代模板选择模式、Wrapper、已有工程接入/升级/禁用或I02完整前置，I02状态保持原未验收边界。实际产品根上的三档只读预检、E01漏拒和E03新旧配置复验另记，不以克隆中的通过替代集成复核。阶段推送与集成后结果见下节。

## 实际产品根复验及阶段推送

实际产品根`e26a73b`与冻结13镜像完成E03新配置31项及旧`@1`三配置复验；[E01四项历史漏拒](eleventh-batch-post-integration-e01-checks.json)全部在会话/Source分配前拒绝。1401项源码及13镜像前后身份一致，[集成后验证](eleventh-batch-post-integration-e03-validation-001.md)与[独立身份复核](eleventh-batch-e03-post-integration-guard-review.md)无阻断。旧配置首次辅助脚本路径拼写错误发生在peer运行前，失败保留，修正后的002独立记录通过。

[实际产品根的doctor](eleventh-batch-post-integration-doctor-result.json)在原冻结工程上下文及空PATH下完成structure/build/run三档只读预检，分别为6/23/24项；实际JSON由已批准schema复核，工程树、主镜像和产品源码均未改变。该证据仍只证明静态执行前提。

[阶段推送记录](eleventh-batch-stage-push.json)中的两个新增代码提交均含`[skip ci]`，MadB分支`codex/c07-arkts`已阶段推送至`e26a73b`，远端身份核对一致。首次HTTP/2查询在push前失败；随后仅本次命令使用HTTP/1.1完成推送，未改TLS或全局配置。对两提交查询到的Actions运行数均为0，含义为未运行，不是远程验证通过；没有PR、标签、发布或dispatch。本批最终文档及独立Lab交付另记。

## L04：真实C06原生页面接线

独立Lab消费原C06 `swift-kotlin`公开候选（生产者`b19d48f`，候选摘要`872d0cbd49bee21d442f24be6536334a7cc8dff3a525839f2a889a7776ac908c`）和10文件公开Observation源导出。应用以一个owner供普通订阅与SwiftUI页面共享，真实onAppear/onDisappear负责页面启动及停止；XCTest通过可访问性按钮驱动三轮进出，独立字面值核对ASCII、Unicode、NUL、空值及字节边界，检查32帧、模型释放和最终native关闭。该macOS接线不替代第六批iOS SwiftUI/UIKit源层验证，也不授予iOS原生或其他配置认证。

原GUI001/002/003均构建通过、目标超时失败，没有业务process receipt或32帧；GUI003实测runner处于`started suspended / wait for debugger`。owned控制进程被恢复不等于业务正常退出/native close；原报告与[GUI003独审](eleventh-batch-l04-native-gui003-review.md)保持失败。没有把该现象认定为唯一根因，也未手工resume或修改系统安全设置。

本批修正测试应用的显式AppKit入口、owned控制进程恢复、当前Xcode默认macOS启动器及正常本地ad-hoc签名，并分别完成[原生独审](eleventh-batch-l04-native-independent-review.md)、[入口及清理差异复核](eleventh-batch-l04-native-delta-review.md)、[启动器复核](eleventh-batch-l04-native-scheme-review.md)与[签名复核](eleventh-batch-l04-native-signing-review.md)。最终签名修正只影响Lab装配及报告，不修改SDK/native协议或32帧、240秒退出条件。实际App/Runner的get-task-allow来自固定Xcode模板，三产物strict签名验证通过；本地ad-hoc不代表发行签名。

干净Lab `153edd12d76bf416e075a0430f18a2eaeabaa92d`完整仓库门禁005退出0；目标运行与独审结果继续单列，仓库门禁不授予SDK或页面通过。

认证前最后一次[GUI004原报告](eleventh-batch-l04-native-gui004-report.json)仍为目标失败：构建和本地签名通过，runner已加载XCTest并连接IDE/daemon，随后在启用UI自动化时等待超时，xcodebuild退出65。testmanagerd明确记录启用automation mode需要认证，并发起`Enable UI Automation`本机认证请求；没有成功返回、业务receipt或32帧。尚未实际完成原生页面运行与关闭，不把独立控制进程退出作为业务清理证明。

独立Lab实际根经完整基线核对从`d270293f`快进到`153edd12`，与受审开发检出的540项tracked字节/模式相同；产品根与旧工作树均保留。实际Lab根仓库门禁再次退出0；已知需人工本机认证，集成后没有重复目标失败运行，也没有把仓库门禁当成目标验收。[L04证据索引](eleventh-batch-l04-native-validation.json)区分每个失败候选与当前实现，L04目录继续保留未验收。

[GUI004最终独审](eleventh-batch-l04-native-gui004-review.md)核对原报告、27个命令、78项Lab输入、87项消费输入及实际认证日志；[认证摘录](eleventh-batch-l04-automation-authentication.json)绑定原始日志。没有观察到可见弹窗或用户拒绝，不把DeveloperTool preflight或某项TCC设置单独当成唯一原因。下一次目标验证须用户在场亲自处理本机认证，再以新报告满足原32帧、正常退出和独审；认证本身不等于验收通过。[Lab阶段记录](eleventh-batch-l04-native-stage.md)保留本轮实现与全部失败边界。

## 后续收口条件

本批关闭E01/E03，E02既有结论保留；E04的C05/E01/E02/E03前置已齐，可按现有公开端口启动有限同步值、错误和显式资源关闭子范围，仍须真实目标和独立包消费才能收包。工程线继续I01/I02的正常构建Wrapper、已有工程受检差异/恢复及模板选择doctor模式，接口就绪部分与I03首链IDE接线并行；I01/I02当前首片不解锁整包验收。Swift认证及本次macOS原生页面证据现已补齐，继续其余普通/便利配置和目标出口；L03、L05其余承诺、C08/L06及C09/C10联合矩阵仍按catalog精确依赖推进，不复用旧候选扩大结论。

## 持久化与复现边界

上述已完成证据已复制到`/Users/changyue/Desktop/MadB/.local/evidence/round11-extensions-doctor-swift/validation`，60939项、9120549721字节文件数据，经源前后守恒及目标逐项字节/模式核对。manifest SHA-256为`aa1569be29189e809d6746a365b4a2df6b7ba63f446d846f247b58485914c6b1`。保留正式13工具、源码tar、公开输入、实际编译产物、xcresult及所有失败；仅排除明列构建缓存，原证据仍保留。三个机器索引已逐项对账manifest并指向持久目录，历史报告内原始路径不回写；这不等于公开分发或无路径依赖的异机重放。

用户随后确认现在可亲自处理本机认证。新的GUI005从已集成Lab实际根运行，输出另存`/private/tmp/madb-chief-round11-ui-auth/gui005`，不修改本节已冻结归档或前四次报告；结果在后续段单列。

## 用户认证后的集成目标复验

用户确认在场后，从实际Lab根运行GUI005，真实通过自动化初始化并启动原生App。首个文本检查失败暴露macOS静态文本使用AX value而非iOS式label；回执中`/tmp`与`/private/tmp`的同一真实可执行文件也被旧字符串比较误拒。原GUI005报告保持失败，两帧仅证明初始化和普通订阅启动，不补造页面/关闭证据。[三文件独审](eleventh-batch-l04-gui005-fix-review.md)确认修复只改变准确的属性读取和绝对路径strict resolve同一目标比较，保留原值、UUID、nonce/PID、错误路径、32帧和240秒检查。

修复冻结为Lab `7a252092a0b50850c961f7d617facce20fbc57a2`，核对实际根153edd12的540项基线后快进集成；新实际根完整仓库门禁退出0，540项源码与集成清单一致。GUI006直接在此干净实际根重新构建和运行：XCTest **1通过、0失败、0跳过**，原始32帧齐备，三个不同页面实例退出后各自排水/释放，六次普通订阅发布持续工作，native owner正常关闭、应用终止回调及正常进程退出均确认，forced=false，runner无遗留。原GUI001–005失败及先前归档不变。

该结果只授予固定C06候选与公开Observation源补充的macOS Arm64原生SwiftUI切片通过，仍无iOS/UIKit原生、真实最低系统/物理设备、故障/迟到回调注入、后台/旋转或同时页面证据，也不是新的C06候选认证或C10发行包。L04整包及其他便利配置继续按catalog验收。

[GUI006最终独审](eleventh-batch-l04-gui006-independent-review.md)逐项复核29个命令、78项Lab输入、87项消费输入、58项候选及10项公开源文件、工具/签名输入，并交叉核对32帧实际值、三次页面排水/释放、任务join源码及正常退出；未发现阻断。首片通过由真实目标、独立值预期和关闭证据共同支持，不单凭报告summary。实际页面及最终关闭截图已导出并查阅，仅保留本地证据，不随源码推送。

[用户认证后的最终结果](eleventh-batch-l04-native-final-result.md)与独审分别保留；它们补充先前GUI004阶段记录，不重写任何失败报告。

用户认证后的补充证据已另存持久目录`/Users/changyue/Desktop/MadB/.local/evidence/round11-extensions-doctor-swift/ui-auth`：4045项、672910034字节文件数据，逐项字节/模式核对及源前后守恒通过；补充manifest SHA-256为`79105e88c57e7f691577b627b184d3819939b988c4d3898ed2bcc35811923305`。L04索引已核对并指向此目录，原validation归档不变。最终文档门禁、两仓精确远端HEAD与逐提交Actions查询由主协调保存于同一证据父目录的`delivery`阶段；Actions未运行与本地通过分列，不创建PR、标签、发布或dispatch。
