# E01/E03：有限组合格式的四项具体决定提案

**建议采用下列四项作为一个格式决定；本文仍待裁定。** 不重议已批准的 C07/L05 选型，不改变旧 profile/1、现有候选或历史验收。只提出 `composition/1 + profile/2` 的最小边界；不引入任意组合求解器。配套 JSON/Python 是设计样例，不是新的任务/状态系统，也不是产品 decoder。

本轮读取了实际产品 AGENTS、共同方法、E01/E03 目录及其相关规范；权威源根为 `/Users/changyue/Desktop/MadB/madb`；设计复验默认读取已捕获且不参与并行写入的 `/private/tmp/madb-chief-round3/madb-baseline`，HEAD `5468d314928f8c5d494768a9b17e3ad6b7d22fcb`，含重要未提交输入。样例的 17 个真实文件逐一与主协调的 round3 捕获对账；绑定该历史输入，不声称覆盖后来集成候选，完整 SHA/size 在 [composition-decision.json](composition-decision.json)；未以 HEAD 替代脏输入。仅写本文件及同名 `.json/.py`。

## 先修正上一稿的具体问题

| 发现 | 影响 | 本提案修正 |
|---|---|---|
| C07 当前 `org.madb.arkts/reader-generator` 是一个聚合锁身份，摘要按 `madb.arkts-component/1` 计算 generators 子集；manifest 每个 component 只能有一个 port | 不能把它当已注册的 reader 和 generator，也不能把其 sha 当 manifest/整包/可执行镜像 SHA | 保留旧身份；新配置提出两个单 port ID，并区分包源、manifest 字节和执行镜像 |
| OutputSet.generation_id 纳入 profile/task/input；源锁 inputs 又决定 IR.source_digest | 把本轮生成资产字节或 OutputSet 写回 composition/源锁会形成回环或改变上游身份 | 输入只声明生成位置；已验收字节仅向下游传递 |
| 上一稿要求“完整语义通过后再算 composition 摘要”；但完整 IR.selector 已需要 profile_digest | 即使无直接 JSON 引用，执行前提仍循环 | 先算结构合格输入的身份；再用真实 IR/输出证明语义，最后准入 |
| 自定义 scope ID，或同文件的两个 asset ID，可给同一实际 owner 换名 | 重复源订阅/排水 owner 可能看似不同责任键 | 用规范来源解析 anchor，scope 由有限配方独立生成，拒绝别名与自由 layer |

依据：`adapters/toolchains/arkts/project.py:314`、`adapters/contracts/src/projections.rs:197/223`、`resolution.rs:92`、`task.rs:325`、`common.schema.json` selector、`extension-manifest.schema.json` component。独立只读复核确认了这些限制；本轮不修改上述代码。

## 决定 A：新包源身份与 SDK owner，不伪造现有注册

建议第一批只支持显式本地 `file-set` 源。以下都是**新提案身份**：包 `madb.arkts-native`；单 port 组件 `madb.arkts-native/reader`、`madb.arkts-native/generator`。样例版本 `0.1.0-design` 不是发布版本。旧 `org.madb.arkts/reader-generator@0.1.0` 继续只用于旧配置，不默认为新 ID 的别名。

SDK owner 改为 `{kind:"sdk-asset", package:PackageId, asset:LocalAssetId}`。资产属于包，不强挂到某个进程；因此 L05 不需要假造 `observer` 端口或另建 runtime。普通和可选 SDK 源继续位于现有 `sdk/`；`extensions/` 仅持清单/索引。

```typescript
// 候选公共字段；均闭合、无隐式默认值。Id/Path/File/FileRef 复用 common/1。
Package = {
  id: Id, version: ExactVersion,
  manifest: FileRef<"madb.extension/1">,
  source: {kind: "file-set", files: File[]}, source_digest: Digest
}
Component = {id: QualifiedComponentId, version: ExactVersion, sha256: Digest}
```

包源清单包含 manifest **原字节**和所分发代码/资产的完整文件清单，禁止自行散扫目录补文件。source_digest **复用既有** `source_digest(files)`：按 path 排序后，对 `{path,sha256,size}` 数组用 `madb.source/1` 域计算，不新增第二种文件集 hash。manifest 不内嵌包源摘要，因此不自哈希。

