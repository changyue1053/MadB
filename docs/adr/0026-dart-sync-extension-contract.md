# ADR-0026 Dart 同步扩展首包契约

项目所有者已确认“采用推荐契约，进入实现”。采用日期为2026-10-27（America/New_York）。本决定属于E04/P10既定扩展范围，保持工作包依赖与四项整包验收条件。

采用[具体提案](../reviews/e04-contract-proposal.md)与[独立审查](../reviews/e04-contract-independent-review.md)绑定的修订2。批准提案SHA-256为`c9e7c0ea1bb7771d171e3ff3b67b9a7c72836fa63af0803a1071b0f44d10ebb4`，审查SHA-256为`b7f150010cbde5c1c258f8428fc64aa00f569a798b8fb07ff1c97f2bcf3a3ce4`。两份原文保留批准前结论；本决定记录随后采用，不构成实现、独立验收或发布证据。

- 首配方限定macOS Arm64、`aarch64-apple-darwin`、Dart Native同步值与显式资源关闭。包身份`org.madb.dart-ffi@0.1.0`，路线`org.madb.dart.c-ffi`，profile为`dart-c-ffi-sync-aarch64-apple-darwin`；普通消费包名`madb_dart_ffi`。不推导Flutter/Web、任意Rust/C++ API或其他平台支持。
- 普通入口采用`MadbLibrary.open(packageDirectory)`、`createEngine()`、`Engine.checksum(Uint8List)`、`state`与同步`close(timeoutMilliseconds: 1000)`。open仅加载并核对同实例bootstrap身份；checked-create在createEngine时才取得handle。类型范围、UTF-8、错误分类与准确字段按提案固定，不把原生状态、加载失败与Dart参数错误混为一类。
- 包字节及加载图分别核对；Dart仅打开受检bridge，全部业务与关闭经同一bridge转发，并核实际native符号来自同一冻结镜像。包内安装清单属于生成器/SDK成套冻结的私有布局，不成为新公共wire。临时镜像缓存不因Dart退出自动删除；宿主在消费进程退出后按精确目录清理，资源close与文件清理分别负责。
- close开始后停止本地业务准入，依次request、await、release，全部成功才closed；失败保留closing和owner，允许按既有幂等协议重试。默认1000ms是等待预算，不是性能验收结果。成功后重复close无副作用，参数失败在原生动作前拒绝。
- owner保留DynamicLibrary，禁止跨isolate转移；同步FFI阻塞当前isolate，不承诺固定OS线程、自动worker或Finalizer关闭。顺序重复open限于本固定包/目标，并逐次核图；任意版本共存与并发open不由探针外推。
- 正式实现复用现有extension/composition/profile2和公开AuthorSession、read/generate/build/package端口。补齐真实C声明reader，不能将设计样例当作已注册组件或导入其他适配器私有lowering。同步选择产生自身受检IR与来源，同时保留原native候选身份；第二自然值模块完整保留errors/variants等实际必需语义。

独立包、正常声明生成与变更、真实Release消费、关闭/异常、禁用及版本隔离、适用EXT检查须分别完成。原型JIT/AOT、raw绑定生成或本决定均不替代这些出口。C05/C06/C07旧候选与认证范围不继承或重写；任务实时状态只维护在catalog。
