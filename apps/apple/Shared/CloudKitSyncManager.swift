import Foundation

#if canImport(CloudKit)
import CloudKit
#endif

public enum CloudKitSyncAccountState: String, Codable, Sendable {
    case available
    case noAccount
    case restricted
    case couldNotDetermine
    case temporarilyUnavailable
}

public struct CloudKitSyncStatus: Codable, Equatable, Sendable {
    public var enabled: Bool
    public var accountState: CloudKitSyncAccountState
    public var pendingLocalEventCount: Int
    public var lastSyncAt: Date?
    public var lastErrorMessage: String?
    public var isSyncing: Bool

    public init(
        enabled: Bool = false,
        accountState: CloudKitSyncAccountState = .couldNotDetermine,
        pendingLocalEventCount: Int = 0,
        lastSyncAt: Date? = nil,
        lastErrorMessage: String? = nil,
        isSyncing: Bool = false
    ) {
        self.enabled = enabled
        self.accountState = accountState
        self.pendingLocalEventCount = pendingLocalEventCount
        self.lastSyncAt = lastSyncAt
        self.lastErrorMessage = lastErrorMessage
        self.isSyncing = isSyncing
    }
}

public protocol CloudKitSyncTransport: Sendable {
    func accountStatus() async throws -> CloudKitSyncAccountState
    func upload(events: [SyncEvent]) async throws
    func fetchRemoteEvents(after date: Date?) async throws -> [SyncEvent]
}

@MainActor
public final class CloudKitSyncCoordinator: ObservableObject, @unchecked Sendable {
    @Published public private(set) var status: CloudKitSyncStatus

    private let service: ReaderService
    private let transportFactory: @Sendable () -> any CloudKitSyncTransport
    private var transport: (any CloudKitSyncTransport)?
    private let defaults: UserDefaults
    private let lastSyncKey: String
    private let enabledKey: String

    public init(
        service: ReaderService,
        transportFactory: @escaping @Sendable () -> any CloudKitSyncTransport = { LiveCloudKitSyncTransport() },
        defaults: UserDefaults = .standard,
        lastSyncKey: String = "infomatrix.cloudkit.last_sync_at",
        enabledKey: String = "infomatrix.cloudkit.enabled"
    ) {
        self.service = service
        self.transportFactory = transportFactory
        self.defaults = defaults
        self.lastSyncKey = lastSyncKey
        self.enabledKey = enabledKey
        self.status = CloudKitSyncStatus(
            enabled: defaults.object(forKey: enabledKey) as? Bool ?? false,
            accountState: .couldNotDetermine,
            pendingLocalEventCount: 0,
            lastSyncAt: Self.decodeDate(defaults.string(forKey: lastSyncKey)),
            lastErrorMessage: nil,
            isSyncing: false
        )
    }

    public func setEnabled(_ enabled: Bool) {
        status.enabled = enabled
        defaults.set(enabled, forKey: enabledKey)
    }

    public func refreshStatus() async {
        let pendingCount = ((try? await service.listPendingSyncEvents(limit: 500))?.count) ?? 0
        let accountState: CloudKitSyncAccountState
        if status.enabled {
            do {
                accountState = try await makeTransport().accountStatus()
            } catch {
                accountState = .couldNotDetermine
                status.lastErrorMessage = error.localizedDescription
            }
        } else {
            accountState = .couldNotDetermine
        }
        status.pendingLocalEventCount = pendingCount
        status.accountState = accountState
    }

