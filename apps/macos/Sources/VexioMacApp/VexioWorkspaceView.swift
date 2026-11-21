import AppKit
import SwiftUI
import UniformTypeIdentifiers

// MARK: - Inspector Tab Enumeration

public enum VexioInspectorTab: String, CaseIterable, Identifiable, Sendable {
    case style = "Style"
    case geometry = "Geometry"
    case shapeData = "Prop.*"
    case layers = "Layers"
    case dataGraphics = "Graphics"
    case validation = "Validate"

    public var id: String { rawValue }

    public var iconName: String {
        switch self {
        case .style: return "paintbrush.pointed"
        case .geometry: return "ruler"
        case .shapeData: return "tablecells"
        case .layers: return "square.3.layers.3d"
        case .dataGraphics: return "chart.bar.xaxis"
        case .validation: return "checkmark.shield"
        }
    }
}

// MARK: - FocusedValue Key for Global macOS Menu Bar Commands

public struct VexioFocusedSessionKey: FocusedValueKey {
    public typealias Value = VexioDocumentSession
}

public extension FocusedValues {
    var vexioDocumentSession: VexioDocumentSession? {
        get { self[VexioFocusedSessionKey.self] }
        set { self[VexioFocusedSessionKey.self] = newValue }
    }
}

// MARK: - Complete macOS Liquid Glass Workstation View

public struct VexioWorkspaceView: View {
    @ObservedObject public var session: VexioDocumentSession
    public let fileURL: URL?

    @Environment(\.undoManager) private var undoManager

    @State private var selectedInspectorTab: VexioInspectorTab = .style
    @State private var stencilSearchQuery: String = ""
    @State private var selectedStencilCategory: String = "All"

    // ShapeSheet Editor State
    @State private var selectedShapeSheetSection: ShapeSheetSectionCategory?
    @State private var activeFormulaCellName: String = "Width"
    @State private var activeFormulaDraft: String = "1.50 in"
    @State private var formulaWriteMode: FormulaWriteMode = .respectGuard

    // Export & Offscreen Preview State
    @State private var activeExportContentType: UTType = .visioDrawing
    @State private var activeExportFilename: String = "Untitled Diagram.vsdx"
    @State private var preparedExportDocument: VexioExportFileDocument?
    @State private var isShowingFileExporter: Bool = false
    @State private var renderedPreviewImage: CGImage?
    @State private var isShowingPreviewSheet: Bool = false

    public init(session: VexioDocumentSession, fileURL: URL?) {
        self.session = session
        self.fileURL = fileURL
    }

    public var body: some View {
        GeometryReader { windowGeo in
            let isCompactStageManagerWidth = windowGeo.size.width < 1120

            ZStack(alignment: .top) {
                // Main Horizontal Split: Vibrancy Stencils | Live Metal Canvas + ShapeSheet | Vibrancy Inspector
                HStack(spacing: 0) {
                    if session.showStencilNavigator && !isCompactStageManagerWidth {
                        VexioStencilNavigatorSidebar(
                            session: session,
                            searchQuery: $stencilSearchQuery,
                            selectedCategory: $selectedStencilCategory
                        )
                        .frame(width: 248)
                        .transition(.move(edge: .leading).combined(with: .opacity))

                        Divider()
                    }

                    // Center Column: Full-bleed Metal Canvas + Floating Liquid Glass Overlays + Collapsible ShapeSheet
                    VStack(spacing: 0) {
                        ZStack {
                            MetalDiagramCanvas(session: session)
                                .ignoresSafeArea(edges: .top)

                            canvasRefractiveOverlays
                        }
                        .frame(maxWidth: .infinity, maxHeight: .infinity)

                        if session.showShapeSheetDrawer {
                            Divider()
                            VexioShapeSheetFormulaDrawer(
                                session: session,
                                selectedSection: $selectedShapeSheetSection,
                                activeCellName: $activeFormulaCellName,
                                formulaDraft: $activeFormulaDraft,
                                writeMode: $formulaWriteMode
                            )
                            .frame(height: min(max(windowGeo.size.height * 0.30, 210), 310))
                            .transition(.move(edge: .bottom).combined(with: .opacity))
                        }
                    }

                    if session.showInspectorSidebar {
                        Divider()
                        VexioInspectorSidebar(
                            session: session,
                            selectedTab: $selectedInspectorTab,
                            onInspectCellInShapeSheet: { cellName in
                                focusCellInShapeSheet(cellName, preferSetF: false)
                            },
                            onRequestDataVisualizerExport: triggerDataVisualizerXlsxExport
                        )
                        .frame(width: isCompactStageManagerWidth ? 276 : 308)
                        .transition(.move(edge: .trailing).combined(with: .opacity))
                    }
                }

                // Dynamic Liquid Glass Top Toolbar floating over workspace
                VexioLiquidGlassTopToolbar(
                    session: session,
                    onRequestExport: triggerExport,
                    onRequestDataVisualizerExport: triggerDataVisualizerXlsxExport,
                    onRequestOffscreenPreview: generateAndShowOffscreenPreview
                )
                .padding(.horizontal, 14)
                .padding(.top, 8)
            }
        }
        .frame(minWidth: 900, minHeight: 600)
        .focusedSceneValue(\.vexioDocumentSession, session)
        .onAppear {
            if let fileURL {
                session.documentTitle = fileURL.deletingPathExtension().lastPathComponent
            }
            session.reloadSelectedShapeSheetCells()
            syncFormulaBarFromSelection(cellName: activeFormulaCellName)
        }
        .userActivity(VexioDocumentSession.continuityEditingActivityType, isActive: true) { activity in
            session.populateUserActivity(activity, fileURL: fileURL)
        }
        .onContinueUserActivity(VexioDocumentSession.continuityEditingActivityType) { activity in
            session.restoreFromUserActivity(activity)
            syncFormulaBarFromSelection(cellName: activeFormulaCellName)
        }
        .fileExporter(
            isPresented: $isShowingFileExporter,
            document: preparedExportDocument,
            contentType: activeExportContentType,
            defaultFilename: activeExportFilename
        ) { result in
            switch result {
            case .success(let savedURL):
                session.statusMessage = "Saved export to \(savedURL.lastPathComponent)"
            case .failure(let error):
                session.statusMessage = "Export cancelled or failed: \(error.localizedDescription)"
            }
        }
        .sheet(isPresented: $isShowingPreviewSheet) {
            VexioRenderedPreviewModal(
                title: session.documentTitle,
                cgImage: renderedPreviewImage,
                onClose: { isShowingPreviewSheet = false }
            )
        }
    }

    // MARK: - Refractive Canvas Overlays (Tool Palette, Guard Banner, Compare Pill, Force Touch Popover, Bottom Pills)

