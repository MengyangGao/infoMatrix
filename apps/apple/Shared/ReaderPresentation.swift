import Foundation

public struct ReaderSidebarRowState: Identifiable, Equatable, Sendable {
    public let id: String
    public let title: String
    public let subtitle: String?
    public let iconName: String
    public let badgeCount: Int?
    public let isSelected: Bool

    public init(
        id: String,
        title: String,
        subtitle: String? = nil,
        iconName: String,
        badgeCount: Int? = nil,
        isSelected: Bool = false
    ) {
        self.id = id
        self.title = title
        self.subtitle = subtitle
        self.iconName = iconName
        self.badgeCount = badgeCount
        self.isSelected = isSelected
    }
}

public struct ReaderSidebarSectionState: Identifiable, Equatable, Sendable {
    public let id: String
    public let title: String
    public let rows: [ReaderSidebarRowState]

    public init(id: String, title: String, rows: [ReaderSidebarRowState]) {
        self.id = id
        self.title = title
        self.rows = rows
    }
}

public struct ReaderDetailPaneState: Equatable, Sendable {
    public let title: String
    public let subtitle: String?
    public let bodyPreview: String?
    public let metadata: [String]

    public init(
        title: String,
        subtitle: String? = nil,
        bodyPreview: String? = nil,
        metadata: [String] = []
    ) {
        self.title = title
        self.subtitle = subtitle
        self.bodyPreview = bodyPreview
        self.metadata = metadata
    }
}

public struct ReaderScreenState: Equatable, Sendable {
    public let headerTitle: String
    public let headerSubtitle: String
    public let sidebarSections: [ReaderSidebarSectionState]
    public let detailPane: ReaderDetailPaneState?
    public let isLoading: Bool
    public let errorMessage: String?
    public let syncStatusLine: String?

    public init(
        headerTitle: String,
        headerSubtitle: String,
        sidebarSections: [ReaderSidebarSectionState],
        detailPane: ReaderDetailPaneState?,
        isLoading: Bool,
        errorMessage: String?,
        syncStatusLine: String?
    ) {
        self.headerTitle = headerTitle
        self.headerSubtitle = headerSubtitle
        self.sidebarSections = sidebarSections
        self.detailPane = detailPane
        self.isLoading = isLoading
        self.errorMessage = errorMessage
        self.syncStatusLine = syncStatusLine
    }
}
