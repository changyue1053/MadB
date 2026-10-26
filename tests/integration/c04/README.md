# C04 只读查询验证

范围：C04 的查询/静态计划子集，非 SDK、执行器、缓存或事务验收。测试使用 C02 人工契约 fixture，源文件仅作为字节输入；它不实现 IR 描述的完整 API，也不冒充编译器生成的元数据。

在仓库锁定开发环境中运行：

```sh
cargo test --workspace --all-features --locked
.venv/bin/python -m unittest discover -s tests/integration/c04 -v
.venv/bin/python -m unittest discover -s tests/architecture -v
```

Python fixture 独立计算真实配置/文件引用和任务摘要，不用生产查询代码生成预期结果。CLI 在临时工作区运行，PATH 仅含失败即留痕的编译器替身，HOME 也隔离。调用前后比较目录项、文件内容摘要、mode 和 mtime；不比较读取可能改变的 atime。所有读取结果包括失败路径均不得产生写入或启动编译器。

覆盖：成功 inspect/check、符号与策略来源、缺失/过期快照、源码变化、配置字节篡改、schema/语义错误、未追踪输入的能力提示、静态图顺序和命名子图、循环/递归/重复输出/错任务摘要、跨 target/profile 工具拒绝、父/叶符号链接、硬链接、FIFO、超限文档及只读权限工作区。端口测试另用确定性触发点覆盖查询期间变化、缺失文件出现和重复任务路径的读取前拒绝。

静态图允许独立 provider 的 compile → metadata → generate，不使用阶段名称替代真实依赖关系；拒绝有环图、重复节点/依赖、输出别名和祖先路径冲突。任务参数仍是被校验的声明，不实际运行，也不授予提交/发布资格。

当前查询范围仅验证已声明输入，缺少完整依赖发现时不能用于命中生成缓存。计划只处理显式任务文件，尚无自动 backend 计划；缺少真实 source-annotation reader 通道时拒绝相应策略。Unix 宿主实现有界读取，Windows 宿主实现和平台验收未完成。

本套件不覆盖 CRIT-05 的 `generate --check`、CRIT-06/09 的取消/事务/恢复及 CRIT-10 的缓存失效；这些检查由 [C04 执行、存储与缓存套件](../c04_package/README.md)承担，其证据边界以该套件说明为准。
