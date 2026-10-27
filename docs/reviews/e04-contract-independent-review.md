# E04 最小公开契约交叉独审

结论：冻结修订2无剩余公开契约阻断，可交用户决定是否采用其普通 Dart API、单目标同步资源映射与有限配方。本结论不是实现验收、E04 完成、扩展注册或发布批准；包/profile/组件身份及默认 1000ms 预算仍属于待决定内容。

绑定提案 `/private/tmp/madb-chief-round12/e04-contract-proposal.md`，SHA256 `c9e7c0ea1bb7771d171e3ff3b67b9a7c72836fa63af0803a1071b0f44d10ebb4`，18871 字节。已读取绑定公开 C05/L02 候选的 header/contract/profile/identity、第二自然值候选的公开头与 contract、原 P10/值/加载机制原件及相关原型源码，并核 ADR0018 边界。未导入产品私有实现或运行目标。

公开契约的关键边界已明确：

- open 接受受检 packageDirectory，核清单/路径/字节并在私有目录加载 bridge；六个业务与关闭函数实际来源必须是同一候选 native 镜像。所有调用经同一 bridge，不把 identity64 当作精确 image 身份，也不混用 bridge A 的 handle 与 native B。
- 本独审发现的 open/checked-create 歧义已关闭：公开 `madb_engine_create_checked()` 校验后会立即创建并转移 handle；最终稿规定 open 仅用同实例 public bootstrap 做 identity-only 预检，createEngine 才调用 checked-create。无隐式预创建或丢弃 Engine。
- 包装开始 close 即进入 closing 并停止本地业务准入，逐步 request→await→release，全部成功才 closed；任一步失败保留 owner 与准确 stage，不自动回 open、不提前 release。closing 不假称 native request 已确认。加载、身份、原生调用和返回契约违例的异常分类及字段一致；Dart 校验错误不伪造 native 状态。
- 单 isolate 不等于固定 OS 线程。保留 DynamicLibrary 的 owner 拒绝跨 isolate 转移，FFI 沿当前调用线程执行；原公开 contract 的 caller/release_thread=caller 不被扩写为创建线程亲和。已有 SendPort 拒绝只证明固定 SDK 下的该次检查，正式 JIT/AOT 边界测试仍须完成。
- bounded checksum 才承诺可精确返回 Dart int；未知 u64 范围拒绝。i32/u32、严格 UTF-8、内嵌 NUL 和有限值 record 保留原约束。第二值模块真实需要的 core.errors/core.variants 已明确保留；不能从 C05 完整 callback contract 静默删 feature，而须建立显式受检同步选择并保留原候选身份/血缘。
- 顺序重复 open 限于本固定目标、固定包且逐次核图；不承诺任意版本并存或并发 open。私有缓存不会被 Dart 退出自动删除，最终稿明确由宿主在消费进程退出后按精确创建目录清理，SDK close 不冒充库卸载或文件清理。

实际证据核对：

| 原件 | SHA256 | 独审核到的事实 |
|---|---|---|
| e04/p10-003/result.json | 056bc900c8cf159e1660c0c1f37d8782e40d2faf3939c98f0fcbdbe6a5bd24d2 | 11 命令 exit 0；25 条声明输出/产物引用、65 项输入引用匹配，before=after。JIT/AOT 普通调用、精确 interface kind=2/profile kind=3、显式 close 和重复创建关闭。 |
| e04/values-001/result.json | 732f96cd8406999d1d8b8015fbb6421c621ae272b2b72d0ec2d7e26d6bfe249a | 5 命令 exit 0；12 条声明引用、24 项输入引用匹配，before=after；两种执行模式各有 8 组布局/值/Unicode/范围/领域错误与清零/恢复观察。 |
| e04/loader-003/result.json | 71777a22ec0b9eefcbe676ae1c68bce648d0120a6a71452bf712e5503121ec23 | 20 命令 exit 0；42 条输出/产物引用、24 项输入引用匹配，before=after。JIT/AOT 的六个 dladdr 来源同一私有 native，字节匹配原候选；同身份外部镜像、错字节、symlink、escape 按精确类别拒绝。 |
| e04/loader-repeat-001/result.json | 77084832cc7e3c5c7b4f2c13513de53a04579bfa6b142fa47d29b298a36a6253 | 3 命令 exit 0，9 条源/日志引用匹配；源代码在一个 main 内依次调用两次普通加载。JIT/AOT 各两条 normal 观察，两个不同 staging；每条六个函数都落自身 native，四份 native 字节均等于候选。 |
| e04/generation-001/result.json | de2a51e6dafed190bfc0929ebaeb36df8aa462a26595c658b283027eb2a2a576 | 5 命令 exit 0、12 条声明引用匹配；记录有限 raw 声明重复生成同字节，实际 JIT/AOT 输出 checksum 上界 16711680 与 request-await-release。 |

资源候选 digest 为 `sha256:9a09515422305c2bf09a452c68d6afe373c27e277b510509f986c55395446959`，native 文件 SHA256 `9aa3fb2ff532a830f6ab1df48956b410d473b8b250cfc86cd6eff1eb50d1136e`。第二自然值候选 digest 为 `sha256:732698ef20ec4354586203229395ed65f5394faa4efe0a3fa477ff24407ccb57`。两库保留独立身份，未拼成统一 ABI。

证据限制仍成立：p10-003 的双 open 原型不证明最终加载装配；loader scratch 不等于完整 loader/异常实现；ffigen raw 声明与手工普通包装不等于正式 reader/generator 双公开会话、资源生成或注册完成。正常快速 close、早于 request 的 Busy 拒绝不证明真实 Timeout、并发关闭或回调排水交错。正式包、OutputSet/字节与生成血缘、独立业务新增/改名/删除、禁用/升级隔离、干净 Release 消费及适用 EXT/Lab 退出仍按提案完成；若现有 wire 无法表达受检选择，须报告具体缺口，不能悄悄改核心或降格旧 profile。

本次只新增本报告；未改提案、产品、原观察，未运行重复目标、构建或新增子代理。历史失败原件保留，最终结论仅为“该冻结契约可交用户决定”。
