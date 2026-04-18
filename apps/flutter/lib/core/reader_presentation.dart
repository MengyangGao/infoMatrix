class ReaderSidebarRowState {
  const ReaderSidebarRowState({
    required this.id,
    required this.title,
    required this.iconName,
    this.subtitle,
    this.badgeCount,
    this.isSelected = false,
  });

  final String id;
  final String title;
  final String? subtitle;
  final String iconName;
  final int? badgeCount;
  final bool isSelected;
}

class ReaderSidebarSectionState {
  const ReaderSidebarSectionState({
    required this.id,
    required this.title,
    required this.rows,
  });

  final String id;
  final String title;
  final List<ReaderSidebarRowState> rows;
}

class ReaderDetailPaneState {
  const ReaderDetailPaneState({
    required this.title,
    required this.metadata,
    this.subtitle,
    this.bodyPreview,
  });

  final String title;
  final String? subtitle;
  final String? bodyPreview;
  final List<String> metadata;
}

class ReaderScreenState {
  const ReaderScreenState({
    required this.headerTitle,
    required this.headerSubtitle,
    required this.sidebarSections,
    required this.isLoading,
    required this.errorMessage,
    this.detailPane,
    this.syncStatusLine,
  });

  final String headerTitle;
  final String headerSubtitle;
  final List<ReaderSidebarSectionState> sidebarSections;
  final ReaderDetailPaneState? detailPane;
  final bool isLoading;
  final String? errorMessage;
  final String? syncStatusLine;
}
