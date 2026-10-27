// Public ffigen API adapter. Build as a pinned executable before author sessions;
// generation does not run pub or mutate the immutable input package.
import 'dart:io';
import 'package:ffigen/ffigen.dart';
import 'package:logging/logging.dart';

Future<void> main(List<String> arguments) async {
  if (arguments.length != 5) {
    throw ArgumentError('expected HEADER OUTPUT SDK INCLUDE LIBCLANG');
  }
  var failed = false;
  final logging = Logger.root.onRecord.listen((record) {
    if (record.level >= Level.SEVERE) failed = true;
    stderr.writeln('${record.level.name}: ${record.message}');
  });
  try {
    final entry = File(arguments[0]).absolute.uri;
    final includes = Directory(arguments[3]).absolute.path;
    final generator = FfiGenerator(
      input: Input(
        entryPoints: [entry],
        include: (uri) => uri == entry || uri.toFilePath().startsWith('$includes/'),
        compilerOptions: ['-isysroot', arguments[2], '-I', includes],
        ignoreSourceErrors: false,
      ),
      output: Output(
        // This adapter is an AOT executable. The public formatter switch avoids
        // ffigen searching its ancestors for an unrelated Dart executable.
        // The generated package is analyzed and compiled by the pinned SDK.
        format: false,
        dart: DartOutput(path: File(arguments[1]).absolute.uri),
        style: DynamicLibraryBindings(wrapperName: 'RawNative'),
        preamble: '// Generated from the locked public C headers and owned bridge declarations.',
      ),
      visitors: [
        Visitor(
          struct: (node) => node.isIncluded = {
            'MadbError', 'MadbCheckedEngineCreateResult',
          }.contains(node.originalName),
          macroConstant: (node) => node.isIncluded = node.originalName.startsWith('MADB_'),
          func: (node) => node.isIncluded = node.originalName.startsWith('madb_dart_'),
        ),
      ],
    );
    await generator.generate(libclangDylib: File(arguments[4]).absolute.uri);
    if (failed) throw StateError('ffigen reported a source or parsing error');
  } finally {
    await logging.cancel();
  }
}
