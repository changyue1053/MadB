# Linux 有限值配方最终独立复核

结论：**固定 Linux 子范围通过，未发现剩余阻断**。结论仅绑定下面的两仓源码、七份候选、实际环境及保留的证据；不赋予 L02 整包、C06/C07/C09/C10、GitHub Actions 或发布状态。本次仅只读核对，未操作容器、未运行 SDK/编译器、未修改两仓。

## 冻结范围与身份

- MadB `e0f74a5fa5a9a6e73400ced70bc81ba06751227f`；Lab `2d06cbefa0f49187f163ce937c8c3bd01a062eb1`。1,186/523 个源文件逐字及执行位核到固定 Git 树，before/after/final 的源码、公开候选、生产者二进制和派生产物布局四组清单一致。
- 目标 `x86_64-unknown-linux-gnu`；实际 QEMU **7.0.0** 有 `/proc/1/exe --version`、原二进制 SHA 与容器/镜像绑定。Clang **18.1.3**、Rust/Cargo **1.99.1**、Python **3.14.4**、bindgen **0.73.2**、cbindgen **0.29.4** 固定工具原字节已核。`/usr/bin/clang` 别名补证的 PATH 与原 stage 一致，解析字节等于固定 Clang。
- 外部候选冻结时间 `2026-10-26T20:26:49.197574+00:00` 早于案例输入记录 `2026-10-26T20:29:12.447393Z`；候选、安装副本、公共验证器结果与冻结清单逐项相符。

| 候选 | SHA256 |
| --- | --- |
| author-0 | `sha256:2a8a74b8c41927982c02138b96e4351391979bc3187389d0c7db990e8464e21f` |
| author-1 | `sha256:773ecbc70cd48958343132c210637502565dde3df6a04913b825075b1a1140bf` |
| author-2 | `sha256:8ece2a00f7c86919cdbe5cf2ec7ec91192f8ed56b14ba479e9f4e1fe020b72e3` |
| author-3 | `sha256:a58742787042a0f2aafadf063bded8ef5c45bfaa57c1eafe26891ef20efdfa8c` |
| author-4 | `sha256:18111822df18e46e9326db888823ab0922b2add5b0ce49ff6cda81a546882f94` |
| aux-grow | `sha256:c42aa5e9070c66539d46b0b2aad2baaee6a433cdba68b0f4624b6346492b6db0` |
| aux-panic | `sha256:1d4b879f0afa98ec0cb8bb16351a46e065132a61e4df1ce31a2044ab8eb2d80e` |

## 正式 Lab 与专项结果分别成立

7 份 `report/1` 按固定产品 schema 校验通过；29 个实例的模板、案例摘要、身份、selector、required_cases、110 个唯一附件和78份原命令日志均匹配。记录为 **23 target-run、5 static、1 compiled**，不能合称29个真实SDK运行。23个 target-run 中，18个真实SDK案例共 **19个进程**（mixed-candidates包含两个方向），另有 **5个 helper-only stub 进程、116个向量**。各真实进程的动态库路径、PID、完整bootstrap身份和实际编译/链接命令吻合。另核到7次导出检查、9次预期编译/链接拒绝、14个消费者可执行及其编译输入。

原生专项为 **codegen 4项、client 3项通过**，不并入Lab29：codegen包含3项真实工具测试和1项纯渲染检查；client包含1项真实C stub测试和2项纯检查。保存的codegen子命令为15次exit0及1次预期SIGABRT。真实分配器harness确认reserve失败、后字段非法先于分配、正常值、超长输出、Result错误及recoverable unwind的输出与计数；primary payload析构再panic时正常释放secondary并返回8，输出零、分配回基线；secondary析构继续panic由guard明确abort，不记作返回8或全部清理成功。生成shim包含固定修复runtime。原P2修复结论沿用独立复现记录，不由Lab测试替代原生复核。

Lab仓库检查另有621项测试成功，日志明确不授予SDK认证；本审未将该计数提升为621个SDK案例。

## 实际输入、归档与失败保留

77项具名工具/runtime/SDK原字节及运输包逐项核对。审核发现正式消费者还引用60个未导出的头文件后，只读追加独立补证，逐字匹配冻结编译记录；原77项、原报告及候选未改。现已覆盖报告记录的 **99个外部编译/头探针/工具路径**。这是具名及实际记录输入的字节绑定，不是完整操作系统导出。

七个成功生产者工作区均核到41项lock输入（其中36项固定产品源）、26份schema、10项backend工具、64项SDK输入、host及18份候选文件。八份归档（七成功加一次author-1失败）共 **8,801条目、1,768,113,809原始文件字节、466,822,994压缩字节**；成员集合、文件摘要/大小/权限、目录与链接目标及原恢复位置均核对。首份author-0使用v1，虽无v2内嵌invocation record，仍由原命令路径、exit0、stdout工作区、author和候选字节绑定；后七份v2另保留原日志摘要。

原prepare的整个target链接clean-tree拒绝仍保留；其后布局修复和成功执行分别记录。author-1首次执行在 `/work=2,144,382,976 ≤ 2,147,483,648` 时由原2GiB守卫终止（exit -9），失败树未赋予成功身份；重试为另一工作区。触发值在`command.failure`和`after`，因先抛错未进入`space.jsonl`，不称连续无缺口。生成/消费/测试子进程维持原守卫；成功stage已记录最低`/work=2,224,766,976`。

首次author-0恢复搬移在已静止源`/work=2,144,321,536`时读取，写到余量大于2GiB的`/cache`，核全归档后释放本轮具名临时副本恢复空间，再继续构建；不将此存储恢复算成功build/test stage或降低守卫。最终记录`/work=2,405,220,352`、`/cache=137,280,614,400`。

## 保留边界及其影响

1. **client专项临时产物不全保留**：stub编译/运行原子日志、临时头和可执行已随TemporaryDirectory清理。冻结测试源码和外层3项成功仍有证据；正式Lab的5个helper进程另有完整日志/头/二进制。因此它不使既有有限测试断言失效，不需补跑；但不能声称该client专项所有临时字节可重查。
2. **8个参数名检查复用源路径**：仅最后一版源留存，8个命令与rmeta在；中间源码可从固定循环推导，不能称为当时原字节留存。现有8次编译结果仍成立，无需扩大门禁。
3. 八个具名生产者树已按manifest完整核验；**未实际恢复并运行**，静止性基于保留的前后历史检查。本结论不覆盖所有Lab/client/unittest临时产物。
4. 原子日志没有独立可信的绝对开始时刻；外部freeze先于案例的顺序按保留记录核对。QEMU执行不扩大为物理x86、性能、其他系统或平台结论。

上述限制是实际证据保留与适用范围；未削减必需检查，未把失败改写为成功。无需为这些已限定断言重新执行整套SDK。

机器结果及全部摘要：[最终JSON](/private/tmp/madb-chief-round7/linux-final-independent-review.json)。主核对：[Lab证据](/private/tmp/madb-chief-round7/linux-review/values-report-audit.json)、[输入闭合](/private/tmp/madb-chief-round7/linux-review/input-closure-audit.json)；独立子审：[原生](/private/tmp/madb-chief-round7/linux-native-evidence-review.md)、[归档](/private/tmp/madb-chief-round7/linux-archive-evidence-review.md)。原生P2历史及修复证据保留于[原独审](/private/tmp/madb-chief-round7/native-independent-review.md)。
