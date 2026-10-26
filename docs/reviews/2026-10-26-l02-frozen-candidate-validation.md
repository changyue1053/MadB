# L02 冻结候选与双目标复验

任务：L02 / C09 已有首链验证子范围。依据 ADR-0020、ADR-0021、validation/02、03、07 和当前 catalog。产品固定为 clean `fbb0a493fa0380797918a5a83c210b1b5f621ae1`，最终独立 Lab 为 clean `9286861babe9b0f3d65a8ac7cc8f87469dc63475`，案例目录摘要为 `sha256:5c74de3764114dc4b82a3a63a506a9de50b2bd8c5c88cd76aeb29dc143557b98`。本补记记录冻结后的消费和修正后的新报告；实现检查见[第六批记录](2026-10-26-chief-architect-sixth-batch.md)，旧报告保留原字节与结论。

## 本次交付和验证边界

已批准的 explain、固定 64 槽 configure/drain 诊断、checked constructor 和逐符号责任表由上述产品提交提供。普通库保持 7 个导出，诊断库为 9 个；测试伴随配置单独绑定。首链消费者只读取公开候选，不导入 MadB 私有源码。主协调单写 catalog 和共享文档；Linux 目标执行由单一执行者串行占用本任务容器，独立审查者只读证据。

Lab 新增五个既有 required ID 的 case/1，分别属于诊断 selector 的 3 例和诊断伴随 selector 的 2 例。预期、fixture、步骤和预算在编译与目标调用前冻结；两个结果复用既有 report/1。诊断开关、必要保护、身份错配、隔离、容量和清理按实际观察核对；stub 与真实 SDK 进程分列，没有数值资源快照时不填造计数。

| 最终目标与配置 | L02 标准报告 | C05 回归报告 |
|---|---|---|
| macOS arm64；固定 Xcode Clang、显式 Mac SDKROOT | 5 例 / 2 份报告，全部通过 | 19 例 / 4 份报告，全部通过 |
| Linux x86_64；QEMU 7 用户态；C05 使用固定 Clang 的私有 cc 路径 | 5 例 / 2 份报告，全部通过 | 19 例 / 4 份报告，全部通过 |

每端 L02 的 84 项观察分别为 56 次真实 SDK 进程、24 次 helper stub 和 4 个导出检查；均含 12 次编译。每端 C05 的 19 项分别为 17 个 target-run、1 个准确的 MissingBootstrap 链接拒绝和 1 个静态 ArtifactMismatch 拒绝，不能统称 19 次运行。运行、编译、静态检查及独立审查各自保留证据层级。Linux 模拟目标不扩大为物理 x86 硬件或性能认证。

## 固定候选身份

两目标各自从 clean 产品提交生成五配置，未使用 allow-dirty。macOS 冻结清单 SHA-256 为 `e00b3267fb2d49d684d4a5fbadca075c29d84c1f0abc38c4fd8027cd19dca574`，90 个公开文件核对不变。Linux 冻结清单为 `cbdd19a385816a3d880c92a5d994f74ef8bb4f25229d7754646ed8c263a80675`，公开候选 tar 为 `f27e1607357b0b8792ca0ba470555c130b39713d60a34b4a30bc5987a304445f`。下表均为 candidate digest 的 SHA-256 值。

| 配置 | macOS arm64 | Linux x86_64 |
|---|---|---|
| c-to-rust | `9a09515422305c2bf09a452c68d6afe373c27e277b510509f986c55395446959` | `2be79bf205dd1613260afccb1fcad4b5895e71f259156f324a49856b4b7c6736` |
| c-to-rust-test | `ef293174133bed23a04c01fe5dc11229c386ad95eb62f7d80ded6716fd02114f` | `677102b8d04b2957742602dc00acdfc8739b0beebd3aea59adca5aa00e6d877e` |
| rust-to-c | `6fbb120b599ccf85fd0868aa54b2750283e5fcb208fefcb6c695301f1668e842` | `a22053ccfe7d1afc579c2e738aee425bae30b75922636c0bd254a55d77337dce` |
| c-to-rust-diagnostics | `8a35b2eaa3fea067996a5a948195183385a70a77127cf877e3aee9892eaab766` | `6caeb2dce347fe270744b62743ae5c9f514f09eea00be4ac2924106d71e2849c` |
| c-to-rust-diagnostics-test | `06259cae9146b314b78699d68ecf3336a0e2da2f618e23315d8fd209a816b604` | `e69793477c3320ac4f4fa87bbac83e8da93e8e62d9aefcd38ab6ac09098d6b4a` |