    private var canvasRefractiveOverlays: some View {
        ZStack {
            // Left Floating Liquid Glass Tool Palette
            HStack {
                VStack {
                    Spacer().frame(height: 68)
                    LiquidGlassFloatingPalette(selectedTool: $session.activeTool)
                    Spacer()
                }
                .padding(.leading, 14)
                Spacer()
            }

            // Top GUARD / Formula Diagnostic Banner
            if let bannerError = session.lastBannerError {
                VStack {
                    Spacer().frame(height: 66)
                    HStack(spacing: 10) {
                        Image(systemName: "lock.trianglebadge.exclamationmark.fill")
                            .foregroundStyle(.orange)
                        Text(bannerError.localizedDescription)
                            .font(.system(size: 12, weight: .medium))
                            .lineLimit(2)

                        if case .cellGuarded(let guardedCell) = bannerError {
                            Button("Override via SETF") {
                                focusCellInShapeSheet(guardedCell, preferSetF: true)
                                session.lastBannerError = nil
                            }
                            .buttonStyle(.borderedProminent)
                            .controlSize(.small)
                        }

                        Button {
                            withAnimation(LiquidGlassPhysics.standardSpring) {
                                session.lastBannerError = nil
                            }
                        } label: {
                            Image(systemName: "xmark.circle.fill")
                                .foregroundStyle(.secondary)
                        }
                        .buttonStyle(.plain)
                    }
                    .padding(.horizontal, 14)
                    .padding(.vertical, 9)
                    .liquidGlassSurface(cornerRadius: 14, elevation: .quickPopover, tint: .orange)
                    Spacer()
                }
                .padding(.horizontal, 76)
                .transition(.move(edge: .top).combined(with: .opacity))
            }

            // Floating Liquid Glass Diagram Compare Summary Card
            if !session.latestDiffSummary.isEmpty {
                VStack {
                    Spacer().frame(height: session.lastBannerError == nil ? 66 : 118)
                    HStack {
                        Spacer()
                        VStack(alignment: .leading, spacing: 6) {
                            HStack(spacing: 8) {
                                Image(systemName: "arrow.left.arrow.right.square.fill")
                                    .foregroundStyle(Color.accentColor)
                                Text("Diagram Compare")
                                    .font(.system(size: 12, weight: .semibold))
                                Spacer()
                                Button("Pin Baseline") {
                                    session.captureComparisonBaseline()
                                }
                                .buttonStyle(.bordered)
                                .controlSize(.mini)

                                Button {
                                    withAnimation(LiquidGlassPhysics.standardSpring) {
                                        session.latestDiffSummary = ""
                                        session.latestDiffEntries = []
                                    }
                                } label: {
                                    Image(systemName: "xmark.circle.fill")
                                        .foregroundStyle(.secondary)
                                }
                                .buttonStyle(.plain)
                            }

                            Text(session.latestDiffSummary)
                                .font(.system(size: 11.5, weight: .medium, design: .monospaced))

                            ForEach(session.latestDiffEntries.prefix(4)) { entry in
                                Text("• \(entry.summary)")
                                    .font(.system(size: 10.5))
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                        }
                        .padding(12)
                        .frame(width: 360)
                        .liquidGlassSurface(cornerRadius: 14, elevation: .quickPopover, tint: .accentColor)
                    }
                    .padding(.trailing, 16)
                    Spacer()
                }
                .transition(.move(edge: .top).combined(with: .opacity))
            }

            // Force Touch Deep-Click ShapeSheet Quick-Popover
            if let inspectContext = session.forceTouchQuickInspect {
                ForceTouchShapeSheetQuickPopover(
                    session: session,
                    context: inspectContext,
                    onOpenFullShapeSheet: { cellName in
                        session.forceTouchQuickInspect = nil
                        focusCellInShapeSheet(cellName, preferSetF: false)
                    },
                    onDismiss: {
                        withAnimation(LiquidGlassPhysics.standardSpring) {
                            session.forceTouchQuickInspect = nil
                        }
                    }
                )
                .position(
                    x: max(inspectContext.viewLocation.x, 180),
                    y: max(inspectContext.viewLocation.y, 160)
                )
                .transition(.scale(scale: 0.90).combined(with: .opacity))
            }

            // Bottom Page Switcher, Subprocess, Compare & Zoom Liquid Glass Pill Controls
            VStack {
                Spacer()
                HStack(spacing: 12) {
                    // Page Switcher, Subprocess Drill-Down & Diagram Compare Pill
                    LiquidGlassPillCluster(cornerRadius: 14) {
                        ForEach(session.pages) { page in
                            let isCurrent = (session.activePageId == page.id)
                            Button {
                                withAnimation(LiquidGlassPhysics.standardSpring) {
                                    session.activePageId = page.id
                                    session.reloadSelectedShapeSheetCells()
                                }
                            } label: {
                                HStack(spacing: 4) {
                                    if page.isBackgroundPage {
                                        Image(systemName: "square.on.square.dashed")
                                            .font(.system(size: 10))
                                    }
                                    Text(page.title)
                                        .font(.system(size: 11.5, weight: isCurrent ? .semibold : .regular))
                                }
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(
                                    Capsule()
                                        .fill(isCurrent ? Color.accentColor.opacity(0.20) : Color.clear)
                                )
                            }
                            .buttonStyle(.plain)
                        }

                        Button {
                            withAnimation(LiquidGlassPhysics.standardSpring) {
                                session.addNewPage()
                            }
                        } label: {
                            Image(systemName: "plus.circle")
                                .font(.system(size: 12.5, weight: .medium))
                        }
                        .buttonStyle(.plain)
                        .help("Insert New Page (⇧⌘N)")

                        Divider().frame(height: 14)

                        Button {
                            withAnimation(LiquidGlassPhysics.standardSpring) {
                                session.createSubprocessDrilldown(undoManager: undoManager)
                            }
                        } label: {
                            Label("Create Subprocess", systemImage: "arrow.turn.down.right")
                                .font(.system(size: 11, weight: .medium))
                        }
                        .buttonStyle(.plain)
                        .help("Create & Link Drill-Down Subprocess Page (⇧⌘P)")

                        Divider().frame(height: 14)

                        Button {
                            withAnimation(LiquidGlassPhysics.standardSpring) {
                                session.compareWithSnapshot()
                            }
                        } label: {
                            Label("Diagram Compare", systemImage: "arrow.left.arrow.right.square")
                                .font(.system(size: 11, weight: .medium))
                        }
                        .buttonStyle(.plain)
                        .help("Compare Active Diagram Against Baseline Snapshot (⇧⌘D)")

                        Divider().frame(height: 14)

                        Text(
                            String(
                                format: "X: %.2f in  Y: %.2f in",
                                session.cursorCanvasInches.x,
                                session.cursorCanvasInches.y
                            )
                        )
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(.secondary)
                    }

                    Spacer()

                    // Zoom & Viewport Reset Pill
                    LiquidGlassPillCluster(cornerRadius: 14) {
                        LiquidGlassIconButton(systemImage: "minus.magnifyingglass", title: "Zoom Out") {
                            session.zoomScale = max(session.zoomScale / 1.25, 0.10)
                        }

                        Slider(value: $session.zoomScale, in: 0.10...8.0)
                            .frame(width: 96)
                            .controlSize(.small)

                        Text("\(Int((session.zoomScale * 100.0).rounded()))%")
                            .font(.system(size: 11.5, weight: .medium, design: .monospaced))
                            .frame(width: 46, alignment: .trailing)

                        LiquidGlassIconButton(systemImage: "plus.magnifyingglass", title: "Zoom In") {
                            session.zoomScale = min(session.zoomScale * 1.25, 16.0)
                        }

                        Divider().frame(height: 14)

                        LiquidGlassIconButton(
                            systemImage: "arrow.up.left.and.down.right.magnifyingglass",
                            title: "Reset Zoom & Center Page (100%)"
                        ) {
                            session.zoomScale = 1.0
                            session.panOffset = .zero
                        }
                    }
                }
                .padding(.horizontal, 16)
                .padding(.bottom, 12)
            }
        }
    }

    // MARK: - Helpers

    private func focusCellInShapeSheet(_ cellName: String, preferSetF: Bool) {
        withAnimation(LiquidGlassPhysics.panelExpansionSpring) {
            session.showShapeSheetDrawer = true
        }
        activeFormulaCellName = cellName
        activeFormulaDraft = session.readCellFormula(cellName: cellName)
        if preferSetF {
            formulaWriteMode = .forceSetF
        }
    }

    private func syncFormulaBarFromSelection(cellName: String) {
        activeFormulaCellName = cellName
        activeFormulaDraft = session.readCellFormula(cellName: cellName)
    }

    private func triggerExport(_ format: VexioExportFormat) {
        do {
            activeExportContentType = format.contentType
            activeExportFilename = "\(session.documentTitle).\(format.defaultFileExtension)"
            preparedExportDocument = try session.exportDocument(as: format)
            isShowingFileExporter = true
        } catch let bridgeErr as VexioBridgeError {
            session.lastBannerError = bridgeErr
        } catch {
            session.statusMessage = error.localizedDescription
        }
    }

    private func triggerDataVisualizerXlsxExport() {
        do {
            activeExportContentType = .spreadsheet
            activeExportFilename = "\(session.documentTitle)-DataVisualizer.xlsx"
            preparedExportDocument = try session.exportDataVisualizerWorkbook()
            isShowingFileExporter = true
        } catch let bridgeErr as VexioBridgeError {
            session.lastBannerError = bridgeErr
        } catch {
            session.statusMessage = error.localizedDescription
        }
    }

    private func generateAndShowOffscreenPreview() {
        do {
            renderedPreviewImage = try session.renderQuickLookPreview(widthPx: 1280, heightPx: 860)
            isShowingPreviewSheet = true
        } catch let bridgeErr as VexioBridgeError {
            session.lastBannerError = bridgeErr
        } catch {
            session.statusMessage = error.localizedDescription
        }
    }
}

// MARK: - Dynamic Liquid Glass Top Toolbar

private struct VexioLiquidGlassTopToolbar: View {
    @ObservedObject var session: VexioDocumentSession
    @Environment(\.undoManager) private var undoManager

