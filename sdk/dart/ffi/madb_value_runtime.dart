// Optional bounded UTF-8 value helpers; select only for value compositions.
part of '../madb_dart_ffi.dart';

Uint8List _encodeUtf8(String value, int maxBytes, String name) {
  final units = value.codeUnits;
  for (var i = 0; i < units.length; i++) {
    final unit = units[i];
    if (unit >= 0xd800 && unit <= 0xdbff) {
      if (++i == units.length || units[i] < 0xdc00 || units[i] > 0xdfff) {
        throw FormatException('$name contains an unpaired UTF-16 surrogate');
      }
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      throw FormatException('$name contains an unpaired UTF-16 surrogate');
    }
  }
  final bytes = Uint8List.fromList(utf8.encode(value));
  _checkIntRange(bytes.length, 0, maxBytes, '$name byte length');
  return bytes;
}

String _decodeUtf8(
  Uint8List bytes,
  int length,
  int maxBytes,
  String operation,
) {
  _checkOutput(
    length >= 0 && length <= maxBytes && bytes.length == maxBytes,
    operation,
  );
  _checkOutput(bytes.skip(length).every((byte) => byte == 0), operation);
  try {
    return utf8.decode(bytes.sublist(0, length), allowMalformed: false);
  } on FormatException {
    throw MadbContractException._(operation, 'invalidOutput');
  }
}

