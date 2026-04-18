import Foundation
import InfoMatrixCore

public final class NativeReaderService: ReaderService, @unchecked Sendable {
    private let dbPath: String?

    public init(dbPath: String? = nil) {
        self.dbPath = dbPath
    }

    private struct FFIEnvelope<T: Decodable>: Decodable {
        let ok: Bool
        let data: T?
        let error: String?
    }

    private struct DefaultDBPathPayload: Decodable {
        let dbPath: String

        enum CodingKeys: String, CodingKey {
            case dbPath = "db_path"
        }
    }

    private func callFFI<T: Decodable>(
        _ ffiFunc: (UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?,
        payload: [String: Any?]? = nil
    ) throws -> T {
        var fullPayload = payload ?? [:]
        if let dbPath = self.dbPath {
            fullPayload["db_path"] = dbPath
        }
        
        let cleanPayload = fullPayload.compactMapValues { $0 }
        let jsonData: Data
        do {
            jsonData = try JSONSerialization.data(withJSONObject: cleanPayload)
        } catch {
            throw NSError(
                domain: "InfoMatrix",
                code: -3,
                userInfo: [NSLocalizedDescriptionKey: "Failed to encode FFI payload: \(error.localizedDescription)"]
            )
        }
        guard let jsonString = String(data: jsonData, encoding: .utf8) else {
            throw NSError(
                domain: "InfoMatrix",
                code: -3,
                userInfo: [NSLocalizedDescriptionKey: "Failed to encode FFI payload as UTF-8"]
            )
        }

        return try jsonString.withCString { cString in
            guard let resultPtr = ffiFunc(cString) else {
                throw NSError(domain: "InfoMatrix", code: -1, userInfo: [NSLocalizedDescriptionKey: "FFI returned null"])
            }
            defer { infomatrix_core_free_string(resultPtr) }

            let resultString = String(cString: resultPtr)
            guard let resultData = resultString.data(using: .utf8) else {
                throw NSError(
                    domain: "InfoMatrix",
                    code: -4,
                    userInfo: [NSLocalizedDescriptionKey: "Failed to decode FFI response as UTF-8"]
                )
            }
            let envelope = try JSONDecoder().decode(FFIEnvelope<T>.self, from: resultData)
            
            if envelope.ok, let data = envelope.data {
                return data
            } else {
                throw NSError(domain: "InfoMatrix", code: -2, userInfo: [NSLocalizedDescriptionKey: envelope.error ?? "Unknown FFI error"])
            }
        }
    }

    private func callFFINoInput<T: Decodable>(
        _ ffiFunc: () -> UnsafeMutablePointer<CChar>?
    ) throws -> T {
        guard let resultPtr = ffiFunc() else {
            throw NSError(domain: "InfoMatrix", code: -1, userInfo: [NSLocalizedDescriptionKey: "FFI returned null"])
        }
        defer { infomatrix_core_free_string(resultPtr) }

        let resultString = String(cString: resultPtr)
        guard let resultData = resultString.data(using: .utf8) else {
            throw NSError(
                domain: "InfoMatrix",
                code: -4,
                userInfo: [NSLocalizedDescriptionKey: "Failed to decode FFI response as UTF-8"]
            )
        }
        let envelope = try JSONDecoder().decode(FFIEnvelope<T>.self, from: resultData)
        
        if envelope.ok, let data = envelope.data {
            return data
        } else {
            throw NSError(domain: "InfoMatrix", code: -2, userInfo: [NSLocalizedDescriptionKey: envelope.error ?? "Unknown FFI error"])
        }
    }

    public static func defaultDBPath() -> String? {
        guard let resultPtr = infomatrix_core_default_db_path_json() else {
            return nil
        }
        defer { infomatrix_core_free_string(resultPtr) }
        
        let resultString = String(cString: resultPtr)
        guard let resultData = resultString.data(using: .utf8),
              let envelope = try? JSONDecoder().decode(FFIEnvelope<DefaultDBPathPayload>.self, from: resultData),
              envelope.ok,
              let dbPath = envelope.data?.dbPath else {
            return nil
        }
        return dbPath
    }

    public func listFeeds() async throws -> [Feed] {
        return try callFFI(infomatrix_core_list_feeds_json)
    }

    public func listItems(feedID: String, limit: Int, searchQuery: String?) async throws -> [ArticleItem] {
        return try callFFI(infomatrix_core_list_items_json, payload: [
            "feed_id": feedID,
            "limit": limit,
            "q": searchQuery
        ])
    }

    public func listAllItems(
        limit: Int,
        searchQuery: String?,
        filter: String,
        kind: String? = nil
    ) async throws -> [ArticleItem] {
        var payload: [String: Any?] = [
            "limit": limit,
            "q": searchQuery,
            "filter": filter
        ]
        if let kind {
            payload["kind"] = kind
        }
        return try callFFI(infomatrix_core_list_entries_json, payload: payload)
    }

    public func itemCounts() async throws -> ItemScopeCounts {
        return try callFFI(infomatrix_core_item_counts_json)
    }

    public func listGroups() async throws -> [FeedGroup] {
        return try callFFI(infomatrix_core_list_groups_json)
    }

    public func createGroup(name: String) async throws -> FeedGroup {
        return try callFFI(infomatrix_core_create_group_json, payload: ["name": name])
    }

    public func itemDetail(itemID: String) async throws -> ArticleDetail {
        return try callFFI(
            infomatrix_core_get_entry_json,
            payload: Self.itemDetailPayload(itemID: itemID)
        )
    }

    public func discoverSite(siteURL: String) async throws -> DiscoverSiteResponse {
        return try callFFI(infomatrix_core_discover_site_json, payload: ["site_url": siteURL])
    }

    public func subscribe(inputURL: String) async throws -> SubscriptionResult {
        return try callFFI(infomatrix_core_subscribe_input_json, payload: ["input_url": inputURL])
    }

    public func createEntry(
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
    ) async throws -> ArticleDetail {
        return try callFFI(infomatrix_core_create_entry_json, payload: [
            "title": title,
            "kind": kind,
            "source_kind": sourceKind,
            "source_id": sourceID,
            "source_url": sourceURL,
            "source_title": sourceTitle,
            "canonical_url": canonicalURL,
            "summary": summary,
            "content_html": contentHTML,
            "content_text": contentText
        ])
    }

    public func fetchFullText(itemID: String) async throws -> ArticleDetail {
        _ = try callFFI(
            infomatrix_core_fetch_fulltext_json,
            payload: Self.fetchFullTextPayload(itemID: itemID)
        ) as EmptyResponse
        return try await itemDetail(itemID: itemID)
    }

    public func importOPML(opmlXML: String) async throws -> OPMLImportResult {
        return try callFFI(infomatrix_core_import_opml_json, payload: ["opml_xml": opmlXML])
    }

    public func exportOPML() async throws -> OPMLExportResult {
        return try callFFI(infomatrix_core_export_opml_json)
    }

    public func refreshDueFeeds(limit: Int) async throws -> RefreshDueResult {
        return try callFFI(infomatrix_core_refresh_due_json, payload: ["limit": limit])
    }

    public func getGlobalNotificationSettings() async throws -> GlobalNotificationSettings {
        return try callFFI(infomatrix_core_get_global_notification_settings_json)
    }

    public func updateGlobalNotificationSettings(_ settings: GlobalNotificationSettings) async throws -> GlobalNotificationSettings {
        return try callFFI(
            infomatrix_core_update_global_notification_settings_json,
            payload: try Self.notificationSettingsPayload(settings)
        )
    }

    public func getFeedNotificationSettings(feedID: String) async throws -> NotificationSettings {
        return try callFFI(infomatrix_core_get_feed_notification_settings_json, payload: ["feed_id": feedID])
    }

    public func updateFeedNotificationSettings(feedID: String, settings: NotificationSettings) async throws -> NotificationSettings {
        return try callFFI(
            infomatrix_core_update_feed_notification_settings_json,
            payload: try Self.notificationSettingsPayload(settings, extraFields: ["feed_id": feedID])
        )
    }

    public func getFeedRefreshSettings(feedID: String) async throws -> RefreshSettings {
        return try callFFI(
            infomatrix_core_get_feed_refresh_settings_json,
            payload: ["feed_id": feedID]
        )
    }

    public func updateFeedRefreshSettings(
        feedID: String,
        settings: RefreshSettings
    ) async throws -> RefreshSettings {
        return try callFFI(
            infomatrix_core_update_feed_refresh_settings_json,
            payload: [
                "feed_id": feedID,
                "settings": [
                    "enabled": settings.enabled,
                    "interval_minutes": settings.intervalMinutes,
                ],
            ]
        )
    }

    public func deleteFeedRefreshSettings(feedID: String) async throws -> RefreshSettings {
        return try callFFI(
            infomatrix_core_delete_feed_refresh_settings_json,
            payload: ["feed_id": feedID]
        )
    }

    public func getGroupRefreshSettings(groupID: String) async throws -> RefreshSettings {
        return try callFFI(
            infomatrix_core_get_group_refresh_settings_json,
            payload: ["group_id": groupID]
        )
    }

    public func updateGroupRefreshSettings(
        groupID: String,
        settings: RefreshSettings
    ) async throws -> RefreshSettings {
        return try callFFI(
            infomatrix_core_update_group_refresh_settings_json,
            payload: [
                "group_id": groupID,
                "settings": [
                    "enabled": settings.enabled,
                    "interval_minutes": settings.intervalMinutes,
                ],
            ]
        )
    }

    public func deleteGroupRefreshSettings(groupID: String) async throws -> RefreshSettings {
        return try callFFI(
            infomatrix_core_delete_group_refresh_settings_json,
            payload: ["group_id": groupID]
        )
    }

    public func listPendingNotificationEvents(limit: Int) async throws -> [NotificationEvent] {
        return try callFFI(infomatrix_core_list_pending_notification_events_json, payload: ["limit": limit])
    }

    public func acknowledgeNotificationEvents(eventIDs: [String]) async throws -> Int {
        struct AckResponse: Decodable { let acknowledged: Int }
        let res: AckResponse = try callFFI(infomatrix_core_ack_notification_events_json, payload: ["event_ids": eventIDs])
        return res.acknowledged
    }

    public func addSubscription(feedURL: String, title: String?) async throws -> String {
        struct AddResponse: Decodable { let feedId: String }
        let res: AddResponse = try callFFI(infomatrix_core_add_subscription_json, payload: ["feed_url": feedURL, "title": title])
        return res.feedId
    }

    public func updateFeed(feedID: String, title: String?, autoFullText: Bool?) async throws {
        var payload: [String: Any?] = ["feed_id": feedID, "title": title]
        if let autoFullText {
            payload["auto_full_text"] = autoFullText
        }
        _ = try callFFI(infomatrix_core_update_feed_json, payload: payload) as EmptyResponse
    }

    public func updateFeedGroup(feedID: String, groupID: String?) async throws {
        _ = try callFFI(infomatrix_core_update_feed_group_json, payload: ["feed_id": feedID, "group_id": groupID]) as EmptyResponse
    }

    public func deleteFeed(feedID: String) async throws {
        _ = try callFFI(infomatrix_core_delete_feed_json, payload: ["feed_id": feedID]) as EmptyResponse
    }

    public func refresh(feedID: String) async throws {
        _ = try callFFI(infomatrix_core_refresh_feed_json, payload: ["feed_id": feedID]) as EmptyResponse
    }

    public func patchItemState(
        itemID: String,
        isRead: Bool?,
        isStarred: Bool?,
        isSavedForLater: Bool?,
        isArchived: Bool?
    ) async throws {
        _ = try callFFI(
            infomatrix_core_patch_item_state_json,
            payload: Self.patchItemStatePayload(
                itemID: itemID,
                isRead: isRead,
                isStarred: isStarred,
                isSavedForLater: isSavedForLater,
                isArchived: isArchived
            )
        ) as EmptyResponse
    }

    public func getCoreMeta() async throws -> CoreMeta {
        return try callFFINoInput(infomatrix_core_meta_json)
    }

    public func listPendingSyncEvents(limit: Int) async throws -> [SyncEvent] {
        return try callFFI(infomatrix_core_list_sync_events_json, payload: ["limit": limit])
    }

    public func acknowledgeSyncEvents(eventIDs: [String]) async throws -> Int {
        struct AckResponse: Decodable { let acknowledged: Int }
        let res: AckResponse = try callFFI(infomatrix_core_ack_sync_events_json, payload: ["event_ids": eventIDs])
        return res.acknowledged
    }

    public func applySyncEvents(_ events: [SyncEvent]) async throws -> Int {
        struct ApplyResponse: Decodable { let applied: Int }
        let res: ApplyResponse = try callFFI(
            infomatrix_core_apply_sync_events_json,
            payload: ["events": events]
        )
        return res.applied
    }

    private struct EmptyResponse: Decodable {}
}

extension NativeReaderService {
    static func itemDetailPayload(itemID: String) -> [String: Any?] {
        [
            "item_id": itemID
        ]
    }

    static func fetchFullTextPayload(itemID: String) -> [String: Any?] {
        [
            "item_id": itemID
        ]
    }

    static func patchItemStatePayload(
        itemID: String,
        isRead: Bool?,
        isStarred: Bool?,
        isSavedForLater: Bool?,
        isArchived: Bool?
    ) -> [String: Any?] {
        var payload: [String: Any?] = [
            "item_id": itemID
        ]
        if let isRead {
            payload["is_read"] = isRead
        }
        if let isStarred {
            payload["is_starred"] = isStarred
        }
        if let isSavedForLater {
            payload["is_saved_for_later"] = isSavedForLater
        }
        if let isArchived {
            payload["is_archived"] = isArchived
        }
        return payload
    }

    static func notificationSettingsPayload<T: Encodable>(
        _ settings: T,
        extraFields: [String: Any?] = [:]
    ) throws -> [String: Any?] {
        let jsonData = try JSONEncoder().encode(settings)
        let settingsObject = try JSONSerialization.jsonObject(with: jsonData)
        var payload = extraFields
        payload["settings"] = settingsObject
        return payload
    }
}
