# I01 剩余格式与首个 Rust/C 执行分片定稿提案

导航副本：仅将目录及带行号路径链接改为文本，规则和结论不变；原始受审字节 SHA-256 为 `09672664872c3d56fe1b81857692be87f4bd8222e38b012e9d5902055e207fa6`，原稿保持于[持久归档](l02-values-evidence-archive.json)。采用或复核仍绑定该原稿。

本文是**待决定、未实现**的精确提案，不是采用决定、生产 schema、验收报告或第二份任务状态表。工作依据为产品 clean `e0f74a5fa5a9a6e73400ced70bc81ba06751227f`、独立 Lab clean `2d06cbefa0f49187f163ce937c8c3bd01a062eb1`。本轮只写 `/private/tmp/madb-chief-round7/` 的提案和设计向量；没有注册 decoder、修改两仓、执行 init/CMake/Ninja 或发布模板。

## 1 决定范围与已批准边界

ADR-0019 的 A/B/C/D **已经批准，本文直接采用，不再次提请裁定**：A 请求摘要完整度不决定错误类别；B 普通成功必须真实同根同计划 CommitNew，dry-run 单列；C `template.input_root` 与原输入闭包、删除重复 SDK 要求；D profile/targets 完整集合选择。已实现的是 ProjectPlan 的现有纯校验及 D；其余公开入口与本提案中的模板/结果解析、包获取、renderer、CommitNew、init 有限配方均尚未实现。

本次剩余决定只有以下五组：

1. 两个 envelope 的剩余字段及严格 JSON/版本/路径子集；模板增加完整 `files` 清单，结果增加 `template_package` 原包 File 和 `evidence_root` 证据根定位；`plan.files` 增加内容身份，不修改已批准 template 对象的五字段。
2. 模板包原始归档字节的身份、可信固定目录、完整解包/重验及离线规则。
3. 三种有限 renderer，特别是不能用 copy/json-values 暗中替代的 C/TOML/CMake 标识符渲染。
4. `madb.init.rust-c-sdk.values@1` 首个执行分片的有限输入、输出与检查表。
5. 结果记录、独立必需检查推导、CommitNew 事实与失败恢复如何接到现有应用层。

首分片选择已有 `madb.native-values/1` 自然 Rust → 普通 C 能力是**本次新增 init recipe 值的明确提议**，不是宣称以前已批准这一工程模板配方。它仅用于先落地模板/创建/CMake 接线，不替换 C05 资源接口目标，不删除 `rust-c-sdk` 最终承诺，也不缩减 I01/I02/I03 或 INIT-01…12。C05 资源/异步接口、更多目标、IDE import/run/debug、Wrapper 与组合验收仍需其既定工作和适用联合证据；不能以本分片关闭整包。

## 2 共用规则

### 2.1 Wire、数值和版本

复用现有 Draft 2020-12 离线 schema 基础设施与 strict_json：单文档 1 MiB、深度 64、数组不超过 4096 项、重复键/尾随值拒绝。数值仅十进制整数词法，范围 `-9007199254740991…9007199254740991`；拒绝 `1.0`、`1e0`、`-0`、NaN/Infinity。布尔不是整数。输出 JSON 使用现有 canonical_json（UTF-16 键排序、数组保序、不折叠 Unicode），需要文本文件末尾换行时在 canonical bytes 后固定加一个 LF；摘要语义投影不加该 LF。

所有下述对象字段**精确且全必需**，只在明确标记的位置允许 null/空数组；没有 extensions 或未知字段容忍。未知 recipe/renderer/角色/IDE/SDK 描述不会因对象 schema 合法而成为可执行能力。结构解码、跨引用语义校验和可执行准入分别报告，不自动降级。

复用 common 的 Id、Digest、File=`{path,sha256,size}`、FileRef=`{path,sha256,size,schema}`、Component=`{id,version,sha256}`。File/FileRef 摘要是原文件字节 SHA-256，不是 JSON 语义摘要。Component.version 仍为不透明版本字符串，不能拿它做通用 SemVer 求解。

本版模板 version、madb_compatibility 的两个端点采用有限正式版本 `N.N.N`：每段 `0` 或不带前导零的十进制正整数，0…65535；按三元整数序比较，范围 `[min_inclusive,max_exclusive)`，min 必须小于 max。首版拒绝预发布、构建后缀、通配符、latest/分支；这是本格式的首片可执行子集，不改现有其他格式的版本语义。若当前 MadB 不能映射为已知正式三元版本，拒绝模板准入，不猜比较结果。

`Path` 先过 common/LogicalPath，再在本版模板执行子集限制 ASCII、最多 32 个组件；所有组件前缀进行大小写别名与 file/directory 冲突检查。文件清单最多 4096 个普通文件、单文件 32 MiB、总内容 128 MiB、父目录最多 8192；这是**新增模板准入复用现有输入预算**，不谎称 ProjectPlan 本身已有这些预算。最终输出绝对根允许空格/中文；不等于内部资产路径已支持任意 Unicode。

### 2.2 两种相对根

- 模板内部 File/FileRef 均相对**完整解包根**；manifest 可以位于 `meta/template.json`，其父目录不是根。例：原 profile.sdk.path 为 `toolchain.json` 时，保留原字节就必须让 SDK 留在包根的 `toolchain.json`；把 profile 与 SDK 一同移入子目录也会断开根相对引用，不能按 profile 父目录补救。
- 结果中的 `template.manifest`、`boundary_profiles` 将上述路径仅在外层加 `template.input_root + '/'`；被引 profile/1、profile/2、composition、lock 等原字节和内部路径完全不改。
- 结果 `template_package`、logs、checks/open attachments 均直接相对报告证据根。
- `template.input_root` 不得与报告其他文件冲突。选中闭包跨出单一原包根、含不支持 schema/引用或缺字节，一律拒绝可执行准入，不增加任意挂载和路径重写。

