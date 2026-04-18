import Foundation

public protocol ReaderService: Sendable {
    func listFeeds() async throws -> [Feed]
    func listItems(feedID: String, limit: Int, searchQuery: String?) async throws -> [ArticleItem]
    func listAllItems(
        limit: Int,
        searchQuery: String?,
        filter: String,
        kind: String?
    ) async throws -> [ArticleItem]
    func itemCounts() async throws -> ItemScopeCounts
    func listGroups() async throws -> [FeedGroup]
    func createGroup(name: String) async throws -> FeedGroup
    func itemDetail(itemID: String) async throws -> ArticleDetail
    func discoverSite(siteURL: String) async throws -> DiscoverSiteResponse
    func subscribe(inputURL: String) async throws -> SubscriptionResult
    func createEntry(
        title: String,
        kind: String?,
        sourceKind: String?,
        sourceID: String?,
        sourceURL: String?,
        sourceTitle: String?,
        canonicalURL: String?,
        summary: String?,
        contentHTML: String?,
        contentText: String?
    ) async throws -> ArticleDetail
    func fetchFullText(itemID: String) async throws -> ArticleDetail
    func importOPML(opmlXML: String) async throws -> OPMLImportResult
    func exportOPML() async throws -> OPMLExportResult
    func refreshDueFeeds(limit: Int) async throws -> RefreshDueResult
    func getGlobalNotificationSettings() async throws -> GlobalNotificationSettings
    func updateGlobalNotificationSettings(_ settings: GlobalNotificationSettings) async throws -> GlobalNotificationSettings
    func getFeedNotificationSettings(feedID: String) async throws -> NotificationSettings
    func updateFeedNotificationSettings(feedID: String, settings: NotificationSettings) async throws -> NotificationSettings
    func getFeedRefreshSettings(feedID: String) async throws -> RefreshSettings
    func updateFeedRefreshSettings(feedID: String, settings: RefreshSettings) async throws -> RefreshSettings
    func deleteFeedRefreshSettings(feedID: String) async throws -> RefreshSettings
    func getGroupRefreshSettings(groupID: String) async throws -> RefreshSettings
    func updateGroupRefreshSettings(groupID: String, settings: RefreshSettings) async throws -> RefreshSettings
    func deleteGroupRefreshSettings(groupID: String) async throws -> RefreshSettings
    func listPendingNotificationEvents(limit: Int) async throws -> [NotificationEvent]
    func acknowledgeNotificationEvents(eventIDs: [String]) async throws -> Int
    func addSubscription(feedURL: String, title: String?) async throws -> String
    func updateFeed(feedID: String, title: String?, autoFullText: Bool?) async throws
    func updateFeedGroup(feedID: String, groupID: String?) async throws
    func deleteFeed(feedID: String) async throws
    func refresh(feedID: String) async throws
    func patchItemState(
        itemID: String,
        isRead: Bool?,
        isStarred: Bool?,
        isSavedForLater: Bool?,
        isArchived: Bool?
    ) async throws

    func getCoreMeta() async throws -> CoreMeta
    func listPendingSyncEvents(limit: Int) async throws -> [SyncEvent]
    func acknowledgeSyncEvents(eventIDs: [String]) async throws -> Int
    func applySyncEvents(_ events: [SyncEvent]) async throws -> Int
}

public struct HTTPReaderService: ReaderService {
    private let baseURL: URL
    private let session: URLSession

    private struct ApiErrorPayload: Decodable {
        let message: String
    }

    public init(baseURL: URL, session: URLSession = .shared) {
        self.baseURL = baseURL
        self.session = session
    }

    public func listFeeds() async throws -> [Feed] {
        let request = URLRequest(url: baseURL.appending(path: "/api/v1/feeds"))
        return try await decode(request)
    }

    public func listItems(
        feedID: String,
        limit: Int = 100,
        searchQuery: String? = nil
    ) async throws -> [ArticleItem] {
        var components = URLComponents(
            url: baseURL.appending(path: "/api/v1/feeds/\(feedID)/items"),
            resolvingAgainstBaseURL: false
        )
        var queryItems = [URLQueryItem(name: "limit", value: String(limit))]
        if let searchQuery, !searchQuery.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            queryItems.append(URLQueryItem(name: "q", value: searchQuery))
        }
        components?.queryItems = queryItems