    let onRequestExport: (VexioExportFormat) -> Void
    let onRequestDataVisualizerExport: () -> Void
    let onRequestOffscreenPreview: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            // Left: Sidebar toggle, Document badge, Undo / Redo
            HStack(spacing: 6) {
                LiquidGlassIconButton(
                    systemImage: "sidebar.left",
                    title: "Toggle Vibrancy Stencil Navigator (⌥⌘1)",
                    isActive: session.showStencilNavigator
                ) {
                    withAnimation(LiquidGlassPhysics.standardSpring) {
                        session.showStencilNavigator.toggle()
                    }
                }

                Divider().frame(height: 18)

                VStack(alignment: .leading, spacing: 1) {
                    HStack(spacing: 6) {
                        Text(session.documentTitle)
                            .font(.system(size: 12.5, weight: .semibold))
                            .lineLimit(1)
                        Text(".\(session.sourceFormatExtension.uppercased())")
                            .font(.system(size: 9.5, weight: .bold, design: .monospaced))
                            .padding(.horizontal, 5)
                            .padding(.vertical, 1.5)
                            .background(Capsule().fill(Color.accentColor.opacity(0.18)))
                    }
                    Text(session.statusMessage)
                        .font(.system(size: 10))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                .frame(maxWidth: 240, alignment: .leading)

                Divider().frame(height: 18)

                LiquidGlassIconButton(systemImage: "arrow.uturn.backward", title: "Undo (⌘Z)") {
                    session.performUndo(undoManager: undoManager)
                }
                LiquidGlassIconButton(systemImage: "arrow.uturn.forward", title: "Redo (⇧⌘Z)") {
                    session.performRedo(undoManager: undoManager)
                }
            }

            Spacer(minLength: 8)

            // Center: Shape Operations, Routing, Auto-Layout, Rotate & Validation
            HStack(spacing: 8) {
                Menu {
                    ForEach(VexioBooleanOperation.allCases) { op in
                        Button {
                            session.applyBooleanOperation(op, undoManager: undoManager)
                        } label: {
                            Label(op.displayName, systemImage: op.symbolName)
                        }
                    }
                } label: {
                    Label("Shape Ops", systemImage: "square.on.square")
                        .font(.system(size: 12, weight: .medium))
                }
                .menuStyle(.borderlessButton)
                .fixedSize()

                Menu {
                    ForEach(ConnectorRoutingStyle.allCases) { style in
                        Button {
                            session.setConnectorRoutingStyle(style, undoManager: undoManager)
                        } label: {
                            Label(
                                style.rawValue,
                                systemImage: session.connectorRouting == style ? "checkmark" : "arrow.triangle.branch"
                            )
                        }
                    }
                } label: {
                    Label("Routing", systemImage: "point.topleft.down.curvedto.point.bottomright.up")
                        .font(.system(size: 12, weight: .medium))
                }
                .menuStyle(.borderlessButton)
                .fixedSize()

                Menu {
                    ForEach(AutoLayoutPreset.allCases) { preset in
                        Button(preset.rawValue) {
                            session.applyAutoLayout(preset, undoManager: undoManager)
                        }
                    }
                } label: {
                    Label("Auto-Layout", systemImage: "rectangle.3.group")
                        .font(.system(size: 12, weight: .medium))
                }
                .menuStyle(.borderlessButton)
                .fixedSize()

                LiquidGlassIconButton(systemImage: "rotate.right", title: "Rotate Selected Shape 90° (⌘R)") {
                    session.rotateSelectedShape(byDegrees: 90.0, undoManager: undoManager)
                }

                Menu {
                    ForEach(ValidationStandardKind.allCases) { std in
                        Button("Validate \(std.rawValue)") {
                            session.runDiagramValidation(standard: std)
                        }
                    }
                } label: {
                    Label("Validate", systemImage: "checkmark.shield")
                        .font(.system(size: 12, weight: .medium))
                }
                .menuStyle(.borderlessButton)
                .fixedSize()
            }

            Spacer(minLength: 8)

            // Right: Preview, Export, ShapeSheet toggle, Inspector toggle
            HStack(spacing: 6) {
                LiquidGlassIconButton(
                    systemImage: "eye",
                    title: "Render Offline Quick Look RGBA Preview"
                ) {
                    onRequestOffscreenPreview()
                }

                Menu {
                    ForEach(VexioExportFormat.allCases) { format in
                        Button("Export \(format.displayName)…") {
                            onRequestExport(format)
                        }
                    }
                    Divider()
                    Button("Export Data Visualizer Workbook (.xlsx)…") {
                        onRequestDataVisualizerExport()
                    }
                } label: {
                    Label("Export", systemImage: "square.and.arrow.up")
                        .font(.system(size: 12, weight: .medium))
                }
                .menuStyle(.borderlessButton)
                .fixedSize()

                Divider().frame(height: 18)

                LiquidGlassIconButton(
                    systemImage: "function",
                    title: "Toggle ShapeSheet Formula Editor (⌥⌘S)",
                    isActive: session.showShapeSheetDrawer
                ) {
                    withAnimation(LiquidGlassPhysics.panelExpansionSpring) {
                        session.showShapeSheetDrawer.toggle()
                    }
                }

                LiquidGlassIconButton(
                    systemImage: "sidebar.right",
                    title: "Toggle Inspector Sidebar (⌥⌘2)",
                    isActive: session.showInspectorSidebar
                ) {
                    withAnimation(LiquidGlassPhysics.standardSpring) {
                        session.showInspectorSidebar.toggle()
                    }
                }
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .liquidGlassSurface(cornerRadius: 16, elevation: .toolbarBar)
    }
}

// MARK: - Left-Hand Vibrancy Stencil Navigator with Search & Drag-and-Drop

private struct VexioStencilNavigatorSidebar: View {
    @ObservedObject var session: VexioDocumentSession
    @Binding var searchQuery: String
    @Binding var selectedCategory: String
    @Environment(\.undoManager) private var undoManager

