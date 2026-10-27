# 已捕获源漂移分类独立审查

结论：此次三个文件的窄修无阻断，可进入新候选的实际 CLI 验证。此结论限于代码、既有定向测试及其原件；尚未证明新正式工具的端到端执行，更不代表 I01/I02/I03 整链验收。

基线为 `ffa458afd86acc5c12b40a1e6f6ab742420254dc`。作者冻结清单 `engineering-source-drift-tested-input-v1.json` SHA-256 为 `591bf8e4bc4c13668d711e795ef790e14058cd1eec7b4e537e9c77b1cbc1d07f`。独立重算其中 3 个源文件、patch 和 10 份证据的大小与 SHA，全部一致；冻结 patch 与现场三文件 diff 完全一致，独立 `git diff --check` 退出 0。精确绑定见同名前缀的 `checks.json`。

报告收束期间协调者已提交 `bf478767273069c47798bc04140185c671d93957`，父提交为上述基线；再次核对三个源文件摘要和相对基线的 patch 均未变。此集成提交不改变下述证据范围。

已依 AGENTS、工程质量规范、ADR-0027 和批准的工程重建契约核对：

- `capture.rs:86` 仅将已有捕获身份的重验转给 `verify_captured`。原 `fs.rs` 从文件开头至初次 `read` 的代码逐字未变，初次有限预算仍返回 InvalidInput。
- `fs.rs:134–154` 在按捕获长度分配前比较 Stamp，随后核对实际 FD、NOFOLLOW 重新打开的路径身份及字节。捕获后增长、缩短、同长改写和同字节换 inode 均有明确 Conflict 路径。路径消失、父目录变文件、链接或非单链接普通文件由专属已捕获入口拒绝；没有把这种分类扩展到初次输入读取。
- 非 EOF 的真实读取错误先返回 Host；EOF 只有观察到身份漂移才转 Conflict，没有漂移则保留原 Host。其他打开或 metadata 错误没有被广泛吞并为 Conflict。应用既有映射仍是 InvalidInput→exit 2、Host→exit 3、Conflict→exit 4，本补丁未修改该映射。
- 发布回归实际先成功建立旧 current，再使新尝试的作者源增长；断言新发布为 false、Conflict、旧 current 字节不变，且旧新两次 attempt 目录均保留。CAS、取消封口、安装和清理逻辑未改。

证据核对：旧真实 CLI 原件 `engineering-source-header-e2e-001/source-changed.json` 确实是 publish 的 PLAN001、exit 2、published false；修前定向原件也明确显示实际 InvalidInput 与期望 Conflict 不符。最终 `engineering-source-drift-tests-05.log` 对应作者冻结命令 `cargo test -p madb-host project_build --no-fail-fast -- --nocapture`，21 passed、0 failed；其他被过滤测试没有计作通过。新增测试使用实际文件和分段 FD 读取覆盖增长、缩短、同长、删除、替换、链接，以及真实 write-only FD 的 EBADF（伴随漂移与不伴随漂移）和无漂移 EOF。测试内部读取闭包提供可控时序，不改变生产错误分类。

本机权限观测明确为 `PermissionDenied, errno Some(13); expected Host`，并通过断言。特权用户仍可读取 mode 000 的条件分支本次没有执行，不计作该分支的运行证明。最终 `engineering-source-drift-clippy-03.log` 对应 `cargo clippy -p madb-host --lib --tests --no-deps -- -D warnings`，作者记录退出 0，原日志正常完成。

修前失败、socket 夹具路径过长、随后 sandbox 拒绝 bind、首次八进制 lint 失败的原件均仍保留并绑定；后续成功没有改写这些历史。审查仅只读源码和证据、计算摘要及执行 diff 检查，没有重跑测试、构建工具、运行目标或修改产品。
