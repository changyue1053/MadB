# 集成后 E03 重放保护：限定只读复核

结论：本次重放及证据保护范围内未发现阻断。实际根为 `/Users/changyue/Desktop/MadB/madb`，HEAD `e26a73b6bf8cbd9c6ffded3240ebb6f9e49a09c4`，状态干净。没有执行 SDK、peer、Cargo 或修改产品/旧报告。

独立比对了全部 1401 个已跟踪文件的 SHA256、大小、文件模式，与固定 receipt 所引用的 `source-inputs.json` 一致。13 个镜像的 SHA256、大小和绝对路径与 receipt `f312f465260147733d6cf3f802d72819f14b653bb2b60b817fa81c67e8a26977` 相同；全部实际为 `0o500` 普通文件、无符号链接。receipt 没有 mode 字段，因此模式/链接只能准确称为实测前后不变，不能称为与 receipt 的 mode 比对。两个成功输出的 before/after 全文相同，且与当前原字节再次核对一致。

`post-integration-e03-001` 实际 exit 0、31 项通过：直接调用已提交的原 checker，没有 `--positives-only` 或错误转成功分支。逐项核对了原 31 名称集合、负例原始 stderr 的精确类别、两项真实 reader 拒绝无生成/提交、三种成功配置的真实双阶段清理、上下游 IR 字节与持久 Source 对账。`post-integration-e03-legacy-002` 实际 exit 0，仅沿用既有 `check_run` 和 source consumer 的普通/增强/禁用三成功配置，保留 `@1` 的 5/8/5 责任与 15/17/15 payload 范围；没有扩大为 `@2` 完整责任或 E02 故障矩阵。

最初 legacy-001 的 `changue` 路径拼写错误确实在首次 git 子进程因工作目录不存在而退出，目录仍空、FileNotFoundError 原日志保留，没有目标动作或通过记录。Python 和 tool-observation 仅绑定运行前后实际字节；本复核不声明其全部隐式依赖闭包，也不构成 SDK/HAR/目标/物理装配或 E03 整包独立验收。

可重复的只读检查脚本：`e03-post-integration-guard-review.py`。结果：`e03-post-integration-guard-review.json`，SHA256 `87fd5b1bca3d05d256f7ab08f01dfe588db37bf3ad38af6f14457ea352c0f6ad`。
