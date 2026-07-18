// swift-tools-version: 6.1
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "TTS29Feature",
    platforms: [.iOS(.v17), .macOS(.v14)],
    products: [
        // Products define the executables and libraries a package produces, making them visible to other packages.
        .library(
            name: "TTS29Feature",
            targets: ["TTS29Feature"]
        ),
    ],
    targets: [
        // Targets are the basic building blocks of a package, defining a module or a test suite.
        // Targets can depend on other targets in this package and products from dependencies.
        .target(
            name: "TTS29Feature",
            resources: [.process("Resources")]
        ),
        .testTarget(
            name: "TTS29FeatureTests",
            dependencies: [
                "TTS29Feature"
            ],
            linkerSettings: [
                .unsafeFlags([
                    "-L../../core/target/aarch64-apple-ios-sim/release",
                    "-ltts29_core",
                ], .when(platforms: [.iOS])),
                .unsafeFlags([
                    "-L../../core/target/release",
                    "-ltts29_core",
                ], .when(platforms: [.macOS])),
            ]
        ),
    ]
)
