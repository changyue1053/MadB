// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "MadBObservation",
    platforms: [.macOS(.v14), .iOS(.v17), .tvOS(.v17), .watchOS(.v10)],
    products: [.library(name: "MadBObservation", targets: ["MadBObservation"])],
    targets: [
        .target(name: "MadBObservation"),
        .testTarget(name: "MadBObservationTests", dependencies: ["MadBObservation"]),
    ]
)