## 3 ProjectTemplate/1 的精确形状

顶层精确为：

```
{schema,id,version,madb_compatibility,default_profile,parameters,profiles,
 modules,boundary_refs,assets,ide_entries,task_roots,validation,files}
```

`schema='madb.project-template/1'`；id 为 Id；version 按 §2；compatibility 为 `{min_inclusive,max_exclusive}`；default_profile 精确引用一个 profile。profiles/modules/validation 非空；其余数组可空但须满足所选有限配方。全部 Id 引用先按身份检查唯一，再按组合闭包检查，不能仅靠 JSON uniqueItems。

| 项 | 全字段与语义 |
|---|---|
| parameters | 见下文的封闭三变体；id 全模板唯一，调用方未知参数拒绝 |
| modules | `{id,role,directory,dependencies}`；directory 为 Path 或 null，null 表示唯一根模块；dependencies 为唯一 Id 集合；role 由所选配方认识。复用 ProjectPlan 的模块/依赖 DAG/owner/目录前缀检查 |
| boundary_refs | `{id,profile:FileRef}`；只接当前支持的 profile/1 或 profile/2，保留其原摘要规则，v2 不降级；profile.sdk、backend_config、composition 的准入由现有校验器与精确配方负责 |
| profiles | `{id,targets,modules,boundary_refs,assets,ide_entries,task_roots,validation,requirements}`；各引用数组为唯一 Id 集合；targets 非空；modules/validation 非空 |
| targets | `{id,family,boundary_refs}`；id 在 profile 内唯一；family 为 Id；boundary_refs 唯一引用集。family 匹配严格采用已批准 D，实际 target/ABI/SDK 从引用的 profile 读取 |
| requirements | 四个必需键 `structure/build/run/open`，每个 `{hosts,tools,ide_entries}`。hosts 是 OR，tools 与 ide_entries 是 AND；没有 sdks 字段 |
| hosts | `{os,arch,min_inclusive,max_exclusive}`；首配方仅认识 macos/aarch64；OS 由该 adapter 解析成三元数字，未知形式拒绝；范围规则同 §2。它只筛宿主，不能代替实际工具/SDK 字节身份 |
| requirements.tools | Component 数组，按 id 唯一；与 profile/已登记配方的独立必需工具取并集，冲突拒绝。空数组不能删掉 recipe 或 profile 的工具要求 |
| assets | `{id,module,source,output,ownership,renderer}`；见 §5 |
| ide_entries | `{id,module,ide,path,run_target,extensions}`；ide 为 `{id,min_inclusive,max_exclusive}`，extensions 为唯一 `{id,version}` 集合，run_target 为 task-root Id 或 null。path 是已计划文件或目录。版本仅由已登记 IDE adapter 解释；未知组合拒绝，禁止任意启动命令 |
| task_roots | `{id,module,recipe,boundary_refs,targets}`；引用所选集合；recipe 为精确版本化 Id；不含 argv/env/shell/hooks/postCreate。模板不提供任意 TaskGraph 或递归 app 调用 |
| validation | `{id,level,target,task_root,required,minimum_evidence,prerequisites}`；id 全模板唯一，target 精确指所选 target.id；task_root 为 Id 或 null；level 仅 structure/build/run/import/debug；required 为 bool；prerequisites 是检查 Id 集合、同选中组合内无环；minimum_evidence 采用既有 static/generated/compiled/linked/host-run/target-run/device-run 语义 |
| files | File 数组；**完整原包普通文件库存，唯独不列 manifest 自身**。manifest 的路径与原字节摘要由可信解析目录给出。库存按 ASCII path 升序且路径唯一；所有 assets/source、boundary_refs 及引用闭包都须落在此库存中 |

参数的精确变体：

- string：`{id,type:'string',required,default,min_length,max_length,pattern,choices}`；default 为 string 或 null；长度按 Unicode scalar 数计，0≤min≤max≤4096；choices 为唯一字符串集，空表示不限枚举；pattern 仅 null 或精确字符串 `[A-Za-z][A-Za-z0-9_]{0,47}`。不开放任意 regex 引擎。所有默认/choices/实际值须满足全部限制。
- boolean：`{id,type:'boolean',required,default}`，default 为 bool 或 null。
- integer：`{id,type:'integer',required,default,minimum,maximum}`，default 为安全整数或 null，min≤max。

null 是“没有默认”，不是有效参数值；required 是默认展开后的必填，不强迫显式输入。`name` 缺省取输出目录 basename，`namespace` 缺省取模板默认；默认不合法即 InvalidInput，不能暗改字符。首配方要求这两项最终均满足上面的 ASCII identifier 子集且长度≤48；C 关键字、Rust 保留字和标准预留标识符由固定语言表拒绝。中文输出根可显式传合法 `--name`；不能把首片 name 限制解释为永远不支持中文显示名称。

## 4 原包身份、获取、完整解包与重验

### 4.1 原始字节与无循环绑定

选定包为**未压缩、受限 POSIX USTAR**。`package_sha256 = 'sha256:' + lowercase_hex(SHA256(整个归档原字节))`，包含 header、payload、padding 和结尾；不加语义域，不用 `madb.source/1` 的文件列表摘要，不用解包后重打包字节，不只算选中 profile。这个定义是待批准的新包规则，而 SHA-256 字节用法与 common.File 一致。

发布目录中的内部受检记录精确具有：`id,version,archive:File,manifest:FileRef,source,recipes`。archive.path 是发布目录逻辑路径，manifest.path 是包内入口路径且 schema 固定 project-template/1；source 为 `bundled` 或单个固定 `https` 绝对 URL（最多 4096 字节，不允许 userinfo、fragment、query 或空 host；URL 本身不是命令，不经 shell 解释）（两者封闭变体：`{kind:'bundled'}` / `{kind:'https',url}`）；recipes 为唯一 Component 集合，绑定内置 recipe/renderer 实现镜像。该记录是随本版本 MadB 发布、由其安装完整性保证的**内部目录值**，本次不新增一个可任意加载的 public catalog schema。manifest 解码后的 id/version 必须与目录记录逐字相同；id/version 不允许对应不同包 SHA；无版本 selector 只取本 MadB 发布目录明确固定的默认，不联网追 latest。

