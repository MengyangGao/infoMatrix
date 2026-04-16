import XCTest
import InfoMatrixCore
@testable import InfoMatrixShell

private actor MockReaderService: ReaderService {
    var feeds: [Feed] = []
    var groups: [FeedGroup] = []
    var itemsByFeed: [String: [ArticleItem]] = [:]
    var itemDetails: [String: ArticleDetail] = [:]
    var discoverResultOverride: DiscoverSiteResponse?
    var discoverError: NSError?
    var listGroupsError: NSError?
    var refreshDueCalls: Int = 0
    var fullTextCalls: Int = 0
    var fullTextError: NSError?
    var subscribeCalls: Int = 0
    var discoverCalls: Int = 0
    var listAllItemsRequests: [(limit: Int, searchQuery: String?, filter: String, kind: String?)] = []
    var globalNotificationSettings = GlobalNotificationSettings(
        backgroundRefreshEnabled: true,
        backgroundRefreshIntervalMinutes: 15,
        digestPolicy: DigestPolicy(enabled: false, intervalMinutes: 60, maxItems: 20),
        defaultFeedSettings: NotificationSettings(
            enabled: false,
            mode: .immediate,
            digestPolicy: DigestPolicy(enabled: false, intervalMinutes: 60, maxItems: 20),
            quietHours: QuietHours(enabled: false, startMinute: 22 * 60, endMinute: 7 * 60),
            minimumIntervalMinutes: 60,
            highPriority: false,
            keywordInclude: [],
            keywordExclude: []
        )
    )
    var notificationSettingsByFeed: [String: NotificationSettings] = [:]
    var pendingNotificationEvents: [NotificationEvent] = []
    var acknowledgedNotificationEventIDs: [String] = []

    func listFeeds() async throws -> [Feed] {
        feeds
    }

    func listItems(feedID: String, limit: Int, searchQuery: String?) async throws -> [ArticleItem] {
        let base = itemsByFeed[feedID] ?? []
        guard let searchQuery, !searchQuery.isEmpty else {
            return Array(base.prefix(limit))
        }
        let filtered = base.filter { item in
            item.title.localizedCaseInsensitiveContains(searchQuery)
        }
        return Array(filtered.prefix(limit))
    }

    func listAllItems(
        limit: Int,
        searchQuery: String?,
        filter: String,
        kind: String?
    ) async throws -> [ArticleItem] {
        listAllItemsRequests.append((limit: limit, searchQuery: searchQuery, filter: filter, kind: kind))
        let all = itemsByFeed.values.flatMap { $0 }
        let scoped: [ArticleItem]
        switch filter {
        case "archive":
            scoped = all.filter(\.isArchived)
        case "unread":
            scoped = all.filter { !$0.isRead && !$0.isArchived }
        case "starred":
            scoped = all.filter { $0.isStarred && !$0.isArchived }
        case "later":
            scoped = all.filter { $0.isSavedForLater && !$0.isArchived }
        default:
            scoped = all.filter { !$0.isArchived }
        }
        let kindScoped = kind.map { requestedKind in
            scoped.filter { $0.kind.lowercased() == requestedKind.lowercased() }
        } ?? scoped
        guard let searchQuery, !searchQuery.isEmpty else {
            return Array(kindScoped.prefix(limit))
        }
        return Array(kindScoped.filter { $0.title.localizedCaseInsensitiveContains(searchQuery) }.prefix(limit))
    }

    func itemCounts() async throws -> ItemScopeCounts {
        let all = itemsByFeed.values.flatMap { $0 }
        return ItemScopeCounts(
            all: all.filter { !$0.isArchived }.count,
            unread: all.filter { !$0.isRead && !$0.isArchived }.count,
            starred: all.filter { $0.isStarred && !$0.isArchived }.count,
            later: all.filter { $0.isSavedForLater && !$0.isArchived }.count,
            notes: all.filter { $0.kind == "note" && !$0.isArchived }.count,
            archive: all.filter(\.isArchived).count
        )
    }

    func listGroups() async throws -> [FeedGroup] {
        if let listGroupsError {
            throw listGroupsError
        }
        return groups
    }

    func createGroup(name: String) async throws -> FeedGroup {
        let group = FeedGroup(id: UUID().uuidString, name: name)
        groups.append(group)
        return group
    }

    func itemDetail(itemID: String) async throws -> ArticleDetail {
        itemDetails[itemID]!
    }

    func discoverSite(siteURL: String) async throws -> DiscoverSiteResponse {
        discoverCalls += 1
        if let discoverError {
            throw discoverError
        }
        if let discoverResultOverride {
            return discoverResultOverride
        }
        let feed = DiscoverFeed(
            url: URL(string: "https://example.com/feed.xml")!,
            title: "Example",
            feedType: "rss",
            confidence: 0.9,
            source: "autodiscovery",
            score: 55
        )
        return DiscoverSiteResponse(
            normalizedSiteURL: URL(string: siteURL)!,
            discoveredFeeds: [feed],
            siteTitle: "Example",
            warnings: []
        )
    }

    func setDiscoverResultOverride(_ response: DiscoverSiteResponse?) {
        discoverResultOverride = response
    }

    func setDiscoverError(_ error: NSError?) {
        discoverError = error
    }

    func setListGroupsError(_ error: NSError?) {
        listGroupsError = error
    }

    func setItemDetail(_ itemID: String, detail: ArticleDetail) {
        itemDetails[itemID] = detail
    }

    func setFeedItems(_ feedID: String, items: [ArticleItem]) {
        itemsByFeed[feedID] = items
    }

    func setFullTextError(_ error: NSError?) {
        fullTextError = error
    }

    func subscribe(inputURL: String) async throws -> SubscriptionResult {
        subscribeCalls += 1
        let feedID = try await addSubscription(feedURL: inputURL, title: nil)
        return SubscriptionResult(
            feedID: feedID,
            resolvedFeedURL: URL(string: inputURL)!,
            subscriptionSource: "direct_feed"
        )
    }

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
    ) async throws -> ArticleDetail {
        let itemID = UUID().uuidString
        let resolvedSourceURL = sourceURL.flatMap(URL.init(string:))
        let requestedTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
        let resolvedTitle: String
        if !requestedTitle.isEmpty {
            resolvedTitle = requestedTitle
        } else if let host = resolvedSourceURL?.host, !host.isEmpty {
            resolvedTitle = host
        } else {
            resolvedTitle = kind ?? (sourceURL == nil ? "note" : "bookmark")
        }
        let resolvedContentHTML = contentHTML ?? resolvedSourceURL.map { url in
            """
            <article>
              <header>
                <h1>\(resolvedTitle)</h1>
                <p>Captured from \(url.absoluteString)</p>
              </header>
              <p>Captured content from \(url.absoluteString)</p>
            </article>
            """
        }
        let resolvedContentText = contentText ?? resolvedSourceURL.map { url in
            "Captured content from \(url.absoluteString)"
        }
        let detail = ArticleDetail(
            id: itemID,
            title: resolvedTitle,
            kind: kind ?? (sourceURL == nil ? "note" : "bookmark"),
            sourceKind: sourceKind ?? (sourceURL == nil ? "manual" : "web"),
            sourceID: sourceID,
            sourceURL: resolvedSourceURL,
            canonicalURL: canonicalURL.flatMap(URL.init(string:)),
            publishedAt: nil,
            summary: summary,
            contentHTML: resolvedContentHTML,
            contentText: resolvedContentText,
            isRead: false,
            isStarred: false,
            isSavedForLater: false,
            isArchived: false
        )
        itemDetails[itemID] = detail
        return detail
    }

    func fetchFullText(itemID: String) async throws -> ArticleDetail {
        fullTextCalls += 1
        if let fullTextError {
            throw fullTextError
        }
        return itemDetails[itemID]!
    }

    func importOPML(opmlXML: String) async throws -> OPMLImportResult {
        _ = try await addSubscription(feedURL: "https://example.com/opml.xml", title: "OPML Imported")
        return OPMLImportResult(parsedFeedCount: 1, uniqueFeedCount: 1, groupedFeedCount: 0)
    }

    func exportOPML() async throws -> OPMLExportResult {
        OPMLExportResult(
            opmlXML: "<?xml version=\"1.0\" encoding=\"UTF-8\"?><opml version=\"2.0\"></opml>",
            feedCount: feeds.count
        )
    }

    func refreshDueFeeds(limit: Int) async throws -> RefreshDueResult {
        refreshDueCalls += 1
        return RefreshDueResult(refreshedCount: 1, totalItemCount: itemsByFeed.values.flatMap { $0 }.count)
    }

    func getGlobalNotificationSettings() async throws -> GlobalNotificationSettings {
        globalNotificationSettings
    }

    func updateGlobalNotificationSettings(
        _ settings: GlobalNotificationSettings
    ) async throws -> GlobalNotificationSettings {
        globalNotificationSettings = settings
        return settings
    }

    func getFeedNotificationSettings(feedID: String) async throws -> NotificationSettings {
        notificationSettingsByFeed[feedID] ?? globalNotificationSettings.defaultFeedSettings
    }

    func updateFeedNotificationSettings(
        feedID: String,
        settings: NotificationSettings
    ) async throws -> NotificationSettings {
        notificationSettingsByFeed[feedID] = settings
        return settings
    }

    func listPendingNotificationEvents(limit: Int) async throws -> [NotificationEvent] {
        Array(pendingNotificationEvents.prefix(limit))
    }

    func acknowledgeNotificationEvents(eventIDs: [String]) async throws -> Int {
        acknowledgedNotificationEventIDs.append(contentsOf: eventIDs)
        pendingNotificationEvents.removeAll { eventIDs.contains($0.id) }
        return eventIDs.count
    }

    func refreshDueCallCount() async -> Int {
        refreshDueCalls
    }

    func fullTextCallCount() async -> Int {
        fullTextCalls
    }

    func setFeedAutoFullText(feedID: String, enabled: Bool) async {
        guard let index = feeds.firstIndex(where: { $0.id == feedID }) else { return }
        feeds[index].autoFullText = enabled
    }

    func addSubscription(feedURL: String, title: String?) async throws -> String {
        let feedID = UUID().uuidString
        let feed = Feed(
            id: feedID,
            title: title ?? "Example",
            feedURL: URL(string: feedURL)!,
            siteURL: nil,
            feedType: "unknown",
            iconURL: nil,
            groups: []
        )
        feeds.append(feed)
        itemsByFeed[feed.id] = [
            ArticleItem(
                id: "item-1",
                title: "Welcome",
                canonicalURL: URL(string: "https://example.com/post"),
                publishedAt: nil,
                isRead: false,
                isStarred: false,
                isSavedForLater: false,
                isArchived: false
            )
        ]
        itemDetails["item-1"] = ArticleDetail(
            id: "item-1",
            title: "Welcome",
            canonicalURL: URL(string: "https://example.com/post"),
            publishedAt: nil,
            summary: "Welcome summary",
            contentHTML: nil,
            contentText: "Welcome body",
            isRead: false,
            isStarred: false,
            isSavedForLater: false,
            isArchived: false
        )
        return feed.id
    }

    func subscribeCallCount() async -> Int {
        subscribeCalls
    }

    func discoverCallCount() async -> Int {
        discoverCalls
    }

    func refresh(feedID: String) async throws {}

    func deleteFeed(feedID: String) async throws {
        feeds.removeAll { $0.id == feedID }
        itemsByFeed[feedID] = nil
    }

    func updateFeed(feedID: String, title: String?, autoFullText: Bool?) async throws {
        guard let index = feeds.firstIndex(where: { $0.id == feedID }) else { return }
        if let title {
            feeds[index].title = title
        }
        if let autoFullText {
            feeds[index].autoFullText = autoFullText
        }
    }

    func updateFeedGroup(feedID: String, groupID: String?) async throws {
        guard let index = feeds.firstIndex(where: { $0.id == feedID }) else { return }
        if let groupID, let group = groups.first(where: { $0.id == groupID }) {
            feeds[index].groups = [group]
        } else {
            feeds[index].groups = []
        }
    }

    func patchItemState(
        itemID: String,
        isRead: Bool?,
        isStarred: Bool?,
        isSavedForLater: Bool?,
        isArchived: Bool?
    ) async throws {
        for (feedID, values) in itemsByFeed {
            guard let index = values.firstIndex(where: { $0.id == itemID }) else {
                continue
            }
            var updated = values[index]
            if let isRead {
                updated.isRead = isRead
            }
            if let isStarred {
                updated.isStarred = isStarred
            }
            if let isSavedForLater {
                updated.isSavedForLater = isSavedForLater
            }
            var nextValues = values
            nextValues[index] = updated
            itemsByFeed[feedID] = nextValues
            if var detail = itemDetails[itemID] {
                detail = ArticleDetail(
                    id: detail.id,
                    title: detail.title,
                    canonicalURL: detail.canonicalURL,
                    publishedAt: detail.publishedAt,
                    summary: detail.summary,
                    contentHTML: detail.contentHTML,
                    contentText: detail.contentText,
                    isRead: isRead ?? detail.isRead,
                    isStarred: isStarred ?? detail.isStarred,
                    isSavedForLater: isSavedForLater ?? detail.isSavedForLater,
                    isArchived: isArchived ?? detail.isArchived
                )
                itemDetails[itemID] = detail
            }
        }
    }

    func getCoreMeta() async throws -> CoreMeta {
        CoreMeta(apiVersion: 2, appVersion: "0.1.0")
    }

    func listPendingSyncEvents(limit: Int) async throws -> [SyncEvent] {
        []
    }

    func acknowledgeSyncEvents(eventIDs: [String]) async throws -> Int {
        0
    }
}