这批新 Component.sha256 明确定义为所属包的 source_digest；两个单 port 可共享同一包源 SHA，id/port/args 在被该 SHA 覆盖的 manifest 中区分。这是**只针对新 ID 的身份约定**，不改旧组件 SHA 含义。manifest.sha256 始终是原文件字节摘要；实际 spawn 仍独立绑定 ProcessRequest.tool 的已安装镜像字节、args、env。包源 SHA 不能替换现有 `bind_tool` 的 installed image digest。本轮另行实现的单镜像 `check_extension_pins` 只允许 component SHA 等于 image SHA，也不能直接套用新 /2 包源模式；/2 需显式受检入口分别绑定包源组件与执行镜像，保留旧函数拒绝及含义。

来源是宿主显式选择的本地根加精确文件字节；HEAD/绝对路径仅作审计定位，不充当内容证明或可迁移身份。不声称该机制完成第三方发布者认证。未知 source.kind 暂拒绝；远程归档/签名在有具体交付需求时扩展同一注册边界。包内路径保持相对路径；进入扁平 lock.inputs 时固定映射为 `packages/<package-id>/<exact-version>/<package-relative-path>`，并拒绝同 ID 多版本和规范化路径冲突。映射只改变该层逻辑路径，字节 sha/size 不变；包源摘要始终用包内路径计算，不能混算两种路径域。包源及 manifest、composition 的文件均由现有 lock.inputs 绑定，schema 由 lock.schemas 绑定，新 component 三元组由 lock.components 绑定；lock.profiles 经 profile/2 间接固定这些关联，首片不增加 lock/2。

配套 manifest 故意 `example:true`：现有 driver 是私有 metadata/generate CLI，尚非 E02 包入口，必须被当前注册器拒绝。JSON 保存候选内容，脚本明确采用规范化 JSON 加单个 LF 序列化并计算完整 SHA，**没有把尚不存在的文件宣称已安装**。样例代码切片只用于格式验证，不是完整可执行包闭包；真正实现必须补入口/完整清单，产生新包源 SHA 后才能注册。该缺口是实施事项，不需要用户再次裁定 owner 模型。

## 决定 B：生成资产分开表达输入意图与输出证明

```typescript
Asset = {id: Id, package: PackageId, source:
  | {kind:"source", files: File[]}
  | {kind:"generated", producer: QualifiedComponentId,
     output_slot: Id, paths: Path[]}}
```

`source` 的 files 必须属于包源清单并被 manifest 的 SDK 路径覆盖；按规范来源唯一，不允许两个 ID 隐藏相同来源。`generated` 初始没有 sha、size、generation_id、task_digest、OutputSet FileRef。其 producer 必须绑定 generator 端口；`output_slot` 是有限配方的精确输出位置标识，不是自由表达式或动态调用。C07 样例使用真实生成路径 `generated/facade/Boundary.ets`，候选 slot `ordinary-facade`。

生产者完成后，由既有 OutputSet 与 host 的验收取得真实文件、task/profile/input/generation 身份和冻结字节；内部受检值绑定为该 slot 的产物。下一阶段只收到这份冻结字节，不回头使用可变路径。不得回填本轮 composition、profile、源锁 inputs；下游新增输入不改变前一阶段已冻结输入。生产者失败、取消、错 lineage 或缺文件时不形成已绑定资产、不提交半批次。

若要复用**以前独立完成**的制品，只能作为新的明确输入固定其已有身份与字节，不能把本轮未来结果伪装为已有源文件。当前 profile.sdk/backend_config 同样不得依赖本轮下游结果；按已知引用角色和 host 事务输入边界检查，不能任意遍历未知 JSON 字段并执行它。

## 决定 C：有限配方生成 required set，完整准入分阶段

责任行仍为 `{boundary,subject,stage,scope,owner}`。取消自由命名 `scopes[].id`，scope 改为 `{kind,anchor,layer}`。公共形状采用 Id/带标签 subject；**合法 kind/layer/anchor 组合由被固定版本的配方产生**，不是调用者可扩张枚举的自由文本。冲突比较先把 asset/anchor 解析为 `(包源SHA, 受检资产路径或producer+slot, 导出成员)`，再比较责任键；同来源的另一别名不会创造新作用域。

