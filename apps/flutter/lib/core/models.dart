class CoreHealth {
  const CoreHealth({required this.status, required this.version});

  final String status;
  final String version;

  factory CoreHealth.fromJson(Map<String, dynamic> json) {
    return CoreHealth(
      status: (json['status'] as String?) ?? 'unknown',
      version: (json['version'] as String?) ?? 'unknown',
    );
  }
}

class FeedGroupModel {
  const FeedGroupModel({
    required this.id,
    required this.name,
  });

  final String id;
  final String name;

  factory FeedGroupModel.fromJson(Map<String, dynamic> json) {
    return FeedGroupModel(
      id: json['id'] as String,
      name: json['name'] as String? ?? 'Untitled group',
    );
  }
}

class FeedModel {
  const FeedModel({
    required this.id,
    required this.title,
    required this.feedUrl,
    required this.feedType,
    this.autoFullText = true,
    this.groups = const <FeedGroupModel>[],
    this.siteUrl,
    this.iconUrl,
  });

  final String id;
  final String title;
  final String feedUrl;
  final String? siteUrl;
  final String? iconUrl;
  final String feedType;
  final bool autoFullText;
  final List<FeedGroupModel> groups;

  factory FeedModel.fromJson(Map<String, dynamic> json) {
    return FeedModel(
      id: json['id'] as String,
      title: json['title'] as String? ?? 'Untitled',
      feedUrl: json['feed_url'] as String,
      siteUrl: json['site_url'] as String?,
      iconUrl: json['icon_url'] as String?,
      feedType: json['feed_type'] as String? ?? 'unknown',
      autoFullText: (json['auto_full_text'] as bool?) ?? true,
      groups: (json['groups'] as List<dynamic>? ?? const <dynamic>[])
          .whereType<Map<String, dynamic>>()
          .map(FeedGroupModel.fromJson)
          .toList(growable: false),
    );
  }
}

class ItemModel {
  const ItemModel({
    required this.id,
    required this.title,
    required this.kind,
    required this.sourceKind,
    required this.isRead,
    required this.isStarred,
    required this.isSavedForLater,
    this.sourceID,
    this.sourceURL,
    this.sourceTitle,
    this.canonicalUrl,
    this.publishedAt,
    this.summaryPreview,
    this.summary,
    this.contentHTML,
    this.contentText,
    this.isArchived = false,
  });

  final String id;
  final String title;
  final String kind;
  final String sourceKind;
  final String? sourceID;
  final String? sourceURL;
  final String? sourceTitle;
  final String? canonicalUrl;
  final String? publishedAt;
  final String? summaryPreview;
  final String? summary;
  final String? contentHTML;
  final String? contentText;
  final bool isRead;
  final bool isStarred;
  final bool isSavedForLater;
  final bool isArchived;

  ItemModel copyWith({
    String? kind,
    String? sourceKind,
    String? sourceID,
    String? sourceURL,
    String? sourceTitle,
    String? canonicalUrl,
    String? publishedAt,
    String? summaryPreview,
    String? summary,
    String? contentHTML,
    String? contentText,
    bool? isRead,
    bool? isStarred,
    bool? isSavedForLater,
    bool? isArchived,
  }) {
    return ItemModel(
      id: id,
      title: title,
      kind: kind ?? this.kind,
      sourceKind: sourceKind ?? this.sourceKind,
      sourceID: sourceID ?? this.sourceID,
      sourceURL: sourceURL ?? this.sourceURL,
      sourceTitle: sourceTitle ?? this.sourceTitle,
      canonicalUrl: canonicalUrl ?? this.canonicalUrl,
      publishedAt: publishedAt ?? this.publishedAt,
      summaryPreview: summaryPreview ?? this.summaryPreview,
      summary: summary ?? this.summary,
      contentHTML: contentHTML ?? this.contentHTML,
      contentText: contentText ?? this.contentText,
      isRead: isRead ?? this.isRead,
      isStarred: isStarred ?? this.isStarred,
      isSavedForLater: isSavedForLater ?? this.isSavedForLater,
      isArchived: isArchived ?? this.isArchived,
    );
  }

