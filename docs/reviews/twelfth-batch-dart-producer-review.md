# E04 正式生产者原件独立复核

结论：有限资源首片的正式生产者原件复核通过，未发现阻断。实际 clean 产品源码、全新生产者工具、Source10、工具五门禁及 Source→Build→Package 候选相互绑定，候选非 provisional。本结论只确认生产者原件与已有装配语义相符；不授独立 Lab、第二值业务、disable/version isolation 或整体 E04 通过。Lab 仍须由根确认本包后另行运行，生产者 smoke 不能替代。

## 本次绑定

生产者根 `/private/tmp/madb-chief-round12/e04-formal-producer-001`；实际产品根 `/Users/changyue/Desktop/MadB/madb`，HEAD `bf478767273069c47798bc04140185c671d93957`。审查起止实际 Git 状态均干净。

- 汇总 `result.json`：7,119 B，SHA-256 `2a4a2d93afb4e87bc762ced0c514b52faa44daa5725ed45d8af89a52ef40d153`。
- `receipt.json`：1,854 B，SHA-256 `44f16aef3058a1d5945af65bdd9e6ad044ddb3566eef20075ab7cb45350c7770`。
- `rust-receipt.json` SHA `3a98abeafd2083bcac96464508c29f2d9b5a382419e4e51e132e5f3c04c66e15`；`ffigen-receipt.json` SHA `62ee8bbeafc714ef90a36b6b6ae415de2549762f62392d911aedcb1ea21f07d5`。
- 复用的已审 E04 `freeze002` SHA `a551c762d08659af42acd681366f1e22b6195001dd825ca664aaf723c3231398`：30 owned + 2 shared 在实际产品根逐项重算匹配。composition Build、task 和 execution/build 共享接缝也与原独审字节一致，未重复全量语义审查。
- 本次只写本报告及 `e04-formal-producer-independent-review-binding.json`；后者 78,208 B，SHA-256 `dca2c7a7d7029478046d76cd98c48aa2fe2f15be7f004bdeb9a67f33f4d35fa0`。其中保留所有检查结果、精确路径及原件散列，mismatches 为空。

## clean 源与全新工具

独立执行只读 Git 查询并重算源文件：1,550 个 tracked 文件的精确集合、大小、散列及文件权限均与 source-before.json/source-after-rust.json 相同。两个完整源快照相等；执行前后 source/head/clean 与四工具身份也相等。不是仅采信 clean=true 字段。

原始 Cargo JSONL 证明同一次 `build --locked --offline --workspace --bins --example dart_ffi_probe --example dart_ffi_peer` 使用本次独占 target，MADB_SOURCE_REVISION 为 bf478。九个 workspace package 全覆盖；13 条 workspace artifact 记录均 fresh=false（本次实际编译），最终 build-finished success=true。receipt 中 artifact 文件与原始 Cargo 输出精确对应，实际 madb/probe/peer 镜像字节等于该次 compiler artifact。

ffigen 从实际 tracked pubspec.yaml、pubspec.lock、tool/ffigen.dart 原样复制到独立工作目录；固定 Dart 3.13.7 先 offline/enforce-lockfile，再 AOT compile，两个命令原始输出与 exit 0 均匹配。原始 depfile 的精确 295 项依赖集合等于 receipt，实际依赖字节匹配；Dart SDK 的完整 1,044 文件集合与构建前、构建后、执行后清单相等，实际字节匹配。

最终 images 下四镜像均实际 0500，大小/散列与 receipt 相同：

| 镜像 | SHA-256 |
|---|---|
| madb | `447b781651e70dc27a9bbcedd554dffb52b8d3dbe3fc994cebc5a5fcfd8b8edb` |
| dart_ffi_probe | `618bebf7f0994bc620fb4fadd927572e38cc06c8fc0f48ea83b6cdbb01700501` |
| dart_ffi_peer | `9040c9ac0a53537a75e0e6579452405ab7535b77baab4a79eee357d94444590a` |
| e04-ffigen | `107de11eed3ef9492933a4ce2bee714d957eca70d94ac36d4bd74cf7110ff8b3` |

Source10、工具门禁与 pipeline 内实际 peer 副本也逐项等于该正式 peer。madb 原始版本记录为 0.1.0。

## Source10 与五门禁

`source-10/result.json` SHA `531f79d15943c34044bdecb32cbf86cd511580a418c50966a0363b6929f76dd2`。独立核十条命令、20 个 stdout/stderr 引用、各实际 storage index、before/after 输入与工具：两次成功均提交并从 revision 1→2，实际公开 Source inventory 相同，reader 合同实际字节传给 generator。其余八项（两个 result cancel、between/accept/commit cancel、两个 output drift、upstream drift）均未提交，实际索引保持 revision 1。被跳过节点和实际执行节点有明确区分，运行节点清理事实没有被“取消”字段替代。

