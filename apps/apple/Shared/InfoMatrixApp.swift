import Foundation
import SwiftUI

public struct InfoMatrixShellApp: App {
    @StateObject private var state: AppState

    public init() {
        let dbPath = NativeReaderService.defaultDBPath()
        let service = NativeReaderService(dbPath: dbPath)
        let syncCoordinator = CloudKitSyncCoordinator(service: service)
        _state = StateObject(
            wrappedValue: AppState(service: service, syncCoordinator: syncCoordinator)
        )
    }

    public var body: some Scene {
        WindowGroup {
            ReaderShellView(state: state)
        }
    }
}
