import 'dart:io';

import 'package:infomatrix_shell/core/ffi_reader_backend.dart';
import 'package:flutter_test/flutter_test.dart';

String? _resolveLibraryPath() {
  final fileName = switch (Platform.operatingSystem) {
    'macos' => 'libffi_bridge.dylib',
    'linux' => 'libffi_bridge.so',
    'windows' => 'ffi_bridge.dll',
    _ => '',
  };

  if (fileName.isEmpty) {
    return null;
  }

  final cwd = Directory.current.path;
  final candidates = <String>[
    '$cwd/../core/target/debug/$fileName',
    '$cwd/../../core/target/debug/$fileName',
    '$cwd/core/target/debug/$fileName',
  ];

  for (final path in candidates) {
    if (File(path).existsSync()) {
      return path;
    }
  }

  return null;
}

void main() {
  test('ffi backend health smoke', () async {
    final path = _resolveLibraryPath();
    if (path == null) {
      return;
    }

    final backend = FfiReaderBackend(explicitLibPath: path);
    final health = await backend.health();
    expect(health.status, 'ok');
  });
}