## Lab 修正与最终 Mac 证据

Lab `f909400fef89c6fe323f2b015dd662551c058fbb` 补齐五例、报告接线和实际 Linux 头文件检查。[源码独审](l02-standard-cases-source-review.json)覆盖 522 项分类变异、16 组报告检查、28 项单测、49 项 Linux 有限头文件断言和 6 组交叉审查。首版将缺字段或错误类型归为 failed 的问题已改正：不完整证据为 error，完整错误值及真实 C 断言失败仍为 failed。原失败记录保留。

后续独审发现 C05 的 started_at 原取报告写出时间。最终 Lab `9286861` 在 group/integrity 执行前捕获时间并原样传入报告，新增 4 项作者测试和 9 条独立正反路径。初次集成检查准确报出 LAB104：packaging case 绑定的 runner 摘要已过期；只更新其 fixture 大小和摘要，步骤、预期、预算不变。旧摘要、错摘要和错大小仍拒绝；[最终源码独审](l02-c05-timing-source-review.json)核对三文件、AST 行为差异及完整 601 项门禁。案例目录身份随真实 fixture 字节变化，因此最终两端均重新运行 C05 和 L02，而非只替换报告标签。

Mac 最终输出为 `c05-macos-timing-002` 与 `l02-macos-standard-004`，两命令 exit0。[执行记录](l02-final-macos-execution.json)绑定实际命令、工具、SDKROOT、报告摘要和起止区间；[最终独审](l02-final-macos-review.json)核对 24 个案例、6 份报告、候选与附件字节、运行前实例冻结、真实编译依赖、清理和全部 84 项 L02 观察。六个报告的 started_at 均位于其真实外层执行区间。

Mac 首次新 Lab 回归 `c05-macos-timing-001` 因直接调用 Clang 未设 SDKROOT，stderr 明确缺 stdlib.h；结果为 3 passed / 4 error / 12 blocked，L02 未启动。随后仅给外层消费者环境设置实际 Xcode Mac SDKROOT，源码、候选、工具及预算不变，在新目录产生上述成功报告。失败目录与原外层记录保持原样。

## 最终 Linux 环境与证据

任务容器使用固定 Ubuntu amd64 根、Rust 1.99.1、Clang 18.1.3、bindgen 0.73.2、cbindgen 0.29.4 和 Python 3.14.4；实际宿主 VM 为 aarch64，x86_64 目标由 QEMU 7 执行。源码、候选和运行位于 Linux 文件系统；大小写不敏感的共享目录只用于缓存及 tar。既有业务容器未修改。Python GCC 编译、初次安装时限、cbindgen GCC 链接和首次五候选构建的磁盘余量中止均保留记录；不将这些失败归咎为已证明的 QEMU 缺陷。

第二次原构建器完整生成五份候选且 clean revision 核对通过。只回收已完成、已逐字核验归档的本任务中间目录，预算与检查不变；最低磁盘余量 2418704384 字节，高于 2 GiB。普通配置的两次 inventory 相同，不扩大为五配置均重建两遍。

最终 clean Lab `9286861` 的真实串行顺序如下，三次执行分别冻结，旧结果不覆盖：

1. `c05-final-002`：原 GNU cc 配置 exit1。C→Rust 普通／伴随及 packaging 通过；Rust→C 首次 Cargo 构建在 build script 链接时 cc 进程 SIGSEGV，记 error，后续三项 blocked；整体为 15 passed / 1 error / 3 blocked，四项 reverse 运行均未发生。
2. `l02-final-002`：保持原配置，exit0，五项标准案例和两个 report/1 通过。
3. `c05-final-003`：唯一一次新配置使用私有目录中的透明 `cc → /usr/bin/clang → /usr/lib/llvm-18/bin/clang`，通过 PATH 供 Cargo 选择；固定镜像身份在前后核对，系统 GNU cc 未修改。原 runner、源码、候选及 120 秒构建预算保持，exit0，19 项通过。这证明该 Clang 消费配置，不表示原 GNU cc 配置恢复。

