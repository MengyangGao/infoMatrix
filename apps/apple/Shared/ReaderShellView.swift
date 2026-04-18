import Foundation
import SwiftUI
import UniformTypeIdentifiers

#if canImport(WebKit)
import WebKit
#endif

#if canImport(AppKit)
import AppKit
#elseif canImport(UIKit)
import UIKit
#endif

private enum ReaderShellColors {
    #if canImport(AppKit)
    static let windowBackground = Color(nsColor: NSColor.windowBackgroundColor)
    static let controlBackground = Color(nsColor: NSColor.controlBackgroundColor)
    static let textBackground = Color(nsColor: NSColor.textBackgroundColor)
    #elseif canImport(UIKit)
    static let windowBackground = Color(uiColor: .systemBackground)
    static let controlBackground = Color(uiColor: .secondarySystemBackground)
    static let textBackground = Color(uiColor: .systemBackground)
    #else
    static let windowBackground = Color(.systemBackground)
    static let controlBackground = Color(.secondarySystemBackground)
    static let textBackground = Color(.systemBackground)
    #endif
}

public struct ReaderShellView: View {
    @ObservedObject private var state: AppState
    @Environment(\.openURL) private var openURL
    @Environment(\.colorScheme) private var colorScheme

    @State private var subscriptionInput: String = ""

    @State private var editingFeed: Feed?
    @State private var editTitle: String = ""
    @State private var editGroupID: String = ""
    @State private var newGroupName: String = ""
    @State private var isImportingOPML = false
    @State private var isExportingOPML = false
    @State private var opmlExportDocument: OPMLTextDocument?
    @State private var activeComposer: EntryComposerKind?
    @State private var notificationSettingsTarget: NotificationSettingsTarget?
    @State private var refreshSettingsTarget: RefreshSettingsTarget?

    private enum FocusedField {
        case subscription
        case search
        case category
    }
    @FocusState private var focusedField: FocusedField?

    fileprivate enum EntryComposerKind: String, Identifiable {
        case bookmark
        case note

        var id: String { rawValue }
    }

    fileprivate enum NotificationSettingsTarget: Identifiable {
        case global
        case feed(Feed)

        var id: String {
            switch self {
            case .global:
                return "global"
            case .feed(let feed):
                return "feed:\(feed.id)"
            }
        }
    }

    fileprivate enum RefreshSettingsTarget: Identifiable {
        case feed(Feed)
        case group(FeedGroup)

        var id: String {
            switch self {
            case .feed(let feed):
                return "feed:\(feed.id)"
            case .group(let group):
                return "group:\(group.id)"
            }
        }
    }

    public init(state: AppState) {
        self.state = state
    }

    public var body: some View {
        NavigationSplitView {
            sidebarColumn
        } content: {
            articleColumn
        } detail: {
            detailColumn
        }
        .navigationSplitViewStyle(.balanced)
        .task {
            await state.bootstrap()
            requestInitialFocus()
        }
        .onAppear {
#if os(macOS)
            NSApplication.shared.activate(ignoringOtherApps: true)
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
                NSApplication.shared.activate(ignoringOtherApps: true)
                NSApplication.shared.windows.first?.makeKeyAndOrderFront(nil)
                NSApplication.shared.windows.first?.orderFrontRegardless()
                requestInitialFocus()
            }
#endif
        }
        .sheet(item: $editingFeed) { feed in
            FeedEditSheet(
                feed: feed,
                groups: state.groups,
                initialTitle: editTitle,
                initialGroupID: editGroupID,
                newGroupName: newGroupName,
                onCancel: {
                    editingFeed = nil
                },
                onSave: { title, groupID, createGroupName in
                    Task {
                        let finalTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
                        await state.renameFeed(feed.id, title: finalTitle.isEmpty ? nil : finalTitle)

                        if !createGroupName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                            if let created = await state.createGroup(createGroupName) {
                                await state.setFeedGroup(feed.id, groupID: created.id)
                            }
                        } else if groupID.isEmpty {
                            await state.setFeedGroup(feed.id, groupID: nil)
                        } else {
                            await state.setFeedGroup(feed.id, groupID: groupID)
                        }
                        editingFeed = nil
                    }
                }
            )
        }
        .sheet(item: $activeComposer) { kind in
            EntryComposerSheet(
                kind: kind,
                onCancel: {
                    activeComposer = nil
                },
                onSaveBookmark: { url, title, note in
                    Task {
                        let saved = await state.createBookmark(url: url, title: title, note: note)
                        if saved {
                            activeComposer = nil
                        }
                    }
                },
                onSaveNote: { title, body in
                    Task {
                        let saved = await state.createNote(title: title, body: body)
                        if saved {
                            activeComposer = nil
                        }
                    }
                }
            )
        }
        .sheet(item: $notificationSettingsTarget) { target in
            NotificationSettingsSheet(
                target: target,
                state: state,
                onCancel: {
                    notificationSettingsTarget = nil
                }
            )
        }
        .sheet(item: $refreshSettingsTarget) { target in
            RefreshSettingsSheet(
                target: target,
                state: state,
                onCancel: {
                    refreshSettingsTarget = nil
                }
            )
        }
        .sheet(item: pendingDiscoveryBinding) { selection in
            DiscoverySelectionSheet(
                selection: selection,
                onCancel: {
                    state.dismissDiscoverySelection()
                },
                onSubscribe: { candidate in
                    Task {
                        _ = await state.subscribeToDiscoveredFeed(candidate)
                    }
                }
            )
        }
        .fileImporter(
            isPresented: $isImportingOPML,
            allowedContentTypes: opmlContentTypes,
            allowsMultipleSelection: false
        ) { result in
            handleOPMLImport(result)
        }
        .fileExporter(
            isPresented: $isExportingOPML,
            document: opmlExportDocument,
            contentType: .xml,
            defaultFilename: "InfoMatrix-Subscriptions"
        ) { result in
            switch result {
            case .success:
                state.errorMessage = "OPML 导出成功"
            case .failure(let error):
                state.errorMessage = "OPML 导出失败: \(error.localizedDescription)"
            }
        }
    }

    private var sidebarColumn: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                ZStack {
                    RoundedRectangle(cornerRadius: 11, style: .continuous)
                        .fill(.quaternary.opacity(0.55))
                    Image(systemName: "newspaper.fill")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(.primary)
                }
                .frame(width: 34, height: 34)

                VStack(alignment: .leading, spacing: 1) {
                    Text("InfoMatrix")
                        .font(.system(size: 17, weight: .semibold, design: .rounded))
                    Text("收件箱")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }

                Spacer(minLength: 0)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
            .padding(.horizontal, 8)
            .padding(.top, 8)
            .padding(.bottom, 6)

            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 8) {
                        subscriptionComposerCard
                            .padding(.bottom, 0)

                        syncStatusCard
                            .padding(.bottom, 0)

                        sidebarSectionHeader("首页")
                        VStack(spacing: 6) {
                            ForEach(SmartFeedShortcut.allCases, id: \.self) { shortcut in
                                Button {
                                    state.didSelectFeed(shortcut.selectionID)
                                } label: {
                                    SidebarSummaryRow(
                                        icon: shortcut.icon,
                                        title: shortcut.title,
                                        count: count(for: shortcut.selectionID),
                                        isSelected: state.selectedFeedID == shortcut.selectionID
                                    )
                                }
                                .buttonStyle(.plain)
                                .id(shortcut.selectionID)
                            }
                        }

                        sidebarSectionHeader("分类")
                        VStack(spacing: 6) {
                            HStack(spacing: 6) {
                                TextField("新建分类", text: $newGroupName)
                                    .textFieldStyle(.roundedBorder)
                                    .focused($focusedField, equals: .category)
                                    .onSubmit {
                                        createSidebarGroup()
                                    }
                                Button {
                                    createSidebarGroup()
                                } label: {
                                    Image(systemName: "plus")
                                }
                                .buttonStyle(.bordered)
                                .disabled(newGroupName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || state.isLoading)
                            }

                            if state.groups.isEmpty {
                                Text("暂无分类")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .padding(.horizontal, 2)
                            } else {
                                ForEach(state.groups, id: \.id) { group in
                                    HStack(spacing: 6) {
                                        Button {
                                            state.selectGroup(group.id)
                                        } label: {
                                            SidebarSummaryRow(
                                                icon: "folder.fill",
                                                title: group.name,
                                                count: feeds(in: group).count,
                                                isSelected: selectedGroupID == group.id
                                            )
                                        }
                                        .buttonStyle(.plain)
                                        .id(AppState.groupSelectionPrefix + group.id)

                                        Menu {
                                            Button("自动刷新…") {
                                                refreshSettingsTarget = .group(group)
                                            }
                                            Button("恢复默认刷新") {
                                                Task {
                                                    _ = await state.resetGroupRefreshSettings(groupID: group.id)
                                                }
                                            }
                                        } label: {
                                            Image(systemName: "ellipsis.circle")
                                                .font(.system(size: 15, weight: .semibold))
                                                .foregroundStyle(.secondary)
                                        }
                                        .buttonStyle(.plain)
                                    }
                                }
                            }
                        }

                        sidebarSectionHeader("订阅")
                        if state.feeds.isEmpty {
                            Text("暂无订阅")
                                .font(.callout)
                                .foregroundStyle(.secondary)
                                .padding(.horizontal, 10)
                                .padding(.vertical, 8)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(cardBackground)
                                .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                        } else {
                            VStack(spacing: 6) {
                                ForEach(sortedFeeds) { feed in
                                    feedRow(feed)
                                        .id(feed.id)
                                }
                            }
                        }
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 4)
                }
                .scrollIndicators(.hidden)
                .onChange(of: state.selectedFeedID) { _, newValue in
                    guard let newValue else { return }
                    withAnimation(.spring(response: 0.32, dampingFraction: 0.9)) {
                        proxy.scrollTo(newValue, anchor: .center)
                    }
                }
                .task(id: state.selectedFeedID) {
                    guard let selectedFeedID = state.selectedFeedID else { return }
                    proxy.scrollTo(selectedFeedID, anchor: .center)
                }
            }
        }
        .background(shellBackground)
    }

    private var articleColumn: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                TextField("搜索当前订阅", text: $state.searchQuery)
                    .textFieldStyle(.roundedBorder)
