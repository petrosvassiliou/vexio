import AppKit
import SwiftUI

// MARK: - Native AppKit Lifecycle Delegate (NSDocument Tabbing, Metal & Continuity Handoff)

@MainActor
public final class VexioAppDelegate: NSObject, NSApplicationDelegate {
    public var pendingExternalURL: URL?

    public func applicationDidFinishLaunching(_ notification: Notification) {
        NSWindow.allowsAutomaticWindowTabbing = true
        let savedAppearance = UserDefaults.standard.string(forKey: "vexio.appearanceMode") ?? "light"
        switch savedAppearance {
        case "dark":
            NSApp.appearance = NSAppearance(named: .darkAqua)
        case "system":
            NSApp.appearance = nil
        default:
            NSApp.appearance = NSAppearance(named: .aqua)
        }
    }

    public func applicationShouldOpenUntitledFile(_ sender: NSApplication) -> Bool {
        // Prevent macOS from popping up an external system NSOpenPanel before our in-app window appears!
        false
    }

    public func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    public func application(_ application: NSApplication, open urls: [URL]) {
        guard let first = urls.first else { return }
        pendingExternalURL = first
        NotificationCenter.default.post(name: .vexioOpenExternalURL, object: first)
    }

    public func application(
        _ application: NSApplication,
        continue userActivity: NSUserActivity,
        restorationHandler: @escaping ([any NSUserActivityRestoring]) -> Void
    ) -> Bool {
        guard userActivity.activityType == VexioDocumentSession.continuityEditingActivityType else {
            return false
        }
        if let urlString = userActivity.userInfo?["fileURLString"] as? String,
           let documentURL = URL(string: urlString),
           FileManager.default.fileExists(atPath: documentURL.path) {
            NotificationCenter.default.post(name: .vexioOpenExternalURL, object: documentURL)
        }
        return true
    }
}

private struct VexioMainWindowHostView: View {
    @StateObject private var session = VexioDocumentSession()
    @State private var initialURL: URL? = nil

    var body: some View {
        VexioWorkspaceView(session: session, fileURL: initialURL)
            .onAppear {
                if let delegate = NSApp.delegate as? VexioAppDelegate,
                   let launchURL = delegate.pendingExternalURL {
                    delegate.pendingExternalURL = nil
                    session.openLocalFile(from: launchURL)
                }
            }
            .onOpenURL { url in
                session.openLocalFile(from: url)
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenExternalURL)) { note in
                if let url = note.object as? URL {
                    session.openLocalFile(from: url)
                }
            }
    }
}

// MARK: - Vexio macOS Native Application Entry Point

@main
public struct VexioMacApp: App {
    @NSApplicationDelegateAdaptor(VexioAppDelegate.self) private var appDelegate
    @AppStorage("vexio.appearanceMode") private var appearanceMode: String = "light"

    public init() {}

    private var resolvedColorScheme: ColorScheme? {
        switch appearanceMode {
        case "dark": return .dark
        case "system": return nil
        default: return .light
        }
    }

    public var body: some Scene {
        WindowGroup("Vexio") {
            VexioMainWindowHostView()
                .preferredColorScheme(resolvedColorScheme)
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1440, height: 900)
        .windowResizability(.contentMinSize)
        .commands {
            SidebarCommands()
            VexioGlobalMenuCommands()
        }
    }
}

// MARK: - Global macOS Menu Bar Commands & Shortcuts (Indexed by macOS Help Search)

public struct VexioGlobalMenuCommands: Commands {
    @FocusedValue(\.vexioDocumentSession) private var session

    public init() {}

