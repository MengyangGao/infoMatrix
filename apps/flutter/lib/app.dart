import 'package:flutter/material.dart';

import 'core/ffi_reader_backend.dart';
import 'core/reader_backend.dart';
import 'ui/reader_shell_page.dart';

class InfoMatrixApp extends StatelessWidget {
  const InfoMatrixApp({
    super.key,
    this.backendFactory,
  });

  final ReaderBackend Function()? backendFactory;

  @override
  Widget build(BuildContext context) {
    const seed = Color(0xFFE85C3A);
    final colorScheme = ColorScheme.fromSeed(
      seedColor: seed,
      brightness: Brightness.light,
    ).copyWith(
      primary: const Color(0xFFDB5534),
      secondary: const Color(0xFF5A6A78),
      tertiary: const Color(0xFFDAA520),
      surface: const Color(0xFFFFFCF8),
      surfaceContainerHighest: const Color(0xFFF4EDE3),
      surfaceContainer: const Color(0xFFF7F1E8),
      background: const Color(0xFFF7F2E9),
      onPrimary: Colors.white,
    );

    return MaterialApp(
      title: 'InfoMatrix',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: colorScheme,
        useMaterial3: true,
        scaffoldBackgroundColor: const Color(0xFFF7F2E9),
        appBarTheme: const AppBarTheme(
          backgroundColor: Colors.transparent,
          surfaceTintColor: Colors.transparent,
          elevation: 0,
          centerTitle: false,
        ),
        cardTheme: CardThemeData(
          color: const Color(0xFFFFFCF8),
          surfaceTintColor: Colors.transparent,
          elevation: 0,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(22),
            side:
                BorderSide(color: colorScheme.outlineVariant.withOpacity(0.35)),
          ),
        ),
        inputDecorationTheme: InputDecorationTheme(
          filled: true,
          fillColor: Colors.white.withOpacity(0.82),
          contentPadding:
              const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
          border: OutlineInputBorder(
            borderRadius: BorderRadius.circular(18),
            borderSide: BorderSide(color: colorScheme.outlineVariant),
          ),
          enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(18),
            borderSide: BorderSide(color: colorScheme.outlineVariant),
          ),
          focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(18),
            borderSide: BorderSide(color: colorScheme.primary, width: 1.4),
          ),
        ),
        filledButtonTheme: FilledButtonThemeData(
          style: FilledButton.styleFrom(
            backgroundColor: colorScheme.primary,
            foregroundColor: Colors.white,
            padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 14),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(18),
            ),
          ),
        ),
        outlinedButtonTheme: OutlinedButtonThemeData(
          style: OutlinedButton.styleFrom(
            foregroundColor: const Color(0xFF29333C),
            padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 14),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(18),
            ),
            side: BorderSide(color: colorScheme.outlineVariant),
          ),
        ),
        popupMenuTheme: PopupMenuThemeData(
          color: const Color(0xFFFFFCF8),
          surfaceTintColor: Colors.transparent,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(18),
          ),
        ),
      ),
      home: _BootstrapPage(
        backendFactory: backendFactory ?? () => FfiReaderBackend(),
      ),
    );
  }
}

class _BootstrapPage extends StatefulWidget {
  const _BootstrapPage({required this.backendFactory});

  final ReaderBackend Function() backendFactory;

  @override
  State<_BootstrapPage> createState() => _BootstrapPageState();
}

class _BootstrapPageState extends State<_BootstrapPage> {
  ReaderBackend? _backend;
  String? _error;

  @override
  void initState() {
    super.initState();
    _initialize();
  }

  Future<void> _initialize() async {
    try {
      final backend = widget.backendFactory();
      setState(() {
        _backend = backend;
      });
    } catch (error) {
      setState(() {
        _error = error.toString();
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_backend != null) {
      return ReaderShellPage(backend: _backend!);
    }

    if (_error != null) {
      return Scaffold(
        appBar: AppBar(title: const Text('InfoMatrix')),
        body: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text('Rust 核心加载失败',
                  style: Theme.of(context).textTheme.titleLarge),
              const SizedBox(height: 8),
              SelectableText(_error!),
              const SizedBox(height: 12),
              const Text(
                '请先在仓库根目录执行：\n'
                '1) cd core && cargo build -p ffi_bridge\n'
                '2) 设置 INFOMATRIX_FFI_LIB_PATH 指向生成的动态库\n'
                '   或保持默认路径 core/target/{debug|release}',
              ),
              const SizedBox(height: 12),
              FilledButton(
                onPressed: _initialize,
                child: const Text('重试加载'),
              ),
            ],
          ),
        ),
      );
    }

    return const Scaffold(
      body: Center(child: CircularProgressIndicator()),
    );
  }
}