#if os(macOS)
                    .focused($focusedField, equals: .search)
#endif
                    .onSubmit {
                        state.applySearch()
                    }

                Button {
                    Task { await state.refreshSelectedFeed() }
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .buttonStyle(.bordered)

                Menu {
                    Button("保存网页…") {
                        activeComposer = .bookmark
                    }
                    Button("新建随想…") {
                        activeComposer = .note
                    }
                    Button("全局通知设置…") {
                        notificationSettingsTarget = .global
                    }
                    Divider()
                    Button("刷新到期订阅") {
                        Task { await state.refreshDueFeeds() }
                    }
                    Button("导入 OPML…") {
                        isImportingOPML = true
                    }
                    Button("导出 OPML…") {
                        Task { await prepareOPMLExport() }
                    }
                } label: {
                    Image(systemName: "ellipsis.circle")
                }
                .buttonStyle(.bordered)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(.bar)

            scopeHeader

            if let errorMessage = state.errorMessage {
                Text(errorMessage)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 12)
                    .padding(.top, 8)
            }

            ScrollViewReader { proxy in
                List {
                    ForEach(groupedItems, id: \.title) { group in
                        Section {
                            ForEach(group.items) { item in
                                Button {
                                    state.didSelectItem(item.id)
                                } label: {
                                    ArticleRow(
                                        item: item,
                                        isSelected: state.selectedItemID == item.id
                                    )
                                }
                                .buttonStyle(.plain)
                                .listRowInsets(EdgeInsets(top: 4, leading: 10, bottom: 4, trailing: 10))
                                .listRowSeparator(.hidden)
                                .id(item.id)
                                .contextMenu {
                                    Button(item.isRead ? "Unread" : "Read") {
                                        state.toggleRead(item.id, current: item.isRead)
                                    }
                                    Button(item.isStarred ? "Unstar" : "Star") {
                                        state.toggleStarred(item.id, current: item.isStarred)
                                    }
                                    Button(item.isSavedForLater ? "Unsave" : "Later") {
                                        state.toggleSavedForLater(item.id, current: item.isSavedForLater)
                                    }
                                    Button(item.isArchived ? "Unarchive" : "Archive") {
                                        state.toggleArchived(item.id, current: item.isArchived)
                                    }
                                }
                            }
                        } header: {
                            Text(group.title)
                                .font(.caption.weight(.semibold))
                                .foregroundStyle(.secondary)
                                .textCase(nil)
                                .padding(.horizontal, 4)
                        }
                    }
                }
                .listStyle(.plain)
                .scrollContentBackground(.hidden)
                .background(ReaderShellColors.controlBackground)
                .onChange(of: state.selectedItemID) { _, newValue in
                    guard let newValue else { return }
                    withAnimation(.spring(response: 0.32, dampingFraction: 0.9)) {
                        proxy.scrollTo(newValue, anchor: .center)
                    }
                }
                .task(id: state.selectedItemID) {
                    guard let selectedItemID = state.selectedItemID else { return }
                    proxy.scrollTo(selectedItemID, anchor: .center)
                }
            }
        }
    }

    @ViewBuilder
    private var detailColumn: some View {
        if let detail = state.selectedItemDetail {
            VStack(alignment: .leading, spacing: 0) {
                VStack(alignment: .leading, spacing: 12) {
                    Text(detail.publishedAt.map(formatDetailDate) ?? "")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .textCase(.uppercase)

                    Text(detail.title)
                        .font(.system(size: 28, weight: .bold, design: .rounded))
                        .lineSpacing(1)

                    if let source = detail.canonicalURL?.host {
                        Text(source)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }

                    HStack(spacing: 8) {
                        ActionChip(title: detail.isRead ? "未读" : "已读", systemImage: "checkmark.circle") {
                            state.toggleRead(detail.id, current: detail.isRead)
                        }
                        ActionChip(title: detail.isStarred ? "取消星标" : "星标", systemImage: "star") {
                            state.toggleStarred(detail.id, current: detail.isStarred)
                        }
                        ActionChip(
                            title: detail.isSavedForLater ? "取消稍后读" : "稍后读",
                            systemImage: "bookmark"
                        ) {
                            state.toggleSavedForLater(detail.id, current: detail.isSavedForLater)
                        }
                        ActionChip(title: detail.isArchived ? "取消归档" : "归档", systemImage: "archivebox") {
                            state.toggleArchived(detail.id, current: detail.isArchived)
                        }
                        ActionChip(title: "抓取全文", systemImage: "text.alignleft") {
                            Task { await state.fetchFullTextForSelectedItem() }
                        }
                        Spacer()
                    }
                }
                .padding(.horizontal, 22)
                .padding(.top, 18)
                .padding(.bottom, 14)

                Divider()

                detailContent(for: detail)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                    .layoutPriority(1)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background(ReaderShellColors.textBackground)
            .animation(.easeOut(duration: 0.2), value: detail.id)
        } else {
            VStack(alignment: .leading, spacing: 12) {
                Text(state.feeds.isEmpty ? "还没有订阅" : emptyDetailTitle)
                    .font(.system(size: 32, weight: .bold, design: .rounded))
                Text(emptyDetailSubtitle)
                    .font(.body)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)

                if state.feeds.isEmpty {
                    Button("去添加订阅") {
                        focusedSubscriptionField()
                    }
                    .buttonStyle(.borderedProminent)
                }

                if let errorMessage = state.errorMessage, !errorMessage.isEmpty {
                    Text(errorMessage)
                        .foregroundStyle(.red)
                        .font(.caption)
                        .multilineTextAlignment(.leading)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
            .padding(.horizontal, 24)
            .padding(.vertical, 24)
            .background(ReaderShellColors.textBackground)
        }
    }

    @ViewBuilder
    private func detailContent(for detail: ArticleDetail) -> some View {
        if let contentHTML = detail.contentHTML, !contentHTML.isEmpty {
            ArticleHTMLView(
                html: contentHTML,
                baseURL: detail.canonicalURL,
                suppressLeadingHeading: true
            )
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        } else if let contentText = detail.contentText?.trimmingCharacters(in: .whitespacesAndNewlines),
                  !contentText.isEmpty
        {
            ScrollView {
                Text(contentText)
                    .font(.system(size: 17, weight: .regular, design: .default))
                    .lineSpacing(5)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 22)
                    .padding(.vertical, 18)
            }
        } else if let summary = detail.summary, !summary.isEmpty {
            ScrollView {
                Text(summary)
                    .font(.system(size: 17, weight: .regular, design: .default))
                    .lineSpacing(5)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 22)
                    .padding(.vertical, 18)
            }
        } else {
            Text("该条目暂无正文内容")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .padding(.horizontal, 22)
                .padding(.vertical, 18)
        }
    }

    private var emptyDetailTitle: String {
        guard let selection = state.selectedFeedID else {
            return "没有选中内容"
        }
        if selection == AppState.allItemsSelectionID {
            return "收件箱为空"
        }
        if selection == AppState.unreadSelectionID {
            return "没有未读内容"
        }
        if selection == AppState.starredSelectionID {
            return "还没有星标内容"
        }
        if selection == AppState.laterSelectionID {
            return "稍后读还空着"
        }
        if selection == AppState.notesSelectionID {
            return "随想还没有内容"
        }
        if selection == AppState.archiveSelectionID {
            return "归档里还没有内容"
        }
        if selection.hasPrefix(AppState.groupSelectionPrefix) {
            return "这个分类里还没有内容"
        }
        return "没有选中内容"
    }

    private var emptyDetailSubtitle: String {
        guard let selection = state.selectedFeedID else {
            return "从左侧选择一个条目，或者搜索内容。"
        }
        if state.feeds.isEmpty {
            return "先添加一个网站或 Feed，InfoMatrix 会自动发现可订阅源。"
        }
        if selection == AppState.allItemsSelectionID {
            return "左侧可以切换未读、星标、稍后读和归档，也可以直接搜索当前收件箱。"
        }
        if selection == AppState.unreadSelectionID {
            return "你可以先刷新订阅，或者切到全部收件箱查看已读内容。"
        }
        if selection == AppState.starredSelectionID {
            return "把重要内容加星后，会自动汇总到这里。"
        }
        if selection == AppState.laterSelectionID {
            return "把想稍后处理的内容存到这里。"
        }
        if selection == AppState.notesSelectionID {
            return "在这里快速记录想法、摘录和待办。"
        }
        if selection == AppState.archiveSelectionID {
            return "归档内容会保留在这里，随时可回看。"
        }
        if selection.hasPrefix(AppState.groupSelectionPrefix) {
            return "这个分类暂时没有条目。"
        }
        return "从左侧选择一个条目，或者搜索内容。"
    }

    private func focusedSubscriptionField() {
#if os(macOS)
        focusedField = .subscription
#endif
    }

    private func requestInitialFocus() {
#if os(macOS)
        if focusedField == nil {
            focusedField = .subscription
        }
        DispatchQueue.main.async {
            if focusedField == nil {
                focusedField = .subscription
            }
        }
#endif
    }

    private var groupedItems: [TimelineGroup] {
        let grouped = Dictionary(grouping: state.items) { item in
            timelineBucket(for: item.publishedAt)
        }
        return grouped
            .keys
            .sorted { $0.date > $1.date }
            .map { key in
                TimelineGroup(
                    title: key.title,
                    items: grouped[key, default: []].sorted {
                        let lhs = publishedDate(for: $0.publishedAt)
                        let rhs = publishedDate(for: $1.publishedAt)
                        if lhs != rhs {
                            return lhs > rhs
                        }
                        return $0.title.localizedCaseInsensitiveCompare($1.title) == .orderedAscending
                    }
                )
            }
    }

    private func count(for selectionID: String) -> Int? {
        switch selectionID {
        case AppState.allItemsSelectionID:
            return state.allItemsCount
        case AppState.unreadSelectionID:
            return state.unreadItemsCount
        case AppState.starredSelectionID:
            return state.starredItemsCount
        case AppState.laterSelectionID:
            return state.laterItemsCount
        case AppState.notesSelectionID:
            return state.notesItemsCount
        case AppState.archiveSelectionID:
            return state.archiveItemsCount
        default:
            return nil
        }
    }

    private func sidebarSectionHeader(_ title: String) -> some View {
        Text(title)
            .font(.caption2.weight(.semibold))
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 4)
    }

    private var subscriptionComposerCard: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Text("添加订阅")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                Spacer()
                Button {
                    activeComposer = .bookmark
                } label: {
                    Label("网页", systemImage: "bookmark")
                }
                .buttonStyle(.bordered)
                .controlSize(.mini)
                Button {
                    activeComposer = .note
                } label: {
                    Label("随想", systemImage: "square.and.pencil")
                }
                .buttonStyle(.bordered)
                .controlSize(.mini)
            }

            HStack(spacing: 8) {
                TextField("输入网站或 Feed URL", text: $subscriptionInput)
                    .textFieldStyle(.roundedBorder)
#if os(macOS)
                    .focused($focusedField, equals: .subscription)
#endif
                    .onSubmit {
                        submitSubscription()
                    }

                Button {
                    submitSubscription()
                } label: {
                    Text("确认")
                }
                .buttonStyle(.borderedProminent)
                .disabled(state.isLoading)
            }
        }
        .padding(10)
        .background(cardBackground)
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(Color.secondary.opacity(0.10), lineWidth: 1)
        )
    }

    private var syncStatusCard: some View {
        Group {
            if let syncStatus = state.syncStatus {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 8) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("iCloud 同步")
                                .font(.caption.weight(.semibold))
                                .foregroundStyle(.secondary)
                            Text(accountStateLabel(for: syncStatus.accountState))
                                .font(.headline)
                        }

                        Spacer(minLength: 0)

                        Toggle(
                            "",
                            isOn: Binding(
                                get: { syncStatus.enabled },
                                set: { state.setCloudKitSyncEnabled($0) }
                            )
                        )
                        .labelsHidden()
                    }

                    Text("待同步 \(syncStatus.pendingLocalEventCount) 条")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    if let lastSyncAt = syncStatus.lastSyncAt {
                        Text("上次同步 \(lastSyncAt.formatted(date: .abbreviated, time: .shortened))")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }

                    HStack(spacing: 8) {
                        Button {
                            Task { await state.syncNow() }
                        } label: {
                            Text(syncStatus.isSyncing ? "同步中…" : "立即同步")
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(syncStatus.isSyncing || !syncStatus.enabled)

                        Button {
                            Task { await state.refreshSyncStatus() }
                        } label: {
                            Text("刷新")
                        }
                        .buttonStyle(.bordered)
                    }

                    if let lastError = syncStatus.lastErrorMessage, !lastError.isEmpty {
                        Text(lastError)
                            .font(.caption2)
                            .foregroundStyle(.red)
                            .lineLimit(3)
                    }
                }
                .padding(10)
                .background(cardBackground)
                .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                        .stroke(Color.secondary.opacity(0.10), lineWidth: 1)
                )
            }
        }
    }

    private var selectedGroupID: String? {
        guard let selection = state.selectedFeedID,
              selection.hasPrefix(AppState.groupSelectionPrefix)
        else {
            return nil
        }
        return String(selection.dropFirst(AppState.groupSelectionPrefix.count))
    }

    private func accountStateLabel(for accountState: CloudKitSyncAccountState) -> String {
        switch accountState {
        case .available:
            return "iCloud 可用"
        case .noAccount:
            return "未登录 iCloud"
        case .restricted:
            return "iCloud 受限"
        case .temporarilyUnavailable:
            return "iCloud 暂不可用"
        case .couldNotDetermine:
            return "状态未知"
        }
    }

    private func createSidebarGroup() {
        let trimmed = newGroupName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        Task { @MainActor in
            if let created = await state.createGroup(trimmed) {
                newGroupName = ""
                state.selectGroup(created.id)
                focusedField = .subscription
            }
        }
    }

    private var cardBackground: some ShapeStyle {
        LinearGradient(
            colors: [
                ReaderShellColors.controlBackground.opacity(colorScheme == .dark ? 0.92 : 0.96),
                Color.accentColor.opacity(colorScheme == .dark ? 0.08 : 0.06)
            ],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
    }

    private var shellBackground: some View {
        LinearGradient(
            colors: [
                ReaderShellColors.windowBackground,
                ReaderShellColors.controlBackground.opacity(colorScheme == .dark ? 0.88 : 0.68)
            ],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
    }

    private var scopeHeader: some View {
        let screen = state.readerScreenState
        return VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                Text(screen.headerTitle)
                    .font(.headline.weight(.semibold))
                Text(screen.headerSubtitle)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                if state.isLoading {
                    ProgressView()
                        .controlSize(.small)
                }
            }

            let search = state.searchQuery.trimmingCharacters(in: .whitespacesAndNewlines)
            if !search.isEmpty {
                HStack(spacing: 8) {
                    Image(systemName: "magnifyingglass")
                        .font(.caption2)
                    Text(search)
                        .font(.caption2)
                        .lineLimit(1)
                    Spacer()
                    Button("清除") {
                        state.searchQuery = ""
                        state.applySearch()
                    }
                    .buttonStyle(.plain)
                    .font(.caption2.weight(.medium))
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(Color.accentColor.opacity(0.10))
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .background(ReaderShellColors.controlBackground)
    }

    private func isSpecialSelection(_ feedID: String) -> Bool {
        feedID == AppState.allItemsSelectionID
            || feedID == AppState.unreadSelectionID
            || feedID == AppState.starredSelectionID
            || feedID == AppState.laterSelectionID
            || feedID == AppState.notesSelectionID
            || feedID == AppState.archiveSelectionID
            || feedID.hasPrefix(AppState.groupSelectionPrefix)
    }

    private func formatDetailDate(_ value: String) -> String {
        guard let date = ISO8601DateFormatter().date(from: value) else { return value }
        return date.formatted(date: .abbreviated, time: .shortened)
    }

    private func beginEdit(_ feed: Feed) {
        editingFeed = feed
        editTitle = feed.title
        editGroupID = feed.groups.first?.id ?? ""
        newGroupName = ""
    }

    @ViewBuilder
    private func feedRow(_ feed: Feed) -> some View {
        Button {
            state.didSelectFeed(feed.id)
        } label: {
            SidebarFeedRow(feed: feed, isSelected: state.selectedFeedID == feed.id)
        }
                .buttonStyle(.plain)
            .contextMenu {
                Button {
                    Task { await state.refresh(feedID: feed.id) }
                } label: {
                    Label("刷新订阅", systemImage: "arrow.clockwise")
                }

                Button {
                    refreshSettingsTarget = .feed(feed)
                } label: {
                    Label("自动刷新…", systemImage: "clock.arrow.circlepath")
                }

                Button {
                    Task { _ = await state.resetFeedRefreshSettings(feedID: feed.id) }
                } label: {
                    Label("恢复默认刷新", systemImage: "arrow.uturn.backward")
                }

                Button {
                    Task { await state.setFeedAutoFullText(feed.id, enabled: !feed.autoFullText) }
                } label: {
                    Label(
                        feed.autoFullText ? "关闭自动抓全文" : "开启自动抓全文",
                        systemImage: feed.autoFullText ? "checkmark.circle.fill" : "checkmark.circle"
                    )
                }

                Button {
                    notificationSettingsTarget = .feed(feed)
                } label: {
                    Label("通知设置…", systemImage: "bell.badge")
                }

                Divider()

                if let siteURL = feed.siteURL {
                    Button {
                        openURL(siteURL)
                    } label: {
                        Label("打开网站", systemImage: "globe")
                    }
                }

                Button {
                    openURL(feed.feedURL)
                } label: {
                    Label("打开 Feed URL", systemImage: "dot.radiowaves.left.and.right")
                }

#if os(macOS)
                Button {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(feed.feedURL.absoluteString, forType: .string)
                } label: {
                    Label("复制 Feed URL", systemImage: "doc.on.doc")
                }
#endif

                Divider()

                Button {
                    beginEdit(feed)
                } label: {
                    Label("编辑订阅源", systemImage: "pencil")
                }

                Divider()

                Button(role: .destructive) {
                    Task { await state.removeFeed(feed.id) }
                } label: {
                    Label("删除订阅", systemImage: "trash")
                }
            }
    }

    private func feeds(in group: FeedGroup) -> [Feed] {
        state.feeds.filter { feed in
            feed.groups.contains(where: { $0.id == group.id })
        }
    }

    private var sortedFeeds: [Feed] {
        state.feeds.sorted { lhs, rhs in
            let lhsGroup = lhs.groups.first?.name ?? ""
            let rhsGroup = rhs.groups.first?.name ?? ""
            if lhsGroup != rhsGroup {
                return lhsGroup.localizedCaseInsensitiveCompare(rhsGroup) == .orderedAscending
            }
            return lhs.title.localizedCaseInsensitiveCompare(rhs.title) == .orderedAscending
        }
    }

    private func feeds(inGroupID groupID: String) -> [Feed] {
        state.feeds.filter { feed in
            feed.groups.contains(where: { $0.id == groupID })
        }
    }

    private func timelineBucket(for publishedAt: String?) -> TimelineBucketKey {
        guard
            let publishedAt,
            let date = ISO8601DateFormatter().date(from: publishedAt)
        else {
            return TimelineBucketKey(title: "NO DATE", date: .distantPast)
        }
        let calendar = Calendar.current
        if calendar.isDateInToday(date) {
            return TimelineBucketKey(title: "TODAY", date: calendar.startOfDay(for: date))
        }
        if calendar.isDateInYesterday(date) {
            return TimelineBucketKey(title: "YESTERDAY", date: calendar.startOfDay(for: date))
        }
        let text = date.formatted(.dateTime.weekday(.wide).month(.abbreviated).day().year())
            .uppercased()
        return TimelineBucketKey(title: text, date: calendar.startOfDay(for: date))
    }

    private func publishedDate(for publishedAt: String?) -> Date {
        guard let publishedAt, let date = ISO8601DateFormatter().date(from: publishedAt) else {
            return .distantPast
        }
        return date
    }

    private func submitSubscription() {
        Task {
            _ = await submitSubscription(input: subscriptionInput)
        }
    }

    @discardableResult
    @MainActor
    func submitSubscription(input: String) async -> Bool {
        return await state.addSubscription(input: input)
    }

    private func handleOPMLImport(_ result: Result<[URL], Error>) {
        guard case let .success(urls) = result, let url = urls.first else {
            if case let .failure(error) = result {
                state.errorMessage = "读取 OPML 失败: \(error.localizedDescription)"
            }
            return
        }

        Task {
            do {
                let didAccessScoped = url.startAccessingSecurityScopedResource()
                defer {
                    if didAccessScoped {
                        url.stopAccessingSecurityScopedResource()
                    }
                }
                let data = try Data(contentsOf: url)
                guard let xml = String(data: data, encoding: .utf8) else {
                    state.errorMessage = "OPML 文件编码不支持（需要 UTF-8）"
                    return
                }
                _ = await state.importOPML(opmlXML: xml)
            } catch {
                state.errorMessage = "读取 OPML 失败: \(error.localizedDescription)"
            }
        }
    }

    private func prepareOPMLExport() async {
        guard let xml = await state.exportOPML() else {
            return
        }
        opmlExportDocument = OPMLTextDocument(text: xml)
        isExportingOPML = true
    }

    private var opmlContentTypes: [UTType] {
        var types: [UTType] = [.xml, .plainText]
        if let opml = UTType(filenameExtension: "opml") {
            types.insert(opml, at: 0)
        }
        return types
    }

    private var pendingDiscoveryBinding: Binding<PendingDiscoverySelection?> {
        Binding(
            get: { state.pendingDiscoverySelection },
            set: { value in
                if value == nil {
                    state.dismissDiscoverySelection()
                }
            }
        )
    }
}

private struct OPMLTextDocument: FileDocument {
    static var readableContentTypes: [UTType] {
        [.xml, .plainText]
    }

    var text: String

    init(text: String) {
        self.text = text
    }

    init(configuration: ReadConfiguration) throws {
        guard let data = configuration.file.regularFileContents,
              let value = String(data: data, encoding: .utf8)
        else {
            throw CocoaError(.fileReadCorruptFile)
        }
        text = value
    }

    func fileWrapper(configuration _: WriteConfiguration) throws -> FileWrapper {
        let data = text.data(using: .utf8) ?? Data()
        return FileWrapper(regularFileWithContents: data)
    }
}

private struct SidebarSummaryRow: View {
    let icon: String
    let title: String
    let count: Int?
    let isSelected: Bool

    var body: some View {
        HStack(spacing: 8) {
            ZStack {
                Circle()
                    .fill(isSelected ? Color.accentColor.opacity(0.18) : Color.secondary.opacity(0.12))
                Image(systemName: icon)
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundStyle(isSelected ? Color.accentColor : Color.secondary)
            }
            .frame(width: 20, height: 20)

            Text(title)
                .font(.system(size: 12.5, weight: .medium))
                .foregroundStyle(.primary)

            Spacer()

            if let count {
                Text("\(count)")
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(isSelected ? Color.accentColor : Color.secondary)
                    .padding(.horizontal, 5)
                    .padding(.vertical, 0)
                    .background(
                        Capsule().fill(isSelected ? Color.accentColor.opacity(0.14) : Color.secondary.opacity(0.10))
                    )
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(isSelected ? Color.accentColor.opacity(0.10) : ReaderShellColors.controlBackground.opacity(0.88))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .stroke(isSelected ? Color.accentColor.opacity(0.28) : Color.secondary.opacity(0.08), lineWidth: 1)
        )
    }
}

private struct SidebarFeedRow: View {
    let feed: Feed
    let isSelected: Bool

    var body: some View {
        HStack(spacing: 10) {
            FeedIconView(feed: feed)
            VStack(alignment: .leading, spacing: 3) {
                Text(feed.title)
                    .font(.system(size: 12.5, weight: .medium))
                    .lineLimit(1)
                Text(subtitle)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Spacer()
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 5)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(isSelected ? Color.accentColor.opacity(0.10) : ReaderShellColors.controlBackground.opacity(0.84))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .stroke(isSelected ? Color.accentColor.opacity(0.28) : Color.secondary.opacity(0.08), lineWidth: 1)
        )
    }

    private var subtitle: String {
        if let group = feed.groups.first?.name, !group.isEmpty {
            return group
        }
        return feed.feedURL.host ?? feed.feedURL.absoluteString
    }
}

private struct FeedIconView: View {
    let feed: Feed

    var body: some View {
        if let iconURL = feed.iconURL {
            AsyncImage(url: iconURL) { image in
                image.resizable().scaledToFill()
            } placeholder: {
                monogram
            }
            .frame(width: 16, height: 16)
            .clipShape(RoundedRectangle(cornerRadius: 4))
        } else {
            monogram
        }
    }

    private var monogram: some View {
        RoundedRectangle(cornerRadius: 4)
            .fill(Color.accentColor.opacity(0.2))
            .frame(width: 16, height: 16)
            .overlay(
                Text(String(feed.title.prefix(1)).uppercased())
                    .font(.caption2)
                    .foregroundStyle(Color.accentColor)
            )
    }
}

private struct ArticleRow: View {
    let item: ArticleItem
    let isSelected: Bool

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            ZStack {
                Circle()
                    .fill(isSelected ? Color.accentColor.opacity(0.18) : Color.secondary.opacity(0.10))
                Image(systemName: itemIconName)
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(item.isStarred ? .yellow : (isSelected ? Color.accentColor : .secondary))
            }
            .frame(width: 22, height: 22)

            VStack(alignment: .leading, spacing: 4) {
                HStack(alignment: .top, spacing: 8) {
                    Text(item.title)
                        .font(.system(size: 13.5, weight: item.isRead ? .regular : .semibold))
                        .lineLimit(2)
                        .foregroundStyle(.primary)

                    Spacer(minLength: 8)

                    if let publishedAt = item.publishedAt {
                        Text(shortDate(publishedAt))
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }

                if let preview = snippetText {
                    Text(preview)
                        .font(.system(size: 11.5))
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }

                HStack(spacing: 6) {
                    if item.isSavedForLater {
                        Label("Later", systemImage: "bookmark.fill")
                            .font(.caption2)
                            .foregroundStyle(.green)
                    }
                    if item.isArchived {
                        Label("Archive", systemImage: "archivebox.fill")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 5)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(isSelected ? Color.accentColor.opacity(0.08) : Color.clear)
        )
    }

    private var snippetText: String? {
        let preview = item.summaryPreview?.trimmingCharacters(in: .whitespacesAndNewlines)
        if let preview, !preview.isEmpty {
            return preview
        }
        if let host = item.canonicalURL?.host {
            return host
        }
        if let host = item.sourceURL?.host {
            return host
        }
        return item.kind.capitalized
    }

    private var itemIconName: String {
        if item.isStarred {
            return "star.fill"
        }
        switch item.kind.lowercased() {
        case "bookmark":
            return "bookmark.fill"
        case "note":
            return "note.text"
        case "quote":
            return "quote.bubble"
        default:
            return "doc.text"
        }
    }

    private func shortDate(_ value: String) -> String {
        guard let date = ISO8601DateFormatter().date(from: value) else { return value }
        return date.formatted(date: .abbreviated, time: .shortened)
    }
}

private struct ActionChip: View {
    let title: String
    let systemImage: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Label(title, systemImage: systemImage)
                .font(.caption)
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(Color.secondary.opacity(0.1))
                .clipShape(Capsule())
        }
        .buttonStyle(.plain)
    }
}

private struct FeedEditSheet: View {
    let feed: Feed
    let groups: [FeedGroup]
    @State var initialTitle: String
    @State var initialGroupID: String
    @State var newGroupName: String
    let onCancel: () -> Void
    let onSave: (_ title: String, _ groupID: String, _ createGroupName: String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("编辑订阅源")
                .font(.title3)
                .fontWeight(.semibold)
            Text(feed.feedURL.absoluteString)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(2)

            TextField("标题", text: $initialTitle)

            Picker("分组", selection: $initialGroupID) {
                Text("无分组").tag("")
                ForEach(groups) { group in
                    Text(group.name).tag(group.id)
                }
            }

            TextField("新建分组（可选）", text: $newGroupName)

            HStack {
                Spacer()
                Button("取消") {
                    onCancel()
                }
                Button("保存") {
                    onSave(initialTitle, initialGroupID, newGroupName)
                }
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(20)
        .frame(width: 460)
    }
}

private struct DiscoverySelectionSheet: View {
    let selection: PendingDiscoverySelection
    let onCancel: () -> Void
    let onSubscribe: (DiscoverFeed) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("选择订阅源")
                .font(.title3)
                .fontWeight(.semibold)
            if let siteTitle = selection.siteTitle, !siteTitle.isEmpty {
                Text(siteTitle)
                    .font(.headline)
            }
            Text(selection.normalizedSiteURL.absoluteString)
                .font(.caption)
                .foregroundStyle(.secondary)

            List(selection.discoveredFeeds) { candidate in
                HStack(alignment: .top, spacing: 10) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(discoveryTitle(for: candidate))
                            .font(.body.weight(.semibold))
                        Text(candidate.url.absoluteString)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(2)
                        HStack(spacing: 8) {
                            Text(candidate.feedType.uppercased())
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                            Text(candidate.source ?? "unknown")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                            Text("score \(candidate.score ?? Int((candidate.confidence * 120.0) - 30.0))")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                            Text(String(format: "%.2f", candidate.confidence))
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                    Spacer()
                    Button("订阅") {
                        onSubscribe(candidate)
                    }
                    .buttonStyle(.borderedProminent)
                }
                .padding(.vertical, 4)
            }
            .frame(minHeight: 220)

            if !selection.warnings.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("诊断")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                    ForEach(selection.warnings, id: \.self) { warning in
                        Text("• \(warning)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }

            HStack {
                Spacer()
                Button("取消") {
                    onCancel()
                }
            }
        }
        .padding(20)
        .frame(width: 620, height: 460)
    }

    private func discoveryTitle(for candidate: DiscoverFeed) -> String {
        let trimmed = candidate.title?.trimmingCharacters(in: .whitespacesAndNewlines)
        if let trimmed, !trimmed.isEmpty {
            return trimmed
        }
        return candidate.url.host ?? candidate.url.absoluteString
    }
}

private struct EntryComposerSheet: View {
    let kind: ReaderShellView.EntryComposerKind
    let onCancel: () -> Void
    let onSaveBookmark: (_ url: String, _ title: String, _ note: String?) -> Void
    let onSaveNote: (_ title: String, _ body: String) -> Void

    @State private var url: String = ""
    @State private var title: String = ""
    @State private var note: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            VStack(alignment: .leading, spacing: 6) {
                Text(sheetTitle)
                    .font(.title2.weight(.semibold))
                Text(sheetSubtitle)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            VStack(alignment: .leading, spacing: 14) {
                if kind == .bookmark {
                    labeledField(
                        label: "网页地址",
                        placeholder: "输入网站或网页 URL",
                        text: $url
                    )

                    labeledField(
                        label: "标题",
                        placeholder: "留空将自动抓取网页标题",
                        text: $title
                    )

                    labeledTextEditor(
                        label: "备注",
                        placeholder: "可以附上一句备注，保存后仍会自动抓取网页正文",
                        text: $note,
                        minimumHeight: 180
                    )
                } else {
                    labeledField(
                        label: "标题",
                        placeholder: "给随想起一个标题",
                        text: $title
                    )

                    labeledTextEditor(
                        label: "内容",
                        placeholder: "写下想法、摘录或待办",
                        text: $note,
                        minimumHeight: 240
                    )
                }
            }
            .padding(18)
            .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 20, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 20, style: .continuous)
                    .stroke(Color.secondary.opacity(0.14), lineWidth: 1)
            )

            HStack(spacing: 10) {
                if kind == .bookmark {
                    Text("标题可以不填，保存后会自动抓取网页标题和正文。")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                } else {
                    Text("随想会直接保存到本地。")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer()

                Button("取消", role: .cancel) {
                    onCancel()
                }
                .buttonStyle(.bordered)

                Button("保存") {
                    if kind == .bookmark {
                        onSaveBookmark(
                            url,
                            title,
                            note.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : note
                        )
                    } else {
                        onSaveNote(title, note)
                    }
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
                .disabled(isSaveDisabled)
            }
        }
        .padding(24)
        .frame(width: 700, height: kind == .bookmark ? 520 : 490)
    }

    private var sheetTitle: String {
        switch kind {
        case .bookmark:
            return "保存网页"
        case .note:
            return "新建随想"
        }
    }

    private var sheetSubtitle: String {
        switch kind {
        case .bookmark:
            return "保存时会自动抓取网页标题与正文，标题可以留空。"
        case .note:
            return "快速记录想法、摘录或待办。"
        }
    }

    private var isSaveDisabled: Bool {
        switch kind {
        case .bookmark:
            return url.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                || title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        case .note:
            return note.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
    }

    @ViewBuilder
    private func labeledField(label: String, placeholder: String, text: Binding<String>) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
            TextField(placeholder, text: text)
                .textFieldStyle(.plain)
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
                .background(
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .fill(Color.secondary.opacity(0.08))
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .stroke(Color.secondary.opacity(0.12), lineWidth: 1)
                )
        }
    }

    @ViewBuilder
    private func labeledTextEditor(
        label: String,
        placeholder: String,
        text: Binding<String>,
        minimumHeight: CGFloat
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
            ZStack(alignment: .topLeading) {
                if text.wrappedValue.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    Text(placeholder)
                        .foregroundStyle(.secondary.opacity(0.7))
                        .padding(.horizontal, 14)
                        .padding(.vertical, 14)
                        .allowsHitTesting(false)
                }
                TextEditor(text: text)
                    .scrollContentBackground(.hidden)
                    .padding(10)
            }
            .frame(minHeight: minimumHeight)
            .background(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .fill(Color.secondary.opacity(0.06))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .stroke(Color.secondary.opacity(0.12), lineWidth: 1)
            )
        }
    }
}

