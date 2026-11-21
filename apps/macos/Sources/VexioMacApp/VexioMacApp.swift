import AppKit
import SwiftUI

// MARK: - Native AppKit Lifecycle Delegate (NSDocument Tabbing, Metal & Continuity Handoff)

@MainActor
public final class VexioAppDelegate: NSObject, NSApplicationDelegate {
    public func applicationDidFinishLaunching(_ notification: Notification) {
        NSWindow.allowsAutomaticWindowTabbing = true
    }

    public func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
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
            NSDocumentController.shared.openDocument(
                withContentsOf: documentURL,
                display: true
            ) { _, _, _ in }
        }
        return true
    }
}

// MARK: - Vexio macOS Native Application Entry Point

@main
public struct VexioMacApp: App {
    @NSApplicationDelegateAdaptor(VexioAppDelegate.self) private var appDelegate

    public init() {}

    public var body: some Scene {
        DocumentGroup(newDocument: { VexioDocumentSession() }) { configuration in
            VexioWorkspaceView(
                session: configuration.document,
                fileURL: configuration.fileURL
            )
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

            Button("Actual Size (100%)") {
                session?.zoomScale = 1.0
                session?.panOffset = .zero
            }
            .keyboardShortcut("0", modifiers: [.command])
            .disabled(session == nil)
        }

        // Tools Menu
        CommandMenu("Tools") {
            Button("Pointer Tool") { session?.activeTool = .pointer }
                .keyboardShortcut("1", modifiers: [.command])
                .disabled(session == nil)

            Button("Dynamic Connector Tool") { session?.activeTool = .connector }
                .keyboardShortcut("2", modifiers: [.command])
                .disabled(session == nil)

            Button("Text Block Tool") { session?.activeTool = .text }
                .keyboardShortcut("3", modifiers: [.command])
                .disabled(session == nil)

            Button("Rectangle Tool") { session?.activeTool = .rectangle }
                .keyboardShortcut("4", modifiers: [.command])
                .disabled(session == nil)

            Button("Ellipse Tool") { session?.activeTool = .ellipse }
                .keyboardShortcut("5", modifiers: [.command])
                .disabled(session == nil)

            Button("Freeform Spline Tool") { session?.activeTool = .freeform }
                .keyboardShortcut("6", modifiers: [.command])
                .disabled(session == nil)

            Button("Markup Ink Tool") { session?.activeTool = .ink }
                .keyboardShortcut("7", modifiers: [.command])
                .disabled(session == nil)
        }

        // Shape, Boolean Operations, Visual Presets & ShapeSheet Protection Menu
        CommandMenu("Shape") {
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
