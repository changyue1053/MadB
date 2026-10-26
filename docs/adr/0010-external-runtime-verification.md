# ADR-0010 主机构建完成与目标运行验证分离

状态：已实现并通过本地集成验证；C07 已按既定候选配置完成完整本地验证并验收收口，GitHub Actions 留作后续独立专项。日期：2026-10-25。适用基线：0.9。关联 DEC-04、C07；不增加任务、阶段或认证枚举。

## 问题

C04 当前 Python 计划/证据收集、CLI 完整图判定、Build 组装和 Package 准入都假设 compile/link/run。C07 的设备安装、启动与 env 销毁必须由独立 Lab 执行，不能放入会准备执行后再重放的主机构建任务。仅新增 ArkTS 子类不足以解除这五处约束，也不能以空 run、静态检查冒充目标运行。

## 决定

使用 profile.required_features 中的必需语义功能 `core.external-runtime-verification@1`，定义两种受检的构建完成策略：

| 选择 | 主机构建提交条件 | 运行验证 |
|---|---|---|
| 未要求此功能，含仅在 supported_features 声明 | 保留 compile/link/run 三种阶段全部通过 | 现有 C05/C06 行为不变；仍不自动等于 Lab 认证 |
| 明确要求且当前参与者认识此功能 | compile/link 全部通过；Build 图仅允许这两种阶段 | 必需目标用例由独立 Lab 运行，缺失则不能认证/发布 |

策略从已校验、受摘要约束的 profile 解析；不能由 CLI 临时开关、任务列表缺 run、语言名、宿主/目标关系或可忽略 extensions 推断。required_features 已进入语义摘要；新模式形成新 profile/lock/candidate 身份，不能沿用旧验收摘要。旧工具不认识新 required feature 时必须明确拒绝，不能退回默认行为。

采用一个小型纯策略定义，供执行前任务绑定、CLI 完整图判定、Build 组装及 Package 准入共用。新模式的 Build 图只允许 compile/link，run、metadata、generate 在任何 helper 启动前拒绝；Metadata/Generate 仍通过各自独立的 Artifact 操作执行。不完整的 compile 子图可执行但不得提交 Build。保留任务/结果精确对应、退出码、phase/tool/profile/generation/hash 和全部父批次证据血缘检查。

不新增 wire 字段、任意可缩减的 phase 集合或 package BuildTask phase。既有 Package 动态输出端口真实装配 HAR 并登记摘要；设备行为不产生主机构建的伪证据。Python 默认三阶段保持，仅开放阶段计划入口，证据按已提交批次实际清单完整枚举，不能从未提交工作目录 glob。

## 兼容与版本

这是新的必需语义功能，不是无行为影响的元数据。现有功能 ID、默认 profile 字节与三阶段预期保持；未知 required feature 的 fail-closed 行为保持。首次发布含该能力的工具时按核心演进规则升级 CLI 能力集的 minor 版本并声明支持矩阵；当前开发提交不创建发布版本。只有新配置显式选择此功能，旧配置不因升级宿主自动降低完成门槛。

核心按受检能力选择策略，不引用 ArkTS、OHOS 或其他具体后端。生成器/SDK 的所有权、取消、线程、字节/项数预算，以及 Candidate required_cases 均不因此改变。主机 Build/Candidate 提交成功不授予 target-run、符合性或发布资格。

## 验证与取舍

保留原 C04/C05/C06 正反测试，新增：新模式 compile/link 提交、compile 子图不提交、run 在执行前拒绝且没有副作用 marker；仅 supported 声明不能切模式；未知功能拒绝；新旧 profile 摘要不同；两阶段 Package 接受完整父证据而拒绝遗漏/外来/重复/失败/跨 profile 证据。随后验证实际 HAR 与独立 HAP，目标未运行时仍为缺失。

该方案扩展公共执行语义，但避免了按语言分支、重复设备操作和伪造 run。可选的任意 phase 配置会允许调用方自行缩减门槛，因此不采用；在 ArkTS 适配器中绕过核心提交检查也不采用。
