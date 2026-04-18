import Foundation

private extension KeyedDecodingContainer {
    func decodeLossyURLIfPresent(forKey key: Key) throws -> URL? {
        if let url = try? decodeIfPresent(URL.self, forKey: key) {
            return url
        }
        if let raw = try decodeIfPresent(String.self, forKey: key) {
            return URL(string: raw.trimmingCharacters(in: .whitespacesAndNewlines))
        }
        return nil
    }
}

public struct FeedGroup: Identifiable, Equatable, Hashable, Codable, Sendable {
    public let id: String
    public let name: String
}

public struct Feed: Identifiable, Equatable, Codable, Sendable {
    public let id: String
    public var title: String
    public var feedURL: URL
    public var siteURL: URL?
    public var feedType: String
    public var autoFullText: Bool
    public var iconURL: URL?
    public var groups: [FeedGroup]

    public init(
        id: String,
        title: String,
        feedURL: URL,
        siteURL: URL?,
        feedType: String,
        autoFullText: Bool = true,
        iconURL: URL? = nil,
        groups: [FeedGroup] = []
    ) {
        self.id = id
        self.title = title
        self.feedURL = feedURL
        self.siteURL = siteURL
        self.feedType = feedType
        self.autoFullText = autoFullText
        self.iconURL = iconURL
        self.groups = groups
    }

    enum CodingKeys: String, CodingKey {
        case id
        case title
        case feedURL = "feed_url"
        case siteURL = "site_url"
        case feedType = "feed_type"
        case autoFullText = "auto_full_text"
        case iconURL = "icon_url"
        case groups
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        title = try container.decodeIfPresent(String.self, forKey: .title) ?? "Untitled"
        feedURL = try container.decode(URL.self, forKey: .feedURL)
        siteURL = try container.decodeLossyURLIfPresent(forKey: .siteURL)
        feedType = try container.decodeIfPresent(String.self, forKey: .feedType) ?? "unknown"
        autoFullText = try container.decodeIfPresent(Bool.self, forKey: .autoFullText) ?? true
        iconURL = try container.decodeLossyURLIfPresent(forKey: .iconURL)
        groups = try container.decodeIfPresent([FeedGroup].self, forKey: .groups) ?? []
    }
}

public struct ArticleItem: Identifiable, Equatable, Codable, Sendable {
    public let id: String
    public var title: String
    public var kind: String
    public var sourceKind: String
    public var sourceID: String?
    public var sourceURL: URL?
    public var canonicalURL: URL?
    public var publishedAt: String?
    public var summaryPreview: String?
    public var isRead: Bool
    public var isStarred: Bool
    public var isSavedForLater: Bool
    public var isArchived: Bool

    public init(
        id: String,
        title: String,
        kind: String = "article",
        sourceKind: String = "feed",
        sourceID: String? = nil,
        sourceURL: URL? = nil,
        canonicalURL: URL?,
        publishedAt: String?,
        summaryPreview: String? = nil,
        isRead: Bool,
        isStarred: Bool,
        isSavedForLater: Bool,
        isArchived: Bool = false
    ) {
        self.id = id
        self.title = title
        self.kind = kind
        self.sourceKind = sourceKind
        self.sourceID = sourceID
        self.sourceURL = sourceURL
        self.canonicalURL = canonicalURL
        self.publishedAt = publishedAt
        self.summaryPreview = summaryPreview
        self.isRead = isRead
        self.isStarred = isStarred
        self.isSavedForLater = isSavedForLater
        self.isArchived = isArchived
    }

