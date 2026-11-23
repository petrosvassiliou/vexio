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

// MARK: - AppKit Unified Full-Size Content Titlebar Configurator (Eliminates Excess Top Gap)

private struct MacWindowUnifiedTitlebarAccessor: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        DispatchQueue.main.async {
            if let window = view.window {
                window.styleMask.insert(.fullSizeContentView)
                window.titleVisibility = .hidden
                window.titlebarAppearsTransparent = true
            }
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        if let window = nsView.window {
            if !window.styleMask.contains(.fullSizeContentView) {
                window.styleMask.insert(.fullSizeContentView)
            }
            window.titleVisibility = .hidden
            window.titlebarAppearsTransparent = true
        }
    }
}

// MARK: - Complete macOS Liquid Glass Workstation View

public struct VexioWorkspaceView: View {
    @ObservedObject public var session: VexioDocumentSession
    @ObservedObject private var i18n: VexioLocalization = .shared
    @ObservedObject private var updater: VexioUpdateManager = .shared
    public let fileURL: URL?

    @Environment(\.undoManager) private var undoManager
    @AppStorage("vexio.skipStartScreenOnLaunch") private var skipStartScreenOnLaunch: Bool = false

    @State private var selectedInspectorTab: VexioInspectorTab = .style
    @State private var stencilSearchQuery: String = ""
    @State private var selectedStencilCategory: String = "All"
    @State private var isBackstagePresented: Bool = false
    @State private var didHandleInitialLaunchPresentation: Bool = false
    @State private var inlineLabelDraft: String = ""
    @State private var renamingPageId: UInt32? = nil
    @State private var renamePageDraft: String = ""

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
            let isCompactStageManagerWidth = windowGeo.size.width < 1160

