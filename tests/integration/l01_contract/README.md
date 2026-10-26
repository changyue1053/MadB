# L01 声明契约检查

本目录验证 [L01 契约输入](../../../docs/examples/language-api/README.md)的实际接受/拒绝边界。不会更改 reader、profile、语义策略或生成输出，也不创建新格式、状态或认证记录。

在仓库根使用 `.python-version` 与 `.node-version` 的固定环境：

```sh
.venv/bin/python -B -m unittest discover -s tests/integration/l01_contract -v
.venv/bin/python -B -m unittest discover -s tests/integration/c06_reader -v
node tests/integration/l01_contract/check_arkts.cjs "$MADB_L01_TYPESCRIPT" "$MADB_L01_TYPESCRIPT_SHA256"
.venv/bin/python -B tests/integration/l01_contract/check_swift.py --output "$MADB_L01_SWIFT_OUTPUT"
```

`MADB_L01_TYPESCRIPT` 必须指向既有、已审查工具安装中的 `typescript.js` 绝对路径；`MADB_L01_TYPESCRIPT_SHA256` 来自该安装的既有精确输入清单。脚本在调用前后核对模块字节，缺参数、缺工具、错误摘要和意外拒绝均失败，不跳过。不要为了使测试通过把当前随意安装的摘要当作独立供应链认证。

Python 测试消费 C06 已存档的真实 UniFFI/ObjC/Swift 编译器描述符，再调用现有 lowering 和 policy gate。新增方法、增加字段、改名与删除方法的变体只证明“可解析结构不能继承固定 SampleEngine 语义”。没有重新执行 UniFFI/SKIE 编译器，也没有从变体产生 SDK。

Node 检查调用生产 `readFacade`，解析实际 C07 普通模板、作者改名输入、三个既有拒绝输入及独立值业务源码。仅替换模板本来需要的两个模块/身份字面值；这不生成 native 库或运行身份。断言来自这里的显式预期，失败诊断必须指向真实源码、实际行列和规定类别；其他编译/环境错误不能充当负例通过。

Node stdout 记录模块/reader/输入摘要、实际版本和通过案例。模块字节核对不是完整工具链闭包或供应链认证；SDK 类型检查、HAR/HAP 构建、目标运行、第二业务模块生成、独立 Lab 消费与发布均未由该检查证明。独立 Lab 不能把生产 reader 作为自己的预期 oracle。

`MADB_L01_SWIFT_OUTPUT` 必须是仓库与工具安装外、父目录已存在的新目录。Swift 检查核对既有 `adapters/toolchains/kotlin_rust/toolchain.json` 的 Xcode/Swift/macOS SDK 身份，将本检查脚本、生产 Observation 源、三个 Swift 夹具与锁的字节捕获到输出；显式编译输出、module cache 和 TMPDIR 均在此输出下，HOME 保持原值。实际 `swiftc` 使用 Swift 6 严格并发及 warnings-as-errors，先编译公共 Observation 模块并保存系统 `@Observable` 宏展开，再 typecheck 正例和两个精确拒绝。报告记录实际 compiler 摘要、输入摘要、参数、退出码和 stdout/stderr 摘要；运行后重新核对源与 compiler 未变化。

正例必须能导入公共模块并调用 start/requestStop/stopAndWait。反例必须分别包含实际消费者文件位置和“非 Sendable”/“MainActor 隔离方法”错误，其他缺 SDK、宏插件启动失败或错误 import 不算负例通过。此检查不运行 Swift fixture，不连接 native，不测试 UI 或独立安装。若执行沙箱禁止 Apple 系统宏插件启动，保留失败输出，经现有工具权限流程允许该编译后使用另一新输出目录重跑，不能禁用宏或删除该检查来制造成功。

逐项设计输入见 [Swift 契约](../../../docs/examples/language-api/swift-contract.md)、[ArkTS 契约](../../../docs/examples/language-api/arkts-contract.md)及 [L01 验收索引](../../../docs/examples/language-api/acceptance.md)。这些明细没有修改任何当前 profile 的接受范围，也没有将后续普通业务生成和目标验收移走。

这些检查可由主协调入口在集成候选上执行。旧 C06/C07 的正式候选证据仍限定于其原输入与范围；本子范围不能登记 L01 整包通过。