    enum CodingKeys: String, CodingKey {
        case id
        case title
        case kind
        case sourceKind = "source_kind"
        case sourceID = "source_id"
        case sourceURL = "source_url"
        case canonicalURL = "canonical_url"
        case publishedAt = "published_at"
        case summaryPreview = "summary_preview"
        case isRead = "is_read"
        case isStarred = "is_starred"
        case isSavedForLater = "is_saved_for_later"
        case isArchived = "is_archived"
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        title = try container.decodeIfPresent(String.self, forKey: .title) ?? "(Untitled item)"
        kind = try container.decodeIfPresent(String.self, forKey: .kind) ?? "article"
        sourceKind = try container.decodeIfPresent(String.self, forKey: .sourceKind) ?? "feed"
        sourceID = try container.decodeIfPresent(String.self, forKey: .sourceID)
        sourceURL = try container.decodeLossyURLIfPresent(forKey: .sourceURL)
        canonicalURL = try container.decodeLossyURLIfPresent(forKey: .canonicalURL)
        publishedAt = try container.decodeIfPresent(String.self, forKey: .publishedAt)
        summaryPreview = try container.decodeIfPresent(String.self, forKey: .summaryPreview)
        isRead = try container.decodeIfPresent(Bool.self, forKey: .isRead) ?? false
        isStarred = try container.decodeIfPresent(Bool.self, forKey: .isStarred) ?? false
        isSavedForLater = try container.decodeIfPresent(Bool.self, forKey: .isSavedForLater) ?? false
        isArchived = try container.decodeIfPresent(Bool.self, forKey: .isArchived) ?? false
    }
}

public struct ItemScopeCounts: Equatable, Codable, Sendable {
    public let all: Int
    public let unread: Int
    public let starred: Int
    public let later: Int
    public let notes: Int
    public let archive: Int
}

public struct DiscoverFeed: Identifiable, Hashable, Codable, Sendable {
    public let url: URL
    public let title: String?
    public let feedType: String
    public let confidence: Double
    public let source: String?
    public let score: Int?

    public var id: String {
        url.absoluteString
    }

    enum CodingKeys: String, CodingKey {
        case url
        case title
        case feedType = "feed_type"
        case confidence
        case source
        case score
    }
}

public struct DiscoverSiteResponse: Codable, Sendable {
    public let normalizedSiteURL: URL
    public let discoveredFeeds: [DiscoverFeed]
    public let siteTitle: String?
    public let warnings: [String]

    enum CodingKeys: String, CodingKey {
        case normalizedSiteURL = "normalized_site_url"
        case discoveredFeeds = "discovered_feeds"
        case siteTitle = "site_title"
        case warnings
    }
}

public struct SubscriptionResult: Codable, Sendable {
    public let feedID: String
    public let resolvedFeedURL: URL
    public let subscriptionSource: String

    enum CodingKeys: String, CodingKey {
        case feedID = "feed_id"
        case resolvedFeedURL = "resolved_feed_url"
        case subscriptionSource = "subscription_source"
    }
}

public struct OPMLImportResult: Codable, Sendable {
    public let parsedFeedCount: Int
    public let uniqueFeedCount: Int
    public let groupedFeedCount: Int

    enum CodingKeys: String, CodingKey {
        case parsedFeedCount = "parsed_feed_count"
        case uniqueFeedCount = "unique_feed_count"
        case groupedFeedCount = "grouped_feed_count"
    }
}

public struct OPMLExportResult: Codable, Sendable {
    public let opmlXML: String
    public let feedCount: Int

    enum CodingKeys: String, CodingKey {
        case opmlXML = "opml_xml"
        case feedCount = "feed_count"
    }
}

public struct RefreshDueResult: Codable, Sendable {
    public let refreshedCount: Int
    public let totalItemCount: Int

    enum CodingKeys: String, CodingKey {
        case refreshedCount = "refreshed_count"
        case totalItemCount = "total_item_count"
    }
}

public enum NotificationMode: String, Codable, Hashable, Sendable {
    case immediate
    case digest
}

public enum NotificationDeliveryState: String, Codable, Hashable, Sendable {
    case pending
    case delivered
    case suppressed
}

public struct QuietHours: Codable, Equatable, Sendable {
    public var enabled: Bool
    public var startMinute: Int
    public var endMinute: Int

    public init(enabled: Bool, startMinute: Int, endMinute: Int) {
        self.enabled = enabled
        self.startMinute = startMinute
        self.endMinute = endMinute
    }

    enum CodingKeys: String, CodingKey {
        case enabled
        case startMinute = "start_minute"
        case endMinute = "end_minute"
    }
}

public struct DigestPolicy: Codable, Equatable, Sendable {
    public var enabled: Bool
    public var intervalMinutes: Int
    public var maxItems: Int

    public init(enabled: Bool, intervalMinutes: Int, maxItems: Int) {
        self.enabled = enabled
        self.intervalMinutes = intervalMinutes
        self.maxItems = maxItems
    }