    private let categories = [
        "All",
        "Basic Flowchart",
        "BPMN 2.0",
        "UML 2.5",
        "Network & Cloud",
        "Floor Plan & CAD",
    ]

    private var filteredMasters: [VexioStencilMaster] {
        VexioDocumentSession.builtInStencilCatalog.filter { master in
            let matchesCategory = (selectedCategory == "All") || (master.category == selectedCategory)
            let trimmed = searchQuery.trimmingCharacters(in: .whitespacesAndNewlines)
            let matchesSearch = trimmed.isEmpty
                || master.name.localizedCaseInsensitiveContains(trimmed)
                || master.category.localizedCaseInsensitiveContains(trimmed)
            return matchesCategory && matchesSearch
        }
    }

    var body: some View {
        VibrancySidebarContainer(material: .sidebar, blendingMode: .behindWindow) {
            VStack(alignment: .leading, spacing: 10) {
                Spacer().frame(height: 54)

                Text("STENCILS & MASTERS")
                    .font(.system(size: 10.5, weight: .bold))
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 12)

                // Search Field
                HStack(spacing: 6) {
                    Image(systemName: "magnifyingglass")
                        .foregroundStyle(.secondary)
                        .font(.system(size: 12))
                    TextField("Search BPMN, UML, CAD…", text: $searchQuery)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12))
                    if !searchQuery.isEmpty {
                        Button {
                            searchQuery = ""
                        } label: {
                            Image(systemName: "xmark.circle.fill")
                                .foregroundStyle(.secondary)
                                .font(.system(size: 11))
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 9)
                .padding(.vertical, 6)
                .background(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(Color.primary.opacity(0.06))
                )
                .padding(.horizontal, 12)

                // Category Filter Pills
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(categories, id: \.self) { cat in
                            let active = (selectedCategory == cat)
                            Button {
                                withAnimation(LiquidGlassPhysics.standardSpring) {
                                    selectedCategory = cat
                                }
                            } label: {
                                Text(cat)
                                    .font(.system(size: 11, weight: active ? .semibold : .regular))
                                    .padding(.horizontal, 8)
                                    .padding(.vertical, 4)
                                    .background(
                                        Capsule()
                                            .fill(active ? Color.accentColor.opacity(0.22) : Color.primary.opacity(0.05))
                                    )
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.horizontal, 12)
                }

                // Draggable Stencil Grid
                ScrollView {
                    LazyVGrid(
                        columns: [GridItem(.flexible(), spacing: 8), GridItem(.flexible(), spacing: 8)],
                        spacing: 8
                    ) {
                        ForEach(filteredMasters) { master in
                            VStack(spacing: 6) {
                                ZStack(alignment: .topTrailing) {
                                    Image(systemName: master.symbolName)
                                        .font(.system(size: 22, weight: .regular))
                                        .foregroundStyle(Color.accentColor)
                                        .frame(maxWidth: .infinity, minHeight: 36)

                                    if master.isGuardedAspect {
                                        Image(systemName: "lock.shield.fill")
                                            .font(.system(size: 9))
                                            .foregroundStyle(.orange)
                                            .help("GUARD(Aspect) Master")
                                    }
                                }

                                Text(master.name)
                                    .font(.system(size: 11, weight: .medium))
                                    .lineLimit(1)

                                Text(master.category)
                                    .font(.system(size: 9.5))
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                            .padding(8)
                            .background(
                                RoundedRectangle(cornerRadius: 10, style: .continuous)
                                    .fill(Color.primary.opacity(0.05))
                            )
                            .overlay(
                                RoundedRectangle(cornerRadius: 10, style: .continuous)
                                    .strokeBorder(Color.primary.opacity(0.08), lineWidth: 0.8)
                            )
                            .contentShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                            .onTapGesture(count: 2) {
                                session.instantiateStencilMaster(
                                    masterId: master.id,
                                    atCanvasInches: CGPoint(x: 4.25, y: 5.50),
                                    undoManager: undoManager
                                )
                            }
                            .onDrag {
                                NSItemProvider(object: master.id as NSString)
                            }
                            .help("Drag onto canvas or double-click to instantiate '\(master.name)'")
                        }
                    }
                    .padding(.horizontal, 12)
                    .padding(.bottom, 16)
                }
            }
        }
    }
}

// MARK: - Right-Hand Native Inspector Sidebar (Style, Geometry, Prop.*, Layers, Data Graphics, Validation)

private struct VexioInspectorSidebar: View {
    @ObservedObject var session: VexioDocumentSession
    @Binding var selectedTab: VexioInspectorTab
    let onInspectCellInShapeSheet: (String) -> Void
    let onRequestDataVisualizerExport: () -> Void

