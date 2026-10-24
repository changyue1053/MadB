# ADR 0007 技术栈版本线与 CPF 鸿蒙主线

状态：设计采用，版本候选待探针与 Lab 验证。设计基线：0.7。日期：2026-10-24。

## 背景

MadB 面向独立开源分发，既要复用互操作后端，也要隔离官方 Kotlin、CPF fork、原生绑定和工具自身的版本约束。鸿蒙 Kotlin 默认采用 CPF-KMP-CMP + akinterop；完整支持还取决于目标 SDK、生成模板、制品分发与真实消费者，不能仅按版本号相同或局部构建成功判断。

本次审查确认：Gobley 已发布组合含需要核对和修复的异步生成模板；OHOS 构建接入不等于上游插件已支持；ArkTS 还存在可复用 UniFFI 接口的实验生成器。Android 新 KMP 插件、Swift C++ 分发方式和 SKIE 对外分发也影响工程模板与验收，必须成为配置选择的一部分。

## 决策

1. **分离版本线与发布身份。** 官方 Kotlin、鸿蒙 CPF、原生实现与绑定、MadB 工具自身分别管理版本。官方与 CPF 共享契约和业务源码，但使用独立构建根、插件环境、生成目录及缓存；首版使用独立发布坐标与根元数据，验证依赖/API 交集、双编译器及干净消费者，不覆盖上传不兼容变体。[KMP 发布说明](https://kotlinlang.org/docs/multiplatform/multiplatform-publish-lib-setup.html)
2. **保留 CPF 鸿蒙主线。** KN-ARKTS-NAPI 默认采用 CPF + akinterop，CMP 仅为共享 UI 组合。KNOI 是独立扩展，不与 akinterop 隐式串联；此项替代原默认次序，保留 KNOI 的研究和测试案例。
3. **按完整能力选择 Rust 绑定组合。** 撤回 Gobley 0.3.7 / UniFFI 0.29.4 作为完整统一 SDK 默认候选。候选必须包含单次完成模板修复、协议配套与对象/异步/取消测试；仅升级 Rust runtime 不算修复。未发布的 Gobley 主线按固定 commit 与补丁身份验证，不能冒充稳定发布版。Kotlin/JVM、Swift 的 UniFFI 独立候选与修复后的 Gobley 配套分别认证，不强行统一全部后端版本。[UniFFI 修复](https://github.com/mozilla/uniffi-rs/pull/2733)、[Gobley 模板](https://github.com/gobley/gobley/blob/v0.3.7/crates/gobley-uniffi-bindgen/src/templates/ffi/Async.kt#L54)
4. **显式承担 CPF/Gobley 构建适配。** MadB 可独立调用 Gobley CLI，但上游 Gradle/Cargo 插件缺少 OHOS target 映射。官方 Kotlin 与 CPF 分别验证编译、cinterop、链接和运行；MadDemo 修改版的局部功能不形成完整后端保证。[上游 target 映射](https://github.com/gobley/gobley/blob/v0.3.7/build-logic/gobley-gradle/src/main/kotlin/rust/targets/RustTarget.kt#L55)
5. **区分 ArkTS Rust 后端。** napi-ohos 直连保留主候选，napi/derive/build/sys 与 ohrs CLI 逐组件锁定。UniFFI-ArkTS 作为独立实验后端，保留自己的 0.31.x 协议锁；生成器、模板、运行库与二进制配套，不自动继承直连后端或其他 UniFFI 版本的认证。其线程、取消与设备运行证据通过后才能扩大支持范围。[ArkTS 生成器](https://github.com/ohos-rs/uniffi-bindgen-arkts)
6. **以消费者和目标证据冻结。** 参考工程环境、候选版本、生成成功和认证配置分别登记。原生 IDE 与 CLI 共用配置，但工程生成、IDE 导入、宿主构建、目标运行分别报告。MadB-Lab 独立消费制品并验证组合链路、内核唯一 owner 和关闭行为；Beta SDK、本地补丁及局部成功不作为通用发行保证。
7. **按分发方式补全工程配置。** Swift C++ 配置选择、Android 新插件和模块装配、SKIE 可分发性及首版发布身份的具体取舍，统一遵循 [ADR 0008](0008-route-selection-and-distribution.md)，不以版本表代替工程设计。

具体候选、宿主限制与验证顺序集中维护在[技术栈与版本选型](../architecture/16-technology-stack-and-versions.md)，后端职责与选择规则见[后端接入方案](../architecture/06-backend-profiles.md)。本 ADR 不重复保存版本清单。

## 代价与边界

独立构建、发布坐标和受控后端补丁增加模板、缓存与验证成本，换取可追踪的制品身份和明确的兼容承诺。Rust 领域实现保持独立，多个导出层不能因此产生多套内核状态；生成器处理机械接线，业务语义、所有权与线程协议仍由契约确定。

十六条稳定路线 ID、既定阶段与完整能力范围保持不变。新增实验后端作为对应路线的独立配置与扩展工作包，不借机缩减必需测试或把候选改标认证。直接 ArkTS/Rust、ArkTS/C/C++ 不强制引入 Kotlin 或 CMP；失败时不能静默改用其他后端或更弱语义。

## 验证与重新评估

| 验证范围 | 关联探针与工作包 | 冻结条件 |
|---|---|---|
| CPF + akinterop + 正式 SDK | P05 / C07 / I05 | 公开依赖、干净构建、真实设备与 Release/HAR 消费 |
| 修复后的 Gobley/UniFFI 与 CPF | P03 / C06；组合关联 C07 | 模板单次完成回归、补丁身份、OHOS 构建适配及完整对象/异步/取消能力 |
| 双构建线、Android 与 IDE | P11 / I05 | 独立发布坐标和元数据、AGP 新插件、原生库装配与受支持宿主导入 |
| Swift 增强与 C++ 分发 | P04 / P07 / P12 / C06 / C08 / X01 | 生成归属、可分发 Framework、公开 API 可调用性及三类 C++ 配置的方向验证 |
| 语言便利层与组合应用 | P12 / C09 | 普通 API/便利 API 一致、共享内核唯一性与完整关闭协议 |

X07 承接 ArkTS-UniFFI 实验扩展，可提前开展可行性探针，记录明确的 UniFFI 协议锁和运行限制；真机任务不得以跳过、仅编译或缺少运行日志通过。任何默认配置的冻结或升级都要求对应制品摘要、用例集合、目标环境与结果证据。失败保留最小复现，再调整明确配置，不自动切换技术路线。
