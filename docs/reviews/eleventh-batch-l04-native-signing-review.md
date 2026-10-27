L04 最终009签名窄差异独立只读审查，2026-10-27。

结论：本次四文件差异静态无未解决阻断，实际 build-only 与签名证据支持恢复 Xcode 正常本地 ad-hoc 装配。签名成功不证明 UI/native 执行或关闭，也不能认定缺失签名/entitlement 是此前 timeout 的唯一原因。最终冻结后的 GUI004 才检验该假设；旧失败保持原结论。

Lab基线 `80118b4fb1ef617d2d789db53baaa2aea8dad512`；实际只有 project.pbxproj、README、verify_l04_native_ui.py、test_l04_native_ui.py 四文件修改。最终 `l04-files-009.json` SHA256 `bdf164237e7ac396b992580608b22e067d3e88367e8e754ebf554c191c40ff7f` 与九项实际 SHA256/size 全匹配。008仅内部中间版，没有作为冻结或 GUI 证据。本审查不写源码、不重跑测试、不启动 GUI、不改变权限或旧产物。

代码复核：两target及 runner命令一致设置 `CODE_SIGNING_ALLOWED=YES`、`CODE_SIGN_IDENTITY=-`、空 DEVELOPMENT_TEAM；没有新增手写 entitlement、对旧候选重签或改变业务/SDK、Release、32帧oracle、240秒条件。`observe_signing` 在UI前对 app、test runner、test bundle 执行 `codesign --verify --deep --strict`，逐行要求 `Signature=adhoc` 和 `TeamIdentifier=not set`；App/Runner还要求实际 `get-task-allow` 为布尔true。签名细节来自 stdout+stderr，entitlement从真实plist解析；test bundle空字典如实保留。结果明确 `local-ad-hoc-development-only / distribution_signature=false`。26项纯测试通过由协调者记录，本审查核新增拒绝规则源码，未再扩大测试。

实际证据已核：

- `l04-signing-diagnostic-003.json`（SHA256 `9b460cf31527e4df117af38b8e275cbc2557f8a57b864eb83815ea71442b2b86`）引用的 SDKSettings.json、Xcode Runner/Base entitlement模板及GUI003构建日志均与实际字节匹配。SDK确实要求 signing/entitlements并允许ad-hoc，两份模板实际均含 get-task-allow=true；这是固定本机配方依据，不是新造权限。
- `l04-signing-build-probe/build-result.json` 及其构建命令日志引用有效；实际只执行 build-for-testing，exit0、timeout=false、TEST BUILD SUCCEEDED，记录4个CodeSign和2个ProcessProductPackaging任务。probe输入清单与消费目录逐项一致；App/NativeOwner与009相同。两项生成xcent引用有效。
- build-only `signing-observation.json` SHA256 `a1ac365386c915d8b8cbc5259c26bc7a154cde75a528e3931eda96898b6505ed` 实核匹配；三产物strict verification均exit0，实际签名均ad-hoc且无Team；App/Runner实际get-task-allow=true。
- 最终观察函数的实际产物结果 `l04-signing-observation-final-check-002/result.json` SHA256 `2372b8eb389fc1ddb87120ecb86cf1a099b565e4f17a08510459da18a327789e` 实核匹配；9条命令引用全部正确、均exit0，签名身份逐项准确，三份实际entitlement输出与result完全一致。
- `l04-startup-preflight-004/result.json` SHA256 `a857e6fabf63c3aeb0e10b3c0c66496b651f62238414c687b4c7f2bae2c02608`；11条命令引用有效，记录的编译工具、codesign及3项签名模板/SDK输入全部匹配当前字节。当前runner已将这些输入纳入 observe_tools / verify_tools 的前后核验。

GUI003原报告SHA256仍为 `a7b85ea9805a95141ee58fbd56a3131e0e85a43180e84bcef01b0e8ff93b7fbb`，未改。上述新证据仅证明构建与本地开发签名装配；没有证明 debugger继续执行、TestSuite启动、业务receipt、32帧或native close。也不授予发布签名、iOS/UIKit或完整L04资格。审查收束。
