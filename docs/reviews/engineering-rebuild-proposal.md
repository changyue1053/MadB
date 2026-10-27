# Rust→C 值工程的可重复构建：最小决定稿

本稿是待独审、待所有者决定的具体差异，不是已批准能力。依据当前 `2fac2f7437e7bc235392fcca8c5d7af86b510e22`、ADR-0019/0023/0024/0025、I01/I02/I03 acceptance 与 `architecture/14-project-initialization.md`。不修改旧模板0.1.0/0.1.1、旧候选、原 init/doctor 的公开格式或 C06/C07 结论。

## 1. 实际差异与推荐范围

0.1.1 的 CMake 只导入已经冻结的 SDK；其 `init_values` 只准在全新 native 根准备一次，后续作者字节变化明确拒绝。`--open` 的拒绝完全符合 ADR-0023 所绑定提案。需要补的是日常作者构建入口，不是给原 init 再增加一次可覆盖执行。

推荐增加一个有限的 **新建 Rust/C 值工程、macOS/aarch64、Debug** 配方，名称 `madb.project.rust-c-values@1`，由模板 `org.madb.template/rust-c-sdk@0.2.0` 显式引用。新模板最初只能显式选择，默认目录仍指向0.1.1，待相同版本完整 INIT/IDE 验收后另行调整推荐目录。0.2.0 不赋予任意 Rust crate、Cargo 依赖、其他平台、C05资源/异步或任意已有 CMake 工程支持。

这一决定只覆盖：固定输入捕获 → 既有 C04 读取/生成/构建/包装 → 普通 C 消费者真实编译链接 → 新完整工程批次发布 → 失败/取消保留旧有效批次。`--open`、真实 IDE 导入/索引/运行/调试仍不由本稿宣称完成。IDE 接入的共同入口在此固定，后续具体 IDE 只接该入口，不复制生成逻辑。

新模板的init仍使用现有InitResult：dry-run只产生计划与独立证据，不复制工具或启动基础工具；structure在最终根创建源码、静态launcher、有限动态描述和来源标记，并复制受检工具到本机状态；build复用下述唯一project build编排，不能先走旧0.1.1初始化执行器再重复构建；run在同一固定完整结果上执行新建示例的既有7:x期望并分别记实际运行结果。运行/证据尾部失败仍保留真实创建和已经发生的发布事实。新模板结构模式也要求能捕获当前源码绑定工具身份和适用macOS基础工具，但不要求只供native build使用的SDK；这项比旧0.1.1更窄的宿主前提由新模板版本承担。

已有任意工程的 attach/upgrade/disable、可选组件变更、通用多文件 patch/恢复 wire 另行定稿；不为本有限构建引入插件或迁移框架，不把整个 I01/I02/I03 宣告解锁。

## 2. 唯一入口及 CLI 语义

新增 `madb project build --root PATH --frozen [--json]`。`--root` 必填且指向已经初始化的该有限配方工程；`--frozen` 必填。拒绝其他 task、target、profile、临时工具路径和隐式升级参数；无网络和 PATH fallback。此入口不是已有 C04 `madb build --root ... --execution ...` 的别名，也不改变其参数或执行身份。内部直接调用同一个 C04 coordinator，禁止启动第二个 MadB 顶层进程。

项目 Wrapper 的公开调用为 `./madbw build --frozen [--json]`，把项目根与上述请求传给受检工具。只支持 `build`；未知命令/额外参数在启动 MadB 前拒绝。Wrapper 不是新的 DAG 或生成器。GUI 的普通构建动作和 CLI 必须调用同一 Wrapper/同一项目描述；不能把直接消费旧 candidate 的 CMake 当成作者构建。

首版采取保守全量执行：每次捕获当前作者输入并生成新尝试，不承诺无变化命中缓存。无变化重建也允许重跑，必须记录原因 `conservative-full-build`，不虚填缓存命中。将来的缓存优化须基于 P09 事实；普通修改源码不更新版本/来源锁。