@MainActor
final class AppStateTests: XCTestCase {
    func testNativeReaderServiceResolvesDefaultDatabasePath() {
        let dbPath = NativeReaderService.defaultDBPath()

        XCTAssertNotNil(dbPath)
        XCTAssertTrue(dbPath?.hasSuffix(".infomatrix/infomatrix.db") == true)
    }

    func testAddSubscriptionAndBootstrap() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        await state.addSubscription(input: "https://example.com")
        await state.bootstrap()

        XCTAssertEqual(state.feeds.count, 1)
        XCTAssertEqual(state.items.count, 1)
    }

    func testAppleSubscriptionComposerUsesDirectFeedFlowForFeedUrls() async {
        let service = MockReaderService()
        let state = AppState(service: service)
        let composer = ReaderShellView(state: state)

        let didSucceed = await composer.submitSubscription(input: "https://example.com/feed.xml")

        XCTAssertTrue(didSucceed)
        let subscribeCalls = await service.subscribeCallCount()
        let discoverCalls = await service.discoverCallCount()
        XCTAssertEqual(subscribeCalls, 1)
        XCTAssertEqual(discoverCalls, 0)
        XCTAssertEqual(state.feeds.count, 1)
        XCTAssertEqual(state.items.count, 1)
        XCTAssertNil(state.pendingDiscoverySelection)
    }

    func testPrepareSubscriptionPresentsDiscoverySelection() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        let didStartSelection = await state.prepareSubscription(input: "https://example.com")
        XCTAssertTrue(didStartSelection)
        XCTAssertNotNil(state.pendingDiscoverySelection)
        XCTAssertTrue(state.feeds.isEmpty)
    }

    func testBootstrapSurvivesMissingGroupsEndpoint() async {
        let service = MockReaderService()
        await service.setListGroupsError(
            NSError(
                domain: "InfoMatrix",
                code: 404,
                userInfo: [NSLocalizedDescriptionKey: "Not Found"]
            )
        )
        let state = AppState(service: service)

        await state.addSubscription(input: "https://example.com")
        await state.bootstrap()

        XCTAssertEqual(state.feeds.count, 1)
        XCTAssertEqual(state.items.count, 1)
        XCTAssertFalse(state.feeds.isEmpty)
    }

    func testLiveAppleSubscriptionSmokeForKnownFeeds() async throws {
        try XCTSkipIf(
            ProcessInfo.processInfo.environment["INFOMATRIX_LIVE_SUBSCRIPTION_SMOKE"] != "1",
            "Set INFOMATRIX_LIVE_SUBSCRIPTION_SMOKE=1 to run the live Apple GUI subscription smoke test."
        )

        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(
            "InfoMatrixLiveSubscription-\(UUID().uuidString)",
            isDirectory: true
        )
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let service = NativeReaderService(dbPath: tempDir.appendingPathComponent("infomatrix.db").path)
        let state = AppState(service: service)
        let composer = ReaderShellView(state: state)
        let feedURLs = [
            "https://blog.samaltman.com/posts.atom",
            "https://mengyanggao.github.io/rss.xml",
            "https://www.ruanyifeng.com/blog/atom.xml",
        ]

        for (index, feedURL) in feedURLs.enumerated() {
            let directTempDir = tempDir.appendingPathComponent("direct-\(index)", isDirectory: true)
            try FileManager.default.createDirectory(at: directTempDir, withIntermediateDirectories: true)
            defer { try? FileManager.default.removeItem(at: directTempDir) }

            let rawPayload: [String: Any] = [
                "db_path": directTempDir.appendingPathComponent("infomatrix.db").path,
                "input_url": feedURL,
            ]
            let rawData = try JSONSerialization.data(withJSONObject: rawPayload)
            let rawJSONString = String(data: rawData, encoding: .utf8)
            XCTAssertNotNil(rawJSONString)
            if let rawJSONString {
                let rawResult = rawJSONString.withCString { cString -> String? in
                    guard let output = infomatrix_core_subscribe_input_json(cString) else {
                        return nil
                    }
                    defer { infomatrix_core_free_string(output) }
                    return String(cString: output)
                }
                let rawEnvelope = try XCTUnwrap(rawResult)
                XCTAssertTrue(rawEnvelope.contains("\"feed_id\""), rawEnvelope)
                XCTAssertTrue(rawEnvelope.contains("\"resolved_feed_url\""), rawEnvelope)
                XCTAssertTrue(rawEnvelope.contains("\"subscription_source\""), rawEnvelope)
            }

            let directService = NativeReaderService(
                dbPath: directTempDir.appendingPathComponent("infomatrix.db").path
            )
            let directResult = try await directService.subscribe(inputURL: feedURL)
            XCTAssertFalse(directResult.feedID.isEmpty, "Empty feed ID for \(feedURL)")
            XCTAssertEqual(directResult.subscriptionSource, "direct_feed")

            let didSucceed = await composer.submitSubscription(input: feedURL)
            XCTAssertTrue(
                didSucceed,
                "Failed to subscribe to \(feedURL): \(state.errorMessage ?? "unknown error")"
            )
            XCTAssertEqual(state.feeds.count, index + 1, "Unexpected feed count after \(feedURL)")
            XCTAssertFalse(state.items.isEmpty, "No items loaded after \(feedURL)")
            XCTAssertNil(state.pendingDiscoverySelection)
            XCTAssertNil(state.errorMessage)
        }
    }

    func testStateTransitions() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        await state.addSubscription(input: "https://example.com")
        await state.bootstrap()

        guard let item = state.items.first else {
            XCTFail("Missing item")
            return
        }

        state.toggleRead(item.id, current: item.isRead)
        state.toggleStarred(item.id, current: item.isStarred)
        state.toggleSavedForLater(item.id, current: item.isSavedForLater)

        try? await Task.sleep(nanoseconds: 200_000_000)

        let updated = state.items.first { $0.id == item.id }
        XCTAssertEqual(updated?.isRead, true)
        XCTAssertEqual(updated?.isStarred, true)
        XCTAssertEqual(updated?.isSavedForLater, true)
    }

    func testSavingBookmarkAllowsBlankTitleAndCapturesContent() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        let initialFullTextCalls = await service.fullTextCallCount()
        let didSave = await state.createBookmark(
            url: "https://example.com",
            title: "",
            note: "Saved note"
        )

        XCTAssertTrue(didSave)
        let fullTextCalls = await service.fullTextCallCount()
        XCTAssertEqual(fullTextCalls, initialFullTextCalls)
        XCTAssertEqual(state.selectedFeedID, AppState.laterSelectionID)
        XCTAssertEqual(state.selectedItemDetail?.title, "example.com")
        XCTAssertEqual(state.selectedItemDetail?.sourceURL?.absoluteString, "https://example.com")
        XCTAssertTrue(
            state.selectedItemDetail?.contentHTML?.contains("Captured content from https://example.com") == true
        )
    }

    func testSelectingUnreadItemMarksItRead() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        await state.addSubscription(input: "https://example.com")
        await state.bootstrap()

        state.selectUnreadScope()
        try? await Task.sleep(nanoseconds: 200_000_000)

        guard let unreadItem = state.items.first else {
            XCTFail("Missing unread item")
            return
        }

        state.didSelectItem(unreadItem.id)
        try? await Task.sleep(nanoseconds: 300_000_000)

        XCTAssertEqual(state.selectedFeedID, AppState.unreadSelectionID)
        XCTAssertEqual(state.unreadItemsCount, 0)
        XCTAssertTrue(state.items.isEmpty)
        XCTAssertEqual(state.selectedItemDetail?.isRead, true)
    }

    func testLaterAndArchiveScopesAreSelectable() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        state.selectLaterScope()
        XCTAssertEqual(state.selectedFeedID, AppState.laterSelectionID)

        state.selectArchiveScope()
        XCTAssertEqual(state.selectedFeedID, AppState.archiveSelectionID)

        await state.bootstrap()
        XCTAssertEqual(state.laterItemsCount, 0)
        XCTAssertEqual(state.archiveItemsCount, 0)
    }

    func testNotesScopeUsesNoteKindFilter() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        await state.addSubscription(input: "https://example.com")
        await state.bootstrap()

        guard let feedID = state.feeds.first?.id else {
            XCTFail("Missing feed")
            return
        }

        await service.setFeedItems(
            feedID,
            items: [
                ArticleItem(
                    id: "article-1",
                    title: "Feed article",
                    kind: "article",
                    sourceKind: "feed",
                    canonicalURL: URL(string: "https://example.com/article"),
                    publishedAt: nil,
                    isRead: false,
                    isStarred: false,
                    isSavedForLater: false,
                    isArchived: false
                ),
                ArticleItem(
                    id: "note-1",
                    title: "Stored note",
                    kind: "note",
                    sourceKind: "manual",
                    canonicalURL: nil,
                    publishedAt: nil,
                    isRead: false,
                    isStarred: false,
                    isSavedForLater: false,
                    isArchived: false
                )
            ]
        )
        await service.setItemDetail(
            "note-1",
            detail: ArticleDetail(
                id: "note-1",
                title: "Stored note",
                canonicalURL: nil,
                publishedAt: nil,
                summary: "Stored note body",
                contentHTML: nil,
                contentText: "Stored note body",
                isRead: false,
                isStarred: false,
                isSavedForLater: false,
                isArchived: false
            )
        )

        state.selectNotesScope()
        try? await Task.sleep(nanoseconds: 200_000_000)

        let requests = await service.listAllItemsRequests
        XCTAssertEqual(requests.last?.kind, "note")
        XCTAssertEqual(state.selectedFeedID, AppState.notesSelectionID)
        XCTAssertEqual(state.items.count, 1)
        XCTAssertEqual(state.items.first?.kind, "note")
    }

    func testSelectingCodeHeavyItemAutoFetchesFullTextByDefault() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        await state.addSubscription(input: "https://example.com")
        await state.bootstrap()

        let codeItemID = "item-1"
        let initialFullTextCalls = await service.fullTextCallCount()
        await service.setItemDetail(
            codeItemID,
            detail: ArticleDetail(
                id: codeItemID,
                title: "Code Heavy",
                canonicalURL: URL(string: "https://example.com/post"),
                publishedAt: nil,
                summary: "Summary",
                contentHTML: "<article><pre><code>let answer = 42</code></pre></article>",
                contentText: "Summary",
                isRead: false,
                isStarred: false,
                isSavedForLater: false,
                isArchived: false
            )
        )

        state.didSelectItem(codeItemID)
        try? await Task.sleep(nanoseconds: 250_000_000)

        let fullTextCalls = await service.fullTextCallCount()
        XCTAssertEqual(
            state.selectedItemDetail?.contentHTML,
            "<article><pre><code>let answer = 42</code></pre></article>"
        )
        XCTAssertGreaterThan(fullTextCalls, initialFullTextCalls)
    }

    func testSelectingCodeHeavyItemSkipsAutoFetchWhenDisabled() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        await state.addSubscription(input: "https://example.com")
        await state.bootstrap()

        guard let feedID = state.feeds.first?.id else {
            XCTFail("Missing feed")
            return
        }

        await state.setFeedAutoFullText(feedID, enabled: false)

        let codeItemID = "item-1"
        let initialFullTextCalls = await service.fullTextCallCount()
        await service.setItemDetail(
            codeItemID,
            detail: ArticleDetail(
                id: codeItemID,
                title: "Code Heavy",
                canonicalURL: URL(string: "https://example.com/post"),
                publishedAt: nil,
                summary: "Summary",
                contentHTML: "<article><pre><code>let answer = 42</code></pre></article>",
                contentText: "Summary",
                isRead: false,
                isStarred: false,
                isSavedForLater: false,
                isArchived: false
            )
        )

        state.didSelectItem(codeItemID)
        try? await Task.sleep(nanoseconds: 250_000_000)

        let fullTextCalls = await service.fullTextCallCount()
        XCTAssertEqual(fullTextCalls, initialFullTextCalls)
        XCTAssertEqual(
            state.selectedItemDetail?.contentHTML,
            "<article><pre><code>let answer = 42</code></pre></article>"
        )

        await state.fetchFullTextForSelectedItem()
        try? await Task.sleep(nanoseconds: 200_000_000)

        let refreshedFullTextCalls = await service.fullTextCallCount()
        XCTAssertGreaterThan(refreshedFullTextCalls, fullTextCalls)
    }

    func testStateMutationDoesNotDependOnFullTextFetch() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        await state.addSubscription(input: "https://example.com")
        await state.bootstrap()

        guard let feedID = state.feeds.first?.id else {
            XCTFail("Missing feed")
            return
        }
        await state.setFeedAutoFullText(feedID, enabled: false)

        await service.setItemDetail(
            "item-1",
            detail: ArticleDetail(
                id: "item-1",
                title: "Welcome",
                canonicalURL: URL(string: "https://example.com/post"),
                publishedAt: nil,
                summary: "Welcome summary",
                contentHTML: "<article><p>Body</p></article>",
                contentText: "Welcome body",
                isRead: false,
                isStarred: false,
                isSavedForLater: false,
                isArchived: false
            )
        )
        await service.setFullTextError(
            NSError(
                domain: "InfoMatrix",
                code: 405,
                userInfo: [NSLocalizedDescriptionKey: "HTTP 405"]
            )
        )

        state.didSelectItem("item-1")
        try? await Task.sleep(nanoseconds: 150_000_000)

        state.toggleStarred("item-1", current: false)
        try? await Task.sleep(nanoseconds: 250_000_000)

        XCTAssertEqual(state.items.first?.isStarred, true)
        XCTAssertEqual(state.starredItemsCount, 1)
        XCTAssertNil(state.errorMessage)
    }

    func testStarredScopeShowsStarredItems() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        await state.addSubscription(input: "https://example.com")
        await state.bootstrap()

        guard let item = state.items.first else {
            XCTFail("Missing item")
            return
        }

        state.toggleStarred(item.id, current: item.isStarred)
        try? await Task.sleep(nanoseconds: 200_000_000)

        state.selectStarredScope()
        try? await Task.sleep(nanoseconds: 200_000_000)

        XCTAssertEqual(state.selectedFeedID, AppState.starredSelectionID)
        XCTAssertEqual(state.items.count, 1)
        XCTAssertEqual(state.items.first?.isStarred, true)
    }

    func testNotificationSettingsRoundTrip() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        _ = await state.addSubscription(input: "https://example.com")
        await state.bootstrap()

        guard let feed = state.feeds.first else {
            XCTFail("Missing feed")
            return
        }

        let updatedFeedSettings = NotificationSettings(
            enabled: true,
            mode: .digest,
            digestPolicy: DigestPolicy(enabled: true, intervalMinutes: 45, maxItems: 8),
            quietHours: QuietHours(enabled: true, startMinute: 21 * 60, endMinute: 7 * 60),
            minimumIntervalMinutes: 20,
            highPriority: true,
            keywordInclude: ["rust"],
            keywordExclude: ["ads"]
        )
        let savedFeedSettings = await state.saveFeedNotificationSettings(feedID: feed.id, settings: updatedFeedSettings)
        XCTAssertTrue(savedFeedSettings)

        let globalSettings = GlobalNotificationSettings(
            backgroundRefreshEnabled: false,
            backgroundRefreshIntervalMinutes: 30,
            digestPolicy: DigestPolicy(enabled: true, intervalMinutes: 90, maxItems: 12),
            defaultFeedSettings: updatedFeedSettings
        )
        let savedGlobalSettings = await state.saveGlobalNotificationSettings(globalSettings)
        XCTAssertTrue(savedGlobalSettings)

        let loadedGlobal = await state.loadGlobalNotificationSettings()
        XCTAssertEqual(loadedGlobal?.backgroundRefreshEnabled, false)
        let loadedFeed = await state.loadFeedNotificationSettings(feedID: feed.id)
        XCTAssertEqual(loadedFeed?.mode, .digest)
    }

    func testGroupSelectionAggregatesFeedItems() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        await state.addSubscription(input: "https://one.example.com")
        await state.addSubscription(input: "https://two.example.com")
        await state.bootstrap()

        guard state.feeds.count >= 2 else {
            XCTFail("Expected two feeds")
            return
        }

        let firstFeedID = state.feeds[0].id
        let secondFeedID = state.feeds[1].id

        let group = await state.createGroup("Tech")
        XCTAssertNotNil(group)

        if let group {
            await state.setFeedGroup(firstFeedID, groupID: group.id)
            await state.setFeedGroup(secondFeedID, groupID: group.id)
            await service.setFeedItems(
                firstFeedID,
                items: [
                    ArticleItem(
                        id: "feed-a-1",
                        title: "Feed A One",
                        canonicalURL: URL(string: "https://one.example.com/a1"),
                        publishedAt: "2026-03-20T10:00:00Z",
                        isRead: false,
                        isStarred: false,
                        isSavedForLater: false,
                        isArchived: false
                    )
                ]
            )
            await service.setItemDetail(
                "feed-a-1",
                detail: ArticleDetail(
                    id: "feed-a-1",
                    title: "Feed A One",
                    canonicalURL: URL(string: "https://one.example.com/a1"),
                    publishedAt: "2026-03-20T10:00:00Z",
                    summary: "Feed A summary",
                    contentHTML: nil,
                    contentText: "Feed A body",
                    isRead: false,
                    isStarred: false,
                    isSavedForLater: false,
                    isArchived: false
                )
            )
            await service.setFeedItems(
                secondFeedID,
                items: [
                    ArticleItem(
                        id: "feed-b-1",
                        title: "Feed B One",
                        canonicalURL: URL(string: "https://two.example.com/b1"),
                        publishedAt: "2026-03-20T11:00:00Z",
                        isRead: false,
                        isStarred: false,
                        isSavedForLater: false,
                        isArchived: false
                    )
                ]
            )
            await service.setItemDetail(
                "feed-b-1",
                detail: ArticleDetail(
                    id: "feed-b-1",
                    title: "Feed B One",
                    canonicalURL: URL(string: "https://two.example.com/b1"),
                    publishedAt: "2026-03-20T11:00:00Z",
                    summary: "Feed B summary",
                    contentHTML: nil,
                    contentText: "Feed B body",
                    isRead: false,
                    isStarred: false,
                    isSavedForLater: false,
                    isArchived: false
                )
            )

            state.selectGroup(group.id)
            try? await Task.sleep(nanoseconds: 250_000_000)

            XCTAssertEqual(state.selectedFeedID, AppState.groupSelectionPrefix + group.id)
            XCTAssertEqual(state.items.count, 2)
            XCTAssertEqual(state.items.first?.title, "Feed B One")
        }
    }

    func testDeleteSubscription() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        await state.addSubscription(input: "https://example.com")
        await state.bootstrap()
        XCTAssertEqual(state.feeds.count, 1)

        if let feedID = state.feeds.first?.id {
            await state.removeFeed(feedID)
        }
        XCTAssertTrue(state.feeds.isEmpty)
    }

    func testRefreshDueFeedsCallsBackend() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        await state.addSubscription(input: "https://example.com")
        await state.bootstrap()

        let didRefresh = await state.refreshDueFeeds()
        XCTAssertTrue(didRefresh)
        let refreshDueCallCount = await service.refreshDueCallCount()
        XCTAssertEqual(refreshDueCallCount, 1)
        XCTAssertEqual(state.errorMessage, "已刷新 1 个到期订阅，共 1 条条目")
    }

    func testWebsiteDiscoveryPresentsMultipleFeedsForSelection() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        let discoverResponse = DiscoverSiteResponse(
            normalizedSiteURL: URL(string: "https://example.com")!,
            discoveredFeeds: [
                DiscoverFeed(
                    url: URL(string: "https://example.com/feed.xml")!,
                    title: "Main Feed",
                    feedType: "rss",
                    confidence: 0.95,
                    source: "autodiscovery",
                    score: 58
                ),
                DiscoverFeed(
                    url: URL(string: "https://example.com/podcast.xml")!,
                    title: "Podcast Feed",
                    feedType: "rss",
                    confidence: 0.92,
                    source: "autodiscovery",
                    score: 42
                )
            ],
            siteTitle: "Example",
            warnings: ["multiple candidates found"]
        )
        await service.setDiscoverResultOverride(discoverResponse)

        let didStartSelection = await state.addSubscription(input: "https://example.com")
        XCTAssertTrue(didStartSelection)
        XCTAssertNotNil(state.pendingDiscoverySelection)
        XCTAssertTrue(state.feeds.isEmpty)

        guard let candidate = state.pendingDiscoverySelection?.discoveredFeeds.last else {
            XCTFail("Missing discovery candidate")
            return
        }

        let didSubscribe = await state.subscribeToDiscoveredFeed(candidate)
        XCTAssertTrue(didSubscribe)
        XCTAssertNil(state.pendingDiscoverySelection)
        XCTAssertEqual(state.feeds.count, 1)
        XCTAssertEqual(state.feeds.first?.feedURL.absoluteString, "https://example.com/podcast.xml")
    }

    func testFeedLikeUrlFallsBackWhenDiscoveryFails() async {
        let service = MockReaderService()
        await service.setDiscoverError(
            NSError(
                domain: "InfoMatrix",
                code: 503,
                userInfo: [NSLocalizedDescriptionKey: "discovery unavailable"]
            )
        )
        let state = AppState(service: service)

        let didSubscribe = await state.addSubscription(input: "https://example.com/blog/?feed=rss2")
        XCTAssertTrue(didSubscribe)
        XCTAssertEqual(state.feeds.count, 1)
        XCTAssertEqual(state.feeds.first?.feedURL.absoluteString, "https://example.com/blog/?feed=rss2")
    }

    func testImportOPMLReloadsFeeds() async {
        let service = MockReaderService()
        let state = AppState(service: service)

        let xml = """
        <?xml version="1.0" encoding="UTF-8"?>
        <opml version="2.0"></opml>
        """

        let didImport = await state.importOPML(opmlXML: xml)
        XCTAssertTrue(didImport)
        XCTAssertEqual(state.feeds.count, 1)
    }
}
