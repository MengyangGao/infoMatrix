import 'models.dart';

class ReaderBackendException implements Exception {
  ReaderBackendException(this.message);

  final String message;

  @override
  String toString() => 'ReaderBackendException: $message';
}

abstract class ReaderBackend {
  Future<CoreHealth> health();

  Future<String> defaultDbPath();

  Future<List<FeedModel>> listFeeds();

  Future<List<FeedGroupModel>> listGroups();

  Future<FeedGroupModel> createGroup(String name);

  Future<void> updateFeed(
    String feedId, {
    String? title,
    bool? autoFullText,
  });

  Future<void> updateFeedGroup(
    String feedId, {
    String? groupId,
  });

  Future<void> deleteFeed(String feedId);

  Future<List<ItemModel>> listItems(
    String feedId, {
    int limit = 100,
    String? searchQuery,
  });

  Future<List<ItemModel>> listAllItems({
    int limit = 200,
    String? searchQuery,
    String filter = 'all',
    String? kind,
  });

  Future<ItemScopeCounts> itemCounts();

  Future<String> addSubscription(String feedUrl, {String? title});

  Future<SubscriptionResultModel> subscribeInput(String inputUrl);

  Future<DiscoverResult> discoverSite(String siteUrl);

  Future<ItemModel> itemDetail(String itemID);

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
  });

  Future<ItemModel> fetchFullText(String itemID);

  Future<OpmlImportResult> importOpml(String opmlXml);

  Future<OpmlExportResult> exportOpml();

  Future<RefreshResult> refreshFeed(String feedId);

  Future<RefreshDueResult> refreshDueFeeds({int limit = 20});

  Future<ItemModel> patchItemState(
    ItemModel item, {
    bool? isRead,
    bool? isStarred,
    bool? isSavedForLater,
    bool? isArchived,
  });

  Future<GlobalNotificationSettings> globalNotificationSettings();

  Future<GlobalNotificationSettings> updateGlobalNotificationSettings(
    GlobalNotificationSettings settings,
  );

  Future<NotificationSettings> feedNotificationSettings(String feedId);

  Future<NotificationSettings> updateFeedNotificationSettings(
    String feedId,
    NotificationSettings settings,
  );

  Future<List<NotificationEventModel>> listPendingNotificationEvents({
    int limit = 50,
  });

  Future<int> acknowledgeNotificationEvents(List<String> eventIDs);
}