  factory ItemModel.fromJson(Map<String, dynamic> json) {
    return ItemModel(
      id: json['id'] as String,
      title: json['title'] as String? ?? '(Untitled item)',
      kind: json['kind'] as String? ?? 'article',
      sourceKind: json['source_kind'] as String? ?? 'feed',
      sourceID: json['source_id'] as String?,
      sourceURL: json['source_url'] as String?,
      sourceTitle: json['source_title'] as String?,
      canonicalUrl: json['canonical_url'] as String?,
      publishedAt: json['published_at'] as String?,
      summaryPreview: json['summary_preview'] as String?,
      summary: json['summary'] as String?,
      contentHTML: json['content_html'] as String?,
      contentText: json['content_text'] as String?,
      isRead: (json['is_read'] as bool?) ?? false,
      isStarred: (json['is_starred'] as bool?) ?? false,
      isSavedForLater: (json['is_saved_for_later'] as bool?) ?? false,
      isArchived: (json['is_archived'] as bool?) ?? false,
    );
  }
}

class ItemScopeCounts {
  const ItemScopeCounts({
    required this.all,
    required this.unread,
    required this.starred,
    required this.later,
    required this.notes,
    required this.archive,
  });

  final int all;
  final int unread;
  final int starred;
  final int later;
  final int notes;
  final int archive;

  factory ItemScopeCounts.fromJson(Map<String, dynamic> json) {
    return ItemScopeCounts(
      all: (json['all'] as num?)?.toInt() ?? 0,
      unread: (json['unread'] as num?)?.toInt() ?? 0,
      starred: (json['starred'] as num?)?.toInt() ?? 0,
      later: (json['later'] as num?)?.toInt() ?? 0,
      notes: (json['notes'] as num?)?.toInt() ?? 0,
      archive: (json['archive'] as num?)?.toInt() ?? 0,
    );
  }
}

enum NotificationMode { immediate, digest }

extension NotificationModeX on NotificationMode {
  String get jsonValue => switch (this) {
        NotificationMode.immediate => 'immediate',
        NotificationMode.digest => 'digest',
      };

  static NotificationMode fromJson(Object? value) {
    final normalized = value?.toString().toLowerCase();
    return switch (normalized) {
      'digest' => NotificationMode.digest,
      _ => NotificationMode.immediate,
    };
  }
}

enum NotificationDeliveryState { pending, delivered, suppressed }

extension NotificationDeliveryStateX on NotificationDeliveryState {
  String get jsonValue => switch (this) {
        NotificationDeliveryState.pending => 'pending',
        NotificationDeliveryState.delivered => 'delivered',
        NotificationDeliveryState.suppressed => 'suppressed',
      };

  static NotificationDeliveryState fromJson(Object? value) {
    final normalized = value?.toString().toLowerCase();
    return switch (normalized) {
      'delivered' => NotificationDeliveryState.delivered,
      'suppressed' => NotificationDeliveryState.suppressed,
      _ => NotificationDeliveryState.pending,
    };
  }
}

class QuietHours {
  const QuietHours({
    required this.enabled,
    required this.startMinute,
    required this.endMinute,
  });

  final bool enabled;
  final int startMinute;
  final int endMinute;

  QuietHours copyWith({
    bool? enabled,
    int? startMinute,
    int? endMinute,
  }) {
    return QuietHours(
      enabled: enabled ?? this.enabled,
      startMinute: startMinute ?? this.startMinute,
      endMinute: endMinute ?? this.endMinute,
    );
  }

  factory QuietHours.fromJson(Map<String, dynamic> json) {
    return QuietHours(
      enabled: json['enabled'] as bool? ?? false,
      startMinute: (json['start_minute'] as num?)?.toInt() ?? 22 * 60,
      endMinute: (json['end_minute'] as num?)?.toInt() ?? 7 * 60,
    );
  }

  Map<String, dynamic> toJson() {
    return <String, dynamic>{
      'enabled': enabled,
      'start_minute': startMinute,
      'end_minute': endMinute,
    };
  }
}

class DigestPolicy {
  const DigestPolicy({
    required this.enabled,
    required this.intervalMinutes,
    required this.maxItems,
  });

  final bool enabled;
  final int intervalMinutes;
  final int maxItems;

  DigestPolicy copyWith({
    bool? enabled,
    int? intervalMinutes,
    int? maxItems,
  }) {
    return DigestPolicy(
      enabled: enabled ?? this.enabled,
      intervalMinutes: intervalMinutes ?? this.intervalMinutes,
      maxItems: maxItems ?? this.maxItems,
    );
  }