当前有限样例只拒绝该配方的未知 anchor/重复资产，尚未证明跨包 re-export 或任意物理实例别名分析。生产中的 required set 来自真实受检 ContractIR + 所选 SDK 配方/导出及生成接线；不能读取申请者 responsibility 数组来定义“需要哪些责任”。通用纯规则只接收确定的 required/provided 值比较，不按 C07 字符串写核心语言分支。首个适配器只提供 C07 普通与 L05 增强的有限规则；未知配方、未知 layer、缺失/多余责任、不同 owner 的同键冲突均拒绝。通过删必需符号或改 scope 不能缩小 required set。

执行顺序：

1. 核结构、schema、显式源文件及版本能力，计算输入 composition/profile 身份；**算出摘要不表示组合有效**。
2. 按这些身份读取真实声明形成 IR；先验证可在生成前确定的必要能力/唯一责任。
3. 在隔离域生成；按真实导出和接线复核 required set、输出 owner 及 slot/文件。只有全部必需节点通过才沿既有 Source/Build/Candidate 管道提交。没有输出前不能声称其导出已存在或完整责任已证明。

真实有限映射（不是整包所有责任的穷举）：

| subject / scope | 唯一职责 | 实际实现 |
|---|---|---|
| `madb.c07.Engine.roundtrip.v1` / call+facade | projection；该同步调用不新增 async/subscription/drain 行 | `sdk/arkts/facade/Boundary.ets.in` |
| `madb.c07.Engine.observeState.v1` / subscription+source | 源注册及预算 | `sdk/arkts/native-runtime/src/protocol.cpp` |
| `madb.c07.Subscription.close.v1` / 同一 observeState 的 subscription+source | source/delivery/entered 排空 | 同一 native protocol |
| 同一 close / subscription+facade | 真实 retire 清除该订阅的 JS listener 引用 | `sdk/arkts/facade/Boundary.ets.in:318` |
| `StateObservation.requestStop/close` / receiver | 接收门、共享 close attempt、保留原订阅直到成功；复用上两行 | `sdk/arkts/observation/StateObservation.ets` |
| `ArkUIStateModel.attach` / receiver+view-model | 真实 Snapshot 到 @Trace 属性的投影 | `sdk/arkts/observation/ArkUIStateModel.ets` |

此处特意不把 runtime dispatcher napi_ref 分配给 Subscription.close：它只在整个 Runtime 的 teardown 后删除（runtime.cpp:127–170）。订阅的 native callback context 经 api.cpp:99–108 的 callbackDrop 释放，受 protocol.cpp:282–324 的 drain 控制；这与 facade listener 是不同责任。L05 不能成为 native drain 的第二 owner；页面持有者仍负责保存/加入 close Promise，不能由 SDK 资产声明代替。C07 的 C provider 配对 release、gateway 接线和一份物理 native 制品仍有各自必需责任；设计样例只验证表中子集，不据此登记完整 coverage。

## 决定 D：新必需 feature 及只影响相关参与者的协商

建议新增候选 `core.extension-composition@1`，表示理解 profile/2 的组合输入、唯一责任、资产来源及只提交完整组合的约束；不增加新的 SDK async/stream 语义或 wire operation。它写入组合 profile 的 required_features，旧核心因不认识该功能/版本明确拒绝，不降成可忽略元数据。名字及含义与格式一并裁定，不在此写入 CORE_FEATURES。

| 参与者 | 本功能何时必需 |
|---|---|
| host 的 schema/decoder、组合解析、纯规则、计划与提交准入 | 总是必需；新 profile/2 不允许旧路径兜底 |
| reader/generator | 读取 profile/2 或其资产/接线前必需；manifest 声明与 hello_ack 均须满足 |
| packager/verifier | 其请求要求解释/验证组合语义时必需；若只执行 host 已限定的普通文件打包/用例，则不机械附加 |
| 只接受既有 BuildTask 的编译器/构建 helper | 不因处于组合中就被迫声明理解 composition；继续核冻结输入/工具/selector |
| Lab/候选身份读取器 | 必须识别相关 profile/组合与所选必需用例，不能把旧候选结论移到新组合 |