    enum CodingKeys: String, CodingKey {
        case enabled
        case intervalMinutes = "interval_minutes"
        case maxItems = "max_items"
    }
}

public struct NotificationSettings: Codable, Equatable, Sendable {
    public var enabled: Bool
    public var mode: NotificationMode
    public var digestPolicy: DigestPolicy
    public var quietHours: QuietHours
    public var minimumIntervalMinutes: Int
    public var highPriority: Bool
    public var keywordInclude: [String]
    public var keywordExclude: [String]

    public init(
        enabled: Bool,
        mode: NotificationMode,
        digestPolicy: DigestPolicy,
        quietHours: QuietHours,
        minimumIntervalMinutes: Int,
        highPriority: Bool,
        keywordInclude: [String],
        keywordExclude: [String]
    ) {
        self.enabled = enabled
        self.mode = mode
        self.digestPolicy = digestPolicy
        self.quietHours = quietHours
        self.minimumIntervalMinutes = minimumIntervalMinutes
        self.highPriority = highPriority
        self.keywordInclude = keywordInclude
        self.keywordExclude = keywordExclude
    }

    enum CodingKeys: String, CodingKey {
        case enabled
        case mode
        case digestPolicy = "digest_policy"
        case quietHours = "quiet_hours"
        case minimumIntervalMinutes = "minimum_interval_minutes"
        case highPriority = "high_priority"
        case keywordInclude = "keyword_include"
        case keywordExclude = "keyword_exclude"
    }
}

public struct RefreshSettings: Codable, Equatable, Sendable {
    public var enabled: Bool
    public var intervalMinutes: Int

    public init(enabled: Bool = true, intervalMinutes: Int = 15) {
        self.enabled = enabled
        self.intervalMinutes = max(1, intervalMinutes)
    }

    enum CodingKeys: String, CodingKey {
        case enabled
        case intervalMinutes = "interval_minutes"
    }
}

public struct GlobalNotificationSettings: Codable, Equatable, Sendable {
    public var backgroundRefreshEnabled: Bool
    public var backgroundRefreshIntervalMinutes: Int
    public var digestPolicy: DigestPolicy
    public var defaultFeedSettings: NotificationSettings

    public init(
        backgroundRefreshEnabled: Bool,
        backgroundRefreshIntervalMinutes: Int,
        digestPolicy: DigestPolicy,
        defaultFeedSettings: NotificationSettings
    ) {
        self.backgroundRefreshEnabled = backgroundRefreshEnabled
        self.backgroundRefreshIntervalMinutes = backgroundRefreshIntervalMinutes
        self.digestPolicy = digestPolicy
        self.defaultFeedSettings = defaultFeedSettings
    }

    enum CodingKeys: String, CodingKey {
        case backgroundRefreshEnabled = "background_refresh_enabled"
        case backgroundRefreshIntervalMinutes = "background_refresh_interval_minutes"
        case digestPolicy = "digest_policy"
        case defaultFeedSettings = "default_feed_settings"
    }
}

public struct NotificationEvent: Identifiable, Codable, Sendable {
    public let id: String
    public let feedID: String?
    public let entryID: String?
    public let canonicalKey: String
    public let contentFingerprint: String
    public let title: String
    public let body: String
    public let mode: NotificationMode
    public let deliveryState: NotificationDeliveryState
    public let reason: String
    public let digestID: String?
    public let createdAt: String
    public let readyAt: String?
    public let deliveredAt: String?
    public let suppressedAt: String?

    enum CodingKeys: String, CodingKey {
        case id
        case feedID = "feed_id"
        case entryID = "entry_id"
        case canonicalKey = "canonical_key"
        case contentFingerprint = "content_fingerprint"
        case title
        case body
        case mode
        case deliveryState = "delivery_state"
        case reason
        case digestID = "digest_id"
        case createdAt = "created_at"
        case readyAt = "ready_at"
        case deliveredAt = "delivered_at"
        case suppressedAt = "suppressed_at"
    }
}

public struct NotificationDigest: Identifiable, Codable, Sendable {
    public let id: String
    public let feedID: String?
    public let entryCount: Int
    public let title: String
    public let body: String
    public let createdAt: String
    public let readyAt: String?
    public let deliveredAt: String?

