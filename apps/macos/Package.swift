// swift-tools-version: 6.0
import PackageDescription

let vexioHeaderSearchFlag = "-I../../crates/vexio-core/include"
let vexioRustLibSearchFlags = [
    "-L../../target/release",
    "-L../../target/debug",
    "-L../../target/universal/release",
]

let package = Package(
    name: "VexioMac",
    defaultLocalization: "en",
    platforms: [
        .macOS(.v15),
    ],
    products: [
        .executable(
            name: "VexioMacApp",
            targets: ["VexioMacApp"]
        ),
        .library(
            name: "VexioQuickLookExtension",
            type: .dynamic,
            targets: ["VexioQuickLookExtension"]
        ),
    ],
    targets: [
        // C / System FFI target exposing ../../crates/vexio-core/include/vexio.h verbatim to Swift 6.
        .systemLibrary(
            name: "VexioCoreFFI",
            path: "Sources/VexioCoreFFI"
        ),

        // Native Swift 6 + SwiftUI/AppKit + Metal (CAMetalLayer) macOS Workstation Application.
        .executableTarget(
            name: "VexioMacApp",
            dependencies: ["VexioCoreFFI"],
            path: "Sources/VexioMacApp",
            swiftSettings: [
                .unsafeFlags([vexioHeaderSearchFlag]),
            ],
            linkerSettings: [
                .unsafeFlags(vexioRustLibSearchFlags),
                .linkedLibrary("vexio_core"),
                .linkedFramework("AppKit"),
                .linkedFramework("SwiftUI"),
                .linkedFramework("Metal"),
                .linkedFramework("QuartzCore"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("UniformTypeIdentifiers"),
            ]
        ),

        // Native Offline Quick Look Thumbnail & Preview App Extension (.vsdx, .vssx, .vstx, .vsd).
        .target(
            name: "VexioQuickLookExtension",
            dependencies: ["VexioCoreFFI"],
            path: "Sources/VexioQuickLookExtension",
            exclude: ["Info.plist"],
            swiftSettings: [
                .unsafeFlags([vexioHeaderSearchFlag]),
            ],
            linkerSettings: [
                .unsafeFlags(vexioRustLibSearchFlags),
                .linkedLibrary("vexio_core"),
                .linkedFramework("AppKit"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("QuickLookThumbnailing"),
                .linkedFramework("QuickLookUI"),
                .linkedFramework("Quartz"),
                .linkedFramework("UniformTypeIdentifiers"),
            ]
        ),
    ]
)
