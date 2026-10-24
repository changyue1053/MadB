# C05 Rust ↔ C 与独立 Lab 整包交付

任务：C05 整包；依据 C04 提交 `8fb8dbe00b2c9787b5ce9977c1c2524c7f7a0c1c`、C05 acceptance、DEC-03/07/08/10 及 CRIT-03/08/11/12。交付为 [MadB #11](https://github.com/changyue1053/MadB/pull/11) 与 [MadB-Lab #2](https://github.com/changyue1053/MadB-Lab/pull/2)；两者的合入状态以 GitHub 为准。

## 结论与范围

C05 五项验收通过：真实双向声明读取与绑定、C04 冻结端口全流程、原生资源与回调协议、公开候选身份校验，以及独立 Lab 干净消费。实际验收目标为 macOS Arm64 与 Linux x86_64；默认 Release 和 test-support 分别绑定 profile，不推导其他目标或其他语言路线的认证。

生产实现固定在 `bd2fc28dca8e72995ee9bce6330c6b37ad8b557d`，独立 Lab 固定在 `13f0159da92d34746b096c2c07427a897967fe48`。后续 `7e2a8c2545b5d0264da699125b6bde05898d5383` 仅修复既有宿主测试夹具的请求读取竞态，未改变 SDK 或候选生成输入。C09/C10 的 S1 部分由本包提供，全部配置、发布渠道和 S5 验收仍未完成。

## 五项验收对应物

| C05 要求 | 实际交付与证明 |
|---|---|
| 两个直接方向 | C 消费 Rust engine；Rust 通过真实 bindgen 绑定及公开安全包装消费独立 C 实现。默认 Rust 7、测试 Rust 14、C 2 个实际导出分别核对；回调未充当第二方向。 |
| 首链六类资产贯通 | 作者 SDK/语义声明、Clang/cbindgen 元数据与生成代码、正式端口、profile/lock、独立案例及预期、公开候选和重现入口全部连接。 |
| 所有权、关闭与调度 | 单调登记令牌、准入、request/await/release、源端租约、同步回调重入、注册回滚、超时、容量与耗尽通过真实消费者验证。 |
| 身份与失败隔离 | bootstrap 先于业务调用；错 ABI/profile、缺 bootstrap、候选篡改、旧契约摘要、错误布局来源及锁引用被拒绝。诊断 sink 失败不改变资源协议。 |
| 工具与 SDK 分离 | SDK 为独立原生构建单元，不依赖工具私有 crate。Lab 在新工程及空 Cargo 缓存离线编译，仅使用公开候选；Linux CI producer/consumer 使用独立 job/checkout。 |

元数据读取限于首链已知 ABI roles；结构来自真实 Clang AST/cbindgen 产物，策略来自 project/profile，经 `madb-native-finalize` 复用既有规则完成。它不是任意 Rust/C 接口的通用生成器。异步、队列、stream 及未知导出按当前能力拒绝。

观察 v1 仅提供同步初始快照 sequence=1/state=0；测试源驱动同一个真实 source lease/callback 实现。CRIT-11 只登记适用的重入、退出和清理交错，不声称覆盖不存在的消费者队列或环境退出实现。公共 runtime-protocol 记录 4096 个登记项、每引擎 64 个订阅、所有权与线程/排水责任，并经 profile.sdk 摘要绑定。

## 实际验证

- MadB 完整门禁：fmt/check/clippy、核心与执行器测试、C03/C04 回归、10 项 C05 流水线拒绝测试、独立 SDK 检查、架构依赖与文档/HTML 检查通过。
- 锁定 Apple 工具链的 21 项真实 reader/生成测试通过，包括实际 bindgen 包编译、重复生成、双指针深度与编译目标覆盖拒绝。
- SDK default/test-support 分别执行严格 clippy 与 Release 单元测试；缺失/错误编译身份、实际导出与生成头一致性另行验证。
- Lab 54 项独立仓库/验证器测试通过。macOS 与 Linux 各执行 19 个公开场景：C→Rust 7、test-support 7、Rust→C 4、候选完整性 1。
- 正式 macOS 运行使用干净生产者和 Lab 提交，不使用 `--allow-dirty`；4 份 `madb.report/1`、19 份 `madb.case/1` 实例全部通过公共 schema。
- 同一生产提交在两个独立 macOS 临时目录完成全流程，三份候选共 51 个文件逐字节相同。prepare 输出身份由正式 C04 DAG 独立重编验证，不作为业务 oracle 或验收报告。
- Linux 由实际宿主记录单独工具/profile 身份；不能继承 Apple 工具链认证。远程报告的目标为 `x86_64-unknown-linux-gnu`，19 项结果均为 passed。

## 正式 macOS 候选身份

| 候选 | candidate_digest |
|---|---|
| c-to-rust | `sha256:3c6c9039f77adbe3631dd7dc3b4c762d29c1a8d4989ce9d7139e26f38ecbcd38` |
| c-to-rust-test | `sha256:db754915261102d8ef3c9c2fd18862e2cbfccb8ec176392a90af42e89f8a583d` |
| rust-to-c | `sha256:648b150890c8ce904ebbfd262667c56ec033d6273a673075912fde3596758146` |

bootstrap 绑定 interface/profile；候选清单绑定交付文件；provenance 绑定实际源与宿主二进制。candidate_digest 不编入库，避免循环身份。默认生产入口要求干净检出，复制的作者输入逐项与固定 Git 提交核对；开发入口显式标记 provisional，正式 Lab 拒绝。

## 实跑发现并修复的问题

1. Cargo 默认 dylib 的绝对安装路径会让消费者加载原构建目录；改为 `@rpath` 与相对加载路径，Lab 用实际加载模块地址验证安装副本。
2. 当前 macOS dyld 拒绝无 UUID 可执行文件；采用内容 UUID、可复现链接与去除构建目录调试映射，双目录实测字节相同。
3. Apple ar 记录输入成员权限；将受控临时副本规范化后归档，冻结输入的 0400/普通输入的 0644 不再改变输出。
4. 工具必须先验证来源 receipt 和镜像摘要再执行；宿主二进制固定并在每阶段前后核对。
5. 生成读取保留指针层级，封闭编译参数集合，拒绝双指针误读及参数覆盖目标。
6. Lab 独立重算 source/source-id/shape/structure/policy/profile/project 身份，核对公开内层引用及布局来源，不能只接受外层清单的自报摘要。
7. CI 暴露旧 artifact fixture 未读 stdin 就返回的竞态；夹具先完整校验请求，增加 256 KiB 回归及连续运行验证。生产 StdinClosed 失败语义保持不变。

## 重现与证据入口

公开命令见 [MadB README](../../README.md#rust--c-首链候选)、[工具链说明](../../adapters/toolchains/rust_c/README.md) 与 [Lab README](https://github.com/changyue1053/MadB-Lab/blob/13f0159da92d34746b096c2c07427a897967fe48/README.md)。

- MadB 代码与夹具修复提交 `7e2a8c2545b5d0264da699125b6bde05898d5383`：[PR 完整门禁](https://github.com/changyue1053/MadB/actions/runs/40095332085)、[PR 原生候选](https://github.com/changyue1053/MadB/actions/runs/40095332069)及对应 push 检查通过。
- Lab 提交 `13f0159da92d34746b096c2c07427a897967fe48`：[PR 基线](https://github.com/changyue1053/MadB-Lab/actions/runs/40095102802)、[PR 独立生产/消费](https://github.com/changyue1053/MadB-Lab/actions/runs/40095102787)及对应 push 检查通过。
- 下载并核验 [Linux Lab 分支证据](https://github.com/changyue1053/MadB-Lab/actions/runs/40095066716)：4 份报告、19 份实例均通过公共 schema，Lab revision 与固定提交匹配。

本机正式候选与报告保存于 `/private/tmp/madb-c05-candidates-bd2fc28`、`/private/tmp/madb-c05-formal-lab`；这些临时路径不是 GitHub 消费依赖。远程工作流保留公开候选与 Lab evidence artifacts，可从固定提交重新构建。报告正常输出不提交到源码仓库。

## 保留的范围边界

未交付 `madb init`、通用任意语言生成器、其他路线、远程缓存、全平台性能预算或正式版本发布。SDK 的裸指针有效性和外国异常不跨 C 边界等调用前提仍须满足；测试不会将这些外部前提转化为任意内存安全证明。首链不授予其他 profile 或 C09/C10 全矩阵验收状态。
