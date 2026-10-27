# E02 第 6 条：ArkTS 替代配置的来源、责任与准入事实（修订 2）

本文件保存第十批实施期间的只读来源核对快照。下文相对取证路径属于该批归档目录；最终实际门禁与收口结论见[第十批记录](2026-10-26-chief-architect-tenth-batch.md)。本快照不随后续实现改写。

本修订只补 KNOI 官方说明的固定研究快照；原版 `e02-alternative-admission-facts.md` 及既有 receipt 原样保留。原版 SHA-256 `51a20595aec0f8f6dac9c994937ed7348ff70ce816fa4a2b99a5589fa7cb1592`。

只读核对；不改产品、模板、catalog 或历史结论，不构建/运行/认证 X 后端。依据当前检出 `3bb06519900a2ab19f0bdd0db11d817160c2482b` 上的实际共享工作树、ADR-0017、架构 16/17、当前公共 schema 与组合输入校验器。状态仍以 catalog 为唯一权威。

**结论：当前事实不支持仅凭旧 `unknown` 字符串拒绝，将 E02 第 6 条写成通过。也不应把完成 X03/X07/X08 加成 E02 的前置。** 本条需要普通/增强/禁用的实际推荐配置证据，并把下面三个具名替代选择的来源、依赖、语义责任和真实准入拒绝对应起来。本报告补事实声明，未运行生产入口，不能自称拒绝测试通过。

## 1 当前 MadB 状态和责任

当前 catalog 中 X03、X07、X08 都是 `not_started / missing`，`evidence=[]`；X07/X08 允许 S0 可行性子范围，正式验收依赖 C11。ADR-0017 明确它们不反向加入主线 L05/L07/C11 的依赖。已选扩展仍有交付责任；未准入不等于取消交付。

公开端口责任沿架构 17，不因研究对象另开插件平台：

- Reader：实际声明/后端元数据到 ContractSnapshot，附来源与支持子集。
- Generator：IR/profile/精确锁到绑定和独立 OutputSet；ABI、模型、异步等阶段有唯一责任方。
- Toolchain：冻结的工具、目标 SDK 和输入到 BuildEvidence；不把编译零退出变成语义/设备通过。
- Packager：按目标闭合公开源码、原生库和运行依赖，不授予提交/发布权。
- SDK：实际 VM/env、执行器、引用、取消、停止、排水和最终释放；E02 进程退出不证明这些已完成。
- Host/Verifier/Lab：校验源与执行镜像、会话及事务；独立设备/消费者验证另立证据。

这些是**需要实现的端口映射**，不是声称三个 X 已导出这些 MadB 组件。

## 2 X07：UniFFI-ArkTS

### 固定来源与上游事实