    enum CodingKeys: String, CodingKey {
        case id
        case feedID = "feed_id"
        case entryCount = "entry_count"
        case title
        case body
        case createdAt = "created_at"
        case readyAt = "ready_at"
        case deliveredAt = "delivered_at"
    }
}

public struct PushEndpointRegistration: Identifiable, Codable, Sendable {
    public let id: String
    public let platform: String
    public let endpoint: String
    public let enabled: Bool
    public let createdAt: String
    public let updatedAt: String

    enum CodingKeys: String, CodingKey {
        case id
        case platform
        case endpoint
        case enabled
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

public struct ArticleDetail: Codable, Sendable {
    public let id: String
    public let title: String
    public let kind: String
    public let sourceKind: String
    public let sourceID: String?
    public let sourceURL: URL?
    public let canonicalURL: URL?
    public let publishedAt: String?
    public let summary: String?
    public let contentHTML: String?
    public let contentText: String?
    public let isRead: Bool
    public let isStarred: Bool
    public let isSavedForLater: Bool
    public let isArchived: Bool

    public init(
        id: String,
        title: String,
        kind: String = "article",
        sourceKind: String = "feed",
        sourceID: String? = nil,
        sourceURL: URL? = nil,
        canonicalURL: URL?,
        publishedAt: String?,
        summary: String?,
        contentHTML: String?,
        contentText: String?,
        isRead: Bool,
        isStarred: Bool,
        isSavedForLater: Bool,
        isArchived: Bool = false
    ) {
        self.id = id
        self.title = title
        self.kind = kind
        self.sourceKind = sourceKind
        self.sourceID = sourceID
        self.sourceURL = sourceURL
        self.canonicalURL = canonicalURL
        self.publishedAt = publishedAt
        self.summary = summary
        self.contentHTML = contentHTML
        self.contentText = contentText
        self.isRead = isRead
        self.isStarred = isStarred
        self.isSavedForLater = isSavedForLater
        self.isArchived = isArchived
    }

    enum CodingKeys: String, CodingKey {
        case id
        case title
        case kind
        case sourceKind = "source_kind"
        case sourceID = "source_id"
        case sourceURL = "source_url"
        case canonicalURL = "canonical_url"
        case publishedAt = "published_at"
        case summary
        case contentHTML = "content_html"
        case contentText = "content_text"
        case isRead = "is_read"
        case isStarred = "is_starred"
        case isSavedForLater = "is_saved_for_later"
        case isArchived = "is_archived"
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        title = try container.decode(String.self, forKey: .title)
        kind = try container.decodeIfPresent(String.self, forKey: .kind) ?? "article"
        sourceKind = try container.decodeIfPresent(String.self, forKey: .sourceKind) ?? "feed"
        sourceID = try container.decodeIfPresent(String.self, forKey: .sourceID)
        sourceURL = try container.decodeLossyURLIfPresent(forKey: .sourceURL)
        canonicalURL = try container.decodeLossyURLIfPresent(forKey: .canonicalURL)
        publishedAt = try container.decodeIfPresent(String.self, forKey: .publishedAt)
        summary = try container.decodeIfPresent(String.self, forKey: .summary)
        contentHTML = try container.decodeIfPresent(String.self, forKey: .contentHTML)
        contentText = try container.decodeIfPresent(String.self, forKey: .contentText)
        isRead = try container.decodeIfPresent(Bool.self, forKey: .isRead) ?? false
        isStarred = try container.decodeIfPresent(Bool.self, forKey: .isStarred) ?? false
        isSavedForLater = try container.decodeIfPresent(Bool.self, forKey: .isSavedForLater) ?? false
        isArchived = try container.decodeIfPresent(Bool.self, forKey: .isArchived) ?? false
    }
}

public struct CoreMeta: Codable, Sendable {
    public let apiVersion: Int
    public let appVersion: String

    enum CodingKeys: String, CodingKey {
        case apiVersion = "api_version"
        case appVersion = "app_version"
    }
}

public struct SyncEvent: Identifiable, Codable, Sendable {
    public let id: String
    public let entityType: String
    public let entityId: String
    public let eventType: String
    public let payloadJson: String
    public let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id
        case entityType = "entity_type"
        case entityId = "entity_id"
        case eventType = "event_type"
        case payloadJson = "payload_json"
        case createdAt = "created_at"
    }
}
