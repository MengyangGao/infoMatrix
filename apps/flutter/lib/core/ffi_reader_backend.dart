import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

import 'models.dart';
import 'reader_backend.dart';

typedef _NativeNoInput = Pointer<Utf8> Function();
typedef _DartNoInput = Pointer<Utf8> Function();

typedef _NativeJsonInput = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _DartJsonInput = Pointer<Utf8> Function(Pointer<Utf8>);

typedef _NativeFreeString = Void Function(Pointer<Utf8>);
typedef _DartFreeString = void Function(Pointer<Utf8>);

class FfiReaderBackend implements ReaderBackend {
  FfiReaderBackend({String? dbPath, String? explicitLibPath})
      : _bindings = _InfoMatrixFfiBindings(explicitLibPath: explicitLibPath),
        _dbPath = dbPath;

  final _InfoMatrixFfiBindings _bindings;
  String? _dbPath;

  @override
  Future<CoreHealth> health() async {
    final Map<String, dynamic> data =
        _bindings.callNoInput('infomatrix_core_health_json');
    return CoreHealth.fromJson(data);
  }

  @override
  Future<String> defaultDbPath() async {
    final Map<String, dynamic> data =
        _bindings.callNoInput('infomatrix_core_default_db_path_json');
    return data['db_path'] as String;
  }

