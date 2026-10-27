# E04 最小公开契约与有限配方提案（修订2最终稿，待决定）

本稿绑定产品 `2fac2f7437e7bc235392fcca8c5d7af86b510e22`、现行 E04 四项验收及 P10、ADR-0018；只提议一个 `aarch64-apple-darwin` / Dart Native 宿主目标。原型不构成扩展包验收或发布。建议批准以下普通 Dart API、同步资源映射和有限配方；不新增 wire、SDK C ABI 或核心语言分支。

## 1. 建议采用的普通 API

生成的消费者包名建议 `madb_dart_ffi`，首个有限 Engine 模块如下。业务名称从受检声明/显式角色映射生成；不是把此固定样例复制为通用生成器。

```dart
final api = MadbLibrary.open(packageDirectory); // 只接受本包锁定的本机候选；先校验接口/profile。
final engine = api.createEngine();
try {
  final int sum = engine.checksum(Uint8List.fromList([0, 1, 127, 128, 255]));
} finally {
  engine.close(timeoutMilliseconds: 1000);
}
```

公开面建议：

- `MadbLibrary.open(String packageDirectory)`：载入该生成包对应的原生库；安装包目录显式指定，open仅经同一受检bridge调用公开bootstrap读取identity64并核对接口/profile，不调用checked-create、不预创建Engine；失败不产生Engine。不提供动态卸载，进程内保留库与函数指针，不把卸载/GC 计作资源关闭。
- `MadbLibrary.createEngine() -> Engine`：此处才调用同一bridge中的原公开checked constructor，重新核对identity后获取并转移唯一handle；`Engine.checksum(Uint8List bytes) -> int`。
- `Engine.state -> EngineState`，枚举仅 `open / closing / closed`。
- `Engine.close({int timeoutMilliseconds = 1000}) -> void`。成功后重复 close 无副作用；调用参数错误在任何原生动作前拒绝。业务在 closing/closed 上拒绝，不借 raw handle 逃逸。
- `MadbCallException`：只读 `operation / stage / status / domain / code`；准确保留现行稳定原生状态。加载错误、接口不符、profile 不符与业务错误分开，建议 `MadbLoadException`、`MadbIdentityException(kind)`。输入范围/不合法字符串分别使用 Dart `RangeError` / `FormatException`，不混成原生成功。

`packageDirectory` 代替原稿任意单库路径：它定位同一冻结包，不能任选bridge A再传native B。公共API决定的是单一安装包装配和身份检查；具体内部实现采用以下机制，不留下两个任选路线：

1. 包内受管安装清单是生成器与SDK配对的**私有资产布局**，不作为新的公共请求/响应、可编辑配置或独立兼容wire；只由固定生成器写入，SDK按内嵌精确摘要消费，不允许用户自报accepted或换清单。内部格式随包/生成器版本一起冻结、未知键/版本拒绝；不注册新公共schema。该清单含bridge/native/identity的相对路径、SHA256与大小；期望清单摘要内嵌于生成Dart模块（无自哈希环）。目录只允许这套受检文件闭包，路径必须规范化、无重复/case alias/父子冲突，包内拒绝符号链接；拒绝路径越界和未知库。外部传入目录先取canonical root，之后所有相对引用必须留在根下。
2. 读取并校验包字节，写入本次`MadbLibrary`拥有的全新私有临时目录；bridge和native保留精确字节，以owner只读/可执行权限冻结。安装清单记录的加载图由构建阶段真实Mach-O依赖检查形成：bridge仅用`@loader_path`关联同目录唯一native资产及声明的系统依赖；不得从全局PATH/任意rpath选SDK库。此为绑定已信任候选，不声称抵御同用户恶意并发篡改。
3. Dart只`DynamicLibrary.open`该私有目录的bridge，**所有业务/close调用都走bridge的转发入口**；不另open原生库或从`DynamicLibrary.process()`解析业务。bridge转发复用原公开C函数，checked constructor也调用同一组函数，既有C ABI不改。
4. 在create前，用bridge内`dladdr`逐一报告其实际绑定的bootstrap/create/checksum/request/await/release函数来源；canonical路径必须全部是本次私有目录中同一native镜像。绑定图验证不执行create。Dart复核该文件和bridge的字节身份，再经同一bridge的identity-only入口调用原公开bootstrap读取64字节、比对原接口/profile，才构造可用library对象；此阶段不得调用create/checked-create；任何错图/错字节先拒绝。正式负例必须含同interface/profile但不同image、双native A/B、错误bridge、链接图逃逸及字节漂移；不能只检查identity64。
5. library与Engine保留bridge引用，不提供unload；活跃owner不能因删除目录/GC/进程退出伪称close。本有限机制将私有目录保留到进程退出，Dart退出**不会自动删除临时文件**；正式安装/验证宿主需在确认消费进程退出后，按本次实际创建的精确目录清单清理。独立SDK用户也需按安装说明清理该私有缓存；Engine.close不删除仍在映射的库文件。不把缓存清理计作SDK资源close，不承诺进程内卸载。

