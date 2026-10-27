# I02 doctor 具体契约独立审查

审查对象：i01-doctor-proposal.md，SHA-256 `60bdba6ebb41197a4d7623dcdfe4d22a150126621f746b195675cd641c96c17d`；源代码依据 MadB `9a72a6417858b7ee1a141ce8b13fa47ba7ef2f94`。主协调未参与初稿编写，实际阅读完整提案和现行 SnapshotAdapter、query、CLI MetadataRequired 映射、0.1.0/0.1.1模板及固定安装身份设计。

结论：作为待项目所有者裁定的具体设计，无未解决阻断。不能据此登记实现、运行或I02整包通过；本次未运行新schema/decoder或doctor命令。

## 修订与关键核对

1. 沿用project/read-request/snapshot和只读根，不以InitResult伪造创建，也不以Report伪造Candidate/Lab。新增独立doctor-result/1及唯一DOCTOR001输出预算码均明确列为待裁定公共差异。
2. 原稿对MetadataRequired和部分配置观察不够精确；修订后META001及现有LOCK005/PORT002转换分支为invalid-input/2，完整configuration仅在SnapshotAdapter.check及现有完整校验后通过，缺IR时阻断recipe/环境依赖，不以prepare局部校验授予通过。
3. 实际main镜像与配方身份对应同一已加载工具，复用既有VALUES_RECIPE、ToolIdentity和TrustedTemplateRecord；未取得身份时允许null，但不据null猜错误类别。0.1.0与0.1.1原profile确有独立id/工具/SDK摘要，能准确区分，不用模板名猜测字节。
4. scope恒为preflight-only，三档仅检查对应执行前提，不运行工具--version、构建、安装或SDK。工具版本为固定收据和实际字节匹配的观察，错字节不能继续显示预期版本；部分doctor观察不能成为InstalledValuesTools执行许可。
5. 新汇总优先级限定doctor，保留每行真实类别；预算超限新增Host事实而保留已有Conflict优先，不改init/C04错误归因。truncated摘要、JSON机器分发和解析前/输出故障例外具体，不能输出截断JSON或0。
6. schema严格性、必需集合、blocked依赖、不同检查级别、旧/未知profile、缺工具/错SDK/漂移、只读/无子进程、输出预算均已列正反验收。独立受控端口和真实冻结工具检查仍是实施后的工作，不能由本审查代替。

## 收口边界

本次仅建议批准新增入口参数、输出格式/诊断码和上述汇总规则。doctor归I02、默认只读及现有拒绝责任已获批准，不重复裁定。模板模式、正常Wrapper、已有工程升级/禁用、完整P09以及I01/I02整包出口保持原要求。不存在需要安装工具、执行命令或修改冻结记录才能批准该提案的前置动作。
