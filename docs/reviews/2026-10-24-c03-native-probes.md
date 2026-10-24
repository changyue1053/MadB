# C03 首链工具链与风险探针

日期：2026-10-24。任务依据：设计 0.9、C02 `99506b48a51859dd7c5fae270425bbcc17e6baec`，分支 `codex/c03-native-probes`。C02 PR #2 尚未合入，本次在其基础上单独交付 C03。

## 验收范围

| 项目 | 本次结果 | 证据与边界 |
|---|---|---|
| P01 本机 CLI | macOS Arm64 隔离安装与 help/version/未实现命令拒绝通过 | release 安装到独立目录；运行时离开仓库、移除 Cargo 路径 |
| P01 帮助进程 | 6 个测试、16 个场景通过 | 真实子进程；版本/IR、功能、身份、异常 stdout、退出 23、超时；不是完整扩展执行器 |
| P01 Linux | 等待本 PR CI | 相同 `scripts/check.py` 入口执行；不以本机成功替代 |
| P02 Arm64 | 两个独立消费者真实运行通过，各 10 个场景 | C→Rust 使用 cbindgen 头和 Rust staticlib；Rust→C 使用 bindgen 声明和独立 C staticlib |
| P02 x86_64 | 布局、目标编译、链接及负例通过 | 未运行，不能宣称 Intel 或 Rosetta 运行认证 |
| 生成形状 | 两目标的完整 C02 声明及 stateless 接口编译通过 | 检查 12 个状态码的有符号类型/数值、函数指针、callback 类型、结构体布局 |
| 安全拒绝 | 两目标均拒绝 C/Rust 错签名和错误 packing | 错签名分别要求指定编译诊断和 Rust E0308；不接受任意失败 |
| 工具及输入锁 | 4 个真实 profile、project、madb.lock 通过 Python schema 与 Rust C02 校验 | 读取本次真实文件字节；篡改复制的头文件或编译后的 C 库均被 FILE004 拒绝，之后恢复原字节 |
| 后续风险 | P03–P15 的负责人与既有工作包/阻断范围明确 | P03/P05 在对应组合集成前先复现；不扩大成 Rust/C 的全局依赖 |

本次 41 条原生命令与工具/输入/产物摘要的可携带记录见 [C03 原生证据](c03-native-evidence.json)，完整本地中间产物保存在忽略的 `reports/c03-reviewed`。重现入口见 [C03 README](../../tests/integration/c03/README.md)。记录经过路径归一化，不携带 SDK、生成器或编译库二进制。

## 实测配套与修正

- Rust 1.99.1、bindgen 0.73.2、cbindgen 0.29.4；Xcode 27.0 (27A266a)、SDK 27.0 (26A425)、Apple Clang 21.0.0。固定 deployment target 14.0 只描述编译，不构成 macOS 14 实机支持承诺。其他 Apple 路线仍按自己的配置验证。
- bindgen 未生成 `INT32_C(...)` 常量；普通整数宏默认又被生成成 `u32`。改为保持原数值的字面量宏，并固定 `--default-macro-constant-type signed`；消费者断言继续要求 `i32`，没有用强转消除错误。C02 的错误码、函数签名和关闭协议未改变。
- Apple LLVM 21 nm 对 Rust LLVM 23 的 archive bitcode 报 `Unknown attribute kind (105)`。Rust 归档改用同 Rust 版本的 llvm-tools；C 工具保持 Apple 套件。没有忽略非零退出码或强制所有生态统一 LLVM。
- 生成器安装先核对归档和内含 Cargo.lock，再按锁构建；运行前核对安装 receipt 与真实二进制摘要。标准库、libclang、SDK settings/link stub、实际传递头文件和命令参数进入观察锁。完整 SDK 归档、生产依赖闭包和缓存失效仍须由 C04/C10 完成，不能从本探针推导。

