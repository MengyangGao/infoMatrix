// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "InfoMatrixShell",
    platforms: [
        .macOS(.v14),
        .iOS(.v17),
    ],
    products: [
        .library(name: "InfoMatrixShell", targets: ["InfoMatrixShell"]),
        .executable(name: "InfoMatrixMacApp", targets: ["InfoMatrixMacApp"]),
    ],
    targets: [
        .binaryTarget(
            name: "InfoMatrixCore",
            path: "Frameworks/InfoMatrixCore.xcframework"
        ),
        .target(
            name: "InfoMatrixShell",
            dependencies: ["InfoMatrixCore"],
            path: "Shared"
        ),
        .executableTarget(
            name: "InfoMatrixMacApp",
            dependencies: ["InfoMatrixShell"],
            path: "macOS/App",
            sources: [
                "InfoMatrixMacApp.swift",
                "NotificationDeliveryCoordinator.swift",
            ]
        ),
        .testTarget(
            name: "InfoMatrixShellTests",
            dependencies: ["InfoMatrixShell"],
            path: "Tests/InfoMatrixShellTests"
        ),
    ]
)
