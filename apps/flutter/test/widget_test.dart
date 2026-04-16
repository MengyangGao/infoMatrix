import 'dart:ui';

import 'package:infomatrix_shell/app.dart';
import 'package:infomatrix_shell/core/models.dart';
import 'package:infomatrix_shell/core/reader_backend.dart';
import 'package:flutter_test/flutter_test.dart';

class _FakeBackend implements ReaderBackend {
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
    return ItemModel(
      id: id ?? 'entry-1',
      title: title,
      kind: kind ?? 'note',
      sourceKind: sourceKind ?? 'manual',
      sourceID: sourceID,
      sourceURL: sourceURL,
      sourceTitle: sourceTitle,
      canonicalUrl: canonicalURL,
      publishedAt: null,
      summaryPreview: summary,
      summary: summary,
      contentHTML: contentHTML,
      contentText: contentText,
      isRead: false,
      isStarred: false,
      isSavedForLater: true,
      isArchived: false,
    );
  }

  @override
  Future<String> addSubscription(String feedUrl, {String? title}) async {
    return 'feed-2';
  }

  @override
  Future<SubscriptionResultModel> subscribeInput(String inputUrl) async {
    return const SubscriptionResultModel(
      feedId: 'feed-1',
      resolvedFeedUrl: 'https://example.com/feed.xml',
      subscriptionSource: 'direct_feed',
    );
  }

  @override
  Future<DiscoverResult> discoverSite(String siteUrl) async {
    return const DiscoverResult(
      normalizedSiteUrl: 'https://example.com',
      discoveredFeeds: <DiscoverFeedCandidate>[],
      warnings: <String>[],
    );
  }

  @override
  Future<OpmlImportResult> importOpml(String opmlXml) async {
    return const OpmlImportResult(
      parsedFeedCount: 1,
      uniqueFeedCount: 1,
      groupedFeedCount: 0,
    );
  }

  @override
  Future<OpmlExportResult> exportOpml() async {
    return const OpmlExportResult(
      opmlXml:
          '<?xml version="1.0" encoding="UTF-8"?><opml version="2.0"></opml>',
      feedCount: 1,
    );
  }

  @override
  Future<String> defaultDbPath() async => '/tmp/infomatrix-test.db';

  @override
  Future<FeedGroupModel> createGroup(String name) async {
    return FeedGroupModel(id: 'group-1', name: name);
  }

  @override
  Future<void> deleteFeed(String feedId) async {}

  @override
  Future<CoreHealth> health() async =>
      const CoreHealth(status: 'ok', version: 'test');

  @override
  Future<List<FeedModel>> listFeeds() async {
    return const <FeedModel>[
      FeedModel(
        id: 'feed-1',
        title: 'Example Feed',
        feedUrl: 'https://example.com/feed.xml',
        feedType: 'rss',
        groups: const <FeedGroupModel>[
          FeedGroupModel(id: 'group-1', name: 'Tech'),
        ],
      ),
    ];
  }

  @override
  Future<List<FeedGroupModel>> listGroups() async {
    return const <FeedGroupModel>[
      FeedGroupModel(id: 'group-1', name: 'Tech'),
    ];
  }

  @override
  Future<List<ItemModel>> listItems(
    String feedId, {
    int limit = 100,
    String? searchQuery,
  }) async {
    return const <ItemModel>[
      ItemModel(
        id: 'item-1',
        title: 'Hello InfoMatrix',
        kind: 'article',
        sourceKind: 'feed',
        canonicalUrl: 'https://example.com/post',
        isRead: false,
        isStarred: false,
        isSavedForLater: false,
        isArchived: false,
      ),
    ];
  }

  @override
  Future<List<ItemModel>> listAllItems({
    int limit = 200,
    String? searchQuery,
    String filter = 'all',
    String? kind,
  }) async {
    return const <ItemModel>[
      ItemModel(
        id: 'note-1',
        title: 'Quick note',
        kind: 'note',
        sourceKind: 'manual',
        isRead: false,
        isStarred: false,
        isSavedForLater: false,
        isArchived: false,
      ),
    ];
  }

  @override
  Future<ItemScopeCounts> itemCounts() async {
    return const ItemScopeCounts(
      all: 1,
      unread: 1,
      starred: 0,
      later: 0,
      notes: 1,
      archive: 0,
    );
  }

  @override
  Future<ItemModel> itemDetail(String itemID) async {
    return const ItemModel(
      id: 'item-1',
      title: 'Hello InfoMatrix',
      kind: 'article',
      sourceKind: 'feed',
      canonicalUrl: 'https://example.com/post',
      publishedAt: '2026-01-01T00:00:00Z',
      summaryPreview: 'A detail view',
      summary: 'A detail view',
      contentText: 'Body',
      isRead: false,
      isStarred: false,
      isSavedForLater: false,
      isArchived: false,
    );
  }

  @override
  Future<ItemModel> fetchFullText(String itemID) async {
    return itemDetail(itemID);
  }

  @override
  Future<ItemModel> patchItemState(
    ItemModel item, {
    bool? isRead,
    bool? isStarred,
    bool? isSavedForLater,
    bool? isArchived,
  }) async {
    return item.copyWith(
      isRead: isRead,
      isStarred: isStarred,
      isSavedForLater: isSavedForLater,
      isArchived: isArchived,
    );
  }

  @override
  Future<void> updateFeed(
    String feedId, {
    String? title,
    bool? autoFullText,
  }) async {}

  @override
  Future<void> updateFeedGroup(
    String feedId, {
    String? groupId,
  }) async {}

  @override
  Future<RefreshResult> refreshFeed(String feedId) async {
    return const RefreshResult(
        status: 'updated', fetchedHttpStatus: 200, itemCount: 1);
  }

  @override
  Future<RefreshDueResult> refreshDueFeeds({int limit = 20}) async {
    return const RefreshDueResult(refreshedCount: 1, totalItemCount: 1);
  }

  @override
  Future<GlobalNotificationSettings> globalNotificationSettings() async {
    return GlobalNotificationSettings(
      backgroundRefreshEnabled: true,
      backgroundRefreshIntervalMinutes: 15,
      digestPolicy: const DigestPolicy(
        enabled: false,
        intervalMinutes: 60,
        maxItems: 20,
      ),
      defaultFeedSettings: const NotificationSettings(
        enabled: true,
        mode: NotificationMode.immediate,
        digestPolicy: DigestPolicy(
          enabled: false,
          intervalMinutes: 60,
          maxItems: 20,
        ),
        quietHours: QuietHours(
          enabled: false,
          startMinute: 22 * 60,
          endMinute: 7 * 60,
        ),
        minimumIntervalMinutes: 60,
        highPriority: false,
        keywordInclude: <String>[],
        keywordExclude: <String>[],
      ),
    );
  }

  @override
  Future<GlobalNotificationSettings> updateGlobalNotificationSettings(
    GlobalNotificationSettings settings,
  ) async {
    return settings;
  }

  @override
  Future<NotificationSettings> feedNotificationSettings(String feedId) async {
    return const NotificationSettings(
      enabled: true,
      mode: NotificationMode.immediate,
      digestPolicy: DigestPolicy(
        enabled: false,
        intervalMinutes: 60,
        maxItems: 20,
      ),
      quietHours: QuietHours(
        enabled: false,
        startMinute: 22 * 60,
        endMinute: 7 * 60,
      ),
      minimumIntervalMinutes: 60,
      highPriority: false,
      keywordInclude: <String>[],
      keywordExclude: <String>[],
    );
  }

  @override
  Future<NotificationSettings> updateFeedNotificationSettings(
    String feedId,
    NotificationSettings settings,
  ) async {
    return settings;
  }

  @override
  Future<List<NotificationEventModel>> listPendingNotificationEvents({
    int limit = 50,
  }) async {
    return const <NotificationEventModel>[
      NotificationEventModel(
        id: 'event-1',
        feedId: 'feed-1',
        entryId: 'item-1',
        title: 'New article',
        body: 'A new article is available',
        mode: NotificationMode.immediate,
        deliveryState: NotificationDeliveryState.pending,
        reason: 'new_item',
        createdAt: '2026-01-01T00:00:00Z',
        readyAt: '2026-01-01T00:00:00Z',
        deliveredAt: null,
        suppressedAt: null,
      ),
    ];
  }

  @override
  Future<int> acknowledgeNotificationEvents(List<String> eventIDs) async {
    return eventIDs.length;
  }
}

