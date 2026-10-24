# C03/P01–P02 可复现探针

任务依据：0.9 设计、C02 `99506b48a51859dd7c5fae270425bbcc17e6baec`。本目录属于工具内部 integration 测试，不是 MadB SDK，也不代替独立 MadB-Lab。

## 已固定的范围

- P01：当前 CLI 在真实 macOS Arm64 / Linux CI 上隔离安装与运行；真实子进程握手、身份/版本/功能拒绝、崩溃/超时/非法 stdout。完整请求状态机、取消、进程树清理和恶意扩展沙箱仍由 C04/E02 交付。
- P02：cbindgen 生成 C 头、bindgen 生成 Rust 声明；两个独立消费者各验证 10 个同步标量/缓冲/错误场景。Arm64 真实运行；x86_64 只做布局、编译、链接。两者均验证 C/Rust 错签名和错误 packing 被拒绝。
- C02 完整 engine ABI 另行生成/编译并核对 12 个错误码及函数指针类型。运行探针只有 stateless checksum/echo，不用假实现补齐 create/observe/close，也不证明释放/并发/取消正确。

`toolchain.json` 固定 Rust 1.99.1、bindgen 0.73.2、cbindgen 0.29.4、Xcode 27.0 (27A266a)、SDK 27.0 (26A425)、Apple Clang 21.0.0 和 deployment target 14.0。14.0 仅是本次编译参数；在较新系统上运行不证明最低系统支持。其他 Apple/Swift 配置不随此探针升级。

## 从独立检出复现

先按根 README 安装开发依赖；P02 要求上述 Xcode/SDK 和 macOS Arm64。只设置当前命令的 `DEVELOPER_DIR` 即可选择安装，勿修改全局 Xcode 或 Rust 默认值。

```sh
rustup target add --toolchain 1.99.1 aarch64-apple-darwin x86_64-apple-darwin
rustup component add --toolchain 1.99.1 llvm-tools-preview
.venv/bin/python scripts/install_native_tools.py --root target/c03-tools
.venv/bin/python -m unittest discover -s tests/integration/c03 -v
.venv/bin/python tests/integration/c03/cli_install.py
.venv/bin/python tests/integration/c03/native.py \
  --output reports/c03-run \
  --bindgen target/c03-tools/bin/bindgen \
  --cbindgen target/c03-tools/bin/cbindgen
cargo run --locked -p madb-contracts --example check_probe_lock -- reports/c03-run
```

安装配方位于 `adapters/toolchains/rust_c/toolchain.json`；本目录同名文件保留原探针锁定快照。旧 `tests/integration/c03/install_tools.py` 入口转发到同一生产安装器，已有 receipt 格式不变。

安装目录与运行输出必须是新目录，避免覆盖已有记录。第二次运行可复用工具，选择新的输出目录。安装器先验证上游归档和内含 Cargo.lock 摘要，再 `cargo install --locked`；receipt 绑定源码、构建 Rust 版本及实际二进制摘要。仅工具的版本文本一致不足以冒充这份安装。

普通 `scripts/check.py` 包含 P01 和已有全部门禁；Linux CI 不执行 Apple P02。缺 P02 环境会明确失败，不算 skipped pass。生成结果逐字节比对 `tests/golden/c03`；更新基线须审查声明/参数变化，运行器没有自动接受差异的开关。

## 产物与证据

```text
<output>/
├── generated/{rust_provider.h,expected.h}
├── <target>/{c_provider.rs,expected.rs,*.a,c_consumer,rust_consumer,...}
├── profiles/<direction-and-target>.json
├── project.json / madb.lock.json / observed-sdk.json
├── tests/... / docs/...                  # 被锁输入的复制快照
├── commands.json / toolchain-lock.json   # 原始命令与工具/SDK观察
└── result.json                          # 运行层级及锁摘要
```

四个 stateless profile、project 与 madb.lock 使用 C02 既有 schema。`freeze.py` 是探针的文件装配；Rust `check_probe_lock` 重新检查 schema、领域能力、语义摘要、工具一致性和文件字节。原始 observations 是测试记录，不新增公共锁协议。生产源读取器/ContractIR、完整依赖闭包、冻结执行与候选提交仍属 C04/C05；本工具不把观察记录当生产缓存命中依据。

锁记录工具/SDK 构建身份、生成器来源、二进制摘要、实际参数、所用传递头文件、SDK settings/link stub 和 Rust target 标准库摘要。它不是 Apple SDK 安装包的完整散列，也不承诺重建后的 Mach-O 二进制逐字节一致。公开仓库仅保存源、生成声明和摘要记录，不分发 Xcode/SDK 或工具二进制。

## 真实发现及处理

1. `INT32_C(...)` 宏在当前 bindgen 下未生成；普通正整数字面量默认又生成 `u32`。首链头文件采用数值不变的普通整数宏，绑定参数固定 `--default-macro-constant-type signed`，独立 Rust 编译断言检查全部状态码的 `i32` 类型及数值，不在消费者里强制转型掩盖问题。
2. Apple LLVM 21 的 nm 不能读取 Rust LLVM 23 的部分 archive bitcode。Rust 归档使用 Rust 1.99.1 的 llvm-tools，C 对象仍使用 Apple 工具；不忽略退出码，也不为此强制所有路线采用相同 LLVM。
3. 两个目标都采用 LP64，本样例的布局相同也是有效观察；错误 packing 必须触发断言。不要编造不同布局来证明跑过两个目标。

C 原生作者仍须保证有效指针、别名、存活与同步访问；自动绑定无法识别任意悬空指针。P02 使用有效本地对象以及明确可拒绝的空指针/超限长度，未对已释放指针作不受控调用。C05 接入 SDK 前补齐资源登记、关闭/排水、异步及独立 Lab。
