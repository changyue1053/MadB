# 工程日常重建真实运行独立审查

结论：本地候选 `0007f8a55510037d055f59411ba9e879b421a02c` 的下列成功/拒绝场景有实际运行与字节证据，但 **不能判为 ADR-0027 §8 全部通过**。有一个确定的错误分类缺陷：捕获后源文件增长被报为 PLAN001/exit 2，批准语义要求 Conflict/exit 4。安装 SDK 实体错身份等缺项也仍保留。此结论不赋予 I01/I02/I03、I06、P09 或 IDE 整包验收；不包含发布、远程 CI 或集成后复验。

## 范围与依据

任务为 I01/I02/I03 的显式模板 0.2.0、固定 macOS Arm64 Rust→C 同步值日常重建运行子范围。实际读取产品 AGENTS、开发/质量门禁、catalog 的 I01/I02/I03/I06 条目、`engineering-rebuild-proposal.md`（尤其 §8）、freeze002、源码快照及以下原始运行报告。当前 catalog 的整包条件和前置不变。

本次只读产品、Lab、候选与运行目录；只写本报告、同前缀检查脚本和证据绑定 JSON。独立检查脚本仅用 Python 标准库，未导入 MadB 私有实现。采用 verify-and-stop：不增加范围、不改产品、不改期望、不重新执行原生构建。原始失败和后续修正分别保留。

## 候选与证据身份

- 冻结候选根：`/private/tmp/madb-chief-round12/engineering-candidate-001/repo`。Git HEAD 为 `0007f8a55510037d055f59411ba9e879b421a02c`，实际 `git status --porcelain` 为空。逐项核验 1548 源文件字节与 mode，并核验 freeze002 的 46 工程文件。
- 工具：`engineering-candidate-001/target/debug/madb`，实际 61,608,040 字节，SHA-256 `8938727e5cee5abf7e06f8005f47632bcece3681dc7112be7c7607c168140a36`。报告中的版本为真实内部 `0.1.0`，源码身份同上；构建日志记录九个 workspace crate 的实际编译及正常结束。
- `source-snapshot.json` SHA-256 `2f7553ff353cb744486bda5f9cce07431ade4b31f5105c1eef8788f94330e808`；`build.log` SHA-256 `f3a5ee173f99415994a9e2204f7c47eb6b289df4a2f92d32a914805fc942cb9e`；均与收据一致。
- 旧模板 0.1.0/0.1.1 的原包、模板 manifest、identity 六文件，实际逐字等于基准 `2fac2f7437e7bc235392fcca8c5d7af86b510e22`。此处没有以开发工作区后续修复替换该候选的证据。

`engineering-runtime-independent-review-check.py` 已实际执行。绑定文件包含 39 条核验记录（含元数据/辅助观察，**不是 39 项测试**）及 248 个文件绑定；字节/摘要核验未发现不一致。对九份成功发布结果独立核验：stdout 与报告一致、实际退出码一致、捕获输入副本、全部日志、各 19 个完整输出、物理 Candidate 库存、原生 compile/link/run 任务与 BuildEvidence 对应、真实 work 目录中的日志/输出，以及链接动态库到发布候选的字节关系。按批准域和 UTF-16 键排序独立重算请求、输入快照及输出身份；保留的 previous 结果记录与旧产物也逐字核验。

## 已读取的真实场景

