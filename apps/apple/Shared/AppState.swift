import Foundation
import SwiftUI

public struct PendingDiscoverySelection: Identifiable, Equatable, Sendable {
    public let id: String
    public let inputURL: String
    public let normalizedSiteURL: URL
    public let siteTitle: String?
    public let warnings: [String]
    public let discoveredFeeds: [DiscoverFeed]

    public init(
        inputURL: String,
        normalizedSiteURL: URL,
        siteTitle: String?,
        warnings: [String],
        discoveredFeeds: [DiscoverFeed]
    ) {
        self.id = normalizedSiteURL.absoluteString
        self.inputURL = inputURL
        self.normalizedSiteURL = normalizedSiteURL
        self.siteTitle = siteTitle
        self.warnings = warnings
        self.discoveredFeeds = discoveredFeeds
    }
}

@MainActor
public final class AppState: ObservableObject {
    public static let allItemsSelectionID = "__all_items__"
    public static let unreadSelectionID = "__unread_items__"
    public static let starredSelectionID = "__starred_items__"
    public static let laterSelectionID = "__later_items__"
    public static let notesSelectionID = "__notes_items__"
    public static let archiveSelectionID = "__archive_items__"
    public static let groupSelectionPrefix = "__group__:"

    @Published public private(set) var feeds: [Feed]
    @Published public private(set) var groups: [FeedGroup]
    @Published public private(set) var items: [ArticleItem]
    @Published public private(set) var allItemsCount: Int
    @Published public private(set) var unreadItemsCount: Int
    @Published public private(set) var starredItemsCount: Int
    @Published public private(set) var laterItemsCount: Int
    @Published public private(set) var notesItemsCount: Int
    @Published public private(set) var archiveItemsCount: Int
    @Published public private(set) var selectedItemDetail: ArticleDetail?
    @Published public var selectedFeedID: String?
    @Published public var selectedItemID: String?
    @Published public var errorMessage: String?
    @Published public var isLoading: Bool
    @Published public var searchQuery: String
    @Published public private(set) var pendingDiscoverySelection: PendingDiscoverySelection?

    private let service: ReaderService

    public init(service: ReaderService, feeds: [Feed] = [], items: [ArticleItem] = []) {
        self.service = service
        self.feeds = feeds
        self.groups = []
        self.items = items
        self.allItemsCount = 0
        self.unreadItemsCount = 0
        self.starredItemsCount = 0
        self.laterItemsCount = 0
        self.notesItemsCount = 0
        self.archiveItemsCount = 0
        self.selectedItemDetail = nil
        self.isLoading = false
        self.searchQuery = ""
        self.pendingDiscoverySelection = nil
    }

    public func bootstrap() async {
        await reloadFeeds()
    }