    public var body: some Commands {
        // App Menu: Check for Updates… (Fixes Remark #8)
        CommandGroup(after: .appInfo) {
            Button("Check for Updates…") {
                Task { @MainActor in
                    await VexioUpdateManager.shared.checkForUpdates(userInitiated: true)
                }
            }
        }

        // File Menu: In-App New / Open / Import Stencil / Save / Save As
        CommandGroup(replacing: .newItem) {
            Button("New / Templates Hub…") {
                NotificationCenter.default.post(name: .vexioShowFileBackstage, object: nil)
            }
            .keyboardShortcut("n", modifiers: [.command])

            Button("Open Diagram or CAD File…") {
                NotificationCenter.default.post(name: .vexioOpenFileSheet, object: nil)
            }
            .keyboardShortcut("o", modifiers: [.command])

            Button("Import Visio Stencil (.vssx / .vss)…") {
                NotificationCenter.default.post(name: .vexioImportStencilSheet, object: nil)
            }
            .keyboardShortcut("i", modifiers: [.command, .shift])

            Divider()

            Button("Save") {
                NotificationCenter.default.post(name: .vexioSaveFileSheet, object: nil)
            }
            .keyboardShortcut("s", modifiers: [.command])

            Button("Save As…") {
                NotificationCenter.default.post(name: .vexioSaveAsFileSheet, object: nil)
            }
            .keyboardShortcut("s", modifiers: [.command, .shift])
        }

        // Format Menu: Native OS Font & Color Panels + Rich Text Formatting Shortcuts
        CommandMenu("Format") {
            Button("Show OS Fonts…") {
                NSFontManager.shared.orderFrontFontPanel(nil)
            }
            .keyboardShortcut("t", modifiers: [.command])

            Button("Show OS Colors…") {
                NSColorPanel.shared.orderFront(nil)
            }
            .keyboardShortcut("c", modifiers: [.command, .shift])

            Divider()

            Button("Bold") {
                session?.applyTextStyle(toggleStyleMask: 1, undoManager: NSApp.keyWindow?.undoManager)
            }
            .keyboardShortcut("b", modifiers: [.command])
            .disabled(session == nil)

            Button("Italic") {
                session?.applyTextStyle(toggleStyleMask: 2, undoManager: NSApp.keyWindow?.undoManager)
            }
            .keyboardShortcut("i", modifiers: [.command])
            .disabled(session == nil)

            Button("Underline") {
                session?.applyTextStyle(toggleStyleMask: 4, undoManager: NSApp.keyWindow?.undoManager)
            }
            .keyboardShortcut("u", modifiers: [.command])
            .disabled(session == nil)

            Divider()

            Button("Align Left") {
                session?.applyTextStyle(horizAlign: 0, undoManager: NSApp.keyWindow?.undoManager)
            }
            .disabled(session == nil)

            Button("Align Center") {
                session?.applyTextStyle(horizAlign: 1, undoManager: NSApp.keyWindow?.undoManager)
            }
            .disabled(session == nil)

            Button("Align Right") {
                session?.applyTextStyle(horizAlign: 2, undoManager: NSApp.keyWindow?.undoManager)
            }
            .disabled(session == nil)
        }

        // Deterministic Journal Undo / Redo
        CommandGroup(replacing: .undoRedo) {
            Button("Undo") {
                session?.performUndo(undoManager: NSApp.keyWindow?.undoManager)
            }
            .keyboardShortcut("z", modifiers: [.command])
            .disabled(session == nil)

            Button("Redo") {
                session?.performRedo(undoManager: NSApp.keyWindow?.undoManager)
            }
            .keyboardShortcut("z", modifiers: [.command, .shift])
            .disabled(session == nil)
        }

        // Workspace Panels & Viewport Zoom
        CommandGroup(after: .sidebar) {
            Divider()

            Button(session?.showStencilNavigator == true ? "Hide Stencil Navigator" : "Show Stencil Navigator") {
                withAnimation(LiquidGlassPhysics.standardSpring) {
                    session?.showStencilNavigator.toggle()
                }
            }
            .keyboardShortcut("1", modifiers: [.command, .option])
            .disabled(session == nil)

            Button(session?.showInspectorSidebar == true ? "Hide Inspector Sidebar" : "Show Inspector Sidebar") {
                withAnimation(LiquidGlassPhysics.standardSpring) {
                    session?.showInspectorSidebar.toggle()
                }
            }
            .keyboardShortcut("2", modifiers: [.command, .option])
            .disabled(session == nil)

            Button(session?.showShapeSheetDrawer == true ? "Hide ShapeSheet Formula Editor" : "Show ShapeSheet Formula Editor") {
                withAnimation(LiquidGlassPhysics.panelExpansionSpring) {
                    session?.showShapeSheetDrawer.toggle()
                }
            }
            .keyboardShortcut("s", modifiers: [.command, .option])
            .disabled(session == nil)

            Divider()

            Button("Zoom In") {
                guard let session else { return }
                session.zoomScale = min(session.zoomScale * 1.25, 16.0)
            }
            .keyboardShortcut("+", modifiers: [.command])
            .disabled(session == nil)

            Button("Zoom Out") {
                guard let session else { return }
                session.zoomScale = max(session.zoomScale / 1.25, 0.10)
            }
            .keyboardShortcut("-", modifiers: [.command])
            .disabled(session == nil)

            Button("Fit Page to Window") {
                session?.fitViewportToActivePage()
            }
            .keyboardShortcut("0", modifiers: [.command])
            .disabled(session == nil)
        }

        // Tools Menu (All 16 Visio Drawing & Diagramming Tools)
        CommandMenu("Tools") {
            Button("Select Tool") { session?.activeTool = .pointer }
                .keyboardShortcut("1", modifiers: [.command])
                .disabled(session == nil)

            Button("Pan Tool") { session?.activeTool = .pan }
                .disabled(session == nil)

            Button("Dynamic Connector Tool") { session?.activeTool = .connector }
                .keyboardShortcut("2", modifiers: [.command])
                .disabled(session == nil)

            Divider()

            Button("Line Tool") { session?.activeTool = .line }
                .disabled(session == nil)

            Button("Arc Tool") { session?.activeTool = .arc }
                .disabled(session == nil)

            Button("Freeform Spline Tool") { session?.activeTool = .freeform }
                .keyboardShortcut("6", modifiers: [.command])
                .disabled(session == nil)

            Divider()

            Button("Rectangle Tool") { session?.activeTool = .rectangle }
                .keyboardShortcut("4", modifiers: [.command])
                .disabled(session == nil)

            Button("Rounded Rectangle Tool") { session?.activeTool = .roundedRect }
                .keyboardShortcut("4", modifiers: [.command, .shift])
                .disabled(session == nil)

            Button("Circle / Ellipse Tool") { session?.activeTool = .ellipse }
                .keyboardShortcut("5", modifiers: [.command])
                .disabled(session == nil)

            Button("Diamond Tool") { session?.activeTool = .diamond }
                .disabled(session == nil)

            Button("Triangle Tool") { session?.activeTool = .triangle }
                .disabled(session == nil)

            Button("Hexagon Tool") { session?.activeTool = .hexagon }
                .disabled(session == nil)

            Button("Star Tool") { session?.activeTool = .star }
                .disabled(session == nil)

            Divider()

            Button("Text Block Tool") { session?.activeTool = .text }
                .keyboardShortcut("3", modifiers: [.command])
                .disabled(session == nil)

            Button("Markup Pen Ink Tool") { session?.activeTool = .ink }
                .keyboardShortcut("7", modifiers: [.command])
                .disabled(session == nil)

            Button("Highlighter Tool") { session?.activeTool = .highlighter }
                .keyboardShortcut("7", modifiers: [.command, .shift])
                .disabled(session == nil)
        }

        // Shape, Boolean Operations, Visual Presets & ShapeSheet Protection Menu
        CommandMenu("Shape") {
            Button("Duplicate Selection") {
                session?.duplicateSelectedShapes(undoManager: NSApp.keyWindow?.undoManager)
            }
            .keyboardShortcut("d", modifiers: [.command])
            .disabled(session == nil)

            Button("Delete Selection") {
                session?.deleteSelectedShapes(undoManager: NSApp.keyWindow?.undoManager)
            }
            .disabled(session == nil)

            Menu("Arrange Order") {
                Button("Bring to Front") {
                    session?.reorderSelectedShapes(mode: "front", undoManager: NSApp.keyWindow?.undoManager)
                }
                Button("Bring Forward") {
                    session?.reorderSelectedShapes(mode: "forward", undoManager: NSApp.keyWindow?.undoManager)
                }
                Button("Send Backward") {
                    session?.reorderSelectedShapes(mode: "backward", undoManager: NSApp.keyWindow?.undoManager)
                }
                Button("Send to Back") {
                    session?.reorderSelectedShapes(mode: "back", undoManager: NSApp.keyWindow?.undoManager)
                }
            }
            .disabled(session == nil)

            Divider()

            Menu("Shape Operations") {
                Button("Union") {
                    session?.applyBooleanOperation(.union, undoManager: NSApp.keyWindow?.undoManager)
                }
                Button("Combine") {
                    session?.applyBooleanOperation(.combine, undoManager: NSApp.keyWindow?.undoManager)
                }
                Button("Fragment") {
                    session?.applyBooleanOperation(.fragment, undoManager: NSApp.keyWindow?.undoManager)
                }
                Button("Intersect") {
                    session?.applyBooleanOperation(.intersect, undoManager: NSApp.keyWindow?.undoManager)
                }
                Button("Subtract") {
                    session?.applyBooleanOperation(.subtract, undoManager: NSApp.keyWindow?.undoManager)
                }
                Button("Trim") {
                    session?.applyBooleanOperation(.trim, undoManager: NSApp.keyWindow?.undoManager)
                }
                Button("Join") {
                    session?.applyBooleanOperation(.join, undoManager: NSApp.keyWindow?.undoManager)
                }
                Divider()
                Button("Offset (+0.125 in)") {
                    session?.applyBooleanOperation(.offset, offsetInches: 0.125, undoManager: NSApp.keyWindow?.undoManager)
                }
            }
            .disabled(session == nil)

            Menu("Visual Effects") {
                ForEach(VexioVisualEffectPreset.allCases) { preset in
                    Button(preset.displayName) {
                        session?.applyVisualPreset(preset, undoManager: NSApp.keyWindow?.undoManager)
                    }
                }
            }
            .disabled(session == nil)

            Divider()

            Button("Rotate 90° Clockwise") {
                session?.rotateSelectedShape(byDegrees: 90.0, undoManager: NSApp.keyWindow?.undoManager)
            }
            .keyboardShortcut("r", modifiers: [.command])
            .disabled(session == nil)

            Button("Rotate 90° Counter-Clockwise") {
                session?.rotateSelectedShape(byDegrees: -90.0, undoManager: NSApp.keyWindow?.undoManager)
            }
            .keyboardShortcut("r", modifiers: [.command, .shift])
            .disabled(session == nil)

            Divider()

            Button("Protect Width with GUARD()") {
                guard let session else { return }
                let val = session.readCellEvaluatedValue(cellName: "Width")
                try? session.setCellFormula(
                    cellName: "Width",
                    formula: "GUARD(\(val.isEmpty ? "1.50 in" : val))",
                    mode: .forceSetF,
                    undoManager: NSApp.keyWindow?.undoManager
                )
            }
            .keyboardShortcut("g", modifiers: [.command, .option])
            .disabled(session == nil)

            Button("Override GUARD(Width) via SETF") {
                try? session?.setCellFormula(
                    cellName: "Width",
                    formula: "1.50 in",
                    mode: .forceSetF,
                    undoManager: NSApp.keyWindow?.undoManager
                )
            }
            .keyboardShortcut("g", modifiers: [.command, .option, .shift])
            .disabled(session == nil)

            Divider()

            Menu("Connector Routing") {
                ForEach(ConnectorRoutingStyle.allCases) { style in
                    Button(style.rawValue) {
                        session?.setConnectorRoutingStyle(style, undoManager: NSApp.keyWindow?.undoManager)
                    }
                }
            }
            .disabled(session == nil)
        }

        // Enterprise Diagramming, Themes, Subprocess, Compare, Connectors & Validation Menu
        CommandMenu("Diagram") {
            Button("Insert New Page") {
                session?.addNewPage()
            }
            .keyboardShortcut("n", modifiers: [.command, .shift])
            .disabled(session == nil)

            Button("Create Subprocess…") {
                session?.createSubprocessDrilldown(undoManager: NSApp.keyWindow?.undoManager)
            }
            .keyboardShortcut("p", modifiers: [.command, .shift])
            .disabled(session == nil)

            Divider()

            Menu("Themes & Variants (6×4)") {
                ForEach(VexioThemePreset.allCases) { theme in
                    Menu(theme.displayName) {
                        ForEach(0..<4, id: \.self) { variantIdx in
                            Button("Variant \(variantIdx + 1)") {
                                session?.applyThemeAndVariant(
                                    themeSlug: theme.rawValue,
                                    variantIndex: UInt8(variantIdx),
                                    undoManager: NSApp.keyWindow?.undoManager
                                )
                            }
                        }
                    }
                }
            }
            .disabled(session == nil)

            Divider()

            Button("Pin Baseline Snapshot for Compare") {
                session?.captureComparisonBaseline()
            }
            .disabled(session == nil)

            Button("Diagram Compare with Snapshot") {
                session?.compareWithSnapshot()
            }
            .keyboardShortcut("d", modifiers: [.command, .shift])
            .disabled(session == nil)

            Divider()

            Menu("Plan 2 Data & Cloud Connectors") {
                Button("Import Data Visualizer Swimlane Flow") {
                    session?.importDataVisualizerSample(undoManager: NSApp.keyWindow?.undoManager)
                }
                Button("Import Cloud Discovery Topology (AWS / Azure / GCP)") {
                    session?.importCloudTopologySample(undoManager: NSApp.keyWindow?.undoManager)
                }
                Button("Import Entra ID / LDAP (.ldif) Org Chart") {
                    session?.importEntraLdifSample(undoManager: NSApp.keyWindow?.undoManager)
                }
                Divider()
                Button("Merge Remote CRDT Co-Authoring Batch") {
                    session?.applyRemoteCrdtBatch()
                }
            }
            .disabled(session == nil)

            Divider()

            Menu("Auto-Layout Active Page") {
                ForEach(AutoLayoutPreset.allCases) { preset in
                    Button(preset.rawValue) {
                        session?.applyAutoLayout(preset, undoManager: NSApp.keyWindow?.undoManager)
                    }
                }
            }
            .disabled(session == nil)

            Button("Auto-Layout Top-to-Bottom") {
                session?.applyAutoLayout(.hierarchicalTopDown, undoManager: NSApp.keyWindow?.undoManager)
            }
            .keyboardShortcut("l", modifiers: [.command, .shift])
            .disabled(session == nil)

            Divider()

            Button("Validate BPMN 2.0 Rules") {
                session?.runDiagramValidation(standard: .bpmn20)
            }
            .keyboardShortcut("v", modifiers: [.command, .shift])
            .disabled(session == nil)

            Button("Validate UML 2.5 Rules") {
                session?.runDiagramValidation(standard: .uml25)
            }
            .disabled(session == nil)

            Button("Validate SharePoint Workflow Rules") {
                session?.runDiagramValidation(standard: .sharePointWorkflow)
            }
            .disabled(session == nil)
        }

        // Preserve macOS Help Search while appending ShapeSheet documentation quick-trigger
        CommandGroup(after: .help) {
            Button("Vexio ShapeSheet Formula & GUARD Reference") {
                withAnimation(LiquidGlassPhysics.panelExpansionSpring) {
                    session?.showShapeSheetDrawer = true
                    session?.statusMessage = "ShapeSheet supports 220+ functions: GUARD, SETF, PAR, PNT, INTERSECTX/Y, GRAVITY"
                }
            }
        }
    }
}