`--frozen` 的限定非常具体：共享描述里的工具、SDK、profile、模板和配方身份不变；每次尝试中 `read.json` 与 C04 `madb.lock/1` 的作者摘要按本次捕获生成在新的受管尝试目录。既有共享锁、旧尝试锁及原0.1.1 native 根均不覆写。它不改变既有 C04 frozen 语义，也不是 `lock --update` 的替代入口。

退出复用已有类别：0全部请求阶段通过；1构建/链接或检查失败；2未知配方/坏输入/不受支持类型；3缺工具或宿主IO；4基准冲突；130取消。取消不掩盖更早已确定的非取消错误，外部清理未确认不得发布成功。

只有已识别build/install-tool子命令后，--json才选择本稿结果格式；未知/缺少project子命令、--help/--version及Wrapper准入之前的错误采用CLI的文本帮助/错误例外。识别子命令后的坏参数必须输出同一结果schema，operation取实际已识别值，不能为了填字段猜另一操作。

## 3. 唯一新增共享输入 `madb.project-build/1`

文件固定为 `.madb/project-build.json`。严格 JSON、未知字段拒绝、现有 LogicalPath/Id/Digest/整数限制；原始字节和语义身份均进入构建证据。不接受 shell 字符串、argv/env、自定义配方代码或下载 URL。

字段及首配方限制如下：

| 字段 | 精确定义 |
|---|---|
| `schema` | 常量 `madb.project-build/1` |
| `recipe` | 常量 `madb.project.rust-c-values@1` |
| `template` | `{id,version,archive}`；archive 为既有 `{path,sha256,size}`，路径固定 `.madb/template-package.tar`；实际归档逐字验证 |
| `namespace` | 沿用现模板`[A-Za-z][A-Za-z0-9_]{0,47}`，仍拒绝语言/生成器保留名；不可在普通构建中变更 |
| `profile` | 既有文档引用 `{path,schema,sha256,size}`；固定 `.madb/template-input/profile.json`，首版 `madb.profile/1`，沿用0.1.1有限能力与精确工具/SDK核对，不凭新模板号扩展能力 |
| `sdk` | 既有文档引用，固定 `.madb/template-input/toolchain.json`；必须与 profile 中声明一致 |
| `tool` | `{id,version,sha256,size,source_revision}`；id固定 `org.madb/cli`、version真实稳定内部版本，源码40位小写提交，必须对应实际运行镜像和干净构建收据；无伪版本截断 |
| `author` | `{language:"rust",entry:"rust/src/lib.rs"}`；仅单一自然 Rust 值声明；其字节不放进共享版本锁，每次实际捕获 |
| `consumer` | `{language:"c",entry:"consumer/main.c",headers:"consumer/include"}`；entry固定、headers为唯一可选用户头目录；目录不存在等价空集合，存在条件本身入输入身份 |
| `configuration` | 常量 `debug` |

模板来源/工具来源/SDK/profile变化不由普通 build 接受：描述必须同时符合固定目录记录及本地已受检初始化来源。未经专用变更流程的局部 repin 不自动成为可信。构建时保存原始描述和原包字节为尝试输入，不修改这些共享来源文件。

## 4. Wrapper 与本机工具定位（需要本次明确批准）

采用**固定静态 `madbw` POSIX launcher + 本机受检工具镜像**，不增加通用脚本 renderer。模板原包保存 launcher 原字节；固定配方将这个唯一文件以0755写入新工程，其余模板文件仍原规则。launcher 与 `.madb/project-build.json` 共同形成受检初始文件计划，不在 CommitNew 后偷偷改写用户源码。

MadB 镜像安装在项目固定受管本机目录 `.madb/wrapper/madb`，不提交版本控制；init 从已核对的当前运行镜像复制到新文件，源/目标字节同时核对 `tool`，不下载。描述中的身份属于共享版本信息；镜像路径属于本机状态，两者不合并为可任意指定路径字段。复制只发生在 `.madb/wrapper` 的受管新文件内，不覆写已有镜像或跟随链接。

