# C04 只读查询与静态任务图子范围

日期：2026-10-24。依据：设计 0.9，C03 基线 `e58dd3448713e6c1ff637d237f89a41b7a8b48dd`。分支 `codex/c04-application-runtime`，基于尚未合入的 C03 分支；未合并 C02/C03。任务 C04 保持进行中，本记录不构成整体验收。

## 范围与行为

app 定义读取和格式端口，contracts 负责已有 schema 到领域值的转换，host 负责目录描述符相对的有界只读 IO，CLI bootstrap 负责装配。plan 提供确定性任务图、依赖子图、循环/递归与受管输出冲突校验。未创建无用途的生成器/执行器空实现。

`inspect/check` 校验已有 request/project/lock/profile/snapshot 与已列文件字节，显示结构和策略来源；`plan` 静态校验显式任务文件。缺少或过期快照返回 metadata 前置任务，调用期间输入变化返回冲突。读端口没有写入和进程操作，宿主也不隐式修缓存。

成功状态明确叫 `TrackedInputsMatch`：只代表声明清单匹配。没有完整编译器依赖闭包、注解 reader 通道或执行结果时，不能宣称源码新鲜性、工具安装、二进制一致性或 SDK 认证。metadata 执行、三层提交、取消/CAS/恢复、缓存和 `generate --check` 保留在 C04 后续清单。

## 独立审查与修正

采用独立差异审查，发现并修正：重复任务文件可通过缓存克隆放大内存；Phase 全局次序误拒绝独立 provider 编译后读取 metadata；任务工具错误借用全局锁中其他 target/profile 的条目。分别增加读取前去重、合法/有环图、跨 target 与同 target 不同 profile 的回归。

CLI 接入 contracts 后，旧“adapter 导入 CLI”负例形成真实 Cargo 环。架构门禁新增 `cargo metadata --no-deps` 声明预检，再执行完整锁定解析；负例仍要求准确的 ARCH003，不能把任意 Cargo 失败当通过。新增 app→host 与 host→contracts 负例。旧 CLI 安装 smoke 的帮助/未知命令预期按新增子命令更新；generate/build/verify 仍必须失败。

## 验证与未完成项

复现命令与场景见[验证入口](../../tests/integration/c04/README.md)。本地 `.venv/bin/python scripts/check.py` 全部通过：47 个 Rust 测试、13 组真实 CLI 查询、9 个架构正反例，加上既有 5 个格式/ABI、6 个 C03 进程和 21 个文档测试，共 101 个测试函数；另有共同格式/摘要向量、隔离 Release 安装、13 个生成 HTML 的一致性与 DOM 替身检查。P02 原生探针未因本次纯宿主改动重跑，不新增原生证据。

独立审查最终复核未发现剩余阻塞问题，并用 CLI 单独验证三项修复。`git diff --check` 通过。远程 CI 以交付 PR 的当前 head checks 为准，本地结果不替代远程结果。

没有修改 MadB-Lab、原生运行时或旧验收记录。Unix 读取实现不证明 Windows 可用；二次读取是冲突探测，不是操作系统原子快照；文本查询结果不是新增公共 JSON 协议。HTML 仅从 catalog 重新生成，不声称新增浏览器视觉验收。