    @Environment(\.undoManager) private var undoManager

    // Local editing buffers
    @State private var newPropertyKey: String = ""
    @State private var newPropertyFormula: String = "\"Value\""

    var body: some View {
        VibrancySidebarContainer(material: .sidebar, blendingMode: .behindWindow) {
            VStack(alignment: .leading, spacing: 10) {
                Spacer().frame(height: 54)

                // Active Sheet Header
                HStack {
                    Label("Sheet.\(session.selectedShapeId)", systemImage: "cube.transparent")
                        .font(.system(size: 12, weight: .semibold, design: .monospaced))
                    Spacer()
                    Text(session.pages.first(where: { $0.id == session.activePageId })?.title ?? "Page-\(session.activePageId)")
                        .font(.system(size: 10.5))
                        .foregroundStyle(.secondary)
                }
                .padding(.horizontal, 12)

                // Segmented Inspector Mode Bar
                HStack(spacing: 4) {
                    ForEach(VexioInspectorTab.allCases) { tab in
                        let isSelected = (selectedTab == tab)
                        Button {
                            withAnimation(LiquidGlassPhysics.standardSpring) {
                                selectedTab = tab
                            }
                        } label: {
                            VStack(spacing: 2) {
                                Image(systemName: tab.iconName)
                                    .font(.system(size: 12, weight: isSelected ? .semibold : .regular))
                                Text(tab.rawValue)
                                    .font(.system(size: 9.5, weight: isSelected ? .semibold : .regular))
                            }
                            .foregroundStyle(isSelected ? Color.accentColor : Color.secondary)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 5)
                            .background(
                                RoundedRectangle(cornerRadius: 8, style: .continuous)
                                    .fill(isSelected ? Color.accentColor.opacity(0.16) : Color.clear)
                            )
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 10)

                Divider()

                ScrollView {
                    VStack(alignment: .leading, spacing: 14) {
                        switch selectedTab {
                        case .style:
                            styleInspectorSection
                        case .geometry:
                            geometryInspectorSection
                        case .shapeData:
                            shapeDataInspectorSection
                        case .layers:
                            layersInspectorSection
                        case .dataGraphics:
                            dataGraphicsInspectorSection
                        case .validation:
                            validationInspectorSection
                        }
                    }
                    .padding(12)
                }
            }
        }
    }

    // 1. Style Tab (6x4 Document Themes, Visual Effects Presets & Fill/Stroke)
    private var styleInspectorSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("DOCUMENT THEMES (6×4 MATRIX)")
                .font(.system(size: 10.5, weight: .bold))
                .foregroundStyle(.secondary)

            LazyVGrid(
                columns: [GridItem(.flexible(), spacing: 6), GridItem(.flexible(), spacing: 6)],
                spacing: 6
            ) {
                ForEach(VexioThemePreset.allCases) { theme in
                    let isSelectedTheme = (session.selectedThemeSlug == theme.rawValue)
                    Button {
                        withAnimation(LiquidGlassPhysics.standardSpring) {
                            session.applyThemeAndVariant(
                                themeSlug: theme.rawValue,
                                variantIndex: session.selectedVariantIndex,
                                undoManager: undoManager
                            )
                        }
                    } label: {
                        HStack(spacing: 6) {
                            HStack(spacing: -3) {
                                Circle()
                                    .fill(theme.primaryPreviewColor)
                                    .frame(width: 11, height: 11)
                                Circle()
                                    .fill(theme.accentPreviewColor)
                                    .frame(width: 11, height: 11)
                            }
                            Text(theme.displayName)
                                .font(.system(size: 11, weight: isSelectedTheme ? .semibold : .regular))
                                .lineLimit(1)
                            Spacer(minLength: 0)
                        }
                        .padding(.horizontal, 8)
                        .padding(.vertical, 6)
                        .background(
                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                .fill(isSelectedTheme ? Color.accentColor.opacity(0.18) : Color.primary.opacity(0.05))
                        )
                        .overlay(
                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                .strokeBorder(
                                    isSelectedTheme ? Color.accentColor.opacity(0.65) : Color.primary.opacity(0.08),
                                    lineWidth: 0.9
                                )
                        )
                    }
                    .buttonStyle(.plain)
                }
            }

            HStack(spacing: 6) {
                Text("Variant")
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(.secondary)
                Spacer()
                ForEach(0..<4, id: \.self) { variantIdx in
                    let isCurrentVariant = (Int(session.selectedVariantIndex) == variantIdx)
                    Button("V\(variantIdx + 1)") {
                        withAnimation(LiquidGlassPhysics.standardSpring) {
                            session.applyThemeAndVariant(
                                themeSlug: session.selectedThemeSlug,
                                variantIndex: UInt8(variantIdx),
                                undoManager: undoManager
                            )
                        }
                    }
                    .buttonStyle(.bordered)
                    .tint(isCurrentVariant ? .accentColor : nil)
                    .controlSize(.small)
                }
            }

            Divider()

            Text("SHAPE VISUAL EFFECTS")
                .font(.system(size: 10.5, weight: .bold))
                .foregroundStyle(.secondary)

            LazyVGrid(
                columns: [GridItem(.flexible(), spacing: 6), GridItem(.flexible(), spacing: 6)],
                spacing: 6
            ) {
                ForEach(VexioVisualEffectPreset.allCases) { preset in
                    let isSelectedPreset = (session.activeVisualPresetSlug == preset.rawValue)
                    Button {
                        session.applyVisualPreset(preset, undoManager: undoManager)
                    } label: {
                        HStack(spacing: 5) {
                            Image(systemName: preset.symbolName)
                                .font(.system(size: 11, weight: .medium))
                                .foregroundStyle(isSelectedPreset ? Color.accentColor : Color.secondary)
                            Text(preset.displayName)
                                .font(.system(size: 10.5, weight: isSelectedPreset ? .semibold : .regular))
                                .lineLimit(1)
                            Spacer(minLength: 0)
                        }
                        .padding(.horizontal, 7)
                        .padding(.vertical, 5.5)
                        .background(
                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                .fill(isSelectedPreset ? Color.accentColor.opacity(0.16) : Color.primary.opacity(0.04))
                        )
                    }
                    .buttonStyle(.plain)
                }
            }

            Divider()

            Text("FILL & STROKE FORMAT")
                .font(.system(size: 10.5, weight: .bold))
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 6) {
                Text("FillForeground (RGB Formula)")
                    .font(.system(size: 11.5, weight: .medium))
                HStack(spacing: 8) {
                    ForEach(
                        [
                            ("Azure", "RGB(235,244,255)", Color.blue.opacity(0.25)),
                            ("Mint", "RGB(231,250,239)", Color.green.opacity(0.25)),
                            ("Amber", "RGB(255,246,229)", Color.orange.opacity(0.25)),
                            ("Iris", "RGB(243,236,255)", Color.purple.opacity(0.25)),
                        ],
                        id: \.0
                    ) { label, formula, previewColor in
                        Button {
                            try? session.setCellFormula(
                                cellName: "FillForegnd",
                                formula: formula,
                                mode: .respectGuard,
                                undoManager: undoManager
                            )
                        } label: {
                            Circle()
                                .fill(previewColor)
                                .overlay(Circle().strokeBorder(Color.primary.opacity(0.25), lineWidth: 1))
                                .frame(width: 24, height: 24)
                        }
                        .buttonStyle(.plain)
                        .help("\(label): \(formula)")
                    }
                }
                Text("Current: \(session.readCellFormula(cellName: "FillForegnd"))")
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(.secondary)
            }