同一固定包在同一进程连续两次open，各自建立新私有目录；本轮JIT/AOT实测两个实例的六个native函数都指向各自目录，未因相同install-name复用前一镜像。首版据此支持该固定目标下的顺序重复open，仍逐次检查加载图；若实际绑定至旧镜像则准确拒绝imageGraphMismatch。此证据不承诺任意不同版本库共存或并发open。

这是建议的内部最小装配路径，本轮loader-003及loader-repeat-001已验证其有限可实施性；正式生成包、完整异常实现与干净独立消费仍须批准后完成。旧p10-003分别打开两库的原型**没有证明此装配保证**，原报告保持其局部范围。

错误的完整公开分类建议：

- `MadbLoadException`字段`kind / path / expectedDigest? / actualDigest? / cause?`；kind固定为`unsupportedTarget / invalidPackage / missingAsset / pathViolation / digestMismatch / nativeLoadFailed / imageGraphMismatch`。`cause`是可选原Dart异常，不作为稳定机器码；摘要字段仅在已取得相应事实时填写，加载失败不编造native状态。
- `MadbIdentityException`字段`kind / expectedInterfaceDigest / actualInterfaceDigest / expectedProfileDigest / actualProfileDigest`；kind固定`interfaceMismatch / profileMismatch`，对应现有checked helper的2/3。bootstrap非0为调用失败，不能混入identity不符；create非0沿下述call类。
- `MadbCallException`字段`operation / stage / status / domain / code`；stage为`bootstrap / create / invoke / requestClose / awaitClosed / releaseHandle`，status/domain/code精确保留原返回值。正常响应却违反清零/长度/UTF-8等后条件是`MadbContractException(operation, reason)`，reason有限为`invalidStatus / invalidErrorPayload / invalidOutput / invalidIdentityLength`；不得伪造一个native InternalFailure值。
- Dart本方参数验证继续用`RangeError/FormatException`；已closing/closed业务调用用`StateError`。异常本身不授予资源已清理结论。

## 2. 类型、错误与关闭语义

| 输入/行为 | 有限映射与准确拒绝 |
|---|---|
| u8 字节缓冲 | `Uint8List`；调用前复制到本方分配内存，原生仅借到同步返回；0长度可用null；最多65536字节；更长准确拒绝。不得跨调用保留。 |
| i32 / u32 | Dart `int`，在写入 FFI 前显式检查有符号/无符号范围，不允许截断。i32最小/最大、u32最大已有独立公开值候选探针。 |
| checksum 的 u64 结果 | 此角色契约保证最多65536×255=16711680，可精确返回 `int`。这不是通用u64保证；任意u64公共值若没有受检范围前提，首版拒绝，不用带符号重解释伪称无损。 |
| handle:u64 | 仅包装私有的SDK实例局部opaque位模式，不公开为业务整数，不允许跨库或跨isolate传递；不声明OS线程固定。无地址重建/全局registry。 |
| UTF-8 值 | 第二业务可复用已通过ADR-0022的有限值形状：严格Unicode、显式byte count、内嵌NUL保留、最大64字节；Dart孤立UTF-16 surrogate拒绝，不能静默转U+FFFD。无需支持任意字符串ABI。 |
| 错误 | MADB状态0–11沿原ABI；非DomainError输出domain/code为0；DomainError保留准确领域身份/码。C ABI不能跨界抛异常；工具失败和Dart验证失败不伪造native业务状态。 |
| close | 参数验证通过并开始close时，包装先进入closing并关闭本地业务准入；request_close、await_closed依次成功才可release_handle；release成功才变closed。任何步骤失败都保closing和owner，可重试；不能用finally提前release。closing表示包装的关闭尝试/业务准入状态，不伪称原生request已确认。 |
| isolate | FFI同步调用实际阻塞当前Dart isolate；await_closed也会同步阻塞到完成或预算到期。此首片不提供Future、不自动启动worker isolate、不转移owner。Dart isolate不等于固定OS线程，FFI沿当前调用线程执行；此有限C ABI采用caller策略，不新增线程亲和保证。1000ms为建议API默认等待预算，需本次决定确认；不是已通过的性能预算。 |
| 不承诺能力 | async/stream/host callback、跨调用borrow、任意C++/Rust对象、Flutter/UI、Dart Web、跨isolate共享资源均不在首版；请求时由生成/能力准入明确拒绝。底层库有observe符号不意味着Dart开放它。 |