成功 Cargo 日志未启 verbose，故只保留实际 Cargo argv 与输出、PATH、透明链接和工具镜像，不编造完整成功链接 argv；失败 002 中实际出现的 cc 链接命令仍保留。C05 公开 environment 维持原 clang/cargo 字段，实际 GNU nm 2.42 的路径、版本、字节与 argv 由外围证据单列；不与候选及 L02 使用的 LLVM nm 18 混同。L02 对四个 profile 各 37 个公共头文件（15 个 resource、22 个系统头）核对真实 Clang 闭包，27 个 Rust rlib 仅作为生产端 receipt 保留。输入原字节归档覆盖实际记录的工具、头及依赖，不冒充全部隐式 OS 动态依赖或可安装 SDK。

[最终 Linux 执行清单](l02-final-linux-execution.json)记录三次执行、六份成功报告和原失败报告，导出冻结清单 SHA-256 为 `26cfa42a29220f4059acc9307aa6ab2220703237102e73d103d2ee5f77c65fde`（408 个文件）。实际输入映射覆盖 193 条 Linux 路径、185 份内容，原字节 tar 为 `798790f1cae8d5e0425e4ef502eb8d0798c03eea2a2cce7987f4f3f18acfbe3a`；完整 Lab 源 bundle 为 `936790fe77366a5302021c27ff3d9a36516f2637e54ccff62e3c44ff3ed50fa8`。[最终 Linux 独审](l02-final-linux-review.json)核对 408 个导出文件、实际 Lab 481 文件、三个执行冻结、24 个成功案例、6 份成功报告和 628 次附件引用；原 GNU cc 失败准确保留，Clang 私有路径和系统 cc 的前后字节吻合。源码及候选独审按精确字节复用；审查未重跑 SDK 或容器。

## 历史结果、归档与剩余交付

Lab `a6ec858` 的初次 Mac L02 为 local observation，不是 report/1，不能事后补身份升级。其 [84 项观察独审](l02-frozen-macos-initial-review.json)与[C05 公开格式复核](l02-frozen-c05-public-format-review.json)保持原结论。Lab `f909400` 两端五项标准案例的[Mac 独审](l02-standard-macos-review.json)和[Linux 独审](l02-standard-linux-review.json)也保留原身份；其中旧 C05 时间字段的限制已明确，不回填最终 Lab revision 或新目录摘要。

首阶段归档为产品仓库外 `.local/evidence/round6-fbb0a49-a6ec858`，清单 SHA-256 为 `5638130ac8406e42465a19490f962db5600c46f72ea2bc17b4809e56bdc1530e`。原清单与报告保持不变；本次完整增量已另存 `.local/evidence/round6-l02-fbb0a49-reports`，包含源 bundle、原候选、成功与失败日志、具名 Linux 输入原字节及独立审查。共 1977 个文件、651631284 字节，复制前后逐文件摘要、大小及模式核对通过，清单 SHA-256 为 `efab2b39715d725fd0f16ebc5ba252313eb52f35281d39ff93d8c12ec4ef5df3`。见[持久归档索引](l02-frozen-evidence-archive.json)；原绝对路径保留，清单给出持久位置映射。

L02 第二独立业务仍须完成 DX-01/02/11：公开声明增方法、改字段、重命名和删除后真实重生成，再由独立消费者验证。固定七／九角色及准确拒绝不能替代该成功路径。有限自然 Rust 值配方已有具体 ABI 与读取链提案及独审，等待项目所有者的具体格式决定；设计编译示例不计生产读取、生成或目标消费实现。L06、C09 和 L07 保持原分工，不增加 L02 新前置。状态仅在 catalog；C06/C07 已确认整包对象不改，本记录不授予其他目标、完整 L02 或公开发布资格。退出对账来源见[审计输入](l02-exit-audit-inputs.json)。

阶段提交 `fbb0a49` / `a6ec858` 已带 `[skip ci]` 推送，核对时两提交均无 Actions run，见[首阶段远端记录](sixth-batch-stage1-push-verification.json)。最终证据提交仍按同一约束交付，不创建 PR、标签或发布；零次运行不是 CI 通过，项目完成后独立补跑的安排不变。文档静态检查与生成一致性单独记录；本次未重新执行浏览器布局或交互验证。

归档前的[范围独审](l02-frozen-document-review.md)核对目标、案例层级、历史结论及第二业务边界；其中标明待填的 Linux 独审和持久归档已由上述实际证据补齐。文档源生成 13 页 HTML 后，`render_visuals.py --check`、`check_docs.py`、29 项文档工具测试和 `git diff --check` 均通过；原生及 SDK 检查复用上述固定源码与候选的独立结果，不把文档门禁当产品验收。