launcher 原字节不嵌入 MadB 镜像摘要，避免“工具嵌入模板、模板再嵌工具摘要”的循环。初始化时已捕获的实际镜像身份写入**独立的** `.madb/project-build.json.tool`；这份动态值由固定recipe生成并进入完整初始文件计划，其输出路径/字段受上述有限schema约束，不增加通用shell renderer。

启动前固定步骤为：shell builtin解析显式脚本路径并拒绝未知参数；拒绝描述、来源标记、`.madb`、`.madb/wrapper`、镜像的符号链接；先从origin的descriptor.sha256提取初始描述摘要并对实际描述字节核对，再用`/usr/bin/env -i LC_ALL=C /usr/bin/plutil -extract tool.sha256 raw -expect string -o - -- <descriptor>`读取预期镜像值；shell限定`sha256:`加64个小写hex；`/usr/bin/env -i LC_ALL=C OPENSSL_CONF=/dev/null /usr/bin/openssl dgst -sha256 -r <image>`读取实际摘要；仅严格相等才以干净环境和固定argv exec镜像。原始descriptor与镜像身份均校验，局部repin不能越过startup。完整严格JSON、重复键、来源/工具身份仍由已核对MadB再次验证；plutil字段提取不当成完整schema验证。无eval、无交互式PATH、无预期SHA shell插值。`HOME/TMPDIR`仅作为明确本机路径输入传给MadB，不能进入子工具任意白名单环境。

已在实际macOS26.6.2/25G83、系统LibreSSL3.3.6上做四项scratch启动观察，精确`/bin/sh`、`env`、`plutil`、`openssl`镜像摘要/字节数见 `engineering/launcher-probe-002/observation.json`；候选配方仅采用该实际宿主观察，在preflight核对这些基础工具和系统构建。基础信任为当前受信macOS系统卷；这些检查是固定身份及漂移检测，不是以OpenSSL自校验构造安全信任根或防同权限恶意替换沙箱。未知系统构建或镜像明确缺项，不从命令名字推导支持。`shasum` 未采用，因为当前Perl默认包含额外本机模块目录，不能把仅脚本digest视作完整基础工具身份。

