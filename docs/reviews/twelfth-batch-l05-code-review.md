# L05 background lifecycle 实施代码独审（冻结前）

**最新 delta 结论：下列两项 P2 均已按窄修关闭，当前十文件未发现剩余阻断，可进入最终冻结后的实际构建/目标验证。build002 不代表最新源码已编译或目标通过。**

以下保留初审发现与原始输入，末尾追加关闭依据。

结论：本次读到的十文件版本有两项 P2，应在正式目标运行前窄修并复核 delta。其余指定生命周期、输入准入和报告边界未发现可确认阻断。本报告是静态代码审查，不授予编译、Native/UI、后台执行或全 L05 通过。

**P2：完整 UI 预期必须来自同一张 hierarchy。** `harness/l05_background_lifecycle_execution.py:133–138` 的 assert_phase 对 phase/page-id/value/ordinary 分别调用 wait_text，每次都重新捕获 layout；先匹配的字段不会在后续快照复验。因此可能没有任何一张画面同时满足 UI_EXPECTED，却分别接受四次瞬时匹配。改为每轮捕获一份 layout，在同份数据内检查全部字段，全部满足才返回该快照；保留现有精确等值、唯一节点和截止时限。补一个字段交替正确、从未同时正确的负例即可，不需扩大测试矩阵。

**P2：完整关闭后的合法生命周期回调不能污染完成证据。** `ApplicationOwner.ets.in:62、115` 的 foreground/background 总先调用 healthy；完成后它会因 finished 抛错，由 EntryAbility 记 L05_BACKGROUND_FAILED。runner 在完整 parse 后才 aa force-stop，若清理触发合法 onBackground，失败行会出现在 COMPLETE 之后，却不再进入这次最终 parse。建议仅在全部 public close 已成功、finished=true 后让这两个回调直接返回；不能在 finishing、close 未完或普通 failure 时放宽检查。pageDisappeared 已接受 finished 后的正常退出，可保持。

已核的正确边界：

- 初次 foreground 与页面 bindPage 通过 startIfReady 一次性交接；creates/pages 检查拒绝重复实例，Runtime/Engine/普通 observer 仅初始化一次。其后同一模型在 close 成功后重新 attach，没有另建桥或观察队列。
- 真实 UI 按钮只调用 moveAbilityToBackground，只有实际 UIAbility 回调调用 background/foreground。background 同步 requestStop；即使其抛错仍保留 drainAndPublish Promise，完成 close 后重新抛出原错误。完整异步链由 owner 保留并记录拒绝，没有将同步生命周期 return 冒充系统等待。
- 后台 close、普通 observer 实际收值前后检查 foreground 状态和代次；保持旧模型最后前台值，并用同一 Engine roundtrip。runner 只有观察到对应 background-published 后才 aa start；恢复 await 原后台任务后复核代次/状态，再 attach 一次。每次 READY/EMPTY 在下一次 Publish 前由 UI 检查，需以上述同快照修复强化。
- 最后以 PUBLISHED_3 和精确计数为前提，join 最新背景/恢复任务，再依次关闭页面订阅、普通 observer、Engine、Runtime。此前轮次由完整后台证据和 READY/PUBLISHED 门槛串行完成。全部 close 成功后才 finished/owner-closed/COMPLETE；此次代码未声称 GC、原生严格资源计数或迟到回调注入。
- 独立 oracle 固定 21 个有序 frame、9 个 UI phase、七次普通值 `[1,2,11,12,21,22,31]`，拒绝错误 nonce/PID/tag、重复/缺失/错值/乱序和完成后观察。背景发布明确在确认排水后，README/report 没有把它当作已排队迟到回调证据。
- Python 独审覆盖 source/candidate/tool 前后核验、实际 HAP native closure、构建与运行阶段、限定 bundle cleanup。build-only 不调用 execute；target/native/ui 与 runtime_close、强制停止/卸载分别记录，cleanup 失败会保留失败，不能冒充 public close。

prepare 显式设置 EntryAbility launchType=singleton；协调者已报告 build002 的实际产物相符。本审不将 build 未自动复验该字段另列冻结阻断：运行期 creates/pages 与 PID/nonce 仍拒绝意外重建。可窄加产物单例字段断言增强绑定，不需新 schema 或平台矩阵。