manifest 不嵌入整个包 SHA，避免自引用。manifest.files 不列自身；目录给 manifest 原字节身份，原包 SHA 又绑定整个 inventory。实际包入口集合必须精确等于 `{manifest.path} ∪ files.path`；不能多出一个未列文件，即使未被选中也拒绝。不同模板中的同路径不共享解包目录。

### 4.2 归档执行子集

以下是本配方的严格子集，不是承诺接受所有 tar：

- magic/version 必须 USTAR 的 `ustar\0` / `00`；仅 typeflag `0` 普通文件；不接受目录 entry、链接、设备、FIFO、GNU/PAX 扩展、sparse、base-256 数字或未知类型。父目录由计划安全创建。
- 路径由 USTAR 的 name/prefix 字段拼合，先做字段完整性/ASCII 校验，再做 §2 Path 与前缀冲突；禁止绝对、空段、`.`/`..`、反斜线、盘符、重复项、大小写别名、file/directory 冲突。USTAR 自身长度容纳不了的逻辑路径在此包子集拒绝。
- header checksum 必须正确，数值是规范 ASCII 八进制；mode 固定 0644，uid/gid/mtime 固定 0，uname/gname/linkname/devmajor/devminor 固定空或规范零，未用字节为零。entry 按完整路径 ASCII 字节升序。
- data 长度严格等于 header size，补至 512 倍数的 padding 全零；最后恰好两个全零 512-byte block，拒绝尾随内容/额外归档。整个包≤133 MiB，解包文件预算另按 §2，不能靠压缩比绕预算。
- 仅在私有 staging 中使用 no-follow/create-new 写普通文件，不以系统 `tar -xf` 替代上述 admission；不恢复 archive owner/mode。后来需要可执行 Wrapper 的权限须来自对应固定配方，不能从任意模板 mode 获权；本首片尚不交付 Wrapper。

USTAR 只作为明确容器，压缩格式/目录包/自定义下载 hook 均未纳入本次可执行值。后续增加容器属于格式准入扩展，不能隐式接受。

### 4.3 获取与冻结次序

1. 解析 selector 到受信固定目录记录，保存该记录与执行 recipe 镜像身份。拒绝用户任意 URL、shell、镜像路径替换和未锁的版本求解。
2. bundled 或缓存命中时，重新打开 no-follow 普通文件并按目录的 size/SHA 完整校验；缓存名只定位，不代表信任。offline 未命中为 Host/3。dry-run 仅用已可信本地/bundled 完整包，不下载、不解包落地工程、不启动进程；缺本地完整字节不能给出成功 dry-run。dry-run仅允许保存本次报告证据；这不创建用户工程或启动子任务。
3. 普通在线请求可下载目录中的**精确 HTTPS URL**，拒绝重定向、非 HTTPS、超 size、截断、额外字节和 digest 不符；不下载系统 SDK、不接受许可证。下载到 create-new 私有文件，边读有界计数，完整校验后才原子写入按 digest 命名的缓存；已存在缓存先验字节，绝不覆盖冲突内容。
4. 校验原包后严格解析 header；在 staging 完整提取全部库存；核对 manifest 原字节、strict JSON、所有 files 的 SHA/size、全部路径及声明引用闭包。**未选中 profile/文件仍须通过完整包库存校验**；只有已选执行闭包必须使用受支持 decoder/配方。不得把未知格式内容猜成已知 SDK。
5. 对解析后的 checked template、选中 profile 原字节、已解析参数、源码资产和 recipe/tool pins 冻结。沿现有 FrozenInputs/TaskGraph/C04/E02 接口携带实际字节，不拿当前路径字符串代替身份。SDK 由 profile.sdk + 已知配方实际验证；profile/2 调既有 v2 composition 校验，原字节根关系不变。
6. 在渲染前、CommitNew 前、每个使用外部文件/工具的任务启动前以及成功结果定稿前复核其实际冻结输入；实际读取使用同一受控句柄或私有不可变副本，避免 hash 后重新按可替换路径打开。SDK/工具的现有闭包重验不降低。首次取得内容与预期 pin 不符为 InvalidInput/2；**冻结后**输入被替换/漂移为 Conflict/4。重验不能号称文件系统原子快照。
7. 证据根保存完整原包到 template_package.path、完整解包根到 input_root、实际诊断/日志；结果只引用真正保存成功的 File。成功必须保存所有必需证据；失败保存不完整时保持真实 nullable 字段并输出保存诊断，不能指向不存在文件。

E02 的包源与运行镜像身份仍各自独立；不会拿 archive SHA 代替 recipe executable SHA，也不会拿已下载版本文字代替实际 image hash。模板原包与 recipe 固定来源随 provenance 保存；受管 tool 下载安装属于 I02/I03 的对应实施，不因本格式批准而宣称已存在。

## 5 资产与有限 renderer

asset.module 精确指 owner，output 固定相对最终根并在 owner 目录内；ownership 仅现有 user/managed/local-state/provenance。非 local-state 的 source 必须 File 且在完整库存内；local-state 必须 source=null、renderer=null，仅预留该普通文件路径，**不创建空占位**。目录由模块计划和文件父目录导出，不把目录伪装 File。

三种 renderer 只接受下述字段：

