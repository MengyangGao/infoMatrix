import XCTest
@testable import InfoMatrixShell

final class NativeReaderServiceTests: XCTestCase {
    func testItemDetailAndFullTextPayloadUseItemIDKey() {
        let detailPayload = NativeReaderService.itemDetailPayload(itemID: "item-123")
        XCTAssertEqual(detailPayload["item_id"] as? String, "item-123")
        XCTAssertNil(detailPayload["id"] as Any?)

        let fullTextPayload = NativeReaderService.fetchFullTextPayload(itemID: "item-123")
        XCTAssertEqual(fullTextPayload["item_id"] as? String, "item-123")
        XCTAssertNil(fullTextPayload["id"] as Any?)
    }

    func testPatchItemStatePayloadUsesItemIDKey() {
        let payload = NativeReaderService.patchItemStatePayload(
            itemID: "item-123",
            isRead: true,
            isStarred: false,
            isSavedForLater: nil,
            isArchived: true
        )

        XCTAssertEqual(payload["item_id"] as? String, "item-123")
        XCTAssertEqual(payload["is_read"] as? Bool, true)
        XCTAssertEqual(payload["is_starred"] as? Bool, false)
        XCTAssertNil(payload["is_saved_for_later"] as Any?)
        XCTAssertEqual(payload["is_archived"] as? Bool, true)
        XCTAssertNil(payload["id"] as Any?)
    }

    func testNotificationSettingsPayloadWrapsSettings() throws {
        let digestPolicy = DigestPolicy(enabled: true, intervalMinutes: 120, maxItems: 7)
        let quietHours = QuietHours(enabled: true, startMinute: 22 * 60, endMinute: 7 * 60)
        let feedSettings = NotificationSettings(
            enabled: true,
            mode: .digest,
            digestPolicy: digestPolicy,
            quietHours: quietHours,
            minimumIntervalMinutes: 30,
            highPriority: true,
            keywordInclude: ["swift"],
            keywordExclude: ["ads"]
        )

        let feedPayload = try NativeReaderService.notificationSettingsPayload(
            feedSettings,
            extraFields: ["feed_id": "feed-1"]
        )
        XCTAssertEqual(feedPayload["feed_id"] as? String, "feed-1")
        XCTAssertNil(feedPayload["enabled"] as Any?)

        let feedWrapper = try XCTUnwrap(feedPayload["settings"] as? [String: Any])
        XCTAssertEqual(feedWrapper["enabled"] as? Bool, true)
        XCTAssertEqual(feedWrapper["mode"] as? String, "digest")
        XCTAssertEqual(feedWrapper["minimum_interval_minutes"] as? Int, 30)
        XCTAssertEqual(feedWrapper["high_priority"] as? Bool, true)
        XCTAssertEqual(feedWrapper["keyword_include"] as? [String], ["swift"])
        XCTAssertEqual(feedWrapper["keyword_exclude"] as? [String], ["ads"])

        let globalSettings = GlobalNotificationSettings(
            backgroundRefreshEnabled: true,
            backgroundRefreshIntervalMinutes: 45,
            digestPolicy: DigestPolicy(enabled: false, intervalMinutes: 60, maxItems: 12),
            defaultFeedSettings: feedSettings
        )
        let globalPayload = try NativeReaderService.notificationSettingsPayload(globalSettings)
        XCTAssertNil(globalPayload["background_refresh_enabled"] as Any?)

        let globalWrapper = try XCTUnwrap(globalPayload["settings"] as? [String: Any])
        XCTAssertEqual(globalWrapper["background_refresh_enabled"] as? Bool, true)
        XCTAssertEqual(globalWrapper["background_refresh_interval_minutes"] as? Int, 45)
        XCTAssertNotNil(globalWrapper["default_feed_settings"] as? [String: Any])
    }
}