  @override
  Future<List<FeedModel>> listFeeds() async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_list_feeds_json',
      <String, dynamic>{},
    );

    final List<dynamic> rows = data._listValue;
    return rows
        .whereType<Map<String, dynamic>>()
        .map(FeedModel.fromJson)
        .toList(growable: false);
  }

  @override
  Future<List<FeedGroupModel>> listGroups() async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_list_groups_json',
      <String, dynamic>{},
    );

    final List<dynamic> rows = data._listValue;
    return rows
        .whereType<Map<String, dynamic>>()
        .map(FeedGroupModel.fromJson)
        .toList(growable: false);
  }

  @override
  Future<FeedGroupModel> createGroup(String name) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_create_group_json',
      <String, dynamic>{'name': name},
    );
    return FeedGroupModel.fromJson(data);
  }

  @override
  Future<void> updateFeed(
    String feedId, {
    String? title,
    bool? autoFullText,
  }) async {
    final payload = <String, dynamic>{'feed_id': feedId, 'title': title};
    if (autoFullText != null) {
      payload['auto_full_text'] = autoFullText;
    }
    await _callWithDb(
      'infomatrix_core_update_feed_json',
      payload,
    );
  }

  @override
  Future<void> updateFeedGroup(
    String feedId, {
    String? groupId,
  }) async {
    await _callWithDb(
      'infomatrix_core_update_feed_group_json',
      <String, dynamic>{'feed_id': feedId, 'group_id': groupId},
    );
  }

  @override
  Future<void> deleteFeed(String feedId) async {
    await _callWithDb(
      'infomatrix_core_delete_feed_json',
      <String, dynamic>{'feed_id': feedId},
    );
  }

  @override
  Future<List<ItemModel>> listItems(
    String feedId, {
    int limit = 100,
    String? searchQuery,
  }) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_list_entries_json',
      <String, dynamic>{
        'feed_id': feedId,
        'filter': 'all',
        'limit': limit,
        'q': searchQuery,
      },
    );

    final List<dynamic> rows = data._listValue;
    return rows
        .whereType<Map<String, dynamic>>()
        .map(ItemModel.fromJson)
        .toList(growable: false);
  }

  @override
  Future<List<ItemModel>> listAllItems({
    int limit = 200,
    String? searchQuery,
    String filter = 'all',
    String? kind,
  }) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_list_entries_json',
      <String, dynamic>{
        'filter': filter,
        'limit': limit,
        'q': searchQuery,
        if (kind != null) 'kind': kind,
      },
    );

    final List<dynamic> rows = data._listValue;
    return rows
        .whereType<Map<String, dynamic>>()
        .map(ItemModel.fromJson)
        .toList(growable: false);
  }

  @override
  Future<ItemScopeCounts> itemCounts() async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_item_counts_json',
      <String, dynamic>{},
    );
    return ItemScopeCounts.fromJson(data);
  }

  @override
  Future<String> addSubscription(String feedUrl, {String? title}) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_add_subscription_json',
      <String, dynamic>{'feed_url': feedUrl, 'title': title},
    );
    final feedId = data['feed_id'] as String?;
    if (feedId == null || feedId.isEmpty) {
      throw ReaderBackendException('add subscription returned no feed_id');
    }
    return feedId;
  }

  @override
  Future<SubscriptionResultModel> subscribeInput(String inputUrl) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_subscribe_input_json',
      <String, dynamic>{'input_url': inputUrl},
    );
    return SubscriptionResultModel.fromJson(data);
  }

  @override
  Future<DiscoverResult> discoverSite(String siteUrl) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_discover_site_json',
      <String, dynamic>{'site_url': siteUrl},
    );
    return DiscoverResult.fromJson(data);
  }

  @override
  Future<ItemModel> itemDetail(String itemID) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_get_entry_json',
      <String, dynamic>{'item_id': itemID},
    );
    return ItemModel.fromJson(data);
  }

  @override
  Future<ItemModel> createEntry({
    String? id,
    required String title,
    String? kind,
    String? sourceKind,
    String? sourceID,
    String? sourceURL,
    String? sourceTitle,
    String? canonicalURL,
    String? summary,
    String? contentHTML,
    String? contentText,
    String? rawHash,
  }) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_create_entry_json',
      <String, dynamic>{
        'id': id,
        'title': title,
        'kind': kind,
        'source_kind': sourceKind,
        'source_id': sourceID,
        'source_url': sourceURL,
        'source_title': sourceTitle,
        'canonical_url': canonicalURL,
        'summary': summary,
        'content_html': contentHTML,
        'content_text': contentText,
        'raw_hash': rawHash,
      },
    );
    return ItemModel.fromJson(data);
  }

  @override
  Future<ItemModel> fetchFullText(String itemID) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_fetch_fulltext_json',
      <String, dynamic>{'item_id': itemID},
    );
    final item = await itemDetail(itemID);
    return item.copyWith(
      contentText: data['content_text'] as String? ?? item.contentText,
    );
  }

  @override
  Future<OpmlImportResult> importOpml(String opmlXml) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_import_opml_json',
      <String, dynamic>{'opml_xml': opmlXml},
    );
    return OpmlImportResult.fromJson(data);
  }

  @override
  Future<OpmlExportResult> exportOpml() async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_export_opml_json',
      <String, dynamic>{},
    );
    return OpmlExportResult.fromJson(data);
  }

  @override
  Future<RefreshResult> refreshFeed(String feedId) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_refresh_feed_json',
      <String, dynamic>{'feed_id': feedId},
    );
    return RefreshResult.fromJson(data);
  }

  @override
  Future<RefreshDueResult> refreshDueFeeds({int limit = 20}) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_refresh_due_json',
      <String, dynamic>{'limit': limit},
    );
    return RefreshDueResult.fromJson(data);
  }

  @override
  Future<GlobalNotificationSettings> globalNotificationSettings() async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_get_global_notification_settings_json',
      <String, dynamic>{},
    );
    return GlobalNotificationSettings.fromJson(data);
  }

  @override
  Future<GlobalNotificationSettings> updateGlobalNotificationSettings(
    GlobalNotificationSettings settings,
  ) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_update_global_notification_settings_json',
      <String, dynamic>{'settings': settings.toJson()},
    );
    return GlobalNotificationSettings.fromJson(data);
  }

  @override
  Future<NotificationSettings> feedNotificationSettings(String feedId) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_get_feed_notification_settings_json',
      <String, dynamic>{'feed_id': feedId},
    );
    return NotificationSettings.fromJson(data);
  }

  @override
  Future<NotificationSettings> updateFeedNotificationSettings(
    String feedId,
    NotificationSettings settings,
  ) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_update_feed_notification_settings_json',
      <String, dynamic>{'feed_id': feedId, 'settings': settings.toJson()},
    );
    return NotificationSettings.fromJson(data);
  }

  @override
  Future<List<NotificationEventModel>> listPendingNotificationEvents({
    int limit = 50,
  }) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_list_pending_notification_events_json',
      <String, dynamic>{'limit': limit},
    );
    final List<dynamic> rows = data._listValue;
    return rows
        .whereType<Map<String, dynamic>>()
        .map(NotificationEventModel.fromJson)
        .toList(growable: false);
  }

  @override
  Future<int> acknowledgeNotificationEvents(List<String> eventIDs) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_ack_notification_events_json',
      <String, dynamic>{'event_ids': eventIDs},
    );
    return (data['acknowledged'] as num?)?.toInt() ?? 0;
  }

  @override
  Future<ItemModel> patchItemState(
    ItemModel item, {
    bool? isRead,
    bool? isStarred,
    bool? isSavedForLater,
    bool? isArchived,
  }) async {
    final Map<String, dynamic> data = await _callWithDb(
      'infomatrix_core_patch_item_state_json',
      <String, dynamic>{
        'item_id': item.id,
        'is_read': isRead,
        'is_starred': isStarred,
        'is_saved_for_later': isSavedForLater,
        'is_archived': isArchived,
      },
    );

    return item.copyWith(
      isRead: data['is_read'] as bool? ?? item.isRead,
      isStarred: data['is_starred'] as bool? ?? item.isStarred,
      isSavedForLater:
          data['is_saved_for_later'] as bool? ?? item.isSavedForLater,
      isArchived: data['is_archived'] as bool? ?? item.isArchived,
    );
  }

  Future<Map<String, dynamic>> _callWithDb(
    String symbol,
    Map<String, dynamic> payload,
  ) async {
    _dbPath ??= await defaultDbPath();
    final request = <String, dynamic>{
      'db_path': _dbPath,
      ...payload,
    };
    return _bindings.callWithInput(symbol, request);
  }
}