`tool-gates-5/result.json` SHA `d74f612688d37913b230cbe5812c80cbf47bd00ace214aa641da076938a7c593`。Source/Build/Package 三例均在实际 helper 产物已 Validated 后，仅修改测试私有 ffigen 副本；Host 提交回调以 external tool/header bytes differ 拒绝，committed=false/index_unchanged=true，实际 storage revision 分别保持 1/2/3。私有副本单链接、修改后实际散列与 fault 一致，原工具未改。

另外两例实际重新 pin backend 输入、保留 profile/lock 中旧工具身份或 clang 参数，原始 stderr 分别为 backend tool tuple 不一致和 SDK root/locked clang 参数不一致。fixture 内容与各自单一错配相符，driver 的非零退出断言通过。这两个子例未单列保存数值退出码，本审不补造数值；外层已保存实际命令、exit 0 及原始输出。以上为有限控制/文件漂移证明，不冒充 OS 信号竞态或 native Timeout。

## 完整正式候选

`pipeline/result.json` SHA `cc00ce848d683b8f81577392f3f2e0f9862bc45f067aea865444ce11e8b45c6d`。三条实际正式 probe 命令与 stdout/stderr 引用全部匹配；79 项输入及 before/after 一致。全部十个后端工具的实际 canonical 镜像字节、profile/lock tuple、SDK root/-isysroot 关系一致；声明的 92 个 compiler headers 与 93 个 build headers 实际字节也一致。

两个公开 Source 会话均 hello_ack/result、EligibleForValidation、完整清理且无 pending；合同实际字节相同。Build 的 compile/link/run 逐步冻结 95/97/102 项输入，后续任务包含此前真实输出的精确字节身份；三个阶段的任务/evidence 身份、输出与日志引用全部匹配，均通过并完整清理。

独立重算四个 storage batch 全部文件（baseline 1 项、Source 17 项、Build 12 项、Candidate 127 项）。父关系精确为：

- Source `8cb8d080055a3cc6cf3000819e8d8a02d899a77ede7abc7ae41bf533ab78f0c8`。
- Build `bae5a6ed84a50eb1dde9f0603444de90b14f2eb35be8fd45418d6f01dc7717ce`，parent 为上述 Source。
- Candidate `bc263b02df612156856608f5991601a7c29dc4d141d6f232f589d5b5dde77186`，parent 为上述 Build；最终 revision 4。

Package 的 126 个 payload 与固定映射、实际磁盘 inventory、进程返回 manifest、存储导出完全一致，另有 candidate.json 自身一项。12 条内层实际命令全部 exit 0，含 clang compile/link、修改加载路径/签名/依赖检查、Dart offline/analyze/JIT/AOT；analyze 无问题，JIT/AOT 均 checksum=510、state=closed、closed_rejected=true。两个消费者退出后精确删除本调用所有的临时 root；该 root 及两个 native staging 路径在独审时均不存在。

包的三个身份分别为：manifest 原始文件 SHA `af088f9087661cfaf11ac81fd53f2cdad0d27e362bfe1421596cc0957b6b4ec0`；语义 candidate_digest `sha256:ca5f71c451be9cfb408501ae2017daf3ec773b6ae23949ea4a6d27a35fe2b47c`；storage Candidate batch 为上列 bc263b02。未混用三种身份。

候选 provenance 的 producer_revision 为实际 bf478，provisional=false，tool_digest 等于本次正式 peer。原 provider 来自 `/private/tmp/madb-chief-round6/l02-formal-macos-001/c-to-rust`，其 manifest SHA `6ae1bbed238a55a357691bd57c7dfa07dc24fb84715e8b51593bbe1a845b7ffd` 及全部 payload 匹配；所有 provider 复制文件、最终 native dylib 与 native identity 原字节不变。最终 native/install.json 三文件身份与 Dart install digest pins 也匹配。

与 development-008 的区别是：本次来自实际 clean 产品 bf478，Rust/ffigen 镜像均重新构建并只读冻结，Source10/五门禁/整链使用这些新镜像重新运行，候选 provisional=false。旧 development-008 仍为 provisional 证据，原报告和原件未覆盖。本审未编译、运行目标、操作设备或修改产品/Lab；正式包可交根决定下一步独立 Lab，尚无该 Lab 结论。