feature 由 host 按真实操作责任计算，不允许调用者随意传空集合绕过。控制帧保持 extension-wire/1、现有 hello required_features/ack.features 和五个操作；旧 profile/1 不增加 feature。协议可用不等于当前 C07 私有 CLI 已适配 E02。

## 摘要与兼容：只增加所需两种格式身份

`Pc` 包含完整输入 composition；集合排序后用 `madb.composition/1` 域及既有 JCS 算语义摘要。packages/components/assets 按 id，包源和源资产 files 按 path；依赖按 from/to/scope，责任按声明的 `(boundary, JCS(subject), stage, JCS(scope))` 排序，冲突另按解析后的规范来源判断。先拒绝重复/别名，不排序去重。order-sensitive args 永不排序；字符串不 trim/归一化。

profile/2 的唯一新增字段为 `composition:{id,semantic_digest,file:FileRef<composition/1>}`。P2 对旧字段沿原 profile 投影处理，只把新增字段替换为 `{id,semantic_digest}`，用 `madb.profile/2` 域。字节 FileRef 先验 hash/size/schema；它变化不自动等于语义变化，任务对实际原始输入的失效规则保留。

依赖方向：包源/manifest → composition → profile/2 → 源锁/IR → OutputSet → 下游与候选。禁止当前批次的逆向引用，包括经 sdk/backend_config 或包源清单绕回。profile/1 的结构、域、摘要和值解释完全不动；未知 profile/3、composition/2、source kind、配方和必需 feature 均拒绝。I01 boundary_refs 初期仍只认 /1；/2 随此决定与共同向量实现后显式加入，不永久封死，也不承诺任意未来版本。

## 设计验证结果和交付界限

配套 `.py` 使用项目 Python 3.14.4，先按捕获 SHA 核实 17 个真实输入，再校验 `.json`。修订版 **49 项设计检查符合预期**：普通/增强/禁用、集合换序不改语义、字节与语义区分、未知版本/字段、旧聚合ID冒充新组件、manifest SHA 冒充组件源 SHA、错字节、生成资产提前哈希、错 producer、缺 drain、改 scope 隐藏双 owner、双资产别名、增强接管 native drain、同步调用硬加 async、禁用漏带 UI、三类摘要回环及相关 feature 缺失。

普通与禁用样例对象及语义摘要完全相同；增强不同。禁用只指下一次选择，不承诺运行时卸载。样例单一包源仍含可选源码，所以包更新可保守使普通配置失效；不宣称已优化增量缓存。这里证明的是有限选择/摘要/拒绝，不证明真实下载、编译或消费者闭包隔离。

样例仅在内存中用真实 facade 模板替换标记，取得一个标为 design-only 的生成字节引用；没有生成 C04 候选、没有编译/设备运行。当前 manifest 作为 design example 通过**现行结构 schema**，并明确保持不可注册。生产 decoder、完整 SDK 导出/接线 oracle、真正入口与全部包源闭包、事务集成和独立普通/增强/禁用消费者仍是决定后的实施验收工作，不能把这 49 项扩大成 E01/E03 或 L05 完成。

原 35 项报告及对应源码已保留在 `composition-decision-r1/`，第二版 46 项报告保留在 `composition-decision-r2/`。独立审查随后发现五处漏拒绝（未选 producer、重 pin 后错误 port、错误 direction、无关 profile_id、asset 未知字段）及一处订阅/runtime 引用 owner 错配；第二轮进一步发现 reader 错端口、generator 缺必需 feature 在重 pin 后仍能通过。本修订按固定配方对 reader/generator 都检查真实 port 与必需 feature，补拒绝向量并修正真实 owner；不能把原报告视为已覆盖这些条件。

复验：`/Users/changyue/Desktop/MadB/madb/.venv/bin/python -B /private/tmp/madb-chief-round3/composition-decision.py`。仅首建样例使用 `--write`；默认复验不修改结果；可用 `--source-root` 显式替换只读源，但仍须匹配原捕获 SHA，不能重 pin 到并行变动的 live 源以制造通过。本文的建议是采用上述四项后在原 E01/E03 工作包实现共同 schema/decoder/digest/向量；不申请扩大已授权产品范围，也不提前宣称格式已批准。