| renderer | 精确输入与确定输出 |
|---|---|
| `{id:'madb.render.copy@1'}` | 原字节复制，任何 token 都不解释 |
| `{id:'madb.render.json-values@1',bindings:[{parameter,pointer}]}` | source 必须通过 §2 strict JSON；RFC6901 pointer 只到已经存在的标量/null，不建 key、不追加 array。解码后的目标地址必须唯一且不能祖先/后代重叠；对象键按原样查找，数组 index 为规范非负十进制，禁止 `-`。参数以 JSON 值替换，不拼字符串；输出 canonical_json + LF。输出若是已登记 MadB 文档，再用其已有 decoder/语义校验 |
| `{id:'madb.render.identifiers@1',language,parameters}` | language 仅 c/toml/cmake；parameters 为非空唯一参数 Id 集合。本版本仅绑定上述 ASCII identifier 字符集的 string 参数，不是通用文本模板。source 必须 UTF-8；唯一标记为精确 `@@MADB:<id>@@`，id 必须在列表中、每个列出的 id 至少出现一次，重复出现可一致替换。未知/未闭合/嵌套标记拒绝；替换后不得残留 `@@MADB:`。值不能含引号、空白、反斜线、`$`、分号、换行或其他语法字节。**一次扫描只识别源标记**，绝不对插入值再次解析；参数不能改路径/工具/命令/env |

标识符 renderer 的安全范围来自字符集和固定资产上下文，不声称任意位置替换都合法。首配方只允许 `name` 在 Cargo package 名称的 TOML 双引号值和 CMake project 的单个 identifier；只允许 namespace 在 C include basename 的双引号字符串和 C identifier 前缀。模板入库审查绑定这些资产原字节；配方执行时按受信目录包 SHA/清单确认，而不是接受任意 token 标记“自称同语境”。结构检查必须再检查实际 Cargo.toml、CMakeLists 和普通 C 文件满足配方布局；真正 C/TOML/CMake 工具解析在相应 build 阶段留证。不能把静态渲染检查写成已编译。

首配方不需要泛化 Kotlin/XML/Swift 字符串 escaping；这些仍为后续明确配方，不允许用此 identifier renderer 处理一般字符串。`name='x;execute_process(...)'`、namespace 带引号/换行等均在参数检查拒绝；不能以“信任模板”接受用户参数注入。模板本身可含源码/CMake 构建描述；这不赋予 manifest 任意 argv/shell hook，也不宣称能阻止作者自己源码里的所有副作用。

## 6 首个有限执行分片

### 6.1 身份与精确支持表

recipe `madb.init.rust-c-sdk.values@1`；template `org.madb.template/rust-c-sdk` 的 `macos-arm64-values` 分片。首片模块 role 只识别 project/rust-library/c-consumer/generated-sdk；name/namespace 两参数；一条 RUST-C-ABI/c11/native-values 边界；backend_config=null；普通 Rust 作者在 `rust/src/lib.rs`，Cargo.toml 无外部依赖或 build.rs，C 消费者 `consumer/main.c`，根 CMakeLists。Cargo 工程提供原生编辑/结构；MadB 生成仍由现有 syn+rustc 见证、双 cbindgen、Clang AST/layout/常量与 C04 接线完成，不用另一个“初始化 FFI 后端”。

| 条目 | 首片明确值/前提 | 真实现状与退出边界 |
|---|---|---|
| 宿主/目标 | 本设计以实际观察 macOS26.6.2/aarch64 为首组合，OS筛选 `[26.6.2,26.6.3)`；目标aarch64-apple-darwin、deployment14.0；不交叉编译 | 既有 native-values 有此目标证据；**init 配方未实现**，不能推广 x86_64/Linux/iOS/OHOS |
| Rust/C 边界 | 已批准有限值配方，不含资源/async/取消；首模板 Reading(i32,String) + normalize | 重用既有普通 C helper/bootstrap 与生成器；不改变 ABI/ContractIR；不代替 C05 资源接口 |
| Rust/生成工具 | Rust1.99.1，bindgen-cli0.73.2，cbindgen0.29.4；源归档与安装镜像分开锁 | 仓库 toolchain.json 已锁版本/源摘要；模板发布时仍须具备当前 host 的精确镜像/SDK 闭包和 MadB/reader/finalizer pins |
| Apple 工具/SDK | Xcode27.0 build27A266a；SDK27.0 build26A425；Apple Clang21.0.0(clang-2100.3.34.2) | 只按既有工具描述与实际完整观察准入；版本字符串匹配不足以替代 SHA/size；不安装 Xcode或接受许可证 |
| CMake/Ninja | 技术栈已选 CMake4.4.4 + Ninja1.13.2，使用 Ninja generator | **尚无本 recipe 的已冻结安装镜像/归档锁、实际执行证据**。本机看到的 CMake4.4.2 不满足，不能回退。登记可执行 recipe 前必须补固定正式包与实际镜像 pins |
| profile | 原 profile/1 字节 + sdk 闭包保持；tools/SDK 实际观察须与其一致 | 现有 `_public_observation` 去掉本机绝对路径，但这不是 generic SDK decoder。prepare 需适配现有 pipeline，使受检原 profile 是权威并精确核对，不能沿旧 helper 再生成一个不同 profile 偷换 |
| 构建 | 先在最终根按 C04 生成/编译 SDK candidate，再 CMake 配置/链接普通 C executable | CMake 是消费工程，现有边界 build_system仍 rustc-clang，不把 profile 改成 cmake，不把库编译当 consumer link |
| Run | 独立子进程执行普通 consumer；初始预期 stdout 精确 `7:x\n`、exit0 | 消费者只用普通头/helper；运行前绑定生成 candidate身份。Lab独立预期不导入产品私有 assertion；真实运行尚无 |
| IDE/Wrapper/调试 | 此分片 ide_entries=[]，--open 任意值明确 InvalidInput | 不声称 INIT-02/05/09；这些仍是 rust-c-sdk 最终承诺与 I02/I03 适用退出项，不以“不适用”删除整包要求 |
| 用户改动 | 作者自然值子集内修改实现、增删改名按现有 reader/生成链重跑；用户 C 代码需随新增 API 正常更新 | 新特征/工具/profile变化按原校验/lock流程；不能静默放宽 profile，不能将一次初始 normalize 演示当作 INIT-04 已通过 |

