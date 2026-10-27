import 'dart:convert';
import 'dart:typed_data';
import 'package:madb_dart_ffi/madb_dart_ffi.dart';
void main(List<String> args) {
  final library = MadbLibrary.open(args.single);
  final engine = library.createEngine();
  final sum = engine.checksum(Uint8List.fromList([0, 1, 254, 255]));
  engine.close();
  engine.close();
  var closed = false;
  try { engine.checksum(Uint8List(0)); } on StateError { closed = true; }
  print(jsonEncode({'checksum': sum, 'state': engine.state.name, 'closed_rejected': closed}));
}