审查输入为 `/private/tmp/madb-chief-round10/lab` 下如下版本（SHA256）：

| 文件 | SHA256 |
|---|---|
| consumers/arkts/l05_background_lifecycle/ApplicationOwner.ets.in | de002d47c0db79f18a092cfcc1e35a5bfbc579622efac2d8fad7e923b707e513 |
| consumers/arkts/l05_background_lifecycle/Assertions.ets | 7f41bb42046e6ba602e27813cdd488adc103f5e0cd4c6fb1486e2eb7dbf06765 |
| consumers/arkts/l05_background_lifecycle/EntryAbility.ets.in | dec262ddd3d28c58b43f1a58380ac1b31421ebf22b5db92813a08cb0ad83433e |
| consumers/arkts/l05_background_lifecycle/Index.ets.in | 91a6ca1cde57307aeef8e3ae6fc31d48487d9ae52bd7012d162059f49f1629da |
| consumers/arkts/l05_background_lifecycle/README.md | b347d0a62a219e9260082c754508d59cfb4528e4c7d7adc5fbbfa88102269751 |
| consumers/arkts/l05_background_lifecycle/Trace.ets | 99047dd71deb61809bb885c04d882872890df54a2324b96d2cbd49b65b738066 |
| harness/l05_background_lifecycle_execution.py | 15af255384b7fa88a71ebdfbef638c7beb10e91bf1c88fde1f27f24b20fa88ed |
| harness/l05_background_lifecycle_observations.py | 00e9c03ab377b7f5dda51af2df9250ea53b3956e79cca1d1802557cfbc1fa454 |
| scripts/verify_l05_background_lifecycle.py | dc1eb978b3e7d054c86ad81d83689ab361678db4fc7afdc576a5227eeb4d5530 |
| scripts/test_l05_background_lifecycle.py | 2fe68729c8fc12e06264b1f9d3dab3f0316ab06f8c58020c724c8738d825bf30 |

复用已读 Lab/ADR 规范，逐读 ArkTS 六文件并并行审查 Python 四文件与必要复用 helper。15 项纯测试和 build002 编译通过为协调者报告，本独审未重跑、未将其当设备证据。没有改两仓源、调用 hdc、构建或运行目标；仅新增本报告。后续 delta 应保留此两项发现及其关闭依据。

## 两项发现关闭与最终 delta

复核当前三文件窄改：

- `ApplicationOwner.ets.in:62、118` 仅在 finished=true 后让 foreground/background 返回。finished 仍只在页面订阅、普通 observer、Engine、Runtime 全部成功 await 之后设置；测试期、finishing 和失败状态仍走原严格检查。因此正常 cleanup 回调不会产生 FAILED after COMPLETE，也未掩盖未完成关闭。
- `l05_background_lifecycle_execution.py:138–157` 每轮获取一份真实 layout，在同一字典内匹配所有 UI_EXPECTED 字段，节点缺失/重复或任一字段不符均不能通过；成功记录该 layout 的文件名与 SHA256。新增交替快照负例和完整单快照正例覆盖此缺口，未改变值、周期或顺序预期。
- 同文件 `101–104` 窄加编译后 HAP 唯一 EntryAbility 且 launchType=singleton 断言；不是对运行期生命周期检查的替代。

最终三项 SHA256：

| 文件 | SHA256 |
|---|---|
| consumers/arkts/l05_background_lifecycle/ApplicationOwner.ets.in | f623159371dc539baeecbde0c4bb74516aeb8d377fda67a9931f83896921f4db |
| harness/l05_background_lifecycle_execution.py | 26e8f6802f3c161d3e287ed4a5ba665e72b38188de0204bf28e222b9a69e891e |
| scripts/test_l05_background_lifecycle.py | 71ba39e4014179f65eff50ab066bde383476871a080dd521a7bf5ab5ccbf2202 |

其余七文件与初审表逐项 SHA256 相同。本独审只读复核了上述实现及两个新增测试；17 项纯测试通过为协调者报告，未自行重跑。build002 为较早版本的 build-only 原件，不能复用于最终版本编译或目标通过声明。最新源须从最终 clean freeze 重编译并实际验证；本次代码审查至此收束，没有 hdc、构建、设备或额外验收动作。