  factory DigestPolicy.fromJson(Map<String, dynamic> json) {
    return DigestPolicy(
      enabled: json['enabled'] as bool? ?? false,
      intervalMinutes: (json['interval_minutes'] as num?)?.toInt() ?? 60,
      maxItems: (json['max_items'] as num?)?.toInt() ?? 20,
    );
  }

  Map<String, dynamic> toJson() {
    return <String, dynamic>{
      'enabled': enabled,
      'interval_minutes': intervalMinutes,
      'max_items': maxItems,
    };
  }
}

class NotificationSettings {
  const NotificationSettings({
    required this.enabled,
    required this.mode,
    required this.digestPolicy,
    required this.quietHours,
    required this.minimumIntervalMinutes,
    required this.highPriority,
    required this.keywordInclude,
    required this.keywordExclude,
  });

  final bool enabled;
  final NotificationMode mode;
  final DigestPolicy digestPolicy;
  final QuietHours quietHours;
  final int minimumIntervalMinutes;
  final bool highPriority;
  final List<String> keywordInclude;
  final List<String> keywordExclude;

  NotificationSettings copyWith({
    bool? enabled,
    NotificationMode? mode,
    DigestPolicy? digestPolicy,
    QuietHours? quietHours,
    int? minimumIntervalMinutes,
    bool? highPriority,
    List<String>? keywordInclude,
    List<String>? keywordExclude,
  }) {
    return NotificationSettings(
      enabled: enabled ?? this.enabled,
      mode: mode ?? this.mode,
      digestPolicy: digestPolicy ?? this.digestPolicy,
      quietHours: quietHours ?? this.quietHours,
      minimumIntervalMinutes:
          minimumIntervalMinutes ?? this.minimumIntervalMinutes,
      highPriority: highPriority ?? this.highPriority,
      keywordInclude: keywordInclude ?? this.keywordInclude,
      keywordExclude: keywordExclude ?? this.keywordExclude,
    );
  }

  factory NotificationSettings.fromJson(Map<String, dynamic> json) {
    return NotificationSettings(
      enabled: json['enabled'] as bool? ?? false,
      mode: NotificationModeX.fromJson(json['mode']),
      digestPolicy: DigestPolicy.fromJson(
        (json['digest_policy'] as Map<String, dynamic>?) ?? <String, dynamic>{},
      ),
      quietHours: QuietHours.fromJson(
        (json['quiet_hours'] as Map<String, dynamic>?) ?? <String, dynamic>{},
      ),
      minimumIntervalMinutes:
          (json['minimum_interval_minutes'] as num?)?.toInt() ?? 60,
      highPriority: json['high_priority'] as bool? ?? false,
      keywordInclude:
          (json['keyword_include'] as List<dynamic>? ?? const <dynamic>[])
              .map((dynamic value) => value.toString())
              .toList(growable: false),
      keywordExclude:
          (json['keyword_exclude'] as List<dynamic>? ?? const <dynamic>[])
              .map((dynamic value) => value.toString())
              .toList(growable: false),
    );
  }

  Map<String, dynamic> toJson() {
    return <String, dynamic>{
      'enabled': enabled,
      'mode': mode.jsonValue,
      'digest_policy': digestPolicy.toJson(),
      'quiet_hours': quietHours.toJson(),
      'minimum_interval_minutes': minimumIntervalMinutes,
      'high_priority': highPriority,
      'keyword_include': keywordInclude,
      'keyword_exclude': keywordExclude,
    };
  }
}

class RefreshSettings {
  const RefreshSettings({
    required this.enabled,
    required this.intervalMinutes,
  });

  final bool enabled;
  final int intervalMinutes;

  RefreshSettings copyWith({
    bool? enabled,
    int? intervalMinutes,
  }) {
    return RefreshSettings(
      enabled: enabled ?? this.enabled,
      intervalMinutes: intervalMinutes ?? this.intervalMinutes,
    );
  }

  factory RefreshSettings.fromJson(Map<String, dynamic> json) {
    return RefreshSettings(
      enabled: (json['enabled'] as bool?) ?? true,
      intervalMinutes: (json['interval_minutes'] as num?)?.toInt() ?? 15,
    );
  }

  Map<String, dynamic> toJson() {
    return <String, dynamic>{
      'enabled': enabled,
      'interval_minutes': intervalMinutes,
    };
  }
}

