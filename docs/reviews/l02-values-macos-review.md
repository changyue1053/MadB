# macOS 正式候选独立审查

导航副本：仅将目录及带行号路径链接改为文本，规则和结论不变；原始受审字节 SHA-256 为 `870f368be8d10eb92ee02b9248e07f9854a123a76ec702a5e1c3c946dfc43c2c`，原稿保持于[持久归档](l02-values-evidence-archive.json)。采用或复核仍绑定该原稿。

结论：固定候选的值配方与既有 L02 记录，和实际源码、工具、运行日志及附件一致；C05 显式 SDKROOT 补跑的 19 个 passed 有对应证据。保留 C05 两份构建日志被覆盖的归档缺口，不对“全部构建原始日志完整”作通过声明。本次只读审查未重跑 SDK、编译器或门禁，未修改源码、候选或原报告，也不裁定 L02 整包退出。

## 固定范围与身份

- Producer：`e0f74a5fa5a9a6e73400ced70bc81ba06751227f`；Lab：`2d06cbefa0f49187f163ce937c8c3bd01a062eb1`。
- 外部 pins 冻结于 `2026-10-26T19:41:47.743774+00:00`。12 份原候选均经过公共 `verify_candidate` 和独立文件/语义摘要核对，无 provisional；7 份新值配方候选及 4 份既有 L02 安装副本逐字一致。
- 实际产品根与 Lab 根分别为 `/Users/changyue/Desktop/MadB/madb`、`/Users/changyue/Desktop/MadB/madb_lab`；审查时 clean，HEAD 为以上版本，1186/523 个受管文件与隔离生产源码逐字一致。
- 7 份真实生产工作区各核对 41 个锁输入（36 个固定源码、5 个生成输入）、26 个 schema、工具实际字节及 120 个 SDK 输入；自然 Rust 作者与独立 Lab fixture 相符。消费者没有借用 MadB 私有实现。
- catalog 为 `sha256:feb2403181bf0a96371f0028c7fc582cbdf690f6682014b0f92adee9e8affaa7`，Lab 523 文件清单为 `sha256:6cac4a432435cff7f622317cd56885397c9896bc6e82e96997bc9772fadcb1d5`。

环境为 macOS 26.6.2 Arm64，目标 `aarch64-apple-darwin`，Apple Clang 21.0.0、Rust 1.99.1、Python 3.14.4、bindgen 0.73.2、cbindgen 0.29.4；固定工具路径和字节摘要见 JSON，不以版本字符串单独认定工具身份。

## 重新汇总的实际结果

| 范围 | report/1 | 案例结果 | 报告分级 | 实际证据范围 |
|---|---:|---|---|---|
| 新值配方 | 7 | 29 passed | 23 target-run、5 static、1 compiled | 18 个真实 SDK 案例，5 个 helper-only 案例；后两类静态/编译案例不计运行 |
| 既有 L02 | 2 | 5 passed | 5 target-run | 56 次真实 SDK 进程、24 次 helper-only 探针、4 次导出检查 |
| C05 显式 SDKROOT 补跑 | 4 | 19 passed | 17 target-run、1 compiled、1 static | 实际消费 3 个 first-route 候选；另 2 个 diagnostics 候选仅核对身份 |

值配方的 18 个真实 SDK 案例含 19 次进程执行，两个方向的混合身份拒绝均有实际链接及运行日志，并启用限定的 `MADB_LAB_MISMATCH_ONLY` 分支。5 个 helper-only 进程覆盖 116 个畸形 provider 向量，不能写成真实 SDK 的运行或分配回收证据。另核对 9 个指定编译/链接拒绝、7 个实际导出检查（5 个独立案例加 2 个辅助候选检查）；不把断言数、进程数与案例数混用。

值配方与既有 L02 分别核对 80/112 个原始命令日志、112/120 个唯一附件，连同原始观察、源归档、编译依赖、工具及消费者二进制。C05 核对 38 个附件、19 个案例摘要、40 个保留进程日志及 7 个资源平衡记录。各报告的 candidate/profile、producer/Lab、catalog 与 case 输入身份一致。

值配方分组开始时间为 `2026-10-26T19:41:50.458976Z`，既有 L02 为 `2026-10-26T19:42:13.719356Z`，均晚于 pins 冻结；源码在执行前捕获开始时间，文件时间与持续时间亦相符。但文件 mtime 仅作佐证，子进程没有独立可信绝对开始戳，不能外推更细时序证明。

## C05 初轮与补跑分别保留

首次目录 formal-macos-lab-r7/c05（原路径：`/private/tmp/madb-chief-round7/formal-macos-lab-r7/c05`） 仍为 exit 1：4 error、12 blocked、3 passed；四次编译错误为找不到 `stdlib.h`。原日志与摘要未变。

补跑目录 formal-macos-c05-r7-sdkroot/reports（原路径：`/private/tmp/madb-chief-round7/formal-macos-c05-r7-sdkroot/reports`） 使用外层显式 `SDKROOT=/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX27.0.sdk`，于 `2026-10-26T19:43:52.065384+00:00` 至 `2026-10-26T19:44:05.333199+00:00` 执行，exit 0。只有这次补跑为 19 passed，不能回填首次失败。

## 尚存归档限制与后续

C05 Rust→C 的 `abi-mismatch`、`profile-mismatch` 在 compile_rust（原路径：`/private/tmp/madb-chief-round7/madb_lab/harness/runners/first_route.py:197`） 把 Cargo build 输出写到 `name.json`，随后 run（原路径：`/private/tmp/madb-chief-round7/madb_lab/harness/runners/first_route.py:357`） 覆盖相同路径。两份构建原始输出已不在现存报告里；provider compile/archive、lock/metadata、实际运行完成记录及固定 Runner 仍保留。本次不推翻已观察到的运行拒绝，但不能认证这两份构建日志完整。建议后续窄修复为构建/运行各用独立日志路径，以新 Lab 版本生成新证据，保留本轮报告。

C05 的三个临时消费者已清理，安装副本与可执行文件无法再次逐字核验；子进程完整环境未归档，SDKROOT 只在外层 run.json 记录。这些限制不适用于已核对现存安装副本的值配方/既有 L02。

结论限于上述固定候选与 macOS Arm64 实际案例，不外推其他平台、真实 ArkTS 页面、GitHub Actions、发布、整个 L02 或其他包的验收。helper 计数不证明 Rust 分配回收，编译/静态拒绝不写成目标运行。

详细证据：[汇总 JSON](/private/tmp/madb-chief-round7/macos-final-independent-review.json)、[值配方与 L02](/private/tmp/madb-chief-round7/macos-review/values-l02-audit.json)、[生产输入](/private/tmp/madb-chief-round7/macos-review/producer-input-audit.json)、[集成源码](/private/tmp/madb-chief-round7/macos-review/integrated-source-audit.json)、[C05 独立子审](/private/tmp/madb-chief-round7/macos-review/c05-audit.md)。汇总 JSON 固定这些审查产物的文件大小和 SHA-256。
