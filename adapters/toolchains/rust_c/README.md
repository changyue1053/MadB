# Rust ↔ C 首链工具链

本目录连接 C05 的受限语言适配器与 C04 冻结执行端口。实际读取与生成在 [generators/rust_c](../../generators/rust_c/README.md)，候选准备在 `adapters/artifacts/rust_c/pack.py`；核心规则、事务和 SDK 生命周期分别由既有责任层维护。

## 公开入口

在 MadB 根目录使用固定开发环境：

```sh
.venv/bin/python scripts/install_native_tools.py --root /absolute/path/to/native-tools
.venv/bin/python scripts/build_first_route.py \
  --output /absolute/path/to/candidates \
  --bindgen /absolute/path/to/native-tools/bin/bindgen \
  --cbindgen /absolute/path/to/native-tools/bin/cbindgen
```

安装与输出目录必须不存在。`toolchain.json` 固定生成器来源、归档及 Cargo.lock 摘要；安装器以锁定 Rust 执行 `cargo install --locked`，成功后写入实际镜像 receipt。原 C03 安装入口转发到同一实现，已有 receipt 兼容。

构建默认要求干净 Git 检出。仅开发验证使用 `--allow-dirty`，脏树候选的 provenance 带 `provisional: true`，正式 Lab 验收拒绝这类产物。构建工作区独立保存冻结输入、执行日志及三个提交层，不写回 SDK 手写源码。

## 流程与产物

`tools.py` 观察并验证工具镜像；`project.py` 准备源、project、profile、ReadRequest 与 lock；`pipeline.py` 通过正式 CLI 端口执行 metadata、generate、build、package；`worker.py` 承担实际 compile/link/run。C04 静态 DAG 的后继输入必须有精确字节身份，因此先显式 prepare 输出，再由 C04 重新编译、链接、运行并校验。prepare 不提供独立预期或验收证据。

| 候选 | 公开内容 |
|---|---|
| `c-to-rust` | 生成 C 头、Rust engine 动态库、接口/profile 身份；未插桩 Release |
| `c-to-rust-test` | 独立 profile、测试头及测试库；barrier、故障注入与资源计数 |
| `rust-to-c` | C 头与静态库、公开 `madb-c-client` crate、真实 bindgen raw 声明及安全包装 |

每个候选有独立 `candidate.json`、身份、锁、来源及构建证据。SDK 原生构建单元不依赖 CLI、app 或工具私有 crate；工具端 `madb-native-finalize` 只负责纯元数据完成，不随 SDK 运行。Lab 在独立检出中使用 `scripts/check.py --first-route --candidates PATH --output PATH`，只消费公开候选，不重生成绑定。

## 配置与平台边界

`profile.backend_config` 保持 `null`。内部 `backend.json` 作为 ReadRequest/lock 的精确输入包含工具定位和参数，由首链适配器验证其固定版本；通用 CAP005 拒绝规则保持不变。公开 `toolchain.json` 仅保留工具镜像、SDK 内容摘要与逻辑名称、编译器语义；宿主绝对路径和执行参数留在内部工作区。

Apple 路径严格核对冻结 Xcode/SDK/Clang 配方。Linux 使用实际 native host 的工具和 SDK 观察生成独立 profile；必须在匹配目标的消费者上运行，各平台证据不能互相替代。生成器只接收首链已知 roles 与真实编译器声明，不是通用 AST 生成入口；异步、队列、stream 及未知导出按当前能力拒绝。`inspect/check/plan` 仍是只读查询。