设计示例引用了真实已冻候选的原 profile/sdk 字节，**仅用于 FileRef 根/原字节示例**；这不是可发布的 init 工具目录，也不证明新 recipe 的 pins 已齐。可执行登记必须绑定实际正式 MadB/recipe 镜像以及精确 CMake/Ninja pins；在此之前 build/run 应明确 blocked/非零，不产生推荐资格。OS区间只为这个待验证首组合提供精确准入筛选，不承诺其它macOS版本；既有工具的版本锁来源与未来模板认证范围分别记录。首recipe只准入profile/1；模板通用decoder可以识别profile/2，但本recipe遇到v2须明确unsupported，不自动降级或声称已完成组合接线。

### 6.2 固定初始输出与有限任务

首recipe除所选assets外，额外初始provenance输出只有：`.madb/template-package.tar` 保存原包逐字节副本，以及 `.madb/template-input/<包内path>` 保存完整解包库存含manifest的逐字节副本。两者都归根模块，必须显式加入同一初始ProjectPlan/RenderedFileSet；模板资产与这些路径或其前缀冲突即拒绝，不能提交后再暗中补文件。发布目录解析记录与recipe来源的诊断证据保存在报告附件，不作为另一个可回放的公共lock格式；其可信来源仍是 result.tool 对应的受检安装目录，未来真实边界lock继续用既有lock/1绑定其实际inputs/tools/components。

报告证据根由应用的证据保存端口管理，处于用户最终根之外的独立本次运行目录，避免把有条件才能生成的报告/日志伪装成CommitNew前已存在文件；结果文件固定保存为该根下的 `init-result.json`；stdout JSON 通过下述 `evidence_root` 明确告知根，文本输出给出同一结果文件绝对路径。已创建工程保留上述可查provenance，报告失败不删除工程。首片不增加新的CLI路径选项，也不在作者工程中隐式覆盖同名报告文件。

### 6.3 有限参数，拒绝任意命令

根 recipe 从 checked template/profile、冻结作者、受检 name/namespace、最终根句柄及实际工具观察构造内部类型化任务；模板不能提供 argv/env。从现有应用内部端口接入 prepare→metadata/generate→build→package 的任务与证据，不递归调用 app build 用例，也不造摘要为零的 BuildTask。

外部 CMake/Ninja 部分仅如下固定形式，所有大括号都来自已验证内部值，各为**单独 argv 元素**，无 shell 拼接：

```
<CMAKE> -S <FINAL_ROOT> -B <FINAL_ROOT/.madb/build/cmake> -G Ninja
  -DCMAKE_MAKE_PROGRAM:FILEPATH=<NINJA>
  -DCMAKE_C_COMPILER:FILEPATH=<LOCKED_CLANG>
  -DCMAKE_OSX_SYSROOT:PATH=<VERIFIED_SDK_ROOT>
  -DCMAKE_OSX_DEPLOYMENT_TARGET:STRING=14.0
  -DCMAKE_BUILD_TYPE:STRING=Debug
  -DMADB_CANDIDATE_ROOT:PATH=<VERIFIED_CURRENT_CANDIDATE>
  -DMADB_LIBRARY_FILE:FILEPATH=<VERIFIED_EXACT_CANDIDATE_LIBRARY>
<CMAKE> --build <FINAL_ROOT/.madb/build/cmake> --target madb_demo --parallel 1
<FINAL_ROOT/.madb/build/cmake/madb_demo>
```

这些是待实现的执行表，不是 template 的 commands 字段。工具路径由受管锁/Host 定位；当前目录固定最终根；环境从现有 ProcessPolicy 的显式白名单构造并清空继承环境，不能接收任意 CFLAGS/CMAKE_PREFIX_PATH/动态库搜索注入。规范化逻辑子路径固定，实际输出根/SDK 路径可以含空格中文；CMake 参数传递及嵌入变量由固定 renderer/adapter 单独处理，必须实测，不能假定 argv 安全就代表 CMake 字符串解析已安全。RPATH/动态库实际路径由此固定配方与 target执行检验，不依赖作者 shell 的 DYLD_LIBRARY_PATH。

相同目标的一条 required 检查链：

| id | 请求达到的级别 | 最低证据 | 必须独立得到的事实 |
|---|---|---|---|
| structure | structure/build/run | static | 参数/完整包/profile/计划/渲染核对；真实 CommitNew；提交后初始源文件身份与完整计划相同 |
| native-generate | build/run | generated | 原作者受检，真实 metadata/生成/物理 ABI/责任gate 与 Source身份；附现有证据，不能只说生成目录存在 |
| c-link | build/run | linked | 原 SDK candidate 验证 + CMake实际配置/消费者编译和链接；绑定此candidate和最终根 |
| ordinary-run | run | host-run | 最终根中实际执行普通消费者，独立预期值/退出码/cleanup；不是 producer 自写 passed |

required 集由**内置配方+所选目标+请求等级**独立推导，再与 template 声明交叉核对；模板省项或将 required 改 false 是 InvalidInput。已选额外 required 声明也必须执行，不允许因为内置最小集通过就忽略。每项结果键为 (id,target)；仅此集合全部 passed 才可能成功。未请求级别的检查不生成伪 skipped 条目；open 对应实际请求另判。缺 CMake/Ninja/SDK 时 structure 可以在自身条件齐备后实际创建；默认 build 的合法请求必须在提交前 preflight 显示缺项并失败，不用 structure偷偷冒充 build。

## 7 InitResult/1 与事实驱动成功

顶层精确字段：

