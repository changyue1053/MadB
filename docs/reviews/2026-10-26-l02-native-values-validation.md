# L02 自然 Rust 值配方：冻结候选与收包对账

任务为 L02 声明复用与诊断基础，依据 ADR-0020/0021/0022、DX-01/02/11 和既有 C04/C09 首链分工。新产品源码固定为 clean `e0f74a5fa5a9a6e73400ced70bc81ba06751227f`，独立 Lab 固定为 clean `2d06cbefa0f49187f163ce937c8c3bd01a062eb1`。本报告补充第二独立业务及集成验证；[第六批正式报告](2026-10-26-l02-frozen-candidate-validation.md)与[第七批开工/实现记录](2026-10-26-chief-architect-seventh-batch.md)保留各自当时的结论。状态只在 catalog 维护。

## 实际实现与普通消费

作者只维护自然 Rust 模块。纯 syn 读取和锁定 rustc 见证确认有限声明，随后生成 shim，执行真实 cbindgen 双头与 Clang ABI/布局/常量检查，形成受检 IR、有效策略、逐符号责任表和 Source/Build/Candidate。没有编辑固定业务映射表或生成产物来适配五步变更。

有限配方为1–16个公开具名字段，字段限 i32/u32/String；String 边界为64字节严格 UTF-8。同步函数返回原记录或明确标签的 Result。普通 C 入口自动核对同次生成的 bootstrap/interface/profile 身份，再执行一次 raw 调用和结果协议检查。输入验证通过后进行一次拥有值转换，作者函数最多调用一次；身份或输入拒绝时不调用作者。raw 对满足有效内存前提且通过 NULL/对齐检查的输出分别初始化；错误时记录值输出保持零，业务 Result 错误另保留 domain/tag。纯值不添加资源、订阅、排水或异步承诺，旧普通/测试伴随/诊断/诊断伴随的7/14/9/16个 native 导出及诊断 ABI 保持原契约。

两个阶段的责任独审按各自字节范围组合：旧 gate 独审核对责任完整性与项目/作者绑定，最终 reader 追加独审覆盖合法标量同名类型及 std 名称；集成后8项 ownership 与17项 reader 检查通过。不把早期审查中的依赖摘要称为最终提交的全部字节。原生 panic 恢复以支持 unwind 的构建为前提；abort/OOM abort 不宣称可恢复。原生独审发现并修复 panic payload 析构再次 panic：可恢复清理返回8且清零、分配回到基线；不可恢复的再次析构 panic 明确终止进程，不以泄漏或递归假装恢复。

## 生成、事务与本地门禁

最终 clean 产品完整 `scripts/check.py` 通过，Lab 完整621项门禁通过。产品早期 clippy、受限沙箱 Unix socket、Python 测试导入路径失败原记录保留，修复或以正常本机权限重跑后才使用最终结果。

最终 clean 产品另执行2项 live 事务测试：同一受管根、boundary/profile ID/target/namespace 完成作者0→增方法→改字段→重命名→删除五步实际生成和候选提交。新增 Result 依法改变 profile digest，不声称五步共享不可变 profile。冻结作者漂移、未知/冲突策略、隐藏编译输入、缺失责任资产等准确拒绝并保留旧 Candidate/current；执行前后产品、Lab 和工具摘要一致。

## macOS 正式候选与目标结果

目标为 aarch64-apple-darwin，Rust 1.99.1、cbindgen 0.29.4、bindgen 0.73.2、Apple Clang 21、Mac SDK 27.0。七个新值候选均来自上述 clean 产品，Lab 仅安装公开包并使用独立预期。