class GlobalNotificationSettings {
  const GlobalNotificationSettings({
    required this.backgroundRefreshEnabled,
    required this.backgroundRefreshIntervalMinutes,
    required this.digestPolicy,
    required this.defaultFeedSettings,
  });

  final bool backgroundRefreshEnabled;
  final int backgroundRefreshIntervalMinutes;
  final DigestPolicy digestPolicy;
  final NotificationSettings defaultFeedSettings;

  GlobalNotificationSettings copyWith({
    bool? backgroundRefreshEnabled,
    int? backgroundRefreshIntervalMinutes,
    DigestPolicy? digestPolicy,
    NotificationSettings? defaultFeedSettings,
  }) {
    return GlobalNotificationSettings(
      backgroundRefreshEnabled:
          backgroundRefreshEnabled ?? this.backgroundRefreshEnabled,
      backgroundRefreshIntervalMinutes: backgroundRefreshIntervalMinutes ??
          this.backgroundRefreshIntervalMinutes,
      digestPolicy: digestPolicy ?? this.digestPolicy,
      defaultFeedSettings: defaultFeedSettings ?? this.defaultFeedSettings,
    );
  }

  factory GlobalNotificationSettings.fromJson(Map<String, dynamic> json) {
    return GlobalNotificationSettings(
      backgroundRefreshEnabled:
          json['background_refresh_enabled'] as bool? ?? true,
      backgroundRefreshIntervalMinutes:
          (json['background_refresh_interval_minutes'] as num?)?.toInt() ?? 15,
      digestPolicy: DigestPolicy.fromJson(
        (json['digest_policy'] as Map<String, dynamic>?) ?? <String, dynamic>{},
      ),
      defaultFeedSettings: NotificationSettings.fromJson(
        (json['default_feed_settings'] as Map<String, dynamic>?) ??
            <String, dynamic>{},
      ),
    );
  }

  Map<String, dynamic> toJson() {
    return <String, dynamic>{
      'background_refresh_enabled': backgroundRefreshEnabled,
      'background_refresh_interval_minutes': backgroundRefreshIntervalMinutes,
      'digest_policy': digestPolicy.toJson(),
      'default_feed_settings': defaultFeedSettings.toJson(),
    };
  }
}

class NotificationEventModel {
  const NotificationEventModel({
    required this.id,
    required this.title,
    required this.body,
    required this.mode,
    required this.deliveryState,
    required this.reason,
    required this.createdAt,
    required this.readyAt,
    required this.deliveredAt,
    required this.suppressedAt,
    this.feedId,
    this.entryId,
    this.digestId,
  });

  final String id;
  final String? feedId;
  final String? entryId;
  final String? digestId;
  final String title;
  final String body;
  final NotificationMode mode;
  final NotificationDeliveryState deliveryState;
  final String reason;
  final String createdAt;
  final String? readyAt;
  final String? deliveredAt;
  final String? suppressedAt;

  factory NotificationEventModel.fromJson(Map<String, dynamic> json) {
    return NotificationEventModel(
      id: json['id'] as String? ?? '',
      feedId: json['feed_id'] as String?,
      entryId: json['entry_id'] as String?,
      digestId: json['digest_id'] as String?,
      title: json['title'] as String? ?? '',
      body: json['body'] as String? ?? '',
      mode: NotificationModeX.fromJson(json['mode']),
      deliveryState:
          NotificationDeliveryStateX.fromJson(json['delivery_state']),
      reason: json['reason'] as String? ?? 'unknown',
      createdAt: json['created_at'] as String? ?? '',
      readyAt: json['ready_at'] as String?,
      deliveredAt: json['delivered_at'] as String?,
      suppressedAt: json['suppressed_at'] as String?,
    );
  }
}

class DiscoverFeedCandidate {
  const DiscoverFeedCandidate({
    required this.url,
    required this.feedType,
    required this.confidence,
    required this.source,
    required this.score,
    this.title,
  });

  final String url;
  final String? title;
  final String feedType;
  final double confidence;
  final String source;
  final int score;

  factory DiscoverFeedCandidate.fromJson(Map<String, dynamic> json) {
    return DiscoverFeedCandidate(
      url: json['url'] as String,
      title: json['title'] as String?,
      feedType: json['feed_type'] as String? ?? 'unknown',
      confidence: (json['confidence'] as num?)?.toDouble() ?? 0,
      source: json['source'] as String? ?? 'unknown',
      score: (json['score'] as num?)?.toInt() ?? 0,
    );
  }
}