```
{schema,request_digest,tool,template,template_package,evidence_root,template_profile,
 boundary_profiles,request,output,plan,creation,termination,exit_code,
 checks,ide_entries,open,recovery,diagnostics,logs}
```

| 字段 | 类型和精确语义 |
|---|---|
| schema | 常量 madb.init-result/1 |
| request_digest | Digest 或 null，直接采用 A 的完整投影/错误类别规则 |
| tool | Component；实际运行工具身份，示例中的 design-model 不是生产 MadB |
| template | null 或**已批准的五字段** `{id,version,manifest:FileRef,package_sha256:Digest,input_root:Path}` |
| template_package | 新增 File 或 null；真实原归档证据，sha256 必须等于 template.package_sha256；template 非 null 时本字段非 null，反向亦然；失败保存不齐不能留下半真引用 |
| evidence_root | 新增 null 或 `{path,redacted}`；未脱敏 path 为实际已创建报告证据根的正规化绝对路径，redacted=true 时精确 `<evidence-root>`。存在任一 File/FileRef 引用时必须非null；未脱敏 stdout JSON可据此解析相对引用。脱敏导出须随结果文件/证据目录提供，不能当独立可重放文档；已知实际根不进入request_digest。普通与dry-run成功均须实际保存该根下固定 `init-result.json`；根/保存创建失败时按真实事实返回诊断、null/已存在字段，不捏造FileRef |
| template_profile | 选中 template profile Id 或 null；不是 boundary profile Id |
| boundary_profiles | 选中边界 FileRef 集，原字节存于 input_root，外层路径加前缀；排序 path 唯一；未解析时空 |
| request | null 或 `{targets,verify,open,dry_run,offline}`；targets 为解析后的有序唯一 family；verify为 structure/build/run；open为 IDE Id或null；两个flag为bool。展示子集不取代完整请求 |
| output | null 或 `{path,redacted}`；path 是正规化绝对最终根字符串，或 redacted=true 时精确 `<project-root>`；不是common相对Path。保留原request_digest，脱敏结果不可当可重放请求 |
| plan | null 或 `{files,task_roots,downloads}`；files 精确行 `{module,path,ownership,sha256,size}`，路径升序唯一；非local-state有实际渲染字节SHA/size；local-state两者null且不落地。task_roots为所选Id集合；downloads见下文 |
| creation | not-created / created；来自内部 Host 提交事实，不从 plan/checks 推导 |
| termination | completed / failed / cancelled |
| exit_code | 0/1/2/3/4；原ErrorKind映射不变；cancelled为1，不另加130 |
| checks | 下述检查行，(id,target)唯一 |
| ide_entries | 已选完整IDE入口行（与受检模板相同），无则[]；不是已import证明 |
| open | `{requested,status,diagnostics,attachments}`；request非null时requested等于request.open，request=null时requested=null；status=passed/failed/error/skipped/unsupported/blocked；未请求必须null/skipped；只证明启动，不等于import/debug |
| recovery | `{kind,message,arguments}`数组；kind=retry/manual/inspect，message非空≤8192，arguments为argv数组或null。仅建议，不自动执行；已created时不得建议重新init覆盖，而应指向既有工程任务 |
| diagnostics | `{code,message,location}`数组；code为现有注册诊断Id，message非空≤8192，location为common位置或null；不能把未注册新码偷偷当已有explain目录 |
| logs | 已保存实际文件的File数组，路径唯一，直接相对证据根 |

`plan.downloads[] = {id,version,source,sha256,size}`，id唯一，source精确复用§4的封闭对象 `{kind:'bundled'}` 或 `{kind:'https',url}`，URL约束相同；version为common.component的非空≤128字符字符串，size为common.safe_uint；它是已锁对象的计划，不宣称已下载。离线失败可保留缺项；dry-run成功只有可信本地完整输入，downloads=[]。构建输出目录/未来C04生成内容不混进**初始 CommitNew source-plan**：该计划必须完整列出所有本次要创建的作者/工程/provenance文件及local-state预留，后续SDK/构建输出由既有 C04 的独立输出计划与 ownership 检查管理。首配方显式保留 `.madb/generated` 模块目录，其初始无文件合法；不能把未知未来输出 SHA 填零。

检查行精确为：

```
{id,level,target,status,requirement,environment,command,evidence_level,
 cleanup,diagnostics,attachments,reason}
```

`checks.environment.items` **精确复用当前 `report.schema.json` 的 environment.items 规则**：对象仅有且必须有 `{id,version,sha256,host,target,source,arguments}`；id/host/target 使用 common.Id，sha256 使用 common.Digest，version 为长度1…128的字符串，source 为长度1…4096的字符串，arguments 为0…4096项数组、每项为长度0…4096的字符串。source 是观察记录中的字符串，**不是**模板目录或 plan.downloads.source 的判别对象；不增加第二种观察来源形状。此处外层 environment 数组为0…4096项，允许静态内建检查为空；实际执行检查仍须携带下文要求的相关工具观察。

`checks.command` 与 `recovery[].arguments` 均为 null 或 argv：非null时必须是1…4096项的非空数组，每项沿上述参数字符串边界为长度0…4096的字符串；空字符串参数可保留，首项命令名必须非空。argv 各元素不拼成 shell 字符串；command 记录真实调用，recovery.arguments 仅为建议，不获得执行授权。实际执行仍需通过现有 ProcessPolicy 的额外字节预算、无NUL和环境白名单检查，wire数量/字符串界限不代替执行准入。

level/evidence语义同模板；status用上述六状态；requirement=required/optional/not-applicable；environment为上述精确形状的实际工具观察数组；command为上述真实单个argv或null，复合检查不能捏造一条“聚合命令”，须附其每个真实子任务证据；evidence_level为枚举或null，未运行不可填编译/运行证据；cleanup=complete/failed/not-run；attachments为实际File集合；reason为非空字符串或null。not-applicable、skipped、unsupported、blocked必须有具体reason。静态内建检查可environment=[]、command=null、cleanup=not-run；实际执行项必须有相关工具镜像与非空证据附件，子进程未正确收尾不能passed。environment.item精确复用上述规则；检查行整体**不是现成 report/CaseResult DTO**，不伪造candidate/Lab revision，也不把build-evidence三个状态直接挪作init六状态。

