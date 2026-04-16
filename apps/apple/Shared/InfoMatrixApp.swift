import Foundation
import SwiftUI

public struct InfoMatrixShellApp: App {
    @StateObject private var state: AppState

    public init() {
        let dbPath = NativeReaderService.defaultDBPath()
        _state = StateObject(
            wrappedValue: AppState(service: NativeReaderService(dbPath: dbPath))
        )
    }

    public var body: some Scene {
        WindowGroup {
            ReaderShellView(state: state)
        }
    }
}