#if canImport(WebKit)
private struct ArticleHTMLView: View {
    let html: String
    let baseURL: URL?
    let suppressLeadingHeading: Bool

    var body: some View {
        EmbeddedWebView(
            html: html,
            baseURL: baseURL,
            suppressLeadingHeading: suppressLeadingHeading
        )
            .frame(maxWidth: .infinity)
            .frame(maxHeight: .infinity, alignment: .topLeading)
    }
}

#if os(macOS)
private struct EmbeddedWebView: NSViewRepresentable {
    let html: String
    let baseURL: URL?
    let suppressLeadingHeading: Bool

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.defaultWebpagePreferences.allowsContentJavaScript = false
        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.setValue(false, forKey: "drawsBackground")
        webView.navigationDelegate = context.coordinator
        return webView
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        if context.coordinator.loadIfNeeded(
            webView,
            html: html,
            baseURL: baseURL,
            suppressLeadingHeading: suppressLeadingHeading
        ) {
            webView.loadHTMLString(
                htmlTemplate(html, suppressLeadingHeading: suppressLeadingHeading),
                baseURL: baseURL
            )
        }
    }

    final class Coordinator: NSObject, WKNavigationDelegate {
        private var lastLoadedHTML: String?
        private var lastLoadedBaseURLString: String?
        private var lastLoadedSuppressLeadingHeading: Bool?

        func loadIfNeeded(
            _ webView: WKWebView,
            html: String,
            baseURL: URL?,
            suppressLeadingHeading: Bool
        ) -> Bool {
            let baseURLString = baseURL?.absoluteString
            guard lastLoadedHTML != html
                || lastLoadedBaseURLString != baseURLString
                || lastLoadedSuppressLeadingHeading != suppressLeadingHeading
            else {
                return false
            }
            lastLoadedHTML = html
            lastLoadedBaseURLString = baseURLString
            lastLoadedSuppressLeadingHeading = suppressLeadingHeading
            return true
        }
    }
}
#else
private struct EmbeddedWebView: UIViewRepresentable {
    let html: String
    let baseURL: URL?
    let suppressLeadingHeading: Bool

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeUIView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.defaultWebpagePreferences.allowsContentJavaScript = false
        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.isOpaque = false
        webView.scrollView.isScrollEnabled = true
        webView.navigationDelegate = context.coordinator
        return webView
    }

    func updateUIView(_ webView: WKWebView, context: Context) {
        if context.coordinator.loadIfNeeded(
            webView,
            html: html,
            baseURL: baseURL,
            suppressLeadingHeading: suppressLeadingHeading
        ) {
            webView.loadHTMLString(
                htmlTemplate(html, suppressLeadingHeading: suppressLeadingHeading),
                baseURL: baseURL
            )
        }
    }

    final class Coordinator: NSObject, WKNavigationDelegate {
        private var lastLoadedHTML: String?
        private var lastLoadedBaseURLString: String?
        private var lastLoadedSuppressLeadingHeading: Bool?

        func loadIfNeeded(
            _ webView: WKWebView,
            html: String,
            baseURL: URL?,
            suppressLeadingHeading: Bool
        ) -> Bool {
            let baseURLString = baseURL?.absoluteString
            guard lastLoadedHTML != html
                || lastLoadedBaseURLString != baseURLString
                || lastLoadedSuppressLeadingHeading != suppressLeadingHeading
            else {
                return false
            }
            lastLoadedHTML = html
            lastLoadedBaseURLString = baseURLString
            lastLoadedSuppressLeadingHeading = suppressLeadingHeading
            return true
        }
    }
}
#endif