关闭失败状态表：参数范围失败→原state/owner不变且无原生动作；首次close进入→本地closing；request失败→closing且不调用await/release；await失败→closing且不调用release；release失败→closing且不丢owner；三步成功→closed；已closed重复close→不再调用native。重试从request起按既有幂等协议前进；不可恢复的InvalidHandle不自动伪装closed，仍准确报告，不能凭进程退出补证。

单isolate依据固定SDK的公开SendPort语义：传递对象图中含`DynamicLibrary`会被VM拒绝；library/Engine强持有该对象，不提供只携带整数的可发送owner替身。已有`e04/isolate_probe.dart`真实向另一个isolate SendPort发送library和Engine，两次均抛`ArgumentError`，原owner仍能调用并显式close，接收isolate真实退出；仅为该固定SDK的拒绝证据。正式JIT/AOT均应复验。允许另一个isolate独立创建自己的library/engine是另一实例使用，不能凭复制对象继承原owner；首版不承诺共享同一个Engine。

资源同步子集仍必须保留底层完成/排水后条件。当前纯同步消费者无订阅、不创建回调，因此未声称验证Dart回调排水或Timeout交错。正式受检异常路径与独立Lab必须补齐可适用责任证明；不得把普通close很快返回当成普遍无阻塞。

## 3. 有限公共配方与组件绑定（建议，不伪装已注册）

建议包身份 `org.madb.dart-ffi@0.1.0`、路线 `org.madb.dart.c-ffi`、profile `dart-c-ffi-sync-aarch64-apple-darwin`，provider=`c/native` 表示已公开C ABI，consumer=`dart/dart-native`、direction=`consumer-to-provider`。Rust提供的既有C候选可消费，但不扩张为任意Rust API。

复用 `madb.extension/1` + `madb.composition/1` + `madb.profile/2`、protocol/IR既有版本及`core.extension-composition@1`。资源子片feature采用现行 `core.values@1 / core.exact-numbers@1 / core.resources@1 / core.explicit-close@1`；若选择第二自然值模块，其公开contract真实要求 `core.errors@1 / core.variants@1 / core.exact-numbers@1 / core.values@1`（offset的throws=ReadingError），完整保留而不能裁掉错误/变体语义。其余feature依实际受检输入目录，禁止把旧Dart example里的 `core.call.sync@1` 等设计字符串静默当有效feature。最终组合参与者分别声明实际理解的语义。

| 组件/资产 | 最小责任与现存接点 |
|---|---|
| C声明 Reader | 复用正式Clang/header/layout读取机制，通过独立 `contract-reader` 的公开 read 会话输出受检IR；C05现有 `org.madb.rust-c/reader-generator` 是聚合/固定首链实现，不能冒充示例 `madb.clang/reader` 已发布。本轮选定新增真实单port公共C声明读取组件，输入仅公开C头、原candidate/contract/identity及受检同步角色选择；通过原read wire与AuthorSession交付。复用Clang结构读取机制的实现由工具维护层公开，不允许扩展直接import其私有模块；不采用“直接信任旧IR”等价替代；不得由扩展import生产私有lowering。 |
| Dart Generator | 新单port `org.madb.dart-ffi/generator`；按受检IR/策略生成ffigen配置、raw bindings、普通Dart薄包装与公开checked-helper导出桥。拒绝未支持的语义。只从宿主固定的符号选择/角色与能力推导责任，申请者responsibilities不能自授覆盖。 |
| Toolchain | 固定Dart/Clang/libclang/SDK镜像，调用既有 build 端口和执行/取消/完整输出检查。复用公开调度支持；Dart只需新增其参数/诊断适配，不重建宿主执行器。 |
| Packager | 复用现行候选/文件清单/许可证与宿主原子提交，新增Dart包目录的有限装配。不授予扩展ArtifactStore/发布权限。 |
| Verifier | 独立Lab实际Dart消费，公共case/1+report/1及固定expectations；扩展只返回观察。 |
| SDK资产 | 原生公开候选头/库/identity/contract作为精确锁定资产；generated Dart、必要桥源码/本机桥库属于Dart包。包源摘要与实际Dart/Clang/组件镜像分别锁定。 |

C05完整contract含callback feature；Dart同步选择不得篡改其原身份或把完整IR当同步能力通过。需要显式、受检的同步视图：保留原C ABI/候选身份以checked helper匹配，选择六个非observe角色并为所用类型/策略产生新Dart selector与来源血缘；申请observe必须拒绝。此是已有字段内的有限读取/生成选择，不提出新wire；若实际现行接口不能无歧义承载这种选择，必须回报具体缺口再决定，不能降格旧profile或忽略feature。

第二独立业务优先复用ADR-0022公开自然值候选的受支持record/值方法形状来验证新增/改名/删除后的正常生成；资源模块与值模块各自保留native身份，不把两库拼成假统一ABI。此处是正式验收用例，不将所有值特性扩为首版必需承诺。