研究固定为 `ohos-rs/uniffi-bindgen-arkts` 提交 `56018734abafc1de7e9d124badb3a33d49243f59`。该提交 Cargo.toml 的生成器版本为 `0.0.1-beta.2`，依赖声明 UniFFI `0.31.0`；同提交 Cargo.lock 实际解析 UniFFI 系列为 `0.31.2`，napi-ohos/derive/build/sys 为 `1.2.0`。不能把版本范围字符串当精确锁，也不能借用 Gobley 或另一条 UniFFI 版本线的结果。[固定 Cargo.toml](https://github.com/ohos-rs/uniffi-bindgen-arkts/blob/56018734abafc1de7e9d124badb3a33d49243f59/Cargo.toml)、[固定 Cargo.lock](https://github.com/ohos-rs/uniffi-bindgen-arkts/blob/56018734abafc1de7e9d124badb3a33d49243f59/Cargo.lock)

上游输入是已构建的 UniFFI cdylib/元数据；输出包含 ArkTS 序列化/对象/async/回调辅助层和另一个 N-API Rust adapter。默认会装入原 cdylib；关闭 staging 时由外部构建提供该库及 link-search 路径。两份库与辅助层不能被当作单个直连 napi-ohos 配置。[固定说明](https://github.com/ohos-rs/uniffi-bindgen-arkts/blob/56018734abafc1de7e9d124badb3a33d49243f59/README.md)、[固定 adapter 依赖](https://github.com/ohos-rs/uniffi-bindgen-arkts/blob/56018734abafc1de7e9d124badb3a33d49243f59/tests/fixtures/generated/callbacks/native/Cargo.toml)

生成的同步 callback 仅在记录的原线程直接调用，异线程分支返回错误；不能把支持 async callback 推导为任意异线程同步调用。上游 smoke 在未提供设备目标时可只完成 HAP 构建；缺项目/工具时还可能跳过。[固定 callback 源码](https://github.com/ohos-rs/uniffi-bindgen-arkts/blob/56018734abafc1de7e9d124badb3a33d49243f59/tests/fixtures/generated/callbacks/native/src/lib.rs#L457)、[固定 smoke](https://github.com/ohos-rs/uniffi-bindgen-arkts/blob/56018734abafc1de7e9d124badb3a33d49243f59/scripts/ci/run_ohos_smoke.sh)

### 依赖与尚缺的 MadB 准入

宿主侧需要固定生成器及 UniFFI 读取/生成配套；作者侧需要匹配 scaffolding；目标侧需要原 cdylib、N-API adapter、ArkTS runtime、所选 ABI/SDK 与所有传递 Native 依赖。Reader 负责真实元数据；Generator 负责序列化/投影/async 适配；SDK/Packager 分别负责资源协议和双库装载闭包。

尚无该配置的 MadB 受检 profile、extension manifest、组件源集合、精确 lock、执行镜像与 Host recipe；没有声明能力到固定目标用例的映射。取消、对象 free、env 退出以及装载配套均不能从上游 fixture 或 C07 借证。**当前拒绝的准确理由是所选 UniFFI-ArkTS 配置没有已注册/已锁定的 MadB 适配与执行配方，而非“上游不支持 async”或“UniFFI 版本随便选”。**

## 3 X03：ANI / ani-rs

### 固定来源与上游事实

研究源码固定为 `ohos-rs/ani-rs` 提交 `4239549a3ef1701b27571611f939abe143de4074`，不是采用后来的 master。workspace 声明 Rust 最低 1.88，`ani 0.1.0-alpha.3`、`ani-sys 0.0.1-alpha.1`、`ani-derive 0.0.1-beta.1`；这些是该提交内容，本文不证明同名发布包等同于此提交。[固定 workspace](https://github.com/ohos-rs/ani-rs/blob/4239549a3ef1701b27571611f939abe143de4074/Cargo.toml)

该提交 `ani` 默认特性为 api26；`async-runtime` 是独立执行器接口，`async` 叠加默认 Tokio 后端。固定异步指南已经有可替换执行器和有界 async stream，不能沿旧 README 把它说成仅同步 iterator。[固定 runtime 配置](https://github.com/ohos-rs/ani-rs/blob/4239549a3ef1701b27571611f939abe143de4074/crates/ani/Cargo.toml)、[固定异步指南](https://github.com/ohos-rs/ani-rs/blob/4239549a3ef1701b27571611f939abe143de4074/website/src/content/docs/guide/async.md)

同一指南区分 Promise 等待注销与业务停止：Rust waiter cancel/Drop 不会终止 ArkTS 操作；backend 的 shutdown 要求线程、任务、闭包和 waker 全部停止执行 addon 代码。stream 有自己的队列、背压、错误和关闭语义。这些要求不能套成 Node-API cleanup hook 的证明。[固定异步与 shutdown 责任](https://github.com/ohos-rs/ani-rs/blob/4239549a3ef1701b27571611f939abe143de4074/website/src/content/docs/guide/async.md)

### 依赖与尚缺的 MadB 准入

必须明确产品 SDK、ArkTS 语言/VM 模式、ANI 头/运行时 ABI、ani-rs 精确源码/镜像、可选执行器及目标库。宏/Reader 提取声明；Generator 产生 ANI/ETS 接线；Toolchain 固定 SDK/头/工具；SDK 分配 attach/detach、引用/线程、Promise 等待、业务取消、stream 排水与 shutdown 责任。

当前没有锁定产品 SDK 和上述 MadB adapter/profile/manifest/lock/镜像/recipe，更无该后端目标运行证据。**准确拒绝是 ANI 运行时与目标 SDK/适配器准入缺失；不是声称固定 ani-rs 源码没有 stream，也不是把 napi-ohos 的 env/线程/关闭认证授予 ANI。** 源码里 api26 的默认选择不替项目裁定具体产品 SDK，也不自动新增 Kotlin/Native ANI 路线。

## 4 X08：KNOI

**MadB 所选 fork、版本和 commit 仍为 unresolved。没有可写入执行锁的已选 SHA；本文不为它选择 fork、不编造镜像或兼容版本。** 本次仅将官方文档固定为研究快照：`Tencent-TDS/KuiklyBase-components` 提交 `92c1b7b044f72b9728d5b356c033d53f30b3e05c` 的 `knoi/README-zh.md`，原始文件 3858 字节、SHA-256 `5333e88e9afeb7bb8a9f2ac739d8b708e6cc6231641b1b60dd726d02906f5b45`。这只是审查材料的身份，**不是 MadB fork/version 选定、支持承诺或后端候选**。[固定官方研究快照](https://github.com/Tencent-TDS/KuiklyBase-components/blob/92c1b7b044f72b9728d5b356c033d53f30b3e05c/knoi/README-zh.md)

本次浏览了官方接入说明以确认责任性质：它区分 Kotlin 侧 KSP/Gradle 插件与 ArkTS 包；共享库命名、sharedLib/initBridge 注册、dlopen 依赖预载以及混淆保留均需显式处理。页面中的示例版本与动态推荐没有被采纳为项目锁。[固定官方接入说明（仅研究快照）](https://github.com/Tencent-TDS/KuiklyBase-components/blob/92c1b7b044f72b9728d5b356c033d53f30b3e05c/knoi/README-zh.md)

依 ADR-0017/X08，先确定并验证实际 Kotlin 编译器/KSP 配套，才能做声明读取/服务代码生成、Kotlin native 与 ArkTS 双向运行库装配。双向服务、异常、JSValue/ArrayBuffer、同步/异步回调、线程、释放和 env 停止分别声明责任。它替代 Kotlin/ArkTS 的边界配置，不接在 akinterop 后面。

**当前最早拒绝理由是来源/版本/编译器配套尚未解析，随后仍缺 MadB manifest/profile/lock/组件/镜像及目标证据。** 不应伪装成“已选某版本，仅缺一份测试”。CPF 与上游示例编译器也不能互相推定兼容。

## 5 现有格式能表达什么

当前 `composition/1` 能表达精确包源文件集合、manifest FileRef、源摘要、组件、资产、作用域依赖和阶段责任；`profile/2` 把组合语义摘要/字节引用、SDK 与工具串到项目及锁。源包内容身份与执行镜像身份是不同检查，不可拿上游仓库 commit 冒充 MadB helper 的镜像 SHA。

但它们是**已解析组合的契约**：composition 的 packages/components/assets/responsibilities 至少各一项；每个 package 需要真实 `madb.extension/1` manifest；profile 需要 tools 和 SDK。没有 `unresolved`、`admitted:false` 或部分锁占位语义。

因此：

1. 可以把上面真实固定源码作为研究证据，并用真实文件字节/摘要说明来源。URI、commit、下载文件 SHA 不表示已有 MadB 扩展包。
2. 将来适配实现存在后，可沿现有格式声明实际组件/输出/依赖/责任，无需因 ANI/KNOI 名字另建 ArkTS 平台。
3. 目前不能为了得到一个“结构通过的 X profile”而虚构组件入口、空壳 manifest、SDK、工具或零摘要。将未解析 KNOI 填进上述必填字段尤其不成立。
4. `restrictions` 和 `extensions` 是说明/非语义元数据，不能暗藏拒绝规则。backend_config 只提供 FileRef 接缝，不意味着已有读取器或允许忽略内容。
5. 允许在负例中提交真实已注册基线的输入并改成明确不支持的选择，以证明拒绝发生在执行前；这种输入必须标为**拒绝反例**，不能另称 X 的完整 profile、锁或适配已完成。相关来源/缺口仍需与本条事实关联，不能只剩匿名 unknown 字符串。

## 6 实际拒绝层与 E02 收口建议

只读当前 `adapters/contracts/src/composition/pins.rs`：

- 非空 backend_config：`CAP005`，要求已注册后端语义校验；不能用 FileRef 让配置静默跳过。
- composition/profile/Host 固定 recipe 的 selection 不一致：`LOCK001`。Host 只准入明确的 profile、route、direction、target。
- selected components 必须等于 Host 要求闭包；组件必须由真实 manifest 导出正确端口，并绑定源包/lock 的精确身份。缺组件或错端口不能靠包名相似通过。
- 包源、entrypoint、SDK、profile、schema 和执行镜像各自有检查责任。上游 feature 自报不授予受检语义与执行权限。

这些是源码事实；本轮没有运行相应拒绝反例，不记为通过。具体测试应断言准确错误层、零后端进程/零提交，并保留基础普通配置的有效候选。

建议由当前 E02 owner 补齐已有第 6 条的证据关联：普通与已选增强的真实 profile/包源/镜像/责任及禁用对照；三个具名替代选择的上表事实、具体准入差异和生产入口拒绝结果。拒绝只证明当前不准入，不授予 X 支持；不启动 X 的构建/认证，也不新增这些 X 的实现为主线前置。若当前生产入口尚不能给出这组可核对结果，E02 仍保留 in_progress/missing；不能删验收或以文档替代执行。

## 核对来源与取得方式

GitHub 网页/原始文件的 web 工具对两个固定提交返回 cache miss；随后对**相同官方 raw URL、相同固定提交**只读获取成功。下载的 11 个文件和 SHA/长度在 `e02-alternative-sources/receipt-network.json`、`receipt-more.json`，最初失败记录也保留；未执行下载源码、未抓新分支替代固定提交。修订 2 另外通过官方 GitHub commit API 在读取前后取得同一提交 `92c1b7b044f72b9728d5b356c033d53f30b3e05c`，并分别获取该提交与本次 master 的 README 原始文件，确认两者逐字节相同。前次 web 渲染没有暴露提交和原始文件字节，本文不追称其当时已固定；本修订重新读固定字节，核实上述接入责任描述。取证并不将 master、该研究提交或其示例版本解析成 MadB 项目选择。

关键固定字节：UniFFI Cargo.toml SHA `95a6cd921bb9eaab2ba489ffaa108a8e55bc253f8a1eae725fec5b809ec28c04`、Cargo.lock `522036f2a94ae38394c25354a7447fd4ec33bde4fca0841f364fc6f8ec2e16d5`；ANI Cargo.toml `22d547e1917c4cb7c9219af17eff47962b4fc7cf0e1740811977f5192685d53e`、异步指南 `3312971000af98df71b13643147d40e5257d3e5dece2597edf433b714e851246`。这些只绑定研究输入。

KNOI 补证独立保存在 `e02-alternative-sources/receipt-knoi-research-v2.json`；包含官方 API/固定 raw/permalink URL、前后 commit 响应摘要、固定和 master README 的长度/摘要及字节相等结果。`knoi-master-before.json`、`knoi-master-after.json`、`knoi-README-zh-fixed.md`、`knoi-README-zh-master-observed.md` 均保留。原 11 份 ANI/UniFFI 来源文件和两份成功 receipt 未修改。