| 配置 | candidate digest（SHA-256） |
|---|---|
| author-0 | `5bb7a82502c42b1704ca188eb3a7ae691ef566b0dcbf1c781e177f3c5f0e8528` |
| author-1 | `5fa4aff72a64dc86f288b48948483eba595fe827e14f7edf38645923a76409d0` |
| author-2 | `732698ef20ec4354586203229395ed65f5394faa4efe0a3fa477ff24407ccb57` |
| author-3 | `e809a02124606f96f4cfdf86f82f9c48cbf6c6a6eebff7509665bbe67a3a2aec` |
| author-4 | `3014713de2871d747f485f385bcded11bcb7c09bd622cccca514ba77eb0e5519` |
| aux-grow | `c45b7c7fedc64a21eac3cb07c2ccf5f92e36226e69d4ebef668b38959f773c5f` |
| aux-panic | `a1cc330c8515866ab7941cf16ef240fff83fef2ef4dc941def390b16f66a40ea` |

七份 report/1 的29项全部通过，分为18项真实 SDK 案例、5项 helper-only、5项静态及1项编译拒绝。实际19次 SDK 进程与5次 stub 进程分列；helper 的116个协议向量不计为真实 Rust SDK 运行。9次指定编译/链接拒绝另有原始证据。两仓实际根集成后，使用相同候选再跑29项通过；这是复验，不增加独立案例数。原始局部观察的 full_l02_acceptance=false 保持不变，整包判断由主协调汇总各子范围证据后在 catalog 维护。

同一新产品另生成旧首链五配置，L02 诊断5项通过（56次 SDK 进程、24次 stub、4次导出观察）；C05 最终19项通过（17项 target-run、1项 compiled、1项 static）。C05 首次因直接 Clang 未设 SDKROOT 而4 error/12 blocked/3 passed，原失败保留；仅补真实 SDKROOT 后在新目录重跑，未改源码、候选或预算。

[Mac 独审](l02-values-macos-review.md)核对12份原候选、11份 values/L02 安装副本、Lab523个源文件、报告附件、七个生产工作区及实际根集成字节。结论附证据限制：旧 C05 Rust→C 两个错配案例的构建日志被同名运行日志覆盖，现存材料支持运行拒绝，不能声称这两份原始构建日志完整保留；三个 C05 临时消费目录已清理。C05 子进程日志也没有独立绝对开始时间，SDKROOT 由外层记录，未归档完整子进程环境；mtime 不充当可信签名时间。后续 C09 修正日志路径须使用新 Lab revision 和新报告，不回写本次或历史结果。

## Linux 正式执行

实际容器固定为 `8552514ab8ea0c225a3cf8858fa67f7fab0f7b721ed73a3026a98c089a8963e8`，镜像 `sha256:4b826b95c4a62ef6be1f5a792519d9a86a235f03459e12d7b01326e9d5a02183`。Ubuntu x86_64 用户态在 aarch64 VM 中由实际 QEMU 7.0.0 执行；`/proc/1/exe --version` 和执行镜像字节另留证，初始未知版本记录没有回填。固定 Rust 1.99.1、Clang 18.1.3、cbindgen 0.29.4、bindgen 0.73.2、Python 3.14.4，不将旧 GNU cc 失败解释为已恢复。

源码、生成、公开候选与消费运行均在大小写敏感的 `/work/l02-values-round7-001`。新 `/cache/round7-values-001` 仅保存编译缓存与归档存储，不作生成/消费执行根。生成、消费和测试子进程保留2 GiB磁盘守卫与原执行预算。首轮 target 整目录符号链接导致 clean 检查拒绝，修正为真实 target 目录及内部 debug 缓存链接；没有改 ignore 规则或绕过检查。首个 author-1 构建触发空间守卫后以 -9 中止，最低2144382976字节；失败命令、日志、空间样本与工作树完整保留。触发值在 command.failure 和 after 中保存，触发时 space() 先抛出，故该次未写入 space.jsonl；不把已有采样称为无缺口的连续记录。