        guard let url = components?.url else {
            throw URLError(.badURL)
        }
        let request = URLRequest(url: url)
        return try await decode(request)
    }

    public func listAllItems(
        limit: Int = 200,
        searchQuery: String? = nil,
        filter: String = "all",
        kind: String? = nil
    ) async throws -> [ArticleItem] {
        var components = URLComponents(
            url: baseURL.appending(path: "/api/v1/entries"),
            resolvingAgainstBaseURL: false
        )
        var queryItems = [
            URLQueryItem(name: "limit", value: String(limit)),
            URLQueryItem(name: "filter", value: filter)
        ]
        if let searchQuery, !searchQuery.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            queryItems.append(URLQueryItem(name: "q", value: searchQuery))
        }
        if let kind, !kind.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            queryItems.append(URLQueryItem(name: "kind", value: kind))
        }
        components?.queryItems = queryItems
        guard let url = components?.url else {
            throw URLError(.badURL)
        }
        return try await decode(URLRequest(url: url))
    }

    public func itemCounts() async throws -> ItemScopeCounts {
        let request = URLRequest(url: baseURL.appending(path: "/api/v1/entries/counts"))
        return try await decode(request)
    }

    public func listGroups() async throws -> [FeedGroup] {
        let request = URLRequest(url: baseURL.appending(path: "/api/v1/groups"))
        return try await decode(request)
    }

    public func createGroup(name: String) async throws -> FeedGroup {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/groups"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        struct Payload: Codable { let name: String }
        request.httpBody = try JSONEncoder().encode(Payload(name: name))
        return try await decode(request)
    }

    public func itemDetail(itemID: String) async throws -> ArticleDetail {
        let request = URLRequest(url: baseURL.appending(path: "/api/v1/entries/\(itemID)"))
        return try await decode(request)
    }

    public func discoverSite(siteURL: String) async throws -> DiscoverSiteResponse {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/discover"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        struct Payload: Codable {
            let siteURL: String

            enum CodingKeys: String, CodingKey {
                case siteURL = "site_url"
            }
        }

        request.httpBody = try JSONEncoder().encode(Payload(siteURL: siteURL))
        return try await decode(request)
    }

    public func subscribe(inputURL: String) async throws -> SubscriptionResult {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/subscribe"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        struct Payload: Codable {
            let inputURL: String

            enum CodingKeys: String, CodingKey {
                case inputURL = "input_url"
            }
        }

        request.httpBody = try JSONEncoder().encode(Payload(inputURL: inputURL))
        return try await decode(request)
    }

    public func createEntry(
        title: String,
        kind: String? = nil,
        sourceKind: String? = nil,
        sourceID: String? = nil,
        sourceURL: String? = nil,
        sourceTitle: String? = nil,
        canonicalURL: String? = nil,
        summary: String? = nil,
        contentHTML: String? = nil,
        contentText: String? = nil
    ) async throws -> ArticleDetail {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/entries"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        struct Payload: Codable {
            let title: String
            let kind: String?
            let sourceKind: String?
            let sourceID: String?
            let sourceURL: String?
            let sourceTitle: String?
            let canonicalURL: String?
            let summary: String?
            let contentHTML: String?
            let contentText: String?

            enum CodingKeys: String, CodingKey {
                case title
                case kind
                case sourceKind = "source_kind"
                case sourceID = "source_id"
                case sourceURL = "source_url"
                case sourceTitle = "source_title"
                case canonicalURL = "canonical_url"
                case summary
                case contentHTML = "content_html"
                case contentText = "content_text"
            }
        }

        request.httpBody = try JSONEncoder().encode(
            Payload(
                title: title,
                kind: kind,
                sourceKind: sourceKind,
                sourceID: sourceID,
                sourceURL: sourceURL,
                sourceTitle: sourceTitle,
                canonicalURL: canonicalURL,
                summary: summary,
                contentHTML: contentHTML,
                contentText: contentText
            )
        )
        return try await decode(request)
    }

    public func fetchFullText(itemID: String) async throws -> ArticleDetail {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/entries/\(itemID)/fulltext"))
        request.httpMethod = "POST"
        _ = try await requestRaw(request)
        return try await itemDetail(itemID: itemID)
    }

    public func importOPML(opmlXML: String) async throws -> OPMLImportResult {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/opml/import"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        struct Payload: Codable {
            let opmlXML: String

            enum CodingKeys: String, CodingKey {
                case opmlXML = "opml_xml"
            }
        }

        request.httpBody = try JSONEncoder().encode(Payload(opmlXML: opmlXML))
        return try await decode(request)
    }

    public func exportOPML() async throws -> OPMLExportResult {
        let request = URLRequest(url: baseURL.appending(path: "/api/v1/opml/export"))
        return try await decode(request)
    }

    public func refreshDueFeeds(limit: Int) async throws -> RefreshDueResult {
        var components = URLComponents(
            url: baseURL.appending(path: "/api/v1/refresh/due"),
            resolvingAgainstBaseURL: false
        )
        components?.queryItems = [URLQueryItem(name: "limit", value: String(limit))]
        guard let url = components?.url else {
            throw URLError(.badURL)
        }
        let request = URLRequest(url: url)
        return try await decode(request)
    }

    public func getGlobalNotificationSettings() async throws -> GlobalNotificationSettings {
        let request = URLRequest(url: baseURL.appending(path: "/api/v1/notifications/settings"))
        return try await decode(request)
    }

    public func updateGlobalNotificationSettings(
        _ settings: GlobalNotificationSettings
    ) async throws -> GlobalNotificationSettings {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/notifications/settings"))
        request.httpMethod = "PUT"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(settings)
        return try await decode(request)
    }

    public func getFeedNotificationSettings(feedID: String) async throws -> NotificationSettings {
        let request = URLRequest(url: baseURL.appending(path: "/api/v1/feeds/\(feedID)/notifications"))
        return try await decode(request)
    }

    public func updateFeedNotificationSettings(
        feedID: String,
        settings: NotificationSettings
    ) async throws -> NotificationSettings {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/feeds/\(feedID)/notifications"))
        request.httpMethod = "PUT"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(settings)
        return try await decode(request)
    }

    public func getFeedRefreshSettings(feedID: String) async throws -> RefreshSettings {
        let request = URLRequest(url: baseURL.appending(path: "/api/v1/feeds/\(feedID)/refresh-settings"))
        return try await decode(request)
    }

    public func updateFeedRefreshSettings(
        feedID: String,
        settings: RefreshSettings
    ) async throws -> RefreshSettings {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/feeds/\(feedID)/refresh-settings"))
        request.httpMethod = "PUT"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(settings)
        return try await decode(request)
    }

    public func deleteFeedRefreshSettings(feedID: String) async throws -> RefreshSettings {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/feeds/\(feedID)/refresh-settings"))
        request.httpMethod = "DELETE"
        return try await decode(request)
    }

    public func getGroupRefreshSettings(groupID: String) async throws -> RefreshSettings {
        let request = URLRequest(url: baseURL.appending(path: "/api/v1/groups/\(groupID)/refresh-settings"))
        return try await decode(request)
    }

    public func updateGroupRefreshSettings(
        groupID: String,
        settings: RefreshSettings
    ) async throws -> RefreshSettings {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/groups/\(groupID)/refresh-settings"))
        request.httpMethod = "PUT"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(settings)
        return try await decode(request)
    }

    public func deleteGroupRefreshSettings(groupID: String) async throws -> RefreshSettings {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/groups/\(groupID)/refresh-settings"))
        request.httpMethod = "DELETE"
        return try await decode(request)
    }

    public func listPendingNotificationEvents(limit: Int = 50) async throws -> [NotificationEvent] {
        var components = URLComponents(
            url: baseURL.appending(path: "/api/v1/notifications/pending"),
            resolvingAgainstBaseURL: false
        )
        components?.queryItems = [URLQueryItem(name: "limit", value: String(limit))]
        guard let url = components?.url else {
            throw URLError(.badURL)
        }
        return try await decode(URLRequest(url: url))
    }

    public func acknowledgeNotificationEvents(eventIDs: [String]) async throws -> Int {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/notifications/pending/ack"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        struct Payload: Codable {
            let eventIDs: [String]

            enum CodingKeys: String, CodingKey {
                case eventIDs = "event_ids"
            }
        }

        struct Response: Codable {
            let acknowledged: Int
        }

        request.httpBody = try JSONEncoder().encode(Payload(eventIDs: eventIDs))
        let response: Response = try await decode(request)
        return response.acknowledged
    }

    public func addSubscription(feedURL: String, title: String? = nil) async throws -> String {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/subscriptions"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        struct Payload: Codable {
            let feedURL: String
            let title: String?

            enum CodingKeys: String, CodingKey {
                case feedURL = "feed_url"
                case title
            }
        }

        struct Response: Codable {
            let feedID: String

            enum CodingKeys: String, CodingKey {
                case feedID = "feed_id"
            }
        }

        request.httpBody = try JSONEncoder().encode(Payload(feedURL: feedURL, title: title))
        let response: Response = try await decode(request)
        return response.feedID
    }

    public func updateFeed(feedID: String, title: String?, autoFullText: Bool?) async throws {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/feeds/\(feedID)"))
        request.httpMethod = "PATCH"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        struct Payload: Encodable {
            let title: String?
            let autoFullText: Bool?

            enum CodingKeys: String, CodingKey {
                case title
                case autoFullText = "auto_full_text"
            }

            func encode(to encoder: any Encoder) throws {
                var container = encoder.container(keyedBy: CodingKeys.self)
                try container.encodeIfPresent(title, forKey: .title)
                try container.encodeIfPresent(autoFullText, forKey: .autoFullText)
            }
        }
        request.httpBody = try JSONEncoder().encode(Payload(title: title, autoFullText: autoFullText))
        _ = try await requestRaw(request)
    }

    public func updateFeedGroup(feedID: String, groupID: String?) async throws {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/feeds/\(feedID)/group"))
        request.httpMethod = "PATCH"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        struct Payload: Codable {
            let groupID: String?
            enum CodingKeys: String, CodingKey { case groupID = "group_id" }
        }
        request.httpBody = try JSONEncoder().encode(Payload(groupID: groupID))
        _ = try await requestRaw(request)
    }

    public func refresh(feedID: String) async throws {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/refresh/\(feedID)"))
        request.httpMethod = "POST"
        _ = try await requestRaw(request)
    }

    public func deleteFeed(feedID: String) async throws {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/feeds/\(feedID)"))
        request.httpMethod = "DELETE"
        _ = try await requestRaw(request)
    }

    public func patchItemState(
        itemID: String,
        isRead: Bool? = nil,
        isStarred: Bool? = nil,
        isSavedForLater: Bool? = nil,
        isArchived: Bool? = nil
    ) async throws {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/entries/\(itemID)/state"))
        request.httpMethod = "PATCH"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        struct Payload: Codable {
            let isRead: Bool?
            let isStarred: Bool?
            let isSavedForLater: Bool?
            let isArchived: Bool?

            enum CodingKeys: String, CodingKey {
                case isRead = "is_read"
                case isStarred = "is_starred"
                case isSavedForLater = "is_saved_for_later"
                case isArchived = "is_archived"
            }
        }

        request.httpBody = try JSONEncoder().encode(
            Payload(
                isRead: isRead,
                isStarred: isStarred,
                isSavedForLater: isSavedForLater,
                isArchived: isArchived
            )
        )

        _ = try await requestRaw(request)
    }

    public func getCoreMeta() async throws -> CoreMeta {
        let request = URLRequest(url: baseURL.appending(path: "/api/v1/meta"))
        return try await decode(request)
    }

    public func listPendingSyncEvents(limit: Int = 100) async throws -> [SyncEvent] {
        var components = URLComponents(
            url: baseURL.appending(path: "/api/v1/sync/pending"),
            resolvingAgainstBaseURL: false
        )
        components?.queryItems = [URLQueryItem(name: "limit", value: String(limit))]
        guard let url = components?.url else {
            throw URLError(.badURL)
        }
        return try await decode(URLRequest(url: url))
    }

    public func acknowledgeSyncEvents(eventIDs: [String]) async throws -> Int {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/sync/pending/ack"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        struct Payload: Codable {
            let eventIDs: [String]
            enum CodingKeys: String, CodingKey { case eventIDs = "event_ids" }
        }

        struct Response: Codable {
            let acknowledged: Int
        }

        request.httpBody = try JSONEncoder().encode(Payload(eventIDs: eventIDs))
        let response: Response = try await decode(request)
        return response.acknowledged
    }

    public func applySyncEvents(_ events: [SyncEvent]) async throws -> Int {
        var request = URLRequest(url: baseURL.appending(path: "/api/v1/sync/events/apply"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        struct Payload: Codable {
            let events: [SyncEvent]
        }

        struct Response: Codable {
            let applied: Int
        }

        request.httpBody = try JSONEncoder().encode(Payload(events: events))
        let response: Response = try await decode(request)
        return response.applied
    }

    private func decode<T: Decodable>(_ request: URLRequest) async throws -> T {
        let data = try await requestRaw(request)
        return try JSONDecoder().decode(T.self, from: data)
    }

    private func requestRaw(_ request: URLRequest) async throws -> Data {
        let (data, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw URLError(.badServerResponse)
        }
        guard (200...299).contains(httpResponse.statusCode) else {
            let backendMessage = try? JSONDecoder().decode(ApiErrorPayload.self, from: data)
            let message = backendMessage?.message ?? "HTTP \(httpResponse.statusCode)"
            throw NSError(
                domain: "InfoMatrix",
                code: httpResponse.statusCode,
                userInfo: [NSLocalizedDescriptionKey: message]
            )
        }
        return data
    }
}