依据：[bindgen 的 libclang 要求](https://rust-lang.github.io/rust-bindgen/requirements.html)、[bindgen 0.73.2](https://github.com/rust-lang/rust-bindgen/tree/v0.73.2)、[cbindgen 0.29.4](https://github.com/mozilla/cbindgen/blob/v0.29.4/docs.md)、[Rust Apple 目标和 deployment target](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html)。实际结果优先于候选版本名称。

## 全部修改范围

1. `tests/fixtures/native-probe`：独立 C/Rust provider/consumer、布局与完整 ABI 形状断言、错签名负例；原型不实现 handle/observe/close。
2. `tests/integration/c03`：固定配方、来源校验安装、真实原生执行、命令/输入记录、既有格式锁装配、P01 帮助进程与隔离 CLI 测试、复现 README。
3. `tests/golden/c03`：锁定工具实际生成的两份 C 头、两个目标各两份 Rust 声明；独立消费者的手写断言避免只比较生成器自己的输出。
4. `adapters/contracts/examples/check_probe_lock.rs`：调用 C02 公开 API 验证真实 probe 文件；库、核心依赖方向和生产 CLI 行为不变。
5. `scripts/check.py`：P01 纳入本机/CI 同一入口；保留原必需状态名，Apple P02 使用显式独立入口。
6. 首链 `expected.h`：宏表示适配真实 bindgen，数值及 ABI 不变；相关说明同步。
7. README、技术栈/后端规范、首链准入、实施计划、风险归属、第三方说明与 review 索引：同步实测组合、六类 S0 资产、问题与后续责任。
8. catalog 与生成 HTML：只通过目录和渲染脚本更新实施状态；没有更改路线数、工作包、阶段或认证结果。

## S0 六类资产交接

| 资产 | C03 后可用内容 | 后续工作 |
|---|---|---|
| 作者与语义 | C02 完整 ABI/协议 + C03 两个有明确安全前提的同步实现 | C04/C05 source reader、有效 IR 和全部语义校验 |
| 生成形状 | 实际 cbindgen/bindgen 输出、双目标编译、独立签名/数值断言 | C04 生成事务、C05 正式 SDK |
| payload/诊断 | C02 schema/向量 + P01 进程负例 | E02/C04 完整协商、请求状态机与退出清理 |
| profile/锁 | 两方向×两目标、原始指令、来源、摘要、SDK/标准库观察 | C04 完整依赖发现、冻结执行和缓存失效 |
| 消费者/预期 | 两独立程序、20 个运行场景、错签名/packing | C05/C09 资源、并发与独立 Lab |
| 包布局/配方 | header、staticlib、绑定、目标目录及复现命令 | C05/C10 按候选 schema 打包、bootstrap/错库校验、离开工具仓库干净消费 |

## 实际检查与保留范围

执行命令：`scripts/check.py`、P02 `native.py`、Rust `check_probe_lock`、复制头文件篡改负例、`git diff --check`。本地完整通用检查已通过：37 个 Rust 测试，5 个格式/ABI、6 个进程、7 个架构和 21 个文档测试，共 76 个测试函数；另有 3 个隔离 CLI 场景、20 个双向原生场景、两目标编译负例及 7 组跨实现摘要向量。最终 CI 结果随后补充。

没有更改 MadB-Lab；没有公开 SDK、完成 C04、认证任何路线或证明最低系统支持。两目标都是 LP64，本样例布局相同；错误 packing 仍会被捕获。宏/头文件生成与 stateless 运行不证明生命周期、线程同步、关闭/排水或任意外部指针安全。未重新执行浏览器视觉验收，HTML 生成/DOM 单元检查不冒充浏览器结果。

C03 完成不代表 S0 整体退出：E01/E02/E04、I01 等已列 S0 准入继续按既有计划交付。C04 可以在 C01/C02/C03 的已验证基线上开发；C05 仍需实际源读取、事务、受管 SDK 与独立 Lab。