            Divider()

            VStack(alignment: .leading, spacing: 6) {
                Text("LineWeight")
                    .font(.system(size: 11.5, weight: .medium))
                HStack(spacing: 6) {
                    ForEach(["0.75 pt", "1.25 pt", "2.00 pt", "3.50 pt"], id: \.self) { weight in
                        Button(weight) {
                            try? session.setCellFormula(
                                cellName: "LineWeight",
                                formula: weight,
                                mode: .respectGuard,
                                undoManager: undoManager
                            )
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                    }
                }
            }

            VStack(alignment: .leading, spacing: 6) {
                Text("Corner Rounding")
                    .font(.system(size: 11.5, weight: .medium))
                HStack(spacing: 6) {
                    ForEach(["0 in", "0.08 in", "0.18 in"], id: \.self) { radius in
                        Button(radius) {
                            try? session.setCellFormula(
                                cellName: "Rounding",
                                formula: radius,
                                mode: .respectGuard,
                                undoManager: undoManager
                            )
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                    }
                }
            }
        }
    }

    // 2. Geometry, Boolean Shape Ops & GUARD Protection Tab
    private var geometryInspectorSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("BOOLEAN SHAPE OPERATIONS")
                .font(.system(size: 10.5, weight: .bold))
                .foregroundStyle(.secondary)

            LazyVGrid(
                columns: [GridItem(.flexible(), spacing: 6), GridItem(.flexible(), spacing: 6)],
                spacing: 6
            ) {
                ForEach(VexioBooleanOperation.allCases) { op in
                    Button {
                        session.applyBooleanOperation(op, undoManager: undoManager)
                    } label: {
                        Label(op.displayName, systemImage: op.symbolName)
                            .font(.system(size: 11, weight: .medium))
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
            }

            Divider()

            Text("SHAPE TRANSFORM & GUARD")
                .font(.system(size: 10.5, weight: .bold))
                .foregroundStyle(.secondary)

            ForEach(["PinX", "PinY", "Width", "Height", "LocPinX", "LocPinY", "Angle"], id: \.self) { cell in
                InspectorCellRowEditor(
                    cellName: cell,
                    formula: session.readCellFormula(cellName: cell),
                    evaluated: session.readCellEvaluatedValue(cellName: cell),
                    onCommitStandard: { updated in
                        try? session.setCellFormula(
                            cellName: cell,
                            formula: updated,
                            mode: .respectGuard,
                            undoManager: undoManager
                        )
                    },
                    onInspectInShapeSheet: {
                        onInspectCellInShapeSheet(cell)
                    }
                )
            }

            Divider()

            Text("Formula Protection Actions")
                .font(.system(size: 11, weight: .semibold))
            HStack(spacing: 8) {
                Button("GUARD(Width)") {
                    let base = session.readCellEvaluatedValue(cellName: "Width")
                    try? session.setCellFormula(
                        cellName: "Width",
                        formula: "GUARD(\(base.isEmpty ? "1.50 in" : base))",
                        mode: .forceSetF,
                        undoManager: undoManager
                    )
                }
                .buttonStyle(.bordered)
                .controlSize(.small)

                Button("Unlock Width (SETF)") {
                    try? session.setCellFormula(
                        cellName: "Width",
                        formula: "1.50 in",
                        mode: .forceSetF,
                        undoManager: undoManager
                    )
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
            }
        }
    }

    // 3. Shape Data (Prop.*) & Plan 2 Data / Cloud / Directory Connectors Tab
    private var shapeDataInspectorSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("PLAN 2 DATA, CLOUD & DIRECTORY")
                .font(.system(size: 10.5, weight: .bold))
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 6) {
                    Button {
                        session.importDataVisualizerSample(undoManager: undoManager)
                    } label: {
                        Label("Data Visualizer", systemImage: "tablecells.badge.ellipsis")
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)

                    Button("Export .xlsx") {
                        onRequestDataVisualizerExport()
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }

                Button {
                    session.importCloudTopologySample(undoManager: undoManager)
                } label: {
                    Label("Cloud Discovery (AWS / Azure / GCP)", systemImage: "cloud.fill")
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .buttonStyle(.bordered)
                .controlSize(.small)

                Button {
                    session.importEntraLdifSample(undoManager: undoManager)
                } label: {
                    Label("Entra ID / LDAP (.ldif) Org Chart", systemImage: "person.3.sequence.fill")
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .buttonStyle(.bordered)
                .controlSize(.small)

                HStack(spacing: 6) {
                    Button {
                        session.applyRemoteCrdtBatch()
                    } label: {
                        Label("CRDT Sync", systemImage: "arrow.triangle.2.circlepath")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)

                    Button {
                        session.compareWithSnapshot()
                    } label: {
                        Label("Compare", systemImage: "arrow.left.arrow.right.square")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
            }
            .padding(8)
            .background(RoundedRectangle(cornerRadius: 8).fill(Color.primary.opacity(0.04)))

            Divider()

            Text("CUSTOM SHAPE DATA (Prop.*)")
                .font(.system(size: 10.5, weight: .bold))
                .foregroundStyle(.secondary)

            ForEach($session.shapeDataProperties) { $prop in
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text(prop.label)
                            .font(.system(size: 11.5, weight: .semibold))
                        Spacer()
                        Text(prop.rowKey)
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundStyle(.secondary)
                    }
                    HStack(spacing: 6) {
                        TextField("Formula", text: $prop.formula)
                            .textFieldStyle(.roundedBorder)
                            .font(.system(size: 11.5, design: .monospaced))
                            .onSubmit {
                                try? session.setCellFormula(
                                    cellName: prop.rowKey,
                                    formula: prop.formula,
                                    mode: .respectGuard,
                                    undoManager: undoManager
                                )
                            }
                        Button("Apply") {
                            try? session.setCellFormula(
                                cellName: prop.rowKey,
                                formula: prop.formula,
                                mode: .respectGuard,
                                undoManager: undoManager
                            )
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                    }
                }
                .padding(8)
                .background(RoundedRectangle(cornerRadius: 8).fill(Color.primary.opacity(0.04)))
            }

            Divider()

            Text("Add Custom Prop.* Row")
                .font(.system(size: 11, weight: .semibold))
            HStack(spacing: 6) {
                TextField("Row Name (e.g. Owner)", text: $newPropertyKey)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: 11))
                TextField("\"Engineering\"", text: $newPropertyFormula)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: 11, design: .monospaced))
            }
            Button("Define Prop.* Cell") {
                let sanitized = newPropertyKey
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                    .replacingOccurrences(of: " ", with: "")
                guard !sanitized.isEmpty else { return }
                let rowKey = sanitized.hasPrefix("Prop.") ? sanitized : "Prop.\(sanitized)"
                session.shapeDataProperties.append(
                    ShapeDataPropertyItem(
                        rowKey: rowKey,
                        label: sanitized,
                        dataType: "String",
                        formula: newPropertyFormula,
                        evaluatedValue: newPropertyFormula.replacingOccurrences(of: "\"", with: "")
                    )
                )
                try? session.setCellFormula(
                    cellName: rowKey,
                    formula: newPropertyFormula,
                    mode: .forceSetF,
                    undoManager: undoManager
                )
                newPropertyKey = ""
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
        }
    }

