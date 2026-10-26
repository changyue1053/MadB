# ADR-0022 自然 Rust 有限值配方与普通 C 入口

状态：项目所有者已确认“采用有限值配方，进入实现”。日期：2026-10-26（America/New_York）。关联 L02 的既有第二独立业务出口及 DX-01/02/11，不新增工作包或目标承诺。

采用[有限值 ABI 提案](../reviews/l02-value-abi-proposal.md)第1–8、10节的完整规则，提案 SHA-256 为 `9a89e9308a7b95c7479f046bc10224a30f2962dd589ad30a97a90923a2ffdcdf`；[独立审查](../reviews/l02-value-abi-independent-review.md)无剩余设计阻断。提案原文的“待批准”及第9节的未实现结论保留其历史快照，不代表当前决定；实现、编译和独立消费继续分别留证。

- 自然 Rust 单模块是唯一作者结构来源；syn 2.0.119 仅解析有限形状，锁定 rustc 1.99.1 检查实际签名、字段和 enum tag，生成 shim 后使用 cbindgen 0.29.4 与实际 Clang 核对物理头、布局和错误常量。作者不维护 FFI facade、字段 IDL 或逐方法映射表。
- 配方限制为1–16个具名公开字段，字段类型 i32/u32/String，String 边界为64字节严格 UTF-8；函数仅同步 `(R)` 或 `(R,i32)` 返回同一 R 或 `Result<R,E>`，E 为明确正 i32 tag 的无 payload enum。所有具体拒绝项按提案执行，不因 syn 可解析而自动支持。
- 采用局部语义版本 `madb.native-values/1`。逻辑模型复用既有 IR、Variant/throws、功能与 profile/1；不新增共同 schema/字段/feature，不打开 backend_config 注册表。旧 `madb.first-route/1` 维持自身精确声明集。
- C 普通入口由同次身份生成，依次执行 bootstrap、interface、profile、raw 调用与输出协议核对。String 的内联表示、尾部清零、错误域/tag、单次转换/作者调用、失败输出及原生指针有效内存前提，均按提案固定，不借用旧资源句柄状态或诊断 API。
- 现有7/14/9/16导出、configure/drain、checked engine constructor及冻结候选保持原契约。新值配方没有资源、async、回调或 C++ RAII 承诺；消费者不依赖 MadB CLI、核心 crate 或生成器。

每符号 required 责任从受检 IR/有效政策建立，provided 从实际生成机制建立，复用核心比较器与 Source/Build/Candidate 资产绑定；不为纯值捏造订阅或排水。实现需完成作者0→1增方法→2改字段→3重命名→4删除的真实生成与独立消费，以及提案第8节准确正反例、旧配方兼容回归和原生转换独审。实现顺序与资源由主协调决定；状态仅写 catalog，不授予公开发布或 Actions 执行资格。