### 7.1 请求摘要和失败映射：直接落实已批 A

不新增第二种 InitRequest wire。内部冻结投影精确为：

```
{template_selector,template_profile_selector,parameter_values,targets,
 verify,open,dry_run,offline,output_path}
```

selector 是用户明确selector字符串或null，parameter_values为已展开合法默认的完整参数对象，targets为按D解析的完整排序唯一family；输出根在hash前作纯词法绝对规范化，不以“当前不存在目录”的realpath猜路径。digest为 `SHA256(UTF8('madb.init-request/1\n') || canonical_json(projection))`；这个域和envelope属于本待定格式，不冒称现有公开decoder已经支持。摘要只绑定请求投影；真正执行还必须同时绑定template_package/profile/tool/source身份，不能从request_digest推断实际输入包身份。

null摘要可能是合法请求在离线缺包时尚无法展开default/profile，也可能是非法请求；两者不混为一种错误。现有分类：非法selector/格式/首次取得内容不符pin/不支持组合→InvalidInput2；缓存/工具/SDK/I/O缺失→Host3；目标已存在、提交竞争、冻结后漂移→Conflict4；真实检查不通过→CheckFailed1；取消→1。main使用实际typed stage error，不按blocked数组文字重算“优先级”。摘要一旦形成，后续任何失败保留。

### 7.2 CommitNew 的内部事实，不增加一个公共任务系统

应用从**同一**checked ProjectPlan 与实际 RenderedFileSet获得完整初始源清单，在最终父目录的私有同卷staging内创建并验字节；最后以 Host 的 no-replace 原子目录提交原语执行 `CommitNew(final_parent_handle,final_basename,checked_plan,rendered_inventory)`。现有 C04 BatchStore/CommitReceipt 的 Source/Build/Candidate 索引提交不具备“创建最终用户根”的事实，不能改名字当完成；应增加窄 Host 操作并保存内部 typed receipt，不新增公开schema或任务系统。

receipt至少在内部绑定：实际最终根身份、精确完整初始文件清单及 owner、这次 no-overwrite 是否成功。仅有目标目录存在不证明是本次创建。竞争者创建文件/目录/符号链接均Conflict4且不覆盖；无可用原子no-replace能力的平台明确拒绝，禁止 exists-check + rename-overwrite。此处涉及真实并发，实施后须独立复核与竞争注入。

成功的提交后，再在**最终根**验证源清单并运行需要最终路径的生成/配置/构建/运行；不能临时目录里编译通过再移动就声称最终根通过。开始build前的工具/SDK缺项应在preflight阻止创建；提交后才出现的失败/漂移/取消则 `created`、非零退出、保留工程、实际cleanup记录、可执行恢复建议，绝不回滚删除用户目录。

普通0退出必须：非null request/digest/template/template_package/evidence_root/template_profile/output/plan；冻结请求dry_run=false；termination=completed；有同根同计划本次CommitNew receipt； independently-derived required检查全部passed且证据层级足够；open与cleanup适用条件满足；必需证据保存成功。JSON本身可伪造声明，因此应用成功判定必须接收冻结请求/受检计划/Host事实而非只接受一个外部InitResult对象。

dry-run0退出必须：冻结请求dry_run=true；可信本地完整包/参数/profile/静态计划可解；证据根与 `init-result.json` 实际保存成功；not-created，completed；非null计划；checks=[]；open skipped且不启动（即使请求open，也保持requested与请求一致，reason由顶层诊断说明dry-run未启动）；不下载、不创建工程、不调用外部工具。可以在内存确定性渲染并给出计划SHA；不能把这称为普通成功或实际工具检查。

## 8 完整设计实例与反例

完整字节在 [示例目录](/private/tmp/madb-chief-round7/i01-final-format-examples/index.json)，不是生产模板/decoder/验收证据。生成脚本 [write-i01-design-examples.py](/private/tmp/madb-chief-round7/write-i01-design-examples.py) 只是构造设计材料，不应复制到产品当新流水线。

- [完整manifest](/private/tmp/madb-chief-round7/i01-final-format-examples/package-input/meta/template.json)：嵌套manifest、包根的原profile/sdk、六个可渲染资产与一个local-state预留；完整包9文件，inventory不含manifest自身。
- [原归档](/private/tmp/madb-chief-round7/i01-final-format-examples/template-design.tar)：实际43008字节，SHA-256 `2a47930bdb3f9298fc055469c8436f019fc648805dbdd3abcb94d78603d0a17b`。这个hash真实，只绑定设计包，不证明可执行工具锁齐备。
- [dry-run完整实例](/private/tmp/madb-chief-round7/i01-final-format-examples/vectors/positive-dry-run.json)：中文/空格绝对输出根、显式合法name、已批完整profile选择、原包/根引用、实际可渲染字节清单；其FileRef在实际设计证据根（原路径：`/private/tmp/madb-chief-round7/i01-final-format-examples/evidence-root`）中有对应字节；这些只是模型资产，不证明工程创建。其摘要可由[完整内部投影](/private/tmp/madb-chief-round7/i01-final-format-examples/vectors/positive-request-projection.json)重算。
- [普通创建成功的状态模型](/private/tmp/madb-chief-round7/i01-final-format-examples/vectors/positive-created-model.json) 与 [独立事实前提](/private/tmp/madb-chief-round7/i01-final-format-examples/vectors/positive-created-model-facts.json)：明确 `actual:false/assumed_for_model:true`，不曾执行CommitNew。仅用于说明接到真实Host事实后如何构造正常structure结果，不能登记pass。
- [离线缺包完整失败实例](/private/tmp/madb-chief-round7/i01-final-format-examples/vectors/positive-offline-cache-miss.json)：request_digest=null、Host3；这是A要求的合法请求受阻模型，不是InvalidInput。

