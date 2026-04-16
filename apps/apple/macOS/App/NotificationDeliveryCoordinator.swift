import AppKit
import Foundation
import UserNotifications
#if canImport(InfoMatrixShell)
import InfoMatrixShell
#endif

@MainActor
final class NotificationDeliveryCoordinator: ObservableObject {
    private let service: ReaderService
    private let pollInterval: Duration
    private var pollTask: Task<Void, Never>?
    private var didRequestAuthorization = false

    init(service: ReaderService, pollIntervalSeconds: Int = 60) {
        self.service = service
        self.pollInterval = .seconds(max(15, pollIntervalSeconds))
    }

    func start() {
        guard pollTask == nil else {
            return
        }

        pollTask = Task { [weak self] in
            guard let self else { return }
            await self.syncPendingNotifications()
            while !Task.isCancelled {
                do {
                    try await Task.sleep(for: pollInterval)
                } catch {
                    break
                }
                await self.syncPendingNotifications()
            }
        }
    }

    func stop() {
        pollTask?.cancel()
        pollTask = nil
    }

    func syncPendingNotifications() async {
        guard supportsSystemNotifications, await ensureAuthorizationIfNeeded() else {
            await updateBadgeCount()
            return
        }

        do {
            let events = try await service.listPendingNotificationEvents(limit: 50)
            guard !events.isEmpty else {
                await updateBadgeCount()
                return
            }

            var deliveredIDs: [String] = []
            for event in events {
                try await deliver(event)
                deliveredIDs.append(event.id)
            }

            if !deliveredIDs.isEmpty {
                _ = try await service.acknowledgeNotificationEvents(eventIDs: deliveredIDs)
            }
        } catch {
            // Keep delivery best-effort and deterministic; failures are surfaced via the UI shell.
        }

        await updateBadgeCount()
    }

    private func deliver(_ event: NotificationEvent) async throws {
        let content = UNMutableNotificationContent()
        content.title = event.title
        content.body = event.body
        content.subtitle = event.mode == .digest ? "摘要" : "新内容"
        content.sound = .default
        content.threadIdentifier = event.feedID ?? "infomatrix"

        let request = UNNotificationRequest(
            identifier: event.id,
            content: content,
            trigger: nil
        )

        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            UNUserNotificationCenter.current().add(request) { error in
                if let error {
                    continuation.resume(throwing: error)
                } else {
                    continuation.resume()
                }
            }
        }
    }

    private func ensureAuthorizationIfNeeded() async -> Bool {
        guard !didRequestAuthorization else {
            return true
        }

        didRequestAuthorization = true
        do {
            return try await requestAuthorization()
        } catch {
            return false
        }
    }

    private func requestAuthorization() async throws -> Bool {
        try await withCheckedThrowingContinuation { continuation in
            UNUserNotificationCenter.current().requestAuthorization(
                options: [.alert, .badge, .sound]
            ) { granted, error in
                if let error {
                    continuation.resume(throwing: error)
                } else {
                    continuation.resume(returning: granted)
                }
            }
        }
    }

    private func updateBadgeCount() async {
        do {
            let counts = try await service.itemCounts()
            NSApp.dockTile.badgeLabel = counts.unread > 0 ? "\(counts.unread)" : nil
        } catch {
            NSApp.dockTile.badgeLabel = nil
        }
    }

    private var supportsSystemNotifications: Bool {
        Bundle.main.bundleURL.pathExtension == "app"
    }
}