## 4. 实际工具与P10证据

- 官方 stable Dart `3.13.7`，revision `416e33b0ffaef062a60ec966bb8c82ed2b83f128`；归档 `dartsdk-macos-arm64-release.zip` SHA256 `9f972ee1e1139cb4f845f35c7c77bef269dd2ec727b239fb17a2ebcb66d0fb2f`，225890953 bytes；仅解包至本任务目录。
- Dart镜像SHA256 `5f36d10cbdb3206fd6951e3090975d23b2f9659c41c2d17a7c8e183888f3f08d`。macOS26.6.2/arm64，实际Apple Clang21.0.0；compiler/SDK源闭包必须在正式候选另行完整冻结。
- 官方ffigen `23.0.0` stable，pub.dev归档SHA256 `6f55d99ac1857fc063b58c3b31753df2bba417028ba98d30c6d625a385855064`。仅采用固定归档中的公开API，不依据main分支；程序式generator配置，依赖完整pubspec.lock。ffigen生成仅是raw声明，不授予资源/错误正确性。
- `e04/p10-003/result.json`：公开Engine候选真实JIT+AOT，普通路径和两种精确身份错配通过，输入前后不变。既有C候选digest `sha256:9a09515422305c2bf09a452c68d6afe373c27e277b510509f986c55395446959`。
- `e04/values-001/result.json`：另一公开自然值候选真实JIT+AOT，8组布局/值/范围/Unicode/领域错误/清零/恢复通过。这里只证明Dart映射可行，不计正式扩展或主线继承。
- `e04/loader-003/result.json`：20命令真实完成，JIT/AOT均只open bridge；六个实际native函数来源同一私有镜像，identity-only预检早于create。同身份但链接至目录外实际镜像准确拒绝`imageGraphMismatch`；错字节、符号链接与路径越界分别拒绝。仅装配机制scratch，不等于正式包或完整loader实现。
- `e04/loader-repeat-001/result.json`：同一进程顺序open两次，JIT/AOT均实测独立私有镜像、正确身份与显式close；临时目录保留在证据目录，没有声称Dart自动清理。
- `e04/generation-001/result.json`：固定ffigen公开API重复生成同字节raw声明，真实JIT/AOT消费及checksum上界通过。它仍使用P10普通候选和scratch桥，不计正式双会话、SDK资产包或EXT通过。
- `p10-001`保留CLI直接VM参数错误；`p10-002`保留原型把interface错配误写成profile预期的oracle失败。`p10-003`按公开identity独立区分kind2/3，未放宽任意错误。`loader-001`保留新bridge Mach-O路径预留不足，`loader-002`保留分析器尝试用户目录的沙箱拒绝；后续仅修本任务桥编译参数与本任务子进程环境。ffigen早期解析诊断/opaque声明原件也保留，最终生成检查不只依赖exit0。

官方依据：[Dart C interop](https://dart.dev/interop/c-interop)、[稳定SDK归档](https://dart.dev/get-dart/archive)、[固定3.13.7 VERSION](https://storage.googleapis.com/dart-archive/channels/stable/release/3.13.7/VERSION)、[数值表示](https://dart.dev/resources/language/number-representation)、[ffigen23.0.0](https://pub.dev/packages/ffigen/versions/23.0.0)。归档内 `sdk/lib/core/int.dart` 与 `sdk/lib/ffi/*` 为本次固定源码依据；不把在线文档最新分支能力当固定SDK能力。

## 5. 不可省略的正式退出及决定前可做部分

决定前工作在此收束：工具与公开资产pin、公开头互调、有限生成以及推荐装配机制已有实证；实施文件与公共端口接点另见`e04-implementation-scope.md`。不追加正式EXT前置运行，也不把本稿API或新包/profile注册到生产并据此验收。

决定后最小正式链：固定包与真实reader/generator公开会话→完整typed组合与OutputSet/实际字节校验→构建/打包未插桩Release→无产品私有源码的全新Dart消费→独立Lab适用case/report→禁用后原C05路线消费→旧新版本锁隔离/失败不覆盖→独审与catalog收包。E04完成仍须四条逐项对账。

EXT适用：01/02真实注册、锁、组件/目标/未知feature拒绝；03真实会话/取消/恶意帧/路径；04数值/空值/编码/不支持声明；05正确分配器/借用/关闭与owner；06异步/流/回调请求准确拒绝、isolate边界；07领域错误与系统/工具失败；08确定性/独立业务/失败保全；09真实公开Dart消费者，无UI明确不适用；10干净Release装载、锁/许可/包一致；11固定环境样本与预算另记录，不从本次启动成功造性能承诺；12不改核心语言分支、禁用与版本隔离。Actions未执行不能写通过，实验后端和其他平台不继承。
