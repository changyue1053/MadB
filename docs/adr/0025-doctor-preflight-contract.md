# ADR-0025 只读 doctor 前提检查与结果

项目所有者已确认“采用推荐契约，进入实现”。采用日期为2026-10-27（America/New_York）。归属I02，不改变工作包阶段、依赖或整包验收条件。

采用[具体提案](../reviews/i01-doctor-proposal.md)第1–8节与[独立审查](../reviews/i01-doctor-independent-review.md)绑定的规则。批准提案SHA-256为`60bdba6ebb41197a4d7623dcdfe4d22a150126621f746b195675cd641c96c17d`，审查SHA-256为`4980d2e412b25df7c6e02cf529d52e145bf87e30c5a6d2ecd0cdf5cd43d82055`。两份原文保留批准前结论；本决定记录随后采用，不能作为实现或验收证据。

- 新增`madb doctor --root DIRECTORY --project PATH --request PATH --snapshot PATH [--verify structure|build|run] [--json]`，沿用现有project/read-request/IR输入和严格路径规则。三档均只检查静态执行前提，不启动外部工具、安装、构建、运行或IDE，不写工程/证据/缓存。
- 新增闭合输出`madb.doctor-result/1`，复用共同文件、组件、选择器、七字段工具观察和诊断类型，不增加输入wire或摘要域。已加载工具身份、原配置字节、部分观察和完整检查结果分开。有限首片精确匹配现行0.1.1内置模板的Rust→C值接口配方，旧/未知组合准确拒绝。
- SnapshotAdapter的prepare仅提供已捕获引用；完整check、声明输入复读、锁/profile/IR及策略交叉检查通过后才能确认configuration、snapshot和recipe。MetadataRequired保留既有码与实际分类，不能据局部结构通过继续探测不可信安装路径。
- 必需检查集合、真实依赖和独立组汇总按提案固定。部分doctor事实不成为InstalledValuesTools执行凭据；固定镜像、SDK及实际资源组分别观察并在返回前重验，不把安装清单存在称作实物齐备。
- 退出0只表示所选级别的静态前提全部成立。多错误汇总为Conflict4 > Host3 > InvalidInput2 > CheckFailed1，每行保留实际类别；该优先级不改变init/C04语义。新增稳定码DOCTOR001只表示结果输出预算超限，不冒用HOST003文件读取预算。缩减结果仍为完整JSON且保留已有更高优先级错误。
- 机器模式分发、参数失败、帮助/版本与OS/输出故障例外严格按提案。不得把无法输出完整机器结果称作成功，也不将build/run级别预检写成真实构建或运行通过。

实现责任沿现有架构：protocol负责严格格式，contracts负责受检转换，app负责只读编排和必需事实，Host负责实际观察与重验，CLI为唯一组合根。独立正反例、真实只读CLI和安装漂移检查另行执行。模板选择模式、Wrapper、已有工程接入/升级/禁用、P09和I01/I02完整出口继续按原计划交付；实时状态只在catalog维护。
