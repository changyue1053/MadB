// Handwritten runtime asset for the ADR-0026 generated Dart library.
part of '../madb_dart_ffi.dart';

enum EngineState { open, closing, closed }

final class MadbLoadException implements Exception {
  final String kind;
  final String path;
  final String? expectedDigest;
  final String? actualDigest;
  final Object? cause;

  const MadbLoadException._(
    this.kind,
    this.path, {
    this.expectedDigest,
    this.actualDigest,
    this.cause,
  });

  @override
  String toString() => 'MadbLoadException($kind, $path)';
}

final class MadbIdentityException implements Exception {
  final String kind;
  final String expectedInterfaceDigest;
  final String actualInterfaceDigest;
  final String expectedProfileDigest;
  final String actualProfileDigest;

  const MadbIdentityException._(
    this.kind,
    this.expectedInterfaceDigest,
    this.actualInterfaceDigest,
    this.expectedProfileDigest,
    this.actualProfileDigest,
  );

  @override
  String toString() => 'MadbIdentityException($kind)';
}

final class MadbCallException implements Exception {
  final String operation;
  final String stage;
  final int status;
  final int domain;
  final int code;

  const MadbCallException._(
    this.operation,
    this.stage,
    this.status,
    this.domain,
    this.code,
  );

  @override
  String toString() =>
      'MadbCallException($operation, $stage, $status, $domain, $code)';
}

final class MadbContractException implements Exception {
  final String operation;
  final String reason;

  const MadbContractException._(this.operation, this.reason);

  @override
  String toString() => 'MadbContractException($operation, $reason)';
}

typedef _StatusResult = ({int status, int domain, int code});
typedef _IdentityResult = ({
  int status,
  int domain,
  int code,
  Uint8List identity,
});
typedef _CreateResult = ({
  int kind,
  int status,
  int domain,
  int code,
  int handle,
});
typedef _ScalarResult = ({int status, int domain, int code, int value});

// Generated adapters allocate/copy/free call-frame memory. Native pointers and
// ffigen types never escape this private, same-library boundary.
abstract interface class _NativeCalls {
  String nativePath(int symbolIndex);
  _IdentityResult bootstrap();
}

abstract interface class _EngineNativeCalls implements _NativeCalls {
  _CreateResult createChecked();
  _ScalarResult checksum(int handle, Uint8List bytes);
  _StatusResult requestClose(int handle);
  _StatusResult awaitClosed(int handle, int timeoutMilliseconds);
  _StatusResult releaseHandle(int handle);
}

void _checkOutput(
  bool condition,
  String operation, [
  String reason = 'invalidOutput',
]) {
  if (!condition) throw MadbContractException._(operation, reason);
}

void _validateCall(String operation, String stage, _StatusResult result) {
  if (result.status < 0 || result.status > 11) {
    throw MadbContractException._(operation, 'invalidStatus');
  }
  if (result.domain < 0 ||
      result.domain > 0xffffffff ||
      result.code < -0x80000000 ||
      result.code > 0x7fffffff ||
      (result.status == 10
          ? result.domain == 0
          : result.domain != 0 || result.code != 0)) {
    throw MadbContractException._(operation, 'invalidErrorPayload');
  }
  if (result.status != 0) {
    throw MadbCallException._(
      operation,
      stage,
      result.status,
      result.domain,
      result.code,
    );
  }
}

void _checkIntRange(int value, int min, int max, String name) {
  if (value < min || value > max) {
    throw RangeError.range(value, min, max, name);
  }
}

final class _Asset {
  final String path;
  final String digest;
  final int size;
  final Uint8List bytes;

  const _Asset(this.path, this.digest, this.size, this.bytes);
}

final class _Install {
  final String bridge;
  final String native;
  final String identity;
  final List<_Asset> assets;

  const _Install(this.bridge, this.native, this.identity, this.assets);
}

final _hexDigest = RegExp(r'^[0-9a-f]{64}$');
final _identityDigest = RegExp(r'^sha256:[0-9a-f]{64}$');
final _assetSegment = RegExp(r'^[a-z0-9][a-z0-9._-]*$');

String _sha256(List<int> bytes) => sha256.convert(bytes).toString();

Never _loadFailure(String kind, String path, {Object? cause}) =>
    throw MadbLoadException._(kind, path, cause: cause);

bool _keys(Map<String, dynamic> value, Set<String> expected) =>
    value.length == expected.length && value.keys.every(expected.contains);

