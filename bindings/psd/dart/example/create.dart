import 'dart:io';
import 'dart:typed_data';

import 'package:photocraft_psd/photocraft_psd.dart';

void main(List<String> args) {
  try {
    if (args.length != 2)
      throw ArgumentError('usage: create <library> <output.psd>');
    final builder = PsdLibrary.open(args[0]).builder(1, 1);
    try {
      final pixels = Uint8List.fromList([255, 0, 0, 128]);
      builder.addLayer('Red', (top: 0, left: 0, bottom: 1, right: 1), pixels);
      builder.composite(pixels);
      builder.resolution(300);
      final doc = builder.build();
      try {
        File(args[1]).writeAsBytesSync(doc.toBytes());
      } finally {
        doc.close();
      }
    } finally {
      builder.close();
    }
  } catch (error) {
    stderr.writeln(error);
    exitCode = 1;
  }
}