class _InfoMatrixFfiBindings {
  _InfoMatrixFfiBindings({String? explicitLibPath})
      : _library = _openLibrary(explicitLibPath) {
    _free = _lookupFreeString();
  }

  final DynamicLibrary _library;
  late final _DartFreeString _free;

  Map<String, dynamic> callNoInput(String symbol) {
    final resolved = _resolveSymbol(symbol);
    final fn = _library.lookupFunction<_NativeNoInput, _DartNoInput>(resolved);
    final responsePtr = fn();
    return _decodeEnvelope(responsePtr);
  }

  Map<String, dynamic> callWithInput(
      String symbol, Map<String, dynamic> payload) {
    final resolved = _resolveSymbol(symbol);
    final fn =
        _library.lookupFunction<_NativeJsonInput, _DartJsonInput>(resolved);
    final inputPtr = jsonEncode(payload).toNativeUtf8();
    try {
      final responsePtr = fn(inputPtr);
      return _decodeEnvelope(responsePtr);
    } finally {
      calloc.free(inputPtr);
    }
  }

  Map<String, dynamic> _decodeEnvelope(Pointer<Utf8> responsePtr) {
    try {
      final jsonText = responsePtr.toDartString();
      final Map<String, dynamic> envelope =
          jsonDecode(jsonText) as Map<String, dynamic>;
      final ok = envelope['ok'] == true;
      if (!ok) {
        final message =
            envelope['error']?.toString() ?? 'Unknown Rust FFI error';
        throw ReaderBackendException(message);
      }

      final dynamic data = envelope['data'];
      if (data is Map<String, dynamic>) {
        return data;
      }
      if (data is List<dynamic>) {
        return <String, dynamic>{'_list': data};
      }
      return <String, dynamic>{'_value': data};
    } finally {
      _free(responsePtr);
    }
  }

  _DartFreeString _lookupFreeString() {
    try {
      return _library.lookupFunction<_NativeFreeString, _DartFreeString>(
        'infomatrix_core_free_string',
      );
    } on ArgumentError {
      return _library.lookupFunction<_NativeFreeString, _DartFreeString>(
        'aurora_core_free_string',
      );
    }
  }

  String _resolveSymbol(String symbol) {
    try {
      _library.lookup<NativeFunction<_NativeNoInput>>(symbol);
      return symbol;
    } on ArgumentError {
      if (symbol.startsWith('infomatrix_core_')) {
        final legacy = symbol.replaceFirst('infomatrix_core_', 'aurora_core_');
        try {
          _library.lookup<NativeFunction<_NativeNoInput>>(legacy);
          return legacy;
        } on ArgumentError {
          return symbol;
        }
      }
      return symbol;
    }
  }

  static DynamicLibrary _openLibrary(String? explicitLibPath) {
    final candidates = <String>[];

    if (explicitLibPath != null && explicitLibPath.isNotEmpty) {
      candidates.add(explicitLibPath);
    }

    final envPath = Platform.environment['INFOMATRIX_FFI_LIB_PATH'];
    if (envPath != null && envPath.isNotEmpty) {
      candidates.add(envPath);
    }

    final fileName = switch (Platform.operatingSystem) {
      'macos' => 'libffi_bridge.dylib',
      'linux' => 'libffi_bridge.so',
      'windows' => 'ffi_bridge.dll',
      'android' => 'libffi_bridge.so',
      _ => throw ReaderBackendException(
          'Unsupported platform for FFI: ${Platform.operatingSystem}'),
    };

    final executable = File(Platform.resolvedExecutable);
    final executableDir = executable.parent.path;
    final executableParent = executable.parent.parent.path;
    final cwd = Directory.current.path;
    candidates.add('$executableDir/$fileName');
    candidates.add('$executableParent/$fileName');
    candidates.add('$executableParent/Frameworks/$fileName');
    candidates.add('$cwd/core/target/debug/$fileName');
    candidates.add('$cwd/core/target/release/$fileName');
    candidates.add('$cwd/../core/target/debug/$fileName');
    candidates.add('$cwd/../core/target/release/$fileName');
    candidates.add('$cwd/../../core/target/debug/$fileName');
    candidates.add('$cwd/../../core/target/release/$fileName');
    candidates.add('$cwd/$fileName');
    candidates.add(fileName);

    for (final path in candidates) {
      try {
        return DynamicLibrary.open(path);
      } catch (_) {
        // Try next path.
      }
    }

    throw ReaderBackendException(
      'Unable to load Rust core library. Tried: ${candidates.join(', ')}',
    );
  }
}

extension on Map<String, dynamic> {
  List<dynamic> get _listValue {
    final dynamic rows = this['_list'];
    if (rows is List<dynamic>) {
      return rows;
    }
    return <dynamic>[];
  }
}