String _relativeAsset(Object? value, String root) {
  if (value is! String ||
      value == 'install.json' ||
      value.split('/').any((part) => _assetSegment.stringMatch(part) != part)) {
    _loadFailure('pathViolation', root);
  }
  return value;
}

File _regularAsset(String root, String relative) {
  final parts = relative.split('/');
  var path = root;
  for (var i = 0; i < parts.length; i++) {
    path += '/${parts[i]}';
    final type = FileSystemEntity.typeSync(path, followLinks: false);
    if (type == FileSystemEntityType.notFound) {
      _loadFailure('missingAsset', path);
    }
    final expected = i == parts.length - 1
        ? FileSystemEntityType.file
        : FileSystemEntityType.directory;
    if (type != expected) _loadFailure('pathViolation', path);
  }
  final file = File(path);
  if ((file.statSync().mode & 0xf000) != 0x8000) {
    _loadFailure('pathViolation', path);
  }
  if (file.resolveSymbolicLinksSync() != '$root/$relative') {
    _loadFailure('pathViolation', path);
  }
  return file;
}

void _verifyBytes(List<int> bytes, String digest, int size, String path) {
  final actual = _sha256(bytes);
  if (bytes.length != size || actual != digest) {
    throw MadbLoadException._(
      'digestMismatch',
      path,
      expectedDigest: digest,
      actualDigest: actual,
    );
  }
}

_Install _readInstall(
  String root,
  String expectedDigest,
  String interfaceDigest,
  String profileDigest,
) {
  final index = _regularAsset(root, 'install.json');
  if (index.lengthSync() > 1024 * 1024) {
    _loadFailure('invalidPackage', index.path);
  }
  final raw = index.readAsBytesSync();
  final actualDigest = _sha256(raw);
  if (actualDigest != expectedDigest) {
    throw MadbLoadException._(
      'digestMismatch',
      index.path,
      expectedDigest: expectedDigest,
      actualDigest: actualDigest,
    );
  }
  final decoded = jsonDecode(utf8.decode(raw, allowMalformed: false));
  if (decoded is! Map<String, dynamic> ||
      !_keys(decoded, {'bridge', 'native', 'identity', 'files'})) {
    _loadFailure('invalidPackage', index.path);
  }
  final bridge = _relativeAsset(decoded['bridge'], root);
  final native = _relativeAsset(decoded['native'], root);
  final identity = _relativeAsset(decoded['identity'], root);
  final names = {bridge, native, identity};
  final entries = decoded['files'];
  if (names.length != 3 || entries is! List || entries.length != 3) {
    _loadFailure('invalidPackage', index.path);
  }
  final assets = <_Asset>[];
  final seen = <String>{};
  for (final entry in entries) {
    if (entry is! Map<String, dynamic> ||
        !_keys(entry, {'path', 'sha256', 'size'})) {
      _loadFailure('invalidPackage', index.path);
    }
    final path = _relativeAsset(entry['path'], root);
    final digest = entry['sha256'];
    final size = entry['size'];
    if (!names.contains(path) ||
        !seen.add(path) ||
        digest is! String ||
        !_hexDigest.hasMatch(digest) ||
        size is! int ||
        size < 0) {
      _loadFailure('invalidPackage', index.path);
    }
    final file = _regularAsset(root, path);
    final bytes = file.readAsBytesSync();
    _verifyBytes(bytes, digest, size, file.path);
    assets.add(_Asset(path, digest, size, bytes));
  }
  final directories = <String>{};
  for (final path in names) {
    var parent = path;
    while (parent.contains('/')) {
      parent = parent.substring(0, parent.lastIndexOf('/'));
      if (names.contains(parent))
        _loadFailure('pathViolation', '$root/$parent');
      directories.add(parent);
    }
  }
  for (final entity in Directory(
    root,
  ).listSync(recursive: true, followLinks: false)) {
    final relative = entity.path.substring(root.length + 1);
    if (entity is Link ||
        (entity is File &&
            relative != 'install.json' &&
            !names.contains(relative)) ||
        (entity is Directory && !directories.contains(relative))) {
      _loadFailure('pathViolation', entity.path);
    }
  }
  final id = jsonDecode(
    utf8.decode(
      assets.singleWhere((asset) => asset.path == identity).bytes,
      allowMalformed: false,
    ),
  );
  if (id is! Map<String, dynamic> ||
      !_keys(id, {
        'schema',
        'bootstrap_version',
        'interface_digest',
        'profile_digest',
      }) ||
      id['schema'] != 'madb.native-identity/1' ||
      id['bootstrap_version'] != 1 ||
      id['interface_digest'] != interfaceDigest ||
      id['profile_digest'] != profileDigest) {
    _loadFailure('invalidPackage', '$root/$identity');
  }
  return _Install(bridge, native, identity, List.unmodifiable(assets));
}