随后仅对本轮已停止的临时工作树逐项记录并校验文件 SHA/大小/模式和链接目标，完整可恢复归档后释放对应临时副本。首次存储迁移读取已停止现场、写入余量超过2 GiB的 /cache；当时源 /work 为2144321536字节，低于守卫，不声称全部归档也在 /work 余量达标后启动。存储迁移释放空间后才重启生成；生成、消费和测试守卫未放宽。七份成功现场与一份失败现场分别保存，公开候选、源码、阶段日志和旧轮次数据保持。author-0 来自原成功执行，其余六个来自新的顺序执行；不把失败 author-1 回填为成功。

七包由主协调通过独立 Lab 公开校验器重算完整性，核对 producer/target/必需案例及实际作者 fixture。外部冻结清单 SHA-256 为 `ef9435bf73b5bac974d2a9bafb3baec32001ca6c6c7d608f7e4d04b6969883d8`，之后才交给正式 verifier。原包 tar SHA-256 为 `6ea4d54428d6d5c6c879bc3d01f02b8039efade9c2a056c5372058347f3a8baa`，45424640字节。

| 配置 | candidate digest（SHA-256） |
|---|---|
| author-0 | `2a8a74b8c41927982c02138b96e4351391979bc3187389d0c7db990e8464e21f` |
| author-1 | `773ecbc70cd48958343132c210637502565dde3df6a04913b825075b1a1140bf` |
| author-2 | `8ece2a00f7c86919cdbe5cf2ec7ec91192f8ed56b14ba479e9f4e1fe020b72e3` |
| author-3 | `a58742787042a0f2aafadf063bded8ef5c45bfaa57c1eafe26891ef20efdfa8c` |
| author-4 | `18111822df18e46e9326db888823ab0922b2add5b0ce49ff6cda81a546882f94` |
| aux-grow | `c42aa5e9070c66539d46b0b2aad2baaee6a433cdba68b0f4624b6346492b6db0` |
| aux-panic | `1d4b879f0afa98ec0cb8bb16351a46e065132a61e4df1ce31a2044ab8eb2d80e` |

正式 Lab 完整仓库门禁与七份 report/1 已通过，29项分级为18项真实 SDK、5项 helper-only、5项 static、1项 compiled；report/1 的23项 target-run 含5项 helper，不扩大为23项真实 SDK。运行前后源码、公开候选、三个 CLI 镜像和 target 布局清单相同。另从相同冻结源码执行4项 codegen 与3项 client 测试，均通过；真实分配失败、Err、unwind、payload 清理与预期 SIGABRT 分列于原生专项，client 属 helper-only。实际 Rust 链接日志、私有 cc→固定 Clang 的镜像和 RLIMIT_CORE=[0,0] 已记录；不宣称 GNU cc 恢复。原生测试不是 report/1，不增加29项公开案例数。client 专项使用临时目录，三项通过有冻结源码与外层日志，但其临时 stub 头、可执行文件和全部子日志未保留；正式 Lab 五个 helper 的日志、安装头及116向量另完整留存。参数名称专项复用源路径，保存8次 rustc 命令与8份 rmeta，不声称每次原始源快照均独立保存。最终[Linux 独审](l02-values-linux-review.md)已完成，无剩余阻断。77项具名工具/runtime/SDK原字节与追加60项正式消费者头文件分别核对，覆盖报告记录的99个外部输入路径。八份工作区归档的8801个条目逐项核验；未实际恢复并运行，不把静态归档核对称为重演。原子日志没有独立可信绝对开始时刻，freeze先后按保留记录核对。

## L02 验收对账

旧 explain、首链资源/回调、64槽生产诊断、关闭采集仍保留保护及自动身份构造，沿用第六批固定候选和独立审查；本轮新产品的 macOS 回归另外绑定。已审的旧 SDK engine、explain 各层/说明目录及 first-route ownership 源码没有漂移；共享 driver/native_ast/门禁增量另由本轮复验覆盖，不用新值配方冒充旧资源协议证据。

