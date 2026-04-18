import SwiftUI

@main
struct InfoMatrixiOSApp: App {
    @StateObject private var state: AppState

    init() {
        let dbPath = NativeReaderService.defaultDBPath()
        let service = NativeReaderService(dbPath: dbPath)
        let syncCoordinator = CloudKitSyncCoordinator(service: service)
        _state = StateObject(
            wrappedValue: AppState(service: service, syncCoordinator: syncCoordinator)
        )
    }

    var body: some Scene {
        WindowGroup {
            ReaderShellView(state: state)
        }
    }
}
