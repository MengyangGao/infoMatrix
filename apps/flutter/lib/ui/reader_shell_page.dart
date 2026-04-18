import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../core/models.dart';
import '../core/reader_presentation.dart';
import '../core/reader_backend.dart';

class ReaderShellPage extends StatefulWidget {
  const ReaderShellPage({
    required this.backend,
    super.key,
  });

  final ReaderBackend backend;

  @override
  State<ReaderShellPage> createState() => _ReaderShellPageState();
}

class _ReaderShellPageState extends State<ReaderShellPage> {
  final TextEditingController _urlController =
      TextEditingController(text: 'https://example.com/feed.xml');
  final TextEditingController _searchController = TextEditingController();

  List<FeedModel> _feeds = const <FeedModel>[];
  List<FeedGroupModel> _groups = const <FeedGroupModel>[];
  List<ItemModel> _items = const <ItemModel>[];
  ItemScopeCounts _counts = const ItemScopeCounts(
    all: 0,
    unread: 0,
    starred: 0,
    later: 0,
    notes: 0,
    archive: 0,
  );
  FeedModel? _selectedFeed;
  String _selectedSelectionID = _inboxSelectionID;
  ItemModel? _selectedItem;

  bool _loading = false;
  String _statusText = '初始化中...';
  int _compactTabIndex = 0;
  Timer? _searchDebounce;
  final Map<String, int> _selectionItemLimits = <String, int>{};

  static const String _inboxSelectionID = '__all_items__';
  static const String _unreadSelectionID = '__unread_items__';
  static const String _starredSelectionID = '__starred_items__';
  static const String _laterSelectionID = '__later_items__';
  static const String _notesSelectionID = '__notes_items__';
  static const String _archiveSelectionID = '__archive_items__';
  static const String _groupSelectionPrefix = '__group__:';

  @override
  void initState() {
    super.initState();
    _bootstrap();
  }