| 现行验收 | 对应证据 |
|---|---|
| 声明复用、来源与普通入口 | 真实 Rust 声明/编译器见证/来源位置；五步变更；独立普通 C 消费，自动身份核对 |
| 关闭诊断保留保护 | 第六批两目标正式诊断关闭案例；本轮 macOS 同提交新候选回归 |
| 普通资源/回调与生成诊断 | 第六批资源/回调及诊断候选，静态、运行及替身层级分列 |
| DEC-07 逐符号唯一责任 | 旧七/九角色与新有限值各自从真实来源派生；核心比较、字节闭包及包内责任资产核对 |
| explain 稳定码、来源、修复、只读与出口 | 第六批51码及实际 CLI/受控读取/格式向量；出口4不冒充真实文件系统竞态复现 |
| DX-01/02/11 第二业务 | 本轮五作者变更、真实同根事务、两目标各自普通候选消费及准确拒绝；目标、工具和限制分别绑定 |

## 集成、证据归档与边界

集成前重新核对实际 HEAD、分支、无脏文件与1155/481个基线文件字节；新路径碰撞检查通过才将两仓各自 fast-forward。集成后1186/523个源文件与冻结检出逐字节一致，旧 detached worktree 与他人内容保留。

本次收口综合[既有证据审计](l02-values-prior-evidence-audit.md)、[收包条件快照](l02-values-closure-audit.md)、[Mac 最终独审](l02-values-macos-review.md)与[Linux 最终独审](l02-values-linux-review.md)。条件快照当时仍缺的Linux七候选、29项正式消费、原生专项和独审，现已由上述实际结果补齐；C05/L01依赖已满足，现行L02出口无剩余阻断，状态由catalog据此登记。

持久根为仓库外 `.local/evidence/round7-l02-e0f74a5-2d06cbe`，[归档索引](l02-values-evidence-archive.json)记录原路径映射、各阶段清单及关键证据摘要。三阶段分别为：Mac/source数据32490文件、4237493574字节（另1符号链接），清单 `51252eb5746cc11aa79a7d2e08453d6467775b874a6813651d0ee0e61999e705`；Linux数据730文件、1066606028字节，清单 `97b3068c9988ec5ffd1fe3553db72c71876350e4717b62f6df7034ac9b6607c6`；最终审查/头文件补证/待裁定I01设计材料100文件、25515985字节，清单 `9e4eebd2506a8f836a591eca4479993b562da2fa0f09c9d84072967e2ba2ec6f`。以上计数不含各阶段manifest自身。复制前后逐文件SHA/大小/模式/链接和完整路径核对通过，原目录及失败现场保留；主协调另核三份manifest身份与18组关键源/归档文件。所有完整度限制、失败和重新执行分别保留。Linux 模拟目标不代表物理硬件或性能；未验 Windows/Linux arm64，不继承为任意 C ABI、Swift/ArkTS 或组合认证。L04/L05、I 链、C08/L06、C09/C10 和 L07 保持各自验收责任，未增设 L02 新前置。

产品 `e0f74a5` 与 Lab `2d06cbe` 阶段推送已成功，带 `[skip ci]`；GitHub 远端 refs 核对一致，本次涉及的三个新提交均未观察到 Actions run。没有创建 PR、标签或发布，零运行不是 CI 通过。文档生成/静态/工具测试单列；本轮未重新执行真实浏览器布局或交互验收。

下一轮按当前队列推进 I01 剩余格式及首个有限配方、E01–E03 真实组合注册/执行准入与独立消费，继续 L04/L05 的真实接线和声明生成；L03、I链、C08/L06及C09/C10联合出口保持原依赖。[I01完整提案](i01-final-format-proposal.md)与[独审](i01-final-format-independent-review.md)仅支持剩余格式决定，已批四项不重问，未注册生产decoder或取得init/IDE验收。

文档收口已从catalog/源生成13页HTML，`render_visuals.py --check`、`check_docs.py`、29项文档工具单测及`git diff --check`通过。首轮静态检查拒绝七处目录/带行号链接；仅在新增导航副本中改为文本路径并注明原文SHA，原始提案/独审字节及结论保持归档。未重跑真实浏览器布局/交互，不将文档检查等同于SDK验收。