private func htmlTemplate(_ content: String, suppressLeadingHeading: Bool = false) -> String {
    let headingRule = suppressLeadingHeading ? "        body > h1:first-child { display: none; }\n" : ""
    return """
    <html>
    <head>
      <meta charset="utf-8" />
      <meta name="viewport" content="width=device-width, initial-scale=1.0" />
      <style>
        :root {
          color-scheme: light dark;
          --bg: #f4f4f5;
          --fg: #1f1f1f;
          --muted: #5c5c5f;
          --border: rgba(0, 0, 0, 0.12);
          --code-bg: rgba(0, 0, 0, 0.05);
          --link: #0a66c2;
        }
        @media (prefers-color-scheme: dark) {
          :root {
            --bg: #121212;
            --fg: #ececec;
            --muted: #a7a7ab;
            --border: rgba(255, 255, 255, 0.16);
            --code-bg: rgba(255, 255, 255, 0.06);
            --link: #7bb3ff;
          }
        }
        html, body { margin: 0; padding: 0; background: transparent; }
        body {
          font-family: -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif;
          font-size: 16px;
          line-height: 1.72;
          letter-spacing: 0;
          word-spacing: 0;
          color: var(--fg);
          max-width: 840px;
          margin: 0 auto;
          padding: 28px 24px 40px;
          overflow-wrap: anywhere;
          -webkit-font-smoothing: antialiased;
          text-rendering: optimizeLegibility;
          background: var(--bg);
        }
        body, body * { color: inherit !important; }
    """ + headingRule + """
        p, ul, ol, blockquote, pre, table, figure, img, h1, h2, h3, h4, h5, h6 {
          max-width: 100%;
        }
        p, ul, ol, blockquote, pre, table, figure { margin: 0 0 1em; }
        img, video, iframe, canvas, svg {
          display: block;
          max-width: 100%;
          max-height: 56vh;
          height: auto;
          margin: 1.15em auto;
          object-fit: contain;
          border-radius: 14px;
        }
        img[alt*="emoji"], img.emoji { display: inline-block; max-height: 1.3em; margin: 0; vertical-align: middle; }
        pre, code {
          font-family: Menlo, Monaco, Consolas, monospace;
          white-space: pre-wrap;
          word-break: break-word;
        }
        pre {
          background: var(--code-bg);
          padding: 12px 14px;
          border-radius: 10px;
          overflow-x: auto;
          border: 1px solid var(--border);
        }
        code {
          padding: 0.08em 0.35em;
          border-radius: 6px;
          background: var(--code-bg);
        }
        pre code {
          padding: 0;
          background: transparent;
        }
        blockquote {
          border-left: 3px solid var(--border);
          margin-left: 0;
          padding-left: 14px;
          color: var(--muted) !important;
          background: rgba(127, 127, 127, 0.06);
          border-radius: 12px;
          padding-top: 12px;
          padding-bottom: 12px;
        }
        a { color: var(--link) !important; text-decoration: none; }
        a:hover { text-decoration: underline; }
        hr {
          border: 0;
          border-top: 1px solid var(--border);
          margin: 1.25em 0;
        }
        table { border-collapse: collapse; display: block; overflow-x: auto; }
        th, td { border: 1px solid var(--border); padding: 0.45em 0.6em; }
      </style>
    </head>
    <body>\(content)</body>
    </html>
    """
}
#else
private struct ArticleHTMLView: View {
    let html: String
    let baseURL: URL?
    let suppressLeadingHeading: Bool
    var body: some View {
        Text(html)
            .textSelection(.enabled)
    }
}
#endif