真实CLI尚未实现；原型正例仅以已固定假工具证明argv与PATH/环境规则，反例证明错字节不执行，不能作为工程构建/IDE验收。原始工具帮助来自本机正式系统；LibreSSL参考仅限[3.3.6正式标签](https://github.com/libressl/portable/releases/tag/v3.3.6)，不把最新manual的额外能力带入配方。Wrapper在工具准入前的错误为stderr固定文本与2/3/4退出，即使传入--json亦如此；机器结果只由已准入MadB产生，这个明确例外须同本稿决定。

新增配套 `madb project install-tool --root PATH --frozen [--json]` 仅作**已有该配方工程的受检本机工具恢复**：当前执行工具真实字节/版本/源码必须精确等于描述，镜像缺失时无覆盖复制；已存在匹配镜像只复核；已存在错误字节拒绝冲突，要求用户保留/移走错误文件后重试。它不重建、不更改来源锁或用户文件、不安装第三方工具，不从网络寻找版本。此命令解决项目复制到另一目录/机器后的工具恢复；不能让0.1.1获得新配方。

Wrapper `build` 只运行正式 build，不自动修复或覆盖工具。离线工具完整可运行，缺失清楚给出上面的准确恢复命令及所需版本/摘要；无全局安装。此处新增两个 CLI 入口及一个固定执行权限规则是实质变更，必须决定；不是已采用 init 通用字段的隐含授权。

## 5. 输入快照与单次执行

每次在工程根保持目录身份，捕获描述、原包、profile、SDK、author、consumer main 与 `consumer/include` 的文件集合/原始字节。有限预算复用现有输入预算；拒绝符号链接、路径穿越、大小写冲突、超限、未知 Rust 源文件/外部模块。Rust 源目录只能有 `lib.rs`；新增其他 `.rs` 明确不受支持，不能静默忽略。C 头目录递归记录所有普通文件与不存在条件，依赖发现核对实际包含头，未声明的外部头拒绝；系统头仅来自已锁SDK/Clang资源。宏和编译参数完全由固定配方及当前profile决定。

具体捕获上限：4096个原始文件、8192个目录、单源文件32MiB、总源字节128MiB；目录成员集合最多16384条，嵌套深度64；本机MadB镜像沿现有LockedInitializationFile的256MiB独立上限，不算可编辑源文件。自生成runtime/compiler输出不偷占用户输入额度；其既有执行/输出预算继续单独生效。无法完整枚举或对账时拒绝，不退化为时间戳命中。

捕获后的源码复制到全新 `.madb/build/attempts/<opaque-id>/inputs`，本次读取/生成/构建/包装只读该固定快照；现场源再次复核通过后才可以发布。业务编译在该最终受管尝试路径完成，不先在临时绝对路径编译再搬动二进制。执行工具/SDK沿用现有完整 InstalledValuesTools 准入与执行前后核验，不能把 doctor 输出转成执行许可。

app只持有状态、约束与 `ProjectBuildExecution` 等窄端口，不依赖CLI/Host或解析命令行。**CLI组合根**向app注入具体执行器，该执行器复用当前 `bootstrap/execution.rs::execute_controlled` 和现有有限preparation；必要时保行为抽取CLI内部共用编排。不是app反向调用CLI coordinator，也不新增通用DAG/插件。旧 init 仍保留一次创建/原始输入冻结。新增构建不调用 init，不把新作者覆盖写入旧 `.madb/build/native/author.rs`。SDK的 Source/Build/Candidate 保持既有三层身份和事务。普通 C 消费者链接本次唯一候选中的头和库，不分别跟随 latest；采用与该候选同尝试内的 CMake/Ninja 工程和输出目录。

初版 native consumer CMake 文件由固定 recipe 提供，只编译快照 consumer/main.c，不调用 Wrapper 或 MadB。因此 `madbw → app → C04 → native CMake` 没有回边。后续 IDE/native外层任务仅把 `madbw build --frozen` 作为该工程作者构建入口，不能把本次内部 consumer CMake 注册为再次调用 MadB 的目标。

## 6. 完整发布、并发与恢复

每个尝试有独立目录、独立 C04 domain 与独立 CMake 输出，两个同时构建不共享可写 compiler 输出。捕获开工时的当前工程批次引用；完成后必须重新核对：用户输入身份、工具/SDK、该次 SDK Candidate 完整性、consumer镜像、日志、进程清理。再以短提交临界区比较开工基准并发布唯一完整工程结果。另一次构建已发布时本次为 Conflict；保留它的自有尝试证据，不悄悄覆盖赢家。不得在外部进程运行期间持有全局提交锁。

工程发布域固定为 `.madb/project-state`，owner类别只准本配方；不是第四个C04层，不交给现有通用`recover`。init首次创建来源标记，绑定初始描述字节、模板原包和受管目录；不可变来源标记随共享工程保存，属于版本来源记录，不是第二份可编辑配置。本地attempts/current可忽略，但不得把原始来源标记忽略。build和install-tool均检查标记及基准描述；标记缺失、陌生owner或同一描述被局部repin时明确拒绝，提示恢复原始版本来源文件，不自动认领/升级任意目录。

来源标记固定 `.madb/project-origin.json`、ownership=provenance，内容是Host私有版本1记录：`kind="madb-project-origin"`、`version=1`、`recipe`、`descriptor`（初始描述FileIdentity）、`template_archive`（原包FileIdentity）、`managed_roots=[".madb/wrapper",".madb/build/attempts",".madb/project-state"]`。它只承载不可变基准，不提供扩展字段或脚本；当前工具严格验证并拒绝未知版本。该内部文件不注册为公共交换schema，不接受第三方producer制造；实际检查是受信本机文件系统中的一致性/误改检测，不声称能阻挡同权限写者同时伪造描述和来源标记。新的构建不回写它；将来升级应先产生明确受检变更计划。

current索引是Host私有存储，只能引用本owner域内实际存在、已逐字核验的完整输出记录及唯一尝试根。不存在current是首次构建；损坏current、坏旧结果、悬空/越界/链接引用都不是首次构建，拒绝且保留原状态。索引记录包含generation和不可变结果身份；CAS比较原索引身份/代数，防止内容ABA被误认为无并发。公开返回值给真正固定的候选引用与消费者路径，不能只给current。包装器、后续IDE运行器固定一个完整结果再消费，不跨批次拼接。旧成功结果、失败/未知清理尝试与来源标记均不自动GC；本稿没有自动recover或热卸载接口。

CAS之前失败或取消保存本次结果/日志；只有本次自有、确认无活动进程的临时内容可清理。旧描述、来源锁、原包、手写文件、旧成功批次和current不变。CAS替换是取消封口点：替换已经成功后，取消是AlreadyFinished，不得再返回130并称未发布。替换成功、fsync或证据保存失败时保留`published=true`和完整结果身份，durability=uncertain，exit3；不能回滚current假装从未提交。若底层IO使替换是否发生不可判定，先只读复核索引和本次身份；仍无法判定则`published=null`/durability=uncertain，exit3，保留完整尝试与previous，给“检查current后重试”的恢复说明，禁止继续消费或自动覆盖。

下次 `madbw build --frozen` 是新尝试，修复用户源码后可重试；有损坏或不确定current须先按明确诊断核对恢复，不能当成干净重试。首轮没有旧成功批次时不得制造可运行current。SIGKILL/未知清理的尝试隔离保留，不凭PID不存在就删除外部进程可能使用的文件。此处先不提供跨尝试resume，更不反向删除用户改动。

## 7. 新增结果 `madb.project-build-result/1`

新增 JSON 只用于上述 `project build`；安装恢复入口复用同文档但 `operation="install-tool"`，无构建结果。所有字段必有且未知字段拒绝：

| 字段 | 定义 |
|---|---|
| `schema` | `madb.project-build-result/1` |
| `operation` | `build` 或 `install-tool` |
| `root` | 既有DisplayPath对象形状，本入口固定为本机绝对path及redacted=false；参数解析前可为null。本稿不新增脱敏导出命令或宣称此本机结果已脱敏 |
| `recipe` | 成功解析后固定recipe；解析前失败为null |
| `request_digest` | 对完整已解析请求 `operation + root + frozen + descriptor原字节引用` 按独立域 `madb.project-build-request/1` 求摘要；不完整为null，错误分类独立 |
| `tool` | 实际运行工具身份 `{id,version,sha256,size,source_revision}`；捕获失败为null，不填声明冒充观察 |
| `artifact_root` | 本次实际创建的受管尝试根DisplayPath，未创建为null；inputs、result中的所有文件引用均相对此根，不相对CLI cwd或工程根 |
| `inputs` | `{files:[{source,file}],directories:[{path,present,members:[{path,kind}]}]}`；source为工程相对路径；file为尝试根中原字节副本的既有FileIdentity(path/sha256/size)，路径必须精确等于`inputs/`+source；目录path为工程相对路径、成员path相对目录，kind只准file/directory；按path排序、不重复、无链接。不存在目录present=false且members=[]；现存空目录present=true且members=[]，二者不同。未捕获的文件不编造；两固定目录为rust/src及consumer/include，必须完整捕获或整体报错；完整集合至少含描述、来源标记、原包、profile、SDK、rust/src/lib.rs及consumer/main.c，并与目录枚举中所有普通文件一一对应 |
| `stages` | build固定九项顺序capture/preflight/prepare/metadata/generate/build/package/consumer-link/publish；install-tool固定三项capture/preflight/install。每项`{id,status,diagnostics,attachments,blocked_by}`，状态passed/failed/error/blocked/skipped/unsupported，blocked_by只准本次序列中更早阶段ID、去重排序；仅blocked时非空，其他状态必须空。请求还未准入时可整个数组为空，一旦开始capture即输出完整固定序列；未执行下游blocked，不能省略或填passed |
| `result` | 仅build且published=true时非null，包含publish阶段error但durability=uncertain的已发布尾窗：`{identity,snapshot_digest,candidate:{root,manifest},consumer,outputs}`。root为尝试相对候选根，manifest/consumer/outputs均FileIdentity；outputs完整覆盖该Candidate manifest、公开候选所有成员及consumer镜像且无额外文件。必须经既有公共Candidate验证、每个成员逐字核对、consumer真实成功链接、输入和工具重验，不能由结果JSON授予执行权限 |
| `installed_tool` | install阶段已完成无覆盖复制/匹配已有镜像且逐字验证时非null：`{path:DisplayPath,sha256,size}`；即使后续证据/结果保存失败返回非零，也必须保留该真实安装事实，不能把它抹为空。build时为null；安装未完成或字节不能确定时为null并保留明确恢复诊断。此对象不等于整个请求成功 |
| `previous` | 开工时实际固定的旧成功结果`{artifact_root:DisplayPath,identity:Digest}`，没有为null；Host按受检索引和旧完整文件核对，不从此观察对象获得执行凭据；不意味着当前已回滚或已执行旧产物 |
| `published` | true仅为本次已实际替换索引；false为明确未替换；null仅为复核后仍不能判定替换事实。install-tool永远false |
| `publication_durability` | not-applicable/confirmed/uncertain。published=false只能not-applicable；published=null只能uncertain；published=true可confirmed或uncertain，后者仍保留真实result且非零 |
| `termination` / `exit_code` | complete/failed/cancelled，与上列退出和状态一致；未知清理不能complete |
| `evidence_root` / `logs` | 实际已保存证据根与文件引用；失败也尽力保存，保存失败不能丢原错误或声称完整证据 |
| `diagnostics` / `recovery` | 复用既有诊断与明确本机恢复步骤形状，排序及错误优先级复用现有约定 |

精确摘要投影如下。统一使用现有`semantic(domain,value)`：`sha256:` + SHA-256(UTF-8(domain + LF + JCS(value)))；JCS对象键按UTF-16排序、数字仅安全整数、无额外LF。集合先按路径UTF-16字典序排序；重复/冲突键和重复路径在摘要前拒绝，不用排序掩盖冲突。

- 请求域`madb.project-build-request/1`，投影仅`{operation,root,frozen,descriptor}`；root是实际解析并固定的工程绝对路径字符串（不是DisplayPath对象），frozen恒true，descriptor是原始`.madb/project-build.json`的FileIdentity（工程相对path）。未完成该四项即request_digest=null；非零错误不决定摘要是否完整。
- 输入域`madb.project-build-input/1`，投影仅`{descriptor,files,directories}`；descriptor同上；files逐项为`{source,sha256,size}`，从完整inputs.files取source及file的原字节sha256/size，**不包含快照副本path**；directories逐项原样取`{path,present,members:[{path,kind}]}`并排序。descriptor必须和files中相同source项的字节身份一致。全部必需源及目录都完整准入后才能求此摘要。
- 输出域`madb.project-build-output/1`，投影仅`{recipe,tool,snapshot_digest,candidate,consumer,outputs}`，结构和公开result内相同，outputs按path排序；不包含identity自身、root/previous/published、耗时、日志和证据根。候选/consumer相对路径以及实际输出字节都参与；不承诺不同绝对编译根或不同物理Candidate实例有相同二进制或输出身份。

已知摘要向量包含规范投影原值、规范UTF-8文本、字节数和结果，位于`engineering/format/digest-vectors.json`；只改作者字节、空/缺头目录、工具身份及输出字节均应改变对应摘要，集合重排不应改变。它们是格式向量，不是伪造SDK执行证据。

`result`的完整输出记录在发布前可以受检准备，但只能在CAS发布成功后出现在公开结果中。Host current索引不指向含published=true的预写公开结果；它只指向已经逐字检查的不可变输出记录。公开结果在CAS后生成并保存，避免CAS失败留下谎称已发布的候选报告。发布后保存报告失败时必须保留published=true与已发生事实、返回错误并给恢复位置，不能倒称未发布。

保留现有统一decoder的**1MiB**上限，project-build输入与结果都不例外（结果计算JCS+末尾LF）。有限配方增加明确内部额度：完整inputs投影、完整outputs库存、全部日志/附件索引、全部阶段诊断/恢复文字分别最多128KiB；根/工具/状态等固定字段与发布尾部另有至少128KiB余量。达到源或目录元数据额度时在capture准入前报PLAN001/exit2，inputs为空且无snapshot_digest，部分原始观察只存实际附件；不能把部分列表伪称完整inputs。生成的outputs库存超限在publish前报EVD001/exit1，拒绝提交。长进程stdout/stderr存附件，协议诊断不内联这些任意字节。

完整结果的实际编码预算仍在CAS前最终检查，只有保留足够尾窗事实空间才进入提交；不放宽公共decoder、不裁去必需输入后声称成功。发布后的stdout/证据写失败按上一节保留真实提交事实；无法输出JSON本身是传输错误，不伪造可读成功。install-tool保存尾部失败同样保留installed_tool与install阶段passed（若复制已成功但目录持久性不确定，install阶段error且installed_tool仍指实际已存在核对字节），exit3并给准确恢复，不删除正确工具。结构schema不能代替来源、完整库存、阶段及事务语义核验；配套scratch schema与正反例只证明格式提案自洽，不是生产decoder或目标验收。

install-tool的取消也有唯一最终准入点：新镜像完成复制/字节复核后的no-replace原子发布，或已有匹配镜像最后复核的接受点。它与取消共用最终准入门；接受后晚到取消为AlreadyFinished，installed_tool非null不得返回130。之后的持久化/回执IO错误保留真实installed_tool与非零退出，不回滚、不称未安装；没有经过该接受点的失败/取消不得填installed_tool。

## 8. 实施文件域与检查条件

预计由工程线持有：新 `crates/madb-app/src/use_cases/project_build/`、新 `crates/madb-cli/src/project.rs` 与 `bootstrap/project/`、新 `adapters/host/src/project_build/`、`adapters/toolchains/rust_c/project_build_values.py`、新模板 `versions/0.2.0/` 与专用定向测试。必要保行为抽取限定 `bootstrap/init/recipe.rs` 与 `initialization_execution` 的小型共用编排；先列精确diff，再经独审，不改旧成功语义。root独占新schema/注册、共享导出、目录、ADR及集成；与E04/L05无共享写入。

独立检查至少覆盖：同一工程冷构建、无变化、仅实现+1输出变化、兼容新增声明接口身份变化；破坏声明导致consumer编译失败且旧current保持；输入集合/间接头变化；工具/SDK错身份、旧模板拒绝、未知命令/参数；空PATH、中文空格路径、离线完整与缺工具；并发两个构建CAS只一个发布；阶段间/活动进程取消；源码在捕获后与publish前变化；工具恢复不覆盖错字节；失败后修复重试；所有共享来源/用户文件/旧批次字节守恒。

P09统计必须分入口、完整捕获真实子进程启动次数、阶段耗时及保守失效原因；不从ps抽样推导完整启动数。不因四类CLI运行成功授予GUI导入、索引或调试；I03适用IDE检查和I06联合退出另验。原生进程/取消、事务并发和Wrapper启动须独立复核，最终代码集成后从干净实际提交重建工具候选再验证。