            ZStack {
                MacWindowUnifiedTitlebarAccessor()
                    .frame(width: 0, height: 0)

                VStack(spacing: 0) {
                    // Unified Top Header (Inline Traffic Lights) + Classic Collapsible Visio Ribbon Strip
                    VexioHeaderAndRibbonBar(
                        session: session,
                        i18n: i18n,
                        updater: updater,
                        isBackstagePresented: $isBackstagePresented,
                        onRequestOpenFile: presentInWindowOpenFileSheet,
                        onRequestSaveFile: presentInWindowSaveSheet,
                        onRequestSaveAsFile: presentInWindowSaveAsSheet,
                        onRequestImportStencil: presentInWindowImportStencilSheet,
                        onRequestExport: triggerExport,
                        onRequestDataVisualizerExport: triggerDataVisualizerXlsxExport,
                        onRequestOffscreenPreview: generateAndShowOffscreenPreview
                    )
                    .padding(.horizontal, 10)
                    .padding(.top, 6)
                    .padding(.bottom, 5)

                    Divider()

                    // Main Horizontal Split: Vibrancy Stencils | Live Metal Canvas + ShapeSheet | Vibrancy Inspector
                    HStack(spacing: 0) {
                        if session.showStencilNavigator {
                            VexioStencilNavigatorSidebar(
                                session: session,
                                searchQuery: $stencilSearchQuery,
                                selectedCategory: $selectedStencilCategory,
                                onRequestImportStencil: presentInWindowImportStencilSheet
                            )
                            .frame(width: isCompactStageManagerWidth ? 236 : 268)
                            .transition(.move(edge: .leading).combined(with: .opacity))

                            Divider()
                        }

                        // Center Column: Full-bleed Metal Canvas + Floating Liquid Glass Overlays + Collapsible ShapeSheet
                        VStack(spacing: 0) {
                            ZStack(alignment: .topLeading) {
                                MetalDiagramCanvas(session: session)

                                canvasRefractiveOverlays

                                if let inlineEdit = session.activeInlineTextEdit {
                                    inlineCanvasLabelEditor(for: inlineEdit)
                                }
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
                            .frame(width: isCompactStageManagerWidth ? 296 : 332)
                            .transition(.move(edge: .trailing).combined(with: .opacity))
                        }
                    }
                }

                // In-App Welcome / File Backstage Hub (Replaces external OS Finder modal on startup)
                if isBackstagePresented {
                    VexioBackstageStartView(
                        session: session,
                        i18n: i18n,
                        updater: updater,
                        isPresented: $isBackstagePresented,
                        onOpenLocalFileSheet: presentInWindowOpenFileSheet,
                        onOpenURLDirectly: { url in
                            session.openLocalFile(from: url)
                        },
                        onImportStencilSheet: presentInWindowImportStencilSheet,
                        onSaveSheet: presentInWindowSaveSheet,
                        onSaveAsSheet: presentInWindowSaveAsSheet,
                        onExportFormat: triggerExport
                    )
                    .transition(.opacity.combined(with: .scale(scale: 0.98)))
                }
            }
        }
        .ignoresSafeArea(.container, edges: .top)
        .frame(minWidth: 960, minHeight: 620)
        .focusedSceneValue(\.vexioDocumentSession, session)
        .onAppear {
            if let fileURL {
                session.openLocalFile(from: fileURL)
            } else if !didHandleInitialLaunchPresentation {
                didHandleInitialLaunchPresentation = true
                if !skipStartScreenOnLaunch {
                    isBackstagePresented = true
                }
            }
            session.ensureStencilsLoadedIfNeeded()
            session.reloadSelectedShapeSheetCells()
            session.syncSelectedShapeMetadataFromCore()
            syncFormulaBarFromSelection(cellName: activeFormulaCellName)
            updater.checkOnLaunchIfEnabled()
        }
        .onChange(of: session.activeInlineTextEdit) { _, newValue in
            if let edit = newValue {
                inlineLabelDraft = edit.initialText
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .vexioShowFileBackstage)) { _ in
            withAnimation(LiquidGlassPhysics.standardSpring) {
                isBackstagePresented = true
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .vexioOpenFileSheet)) { _ in
            presentInWindowOpenFileSheet()
        }
        .onReceive(NotificationCenter.default.publisher(for: .vexioSaveFileSheet)) { _ in
            presentInWindowSaveSheet()
        }
        .onReceive(NotificationCenter.default.publisher(for: .vexioSaveAsFileSheet)) { _ in
            presentInWindowSaveAsSheet()
        }
        .onReceive(NotificationCenter.default.publisher(for: .vexioImportStencilSheet)) { _ in
            presentInWindowImportStencilSheet()
        }
        .userActivity(VexioDocumentSession.continuityEditingActivityType, isActive: true) { activity in
            session.populateUserActivity(activity, fileURL: session.currentFileURL ?? fileURL)
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
        .sheet(isPresented: $updater.isShowingUpdateDialog) {
            VexioUpdateDialogSheet(updater: updater, i18n: i18n)
        }
    }

    // MARK: - Inline Canvas Shape / Text Box Label Editor Overlay (Uses Selected OS Font)

    @ViewBuilder
    private func inlineCanvasLabelEditor(for edit: InlineShapeTextEditContext) -> some View {
        let align: NSTextAlignment = switch session.selectedHorizAlign {
        case 0: .left
        case 2: .right
        default: .center
        }
        let boxW = max(edit.viewRect.width, 132)
        let boxH = max(min(edit.viewRect.height, 64), 24)

        SelectAllInlineTextField(
            text: $inlineLabelDraft,
            placeholder: "",
            fontFamily: session.selectedFontFamily,
            fontSize: max(CGFloat(session.selectedFontSizePt * Double(session.zoomScale)), 11.5),
            fontWeight: (session.selectedCharStyle & 1) != 0 ? .bold : .regular,
            alignment: align,
            isCanvasInPlace: true,
            onCommit: {
                session.commitInlineEditing(newText: inlineLabelDraft, undoManager: undoManager)
            },
            onCancel: {
                session.activeInlineTextEdit = nil
            }
        )
        .padding(.horizontal, 4)
        .padding(.vertical, 2)
        .frame(width: boxW, height: boxH)
        .background(Color(nsColor: .textBackgroundColor))
        .overlay(
            RoundedRectangle(cornerRadius: 2, style: .continuous)
                .stroke(Color.accentColor, lineWidth: 1.5)
        )
        .fixedSize()
        .position(x: edit.viewRect.midX, y: edit.viewRect.midY)
    }

    // MARK: - Refractive Canvas Overlays (Tool Palette, Guard Banner, Compare Pill, Force Touch Popover, Bottom Pills)