  @override
  void dispose() {
    _searchDebounce?.cancel();
    _urlController.dispose();
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _bootstrap() async {
    await _runBusy(() async {
      final health = await widget.backend.health();
      _statusText = 'Core ${health.status} • v${health.version}';
      await _loadFeeds(resetSelection: true);
    });
  }

  Future<void> _loadFeeds({
    bool resetSelection = false,
    String? preferredSelectionID,
  }) async {
    final feedsTask = widget.backend.listFeeds();
    final groupsTask = widget.backend.listGroups();
    final countsTask = widget.backend.itemCounts();
    final feeds = await feedsTask;
    final groups = await groupsTask;
    final counts = await countsTask;

    _feeds = feeds;
    _groups = groups;
    _counts = counts;
    if (_feeds.isEmpty) {
      _selectedFeed = null;
      _selectedSelectionID = _inboxSelectionID;
      _items = const <ItemModel>[];
      _selectedItem = null;
      return;
    }

    final selectedId = preferredSelectionID ??
        (resetSelection ? _inboxSelectionID : _selectedSelectionID);
    if (_isGroupSelection(selectedId) || _isSpecialSelection(selectedId)) {
      _selectedSelectionID = selectedId;
      _selectedFeed = null;
    } else {
      _selectedSelectionID = selectedId;
      _selectedFeed = _feeds.firstWhere(
        (feed) => feed.id == selectedId,
        orElse: () => _feeds.first,
      );
      _selectedSelectionID = _selectedFeed!.id;
    }

    await _loadItems();
  }

  Future<void> _loadItems() async {
    final query = _searchController.text.trim();
    final searchQuery = query.isEmpty ? null : query;
    final selectionID = _selectedSelectionID;
    final limit = _selectionLimit(selectionID);

    if (selectionID == _inboxSelectionID) {
      _items = await widget.backend.listAllItems(
        limit: limit,
        searchQuery: searchQuery,
        filter: 'all',
      );
    } else if (selectionID == _unreadSelectionID) {
      _items = await widget.backend.listAllItems(
        limit: limit,
        searchQuery: searchQuery,
        filter: 'unread',
      );
    } else if (selectionID == _starredSelectionID) {
      _items = await widget.backend.listAllItems(
        limit: limit,
        searchQuery: searchQuery,
        filter: 'starred',
      );
    } else if (selectionID == _laterSelectionID) {
      _items = await widget.backend.listAllItems(
        limit: limit,
        searchQuery: searchQuery,
        filter: 'later',
      );
    } else if (selectionID == _notesSelectionID) {
      _items = await widget.backend.listAllItems(
        limit: limit,
        searchQuery: searchQuery,
        filter: 'all',
        kind: 'note',
      );
    } else if (selectionID == _archiveSelectionID) {
      _items = await widget.backend.listAllItems(
        limit: limit,
        searchQuery: searchQuery,
        filter: 'archive',
      );
    } else if (_isGroupSelection(selectionID)) {
      final groupID = _groupIDFromSelection(selectionID);
      final groupedFeeds = _feeds
          .where(
            (feed) => feed.groups.any((group) => group.id == groupID),
          )
          .toList(growable: false);
      final collected = <ItemModel>[];
      for (final feed in groupedFeeds) {
        final items = await widget.backend.listItems(
          feed.id,
          limit: limit,
          searchQuery: searchQuery,
        );
        collected.addAll(items);
      }
      _items = _deduplicatedItems(collected);
    } else if (_selectedFeed != null) {
      _items = await widget.backend.listItems(
        _selectedFeed!.id,
        limit: limit,
        searchQuery: searchQuery,
      );
    } else {
      _items = const <ItemModel>[];
    }

    if (_items.isEmpty) {
      _selectedItem = null;
      return;
    }
    final previousSelectedID = _selectedItem?.id;
    _selectedItem = _items.firstWhere(
      (item) => item.id == previousSelectedID,
      orElse: () => _items.first,
    );
  }

  Future<void> _addFeedDirectly() async {
    final inputUrl = _urlController.text.trim();
    if (inputUrl.isEmpty) {
      _showSnack('请输入 feed 或站点 URL');
      return;
    }

    await _runBusy(() async {
      final result = await widget.backend.subscribeInput(inputUrl);
      await _loadFeeds(preferredSelectionID: result.feedId);
      await _refreshCurrentSelection();
      _statusText = '订阅已添加 (${result.subscriptionSource})';
    });
  }

  Future<void> _discoverAndAdd() async {
    final siteUrl = _urlController.text.trim();
    if (siteUrl.isEmpty) {
      _showSnack('请输入站点 URL');
      return;
    }

    await _runBusy(() async {
      final result = await widget.backend.discoverSite(siteUrl);
      final candidates =
          List<DiscoverFeedCandidate>.from(result.discoveredFeeds)
            ..sort((a, b) => b.score.compareTo(a.score));

      if (candidates.isEmpty) {
        final direct = await widget.backend.subscribeInput(siteUrl);
        await _loadFeeds(preferredSelectionID: direct.feedId);
        final warningText =
            result.warnings.isEmpty ? '' : '，${result.warnings.first}';
        _statusText =
            '站点发现无结果$warningText，已改用智能订阅 (${direct.subscriptionSource})';
        return;
      }

      final candidate = await _pickCandidate(candidates);
      if (candidate == null) {
        _statusText = '已取消选择';
        return;
      }

      final feedId = await widget.backend.addSubscription(
        candidate.url,
        title: candidate.title,
      );
      await _loadFeeds(preferredSelectionID: feedId);
      await _refreshCurrentSelection();
      _statusText = '已通过站点发现订阅: ${candidate.title ?? candidate.url}';
    });
  }

  Future<DiscoverFeedCandidate?> _pickCandidate(
    List<DiscoverFeedCandidate> candidates,
  ) async {
    if (candidates.length == 1) {
      return candidates.first;
    }

    return showDialog<DiscoverFeedCandidate>(
      context: context,
      builder: (context) {
        return AlertDialog(
          title: const Text('选择要订阅的 Feed'),
          content: SizedBox(
            width: 560,
            child: ListView.builder(
              shrinkWrap: true,
              itemCount: candidates.length,
              itemBuilder: (context, index) {
                final candidate = candidates[index];
                return ListTile(
                  title: Text(candidate.title ?? candidate.url),
                  subtitle: Text(
                    'score ${candidate.score} • ${candidate.feedType} • ${candidate.source} • ${candidate.url}',
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                  onTap: () => Navigator.of(context).pop(candidate),
                );
              },
            ),
          ),
          actions: <Widget>[
            TextButton(
              onPressed: () => Navigator.of(context).pop(),
              child: const Text('取消'),
            ),
          ],
        );
      },
    );
  }

  Future<void> _refreshCurrentSelection() async {
    await _runBusy(() async {
      final feed = _selectedFeed;
      if (feed != null) {
        final result = await widget.backend.refreshFeed(feed.id);
        _statusText =
            '刷新完成: ${result.status}, HTTP ${result.fetchedHttpStatus}, 新增/更新 ${result.itemCount}, 通知 ${result.notificationCount}, 抑制 ${result.suppressedNotificationCount}';
      } else if (_isGroupSelection(_selectedSelectionID)) {
        final groupID = _groupIDFromSelection(_selectedSelectionID);
        final groupedFeeds = _feeds
            .where(
              (value) => value.groups.any((group) => group.id == groupID),
            )
            .toList(growable: false);
        var totalItemCount = 0;
        var refreshedCount = 0;
        final errors = <String>[];
        for (final groupedFeed in groupedFeeds) {
          try {
            final result = await widget.backend.refreshFeed(groupedFeed.id);
            refreshedCount += 1;
            totalItemCount += result.itemCount;
          } catch (error) {
            errors.add(error.toString());
          }
        }
        if (errors.isNotEmpty && refreshedCount == 0) {
          _statusText = '分类刷新失败: ${errors.first}';
        } else if (errors.isNotEmpty) {
          _statusText =
              '已刷新 ${refreshedCount} 个分类订阅，共 $totalItemCount 条条目，${errors.first}';
        } else {
          _statusText = '已刷新 ${refreshedCount} 个分类订阅，共 $totalItemCount 条条目';
        }
      } else {
        final result = await widget.backend.refreshDueFeeds();
        _statusText =
            '已刷新 ${result.refreshedCount} 个到期订阅，共 ${result.totalItemCount} 条条目';
      }
      await _loadFeeds(preferredSelectionID: _selectedSelectionID);
    });
  }

  Future<void> _refreshDueFeeds() async {
    await _runBusy(() async {
      final result = await widget.backend.refreshDueFeeds();
      await _loadFeeds(preferredSelectionID: _selectedSelectionID);
      _statusText =
          '已刷新 ${result.refreshedCount} 个到期订阅，共 ${result.totalItemCount} 条条目';
    });
  }

  Future<void> _importOpml() async {
    final xml = await _promptOpmlInput();
    if (xml == null) {
      return;
    }

    await _runBusy(() async {
      final result = await widget.backend.importOpml(xml);
      await _loadFeeds(resetSelection: true);
      _statusText = 'OPML 导入完成: ${result.uniqueFeedCount} 个订阅';
    });
  }

  Future<void> _exportOpml() async {
    OpmlExportResult? result;
    await _runBusy(() async {
      result = await widget.backend.exportOpml();
      _statusText = 'OPML 已生成: ${result!.feedCount} 个订阅';
    });

    if (!mounted || result == null) {
      return;
    }
    await _showOpmlExportDialog(result!);
  }

  Future<String?> _promptOpmlInput() async {
    final controller = TextEditingController();
    try {
      return await showDialog<String>(
        context: context,
        builder: (context) {
          return AlertDialog(
            title: const Text('导入 OPML'),
            content: SizedBox(
              width: 680,
              child: TextField(
                controller: controller,
                maxLines: 16,
                decoration: const InputDecoration(
                  hintText: '粘贴 OPML XML 内容',
                  border: OutlineInputBorder(),
                ),
              ),
            ),
            actions: <Widget>[
              TextButton(
                onPressed: () => Navigator.of(context).pop(),
                child: const Text('取消'),
              ),
              FilledButton(
                onPressed: () =>
                    Navigator.of(context).pop(controller.text.trim()),
                child: const Text('导入'),
              ),
            ],
          );
        },
      );
    } finally {
      controller.dispose();
    }
  }

  Future<void> _showOpmlExportDialog(OpmlExportResult result) async {
    await showDialog<void>(
      context: context,
      builder: (context) {
        return AlertDialog(
          title: Text('导出 OPML (${result.feedCount})'),
          content: SizedBox(
            width: 680,
            child: SingleChildScrollView(
              child: SelectableText(result.opmlXml),
            ),
          ),
          actions: <Widget>[
            TextButton(
              onPressed: () => Navigator.of(context).pop(),
              child: const Text('关闭'),
            ),
            FilledButton(
              onPressed: () async {
                await Clipboard.setData(ClipboardData(text: result.opmlXml));
                if (mounted) {
                  Navigator.of(context).pop();
                  _showSnack('OPML 已复制到剪贴板');
                }
              },
              child: const Text('复制'),
            ),
          ],
        );
      },
    );
  }

  Future<void> _toggleItemRead(ItemModel item) async {
    await _patchItem(item, isRead: !item.isRead);
  }

  Future<void> _toggleItemStar(ItemModel item) async {
    await _patchItem(item, isStarred: !item.isStarred);
  }

  Future<void> _toggleItemLater(ItemModel item) async {
    await _patchItem(item, isSavedForLater: !item.isSavedForLater);
  }

  Future<void> _toggleItemArchive(ItemModel item) async {
    await _patchItem(item, isArchived: !item.isArchived);
  }

  Future<void> _patchItem(
    ItemModel item, {
    bool? isRead,
    bool? isStarred,
    bool? isSavedForLater,
    bool? isArchived,
  }) async {
    await _runBusy(() async {
      final updated = await widget.backend.patchItemState(
        item,
        isRead: isRead,
        isStarred: isStarred,
        isSavedForLater: isSavedForLater,
        isArchived: isArchived,
      );

      _items = _items
          .map((value) => value.id == updated.id ? updated : value)
          .toList(growable: false);
      if (_selectedItem?.id == updated.id) {
        _selectedItem = updated;
      }
      await _loadFeeds();
      _statusText = '条目状态已更新';
    });
  }

  bool _isSpecialSelection(String selectionID) {
    return selectionID == _inboxSelectionID ||
        selectionID == _unreadSelectionID ||
        selectionID == _starredSelectionID ||
        selectionID == _laterSelectionID ||
        selectionID == _notesSelectionID ||
        selectionID == _archiveSelectionID;
  }

  bool _isGroupSelection(String selectionID) {
    return selectionID.startsWith(_groupSelectionPrefix);
  }

  String _groupIDFromSelection(String selectionID) {
    if (!_isGroupSelection(selectionID)) {
      return '';
    }
    return selectionID.substring(_groupSelectionPrefix.length);
  }

  String _groupSelectionID(String groupID) {
    return '$_groupSelectionPrefix$groupID';
  }

  int _defaultSelectionLimit(String selectionID) {
    switch (selectionID) {
      case _inboxSelectionID:
        return max(250, _counts.all + 50);
      case _unreadSelectionID:
        return max(250, _counts.unread + 50);
      case _starredSelectionID:
        return max(250, _counts.starred + 50);
      case _laterSelectionID:
        return max(250, _counts.later + 50);
      case _notesSelectionID:
        return max(100, _counts.notes + 20);
      case _archiveSelectionID:
        return max(250, _counts.archive + 50);
      default:
        return _isGroupSelection(selectionID) ? 1000 : 250;
    }
  }

  int _selectionLimit(String selectionID) {
    return _selectionItemLimits[selectionID] ??
        _defaultSelectionLimit(selectionID);
  }

  bool _canLoadMoreItems() {
    return _items.isNotEmpty &&
        _items.length >= _selectionLimit(_selectedSelectionID);
  }

  int _loadMoreIncrement(String selectionID) {
    return _isGroupSelection(selectionID) ? 200 : 100;
  }

  Future<void> _loadMoreItems() async {
    final selectionID = _selectedSelectionID;
    final currentLimit = _selectionLimit(selectionID);
    _selectionItemLimits[selectionID] =
        currentLimit + _loadMoreIncrement(selectionID);
    await _runBusy(() async {
      await _loadItems();
      _statusText = '已加载更多条目';
    });
  }

  List<ItemModel> _deduplicatedItems(List<ItemModel> items) {
    final seen = <String>{};
    final deduplicated = items
        .where(
            (item) => seen.add(item.canonicalUrl ?? item.sourceURL ?? item.id))
        .toList(growable: false);
    deduplicated.sort((lhs, rhs) {
      final lhsDate = DateTime.tryParse(lhs.publishedAt ?? '') ??
          DateTime.fromMillisecondsSinceEpoch(0);
      final rhsDate = DateTime.tryParse(rhs.publishedAt ?? '') ??
          DateTime.fromMillisecondsSinceEpoch(0);
      final dateCompare = rhsDate.compareTo(lhsDate);
      if (dateCompare != 0) {
        return dateCompare;
      }
      return lhs.title.toLowerCase().compareTo(rhs.title.toLowerCase());
    });
    return deduplicated;
  }

  String get _selectionTitle {
    return _screenState.headerTitle;
  }

  String get _selectionSubtitle {
    return _screenState.headerSubtitle;
  }

  String get _selectionEmptyMessage {
    if (_selectedFeed != null) {
      return '该订阅暂无条目，点击刷新按钮试试。';
    }
    if (_isGroupSelection(_selectedSelectionID)) {
      return '这个分类里还没有内容。';
    }
    switch (_selectedSelectionID) {
      case _inboxSelectionID:
        return '收件箱还是空的。先订阅一个 RSS，或者保存一个网页。';
      case _unreadSelectionID:
        return '没有未读内容。';
      case _starredSelectionID:
        return '还没有星标内容。';
      case _laterSelectionID:
        return '稍后读还空着。';
      case _notesSelectionID:
        return '还没有随想。';
      case _archiveSelectionID:
        return '归档里还没有内容。';
      default:
        return '没有内容。';
    }
  }

  Future<void> _selectFeed(FeedModel feed) async {
    await _runBusy(() async {
      _selectedFeed = feed;
      _selectedSelectionID = feed.id;
      _selectedItem = null;
      await _loadItems();
    });
  }

  Future<void> _selectGroup(FeedGroupModel group) async {
    await _runBusy(() async {
      _selectedFeed = null;
      _selectedSelectionID = _groupSelectionID(group.id);
      _selectedItem = null;
      await _loadItems();
    });
  }

  Future<void> _selectSpecialScope(String selectionID) async {
    await _runBusy(() async {
      _selectedFeed = null;
      _selectedSelectionID = selectionID;
      _selectedItem = null;
      await _loadItems();
    });
  }

  Future<void> _selectItem(ItemModel item) async {
    await _runBusy(() async {
      _compactTabIndex = 2;
      final currentItem = _items.firstWhere(
        (value) => value.id == item.id,
        orElse: () => item,
      );
      if (!currentItem.isRead) {
        final updated = await widget.backend.patchItemState(
          currentItem,
          isRead: true,
        );
        _items = _items
            .map((value) => value.id == updated.id ? updated : value)
            .toList(growable: false);
        _selectedItem = updated;
        await _loadFeeds(preferredSelectionID: _selectedSelectionID);
      }

      final detail = await widget.backend.itemDetail(item.id);
      var hydratedDetail = detail;
      final autoFullText = _shouldAutoFetchFullTextFor(item);
      if (autoFullText) {
        try {
          hydratedDetail = await widget.backend.fetchFullText(item.id);
        } catch (error) {
          _statusText = '抓取全文失败: $error';
        }
      }
      _selectedItem = hydratedDetail;
      _items = _items
          .map((value) => value.id == hydratedDetail.id ? hydratedDetail : value)
          .toList(growable: false);
    });
  }

  ReaderScreenState get _screenState {
    return ReaderScreenState(
      headerTitle: _selectionTitleForPresentation,
      headerSubtitle: _selectionSubtitleForPresentation,
      sidebarSections: <ReaderSidebarSectionState>[
        ReaderSidebarSectionState(
          id: 'special',
          title: '首页',
          rows: <ReaderSidebarRowState>[
            ReaderSidebarRowState(
              id: _inboxSelectionID,
              title: '全部',
              iconName: 'tray.full',
              badgeCount: _counts.all,
              isSelected: _selectedSelectionID == _inboxSelectionID,
            ),
            ReaderSidebarRowState(
              id: _unreadSelectionID,
              title: '未读',
              iconName: 'circle.dashed',
              badgeCount: _counts.unread,
              isSelected: _selectedSelectionID == _unreadSelectionID,
            ),
            ReaderSidebarRowState(
              id: _starredSelectionID,
              title: '星标',
              iconName: 'star',
              badgeCount: _counts.starred,
              isSelected: _selectedSelectionID == _starredSelectionID,
            ),
            ReaderSidebarRowState(
              id: _laterSelectionID,
              title: '稍后读',
              iconName: 'clock',
              badgeCount: _counts.later,
              isSelected: _selectedSelectionID == _laterSelectionID,
            ),
            ReaderSidebarRowState(
              id: _notesSelectionID,
              title: '随想',
              iconName: 'square.and.pencil',
              badgeCount: _counts.notes,
              isSelected: _selectedSelectionID == _notesSelectionID,
            ),
            ReaderSidebarRowState(
              id: _archiveSelectionID,
              title: '归档',
              iconName: 'archivebox',
              badgeCount: _counts.archive,
              isSelected: _selectedSelectionID == _archiveSelectionID,
            ),
          ],
        ),
        ReaderSidebarSectionState(
          id: 'groups',
          title: '分类',
          rows: _groups
              .map(
                (group) => ReaderSidebarRowState(
                  id: _groupSelectionID(group.id),
                  title: group.name,
                  iconName: 'folder.fill',
                  badgeCount: _feeds
                      .where(
                        (feed) => feed.groups.any((value) => value.id == group.id),
                      )
                      .length,
                  isSelected: _selectedSelectionID == _groupSelectionID(group.id),
                ),
              )
              .toList(growable: false),
        ),
        ReaderSidebarSectionState(
          id: 'feeds',
          title: '订阅',
          rows: _feeds
              .map(
                (feed) => ReaderSidebarRowState(
                  id: feed.id,
                  title: feed.title,
                  subtitle: feed.siteUrl ?? feed.feedUrl,
                  iconName: 'newspaper.fill',
                  isSelected: _selectedSelectionID == feed.id,
                ),
              )
              .toList(growable: false),
        ),
      ],
      detailPane: _selectedItem == null
          ? null
          : ReaderDetailPaneState(
              title: _selectedItem!.title,
              subtitle: _selectedItem!.canonicalUrl,
              bodyPreview: _selectedItem!.summary ?? _selectedItem!.contentText,
              metadata: <String>[
                _selectedItem!.kind,
                _selectedItem!.sourceKind,
                if (_selectedItem!.publishedAt != null) _selectedItem!.publishedAt!,
              ],
            ),
      isLoading: _loading,
      errorMessage: null,
    );
  }

  String get _selectionTitleForPresentation {
    if (_selectedFeed != null) {
      return _selectedFeed!.title;
    }
    if (_isGroupSelection(_selectedSelectionID)) {
      return _groups
          .firstWhere(
            (group) => group.id == _groupIDFromSelection(_selectedSelectionID),
            orElse: () => const FeedGroupModel(id: '', name: '分类'),
          )
          .name;
    }
    switch (_selectedSelectionID) {
      case _inboxSelectionID:
        return '收件箱';
      case _unreadSelectionID:
        return '未读';
      case _starredSelectionID:
        return '星标';
      case _laterSelectionID:
        return '稍后读';
      case _notesSelectionID:
        return '随想';
      case _archiveSelectionID:
        return '归档';
      default:
        return '内容';
    }
  }

  String get _selectionSubtitleForPresentation {
    if (_selectedFeed != null) {
      return '${_items.length} 条条目';
    }
    if (_isGroupSelection(_selectedSelectionID)) {
      final groupID = _groupIDFromSelection(_selectedSelectionID);
      final feedCount = _feeds
          .where((feed) => feed.groups.any((group) => group.id == groupID))
          .length;
      return '$feedCount 个订阅 · ${_items.length} 条条目';
    }
    switch (_selectedSelectionID) {
      case _inboxSelectionID:
        return '${_items.length} 条内容';
      case _unreadSelectionID:
        return '${_items.length} 条未读';
      case _starredSelectionID:
        return '${_items.length} 条星标';
      case _laterSelectionID:
        return '${_items.length} 条稍后读';
      case _notesSelectionID:
        return '${_items.length} 条随想';
      case _archiveSelectionID:
        return '${_items.length} 条归档';
      default:
        return '${_items.length} 条内容';
    }
  }

  Future<void> _fetchFullTextForSelectedItem() async {
    final item = _selectedItem;
    if (item == null) {
      return;
    }

    await _runBusy(() async {
      final detail = await widget.backend.fetchFullText(item.id);
      _selectedItem = detail;
      _items = _items
          .map((value) => value.id == detail.id ? detail : value)
          .toList(growable: false);
      _statusText = '已抓取全文';
    });
  }

  bool _shouldAutoFetchFullTextFor(ItemModel item) {
    if (item.sourceKind != 'feed') {
      return false;
    }
    FeedModel? feed;
    if (item.sourceID != null) {
      for (final candidate in _feeds) {
        if (candidate.id == item.sourceID) {
          feed = candidate;
          break;
        }
      }
    }
    feed ??= _selectedFeed;
    return feed?.autoFullText ?? true;
  }

  Future<void> _composeBookmark() async {
    final result = await showDialog<_BookmarkDraft>(
      context: context,
      builder: (context) =>
          const _EntryComposerSheet(kind: _EntryComposerKind.bookmark),
    );
    if (result == null) {
      return;
    }

    await _runBusy(() async {
      final created = await widget.backend.createEntry(
        title: result.title,
        kind: 'bookmark',
        sourceKind: 'web',
        sourceURL: result.url,
        sourceTitle: result.title,
        canonicalURL: result.url,
        summary: result.note,
        contentText: result.note,
      );
      final later = await widget.backend.patchItemState(
        created,
        isSavedForLater: true,
      );
      final detailed = await widget.backend.fetchFullText(later.id);
      _selectedFeed = null;
      _selectedSelectionID = _laterSelectionID;
      _selectedItem = detailed;
      await _loadFeeds();
      _statusText = '网页已保存到稍后读';
    });
  }

  Future<void> _composeNote() async {
    final result = await showDialog<_NoteDraft>(
      context: context,
      builder: (context) =>
          const _EntryComposerSheet(kind: _EntryComposerKind.note),
    );
    if (result == null) {
      return;
    }

    await _runBusy(() async {
      final created = await widget.backend.createEntry(
        title: result.title,
        kind: 'note',
        sourceKind: 'manual',
        sourceTitle: '手动输入',
        summary: result.body,
        contentText: result.body,
      );
      _selectedFeed = null;
      _selectedSelectionID = _notesSelectionID;
      _selectedItem = created;
      await _loadFeeds();
      _statusText = '随想已保存';
    });
  }

  Future<void> _openGlobalNotificationSettings() async {
    GlobalNotificationSettings? settings;
    await _runBusy(() async {
      settings = await widget.backend.globalNotificationSettings();
    });
    if (!mounted || settings == null) {
      return;
    }

    final result = await showDialog<Object>(
      context: context,
      builder: (context) => _NotificationSettingsDialog.global(
        initialGlobalSettings: settings!,
      ),
    );
    if (result is! GlobalNotificationSettings) {
      return;
    }

    await _runBusy(() async {
      await widget.backend.updateGlobalNotificationSettings(result);
      _statusText = '全局通知设置已保存';
    });
  }

  Future<void> _openFeedNotificationSettings(FeedModel feed) async {
    NotificationSettings? settings;
    await _runBusy(() async {
      settings = await widget.backend.feedNotificationSettings(feed.id);
    });
    if (!mounted || settings == null) {
      return;
    }

    final result = await showDialog<Object>(
      context: context,
      builder: (context) => _NotificationSettingsDialog.feed(
        title: feed.title,
        initialSettings: settings!,
      ),
    );
    if (result is! NotificationSettings) {
      return;
    }

    await _runBusy(() async {
      await widget.backend.updateFeedNotificationSettings(feed.id, result);
      _statusText = '《${feed.title}》通知设置已保存';
    });
  }

  Future<void> _openFeedRefreshSettings(FeedModel feed) async {
    RefreshSettings? settings;
    await _runBusy(() async {
      settings = await widget.backend.feedRefreshSettings(feed.id);
    });
    if (!mounted || settings == null) {
      return;
    }

    final result = await showDialog<Object>(
      context: context,
      builder: (context) => _RefreshSettingsDialog(
        title: '${feed.title} 自动刷新',
        initialSettings: settings!,
      ),
    );
    if (result is! RefreshSettings) {
      return;
    }

    await _runBusy(() async {
      await widget.backend.updateFeedRefreshSettings(feed.id, result);
      _statusText = '《${feed.title}》自动刷新设置已保存';
    });
  }

  Future<void> _resetFeedRefreshSettings(FeedModel feed) async {
    await _runBusy(() async {
      await widget.backend.deleteFeedRefreshSettings(feed.id);
      _statusText = '《${feed.title}》已恢复默认刷新';
    });
  }

  Future<void> _openGroupRefreshSettings(FeedGroupModel group) async {
    RefreshSettings? settings;
    await _runBusy(() async {
      settings = await widget.backend.groupRefreshSettings(group.id);
    });
    if (!mounted || settings == null) {
      return;
    }

    final result = await showDialog<Object>(
      context: context,
      builder: (context) => _RefreshSettingsDialog(
        title: '${group.name} 自动刷新',
        initialSettings: settings!,
      ),
    );
    if (result is! RefreshSettings) {
      return;
    }

    await _runBusy(() async {
      await widget.backend.updateGroupRefreshSettings(group.id, result);
      _statusText = '《${group.name}》自动刷新设置已保存';
    });
  }

  Future<void> _resetGroupRefreshSettings(FeedGroupModel group) async {
    await _runBusy(() async {
      await widget.backend.deleteGroupRefreshSettings(group.id);
      _statusText = '《${group.name}》已恢复默认刷新';
    });
  }

  Future<void> _openFeedEditor(FeedModel feed) async {
    var groups = _groups;
    if (groups.isEmpty) {
      try {
        groups = await widget.backend.listGroups();
      } catch (error) {
        _showSnack('加载分类失败: $error');
        return;
      }
    }
    if (!mounted) {
      return;
    }

    final result = await showDialog<_FeedEditDraft>(
      context: context,
      builder: (context) => _FeedEditDialog(
        feed: feed,
        groups: groups,
      ),
    );
    if (result == null) {
      return;
    }

    await _runBusy(() async {
      final trimmedTitle = result.title.trim();
      final requestedTitle = trimmedTitle.isEmpty ? '' : trimmedTitle;
      final autoFullTextChanged = result.autoFullText != feed.autoFullText;
      if (requestedTitle != feed.title || autoFullTextChanged) {
        await widget.backend.updateFeed(
          feed.id,
          title: requestedTitle != feed.title ? requestedTitle : null,
          autoFullText: autoFullTextChanged ? result.autoFullText : null,
        );
      }

      if (result.newGroupName.trim().isNotEmpty) {
        final created = await widget.backend.createGroup(result.newGroupName);
        await widget.backend.updateFeedGroup(feed.id, groupId: created.id);
      } else {
        await widget.backend.updateFeedGroup(feed.id, groupId: result.groupId);
      }

      _statusText = '订阅已更新';
      await _loadFeeds(preferredSelectionID: _selectedSelectionID);
    });
  }

  Future<void> _confirmDeleteFeed(FeedModel feed) async {
    final shouldDelete = await showDialog<bool>(
      context: context,
      builder: (context) {
        return AlertDialog(
          title: const Text('删除订阅'),
          content: Text('确定要删除《${feed.title}》吗？此操作会移除该订阅及其本地条目。'),
          actions: <Widget>[
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: const Text('取消'),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: const Text('删除'),
            ),
          ],
        );
      },
    );

    if (shouldDelete != true) {
      return;
    }

    final shouldResetSelection =
        _selectedSelectionID == feed.id || _selectedFeed?.id == feed.id;
    await _runBusy(() async {
      await widget.backend.deleteFeed(feed.id);
      _selectedFeed = null;
      _selectedItem = null;
      _selectedSelectionID =
          shouldResetSelection ? _inboxSelectionID : _selectedSelectionID;
      await _loadFeeds(
        resetSelection: shouldResetSelection,
        preferredSelectionID:
            shouldResetSelection ? _inboxSelectionID : _selectedSelectionID,
      );
      _statusText = '订阅已删除';
    });
  }

  Future<void> _openNotificationCenter() async {
    await showDialog<void>(
      context: context,
      builder: (context) => NotificationCenterDialog(backend: widget.backend),
    );
  }

  String _plainTextFromHtml(String html) {
    final withoutTags = html.replaceAll(RegExp(r'<[^>]+>'), ' ');
    return withoutTags
        .split(RegExp(r'\s+'))
        .where((part) => part.isNotEmpty)
        .join(' ');
  }

  String _entryBodyText(ItemModel item) {
    final contentText = item.contentText?.trim();
    if (contentText != null && contentText.isNotEmpty) {
      return contentText;
    }
    final contentHTML = item.contentHTML?.trim();
    if (contentHTML != null && contentHTML.isNotEmpty) {
      return _plainTextFromHtml(contentHTML);
    }
    final summary = item.summary?.trim();
    if (summary != null && summary.isNotEmpty) {
      return summary;
    }
    final preview = item.summaryPreview?.trim();
    if (preview != null && preview.isNotEmpty) {
      return preview;
    }
    return '该条目暂无正文内容';
  }

  Future<void> _runBusy(Future<void> Function() action) async {
    if (!mounted) {
      return;
    }
    setState(() {
      _loading = true;
    });

    try {
      await action();
    } on ReaderBackendException catch (error) {
      _statusText = '失败: ${error.message}';
      _showSnack(error.message);
    } catch (error) {
      _statusText = '失败: $error';
      _showSnack(error.toString());
    }

    if (!mounted) {
      return;
    }
    setState(() {
      _loading = false;
    });
  }

  void _showSnack(String message) {
    if (!mounted) {
      return;
    }
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(message)),
    );
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final compact = constraints.maxWidth < 900;
        return Scaffold(
          appBar: AppBar(
            title: const Text('InfoMatrix'),
            actions: <Widget>[
              if (_loading)
                const Padding(
                  padding: EdgeInsets.symmetric(horizontal: 16),
                  child: Center(
                      child: SizedBox.square(
                          dimension: 16,
                          child: CircularProgressIndicator(strokeWidth: 2))),
                ),
            ],
          ),
          body: Column(
            children: <Widget>[
              _buildTopBar(compact: compact),
              Expanded(
                child: compact ? _buildCompactLayout() : _buildWideLayout(),
              ),
              _buildStatusBar(),
            ],
          ),
        );
      },
    );
  }

  Widget _buildTopBar({required bool compact}) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 12, 12, 8),
      child: Row(
        children: <Widget>[
          Expanded(
            child: TextField(
              controller: _searchController,
              textInputAction: TextInputAction.search,
              decoration: const InputDecoration(
                hintText: '搜索当前收件箱 / 订阅 / 随想',
                prefixIcon: Icon(Icons.search),
              ),
              onSubmitted: (_) => _applySearch(),
            ),
          ),
          const SizedBox(width: 8),
          if (compact)
            IconButton(
              tooltip: '保存网页',
              onPressed: _loading ? null : _composeBookmark,
              icon: const Icon(Icons.bookmark_add_outlined),
            )
          else ...<Widget>[
            TextButton.icon(
              onPressed: _loading ? null : _composeBookmark,
              icon: const Icon(Icons.bookmark_add_outlined),
              label: const Text('网页'),
            ),
            const SizedBox(width: 4),
            TextButton.icon(
              onPressed: _loading ? null : _composeNote,
              icon: const Icon(Icons.edit_note),
              label: const Text('随想'),
            ),
          ],
          const SizedBox(width: 8),
          OutlinedButton.icon(
            onPressed: _loading ? null : _refreshCurrentSelection,
            icon: const Icon(Icons.refresh),
            label: const Text('刷新'),
          ),
          const SizedBox(width: 8),
          PopupMenuButton<String>(
            tooltip: '更多操作',
            onSelected: _loading ? null : _handleTopBarAction,
            itemBuilder: (context) => const <PopupMenuEntry<String>>[
              PopupMenuItem<String>(
                value: 'bookmark',
                child: Text('保存网页…'),
              ),
              PopupMenuItem<String>(
                value: 'note',
                child: Text('新建随想…'),
              ),
              PopupMenuDivider(),
              PopupMenuItem<String>(
                value: 'notifications_global',
                child: Text('全局通知设置…'),
              ),
              PopupMenuItem<String>(
                value: 'notifications_center',
                child: Text('通知中心…'),
              ),
              PopupMenuDivider(),
              PopupMenuItem<String>(
                value: 'refresh_due',
                child: Text('刷新到期订阅'),
              ),
              PopupMenuDivider(),
              PopupMenuItem<String>(
                value: 'import_opml',
                child: Text('导入 OPML'),
              ),
              PopupMenuItem<String>(
                value: 'export_opml',
                child: Text('导出 OPML'),
              ),
            ],
            child: const Padding(
              padding: EdgeInsets.symmetric(horizontal: 8, vertical: 8),
              child: Icon(Icons.more_horiz),
            ),
          ),
        ],
      ),
    );
  }

  Future<void> _handleTopBarAction(String value) async {
    switch (value) {
      case 'bookmark':
        await _composeBookmark();
        return;
      case 'note':
        await _composeNote();
        return;
      case 'notifications_global':
        await _openGlobalNotificationSettings();
        return;
      case 'notifications_center':
        await _openNotificationCenter();
        return;
      case 'refresh_due':
        await _refreshDueFeeds();
        return;
      case 'import_opml':
        await _importOpml();
        return;
      case 'export_opml':
        await _exportOpml();
        return;
    }
  }

  Future<void> _applySearch() async {
    await _runBusy(() async {
      await _loadFeeds();
    });
  }

  Widget _buildStatusBar() {
    return Container(
      width: double.infinity,
      color: Theme.of(context).colorScheme.surfaceContainer,
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      child: Text(
        _statusText,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
    );
  }

  Widget _buildWideLayout() {
    return Row(
      children: <Widget>[
        SizedBox(width: 272, child: _buildFeedsPane()),
        const VerticalDivider(width: 1),
        SizedBox(width: 388, child: _buildItemsPane()),
        const VerticalDivider(width: 1),
        Expanded(child: _buildDetailPane()),
      ],
    );
  }

  Widget _buildCompactLayout() {
    return Column(
      children: <Widget>[
        SegmentedButton<int>(
          segments: const <ButtonSegment<int>>[
            ButtonSegment<int>(value: 0, label: Text('收件箱')),
            ButtonSegment<int>(value: 1, label: Text('条目')),
            ButtonSegment<int>(value: 2, label: Text('详情')),
          ],
          selected: <int>{_compactTabIndex},
          onSelectionChanged: (selection) {
            setState(() {
              _compactTabIndex = selection.first;
            });
          },
        ),
        Expanded(
          child: IndexedStack(
            index: _compactTabIndex,
            children: <Widget>[
              _buildFeedsPane(),
              _buildItemsPane(),
              _buildDetailPane(),
            ],
          ),
        ),
        NavigationBar(
          selectedIndex: _compactTabIndex,
          destinations: const <Widget>[
            NavigationDestination(icon: Icon(Icons.inbox), label: '收件箱'),
            NavigationDestination(icon: Icon(Icons.list), label: '条目'),
            NavigationDestination(icon: Icon(Icons.article), label: '详情'),
          ],
          onDestinationSelected: (index) {
            setState(() {
              _compactTabIndex = index;
            });
          },
        ),
      ],
    );
  }

  Widget _buildFeedsPane() {
    final feedCards = _feeds.map((feed) {
      final selected = feed.id == _selectedFeed?.id;
      return Padding(
        padding: const EdgeInsets.only(bottom: 8),
        child: _FeedCard(
          feed: feed,
          selected: selected,
          onTap: () => _selectFeed(feed),
          onEdit: () => _openFeedEditor(feed),
          onDelete: () => _confirmDeleteFeed(feed),
          onNotificationSettings: () => _openFeedNotificationSettings(feed),
          onRefreshSettings: () => _openFeedRefreshSettings(feed),
          onResetRefreshSettings: () => _resetFeedRefreshSettings(feed),
        ),
      );
    }).toList(growable: false);

    return ListView(
      padding: const EdgeInsets.fromLTRB(12, 12, 12, 18),
      children: <Widget>[
        _buildScopeSection(),
        const SizedBox(height: 12),
        if (_groups.isNotEmpty) ...<Widget>[
          _buildGroupSection(),
          const SizedBox(height: 12),
        ],
        _buildSubscriptionComposer(),
        const SizedBox(height: 12),
        if (_feeds.isEmpty)
          Center(
            child: Padding(
              padding: const EdgeInsets.all(24),
              child: Card(
                child: Padding(
                  padding: const EdgeInsets.all(20),
                  child: Text(
                    '还没有订阅。把网址放在左侧直接添加。',
                    style: Theme.of(context).textTheme.bodyLarge,
                  ),
                ),
              ),
            ),
          )
        else
          ...feedCards,
      ],
    );
  }

  Widget _buildGroupSection() {
    return Container(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerLowest,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(
          color: Theme.of(context).colorScheme.outlineVariant.withOpacity(0.45),
        ),
      ),
      padding: const EdgeInsets.all(10),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            '分类',
            style: Theme.of(context).textTheme.titleSmall,
          ),
          const SizedBox(height: 8),
          ..._groups.map((group) {
            final count = _feeds
                .where((feed) => feed.groups.any((entry) => entry.id == group.id))
                .length;
            return Padding(
              padding: const EdgeInsets.only(bottom: 6),
              child: Row(
                children: <Widget>[
                  Expanded(
                    child: _ScopeTile(
                      icon: Icons.folder_outlined,
                      title: group.name,
                      count: count,
                      selected: _selectedSelectionID == _groupSelectionID(group.id),
                      onTap: () => _selectGroup(group),
                    ),
                  ),
                  const SizedBox(width: 6),
                  PopupMenuButton<String>(
                    tooltip: '分类操作',
                    onSelected: (value) {
                      if (value == 'refresh') {
                        _openGroupRefreshSettings(group);
                      } else if (value == 'reset_refresh') {
                        _resetGroupRefreshSettings(group);
                      }
                    },
                    itemBuilder: (context) => const <PopupMenuEntry<String>>[
                      PopupMenuItem<String>(
                        value: 'refresh',
                        child: Text('自动刷新…'),
                      ),
                      PopupMenuItem<String>(
                        value: 'reset_refresh',
                        child: Text('恢复默认刷新'),
                      ),
                    ],
                    child: Icon(
                      Icons.more_horiz,
                      color: Theme.of(context).colorScheme.secondary,
                    ),
                  ),
                ],
              ),
            );
          }),
        ],
      ),
    );
  }

  Widget _buildScopeSection() {
    return Container(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerLowest,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(
          color: Theme.of(context).colorScheme.outlineVariant.withOpacity(0.45),
        ),
      ),
      padding: const EdgeInsets.all(10),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            '收件箱',
            style: Theme.of(context).textTheme.titleSmall,
          ),
          const SizedBox(height: 8),
          ...<Widget>[
            _ScopeTile(
              icon: Icons.inbox_outlined,
              title: '全部',
              count: _counts.all,
              selected: _selectedSelectionID == _inboxSelectionID,
              onTap: () => _selectSpecialScope(_inboxSelectionID),
            ),
            _ScopeTile(
              icon: Icons.mark_email_unread_outlined,
              title: '未读',
              count: _counts.unread,
              selected: _selectedSelectionID == _unreadSelectionID,
              onTap: () => _selectSpecialScope(_unreadSelectionID),
            ),
            _ScopeTile(
              icon: Icons.star_border,
              title: '星标',
              count: _counts.starred,
              selected: _selectedSelectionID == _starredSelectionID,
              onTap: () => _selectSpecialScope(_starredSelectionID),
            ),
            _ScopeTile(
              icon: Icons.bookmark_border,
              title: '稍后读',
              count: _counts.later,
              selected: _selectedSelectionID == _laterSelectionID,
              onTap: () => _selectSpecialScope(_laterSelectionID),
            ),
            _ScopeTile(
              icon: Icons.note_alt_outlined,
              title: '随想',
              count: _counts.notes,
              selected: _selectedSelectionID == _notesSelectionID,
              onTap: () => _selectSpecialScope(_notesSelectionID),
            ),
            _ScopeTile(
              icon: Icons.archive_outlined,
              title: '归档',
              count: _counts.archive,
              selected: _selectedSelectionID == _archiveSelectionID,
              onTap: () => _selectSpecialScope(_archiveSelectionID),
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildSubscriptionComposer() {
    return Container(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerLowest,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(
          color: Theme.of(context).colorScheme.outlineVariant.withOpacity(0.45),
        ),
      ),
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              Text(
                '添加订阅',
                style: Theme.of(context).textTheme.titleSmall,
              ),
              const Spacer(),
            ],
          ),
          const SizedBox(height: 4),
          Wrap(
            spacing: 4,
            runSpacing: 4,
            children: <Widget>[
              TextButton.icon(
                onPressed: _loading ? null : _composeBookmark,
                icon: const Icon(Icons.bookmark_add_outlined),
                label: const Text('网页'),
              ),
              TextButton.icon(
                onPressed: _loading ? null : _composeNote,
                icon: const Icon(Icons.edit_note),
                label: const Text('随想'),
              ),
            ],
          ),
          const SizedBox(height: 8),
          TextField(
            controller: _urlController,
            decoration: const InputDecoration(
              labelText: 'Feed URL 或站点 URL',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 10),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: <Widget>[
              FilledButton.icon(
                onPressed: _loading ? null : _addFeedDirectly,
                icon: const Icon(Icons.add),
                label: const Text('智能订阅'),
              ),
              OutlinedButton.icon(
                onPressed: _loading ? null : _discoverAndAdd,
                icon: const Icon(Icons.travel_explore),
                label: const Text('站点发现'),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildItemsPane() {
    return Column(
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 12, 12, 8),
          child: Row(
            children: <Widget>[
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      _selectionTitle,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                    const SizedBox(height: 2),
                    Text(
                      _selectionSubtitle,
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                  ],
                ),
              ),
              if (_canLoadMoreItems()) ...<Widget>[
                Tooltip(
                  message: '继续加载更多条目',
                  child: TextButton.icon(
                    onPressed: _loading ? null : _loadMoreItems,
                    icon: const Icon(Icons.expand_more),
                    label: const Text('加载更多'),
                  ),
                ),
                const SizedBox(width: 8),
              ],
              IconButton(
                tooltip: '刷新当前视图',
                onPressed: _loading ? null : _refreshCurrentSelection,
                icon: const Icon(Icons.refresh),
              ),
            ],
          ),
        ),
        const Divider(height: 1),
        Expanded(
          child: _items.isEmpty
              ? Center(
                  child: Padding(
                    padding: const EdgeInsets.all(24),
                    child: Card(
                      child: Padding(
                        padding: const EdgeInsets.all(20),
                        child: Text(
                          _selectionEmptyMessage,
                          style: Theme.of(context).textTheme.bodyLarge,
                        ),
                      ),
                    ),
                  ),
                )
              : ListView.separated(
                  padding: const EdgeInsets.fromLTRB(12, 12, 12, 18),
                  itemCount: _items.length,
                  separatorBuilder: (_, __) => const SizedBox(height: 8),
                  itemBuilder: (context, index) {
                    final item = _items[index];
                    return _ItemCard(
                      item: item,
                      selected: item.id == _selectedItem?.id,
                      onTap: () => _selectItem(item),
                    );
                  },
                ),
        ),
      ],
    );
  }

  Widget _buildDetailPane() {
    final item = _selectedItem;
    if (item == null) {
      return const Center(child: Text('请选择条目查看详情'));
    }

    return SingleChildScrollView(
      padding: const EdgeInsets.all(14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            item.title,
            style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                  height: 1.08,
                ),
          ),
          const SizedBox(height: 6),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: <Widget>[
              _Badge(label: item.kind.toUpperCase()),
              _Badge(label: item.sourceKind.toUpperCase()),
              if (item.isRead) const _Badge(label: 'READ'),
              if (item.isStarred) const _Badge(label: 'STARRED'),
              if (item.isSavedForLater) const _Badge(label: 'LATER'),
              if (item.isArchived) const _Badge(label: 'ARCHIVE'),
            ],
          ),
          const SizedBox(height: 8),
          Text(item.publishedAt ?? '未提供发布时间'),
          if (item.canonicalUrl != null || item.sourceURL != null) ...<Widget>[
            const SizedBox(height: 3),
            SelectableText(item.canonicalUrl ?? item.sourceURL ?? ''),
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: <Widget>[
                if (item.canonicalUrl != null || item.sourceURL != null)
                  OutlinedButton.icon(
                    onPressed: _loading
                        ? null
                        : () async {
                            await Clipboard.setData(
                              ClipboardData(
                                text: item.canonicalUrl ?? item.sourceURL ?? '',
                              ),
                            );
                            _showSnack('链接已复制');
                          },
                    icon: const Icon(Icons.copy, size: 18),
                    label: const Text('复制链接'),
                  ),
              ],
            ),
          ],
          const SizedBox(height: 12),
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: <Widget>[
              FilterChip(
                label: const Text('已读'),
                selected: item.isRead,
                onSelected: _loading ? null : (_) => _toggleItemRead(item),
              ),
              FilterChip(
                label: const Text('星标'),
                selected: item.isStarred,
                onSelected: _loading ? null : (_) => _toggleItemStar(item),
              ),
              FilterChip(
                label: const Text('稍后读'),
                selected: item.isSavedForLater,
                onSelected: _loading ? null : (_) => _toggleItemLater(item),
              ),
              FilterChip(
                label: const Text('归档'),
                selected: item.isArchived,
                onSelected: _loading ? null : (_) => _toggleItemArchive(item),
              ),
              FilterChip(
                label: const Text('抓取全文'),
                selected: false,
                onSelected:
                    _loading ? null : (_) => _fetchFullTextForSelectedItem(),
              ),
            ],
          ),
          const SizedBox(height: 12),
          _EntryBody(detail: item),
        ],
      ),
    );
  }
}