void _readOnly(String path, bool executable) {
  // Only the platform allocator/permission primitive uses process lookup.
  // SDK business symbols are resolved exclusively through the verified bridge.
  final chmod = DynamicLibrary.process()
      .lookupFunction<
        Int32 Function(Pointer<Uint8>, Uint16),
        int Function(Pointer<Uint8>, int)
      >('chmod');
  final bytes = utf8.encode(path);
  final pointer = ffi.calloc<Uint8>(bytes.length + 1);
  try {
    pointer.asTypedList(bytes.length).setAll(0, bytes);
    if (chmod(pointer, executable ? 320 : 256) != 0) {
      _loadFailure('invalidPackage', path);
    }
  } finally {
    ffi.calloc.free(pointer);
  }
}

final class _NativeLibrary {
  final DynamicLibrary bridge;
  final _NativeCalls calls;
  final String stagingDirectory;
  final String interfaceDigest;
  final String profileDigest;

  const _NativeLibrary._(
    this.bridge,
    this.calls,
    this.stagingDirectory,
    this.interfaceDigest,
    this.profileDigest,
  );

  factory _NativeLibrary.open(
    String packageDirectory, {
    required String installDigest,
    required String interfaceDigest,
    required String profileDigest,
    required int symbolCount,
  }) {
    if (Abi.current() != Abi.macosArm64 ||
        Platform.version.split(' ').first != '3.13.7') {
      _loadFailure('unsupportedTarget', packageDirectory);
    }
    if (!_hexDigest.hasMatch(installDigest) ||
        !_identityDigest.hasMatch(interfaceDigest) ||
        !_identityDigest.hasMatch(profileDigest) ||
        symbolCount <= 0) {
      _loadFailure('invalidPackage', packageDirectory);
    }
    try {
      final root = Directory(packageDirectory).resolveSymbolicLinksSync();
      final install = _readInstall(
        root,
        installDigest,
        interfaceDigest,
        profileDigest,
      );
      final staging = Directory.systemTemp.createTempSync('madb-dart-ffi-');
      // Cache ownership is explicit and separate from SDK handles. The launcher
      // may supply its own TMPDIR and remove it only after this process exits.
      for (final asset in install.assets) {
        final file = File('${staging.path}/${asset.path}');
        file.parent.createSync(recursive: true);
        file.writeAsBytesSync(asset.bytes, flush: true);
        _readOnly(
          file.path,
          asset.path == install.bridge || asset.path == install.native,
        );
      }
      final stagedRoot = staging.resolveSymbolicLinksSync();
      final native = File('$stagedRoot/${install.native}')
          .resolveSymbolicLinksSync();
      late final DynamicLibrary bridge;
      late final _NativeCalls calls;
      try {
        bridge = DynamicLibrary.open('$stagedRoot/${install.bridge}');
        calls = _generatedCalls(bridge);
      } catch (error) {
        _loadFailure(
          'nativeLoadFailed',
          '$stagedRoot/${install.bridge}',
          cause: error,
        );
      }
      for (var index = 0; index < symbolCount; index++) {
        try {
          final path = calls.nativePath(index);
          if (!path.startsWith('/') ||
              File(path).resolveSymbolicLinksSync() != native) {
            _loadFailure('imageGraphMismatch', path);
          }
        } on MadbLoadException {
          rethrow;
        } catch (error) {
          _loadFailure('imageGraphMismatch', native, cause: error);
        }
      }
      for (final asset in install.assets) {
        final file = _regularAsset(stagedRoot, asset.path);
        _verifyBytes(
          file.readAsBytesSync(),
          asset.digest,
          asset.size,
          file.path,
        );
      }
      final library = _NativeLibrary._(
        bridge,
        calls,
        stagedRoot,
        interfaceDigest,
        profileDigest,
      );
      library._verifyIdentity('MadbLibrary.open');
      return library;
    } on FileSystemException catch (error) {
      _loadFailure(
        'invalidPackage',
        error.path ?? packageDirectory,
        cause: error,
      );
    } on FormatException catch (error) {
      _loadFailure('invalidPackage', packageDirectory, cause: error);
    }
  }