private struct RefreshSettingsSheet: View {
    let target: ReaderShellView.RefreshSettingsTarget
    @ObservedObject var state: AppState
    let onCancel: () -> Void

    @State private var settings = RefreshSettings()
    @State private var isLoading = true

    var body: some View {
        NavigationStack {
            Group {
                if isLoading {
                    ProgressView("加载自动刷新设置…")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    Form {
                        Section {
                            Toggle("启用自动刷新", isOn: $settings.enabled)
                            Stepper(
                                "刷新间隔 \(settings.intervalMinutes) 分钟",
                                value: $settings.intervalMinutes,
                                in: 5...1440,
                                step: 5
                            )
                        } footer: {
                            Text("自动刷新只影响后台轮询调度，不会阻止手动刷新。")
                        }
                    }
                }
            }
            .navigationTitle(title)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("取消", action: onCancel)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("保存") {
                        Task { await save() }
                    }
                    .disabled(isLoading)
                }
            }
        }
        .frame(minWidth: 420, minHeight: 240)
        .task {
            await load()
        }
    }

    private var title: String {
        switch target {
        case .feed(let feed):
            return "\(feed.title) 自动刷新"
        case .group(let group):
            return "\(group.name) 自动刷新"
        }
    }

    private func load() async {
        switch target {
        case .feed(let feed):
            if let loaded = await state.loadFeedRefreshSettings(feedID: feed.id) {
                settings = loaded
            }
        case .group(let group):
            if let loaded = await state.loadGroupRefreshSettings(groupID: group.id) {
                settings = loaded
            }
        }
        isLoading = false
    }

    private func save() async {
        switch target {
        case .feed(let feed):
            _ = await state.saveFeedRefreshSettings(feedID: feed.id, settings: settings)
        case .group(let group):
            _ = await state.saveGroupRefreshSettings(groupID: group.id, settings: settings)
        }
        onCancel()
    }
}