class _FeedCard extends StatelessWidget {
  const _FeedCard({
    required this.feed,
    required this.selected,
    required this.onTap,
    required this.onEdit,
    required this.onDelete,
    required this.onNotificationSettings,
    required this.onRefreshSettings,
    required this.onResetRefreshSettings,
  });

  final FeedModel feed;
  final bool selected;
  final VoidCallback onTap;
  final VoidCallback onEdit;
  final VoidCallback onDelete;
  final VoidCallback onNotificationSettings;
  final VoidCallback onRefreshSettings;
  final VoidCallback onResetRefreshSettings;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final surface = selected
        ? colorScheme.primaryContainer.withOpacity(0.45)
        : colorScheme.surface;

    return Material(
      color: surface,
      borderRadius: BorderRadius.circular(14),
      elevation: selected ? 1 : 0,
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(14),
        child: Container(
          padding: const EdgeInsets.all(10),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(14),
            border: Border.all(
              color: selected
                  ? colorScheme.primary.withOpacity(0.30)
                  : colorScheme.outlineVariant.withOpacity(0.32),
            ),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Container(
                width: 32,
                height: 32,
                decoration: BoxDecoration(
                  color: selected
                      ? colorScheme.primary.withOpacity(0.14)
                      : colorScheme.primaryContainer.withOpacity(0.55),
                  borderRadius: BorderRadius.circular(10),
                ),
                alignment: Alignment.center,
                clipBehavior: Clip.antiAlias,
                child: feed.iconUrl == null
                    ? Text(
                        feed.title.isNotEmpty
                            ? feed.title[0].toUpperCase()
                            : '?',
                        style: TextStyle(
                          color: selected
                              ? colorScheme.primary
                              : colorScheme.onPrimaryContainer,
                          fontWeight: FontWeight.w700,
                          fontSize: 12,
                        ),
                      )
                    : Image.network(
                        feed.iconUrl!,
                        fit: BoxFit.cover,
                        width: 32,
                        height: 32,
                        errorBuilder: (context, error, stackTrace) {
                          return Text(
                            feed.title.isNotEmpty
                                ? feed.title[0].toUpperCase()
                                : '?',
                            style: TextStyle(
                              color: selected
                                  ? colorScheme.primary
                                  : colorScheme.onPrimaryContainer,
                              fontWeight: FontWeight.w700,
                              fontSize: 12,
                            ),
                          );
                        },
                      ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      feed.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.titleSmall?.copyWith(
                            fontWeight: FontWeight.w600,
                          ),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      feed.siteUrl ?? feed.feedUrl,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                    const SizedBox(height: 6),
                    Wrap(
                      spacing: 6,
                      runSpacing: 6,
                      children: <Widget>[
                        _Badge(label: feed.feedType.toUpperCase()),
                        if (feed.siteUrl != null) const _Badge(label: 'SITE'),
                        ...feed.groups
                            .take(2)
                            .map((group) => _Badge(label: group.name)),
                        if (feed.groups.length > 2)
                          _Badge(label: '+${feed.groups.length - 2}'),
                      ],
                    ),
                  ],
                ),
              ),
              PopupMenuButton<String>(
                tooltip: '订阅操作',
                onSelected: (value) {
                  if (value == 'edit') {
                    onEdit();
                  } else if (value == 'delete') {
                    onDelete();
                  } else if (value == 'notifications') {
                    onNotificationSettings();
                  } else if (value == 'refresh') {
                    onRefreshSettings();
                  } else if (value == 'reset_refresh') {
                    onResetRefreshSettings();
                  }
                },
                itemBuilder: (context) => const <PopupMenuEntry<String>>[
                  PopupMenuItem<String>(
                    value: 'edit',
                    child: Text('编辑订阅…'),
                  ),
                  PopupMenuItem<String>(
                    value: 'notifications',
                    child: Text('通知设置…'),
                  ),
                  PopupMenuItem<String>(
                    value: 'refresh',
                    child: Text('自动刷新…'),
                  ),
                  PopupMenuItem<String>(
                    value: 'reset_refresh',
                    child: Text('恢复默认刷新'),
                  ),
                  PopupMenuDivider(),
                  PopupMenuItem<String>(
                    value: 'delete',
                    child: Text('删除订阅…'),
                  ),
                ],
                child: Padding(
                  padding: const EdgeInsets.only(left: 6),
                  child: Icon(
                    Icons.more_horiz,
                    size: 18,
                    color:
                        selected ? colorScheme.primary : colorScheme.secondary,
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ItemCard extends StatelessWidget {
  const _ItemCard({
    required this.item,
    required this.selected,
    required this.onTap,
  });

  final ItemModel item;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final surface = selected
        ? colorScheme.primaryContainer.withOpacity(0.42)
        : colorScheme.surface;

    return Material(
      color: surface,
      borderRadius: BorderRadius.circular(14),
      elevation: selected ? 1 : 0,
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(14),
        child: Container(
          padding: const EdgeInsets.all(10),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(14),
            border: Border.all(
              color: selected
                  ? colorScheme.primary.withOpacity(0.30)
                  : colorScheme.outlineVariant.withOpacity(0.32),
            ),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Icon(
                item.kind == 'note'
                    ? Icons.note_alt_outlined
                    : item.kind == 'bookmark'
                        ? Icons.bookmark_outline
                        : item.isRead
                            ? Icons.mark_email_read_outlined
                            : Icons.mark_email_unread,
                size: 18,
                color: item.kind == 'note'
                    ? colorScheme.tertiary
                    : item.isRead
                        ? colorScheme.secondary
                        : colorScheme.primary,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      item.title,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.titleSmall?.copyWith(
                            fontWeight:
                                item.isRead ? FontWeight.w500 : FontWeight.w700,
                          ),
                    ),
                    if ((item.summaryPreview ?? item.summary ?? '')
                        .trim()
                        .isNotEmpty) ...[
                      const SizedBox(height: 3),
                      Text(
                        item.summaryPreview ?? item.summary ?? '',
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                    ],
                    const SizedBox(height: 4),
                    Wrap(
                      spacing: 6,
                      runSpacing: 6,
                      children: <Widget>[
                        _Badge(label: item.kind.toUpperCase()),
                        _Badge(label: item.sourceKind.toUpperCase()),
                        if ((Uri.tryParse(item.canonicalUrl ?? '')?.host ?? '')
                            .isNotEmpty)
                          _Badge(
                              label:
                                  Uri.tryParse(item.canonicalUrl ?? '')!.host),
                        if (item.isRead) const _Badge(label: 'READ'),
                        if (item.isStarred) const _Badge(label: 'STARRED'),
                        if (item.isSavedForLater) const _Badge(label: 'LATER'),
                        if (item.isArchived) const _Badge(label: 'ARCHIVE'),
                      ],
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 12),
              Column(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: <Widget>[
                  Text(
                    item.publishedAt ?? '未提供发布时间',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  const SizedBox(height: 10),
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      Icon(
                        item.isStarred ? Icons.star : Icons.star_border,
                        size: 17,
                        color: item.isStarred
                            ? colorScheme.tertiary
                            : colorScheme.secondary,
                      ),
                      const SizedBox(width: 6),
                      Icon(
                        item.isSavedForLater
                            ? Icons.bookmark
                            : Icons.bookmark_border,
                        size: 17,
                        color: item.isSavedForLater
                            ? colorScheme.primary
                            : colorScheme.secondary,
                      ),
                    ],
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _Badge extends StatelessWidget {
  const _Badge({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: colorScheme.surfaceContainerHighest.withOpacity(0.72),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        label,
        style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: colorScheme.onSurfaceVariant,
              fontWeight: FontWeight.w600,
              letterSpacing: 0.2,
            ),
      ),
    );
  }
}

class _NotificationSettingsDialog extends StatefulWidget {
  const _NotificationSettingsDialog.feed({
    required this.title,
    required this.initialSettings,
  })  : globalMode = false,
        initialGlobalSettings = null;

  _NotificationSettingsDialog.global({
    required GlobalNotificationSettings initialGlobalSettings,
  })  : globalMode = true,
        title = '全局通知设置',
        initialSettings = initialGlobalSettings.defaultFeedSettings,
        initialGlobalSettings = initialGlobalSettings;

  final bool globalMode;
  final String title;
  final NotificationSettings initialSettings;
  final GlobalNotificationSettings? initialGlobalSettings;

  @override
  State<_NotificationSettingsDialog> createState() =>
      _NotificationSettingsDialogState();
}

class _NotificationSettingsDialogState
    extends State<_NotificationSettingsDialog> {
  late NotificationSettings _settings;
  late bool _backgroundRefreshEnabled;
  late final TextEditingController _backgroundIntervalController;
  late final TextEditingController _globalDigestIntervalController;
  late final TextEditingController _globalDigestMaxController;
  late final TextEditingController _minIntervalController;
  late final TextEditingController _digestIntervalController;
  late final TextEditingController _digestMaxController;
  late final TextEditingController _quietStartController;
  late final TextEditingController _quietEndController;
  late final TextEditingController _includeController;
  late final TextEditingController _excludeController;
  late bool _enabled;
  late bool _quietEnabled;
  late bool _highPriority;
  late bool _globalDigestEnabled;
  late NotificationMode _mode;
  bool _saving = false;
  String? _validationError;

  @override
  void initState() {
    super.initState();
    _settings = widget.initialSettings;
    _backgroundRefreshEnabled =
        widget.initialGlobalSettings?.backgroundRefreshEnabled ?? true;
    _backgroundIntervalController = TextEditingController(
      text: widget.initialGlobalSettings?.backgroundRefreshIntervalMinutes
              .toString() ??
          '15',
    );
    _globalDigestIntervalController = TextEditingController(
      text: (widget.initialGlobalSettings?.digestPolicy.intervalMinutes ??
              _settings.digestPolicy.intervalMinutes)
          .toString(),
    );
    _globalDigestMaxController = TextEditingController(
      text: (widget.initialGlobalSettings?.digestPolicy.maxItems ??
              _settings.digestPolicy.maxItems)
          .toString(),
    );
    _globalDigestEnabled = widget.initialGlobalSettings?.digestPolicy.enabled ??
        _settings.digestPolicy.enabled;
    _minIntervalController = TextEditingController(
        text: _settings.minimumIntervalMinutes.toString());
    _digestIntervalController = TextEditingController(
        text: _settings.digestPolicy.intervalMinutes.toString());
    _digestMaxController =
        TextEditingController(text: _settings.digestPolicy.maxItems.toString());
    _quietStartController = TextEditingController(
      text: formatMinuteOfDay(_settings.quietHours.startMinute),
    );
    _quietEndController = TextEditingController(
      text: formatMinuteOfDay(_settings.quietHours.endMinute),
    );
    _includeController =
        TextEditingController(text: _settings.keywordInclude.join(', '));
    _excludeController =
        TextEditingController(text: _settings.keywordExclude.join(', '));
    _enabled = _settings.enabled;
    _quietEnabled = _settings.quietHours.enabled;
    _highPriority = _settings.highPriority;
    _mode = _settings.mode;
  }

  @override
  void dispose() {
    _backgroundIntervalController.dispose();
    _globalDigestIntervalController.dispose();
    _globalDigestMaxController.dispose();
    _minIntervalController.dispose();
    _digestIntervalController.dispose();
    _digestMaxController.dispose();
    _quietStartController.dispose();
    _quietEndController.dispose();
    _includeController.dispose();
    _excludeController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final title = widget.globalMode ? widget.title : widget.title;
    return AlertDialog(
      title: Text(title),
      content: SizedBox(
        width: 760,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              if (_validationError != null) ...<Widget>[
                Text(
                  _validationError!,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
                const SizedBox(height: 8),
              ],
              if (widget.globalMode) ...<Widget>[
                Text(
                  '全局刷新',
                  style: Theme.of(context).textTheme.titleSmall,
                ),
                const SizedBox(height: 8),
                SwitchListTile.adaptive(
                  contentPadding: EdgeInsets.zero,
                  value: _backgroundRefreshEnabled,
                  onChanged: (value) {
                    setState(() {
                      _backgroundRefreshEnabled = value;
                    });
                  },
                  title: const Text('启用后台刷新'),
                ),
                TextField(
                  controller: _backgroundIntervalController,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(
                    labelText: '后台刷新间隔（分钟）',
                  ),
                ),
                const SizedBox(height: 12),
                Text(
                  '全局摘要',
                  style: Theme.of(context).textTheme.titleSmall,
                ),
                const SizedBox(height: 8),
                SwitchListTile.adaptive(
                  contentPadding: EdgeInsets.zero,
                  value: _globalDigestEnabled,
                  onChanged: (value) {
                    setState(() {
                      _globalDigestEnabled = value;
                    });
                  },
                  title: const Text('启用摘要模式'),
                ),
                TextField(
                  controller: _globalDigestIntervalController,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(
                    labelText: '摘要间隔（分钟）',
                  ),
                ),
                const SizedBox(height: 8),
                TextField(
                  controller: _globalDigestMaxController,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(
                    labelText: '单个摘要最多条目',
                  ),
                ),
                const SizedBox(height: 16),
                Text(
                  '默认订阅通知',
                  style: Theme.of(context).textTheme.titleSmall,
                ),
                const SizedBox(height: 8),
              ],
              SwitchListTile.adaptive(
                contentPadding: EdgeInsets.zero,
                value: _enabled,
                onChanged: (value) {
                  setState(() {
                    _enabled = value;
                  });
                },
                title: const Text('启用通知'),
              ),
              DropdownButtonFormField<NotificationMode>(
                value: _mode,
                decoration: const InputDecoration(labelText: '通知模式'),
                items: const <DropdownMenuItem<NotificationMode>>[
                  DropdownMenuItem(
                    value: NotificationMode.immediate,
                    child: Text('即时'),
                  ),
                  DropdownMenuItem(
                    value: NotificationMode.digest,
                    child: Text('摘要'),
                  ),
                ],
                onChanged: (value) {
                  if (value == null) return;
                  setState(() {
                    _mode = value;
                  });
                },
              ),
              const SizedBox(height: 8),
              if (_mode == NotificationMode.digest) ...<Widget>[
                TextField(
                  controller: _digestIntervalController,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(
                    labelText: '摘要间隔（分钟）',
                  ),
                ),
                const SizedBox(height: 8),
                TextField(
                  controller: _digestMaxController,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(
                    labelText: '摘要最多条目',
                  ),
                ),
                const SizedBox(height: 8),
              ],
              SwitchListTile.adaptive(
                contentPadding: EdgeInsets.zero,
                value: _quietEnabled,
                onChanged: (value) {
                  setState(() {
                    _quietEnabled = value;
                  });
                },
                title: const Text('静默时段'),
              ),
              if (_quietEnabled) ...<Widget>[
                Row(
                  children: <Widget>[
                    Expanded(
                      child: TextField(
                        controller: _quietStartController,
                        decoration:
                            const InputDecoration(labelText: '开始 HH:MM'),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: TextField(
                        controller: _quietEndController,
                        decoration:
                            const InputDecoration(labelText: '结束 HH:MM'),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
              ],
              TextField(
                controller: _minIntervalController,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(
                  labelText: '同一 Feed 最小通知间隔（分钟）',
                ),
              ),
              const SizedBox(height: 8),
              SwitchListTile.adaptive(
                contentPadding: EdgeInsets.zero,
                value: _highPriority,
                onChanged: (value) {
                  setState(() {
                    _highPriority = value;
                  });
                },
                title: const Text('高优先级 Feed'),
              ),
              TextField(
                controller: _includeController,
                decoration: const InputDecoration(
                  labelText: '关键词包含（逗号分隔）',
                ),
              ),
              const SizedBox(height: 8),
              TextField(
                controller: _excludeController,
                decoration: const InputDecoration(
                  labelText: '关键词排除（逗号分隔）',
                ),
              ),
            ],
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: _saving ? null : () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        FilledButton(
          onPressed: _saving ? null : _save,
          child: const Text('保存'),
        ),
      ],
    );
  }

  Future<void> _save() async {
    final validationErrors = <String?>[
      if (widget.globalMode) ...[
        validatePositiveIntInput(
          _backgroundIntervalController.text,
          fieldLabel: '后台刷新间隔',
        ),
        validatePositiveIntInput(
          _globalDigestIntervalController.text,
          fieldLabel: '摘要间隔',
        ),
        validatePositiveIntInput(
          _globalDigestMaxController.text,
          fieldLabel: '单个摘要最多条目',
        ),
      ],
      if (_mode == NotificationMode.digest) ...[
        validatePositiveIntInput(
          _digestIntervalController.text,
          fieldLabel: '摘要间隔',
        ),
        validatePositiveIntInput(
          _digestMaxController.text,
          fieldLabel: '摘要最多条目',
        ),
      ],
      validatePositiveIntInput(
        _minIntervalController.text,
        fieldLabel: '同一 Feed 最小通知间隔',
      ),
      validateMinuteOfDayInput(
        _quietStartController.text,
        fieldLabel: '静默时段开始时间',
      ),
      validateMinuteOfDayInput(
        _quietEndController.text,
        fieldLabel: '静默时段结束时间',
      ),
    ].whereType<String>().toList(growable: false);

    if (validationErrors.isNotEmpty) {
      setState(() {
        _validationError = validationErrors.first;
      });
      return;
    }

    setState(() {
      _saving = true;
      _validationError = null;
    });

    try {
      final digestPolicy = DigestPolicy(
        enabled: _mode == NotificationMode.digest,
        intervalMinutes: parsePositiveIntInput(
          _mode == NotificationMode.digest
              ? _digestIntervalController.text
              : _globalDigestIntervalController.text,
          fallback: 60,
        ),
        maxItems: parsePositiveIntInput(
          _mode == NotificationMode.digest
              ? _digestMaxController.text
              : _globalDigestMaxController.text,
          fallback: 20,
        ),
      );

      final settings = NotificationSettings(
        enabled: _enabled,
        mode: _mode,
        digestPolicy: digestPolicy,
        quietHours: QuietHours(
          enabled: _quietEnabled,
          startMinute: parseMinuteOfDayInput(_quietStartController.text),
          endMinute: parseMinuteOfDayInput(_quietEndController.text),
        ),
        minimumIntervalMinutes: parsePositiveIntInput(
          _minIntervalController.text,
          fallback: 15,
        ),
        highPriority: _highPriority,
        keywordInclude: splitKeywords(_includeController.text),
        keywordExclude: splitKeywords(_excludeController.text),
      );

      if (!mounted) {
        return;
      }
      if (widget.globalMode) {
        final global = GlobalNotificationSettings(
          backgroundRefreshEnabled: _backgroundRefreshEnabled,
          backgroundRefreshIntervalMinutes: parsePositiveIntInput(
            _backgroundIntervalController.text,
            fallback: 15,
          ),
          digestPolicy: DigestPolicy(
            enabled: _globalDigestEnabled,
            intervalMinutes: parsePositiveIntInput(
              _globalDigestIntervalController.text,
              fallback: 60,
            ),
            maxItems: parsePositiveIntInput(
              _globalDigestMaxController.text,
              fallback: 20,
            ),
          ),
          defaultFeedSettings: settings,
        );
        Navigator.of(context).pop(global);
      } else {
        Navigator.of(context).pop(settings);
      }
    } finally {
      if (mounted) {
        setState(() {
          _saving = false;
        });
      }
    }
  }
}

class _RefreshSettingsDialog extends StatefulWidget {
  const _RefreshSettingsDialog({
    required this.title,
    required this.initialSettings,
  });

  final String title;
  final RefreshSettings initialSettings;

  @override
  State<_RefreshSettingsDialog> createState() => _RefreshSettingsDialogState();
}

class _RefreshSettingsDialogState extends State<_RefreshSettingsDialog> {
  late bool _enabled;
  late final TextEditingController _intervalController;
  String? _validationError;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    _enabled = widget.initialSettings.enabled;
    _intervalController = TextEditingController(
      text: widget.initialSettings.intervalMinutes.toString(),
    );
  }

  @override
  void dispose() {
    _intervalController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text(widget.title),
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            if (_validationError != null) ...<Widget>[
              Text(
                _validationError!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
              const SizedBox(height: 8),
            ],
            SwitchListTile.adaptive(
              contentPadding: EdgeInsets.zero,
              value: _enabled,
              onChanged: (value) {
                setState(() {
                  _enabled = value;
                });
              },
              title: const Text('启用自动刷新'),
            ),
            TextField(
              controller: _intervalController,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(
                labelText: '刷新间隔（分钟）',
              ),
            ),
            const SizedBox(height: 8),
            Text(
              '刷新间隔决定这个分类或订阅被后台轮询的节奏。',
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: _saving ? null : () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        FilledButton(
          onPressed: _saving ? null : _save,
          child: const Text('保存'),
        ),
      ],
    );
  }

  Future<void> _save() async {
    final validationError = validatePositiveIntInput(
      _intervalController.text,
      fieldLabel: '刷新间隔',
    );
    if (validationError != null) {
      setState(() {
        _validationError = validationError;
      });
      return;
    }

    setState(() {
      _saving = true;
      _validationError = null;
    });

    try {
      final settings = RefreshSettings(
        enabled: _enabled,
        intervalMinutes: parsePositiveIntInput(
          _intervalController.text,
          fallback: 15,
        ),
      );
      if (!mounted) {
        return;
      }
      Navigator.of(context).pop(settings);
    } finally {
      if (mounted) {
        setState(() {
          _saving = false;
        });
      }
    }
  }
}

class NotificationCenterDialog extends StatefulWidget {
  const NotificationCenterDialog({required this.backend, super.key});

  final ReaderBackend backend;

  @override
  State<NotificationCenterDialog> createState() =>
      _NotificationCenterDialogState();
}

class _NotificationCenterDialogState extends State<NotificationCenterDialog> {
  List<NotificationEventModel> _events = const <NotificationEventModel>[];
  bool _loading = false;
  String _status = '正在加载通知...';

  @override
  void initState() {
    super.initState();
    _reload();
  }

  Future<void> _reload() async {
    setState(() {
      _loading = true;
    });
    try {
      final events = await widget.backend.listPendingNotificationEvents();
      if (!mounted) return;
      setState(() {
        _events = events;
        _status = events.isEmpty ? '没有待投递通知' : '共有 ${events.length} 条待投递通知';
      });
    } catch (error) {
      if (!mounted) return;
      setState(() {
        _status = '加载通知失败: $error';
      });
    } finally {
      if (mounted) {
        setState(() {
          _loading = false;
        });
      }
    }
  }

  Future<void> _acknowledgeAll() async {
    if (_events.isEmpty) {
      return;
    }
    setState(() {
      _loading = true;
    });
    try {
      final acknowledged = await widget.backend.acknowledgeNotificationEvents(
        _events.map((event) => event.id).toList(growable: false),
      );
      if (!mounted) return;
      setState(() {
        _status = '已标记 $acknowledged 条通知为已送达';
      });
      await _reload();
    } catch (error) {
      if (!mounted) return;
      setState(() {
        _status = '确认通知失败: $error';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('通知中心'),
      content: SizedBox(
        width: 720,
        height: 520,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              children: <Widget>[
                Expanded(child: Text(_status)),
                IconButton(
                  onPressed: _loading ? null : _reload,
                  icon: const Icon(Icons.refresh),
                  tooltip: '刷新',
                ),
                TextButton(
                  onPressed: _loading ? null : _acknowledgeAll,
                  child: const Text('全部标记已送达'),
                ),
              ],
            ),
            const Divider(height: 16),
            Expanded(
              child: _events.isEmpty
                  ? const Center(child: Text('暂无待处理通知'))
                  : ListView.separated(
                      itemCount: _events.length,
                      separatorBuilder: (_, __) => const SizedBox(height: 8),
                      itemBuilder: (context, index) {
                        final event = _events[index];
                        return Card(
                          child: Padding(
                            padding: const EdgeInsets.all(12),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: <Widget>[
                                Text(
                                  event.title,
                                  style: Theme.of(context).textTheme.titleSmall,
                                ),
                                const SizedBox(height: 4),
                                Text(
                                  event.body,
                                  maxLines: 3,
                                  overflow: TextOverflow.ellipsis,
                                ),
                                const SizedBox(height: 8),
                                Wrap(
                                  spacing: 6,
                                  runSpacing: 6,
                                  children: <Widget>[
                                    _Badge(
                                        label: event.mode.name.toUpperCase()),
                                    _Badge(
                                      label: event.deliveryState.name
                                          .toUpperCase(),
                                    ),
                                    _Badge(label: event.reason),
                                    if (event.feedId != null)
                                      _Badge(label: event.feedId!),
                                  ],
                                ),
                              ],
                            ),
                          ),
                        );
                      },
                    ),
            ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('关闭'),
        ),
      ],
    );
  }
}

class _FeedEditDraft {
  const _FeedEditDraft({
    required this.title,
    required this.groupId,
    required this.newGroupName,
    required this.autoFullText,
  });

  final String title;
  final String? groupId;
  final String newGroupName;
  final bool autoFullText;
}

class _FeedEditDialog extends StatefulWidget {
  const _FeedEditDialog({
    required this.feed,
    required this.groups,
  });

  final FeedModel feed;
  final List<FeedGroupModel> groups;

  @override
  State<_FeedEditDialog> createState() => _FeedEditDialogState();
}

class _FeedEditDialogState extends State<_FeedEditDialog> {
  late final TextEditingController _titleController;
  late final TextEditingController _newGroupController;
  String? _selectedGroupId;
  late bool _autoFullText;

  @override
  void initState() {
    super.initState();
    _titleController = TextEditingController(text: widget.feed.title);
    _newGroupController = TextEditingController();
    _selectedGroupId =
        widget.feed.groups.isNotEmpty ? widget.feed.groups.first.id : null;
    _autoFullText = widget.feed.autoFullText;
  }

  @override
  void dispose() {
    _titleController.dispose();
    _newGroupController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final allGroups = widget.groups.toList(growable: false)
      ..sort((lhs, rhs) =>
          lhs.name.toLowerCase().compareTo(rhs.name.toLowerCase()));

    return AlertDialog(
      title: Text('编辑《${widget.feed.title}》'),
      content: SizedBox(
        width: 560,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              TextField(
                controller: _titleController,
                decoration: const InputDecoration(
                  labelText: '订阅标题',
                ),
              ),
              const SizedBox(height: 12),
              DropdownButtonFormField<String?>(
                value: _selectedGroupId,
                decoration: const InputDecoration(
                  labelText: '分类',
                ),
                items: <DropdownMenuItem<String?>>[
                  const DropdownMenuItem<String?>(
                    value: null,
                    child: Text('未分组'),
                  ),
                  ...allGroups.map(
                    (group) => DropdownMenuItem<String?>(
                      value: group.id,
                      child: Text(group.name),
                    ),
                  ),
                ],
                onChanged: (value) {
                  setState(() {
                    _selectedGroupId = value;
                  });
                },
              ),
              const SizedBox(height: 12),
              TextField(
                controller: _newGroupController,
                decoration: const InputDecoration(
                  labelText: '新建分类（可选）',
                ),
              ),
              const SizedBox(height: 12),
              CheckboxListTile(
                value: _autoFullText,
                controlAffinity: ListTileControlAffinity.leading,
                contentPadding: EdgeInsets.zero,
                title: const Text('自动抓取全文'),
                subtitle: const Text('选中条目时自动加载全文内容'),
                onChanged: (value) {
                  setState(() {
                    _autoFullText = value ?? true;
                  });
                },
              ),
              const SizedBox(height: 8),
              Text(
                '如果填写新分类，它会优先于上面的选择。',
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ],
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        FilledButton(
          onPressed: _save,
          child: const Text('保存'),
        ),
      ],
    );
  }

  void _save() {
    Navigator.of(context).pop(
      _FeedEditDraft(
        title: _titleController.text.trim(),
        groupId: _selectedGroupId,
        newGroupName: _newGroupController.text.trim(),
        autoFullText: _autoFullText,
      ),
    );
  }
}

String? validatePositiveIntInput(
  String value, {
  required String fieldLabel,
  int minimum = 1,
}) {
  final parsed = int.tryParse(value.trim());
  if (parsed == null) {
    return '$fieldLabel 必须是整数';
  }
  if (parsed < minimum) {
    return '$fieldLabel 不能小于 $minimum';
  }
  return null;
}

String? validateMinuteOfDayInput(
  String value, {
  required String fieldLabel,
}) {
  final match = RegExp(r'^(\d{1,2}):(\d{2})$').firstMatch(value.trim());
  if (match == null) {
    return '$fieldLabel 必须是 HH:MM';
  }

  final hour = int.tryParse(match.group(1) ?? '');
  final minute = int.tryParse(match.group(2) ?? '');
  if (hour == null ||
      minute == null ||
      hour < 0 ||
      hour > 23 ||
      minute < 0 ||
      minute > 59) {
    return '$fieldLabel 必须是 00:00 到 23:59';
  }
  return null;
}

int parsePositiveIntInput(
  String value, {
  int fallback = 0,
  int minimum = 1,
}) {
  final parsed = int.tryParse(value.trim());
  if (parsed == null || parsed < minimum) {
    return fallback;
  }
  return parsed;
}

int parseMinuteOfDayInput(String value, {int fallback = 0}) {
  final error = validateMinuteOfDayInput(value, fieldLabel: '时间');
  if (error != null) {
    return fallback;
  }
  final match = RegExp(r'^(\d{1,2}):(\d{2})$').firstMatch(value.trim());
  final hour = int.tryParse(match?.group(1) ?? '') ?? 0;
  final minute = int.tryParse(match?.group(2) ?? '') ?? 0;
  return (hour.clamp(0, 23) * 60) + minute.clamp(0, 59);
}

String formatMinuteOfDay(int minuteOfDay) {
  final hours = (minuteOfDay ~/ 60).clamp(0, 23);
  final minutes = (minuteOfDay % 60).clamp(0, 59);
  return '${hours.toString().padLeft(2, '0')}:${minutes.toString().padLeft(2, '0')}';
}

List<String> splitKeywords(String value) {
  return value
      .split(',')
      .map((part) => part.trim())
      .where((part) => part.isNotEmpty)
      .toList(growable: false);
}

class _ScopeTile extends StatelessWidget {
  const _ScopeTile({
    required this.icon,
    required this.title,
    required this.count,
    required this.selected,
    required this.onTap,
  });

  final IconData icon;
  final String title;
  final int count;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final surface = selected
        ? colorScheme.primaryContainer.withOpacity(0.45)
        : colorScheme.surface;

    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Material(
        color: surface,
        borderRadius: BorderRadius.circular(14),
        elevation: selected ? 1 : 0,
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(14),
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(14),
              border: Border.all(
                color: selected
                    ? colorScheme.primary.withOpacity(0.30)
                    : colorScheme.outlineVariant.withOpacity(0.32),
              ),
            ),
            child: Row(
              children: <Widget>[
                Icon(icon,
                    color:
                        selected ? colorScheme.primary : colorScheme.secondary,
                    size: 18),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    title,
                    style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                          fontWeight:
                              selected ? FontWeight.w700 : FontWeight.w600,
                        ),
                  ),
                ),
                if (count > 0)
                  _Badge(label: '$count')
                else
                  const _Badge(label: '0'),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _EntryBody extends StatelessWidget {
  const _EntryBody({required this.detail});

  final ItemModel detail;

  @override
  Widget build(BuildContext context) {
    final body = detail.contentText?.trim();
    final html = detail.contentHTML?.trim();
    final summary = detail.summary?.trim();
    final preview = detail.summaryPreview?.trim();

    final content = (body != null && body.isNotEmpty)
        ? body
        : (html != null && html.isNotEmpty)
            ? html.replaceAll(RegExp(r'<[^>]+>'), ' ')
            : (summary != null && summary.isNotEmpty)
                ? summary
                : (preview != null && preview.isNotEmpty)
                    ? preview
                    : '该条目暂无正文内容';

    return SelectableText(
      content.split(RegExp(r'\s+')).where((part) => part.isNotEmpty).join(' '),
      style: Theme.of(context).textTheme.bodyLarge?.copyWith(height: 1.5),
    );
  }
}

enum _EntryComposerKind { bookmark, note }

class _BookmarkDraft {
  const _BookmarkDraft({
    required this.url,
    required this.title,
    this.note,
  });

  final String url;
  final String title;
  final String? note;
}

class _NoteDraft {
  const _NoteDraft({
    required this.title,
    required this.body,
  });

  final String title;
  final String body;
}

class _EntryComposerSheet extends StatefulWidget {
  const _EntryComposerSheet({required this.kind});

  final _EntryComposerKind kind;

  const _EntryComposerSheet.bookmark() : kind = _EntryComposerKind.bookmark;
  const _EntryComposerSheet.note() : kind = _EntryComposerKind.note;

  @override
  State<_EntryComposerSheet> createState() => _EntryComposerSheetState();
}

class _EntryComposerSheetState extends State<_EntryComposerSheet> {
  final TextEditingController _urlController = TextEditingController();
  final TextEditingController _titleController = TextEditingController();
  final TextEditingController _noteController = TextEditingController();

  @override
  void dispose() {
    _urlController.dispose();
    _titleController.dispose();
    _noteController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final bookmark = widget.kind == _EntryComposerKind.bookmark;
    return Dialog(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 640),
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                bookmark ? '保存网页' : '新建随想',
                style: Theme.of(context).textTheme.titleLarge,
              ),
              const SizedBox(height: 16),
              if (bookmark) ...<Widget>[
                TextField(
                  controller: _urlController,
                  onChanged: (_) => setState(() {}),
                  decoration: const InputDecoration(
                    labelText: 'URL',
                    hintText: 'https://example.com/article',
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: _titleController,
                  onChanged: (_) => setState(() {}),
                  decoration: const InputDecoration(
                    labelText: '标题',
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: _noteController,
                  onChanged: (_) => setState(() {}),
                  maxLines: 5,
                  decoration: const InputDecoration(
                    labelText: '备注（可选）',
                  ),
                ),
              ] else ...<Widget>[
                TextField(
                  controller: _titleController,
                  onChanged: (_) => setState(() {}),
                  decoration: const InputDecoration(
                    labelText: '标题',
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: _noteController,
                  onChanged: (_) => setState(() {}),
                  maxLines: 10,
                  decoration: const InputDecoration(
                    labelText: '正文',
                  ),
                ),
              ],
              const SizedBox(height: 18),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: <Widget>[
                  TextButton(
                    onPressed: () => Navigator.of(context).pop(),
                    child: const Text('取消'),
                  ),
                  const SizedBox(width: 8),
                  FilledButton(
                    onPressed: _isSaveDisabled ? null : _save,
                    child: const Text('保存'),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  bool get _isSaveDisabled {
    if (widget.kind == _EntryComposerKind.bookmark) {
      return _urlController.text.trim().isEmpty ||
          _titleController.text.trim().isEmpty;
    }
    return _noteController.text.trim().isEmpty;
  }

  void _save() {
    if (widget.kind == _EntryComposerKind.bookmark) {
      Navigator.of(context).pop(
        _BookmarkDraft(
          url: _urlController.text.trim(),
          title: _titleController.text.trim(),
          note: _noteController.text.trim().isEmpty
              ? null
              : _noteController.text.trim(),
        ),
      );
      return;
    }

    Navigator.of(context).pop(
      _NoteDraft(
        title: _titleController.text.trim().isEmpty
            ? '未命名随想'
            : _titleController.text.trim(),
        body: _noteController.text.trim(),
      ),
    );
  }
}