    public func reloadFeeds() async {
        isLoading = true
        defer { isLoading = false }

        do {
            async let fetchedFeedsTask = service.listFeeds()
            async let fetchedGroupsTask = service.listGroups()
            async let countsTask = service.itemCounts()

            let fetchedFeeds = (try await fetchedFeedsTask).filter { !isSpecialSelection($0.id) }
            let fetchedGroups = (try? await fetchedGroupsTask) ?? []
            let counts = try? await countsTask

            withAnimation(.snappy(duration: 0.18)) {
                feeds = fetchedFeeds
                groups = fetchedGroups
                if let counts {
                    applyScopeCounts(counts)
                } else {
                    applyScopeCounts(.init(all: 0, unread: 0, starred: 0, later: 0, notes: 0, archive: 0))
                }
                if selectedFeedID == nil {
                    selectedFeedID = AppState.allItemsSelectionID
                }
            }
            if await refreshItemsForSelection(query: searchQuery) {
                errorMessage = nil
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    @discardableResult
    public func addSubscription(input: String) async -> Bool {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            errorMessage = "请输入网站或 Feed URL"
            return false
        }
        let normalized = normalizeInputURL(trimmed)
        guard URL(string: normalized) != nil else {
            errorMessage = "无效的 URL"
            return false
        }

        isLoading = true
        defer { isLoading = false }

        if looksLikeFeedURL(normalized) {
            if await subscribeDirectly(inputURL: normalized) {
                return true
            }
            do {
                let discovered = try await service.discoverSite(siteURL: normalized)
                return await handleDiscoveryResult(discovered, inputURL: normalized)
            } catch {
                errorMessage = error.localizedDescription
                return false
            }
        }

        do {
            let discovered = try await service.discoverSite(siteURL: normalized)
            return await handleDiscoveryResult(discovered, inputURL: normalized)
        } catch {
            if await subscribeDirectly(inputURL: normalized) {
                return true
            }
            errorMessage = error.localizedDescription
            return false
        }
    }

    @discardableResult
    public func prepareSubscription(input: String) async -> Bool {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            errorMessage = "请输入网站或 Feed URL"
            return false
        }
        let normalized = normalizeInputURL(trimmed)
        guard URL(string: normalized) != nil else {
            errorMessage = "无效的 URL"
            return false
        }

        isLoading = true
        defer { isLoading = false }

        do {
            let discovered = try await service.discoverSite(siteURL: normalized)
            return await prepareDiscoverySelection(discovered, inputURL: normalized)
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    @discardableResult
    public func subscribeToDiscoveredFeed(_ candidate: DiscoverFeed) async -> Bool {
        isLoading = true
        defer { isLoading = false }

        do {
            let feedID = try await service.addSubscription(
                feedURL: candidate.url.absoluteString,
                title: candidate.title
            )
            pendingDiscoverySelection = nil
            await completeSubscription(feedID: feedID)
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    public func dismissDiscoverySelection() {
        pendingDiscoverySelection = nil
    }

    @discardableResult
    public func importOPML(opmlXML: String) async -> Bool {
        let trimmed = opmlXML.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            errorMessage = "OPML 内容为空"
            return false
        }

        isLoading = true
        defer { isLoading = false }

        do {
            let result = try await service.importOPML(opmlXML: trimmed)
            await reloadFeeds()
            errorMessage = "已导入 \(result.uniqueFeedCount) 个订阅（解析 \(result.parsedFeedCount) 条）"
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    public func exportOPML() async -> String? {
        isLoading = true
        defer { isLoading = false }

        do {
            let result = try await service.exportOPML()
            errorMessage = "已生成 OPML（\(result.feedCount) 个订阅）"
            return result.opmlXML
        } catch {
            errorMessage = error.localizedDescription
            return nil
        }
    }

    @discardableResult
    public func refreshDueFeeds(limit: Int = 20) async -> Bool {
        isLoading = true
        defer { isLoading = false }

        do {
            let result = try await service.refreshDueFeeds(limit: limit)
            await reloadFeeds()
            errorMessage = "已刷新 \(result.refreshedCount) 个到期订阅，共 \(result.totalItemCount) 条条目"
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    public func refreshSelectedFeed() async {
        guard let feedID = selectedFeedID else { return }
        if let groupID = selectedGroupID(from: feedID) {
            await refreshGroup(groupID: groupID)
            return
        }
        guard !isSpecialSelection(feedID) else {
            _ = await refreshItemsForSelection(query: searchQuery)
            return
        }
        await refresh(feedID: feedID)
    }

    public func refresh(feedID: String) async {
        guard !isSpecialSelection(feedID) else {
            return
        }
        do {
            try await service.refresh(feedID: feedID)
            _ = await refreshItemsForSelection(query: searchQuery)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func removeFeed(_ feedID: String) async {
        isLoading = true
        defer { isLoading = false }

        do {
            try await service.deleteFeed(feedID: feedID)
            if selectedFeedID == feedID {
                selectedFeedID = nil
                selectedItemID = nil
                selectedItemDetail = nil
            }
            await reloadFeeds()
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func fetchFullTextForSelectedItem() async {
        guard let itemID = selectedItemID else { return }
        do {
            selectedItemDetail = try await service.fetchFullText(itemID: itemID)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func applySearch() {
        guard selectedFeedID != nil else { return }
        Task {
            _ = await refreshItemsForSelection(query: searchQuery)
        }
    }

    public func didSelectFeed(_ feedID: String?) {
        selectedFeedID = feedID
        selectedItemID = nil
        selectedItemDetail = nil
        guard feedID != nil else {
            items = []
            return
        }
        Task {
            _ = await refreshItemsForSelection(query: searchQuery)
        }
    }

    public func didSelectItem(_ itemID: String?) {
        selectedItemID = itemID
        selectedItemDetail = nil
        guard let itemID else { return }
        let currentItem = items.first(where: { $0.id == itemID })
        Task {
            do {
                if let currentItem, !currentItem.isRead {
                    try await service.patchItemState(
                        itemID: itemID,
                        isRead: true,
                        isStarred: nil,
                        isSavedForLater: nil,
                        isArchived: nil
                    )
                    if let counts = try? await refreshScopeCounts() {
                        applyScopeCounts(counts)
                    }
                    _ = await refreshItemsForSelection(query: searchQuery)
                }
                selectedItemID = itemID
                selectedItemDetail = try await hydratedDetail(
                    itemID: itemID,
                    allowFullText: shouldAutoFetchFullText(for: itemID)
                )
            } catch {
                if (error as NSError).code != 404 {
                    errorMessage = error.localizedDescription
                }
            }
        }
    }

    public func toggleRead(_ itemID: String, current: Bool) {
        Task {
            await patchAndRefreshVisibleState(
                itemID: itemID,
                isRead: !current,
                isStarred: nil,
                isSavedForLater: nil
            )
        }
    }

    public func toggleStarred(_ itemID: String, current: Bool) {
        Task {
            await patchAndRefreshVisibleState(
                itemID: itemID,
                isRead: nil,
                isStarred: !current,
                isSavedForLater: nil
            )
        }
    }

    public func toggleSavedForLater(_ itemID: String, current: Bool) {
        Task {
            await patchAndRefreshVisibleState(
                itemID: itemID,
                isRead: nil,
                isStarred: nil,
                isSavedForLater: !current
            )
        }
    }

    public func toggleArchived(_ itemID: String, current: Bool) {
        Task {
            await patchAndRefreshVisibleState(
                itemID: itemID,
                isRead: nil,
                isStarred: nil,
                isSavedForLater: nil,
                isArchived: !current
            )
        }
    }

    public func toggleReadForSelectedItem() {
        guard let selected = selectedItemDetail else { return }
        toggleRead(selected.id, current: selected.isRead)
    }

    public func toggleStarForSelectedItem() {
        guard let selected = selectedItemDetail else { return }
        toggleStarred(selected.id, current: selected.isStarred)
    }

    public func toggleLaterForSelectedItem() {
        guard let selected = selectedItemDetail else { return }
        toggleSavedForLater(selected.id, current: selected.isSavedForLater)
    }

    public func toggleArchiveForSelectedItem() {
        guard let selected = selectedItemDetail else { return }
        toggleArchived(selected.id, current: selected.isArchived)
    }

    public func selectAllItemsScope() {
        didSelectFeed(AppState.allItemsSelectionID)
    }

    public func selectUnreadScope() {
        didSelectFeed(AppState.unreadSelectionID)
    }

    public func selectStarredScope() {
        didSelectFeed(AppState.starredSelectionID)
    }

    public func selectGroup(_ groupID: String) {
        didSelectFeed(AppState.groupSelectionPrefix + groupID)
    }

    public func selectLaterScope() {
        didSelectFeed(AppState.laterSelectionID)
    }

    public func selectNotesScope() {
        didSelectFeed(AppState.notesSelectionID)
    }

    public func selectArchiveScope() {
        didSelectFeed(AppState.archiveSelectionID)
    }

    @discardableResult
    public func createBookmark(url: String, title: String, note: String?) async -> Bool {
        let trimmedURL = url.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedNote = note?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let parsedURL = URL(string: trimmedURL) else {
            errorMessage = "请填写有效的网页 URL"
            return false
        }

        isLoading = true
        defer { isLoading = false }

        do {
            let created = try await service.createEntry(
                title: trimmedTitle,
                kind: "bookmark",
                sourceKind: "web",
                sourceID: nil,
                sourceURL: parsedURL.absoluteString,
                sourceTitle: trimmedTitle.isEmpty ? nil : trimmedTitle,
                canonicalURL: parsedURL.absoluteString,
                summary: trimmedNote?.isEmpty == false ? trimmedNote : nil,
                contentHTML: nil,
                contentText: nil
            )
            try await service.patchItemState(
                itemID: created.id,
                isRead: nil,
                isStarred: nil,
                isSavedForLater: true,
                isArchived: nil
            )
            selectLaterScope()
            await reloadFeeds()
            selectedItemID = created.id
            selectedItemDetail = created
            errorMessage = "已保存网页"
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    @discardableResult
    public func createNote(title: String, body: String) async -> Bool {
        let trimmedTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedBody = body.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedBody.isEmpty else {
            errorMessage = "随想内容不能为空"
            return false
        }

        isLoading = true
        defer { isLoading = false }

        do {
            let created = try await service.createEntry(
                title: trimmedTitle.isEmpty ? "未命名随想" : trimmedTitle,
                kind: "note",
                sourceKind: "manual",
                sourceID: nil,
                sourceURL: nil,
                sourceTitle: nil,
                canonicalURL: nil,
                summary: trimmedBody,
                contentHTML: nil,
                contentText: trimmedBody
            )
            selectNotesScope()
            await reloadFeeds()
            selectedItemID = created.id
            selectedItemDetail = created
            errorMessage = "已保存随想"
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    public func loadGlobalNotificationSettings() async -> GlobalNotificationSettings? {
        do {
            return try await service.getGlobalNotificationSettings()
        } catch {
            errorMessage = error.localizedDescription
            return nil
        }
    }

    @discardableResult
    public func saveGlobalNotificationSettings(_ settings: GlobalNotificationSettings) async -> Bool {
        do {
            _ = try await service.updateGlobalNotificationSettings(settings)
            errorMessage = "全局通知设置已保存"
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    public func loadFeedNotificationSettings(feedID: String) async -> NotificationSettings? {
        do {
            return try await service.getFeedNotificationSettings(feedID: feedID)
        } catch {
            errorMessage = error.localizedDescription
            return nil
        }
    }

    @discardableResult
    public func saveFeedNotificationSettings(feedID: String, settings: NotificationSettings) async -> Bool {
        do {
            _ = try await service.updateFeedNotificationSettings(feedID: feedID, settings: settings)
            errorMessage = "订阅通知设置已保存"
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    public func renameFeed(_ feedID: String, title: String?) async {
        do {
            try await service.updateFeed(feedID: feedID, title: title ?? "", autoFullText: nil)
            await reloadFeeds()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func setFeedAutoFullText(_ feedID: String, enabled: Bool) async {
        do {
            try await service.updateFeed(feedID: feedID, title: nil, autoFullText: enabled)
            updateLocalFeed(feedID: feedID, autoFullText: enabled)
            if enabled,
               let selectedItemID,
               let feed = feed(for: selectedItemID),
               feed.id == feedID
            {
                await fetchFullTextForSelectedItem()
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func setFeedGroup(_ feedID: String, groupID: String?) async {
        do {
            try await service.updateFeedGroup(feedID: feedID, groupID: groupID)
            await reloadFeeds()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    public func createGroup(_ name: String) async -> FeedGroup? {
        do {
            let group = try await service.createGroup(name: name)
            await reloadFeeds()
            return group
        } catch {
            errorMessage = error.localizedDescription
            return nil
        }
    }

    private func patchAndRefreshVisibleState(
        itemID: String,
        isRead: Bool?,
        isStarred: Bool?,
        isSavedForLater: Bool?,
        isArchived: Bool? = nil
    ) async {
        do {
            try await service.patchItemState(
                itemID: itemID,
                isRead: isRead,
                isStarred: isStarred,
                isSavedForLater: isSavedForLater,
                isArchived: isArchived
            )
            if let counts = try? await refreshScopeCounts() {
                applyScopeCounts(counts)
            }
            _ = await refreshItemsForSelection(query: searchQuery)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func refreshItemsForSelection(query: String?) async -> Bool {
        do {
            let fetched = try await fetchItemsForSelection(query: query)
            withAnimation(.snappy(duration: 0.18)) {
                items = fetched.items
                selectedFeedID = fetched.selectionID
                selectedItemID = fetched.selectedItemID
            }

            if let selectedItemID = fetched.selectedItemID {
                selectedItemDetail = try await hydratedDetail(itemID: selectedItemID, allowFullText: false)
            } else {
                selectedItemDetail = nil
            }
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    private struct SelectionFetchResult {
        let selectionID: String
        let items: [ArticleItem]
        let selectedItemID: String?
    }

    private func fetchItemsForSelection(query: String?) async throws -> SelectionFetchResult {
        guard let selection = selectedFeedID else {
            return SelectionFetchResult(selectionID: AppState.allItemsSelectionID, items: [], selectedItemID: nil)
        }

        let fetched: [ArticleItem]
        if selection == AppState.allItemsSelectionID {
            fetched = try await service.listAllItems(
                limit: selectionLimit(for: selection),
                searchQuery: query,
                filter: "all",
                kind: nil
            )
        } else if selection == AppState.unreadSelectionID {
            fetched = try await service.listAllItems(
                limit: selectionLimit(for: selection),
                searchQuery: query,
                filter: "unread",
                kind: nil
            )
        } else if selection == AppState.starredSelectionID {
            fetched = try await service.listAllItems(
                limit: selectionLimit(for: selection),
                searchQuery: query,
                filter: "starred",
                kind: nil
            )
        } else if selection == AppState.laterSelectionID {
            fetched = try await service.listAllItems(
                limit: selectionLimit(for: selection),
                searchQuery: query,
                filter: "later",
                kind: nil
            )
        } else if selection == AppState.archiveSelectionID {
            fetched = try await service.listAllItems(
                limit: selectionLimit(for: selection),
                searchQuery: query,
                filter: "archive",
                kind: nil
            )
        } else if selection == AppState.notesSelectionID {
            fetched = try await service.listAllItems(
                limit: selectionLimit(for: selection),
                searchQuery: query,
                filter: "all",
                kind: "note"
            )
        } else if let groupID = selectedGroupID(from: selection) {
            let groupedFeeds = feeds.filter { feed in
                feed.groups.contains(where: { $0.id == groupID })
            }
            var collected: [ArticleItem] = []
            let totalLimit = selectionLimit(for: selection)
            let perFeedLimit = max(50, min(250, (totalLimit / max(groupedFeeds.count, 1)) + 20))
            for feed in groupedFeeds {
                let items = try await service.listItems(
                    feedID: feed.id,
                    limit: perFeedLimit,
                    searchQuery: query
                )
                collected.append(contentsOf: items)
                if collected.count >= totalLimit {
                    break
                }
            }
            fetched = Array(deduplicatedItems(collected).prefix(totalLimit))
        } else {
            fetched = try await service.listItems(
                feedID: selection,
                limit: selectionLimit(for: selection),
                searchQuery: query
            )
        }
        if let selectedItemID, fetched.contains(where: { $0.id == selectedItemID }) {
            return SelectionFetchResult(
                selectionID: selection,
                items: fetched,
                selectedItemID: selectedItemID
            )
        }

        return SelectionFetchResult(
            selectionID: selection,
            items: fetched,
            selectedItemID: fetched.first?.id
        )
    }

    private func isSpecialSelection(_ feedID: String) -> Bool {
        feedID == AppState.allItemsSelectionID
            || feedID == AppState.unreadSelectionID
            || feedID == AppState.starredSelectionID
            || feedID == AppState.laterSelectionID
            || feedID == AppState.notesSelectionID
            || feedID == AppState.archiveSelectionID
    }

    private func selectedGroupID(from selectionID: String) -> String? {
        guard selectionID.hasPrefix(AppState.groupSelectionPrefix) else {
            return nil
        }
        return String(selectionID.dropFirst(AppState.groupSelectionPrefix.count))
    }

    private func deduplicatedItems(_ items: [ArticleItem]) -> [ArticleItem] {
        var seen = Set<String>()
        return items
            .filter { seen.insert($0.id).inserted }
            .sorted {
                let lhs = publishedDate(from: $0.publishedAt)
                let rhs = publishedDate(from: $1.publishedAt)
                if lhs != rhs {
                    return lhs > rhs
                }
                return $0.title.localizedCaseInsensitiveCompare($1.title) == .orderedAscending
            }
    }

    private func normalizeInputURL(_ value: String) -> String {
        if value.hasPrefix("http://") || value.hasPrefix("https://") {
            return value
        }
        return "https://\(value)"
    }

    private func looksLikeFeedURL(_ value: String) -> Bool {
        guard let url = URL(string: value) else { return false }
        let absolute = url.absoluteString.lowercased()
        let path = url.path.lowercased()
        if path.hasSuffix(".xml") || path.hasSuffix(".atom") || path.hasSuffix(".rss") || path.hasSuffix(".json") {
            return true
        }
        if path.contains("/feed")
            || path.hasSuffix("/feed")
            || path.contains("/rss")
            || path.hasSuffix("/rss")
            || path.contains("/atom")
            || path.hasSuffix("/atom")
            || path.contains("/json")
            || path.hasSuffix("/json")
        {
            return true
        }

        if let components = URLComponents(url: url, resolvingAgainstBaseURL: false) {
            for item in components.queryItems ?? [] {
                let name = item.name.lowercased()
                let value = item.value?.lowercased() ?? ""
                if name.contains("feed") || name.contains("rss") || name.contains("atom") || name.contains("json") {
                    return true
                }
                if value.contains("feed") || value.contains("rss") || value.contains("atom") || value.contains("json") {
                    return true
                }
                if value.contains("format=rss")
                    || value.contains("format=atom")
                    || value.contains("output=rss")
                    || value.contains("output=atom")
                {
                    return true
                }
            }
        }

        return absolute.contains("feedburner.com")
    }

    private func deduplicatedFeeds(_ feeds: [DiscoverFeed]) -> [DiscoverFeed] {
        var seen = Set<String>()
        let deduplicated = feeds.filter { candidate in
            seen.insert(candidate.url.absoluteString).inserted
        }
        return deduplicated.sorted {
            let lhsScore = $0.score ?? Int(($0.confidence * 120.0) - 30.0)
            let rhsScore = $1.score ?? Int(($1.confidence * 120.0) - 30.0)
            if lhsScore != rhsScore {
                return lhsScore > rhsScore
            }
            if $0.confidence != $1.confidence {
                return $0.confidence > $1.confidence
            }
            return $0.url.absoluteString < $1.url.absoluteString
        }
    }

    private func completeSubscription(feedID: String) async {
        pendingDiscoverySelection = nil
        selectedFeedID = feedID
        selectedItemID = nil
        selectedItemDetail = nil
        await reloadFeeds()
        errorMessage = nil
    }

    private func subscribeDirectly(inputURL: String) async -> Bool {
        do {
            let result = try await service.subscribe(inputURL: inputURL)
            await completeSubscription(feedID: result.feedID)
            return true
        } catch {
            return false
        }
    }

    private func handleDiscoveryResult(
        _ discovered: DiscoverSiteResponse,
        inputURL: String
    ) async -> Bool {
        let candidates = deduplicatedFeeds(discovered.discoveredFeeds)
        if candidates.isEmpty {
            let didSubscribe = await subscribeDirectly(inputURL: inputURL)
            if !didSubscribe {
                errorMessage = "未能从输入中识别可订阅的 Feed"
            }
            return didSubscribe
        }
        if candidates.count == 1, let candidate = candidates.first {
            do {
                let feedID = try await service.addSubscription(
                    feedURL: candidate.url.absoluteString,
                    title: candidate.title
                )
                await completeSubscription(feedID: feedID)
                return true
            } catch {
                errorMessage = error.localizedDescription
                return false
            }
        }

        pendingDiscoverySelection = PendingDiscoverySelection(
            inputURL: inputURL,
            normalizedSiteURL: discovered.normalizedSiteURL,
            siteTitle: discovered.siteTitle,
            warnings: discovered.warnings,
            discoveredFeeds: candidates
        )
        errorMessage = nil
        return true
    }

    private func prepareDiscoverySelection(
        _ discovered: DiscoverSiteResponse,
        inputURL: String,
        candidates: [DiscoverFeed]? = nil
    ) async -> Bool {
        let deduplicated = candidates ?? deduplicatedFeeds(discovered.discoveredFeeds)
        let selectionCandidates: [DiscoverFeed]
        if deduplicated.isEmpty {
            guard let fallbackURL = URL(string: inputURL) else {
                errorMessage = "未能从输入中识别可订阅的 Feed"
                return false
            }
            selectionCandidates = [
                DiscoverFeed(
                    url: fallbackURL,
                    title: fallbackURL.host.map { "\($0)" },
                    feedType: "unknown",
                    confidence: 1.0,
                    source: "input",
                    score: 0
                )
            ]
        } else {
            selectionCandidates = deduplicated
        }

        pendingDiscoverySelection = PendingDiscoverySelection(
            inputURL: inputURL,
            normalizedSiteURL: discovered.normalizedSiteURL,
            siteTitle: discovered.siteTitle,
            warnings: discovered.warnings,
            discoveredFeeds: selectionCandidates
        )
        errorMessage = nil
        return true
    }

    private func refreshScopeCounts() async throws -> ItemScopeCounts {
        try await service.itemCounts()
    }

    private func applyScopeCounts(_ counts: ItemScopeCounts) {
        allItemsCount = counts.all
        unreadItemsCount = counts.unread
        starredItemsCount = counts.starred
        laterItemsCount = counts.later
        notesItemsCount = counts.notes
        archiveItemsCount = counts.archive
    }

    private func selectionLimit(for selectionID: String) -> Int {
        switch selectionID {
        case AppState.allItemsSelectionID:
            return max(250, allItemsCount + 50)
        case AppState.unreadSelectionID:
            return max(250, unreadItemsCount + 50)
        case AppState.starredSelectionID:
            return max(250, starredItemsCount + 50)
        case AppState.laterSelectionID:
            return max(250, laterItemsCount + 50)
        case AppState.notesSelectionID:
            return max(100, notesItemsCount + 20)
        case AppState.archiveSelectionID:
            return max(250, archiveItemsCount + 50)
        default:
            return 1000
        }
    }

    private func refreshGroup(groupID: String) async {
        let groupedFeeds = feeds.filter { feed in
            feed.groups.contains(where: { $0.id == groupID })
        }
        let service = self.service
        let refreshErrors = await withTaskGroup(of: String?.self) { group in
            for feed in groupedFeeds {
                group.addTask {
                    do {
                        try await service.refresh(feedID: feed.id)
                        return nil
                    } catch {
                        return error.localizedDescription
                    }
                }
            }

            var errors: [String] = []
            for await error in group {
                if let error {
                    errors.append(error)
                }
            }
            return errors
        }
        if let firstError = refreshErrors.first {
            errorMessage = firstError
        }
        _ = await refreshItemsForSelection(query: searchQuery)
    }

    private func publishedDate(from value: String?) -> Date {
        guard let value, let date = ISO8601DateFormatter().date(from: value) else {
            return .distantPast
        }
        return date
    }

    private func updateLocalFeed(feedID: String, title: String? = nil, autoFullText: Bool? = nil) {
        guard let index = feeds.firstIndex(where: { $0.id == feedID }) else { return }
        if let title {
            feeds[index].title = title
        }
        if let autoFullText {
            feeds[index].autoFullText = autoFullText
        }
    }

    private func feed(for itemID: String) -> Feed? {
        guard let item = items.first(where: { $0.id == itemID }), item.sourceKind == "feed" else {
            return nil
        }
        if let sourceID = item.sourceID, let feed = feeds.first(where: { $0.id == sourceID }) {
            return feed
        }
        if let selectedFeedID, !isSpecialSelection(selectedFeedID) {
            return feeds.first(where: { $0.id == selectedFeedID })
        }
        return nil
    }

    private func shouldAutoFetchFullText(for itemID: String) -> Bool {
        feed(for: itemID)?.autoFullText ?? false
    }

    private func hydratedDetail(itemID: String, allowFullText: Bool) async throws -> ArticleDetail {
        var detail = try await service.itemDetail(itemID: itemID)
        guard allowFullText else {
            return detail
        }
        do {
            detail = try await service.fetchFullText(itemID: itemID)
        } catch {
            if (error as NSError).code != 404 {
                errorMessage = error.localizedDescription
            }
        }
        return detail
    }
}
