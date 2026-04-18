import AppKit
import Foundation
import SwiftUI
#if canImport(InfoMatrixShell)
import InfoMatrixShell
#endif

final class InfoMatrixAppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) {
            NSApplication.shared.activate(ignoringOtherApps: true)
            NSApplication.shared.windows.first?.makeKeyAndOrderFront(nil)
            NSApplication.shared.windows.first?.orderFrontRegardless()
        }
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        NSApplication.shared.activate(ignoringOtherApps: true)
        NSApplication.shared.windows.first?.makeKeyAndOrderFront(nil)
    }
}

@main
struct InfoMatrixMacAppMain: App {
    @NSApplicationDelegateAdaptor(InfoMatrixAppDelegate.self) private var appDelegate
    @StateObject private var state: AppState
    @StateObject private var notificationCoordinator: NotificationDeliveryCoordinator

    init() {
        let dbPath = NativeReaderService.defaultDBPath()
        let service = NativeReaderService(dbPath: dbPath)
        let syncCoordinator = CloudKitSyncCoordinator(service: service)
        _state = StateObject(
            wrappedValue: AppState(
                service: service,
                syncCoordinator: syncCoordinator
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

                Button("未读条目") {
                    state.selectUnreadScope()
                }
                .keyboardShortcut("2", modifiers: [.command])

                Divider()

                Button("星标条目") {
                    state.selectStarredScope()
                }
                .keyboardShortcut("3", modifiers: [.command])

                Button("稍后读条目") {
                    state.selectLaterScope()
                }
                .keyboardShortcut("4", modifiers: [.command])

                Button("归档条目") {
                    state.selectArchiveScope()
                }
                .keyboardShortcut("5", modifiers: [.command])

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