    // 4. Layers Tab
    private var layersInspectorSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("VISIO & CAD LAYER MANAGER")
                .font(.system(size: 10.5, weight: .bold))
                .foregroundStyle(.secondary)

            ForEach($session.layers) { $layer in
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text(layer.name)
                            .font(.system(size: 12, weight: .medium))
                        Spacer()
                        Button("Assign Shape") {
                            try? session.setCellFormula(
                                cellName: "LayerMember",
                                formula: "\"\(layer.id)\"",
                                mode: .respectGuard,
                                undoManager: undoManager
                            )
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.mini)
                    }

                    HStack(spacing: 12) {
                        Toggle("Visible", isOn: $layer.isVisible)
                        Toggle("Lock", isOn: $layer.isLocked)
                        Toggle("Snap", isOn: $layer.isSnapGlueEnabled)
                    }
                    .toggleStyle(.checkbox)
                    .font(.system(size: 11))
                }
                .padding(8)
                .background(RoundedRectangle(cornerRadius: 8).fill(Color.primary.opacity(0.04)))
            }
        }
    }

    // 5. Data Graphics Tab
    private var dataGraphicsInspectorSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("DATA GRAPHICS & LEGENDS")
                .font(.system(size: 10.5, weight: .bold))
                .foregroundStyle(.secondary)

            Toggle("Enable Live Data Graphics Overlay", isOn: $session.dataGraphicConfig.isEnabled)
                .font(.system(size: 12, weight: .medium))

            Group {
                LabeledContent("Icon Set Binding") {
                    TextField("Prop.Status", text: $session.dataGraphicConfig.iconSetField)
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 135)
                }
                LabeledContent("Data Bar Binding") {
                    TextField("Prop.CompletionPct", text: $session.dataGraphicConfig.dataBarField)
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 135)
                }
                LabeledContent("Text Callout") {
                    TextField("Prop.Department", text: $session.dataGraphicConfig.textCalloutField)
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 135)
                }
            }
            .font(.system(size: 11.5))

            Toggle("Color by Value", isOn: $session.dataGraphicConfig.colorByValueEnabled)
                .font(.system(size: 11.5))

            Toggle("Display Data Graphic Legend on Page", isOn: $session.dataGraphicConfig.showLegendOnPage)
                .font(.system(size: 11.5))
        }
    }

    // 6. Diagram Validation Tab (BPMN 2.0 / UML 2.5 / SharePoint)
    private var validationInspectorSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("STRUCTURAL RULE VALIDATOR")
                .font(.system(size: 10.5, weight: .bold))
                .foregroundStyle(.secondary)

            HStack(spacing: 6) {
                ForEach(ValidationStandardKind.allCases) { standard in
                    Button(standard.rawValue) {
                        session.runDiagramValidation(standard: standard)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
            }

            if session.validationDiagnostics.isEmpty {
                Text("Select BPMN 2.0, UML 2.5, or SharePoint to run deterministic graph validation.")
                    .font(.system(size: 11.5))
                    .foregroundStyle(.secondary)
                    .padding(.top, 4)
            } else {
                ForEach(session.validationDiagnostics) { item in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Image(
                                systemName: item.severity == .error
                                    ? "xmark.octagon.fill"
                                    : (item.severity == .warning ? "exclamationmark.triangle.fill" : "checkmark.circle.fill")
                            )
                            .foregroundStyle(
                                item.severity == .error ? .red : (item.severity == .warning ? .orange : .green)
                            )
                            Text(item.ruleId)
                                .font(.system(size: 11, weight: .bold, design: .monospaced))
                            Spacer()
                            Text("Sheet.\(item.shapeId)")
                                .font(.system(size: 10, design: .monospaced))
                                .foregroundStyle(.secondary)
                        }
                        Text(item.message)
                            .font(.system(size: 11))
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .padding(8)
                    .background(RoundedRectangle(cornerRadius: 8).fill(Color.primary.opacity(0.05)))
                    .onTapGesture {
                        session.selectedShapeId = item.shapeId
                        session.reloadSelectedShapeSheetCells()
                    }
                }
            }
        }
    }
}

private struct InspectorCellRowEditor: View {
    let cellName: String
    let formula: String
    let evaluated: String
    let onCommitStandard: (String) -> Void
    let onInspectInShapeSheet: () -> Void

    @State private var draft: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack {
                Text(cellName)
                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                if formula.uppercased().hasPrefix("GUARD(") {
                    Label("GUARD", systemImage: "lock.fill")
                        .font(.system(size: 9.5, weight: .bold))
                        .foregroundStyle(.orange)
                }
                Spacer()
                Button("fx") {
                    onInspectInShapeSheet()
                }
                .buttonStyle(.borderless)
                .font(.system(size: 10.5, weight: .bold, design: .monospaced))
                .help("Inspect \(cellName) in ShapeSheet")
            }

            HStack(spacing: 6) {
                TextField(formula, text: $draft)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: 11, design: .monospaced))
                    .onSubmit {
                        onCommitStandard(draft)
                    }
                Text("= \(evaluated)")
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
        }
        .onAppear { draft = formula }
        .onChange(of: formula) { _, updated in draft = updated }
    }
}

// MARK: - Bottom Collapsible ShapeSheet Formula Editor

private struct VexioShapeSheetFormulaDrawer: View {
    @ObservedObject var session: VexioDocumentSession
    @Binding var selectedSection: ShapeSheetSectionCategory?
    @Binding var activeCellName: String
    @Binding var formulaDraft: String
    @Binding var writeMode: FormulaWriteMode

