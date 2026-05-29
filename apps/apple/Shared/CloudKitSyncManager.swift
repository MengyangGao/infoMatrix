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
    func upload(events: [SyncEvent], originDeviceID: String) async throws
    func fetchRemoteEvents(after date: Date?, excludingOriginDeviceID originDeviceID: String) async throws -> [SyncEvent]
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
    private let deviceIDKey: String
    private let originDeviceID: String

    public init(
        service: ReaderService,
        transportFactory: @escaping @Sendable () -> any CloudKitSyncTransport = { LiveCloudKitSyncTransport() },
        defaults: UserDefaults = .standard,
        lastSyncKey: String = "infomatrix.cloudkit.last_sync_at",
        enabledKey: String = "infomatrix.cloudkit.enabled",
        deviceIDKey: String = "infomatrix.cloudkit.origin_device_id"
    ) {
        self.service = service
        self.transportFactory = transportFactory
        self.defaults = defaults
        self.lastSyncKey = lastSyncKey
        self.enabledKey = enabledKey
        self.deviceIDKey = deviceIDKey
        if let existingDeviceID = defaults.string(forKey: deviceIDKey), !existingDeviceID.isEmpty {
            self.originDeviceID = existingDeviceID
        } else {
            let newDeviceID = UUID().uuidString
            defaults.set(newDeviceID, forKey: deviceIDKey)
            self.originDeviceID = newDeviceID
        }
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
        let pendingCount = await pendingSyncEventCount()
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
            status.pendingLocalEventCount = await pendingSyncEventCount()
            return
        }

        status.isSyncing = true
        defer { status.isSyncing = false }

        let maxRetries = 3
        var lastError: Error?

        for attempt in 0..<maxRetries {
            do {
                let accountState = try await makeTransport().accountStatus()
                status.accountState = accountState
                guard accountState == .available else {
                    status.lastErrorMessage = "CloudKit account is not available"
                    status.pendingLocalEventCount = await pendingSyncEventCount()
                    return
                }

                let pending = try await service.listPendingSyncEvents(limit: 500)
                if !pending.isEmpty {
                    try await makeTransport().upload(events: pending, originDeviceID: originDeviceID)
                    _ = try await service.acknowledgeSyncEvents(eventIDs: pending.map(\.id))
                }

                let remoteEvents = try await makeTransport().fetchRemoteEvents(
                    after: status.lastSyncAt,
                    excludingOriginDeviceID: originDeviceID
                )
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

                status.pendingLocalEventCount = await pendingSyncEventCount()
                status.lastErrorMessage = nil
                return
            } catch {
                lastError = error
                if attempt < maxRetries - 1 {
                    let delay = UInt64(pow(2.0, Double(attempt)) * 1_000_000_000)
                    try? await Task.sleep(nanoseconds: delay)
                }
            }
        }

        status.lastErrorMessage = lastError?.localizedDescription ?? "CloudKit sync failed after \(maxRetries) attempts"
        status.pendingLocalEventCount = await pendingSyncEventCount()
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

    private func pendingSyncEventCount() async -> Int {
        ((try? await service.listPendingSyncEvents(limit: 500))?.count) ?? 0
    }
}

#if canImport(CloudKit)
#if os(macOS)
import Security

private func hasCloudKitEntitlements() -> Bool {
    guard let task = SecTaskCreateFromSelf(kCFAllocatorDefault) else { return false }
    let value = SecTaskCopyValueForEntitlement(
        task,
        "com.apple.developer.icloud-services" as CFString,
        nil
    )
    guard let services = value as? [String] else { return false }
    return services.contains("CloudKit")
}
#else
private func hasCloudKitEntitlements() -> Bool {
    // iOS builds always include CloudKit entitlements when built with
    // a provisioning profile; simulator builds also work correctly.
    return true
}
#endif

public final class LiveCloudKitSyncTransport: CloudKitSyncTransport, @unchecked Sendable {
    private let container: CKContainer?
    private let recordType = "InfoMatrixSyncEvent"
    private let createdAtKey = "created_at"
    private let originDeviceIDKey = "origin_device_id"

    public init() {
        if hasCloudKitEntitlements() {
            self.container = CKContainer.default()
        } else {
            self.container = nil
        }
    }

    public func accountStatus() async throws -> CloudKitSyncAccountState {
        guard let container else { return .couldNotDetermine }
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

    public func upload(events: [SyncEvent], originDeviceID: String) async throws {
        guard !events.isEmpty, let container else { return }
        let records = events.map { event -> CKRecord in
            let record = CKRecord(recordType: recordType, recordID: CKRecord.ID(recordName: event.id))
            record["entity_type"] = event.entityType as CKRecordValue
            record["entity_id"] = event.entityId as CKRecordValue
            record["event_type"] = event.eventType as CKRecordValue
            record["payload_json"] = event.payloadJson as CKRecordValue
            record[createdAtKey] = Self.decodeDate(event.createdAt) as CKRecordValue?
            record[originDeviceIDKey] = originDeviceID as CKRecordValue
            return record
        }
        _ = try await container.privateCloudDatabase.modifyRecords(saving: records, deleting: [])
    }

    public func fetchRemoteEvents(
        after date: Date?,
        excludingOriginDeviceID originDeviceID: String
    ) async throws -> [SyncEvent] {
        guard let container else { return [] }
        let predicate: NSPredicate
        if let date {
            predicate = NSPredicate(format: "%K > %@", createdAtKey, date as NSDate)
        } else {
            predicate = NSPredicate(value: true)
        }
        let query = CKQuery(recordType: recordType, predicate: predicate)
        var matchResults: [(CKRecord.ID, Result<CKRecord, any Error>)] = []
        let initial = try await container.privateCloudDatabase.records(matching: query)
        matchResults.append(contentsOf: initial.matchResults)
        var cursor = initial.queryCursor
        while let currentCursor = cursor {
            let page = try await container.privateCloudDatabase.records(continuingMatchFrom: currentCursor)
            matchResults.append(contentsOf: page.matchResults)
            cursor = page.queryCursor
        }

        return matchResults.compactMap { recordID, result in
            guard case .success(let record) = result else { return nil }
            if let recordOriginDeviceID = record[originDeviceIDKey] as? String,
               recordOriginDeviceID == originDeviceID {
                return nil
            }
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
                createdAt: Self.encodeDate(createdAt)
            )
        }
        .sorted { lhs, rhs in
            let lhsDate = Self.decodeDate(lhs.createdAt) ?? .distantPast
            let rhsDate = Self.decodeDate(rhs.createdAt) ?? .distantPast
            if lhsDate != rhsDate {
                return lhsDate < rhsDate
            }
            return lhs.id < rhs.id
        }
    }

    private static func encodeDate(_ date: Date) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter.string(from: date)
    }

    private static func decodeDate(_ value: String) -> Date? {
        let fractionalFormatter = ISO8601DateFormatter()
        fractionalFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return fractionalFormatter.date(from: value) ?? ISO8601DateFormatter().date(from: value)
    }
}
#else
public final class LiveCloudKitSyncTransport: CloudKitSyncTransport {
    public init() {}

    public func accountStatus() async throws -> CloudKitSyncAccountState {
        .couldNotDetermine
    }

    public func upload(events: [SyncEvent], originDeviceID: String) async throws {}

    public func fetchRemoteEvents(
        after date: Date?,
        excludingOriginDeviceID originDeviceID: String
    ) async throws -> [SyncEvent] {
        []
    }
}
#endif