private struct NotificationSettingsSheet: View {
    let target: ReaderShellView.NotificationSettingsTarget
    @ObservedObject var state: AppState
    let onCancel: () -> Void

    @State private var globalSettings = Self.defaultGlobalSettings()
    @State private var feedSettings = Self.defaultFeedSettings()
    @State private var isLoading = true

    var body: some View {
        NavigationStack {
            Group {
                if isLoading {
                    ProgressView("加载通知设置…")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    Form {
                        switch target {
                        case .global:
                            Section("全局刷新") {
                                Toggle(
                                    "启用后台刷新",
                                    isOn: $globalSettings.backgroundRefreshEnabled
                                )
                                Stepper(
                                    "刷新间隔 \(globalSettings.backgroundRefreshIntervalMinutes) 分钟",
                                    value: $globalSettings.backgroundRefreshIntervalMinutes,
                                    in: 5...240,
                                    step: 5
                                )
                            }

                            Section("全局摘要") {
                                Toggle("启用摘要", isOn: $globalSettings.digestPolicy.enabled)
                                Stepper(
                                    "摘要间隔 \(globalSettings.digestPolicy.intervalMinutes) 分钟",
                                    value: $globalSettings.digestPolicy.intervalMinutes,
                                    in: 15...1440,
                                    step: 15
                                )
                                Stepper(
                                    "每次最多 \(globalSettings.digestPolicy.maxItems) 条",
                                    value: $globalSettings.digestPolicy.maxItems,
                                    in: 3...100,
                                    step: 1
                                )
                            }

                            NotificationSettingsForm(
                                title: "默认订阅通知",
                                settings: $globalSettings.defaultFeedSettings
                            )
                        case .feed:
                            NotificationSettingsForm(
                                title: "订阅通知",
                                settings: $feedSettings
                            )
                        }
                    }
                }
            }
            .navigationTitle(title)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("取消", action: onCancel)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("保存") {
                        Task { await save() }
                    }
                    .disabled(isLoading)
                }
            }
        }
        .frame(minWidth: 520, minHeight: 620)
        .task {
            await load()
        }
    }

    private var title: String {
        switch target {
        case .global:
            return "全局通知设置"
        case .feed(let feed):
            return "\(feed.title) 通知设置"
        }
    }

    private func load() async {
        switch target {
        case .global:
            if let settings = await state.loadGlobalNotificationSettings() {
                globalSettings = settings
            }
        case .feed(let feed):
            if let settings = await state.loadFeedNotificationSettings(feedID: feed.id) {
                feedSettings = settings
            }
        }
        isLoading = false
    }

    private func save() async {
        switch target {
        case .global:
            _ = await state.saveGlobalNotificationSettings(globalSettings)
        case .feed(let feed):
            _ = await state.saveFeedNotificationSettings(feedID: feed.id, settings: feedSettings)
        }
        onCancel()
    }

    private static func defaultGlobalSettings() -> GlobalNotificationSettings {
        GlobalNotificationSettings(
            backgroundRefreshEnabled: true,
            backgroundRefreshIntervalMinutes: 15,
            digestPolicy: DigestPolicy(enabled: false, intervalMinutes: 60, maxItems: 20),
            defaultFeedSettings: defaultFeedSettings()
        )
    }

    private static func defaultFeedSettings() -> NotificationSettings {
        NotificationSettings(
            enabled: false,
            mode: .immediate,
            digestPolicy: DigestPolicy(enabled: false, intervalMinutes: 60, maxItems: 20),
            quietHours: QuietHours(enabled: false, startMinute: 22 * 60, endMinute: 7 * 60),
            minimumIntervalMinutes: 60,
            highPriority: false,
            keywordInclude: [],
            keywordExclude: []
        )
    }
}