  ({String interfaceDigest, String profileDigest}) _readIdentity(
    String operation,
  ) {
    final result = calls.bootstrap();
    _checkOutput(
      result.identity.length == 64,
      operation,
      'invalidIdentityLength',
    );
    if (result.status >= 1 && result.status <= 11) {
      // The adapter always requests the public fixed 64-byte capacity. Failed
      // bootstrap must clear that complete frame before its status is exposed.
      _checkOutput(result.identity.every((byte) => byte == 0), operation);
    }
    _validateCall(operation, 'bootstrap', (
      status: result.status,
      domain: result.domain,
      code: result.code,
    ));
    String digest(int start) =>
        'sha256:' +
        result.identity
            .sublist(start, start + 32)
            .map((byte) => byte.toRadixString(16).padLeft(2, '0'))
            .join();
    return (interfaceDigest: digest(0), profileDigest: digest(32));
  }

  void _verifyIdentity(String operation, {int? checkedKind}) {
    final actual = _readIdentity(operation);
    final kind = actual.interfaceDigest != interfaceDigest
        ? 'interfaceMismatch'
        : actual.profileDigest != profileDigest
        ? 'profileMismatch'
        : null;
    if (checkedKind != null &&
        kind != (checkedKind == 2 ? 'interfaceMismatch' : 'profileMismatch')) {
      throw MadbContractException._(operation, 'invalidOutput');
    }
    if (kind != null) {
      throw MadbIdentityException._(
        kind,
        interfaceDigest,
        actual.interfaceDigest,
        profileDigest,
        actual.profileDigest,
      );
    }
  }
}

final class _EngineOwner {
  // Retaining the DynamicLibrary transitively makes the owner unsendable through
  // SendPort on the pinned SDK. No integer-only transferable owner is exposed.
  final _NativeLibrary library;
  final _EngineNativeCalls calls;
  final int handle;
  EngineState _state = EngineState.open;
  bool _inClose = false;

  _EngineOwner._(this.library, this.calls, this.handle);

  factory _EngineOwner.create(_NativeLibrary library) {
    final calls = library.calls;
    if (calls is! _EngineNativeCalls) {
      throw StateError('this generated module has no Engine resource');
    }
    final result = calls.createChecked();
    _checkOutput(result.kind >= 0 && result.kind <= 4, 'createEngine');
    if (result.kind == 2 || result.kind == 3) {
      _checkOutput(
        result.handle == 0 &&
            result.status == 0 &&
            result.domain == 0 &&
            result.code == 0,
        'createEngine',
      );
      // The checked constructor carries no actual identity bytes. Read them
      // from the same bridge; never invent those public exception fields.
      library._verifyIdentity('createEngine', checkedKind: result.kind);
      throw const MadbContractException._('createEngine', 'invalidOutput');
    }
    if (result.kind != 0) {
      _checkOutput(result.handle == 0 && result.status != 0, 'createEngine');
      _validateCall('createEngine', result.kind == 1 ? 'bootstrap' : 'create', (
        status: result.status,
        domain: result.domain,
        code: result.code,
      ));
      throw const MadbContractException._('createEngine', 'invalidOutput');
    }
    _checkOutput(result.status == 0 && result.handle != 0, 'createEngine');
    _validateCall('createEngine', 'create', (
      status: result.status,
      domain: result.domain,
      code: result.code,
    ));
    return _EngineOwner._(library, calls, result.handle);
  }

  EngineState get state => _state;

  void checkOpen(String operation) {
    if (_state != EngineState.open) {
      throw StateError('$operation requires an open Engine (${_state.name})');
    }
  }

  void close({int timeoutMilliseconds = 1000}) {
    _checkIntRange(timeoutMilliseconds, 0, 0xffffffff, 'timeoutMilliseconds');
    if (_state == EngineState.closed) return;
    if (_inClose) throw StateError('Engine.close is already executing');
    _state = EngineState.closing;
    _inClose = true;
    try {
      _validateCall('close', 'requestClose', calls.requestClose(handle));
      _validateCall(
        'close',
        'awaitClosed',
        calls.awaitClosed(handle, timeoutMilliseconds),
      );
      _validateCall('close', 'releaseHandle', calls.releaseHandle(handle));
      _state = EngineState.closed;
    } finally {
      // Any exception leaves closing and keeps both handle and loaded image.
      _inClose = false;
    }
  }
}