class _RecordingBackend extends _FakeBackend {
  final List<Map<String, Object?>> patchCalls = <Map<String, Object?>>[];

  @override
  Future<ItemModel> patchItemState(
    ItemModel item, {
    bool? isRead,
    bool? isStarred,
    bool? isSavedForLater,
    bool? isArchived,
  }) async {
    patchCalls.add(<String, Object?>{
      'item_id': item.id,
      'is_read': isRead,
      'is_starred': isStarred,
      'is_saved_for_later': isSavedForLater,
      'is_archived': isArchived,
    });
    return super.patchItemState(
      item,
      isRead: isRead,
      isStarred: isStarred,
      isSavedForLater: isSavedForLater,
      isArchived: isArchived,
    );
  }
}

class _PagedBackend extends _FakeBackend {
  final List<int> listItemLimits = <int>[];

  @override
  Future<List<ItemModel>> listItems(
    String feedId, {
    int limit = 100,
    String? searchQuery,
  }) async {
    listItemLimits.add(limit);
    final itemCount = limit > 260 ? 260 : limit;
    return List<ItemModel>.generate(
      itemCount,
      (index) => ItemModel(
        id: 'item-$index',
        title: 'Hello InfoMatrix $index',
        kind: 'article',
        sourceKind: 'feed',
        canonicalUrl: 'https://example.com/post/$index',
        isRead: false,
        isStarred: false,
        isSavedForLater: false,
        isArchived: false,
      ),
      growable: false,
    );
  }
}