    @Environment(\.undoManager) private var undoManager

    private var visibleEntries: [ShapeSheetCellEntry] {
        guard let selectedSection else { return session.shapeSheetCells }
        return session.shapeSheetCells.filter { $0.section == selectedSection }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Formula Bar Header
            HStack(spacing: 10) {
                Label("Sheet.\(session.selectedShapeId)!\(activeCellName)", systemImage: "function")
                    .font(.system(size: 12, weight: .semibold, design: .monospaced))
                    .frame(minWidth: 165, alignment: .leading)

                Picker("Mode", selection: $writeMode) {
                    ForEach(FormulaWriteMode.allCases) { mode in
                        Text(mode.rawValue).tag(mode)
                    }
                }
                .pickerStyle(.menu)
                .frame(width: 220)
                .controlSize(.small)

                HStack(spacing: 6) {
                    Text("=")
                        .font(.system(size: 13, weight: .bold, design: .monospaced))
                        .foregroundStyle(.secondary)
                    TextField("Enter Visio ShapeSheet formula (e.g. GUARD(Width*0.5))", text: $formulaDraft)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12, design: .monospaced))
                        .onSubmit(commitFormula)
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 5)
                .background(
                    RoundedRectangle(cornerRadius: 7, style: .continuous)
                        .fill(Color(nsColor: .textBackgroundColor))
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 7, style: .continuous)
                        .strokeBorder(Color.primary.opacity(0.14), lineWidth: 1)
                )

                Button("Commit") {
                    commitFormula()
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)

                Button {
                    withAnimation(LiquidGlassPhysics.panelExpansionSpring) {
                        session.showShapeSheetDrawer = false
                    }
                } label: {
                    Image(systemName: "chevron.down.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .help("Collapse ShapeSheet Drawer")
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(.bar)

            Divider()

            // Section Filter Strip + Cell Grid
            HStack(spacing: 0) {
                VStack(alignment: .leading, spacing: 4) {
                    Button {
                        selectedSection = nil
                    } label: {
                        Text("All Sections")
                            .font(.system(size: 11, weight: selectedSection == nil ? .semibold : .regular))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                    }
                    .buttonStyle(.plain)

                    ForEach(ShapeSheetSectionCategory.allCases) { category in
                        let active = (selectedSection == category)
                        Button {
                            selectedSection = category
                        } label: {
                            Text(category.rawValue)
                                .font(.system(size: 11, weight: active ? .semibold : .regular))
                                .foregroundStyle(active ? Color.accentColor : Color.primary)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(
                                    RoundedRectangle(cornerRadius: 6)
                                        .fill(active ? Color.accentColor.opacity(0.14) : Color.clear)
                                )
                        }
                        .buttonStyle(.plain)
                    }
                    Spacer()
                }
                .padding(8)
                .frame(width: 175)

                Divider()

                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(visibleEntries) { entry in
                            let isSelectedRow = (entry.cellName == activeCellName)
                            HStack(spacing: 12) {
                                HStack(spacing: 5) {
                                    Text(entry.cellName)
                                        .font(.system(size: 11.5, weight: .semibold, design: .monospaced))
                                    if entry.isGuarded {
                                        Image(systemName: "lock.fill")
                                            .font(.system(size: 9.5))
                                            .foregroundStyle(.orange)
                                    }
                                }
                                .frame(width: 150, alignment: .leading)

                                Text(entry.section.rawValue)
                                    .font(.system(size: 10.5))
                                    .foregroundStyle(.secondary)
                                    .frame(width: 130, alignment: .leading)

                                Text(entry.formula)
                                    .font(.system(size: 11.5, design: .monospaced))
                                    .frame(maxWidth: .infinity, alignment: .leading)

                                Text(entry.evaluatedValue)
                                    .font(.system(size: 11.5, design: .monospaced))
                                    .foregroundStyle(.secondary)
                                    .frame(width: 140, alignment: .trailing)
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 5)
                            .background(isSelectedRow ? Color.accentColor.opacity(0.15) : Color.clear)
                            .contentShape(Rectangle())
                            .onTapGesture {
                                activeCellName = entry.cellName
                                formulaDraft = entry.formula
                            }
                            Divider()
                        }
                    }
                }
            }
            .background(Color(nsColor: .controlBackgroundColor))
        }
    }

    private func commitFormula() {
        try? session.setCellFormula(
            cellName: activeCellName,
            formula: formulaDraft,
            mode: writeMode,
            undoManager: undoManager
        )
    }
}

// MARK: - Force Touch Deep-Click ShapeSheet Quick-Popover

private struct ForceTouchShapeSheetQuickPopover: View {
    @ObservedObject var session: VexioDocumentSession
    let context: ForceTouchQuickInspectContext
    let onOpenFullShapeSheet: (String) -> Void
    let onDismiss: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Label("Force Touch Inspect · Sheet.\(context.shapeId)", systemImage: "hand.tap.fill")
                    .font(.system(size: 11.5, weight: .semibold))
                Spacer()
                Button(action: onDismiss) {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
            }

            Divider()

            ForEach(["PinX", "PinY", "Width", "Height", "Angle", "LockAspect"], id: \.self) { cell in
                HStack {
                    Text(cell)
                        .font(.system(size: 11, weight: .semibold, design: .monospaced))
                        .frame(width: 82, alignment: .leading)
                    Text(session.readCellFormula(cellName: cell))
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(.primary)
                    Spacer()
                    Button("Edit fx") {
                        onOpenFullShapeSheet(cell)
                    }
                    .buttonStyle(.borderless)
                    .font(.system(size: 10.5, weight: .medium))
                }
            }
        }
        .padding(12)
        .frame(width: 310)
        .liquidGlassSurface(cornerRadius: 16, elevation: .quickPopover)
    }
}

// MARK: - Offscreen RGBA Preview Sheet (vexio_render_preview_rgba Verification Modal)

private struct VexioRenderedPreviewModal: View {
    let title: String
    let cgImage: CGImage?
    let onClose: () -> Void

    var body: some View {
        VStack(spacing: 12) {
            HStack {
                Label("Offline RGBA Preview — \(title)", systemImage: "photo.artframe")
                    .font(.headline)
                Spacer()
                Button("Done", action: onClose)
                    .keyboardShortcut(.defaultAction)
            }

            if let cgImage {
                Image(decorative: cgImage, scale: 2.0)
                    .resizable()
                    .scaledToFit()
                    .frame(minWidth: 560, minHeight: 380)
                    .background(Color.white)
                    .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            } else {
                ContentUnavailableView(
                    "Preview Unavailable",
                    systemImage: "exclamationmark.triangle",
                    description: Text("vexio_render_preview_rgba returned an empty frame.")
                )
                .frame(width: 480, height: 300)
            }
        }
        .padding(18)
        .frame(minWidth: 620, minHeight: 460)
    }
}