每个JSON反例都是完整独立文件，不是需读者脑补的patch；共同预置事实/原包是本节设计正例。预期如下：

| 完整反例文件（vectors/） | 应拒绝的位置/原因 |
|---|---|
| negative-unknown-template-field.json | 模板严格对象：postCreate未知 |
| negative-arbitrary-recipe-argv.json | task_root未知argv，不执行shell |
| negative-unknown-renderer.json | 有限renderer未知，不能退回copy |
| negative-parameter-injection.json | namespace默认违反有限标识符语义 |
| negative-double-sdk-source.json | requirements.sdks未知，不能复活C已删除的第二SDK权威 |
| negative-inventory-omission.json | 包实际含SDK文件而files漏项；完整库存集合不等 |
| negative-parent-relative-root.json | 包内FileRef路径含..，不能以manifest父目录修复 |
| negative-json-pointer-overlap.json | 同一解析节点被重复绑定 |
| negative-success-without-created.json | 冻结请求不是dry-run，却not-created/exit0 |
| negative-missing-required-check.json | 内置structure要求独立存在，空checks不能真空成功 |
| negative-duplicate-check-key.json | (id,target)重复 |
| negative-wrong-package-sha.json | result.template.package_sha256与实际template_package不等 |
| negative-open-request-mismatch.json | request.open与open.requested不等 |
| negative-template-root-collapsed.json | input_root指manifest父目录，原包根与闭包引用不成立 |
| negative-json-fraction/negative-zero/duplicate-key.json | strict JSON词法/重复键先拒绝，不当普通JSON数值处理 |

`negative-package-byte-mutation.tar` 为原归档单字节改变，应在目录锁SHA校验拒绝；`negative-package-unlisted-file.tar` 有完整额外entry，即使调用方给它新包SHA，仍在manifest.files精确库存拒绝。失败注入的执行向量还必须在实施阶段独立覆盖：归档链接/路径穿越/截断/size超限/混合大小写；已冻结资产被换；提交竞争与不支持no-replace；成功后保存证据失败；build子进程取消；已created后失败保持用户工程。设计反例不是这些真实Host验证已经执行的证据。

## 9 实施工作包与收口门槛

批准本提案后可分三片串接，root统一维护共享格式/目录，独立文件职责可并行：

1. **I01-F 格式与纯规则**：两schema/decoder、完整包inventory模型/选择/参数/renderer/跨文档校验/独立成功谓词；直接复用现有ProjectPlan及D，不重写它。导入设计正反向量但先由独立审查纠正，生产校验器须自己执行；补fixture摘要与profile/1、profile/2闭包反例。此片可以独立implemented子范围，不能整包passed。
2. **I01-P 包与创建**：窄目录/获取/归档/Workspace Host端口，实际输入冻结/取包/原字节保存、CommitNew和恢复；no-overwrite并发原生路径独立复核。普通structure、dry-run、offline、失败保留、空格/中文最终根必须真实运行。绝不把新增单元测试数量当Host完成。
3. **I01-R 首Rust/C分片**：先补真实固定CMake/Ninja/recipe镜像/SDK来源 pins 与发布目录，再适配既有native-values C04 pipeline接收受检原profile，接最终根CMake消费、链接和Run；Lab独立消费同候选、独立预期，并核自然修改/新增普通导出。未齐工具或执行条件如实阻断该片，不阻止格式/纯规则施工。

I02 doctor/环境修复、I03 Wrapper/工程任务接线仍按既定目录依赖推进，不把本提案里的模板requirements或suggested recovery变成另一套doctor。profile/2+E02组合执行保持现有精确身份，不因template decoder能读/2就宣称任意composition可运行。C05资源接口的init配方、IDE与其他路线需按原目标补齐，**I01最终退出仍以catalog/验证规范为准，本提案未改任何验收条件**。

执行日志/历史报告保持原结论；采用状态由主协调写决定/目录，HTML仍源模板生成。本材料仅支持决定审查，不将C06/C07、L02原候选认证转授给init，也不继承其他平台发布资格。

## 10 依据与独立审查

实际宿主探针 `sw_vers -productVersion` 返回26.6.2；仅用于设计首组合筛选，不是init执行证明。实际读取：两仓AGENTS；`docs/catalog/implementation-plan.json`；ADR-0019；第六批报告及i01验证/审查JSON；project-init-decision及独审；旧格式草案/审查；product/04、architecture/14、validation/06；common/profile/profile-v2/lock/report/execution schemas；strict_json/canonical/project-selection/ProjectPlan/FrozenInputs/ProcessPolicy/transaction；Rust/C toolchain与_public_observation。精确实现差异已整理于 [只读范围审计](/private/tmp/madb-chief-round7/i01-next-scope.md)。

第三方依据只用于固定版本与格式基础，不视为MadB执行证据：[CMake 4.4.4正式发布](https://discourse.cmake.org/t/cmake-4-4-4-available-for-download/15862)、[CMake 4.4版本的cmake(1)](https://cmake.org/cmake/help/v4.4/manual/cmake.1.html)、[Ninja v1.13.2固定发布](https://github.com/ninja-build/ninja/releases/tag/v1.13.2)、[GNU tar 1.35格式文档](https://www.gnu.org/s/tar/manual/html_chapter/Formats.html)。本提案自定更窄USTAR admission，不从“tar兼容”推导任意实现安全解包。

独立审查结论另存 `i01-final-format-independent-review.md`，审查完成前不把本文件视为已定稿批准。所有剩余字段/新recipe值由主协调合并进入既有决定流程，不向用户重复询问A–D。