private struct NotificationSettingsForm: View {
    let title: String
    @Binding var settings: NotificationSettings
    @State private var quietHoursStartText: String = ""
    @State private var quietHoursEndText: String = ""

    var body: some View {
        Section {
            Toggle("启用通知", isOn: $settings.enabled)
            Picker("模式", selection: $settings.mode) {
                Text("即时").tag(NotificationMode.immediate)
                Text("摘要").tag(NotificationMode.digest)
            }
            .pickerStyle(.segmented)
            .onChange(of: settings.mode) { _, newValue in
                settings.digestPolicy.enabled = newValue == .digest
            }

            Stepper(
                "最小间隔 \(settings.minimumIntervalMinutes) 分钟",
                value: $settings.minimumIntervalMinutes,
                in: 5...1440,
                step: 5
            )
            Toggle("高优先级", isOn: $settings.highPriority)

            if settings.mode == .digest {
                Toggle("启用摘要", isOn: $settings.digestPolicy.enabled)
                Stepper(
                    "摘要间隔 \(settings.digestPolicy.intervalMinutes) 分钟",
                    value: $settings.digestPolicy.intervalMinutes,
                    in: 15...1440,
                    step: 15
                )
                Stepper(
                    "摘要最多 \(settings.digestPolicy.maxItems) 条",
                    value: $settings.digestPolicy.maxItems,
                    in: 3...100,
                    step: 1
                )
            }

            Toggle("勿扰时间", isOn: $settings.quietHours.enabled)
            HStack(spacing: 8) {
                TextField("开始 HH:mm", text: $quietHoursStartText)
                    .onChange(of: quietHoursStartText) { _, value in
                        if let minutes = minutes(from: value) {
                            settings.quietHours.startMinute = minutes
                        }
                    }
                TextField("结束 HH:mm", text: $quietHoursEndText)
                    .onChange(of: quietHoursEndText) { _, value in
                        if let minutes = minutes(from: value) {
                            settings.quietHours.endMinute = minutes
                        }
                    }
            }

            TextField("包含关键词（逗号分隔）", text: keywordIncludeBinding)
            TextField("排除关键词（逗号分隔）", text: keywordExcludeBinding)
        } header: {
            Text(title)
        } footer: {
            Text("只对新内容发通知，摘要会把同一订阅的更新合并成一条。")
        }
        .onAppear {
            quietHoursStartText = minutesString(from: settings.quietHours.startMinute)
            quietHoursEndText = minutesString(from: settings.quietHours.endMinute)
            settings.digestPolicy.enabled = settings.mode == .digest
        }
    }