    public func syncNow() async {
        guard status.enabled else {
            status.lastErrorMessage = "CloudKit sync is disabled"
            return
        }

        status.isSyncing = true
        defer { status.isSyncing = false }

        do {
            let accountState = try await makeTransport().accountStatus()
            status.accountState = accountState
            guard accountState == .available else {
                status.lastErrorMessage = "CloudKit account is not available"
                return
            }

            let pending = try await service.listPendingSyncEvents(limit: 500)
            if !pending.isEmpty {
                try await makeTransport().upload(events: pending)
                _ = try await service.acknowledgeSyncEvents(eventIDs: pending.map(\.id))
            }

            let remoteEvents = try await makeTransport().fetchRemoteEvents(after: status.lastSyncAt)
            if !remoteEvents.isEmpty {
                _ = try await service.applySyncEvents(remoteEvents)
                if let newest = remoteEvents.compactMap({ Self.decodeDate($0.createdAt) }).max() {
                    status.lastSyncAt = newest
                    defaults.set(Self.encodeDate(newest), forKey: lastSyncKey)
                }
            } else {
                let now = Date()
                status.lastSyncAt = now
                defaults.set(Self.encodeDate(now), forKey: lastSyncKey)
            }

            status.pendingLocalEventCount = ((try? await service.listPendingSyncEvents(limit: 500))?.count) ?? 0
            status.lastErrorMessage = nil
        } catch {
            status.lastErrorMessage = error.localizedDescription
        }
    }

    private static func encodeDate(_ date: Date) -> String {
        ISO8601DateFormatter().string(from: date)
    }

    private static func decodeDate(_ value: String?) -> Date? {
        guard let value else { return nil }
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter.date(from: value) ?? ISO8601DateFormatter().date(from: value)
    }

    private func makeTransport() -> any CloudKitSyncTransport {
        if let transport {
            return transport
        }

        let createdTransport = transportFactory()
        transport = createdTransport
        return createdTransport
    }
}

#if canImport(CloudKit)
public final class LiveCloudKitSyncTransport: CloudKitSyncTransport, @unchecked Sendable {
    private let container: CKContainer
    private let recordType = "InfoMatrixSyncEvent"
    private let createdAtKey = "created_at"

    public init(container: CKContainer = .default()) {
        self.container = container
    }

    public func accountStatus() async throws -> CloudKitSyncAccountState {
        switch try await container.accountStatus() {
        case .available:
            return .available
        case .noAccount:
            return .noAccount
        case .restricted:
            return .restricted
        case .temporarilyUnavailable:
            return .temporarilyUnavailable
        case .couldNotDetermine:
            return .couldNotDetermine
        @unknown default:
            return .couldNotDetermine
        }
    }

    public func upload(events: [SyncEvent]) async throws {
        guard !events.isEmpty else { return }
        let records = events.map { event -> CKRecord in
            let record = CKRecord(recordType: recordType, recordID: CKRecord.ID(recordName: event.id))
            record["entity_type"] = event.entityType as CKRecordValue
            record["entity_id"] = event.entityId as CKRecordValue
            record["event_type"] = event.eventType as CKRecordValue
            record["payload_json"] = event.payloadJson as CKRecordValue
            record[createdAtKey] = ISO8601DateFormatter().date(from: event.createdAt) as CKRecordValue?
            return record
        }
        _ = try await container.privateCloudDatabase.modifyRecords(saving: records, deleting: [])
    }

    public func fetchRemoteEvents(after date: Date?) async throws -> [SyncEvent] {
        let predicate: NSPredicate
        if let date {
            predicate = NSPredicate(format: "%K > %@", createdAtKey, date as NSDate)
        } else {
            predicate = NSPredicate(value: true)
        }
        let query = CKQuery(recordType: recordType, predicate: predicate)
        let result = try await container.privateCloudDatabase.records(matching: query)
        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        return result.matchResults.compactMap { recordID, result in
            guard case .success(let record) = result else { return nil }
            guard
                let entityType = record["entity_type"] as? String,
                let entityId = record["entity_id"] as? String,
                let eventType = record["event_type"] as? String,
                let payloadJson = record["payload_json"] as? String,
                let createdAt = record[createdAtKey] as? Date
            else {
                return nil
            }
            return SyncEvent(
                id: recordID.recordName,
                entityType: entityType,
                entityId: entityId,
                eventType: eventType,
                payloadJson: payloadJson,
                createdAt: dateFormatter.string(from: createdAt)
            )
        }
    }
}
#else
public final class LiveCloudKitSyncTransport: CloudKitSyncTransport {
    public init() {}

    public func accountStatus() async throws -> CloudKitSyncAccountState {
        .couldNotDetermine
    }

    public func upload(events: [SyncEvent]) async throws {}

    public func fetchRemoteEvents(after date: Date?) async throws -> [SyncEvent] {
        []
    }
}
#endif