void main() {
  testWidgets('renders shell with fake backend', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1600, 1200));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(
      InfoMatrixApp(backendFactory: () => _FakeBackend()),
    );

    await tester.pumpAndSettle();

    expect(find.text('InfoMatrix'), findsOneWidget);
    expect(find.text('收件箱'), findsWidgets);
    expect(find.text('分类'), findsOneWidget);
    expect(find.text('Tech'), findsWidgets);
    expect(find.textContaining('1 条内容'), findsWidgets);
    expect(find.textContaining('Core ok'), findsOneWidget);
    expect(find.text('智能订阅'), findsOneWidget);
    expect(find.text('刷新'), findsOneWidget);
  });

  testWidgets('selecting an unread item marks it read', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1600, 1200));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    final backend = _RecordingBackend();

    await tester.pumpWidget(
      InfoMatrixApp(backendFactory: () => backend),
    );

    await tester.pumpAndSettle();
    await tester.tap(find.text('Example Feed').first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Hello InfoMatrix').first);
    await tester.pumpAndSettle();

    expect(backend.patchCalls, isNotEmpty);
    expect(
      backend.patchCalls.any((call) => call['is_read'] == true),
      isTrue,
    );
  });

  testWidgets('load more expands the feed list limit', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1600, 1200));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    final backend = _PagedBackend();

    await tester.pumpWidget(
      InfoMatrixApp(backendFactory: () => backend),
    );

    await tester.pumpAndSettle();
    await tester.tap(find.text('Example Feed').first);
    await tester.pumpAndSettle();

    expect(find.text('加载更多'), findsOneWidget);
    await tester.tap(find.text('加载更多'));
    await tester.pumpAndSettle();

    expect(backend.listItemLimits, isNotEmpty);
    expect(backend.listItemLimits.first, 250);
    expect(backend.listItemLimits.last, 350);
  });
}