    private var keywordIncludeBinding: Binding<String> {
        Binding(
            get: { settings.keywordInclude.joined(separator: ", ") },
            set: { settings.keywordInclude = splitKeywords($0) }
        )
    }

    private var keywordExcludeBinding: Binding<String> {
        Binding(
            get: { settings.keywordExclude.joined(separator: ", ") },
            set: { settings.keywordExclude = splitKeywords($0) }
        )
    }

    private func minutesString(from value: Int) -> String {
        let clamped = max(0, min(23 * 60 + 59, value))
        return String(format: "%02d:%02d", clamped / 60, clamped % 60)
    }

    private func minutes(from value: String) -> Int? {
        let pieces = value.split(separator: ":", omittingEmptySubsequences: false)
        guard pieces.count == 2,
              let hour = Int(pieces[0]),
              let minute = Int(pieces[1]),
              (0...23).contains(hour),
              (0...59).contains(minute)
        else {
            return nil
        }
        return hour * 60 + minute
    }

    private func splitKeywords(_ value: String) -> [String] {
        value
            .split { $0 == "," || $0 == "\n" || $0 == "\t" }
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
    }
}

private struct TimelineBucketKey: Hashable {
    let title: String
    let date: Date
}

private struct TimelineGroup {
    let title: String
    let items: [ArticleItem]
}

private enum SmartFeedShortcut: CaseIterable {
    private static let allSelectionID = "__all_items__"
    private static let unreadSelectionID = "__unread_items__"
    private static let starredSelectionID = "__starred_items__"
    private static let laterSelectionID = "__later_items__"
    private static let notesSelectionID = "__notes_items__"
    private static let archiveSelectionID = "__archive_items__"

    case all
    case unread
    case starred
    case later
    case notes
    case archive

    var selectionID: String {
        switch self {
        case .all:
            return Self.allSelectionID
        case .unread:
            return Self.unreadSelectionID
        case .starred:
            return Self.starredSelectionID
        case .later:
            return Self.laterSelectionID
        case .notes:
            return Self.notesSelectionID
        case .archive:
            return Self.archiveSelectionID
        }
    }

    var title: String {
        switch self {
        case .all:
            return "收件箱"
        case .unread:
            return "未读"
        case .starred:
            return "星标"
        case .later:
            return "稍后读"
        case .notes:
            return "随想"
        case .archive:
            return "归档"
        }
    }

    var icon: String {
        switch self {
        case .all:
            return "tray.full"
        case .unread:
            return "circle.fill"
        case .starred:
            return "star.fill"
        case .later:
            return "bookmark.fill"
        case .notes:
            return "note.text"
        case .archive:
            return "archivebox.fill"
        }
    }

}
