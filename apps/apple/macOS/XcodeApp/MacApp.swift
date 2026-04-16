import SwiftUI

@main
struct InfoMatrixMacXcodeApp: App {
    @StateObject private var state: AppState
    @StateObject private var notificationCoordinator: NotificationDeliveryCoordinator

    init() {
        let dbPath = NativeReaderService.defaultDBPath()
        let service = NativeReaderService(dbPath: dbPath)
        _state = StateObject(
            wrappedValue: AppState(
                service: service
            )
        )
        _notificationCoordinator = StateObject(
            wrappedValue: NotificationDeliveryCoordinator(service: service)
        )
    }

    var body: some Scene {
        WindowGroup {
            ReaderShellView(state: state)
                .task {
                    notificationCoordinator.start()
                    await notificationCoordinator.syncPendingNotifications()
                }
        }
        .commands {
            CommandMenu("Reader") {
                Button("刷新") {
                    Task { await state.refreshSelectedFeed() }
                }
                .keyboardShortcut("r", modifiers: [.command])

                Divider()

                Button("全部条目") {
                    state.selectAllItemsScope()
                }
                .keyboardShortcut("1", modifiers: [.command])

                Button("归档条目") {
                    state.selectArchiveScope()
                }
                .keyboardShortcut("2", modifiers: [.command])

                Divider()

                Button("切换已读") {
                    state.toggleReadForSelectedItem()
                }
                .keyboardShortcut("m", modifiers: [.command, .shift])

                Button("切换星标") {
                    state.toggleStarForSelectedItem()
                }
                .keyboardShortcut("s", modifiers: [.command, .shift])

                Button("切换稍后读") {
                    state.toggleLaterForSelectedItem()
                }
                .keyboardShortcut("l", modifiers: [.command, .shift])

                Button("切换归档") {
                    state.toggleArchiveForSelectedItem()
                }
                .keyboardShortcut("a", modifiers: [.command, .shift])
            }
        }
    }
}