| 场景与原件 | 实际结果及边界 |
|---|---|
| `engineering-e2e-001/observations/cold-init-run.*` | 新中文空格目录真实 init/run exit 0；保存的嵌套 project/build-result 与日志闭合；普通 C 输出 `7:x`。 |
| `wrapper-no-change.*` 与 `wrapper-no-change-consumer.*` | `madbw build --frozen` 实际完整重建、发布新结果，消费者 exit 0、`7:x`。strategy 证据声明 conservative-full-build/cache_hit=false；不解释为缓存命中。 |
| `rust-implementation-refresh.*`、`consumer-refresh.*` | Rust 实现 +1 后真实重建；原消费者预期 7 的运行 exit 1，更新消费者预期后实际 `8:x`。实现变化时契约身份保持，不能将此扩为任意源码模式支持。 |
| `compatible-addition-002.*` | 新自然 Rust `Reading -> Reading` 方法 `twice`，契约身份变化、公开头声明/普通包装均存在，实际 nm 导出 `_reading_demo_raw_twice`；消费者输入 6 并断言输出 12，实际 stdout `8:x twice=12`。 |
| `breaking-field-old-consumer-002.*` | 字段 value→amount 后，实际 Clang 报四条 no member named 'value'，consumer-link 失败、exit 1、未发布。保存的失败后 current 观察仍引用上一完整结果；观察的 current SHA、旧记录 SHA 及实际旧 19 产物均核验一致。 |
| `public-field-refresh-002.*` 与 `public-field-consumer-002.*` | 更新普通 C 消费代码后真实重建发布并运行 `8:x twice=12`。它是前述字段破坏的修复重试证据。 |
| `engineering-source-header-e2e-001/headers-added-retry.*` | 增加递归头目录后实际 9 个捕获文件（含 bridge.h 与 nested/values.h）；完整重建成功，实际 `7:x`。 |
| 同目录 `indirect-header-changed.*` | 两轮捕获输入只改变 `consumer/include/nested/values.h`；快照和结果摘要均变化，实际消费者 `9:x`，exit 0。无其他源码修改代替间接头失效。 |
| `engineering-concurrent-e2e-001/build-{1,2}.*` | 两个真实 CLI 从 absent current 启动，独立尝试根；结果分别 4/STORE003 与 0，只有成功者发布，失败者 result=null，旧/失败尝试保留。保存的 current 指向赢家且其记录/输出仍逐字吻合。仅这一真实交错，不代表穷举并发或完整 P09 子进程计数。 |
| `engineering-cancel-e2e-002/{started,barrier,observation}.json` 及 stdout/stderr | 真实直接子进程 values_worker 被观察到后向自有 CLI 发 SIGINT；实际 exit 130、未发布。原始 process 日志记录 SIGTERM、leader_reaped/stdout_eof/stderr_eof=true、group_kill_attempted=true、signal_failed=false、pending_cleanup=false；之后精确已知子 PID 观察为不存在。首次无 activity barrier 的 cancel001 仍只算失败的探针，不替代本例。 |
| `engineering-install-e2e-001/observations/*` | 真实 structure 创建；已有精确镜像 install exit 0、错镜像 install exit 4/STORE003、错镜像 Wrapper exit 4、缺镜像恢复 exit 0。最终工具字节与候选相同。inode 保持/错镜像未覆盖目前只有当时 inline harness 汇总布尔，未保存当时 before/after stat 原件，故不提升为本审独立复现；作者正在另建 002 原始重放，须另审。 |
| `engineering-rejections-e2e-001/observations/*` | 未知任务、未知参数、缺 frozen、Wrapper 未知任务、修改 profile/toolchain/origin、额外 Rust 源、旧模板缺 descriptor 的九种拒绝均有原始退出/流。旧模板当前准确记录 Host3，不冒称特殊格式诊断；modified-toolchain 只证明声明文件漂移拒绝，不证明安装 SDK 实体错身份。 |

## 确定缺陷

**P2：捕获后源文件增长被误分为输入预算错误。**

原件为 `engineering-source-header-e2e-001/source-changed.json` / `.stdout`、`source-change-barrier.json` 与 `source-change-partial-observation.json`。观察屏障位于 captured source、contract、build-execution 已存在之后；随后仅向作者文件追加注释。实际 capture→consumer-link 均 passed，publish failed，`published=false`、`result=null`，诊断 `PLAN001: file exceeds the finite byte budget`，进程及报告 exit 均为 2。旧 current 保持且作者原字节由 harness finally 恢复，但错误分类未达到已批准的 Conflict4。

在冻结候选 `adapters/host/src/project_build/capture.rs:86`，`verify_sources` 用捕获时的 `bytes.len()` 作为 `fs::read` 的上限；`fs.rs:105` 的超长拒绝发生在字节/身份比较前。因此已捕获文件增长先落入新输入预算错误，未进入 captured input identity changed 的冲突分类。缺陷已即时通知根协调者及工程作者；本审不改代码、不放宽期待。需窄修、独立复核、冻结新源码和真实镜像，在新候选重放正常重建及增长漂移；原失败保留。

## 未完成及不可外推部分

1. 安装 SDK 实体错字节/缺失尚无真实场景；禁止把 toolchain JSON 修改或项目本地 MadB 镜像错字节替代此项。后续若损坏已发布 libreading_demo.dylib，只能证明旧 SDK 产物完整性拒绝，仍不等同安装 SDK 的准入检查。
2. Wrapper 固定源码实际以 `env -i PATH=` 启动 CLI，当前真实 Wrapper 场景与该字节绑定；尚未单列外层空 PATH 启动的原始环境记录。已有本地完整输入运行与无下载执行路径，不等同网络被物理隔离的证明；不能声称已完成所有离线/缺工具矩阵。
3. 本审的活动进程取消覆盖一次明确 barrier；未补造阶段间或发布尾窗的真实运行证据。既有模型/定向单元检查和其它独审应保留各自范围。
4. 原 fixture 使用 C 保留词 double 及不在有限配方内的标量签名，其失败原件保留。002 修正为已批准的 record 方法并以独立 6→12 断言证明。首次安装 fixture 尝试写受保护镜像的失败日志也保留，之后只修改自有 scratch fixture。没有把任意失败计为预期成功。
5. 这里的 wall_seconds 是对应 harness 的观察区间，并发两个进程的串行收取退出尤其不能作为完整阶段耗时。没有采集完整子进程启动次数，不能据此宣称 P09 或性能结论。
6. 冻结源候选属于本地开发候选，未代替实际集成提交。最终集成后仍须从干净实际提交重建工具并重验适用场景。真实 IDE 导入、索引、GUI Run/Debug、已有工程升级/禁用、其他平台和完整工程链仍按 catalog 等待各自条件。

## 可复核入口

检查命令：`python3 /private/tmp/madb-chief-round12/engineering-runtime-independent-review-check.py`。

紧凑证据索引：`/private/tmp/madb-chief-round12/engineering-runtime-independent-review-binding.json`。索引绑定原件路径、字节长度和 SHA；原始任务失败、缺项与上述审查语义不能被 `integrity_issues=[]` 误读为整体验收通过。