class DiscoverResult {
  const DiscoverResult({
    required this.normalizedSiteUrl,
    required this.discoveredFeeds,
    required this.warnings,
    this.siteTitle,
  });

  final String normalizedSiteUrl;
  final List<DiscoverFeedCandidate> discoveredFeeds;
  final String? siteTitle;
  final List<String> warnings;

  factory DiscoverResult.fromJson(Map<String, dynamic> json) {
    final feeds = (json['discovered_feeds'] as List<dynamic>? ?? <dynamic>[])
        .whereType<Map<String, dynamic>>()
        .map(DiscoverFeedCandidate.fromJson)
        .toList(growable: false);

    final warnings = (json['warnings'] as List<dynamic>? ?? <dynamic>[])
        .map((dynamic value) => value.toString())
        .toList(growable: false);

    return DiscoverResult(
      normalizedSiteUrl: json['normalized_site_url'] as String? ?? '',
      discoveredFeeds: feeds,
      siteTitle: json['site_title'] as String?,
      warnings: warnings,
    );
  }
}

class RefreshResult {
  const RefreshResult({
    required this.status,
    required this.fetchedHttpStatus,
    required this.itemCount,
    this.notificationCount = 0,
    this.suppressedNotificationCount = 0,
  });

  final String status;
  final int fetchedHttpStatus;
  final int itemCount;
  final int notificationCount;
  final int suppressedNotificationCount;

  factory RefreshResult.fromJson(Map<String, dynamic> json) {
    return RefreshResult(
      status: json['status'] as String? ?? 'unknown',
      fetchedHttpStatus: (json['fetched_http_status'] as num?)?.toInt() ?? 0,
      itemCount: (json['item_count'] as num?)?.toInt() ?? 0,
      notificationCount: (json['notification_count'] as num?)?.toInt() ?? 0,
      suppressedNotificationCount:
          (json['suppressed_notification_count'] as num?)?.toInt() ?? 0,
    );
  }
}

class RefreshDueResult {
  const RefreshDueResult({
    required this.refreshedCount,
    required this.totalItemCount,
  });

  final int refreshedCount;
  final int totalItemCount;

  factory RefreshDueResult.fromJson(Map<String, dynamic> json) {
    return RefreshDueResult(
      refreshedCount: (json['refreshed_count'] as num?)?.toInt() ?? 0,
      totalItemCount: (json['total_item_count'] as num?)?.toInt() ?? 0,
    );
  }
}

class SubscriptionResultModel {
  const SubscriptionResultModel({
    required this.feedId,
    required this.resolvedFeedUrl,
    required this.subscriptionSource,
  });

  final String feedId;
  final String resolvedFeedUrl;
  final String subscriptionSource;

  factory SubscriptionResultModel.fromJson(Map<String, dynamic> json) {
    return SubscriptionResultModel(
      feedId: json['feed_id'] as String,
      resolvedFeedUrl: json['resolved_feed_url'] as String,
      subscriptionSource: json['subscription_source'] as String? ?? 'unknown',
    );
  }
}

class OpmlImportResult {
  const OpmlImportResult({
    required this.parsedFeedCount,
    required this.uniqueFeedCount,
    required this.groupedFeedCount,
  });

  final int parsedFeedCount;
  final int uniqueFeedCount;
  final int groupedFeedCount;

  factory OpmlImportResult.fromJson(Map<String, dynamic> json) {
    return OpmlImportResult(
      parsedFeedCount: (json['parsed_feed_count'] as num?)?.toInt() ?? 0,
      uniqueFeedCount: (json['unique_feed_count'] as num?)?.toInt() ?? 0,
      groupedFeedCount: (json['grouped_feed_count'] as num?)?.toInt() ?? 0,
    );
  }
}

class OpmlExportResult {
  const OpmlExportResult({
    required this.opmlXml,
    required this.feedCount,
  });

  final String opmlXml;
  final int feedCount;

  factory OpmlExportResult.fromJson(Map<String, dynamic> json) {
    return OpmlExportResult(
      opmlXml: json['opml_xml'] as String? ?? '',
      feedCount: (json['feed_count'] as num?)?.toInt() ?? 0,
    );
  }
}
