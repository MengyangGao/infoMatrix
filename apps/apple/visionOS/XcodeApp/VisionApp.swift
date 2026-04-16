import SwiftUI

@main
struct InfoMatrixVisionApp: App {
    @StateObject private var state: AppState

    init() {
        let dbPath = NativeReaderService.defaultDBPath()
        _state = StateObject(wrappedValue: AppState(service: NativeReaderService(dbPath: dbPath)))
    }

    var body: some Scene {
        WindowGroup {
            ReaderShellView(state: state)
        }
    }
}