    private var canvasRefractiveOverlays: some View {
        ZStack {
            // Left Floating Liquid Glass Tool Palette
            HStack {
                VStack {
                    Spacer().frame(height: 12)
                    LiquidGlassFloatingPalette(selectedTool: $session.activeTool)
                    Spacer()
                }
                .padding(.leading, 14)
                Spacer()
            }

            // Top GUARD / Formula Diagnostic Banner
            if let bannerError = session.lastBannerError {
                VStack {
                    Spacer().frame(height: 12)
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

            // Bottom Page Switcher (Single-Line Scrollable + Inline Rename), Subprocess, Compare & Zoom Liquid Glass Pill Controls
            VStack {
                Spacer()
                HStack(spacing: 12) {
                    LiquidGlassPillCluster(cornerRadius: 14) {
                        ScrollView(.horizontal, showsIndicators: false) {
                            HStack(spacing: 4) {
                                ForEach(session.pages) { page in
                                    let isCurrent = (session.activePageId == page.id)
                                    if renamingPageId == page.id {
                                        SelectAllInlineTextField(
                                            text: $renamePageDraft,
                                            placeholder: page.title,
                                            fontSize: 11.5,
                                            fontWeight: .semibold,
                                            onCommit: {
                                                session.renamePage(pageId: page.id, to: renamePageDraft, undoManager: undoManager)
                                                renamingPageId = nil
                                            },
                                            onCancel: {
                                                renamingPageId = nil
                                            }
                                        )
                                        .frame(width: 132, height: 22)
                                    } else {
                                        HStack(spacing: 4) {
                                            if page.isBackgroundPage {
                                                Image(systemName: "square.on.square.dashed")
                                                    .font(.system(size: 10))
                                            }
                                            Text(page.title)
                                                .font(.system(size: 11.5, weight: isCurrent ? .semibold : .regular))
                                                .lineLimit(1)
                                                .fixedSize(horizontal: true, vertical: false)
                                        }
                                        .padding(.horizontal, 8)
                                        .padding(.vertical, 4)
                                        .background(
                                            Capsule()
                                                .fill(isCurrent ? Color.accentColor.opacity(0.22) : Color.clear)
                                        )
                                        .contentShape(Capsule())
                                        .help("\(page.title) — \(i18n.t("doubleClickRenameHint"))")
                                        .onTapGesture(count: 2) {
                                            session.selectPage(page.id)
                                            renamePageDraft = page.title
                                            renamingPageId = page.id
                                        }
                                        .onTapGesture(count: 1) {
                                            withAnimation(LiquidGlassPhysics.standardSpring) {
                                                session.selectPage(page.id)
                                            }
                                        }
                                        .contextMenu {
                                            Button("Switch to \(page.title)") {
                                                session.selectPage(page.id)
                                            }
                                            Button(i18n.t("renamePage")) {
                                                session.selectPage(page.id)
                                                renamePageDraft = page.title
                                                renamingPageId = page.id
                                            }
                                            Divider()
                                            Button("Insert New Page") {
                                                session.addNewPage()
                                            }
                                            Button("Create Linked Subprocess…") {
                                                session.createSubprocessDrilldown(undoManager: undoManager)
                                            }
                                            Divider()
                                            Button("Auto-Layout Active Page") {
                                                session.applyAutoLayout(.hierarchicalTopDown, undoManager: undoManager)
                                            }
                                        }
                                    }
                                }
                            }
                            .padding(.vertical, 1)
                        }
                        .frame(maxWidth: 440, maxHeight: 28)
                        .fixedSize(horizontal: false, vertical: true)

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
                            Image(systemName: "arrow.turn.down.right")
                                .font(.system(size: 11.5, weight: .medium))
                        }
                        .buttonStyle(.plain)
                        .help("Create & Link Drill-Down Subprocess Page (⇧⌘P)")

                        Button {
                            withAnimation(LiquidGlassPhysics.standardSpring) {
                                session.compareWithSnapshot()
                            }
                        } label: {
                            Image(systemName: "arrow.left.arrow.right.square")
                                .font(.system(size: 11.5, weight: .medium))
                        }
                        .buttonStyle(.plain)
                        .help("Compare Active Diagram Against Baseline Snapshot (⇧⌘D)")

                        Divider().frame(height: 14)

                        Text(
                            "X: \(i18n.unitPreference.formatFromInches(Double(session.cursorCanvasInches.x)))  Y: \(i18n.unitPreference.formatFromInches(Double(session.cursorCanvasInches.y)))"
                        )
                        .font(.system(size: 10.5, design: .monospaced))
                        .lineLimit(1)
                        .fixedSize(horizontal: true, vertical: false)
                        .foregroundStyle(.secondary)
                    }
                    .fixedSize(horizontal: false, vertical: true)

                    Spacer(minLength: 8)

                    // Zoom & Viewport Fit Pill
                    LiquidGlassPillCluster(cornerRadius: 14) {
                        LiquidGlassIconButton(systemImage: "minus.magnifyingglass", title: "Zoom Out") {
                            session.zoomScale = max(session.zoomScale / 1.25, 0.10)
                        }

                        Slider(value: $session.zoomScale, in: 0.10...8.0)
                            .frame(width: 88)
                            .controlSize(.small)

                        Text("\(Int((session.zoomScale * 100.0).rounded()))%")
                            .font(.system(size: 11.5, weight: .medium, design: .monospaced))
                            .lineLimit(1)
                            .frame(width: 46, alignment: .trailing)

                        LiquidGlassIconButton(systemImage: "plus.magnifyingglass", title: "Zoom In") {
                            session.zoomScale = min(session.zoomScale * 1.25, 16.0)
                        }

                        Divider().frame(height: 14)

                        LiquidGlassIconButton(
                            systemImage: "arrow.up.left.and.down.right.magnifyingglass",
                            title: "Fit Page to Window (⌘0)"
                        ) {
                            session.fitViewportToActivePage()
                        }
                    }
                    .fixedSize(horizontal: true, vertical: true)
                }
                .padding(.horizontal, 16)
                .padding(.bottom, 10)
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

    // MARK: - In-Window NSOpenPanel / NSSavePanel Sheets (Save vs Save As…)

    private func presentInWindowOpenFileSheet() {
        let panel = NSOpenPanel()
        panel.title = i18n.t("openFile")
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.allowedContentTypes = [
            .visioDrawing,
            .visioMacroDrawing,
            .visioStencil,
            .visioTemplate,
            .visioLegacyDrawing,
            .autoCADDWG,
            .autoCADDXF,
            .data,
        ]
        if let window = NSApp.keyWindow ?? NSApp.windows.first {
            panel.beginSheetModal(for: window) { response in
                guard response == .OK, let url = panel.url else { return }
                Task { @MainActor in
                    isBackstagePresented = false
                    session.openLocalFile(from: url)
                }
            }
        }
    }

    private func presentInWindowImportStencilSheet() {
        let panel = NSOpenPanel()
        panel.title = i18n.t("importStencil")
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        panel.allowedContentTypes = [
            .visioStencil,
            .visioDrawing,
            .visioTemplate,
            .visioLegacyDrawing,
            .zip,
            .svg,
            .png,
            .xml,
            .data,
        ]
        if let window = NSApp.keyWindow ?? NSApp.windows.first {
            panel.beginSheetModal(for: window) { response in
                guard response == .OK else { return }
                let urls = panel.urls
                Task { @MainActor in
                    isBackstagePresented = false
                    for url in urls {
                        session.importVisioStencilFile(from: url)
                    }
                }
            }
        }
    }

    private func presentInWindowSaveSheet() {
        if let existing = session.currentFileURL {
            do {
                try session.saveToURL(existing)
                isBackstagePresented = false
                return
            } catch {
                session.statusMessage = error.localizedDescription
            }
        }
        presentInWindowSaveAsSheet()
    }

    private func presentInWindowSaveAsSheet() {
        let panel = NSSavePanel()
        panel.title = i18n.t("saveAs")
        let formats: [(label: String, ext: String)] = [
            ("Visio Drawing (.vsdx)", "vsdx"),
            ("Visio Stencil (.vssx)", "vssx"),
            ("Visio Template (.vstx)", "vstx"),
            ("Visio Legacy Binary (.vsd)", "vsd"),
            ("Visio XML Drawing (.vdx)", "vdx"),
            ("AutoCAD Drawing Exchange (.dxf)", "dxf"),
            ("Vector PDF Document (.pdf)", "pdf"),
            ("Scalable Vector Graphics (.svg)", "svg"),
            ("PNG Image (.png)", "png"),
        ]
        let popup = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 260, height: 26), pullsDown: false)
        for item in formats {
            popup.addItem(withTitle: item.label)
        }
        let accessory = NSStackView()
        accessory.orientation = .horizontal
        accessory.spacing = 8
        accessory.edgeInsets = NSEdgeInsets(top: 8, left: 12, bottom: 8, right: 12)
        let fmtLabel = NSTextField(labelWithString: "Format:")
        fmtLabel.font = NSFont.systemFont(ofSize: 12, weight: .medium)
        accessory.addArrangedSubview(fmtLabel)
        accessory.addArrangedSubview(popup)
        panel.accessoryView = accessory
        panel.nameFieldStringValue = "\(session.documentTitle).vsdx"
        panel.canCreateDirectories = true
        panel.isExtensionHidden = false

        if let window = NSApp.keyWindow ?? NSApp.windows.first {
            panel.beginSheetModal(for: window) { response in
                guard response == .OK, var chosenURL = panel.url else { return }
                let idx = max(0, min(popup.indexOfSelectedItem, formats.count - 1))
                let chosenExt = formats[idx].ext
                if chosenURL.pathExtension.lowercased() != chosenExt {
                    chosenURL = chosenURL.deletingPathExtension().appendingPathExtension(chosenExt)
                }
                Task { @MainActor in
                    isBackstagePresented = false
                    do {
                        try session.saveToURL(chosenURL)
                    } catch {
                        session.statusMessage = error.localizedDescription
                    }
                }
            }
        }
    }
}

// MARK: - Notification Names for Global macOS Menu Bar Actions

public extension Notification.Name {
    static let vexioShowFileBackstage = Notification.Name("app.vexio.showFileBackstage")
    static let vexioOpenFileSheet = Notification.Name("app.vexio.openFileSheet")
    static let vexioSaveFileSheet = Notification.Name("app.vexio.saveFileSheet")
    static let vexioSaveAsFileSheet = Notification.Name("app.vexio.saveAsFileSheet")
    static let vexioImportStencilSheet = Notification.Name("app.vexio.importStencilSheet")
    static let vexioOpenExternalURL = Notification.Name("app.vexio.openExternalURL")
}

// MARK: - Left-Hand Vibrancy Stencil Navigator (All Visio Categories + Import Visio Stencil .vssx/.vss)

private struct VexioStencilNavigatorSidebar: View {
    @ObservedObject var session: VexioDocumentSession
    @ObservedObject private var i18n: VexioLocalization = .shared
    @Binding var searchQuery: String
    @Binding var selectedCategory: String
    let onRequestImportStencil: () -> Void
    @Environment(\.undoManager) private var undoManager

    @State private var isStencilDropTargeted: Bool = false
    @State private var isOfficialCatalogPresented: Bool = false
    @State private var expandedCategories: Set<String> = [
        "Microsoft 365 & Office",
        "Microsoft Servers & Active Directory",
        "Microsoft Azure & Hybrid Cloud",
        "Network & Rack Equipment",
    ]

    private var visibleCategories: [String] {
        if selectedCategory != "All" && session.stencilCategories.contains(selectedCategory) {
            return [selectedCategory]
        }
        return session.stencilCategories
    }

    private func masters(in category: String) -> [VexioStencilMaster] {
        let trimmed = searchQuery.trimmingCharacters(in: .whitespacesAndNewlines)
        return session.stencilCatalog.filter { master in
            guard master.category == category else { return false }
            guard !trimmed.isEmpty else { return true }
            return master.name.localizedCaseInsensitiveContains(trimmed)
                || master.category.localizedCaseInsensitiveContains(trimmed)
                || master.prompt.localizedCaseInsensitiveContains(trimmed)
        }
    }

    var body: some View {
        VibrancySidebarContainer(material: .sidebar, blendingMode: .behindWindow) {
            VStack(alignment: .leading, spacing: 8) {
                Spacer().frame(height: 8)

                HStack(spacing: 6) {
                    Text("\(i18n.t("stencils").uppercased()) (\(session.stencilCategories.count))")
                        .font(.system(size: 10.5, weight: .bold))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                    Spacer()
                    Button {
                        isOfficialCatalogPresented = true
                    } label: {
                        Label("Packs", systemImage: "icloud.and.arrow.down")
                            .font(.system(size: 10.5, weight: .semibold))
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.mini)
                    .help("Download Official Microsoft 365, Azure & NetEquip Stencil Packs")

                    Button {
                        onRequestImportStencil()
                    } label: {
                        Label("Import", systemImage: "plus.rectangle.on.folder")
                            .font(.system(size: 10.5, weight: .semibold))
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.mini)
                    .help(i18n.t("importStencil"))

                    Button(expandedCategories.count >= session.stencilCategories.count ? "Collapse" : "Expand") {
                        withAnimation(LiquidGlassPhysics.standardSpring) {
                            if expandedCategories.count >= session.stencilCategories.count {
                                expandedCategories = ["Basic Shapes", "Basic Flowchart"]
                            } else {
                                expandedCategories = Set(session.stencilCategories)
                            }
                        }
                    }
                    .buttonStyle(.borderless)
                    .font(.system(size: 10.5, weight: .medium))
                }
                .padding(.horizontal, 12)

                // Search Field
                HStack(spacing: 6) {
                    Image(systemName: "magnifyingglass")
                        .foregroundStyle(.secondary)
                        .font(.system(size: 12))
                    TextField(
                        "Search \(session.stencilCatalog.count) shapes in \(session.stencilCategories.count) packs…",
                        text: $searchQuery
                    )
                    .textFieldStyle(.plain)
                    .font(.system(size: 11.5))
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

                // Category Filter Pills (All + 17 Visio Packs)
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 5) {
                        ForEach(["All"] + session.stencilCategories, id: \.self) { cat in
                            let active = (selectedCategory == cat)
                            Button {
                                withAnimation(LiquidGlassPhysics.standardSpring) {
                                    selectedCategory = cat
                                    if cat != "All" {
                                        expandedCategories.insert(cat)
                                    }
                                }
                            } label: {
                                Text(cat)
                                    .font(.system(size: 10.5, weight: active ? .semibold : .regular))
                                    .padding(.horizontal, 8)
                                    .padding(.vertical, 3.5)
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

                // Collapsible 17-Category Stencil Accordions with Crisp Vector Previews
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 8) {
                        let isSearching = !searchQuery.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        ForEach(visibleCategories, id: \.self) { category in
                            let catMasters = masters(in: category)
                            if !catMasters.isEmpty {
                                let isExpanded = isSearching
                                    || selectedCategory == category
                                    || expandedCategories.contains(category)

                                VStack(alignment: .leading, spacing: 6) {
                                    Button {
                                        withAnimation(LiquidGlassPhysics.standardSpring) {
                                            if expandedCategories.contains(category) {
                                                expandedCategories.remove(category)
                                            } else {
                                                expandedCategories.insert(category)
                                            }
                                        }
                                    } label: {
                                        HStack(spacing: 6) {
                                            Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                                                .font(.system(size: 9.5, weight: .bold))
                                                .foregroundStyle(.secondary)
                                                .frame(width: 11)
                                            Text(category)
                                                .font(.system(size: 11.5, weight: .semibold))
                                                .foregroundStyle(.primary)
                                                .lineLimit(1)
                                            Spacer()
                                            Text("\(catMasters.count)")
                                                .font(.system(size: 10, weight: .medium, design: .monospaced))
                                                .foregroundStyle(.secondary)
                                                .padding(.horizontal, 5)
                                                .padding(.vertical, 1)
                                                .background(Capsule().fill(Color.primary.opacity(0.07)))
                                        }
                                        .padding(.horizontal, 8)
                                        .padding(.vertical, 5)
                                        .background(
                                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                                .fill(Color.primary.opacity(0.04))
                                        )
                                        .contentShape(Rectangle())
                                    }
                                    .buttonStyle(.plain)

                                    if isExpanded {
                                        LazyVGrid(
                                            columns: [GridItem(.flexible(), spacing: 7), GridItem(.flexible(), spacing: 7)],
                                            spacing: 7
                                        ) {
                                            ForEach(catMasters) { master in
                                                stencilCard(for: master)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    .padding(.horizontal, 12)
                    .padding(.bottom, 16)
                }
            }
        }
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .strokeBorder(isStencilDropTargeted ? Color.accentColor : Color.clear, lineWidth: 2)
        )
        .onDrop(of: [.fileURL], isTargeted: $isStencilDropTargeted) { providers in
            for provider in providers {
                _ = provider.loadObject(ofClass: URL.self) { droppedURL, _ in
                    guard let droppedURL else { return }
                    Task { @MainActor in
                        session.importVisioStencilFile(from: droppedURL)
                    }
                }
            }
                return !providers.isEmpty
        }
        .sheet(isPresented: $isOfficialCatalogPresented) {
            VexioOfficialStencilCatalogSheet(session: session) {
                isOfficialCatalogPresented = false
            }
        }
    }

    @ViewBuilder
    private func stencilCard(for master: VexioStencilMaster) -> some View {
        VStack(spacing: 5) {
            ZStack(alignment: .topTrailing) {
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(Color.white.opacity(0.92))
                    .frame(height: 48)
                    .overlay(
                        StencilVectorPreviewCanvas(
                            svgMarkup: master.svgPreview,
                            pngDataUrl: master.pngPreview,
                            fallbackSymbol: master.symbolName
                        )
                        .padding(3)
                    )

                if master.isGuardedAspect {
                    Image(systemName: "lock.shield.fill")
                        .font(.system(size: 8.5))
                        .foregroundStyle(.orange)
                        .padding(3)
                        .help("GUARD(Aspect) Master")
                }
            }

            Text(master.name)
                .font(.system(size: 10.5, weight: .medium))
                .lineLimit(1)
        }
        .padding(6)
        .background(
            RoundedRectangle(cornerRadius: 9, style: .continuous)
                .fill(Color.primary.opacity(0.045))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 9, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.09), lineWidth: 0.8)
        )
        .contentShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
        .onTapGesture {
            session.instantiateStencilMaster(
                masterId: master.id,
                atCanvasInches: CGPoint(x: 4.50, y: 4.50),
                undoManager: undoManager
            )
        }
        .onDrag {
            NSItemProvider(object: master.id as NSString)
        }
        .contextMenu {
            Button("Add '\(master.name)' to Center of Page") {
                session.instantiateStencilMaster(
                    masterId: master.id,
                    atCanvasInches: CGPoint(x: 5.50, y: 4.25),
                    undoManager: undoManager
                )
            }
            Button("Add '\(master.name)' at Last Cursor Position") {
                let target = session.cursorCanvasInches == .zero
                    ? CGPoint(x: 4.25, y: 4.25)
                    : session.cursorCanvasInches
                session.instantiateStencilMaster(
                    masterId: master.id,
                    atCanvasInches: target,
                    undoManager: undoManager
                )
            }
            Divider()
            Button("Copy Shape Name") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(master.name, forType: .string)
            }
        }
        .help(master.prompt.isEmpty ? "Click or drag '\(master.name)' onto canvas" : "\(master.name) — \(master.prompt)")
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
                Spacer().frame(height: 8)

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

                // Segmented Inspector Mode Bar (Single-Line, Zero Wrapping)
                HStack(spacing: 3) {
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
                                    .lineLimit(1)
                                    .minimumScaleFactor(0.85)
                            }
                            .foregroundStyle(isSelected ? Color.accentColor : Color.secondary)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 5)
                            .padding(.horizontal, 2)
                            .background(
                                RoundedRectangle(cornerRadius: 8, style: .continuous)
                                    .fill(isSelected ? Color.accentColor.opacity(0.16) : Color.clear)
                            )
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 8)

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

            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    ColorPicker(
                        "Fill Color",
                        selection: Binding(
                            get: { Color(vexioHex: session.selectedFillColorHex) },
                            set: { session.applyFillColor($0, undoManager: undoManager) }
                        ),
                        supportsOpacity: false
                    )
                    .font(.system(size: 11.5, weight: .medium))

                    Spacer()

                    ColorPicker(
                        "Line Color",
                        selection: Binding(
                            get: { Color(vexioHex: session.selectedLineColorHex) },
                            set: { session.applyLineColor($0, undoManager: undoManager) }
                        ),
                        supportsOpacity: false
                    )
                    .font(.system(size: 11.5, weight: .medium))
                }

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
                                .frame(width: 22, height: 22)
                        }
                        .buttonStyle(.plain)
                        .help("\(label): \(formula)")
                    }
                }
                Text("Fill: \(session.readCellFormula(cellName: "FillForegnd"))")
                    .font(.system(size: 10, design: .monospaced))
                    .lineLimit(1)
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

private struct VexioOfficialStencilCatalogSheet: View {
    @ObservedObject var session: VexioDocumentSession
    let onClose: () -> Void

    @State private var catalog: [OfficialStencilCatalogEntry] = []
    @State private var downloadingIds: Set<String> = []
    @State private var isInstallingAll: Bool = false

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 10) {
                Label("Official & Enterprise Stencil Catalog", systemImage: "icloud.and.arrow.down.fill")
                    .font(.system(size: 16, weight: .semibold))
                Spacer()
                Button {
                    guard !isInstallingAll else { return }
                    isInstallingAll = true
                    Task { @MainActor in
                        for entry in catalog where !session.isOfficialStencilInstalled(entry) {
                            downloadingIds.insert(entry.id)
                            _ = await session.downloadOfficialStencilPack(entry)
                            downloadingIds.remove(entry.id)
                        }
                        isInstallingAll = false
                    }
                } label: {
                    if isInstallingAll {
                        HStack(spacing: 6) {
                            ProgressView().controlSize(.small)
                            Text("Installing All…")
                        }
                    } else {
                        Label("Install All Packs", systemImage: "arrow.down.circle.fill")
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(isInstallingAll)

                Button("Done", action: onClose)
                    .keyboardShortcut(.defaultAction)
            }

            Text("Downloads official Microsoft 365, Azure V25, Purview/Security, Active Directory, Cisco & NetEquip (.vssx, .vss, .zip) stencil packs directly into ~/Library/Application Support/Vexio/Stencils for 100% offline use.")
                .font(.system(size: 12))
                .foregroundStyle(.secondary)

            ScrollView {
                LazyVStack(alignment: .leading, spacing: 8) {
                    ForEach(catalog) { entry in
                        let installed = session.isOfficialStencilInstalled(entry)
                        let busy = downloadingIds.contains(entry.id)
                        HStack(alignment: .center, spacing: 12) {
                            VStack(alignment: .leading, spacing: 3) {
                                HStack(spacing: 6) {
                                    Text(entry.title)
                                        .font(.system(size: 13, weight: .semibold))
                                    Text(entry.categoryGroup)
                                        .font(.system(size: 10, weight: .medium))
                                        .padding(.horizontal, 6)
                                        .padding(.vertical, 2)
                                        .background(Capsule().fill(Color.accentColor.opacity(0.14)))
                                }
                                Text(entry.description)
                                    .font(.system(size: 11.5))
                                    .foregroundStyle(.secondary)
                                Text("\(entry.vendor) • ~\(max(1, entry.approxSizeKb / 1024 == 0 ? entry.approxSizeKb : entry.approxSizeKb / 1024)) \(entry.approxSizeKb >= 1024 ? "MB" : "KB")")
                                    .font(.system(size: 10.5))
                                    .foregroundStyle(.tertiary)
                            }
                            Spacer()
                            if busy {
                                ProgressView()
                                    .controlSize(.small)
                            } else {
                                Button(installed ? "Reinstall" : "Install") {
                                    downloadingIds.insert(entry.id)
                                    Task { @MainActor in
                                        _ = await session.downloadOfficialStencilPack(entry)
                                        downloadingIds.remove(entry.id)
                                    }
                                }
                                .buttonStyle(.bordered)
                                .tint(installed ? .green : .accentColor)
                            }
                        }
                        .padding(10)
                        .background(
                            RoundedRectangle(cornerRadius: 9, style: .continuous)
                                .fill(Color.primary.opacity(0.045))
                        )
                    }
                }
            }
        }
        .padding(18)
        .frame(minWidth: 660, minHeight: 500)
        .onAppear {
            catalog = session.listOfficialStencilCatalog()
        }
    }
}
