import AppKit
import SwiftUI
import UniformTypeIdentifiers

// MARK: - 1:1 Microsoft Visio Object Inspector Tab Enumeration

public enum VexioInspectorTab: String, CaseIterable, Identifiable, Sendable {
    case format = "format"
    case sizeAndPos = "sizePos"
    case shapeData = "shapeData"
    case layers = "layers"

    public var id: String { rawValue }

    public var iconName: String {
        switch self {
        case .format: return "paintbrush.pointed"
        case .sizeAndPos: return "ruler"
        case .shapeData: return "list.bullet.rectangle"
        case .layers: return "square.3.layers.3d"
        }
    }

    @MainActor
    public func localizedTitle(_ i18n: VexioLocalization) -> String {
        switch self {
        case .format: return i18n.t("inspectorFormat")
        case .sizeAndPos: return i18n.t("inspectorSizePos")
        case .shapeData: return i18n.t("inspectorData")
        case .layers: return i18n.t("inspectorLayers")
        }
    }

    public var tooltipDescription: String {
        switch self {
        case .format: return "Format Shape — Fill, Line, Arrows, Text & Effects"
        case .sizeAndPos: return "Size & Position — X, Y, Width, Height, Angle & Protection"
        case .shapeData: return "Shape Data (Prop.*) — Custom Properties & Data Graphics"
        case .layers: return "Layer Properties — Object Layer Membership & Visibility/Lock"
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

// MARK: - AppKit Unified Titlebar & Unsaved-Changes Window Close Delegate

private struct MacWindowUnifiedTitlebarAccessor: NSViewRepresentable {
    @ObservedObject var session: VexioDocumentSession

    @MainActor
    final class WindowCloseCoordinator: NSObject, NSWindowDelegate {
        weak var session: VexioDocumentSession?
        weak var attachedWindow: NSWindow?
        private var isApprovedToClose: Bool = false

        func bind(window: NSWindow, session: VexioDocumentSession) {
            self.session = session
            if attachedWindow !== window || window.delegate !== self {
                attachedWindow = window
                window.delegate = self
            }
            if window.isDocumentEdited != session.hasUnsavedChanges {
                window.isDocumentEdited = session.hasUnsavedChanges
            }
            if let appDel = NSApp.delegate as? VexioAppDelegate {
                appDel.registerSession(session, window: window)
            }
        }

        func windowShouldClose(_ sender: NSWindow) -> Bool {
            if isApprovedToClose {
                return true
            }
            if let appDel = NSApp.delegate as? VexioAppDelegate, appDel.isConfirmedForceTerminating {
                return true
            }
            guard let session, session.hasUnsavedChanges else {
                return true
            }
            VexioUnsavedChangesCoordinator.promptSaveChanges(for: session, attachedTo: sender) { [weak self, weak sender] shouldClose in
                guard let self, let sender, shouldClose else { return }
                self.isApprovedToClose = true
                sender.close()
            }
            return false
        }

        func windowWillClose(_ notification: Notification) {
            if let session, let appDel = NSApp.delegate as? VexioAppDelegate {
                appDel.unregisterSession(session)
            }
        }
    }

    func makeCoordinator() -> WindowCloseCoordinator {
        WindowCloseCoordinator()
    }

    private static func configure(_ window: NSWindow) {
        UserDefaults.standard.register(defaults: ["NSInitialToolTipDelay": 350])
        if !window.styleMask.contains(.fullSizeContentView) {
            window.styleMask.insert(.fullSizeContentView)
        }
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        if window.toolbar == nil {
            let tb = NSToolbar(identifier: "app.vexio.unifiedHeaderToolbar")
            window.toolbar = tb
        }
        window.toolbarStyle = .unifiedCompact
    }

    func makeNSView(context: Context) -> NSView {
        UserDefaults.standard.register(defaults: ["NSInitialToolTipDelay": 350])
        let view = NSView(frame: .zero)
        let coord = context.coordinator
        let currentSession = session
        DispatchQueue.main.async {
            if let window = view.window {
                Self.configure(window)
                coord.bind(window: window, session: currentSession)
            }
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        if let window = nsView.window {
            Self.configure(window)
            context.coordinator.bind(window: window, session: session)
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

    @State private var selectedInspectorTab: VexioInspectorTab = .format
    @State private var stencilSearchQuery: String = ""
    @State private var selectedStencilCategory: String = "All"
    @State private var isBackstagePresented: Bool = false
    @State private var didHandleInitialLaunchPresentation: Bool = false
    @State private var inlineLabelDraft: String = ""
    @State private var renamingPageId: UInt32? = nil
    @State private var renamePageDraft: String = ""
    @State private var isStencilCatalogPresented: Bool = false
    @State private var pageSetupSheetPageId: UInt32? = nil
    @State private var isBordersAndTitlesSheetPresented: Bool = false
    @State private var isMarkupStudioSheetPresented: Bool = false
    @State private var isSlideSnippetsSheetPresented: Bool = false
    @State private var isVbaStudioSheetPresented: Bool = false
    @State private var isCommentsSheetPresented: Bool = false
    @State private var isHyperlinkSheetPresented: Bool = false
    @State private var isDomainWizardsSheetPresented: Bool = false
    @State private var initialWizardSlug: String = "gantt"
    @State private var isCadToolsSheetPresented: Bool = false
    @State private var isSmartActionsSheetPresented: Bool = false
    @State private var isInsertFieldSheetPresented: Bool = false
    @State private var isFindReplaceSheetPresented: Bool = false
    @State private var isSnapGlueSheetPresented: Bool = false
    @State private var isNumberShapesSheetPresented: Bool = false

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
        workspaceWithNotifications
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
            .sheet(isPresented: $isStencilCatalogPresented) {
                VexioOfficialStencilCatalogSheet(session: session) {
                    isStencilCatalogPresented = false
                }
            }
            .sheet(isPresented: $isBordersAndTitlesSheetPresented) {
                VexioBordersAndTitlesSheet(session: session) {
                    isBordersAndTitlesSheetPresented = false
                }
            }
            .sheet(isPresented: $isMarkupStudioSheetPresented) {
                VexioMarkupStudioSheet(session: session) {
                    isMarkupStudioSheetPresented = false
                }
            }
            .sheet(isPresented: $isSlideSnippetsSheetPresented) {
                VexioSlideSnippetsSheet(session: session, onExportPptx: {
                    isSlideSnippetsSheetPresented = false
                    NotificationCenter.default.post(name: .vexioExportFormatRequested, object: "pptx")
                }) {
                    isSlideSnippetsSheetPresented = false
                }
            }
            .sheet(
                isPresented: Binding(
                    get: { pageSetupSheetPageId != nil },
                    set: { if !$0 { pageSetupSheetPageId = nil } }
                )
            ) {
                if let pid = pageSetupSheetPageId {
                    VexioPageSetupSheet(
                        session: session,
                        pageId: pid,
                        onClose: { pageSetupSheetPageId = nil }
                    )
                }
            }
    }

    private var workspaceWithMilestone1Sheets: some View {
        workspaceGeometryContent
            .sheet(isPresented: $isVbaStudioSheetPresented) {
                VexioVbaMacroStudioSheet(session: session) {
                    isVbaStudioSheetPresented = false
                }
            }
            .sheet(isPresented: $isCommentsSheetPresented) {
                VexioReviewCommentsSheet(session: session) {
                    isCommentsSheetPresented = false
                }
            }
            .sheet(isPresented: $isHyperlinkSheetPresented) {
                VexioHyperlinkScreentipSheet(session: session) {
                    isHyperlinkSheetPresented = false
                }
            }
            .sheet(isPresented: $isDomainWizardsSheetPresented) {
                VexioDomainWizardsSheet(session: session, initialSlug: initialWizardSlug) {
                    isDomainWizardsSheetPresented = false
                }
            }
            .sheet(isPresented: $isCadToolsSheetPresented) {
                VexioCadToolsSheet(session: session, onExportDwg: {
                    isCadToolsSheetPresented = false
                    NotificationCenter.default.post(name: .vexioExportFormatRequested, object: "dwg")
                }) {
                    isCadToolsSheetPresented = false
                }
            }
    }

    private var workspaceWithMilestone2Sheets: some View {
        workspaceWithMilestone1Sheets
            .sheet(isPresented: $isSmartActionsSheetPresented) {
                VexioSmartActionsSheet(session: session) {
                    isSmartActionsSheetPresented = false
                }
            }
            .sheet(isPresented: $isInsertFieldSheetPresented) {
                VexioInsertFieldSheet(session: session) {
                    isInsertFieldSheetPresented = false
                }
            }
            .sheet(isPresented: $isFindReplaceSheetPresented) {
                VexioFindReplaceSheet(session: session) {
                    isFindReplaceSheetPresented = false
                }
            }
            .sheet(isPresented: $isSnapGlueSheetPresented) {
                VexioSnapGlueSheet(session: session) {
                    isSnapGlueSheetPresented = false
                }
            }
            .sheet(isPresented: $isNumberShapesSheetPresented) {
                VexioNumberShapesSheet(session: session) {
                    isNumberShapesSheetPresented = false
                }
            }
    }

    private var workspaceWithLifecycleNotifications: some View {
        workspaceWithMilestone2Sheets
            .onAppear {
                if let fileURL {
                    session.openLocalFile(from: fileURL)
                } else if !didHandleInitialLaunchPresentation {
                    didHandleInitialLaunchPresentation = true
                    let captureOut = (ProcessInfo.processInfo.environment["VEXIO_CAPTURE_SCREENSHOT"] ?? "")
                        .trimmingCharacters(in: .whitespacesAndNewlines)
                    if !captureOut.isEmpty {
                        isBackstagePresented = false
                        scheduleAutomatedWindowScreenshot(outputPath: captureOut)
                    } else if !skipStartScreenOnLaunch {
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
            .onChange(of: isBackstagePresented) { _, presented in
                if !presented && session.isAutoFitPageMode {
                    DispatchQueue.main.async {
                        session.fitViewportToActivePage()
                    }
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
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenStencilCatalogSheet)) { _ in
                isStencilCatalogPresented = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenBordersAndTitlesSheet)) { _ in
                isBordersAndTitlesSheetPresented = true
            }
    }

    private var workspaceWithNotifications: some View {
        workspaceWithLifecycleNotifications
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenMarkupStudioSheet)) { _ in
                isMarkupStudioSheetPresented = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenSlideSnippetsSheet)) { _ in
                session.refreshSlideSnippets()
                isSlideSnippetsSheetPresented = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenVbaStudioSheet)) { _ in
                isVbaStudioSheetPresented = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenCommentsSheet)) { _ in
                isCommentsSheetPresented = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenHyperlinkSheet)) { _ in
                isHyperlinkSheetPresented = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenDomainWizardsSheet)) { note in
                if let slug = note.object as? String, !slug.isEmpty {
                    initialWizardSlug = slug
                }
                isDomainWizardsSheetPresented = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenCadToolsSheet)) { _ in
                isCadToolsSheetPresented = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenSmartActionsSheet)) { _ in
                isSmartActionsSheetPresented = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenInsertFieldSheet)) { _ in
                isInsertFieldSheetPresented = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenFindReplaceSheet)) { _ in
                isFindReplaceSheetPresented = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenSnapGlueSheet)) { _ in
                isSnapGlueSheetPresented = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenNumberShapesSheet)) { _ in
                isNumberShapesSheetPresented = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioOpenPageSetupSheet)) { note in
                if let pid = note.object as? UInt32 {
                    pageSetupSheetPageId = pid
                } else {
                    pageSetupSheetPageId = session.activePageId
                }
            }
            .onReceive(NotificationCenter.default.publisher(for: .vexioExportFormatRequested)) { note in
                if let raw = note.object as? String, let fmt = VexioExportFormat(rawValue: raw) {
                    triggerExport(fmt)
                }
            }
    }

    private var workspaceGeometryContent: some View {
        GeometryReader { windowGeo in
            let isCompactStageManagerWidth = windowGeo.size.width < 1160

            ZStack {
                MacWindowUnifiedTitlebarAccessor(session: session)
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

                    Divider()

                    // Main Horizontal Split: Vibrancy Stencils | Live Metal Canvas + ShapeSheet | Vibrancy Inspector
                    HStack(spacing: 0) {
                        if session.showStencilNavigator {
                            VexioStencilNavigatorSidebar(
                                session: session,
                                searchQuery: $stencilSearchQuery,
                                selectedCategory: $selectedStencilCategory,
                                onRequestImportStencil: presentInWindowImportStencilSheet,
                                onRequestOpenStencilCatalog: { isStencilCatalogPresented = true }
                            )
                            .frame(width: isCompactStageManagerWidth ? 258 : 288)
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
                                onRequestDataVisualizerExport: triggerDataVisualizerXlsxExport,
                                onRequestOpenPageSetup: { targetPageId in
                                    pageSetupSheetPageId = targetPageId
                                }
                            )
                            .frame(width: isCompactStageManagerWidth ? 296 : 332)
                            .transition(.move(edge: .trailing).combined(with: .opacity))
                        }
                    }
                }

                // In-App Welcome / Startup Hub (Replaces external OS Finder modal on startup)
                if isBackstagePresented {
                    VexioBackstageStartView(
                        session: session,
                        i18n: i18n,
                        updater: updater,
                        isPresented: $isBackstagePresented,
                        onOpenLocalFileSheet: presentInWindowOpenFileSheet,
                        onOpenURLDirectly: { url in
                            session.openLocalFile(from: url)
                        }
                    )
                    .transition(.opacity.combined(with: .scale(scale: 0.98)))
                }
            }
        }
        .ignoresSafeArea(.container, edges: .top)
        .frame(minWidth: 960, minHeight: 620)
        .focusedSceneValue(\.vexioDocumentSession, session)
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
            bottomLiquidGlassBar
        }
    }

    private var bottomLiquidGlassBar: some View {
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
                                        pageTabContextMenu(
                                            pageId: page.id,
                                            pageTitle: page.title,
                                            isBackground: page.isBackgroundPage
                                        )
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
                        session.setManualZoomScale(session.zoomScale / 1.25)
                    }

                    Slider(
                        value: Binding(
                            get: { session.zoomScale },
                            set: { session.setManualZoomScale($0) }
                        ),
                        in: 0.10...8.0
                    )
                    .frame(width: 88)
                    .controlSize(.small)

                    Text("\(Int((session.zoomScale * 100.0).rounded()))%")
                        .font(.system(size: 11.5, weight: .medium, design: .monospaced))
                        .lineLimit(1)
                        .frame(width: 46, alignment: .trailing)

                    LiquidGlassIconButton(systemImage: "plus.magnifyingglass", title: "Zoom In") {
                        session.setManualZoomScale(session.zoomScale * 1.25)
                    }

                    Divider().frame(height: 14)

                    LiquidGlassIconButton(
                        systemImage: "arrow.up.left.and.down.right.magnifyingglass",
                        title: "Fit Page to Window (⌘0)",
                        isActive: session.isAutoFitPageMode
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

    @ViewBuilder
    private func pageTabContextMenu(pageId: UInt32, pageTitle: String, isBackground: Bool) -> some View {
        let canDelete = !session.pages.filter { !$0.isBackgroundPage && $0.id != pageId }.isEmpty
        Button("Switch to \(pageTitle)") {
            session.selectPage(pageId)
        }
        Button(i18n.t("renamePage")) {
            session.selectPage(pageId)
            renamePageDraft = pageTitle
            renamingPageId = pageId
        }
        Button("Duplicate Page") {
            session.duplicatePage(pageId: pageId, undoManager: undoManager)
        }
        Button("Delete Page", role: .destructive) {
            session.deletePage(pageId: pageId, undoManager: undoManager)
        }
        .disabled(!canDelete)
        Divider()
        Button(i18n.t("pageSetup")) {
            session.selectPage(pageId)
            pageSetupSheetPageId = pageId
        }
        Menu("Quick Page Presets") {
            Button("ISO A4 Landscape (297 × 210 mm)") {
                session.selectPage(pageId)
                session.applyPagePreset(.a4, landscape: true, undoManager: undoManager)
            }
            Button("ISO A4 Portrait (210 × 297 mm)") {
                session.selectPage(pageId)
                session.applyPagePreset(.a4, landscape: false, undoManager: undoManager)
            }
            Button("ISO A3 Landscape (420 × 297 mm)") {
                session.selectPage(pageId)
                session.applyPagePreset(.a3, landscape: true, undoManager: undoManager)
            }
            Button("US Letter Landscape (11 × 8.5 in)") {
                session.selectPage(pageId)
                session.applyPagePreset(.letter, landscape: true, undoManager: undoManager)
            }
        }
        Menu("Page Background") {
            Button(isBackground ? "✓ Background Page" : "Mark as Background Page") {
                session.updatePageBackground(
                    pageId: pageId,
                    isBackground: !isBackground,
                    undoManager: undoManager
                )
            }
            Divider()
            Button("Paper: Pure White") {
                session.updatePageBackground(pageId: pageId, isBackground: isBackground, pageColorHex: "#FFFFFF", undoManager: undoManager)
            }
            Button("Paper: Warm Ivory") {
                session.updatePageBackground(pageId: pageId, isBackground: isBackground, pageColorHex: "#FAF8F5", undoManager: undoManager)
            }
            Button("Paper: Soft Slate") {
                session.updatePageBackground(pageId: pageId, isBackground: isBackground, pageColorHex: "#F1F5F9", undoManager: undoManager)
            }
            Button("Paper: Blueprint Tint") {
                session.updatePageBackground(pageId: pageId, isBackground: isBackground, pageColorHex: "#EFF6FF", undoManager: undoManager)
            }
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

    @MainActor
    private static func collectMetalCanvasViews(in view: NSView, into list: inout [VexioMetalNSView]) {
        if let metalView = view as? VexioMetalNSView {
            list.append(metalView)
        }
        for sub in view.subviews {
            collectMetalCanvasViews(in: sub, into: &list)
        }
    }

    @MainActor
    private func scheduleAutomatedWindowScreenshot(outputPath: String) {
        let env = ProcessInfo.processInfo.environment
        if let appearance = env["VEXIO_CAPTURE_APPEARANCE"]?.lowercased() {
            UserDefaults.standard.set(appearance, forKey: "vexio.appearanceMode")
            NSApp.appearance = (appearance == "dark")
                ? NSAppearance(named: .darkAqua)
                : NSAppearance(named: .aqua)
        }
        if let templateId = env["VEXIO_CAPTURE_TEMPLATE"], !templateId.isEmpty {
            session.loadStarterTemplate(templateId, undoManager: nil)
        }
        if let customTitle = env["VEXIO_CAPTURE_DOC_TITLE"], !customTitle.isEmpty {
            session.documentTitle = customTitle
        }
        if let borderRaw = env["VEXIO_CAPTURE_BORDER"],
           let borderStyle = VexioBorderTitleStyle(rawValue: borderRaw) {
            session.applyBordersAndTitles(
                style: borderStyle,
                title: session.documentTitle,
                subtitle: env["VEXIO_CAPTURE_SUBTITLE"] ?? "",
                author: "Vexio macOS Workstation",
                dateOrRev: "Rev 1.0",
                undoManager: nil
            )
        }
        if let cat = env["VEXIO_CAPTURE_STENCIL_CATEGORY"], !cat.isEmpty {
            selectedStencilCategory = cat
        }
        if env["VEXIO_CAPTURE_SHAPESHEET"] == "1" {
            session.showShapeSheetDrawer = true
        }
        if env["VEXIO_CAPTURE_VALIDATE"] == "bpmn" {
            session.runDiagramValidation(standard: .bpmn20)
        }
        session.reloadSelectedShapeSheetCells()
        session.syncSelectedShapeMetadataFromCore()

        let capturedSession = session
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 400_000_000)
            if let window = NSApp.windows.first(where: { $0.isVisible }) ?? NSApp.windows.first {
                window.setFrame(NSRect(x: 60, y: 60, width: 1600, height: 1000), display: true)
                window.makeKeyAndOrderFront(nil)
            }
            capturedSession.fitViewportToActivePage()

            try? await Task.sleep(nanoseconds: 1_850_000_000)
            guard let window = NSApp.windows.first(where: { $0.isVisible }) ?? NSApp.windows.first,
                  let contentView = window.contentView
            else {
                Darwin.exit(1)
            }
            let rootView = contentView
            rootView.wantsLayer = true
            rootView.layoutSubtreeIfNeeded()

            var metalViews: [VexioMetalNSView] = []
            Self.collectMetalCanvasViews(in: rootView, into: &metalViews)
            for mv in metalViews {
                mv.refreshCaptureLayerContents()
            }
            rootView.displayIfNeeded()

            let ptWidth = max(Int(round(rootView.bounds.width)), 1)
            let ptHeight = max(Int(round(rootView.bounds.height)), 1)
            let pxWidth = ptWidth * 2
            let pxHeight = ptHeight * 2

            guard let rep = NSBitmapImageRep(
                bitmapDataPlanes: nil,
                pixelsWide: pxWidth,
                pixelsHigh: pxHeight,
                bitsPerSample: 8,
                samplesPerPixel: 4,
                hasAlpha: true,
                isPlanar: false,
                colorSpaceName: .calibratedRGB,
                bytesPerRow: 0,
                bitsPerPixel: 0
            ) else {
                Darwin.exit(2)
            }
            rep.size = NSSize(width: ptWidth, height: ptHeight)
            NSGraphicsContext.saveGraphicsState()
            if let ctx = NSGraphicsContext(bitmapImageRep: rep) {
                NSGraphicsContext.current = ctx
                let isDark = (NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua)
                (isDark ? NSColor(calibratedRed: 0.08, green: 0.09, blue: 0.12, alpha: 1.0)
                        : NSColor(calibratedRed: 0.95, green: 0.96, blue: 0.98, alpha: 1.0)).setFill()
                NSRect(x: 0, y: 0, width: ptWidth, height: ptHeight).fill()
            }
            NSGraphicsContext.restoreGraphicsState()
            rootView.cacheDisplay(in: rootView.bounds, to: rep)

            if let pngData = rep.representation(using: .png, properties: [:]) {
                let targetURL = URL(fileURLWithPath: outputPath)
                try? FileManager.default.createDirectory(
                    at: targetURL.deletingLastPathComponent(),
                    withIntermediateDirectories: true
                )
                try? pngData.write(to: targetURL)
            }
            if let appDel = NSApp.delegate as? VexioAppDelegate {
                appDel.isConfirmedForceTerminating = true
            }
            Darwin.exit(0)
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
    static let vexioOpenStencilCatalogSheet = Notification.Name("app.vexio.openStencilCatalogSheet")
    static let vexioOpenPageSetupSheet = Notification.Name("app.vexio.openPageSetupSheet")
    static let vexioOpenBordersAndTitlesSheet = Notification.Name("app.vexio.openBordersAndTitlesSheet")
    static let vexioOpenMarkupStudioSheet = Notification.Name("app.vexio.openMarkupStudioSheet")
    static let vexioOpenSlideSnippetsSheet = Notification.Name("app.vexio.openSlideSnippetsSheet")
    static let vexioOpenVbaStudioSheet = Notification.Name("app.vexio.openVbaStudioSheet")
    static let vexioOpenCommentsSheet = Notification.Name("app.vexio.openCommentsSheet")
    static let vexioOpenHyperlinkSheet = Notification.Name("app.vexio.openHyperlinkSheet")
    static let vexioOpenDomainWizardsSheet = Notification.Name("app.vexio.openDomainWizardsSheet")
    static let vexioOpenCadToolsSheet = Notification.Name("app.vexio.openCadToolsSheet")
    static let vexioOpenSmartActionsSheet = Notification.Name("app.vexio.openSmartActionsSheet")
    static let vexioOpenInsertFieldSheet = Notification.Name("app.vexio.openInsertFieldSheet")
    static let vexioOpenFindReplaceSheet = Notification.Name("app.vexio.openFindReplaceSheet")
    static let vexioOpenSnapGlueSheet = Notification.Name("app.vexio.openSnapGlueSheet")
    static let vexioOpenNumberShapesSheet = Notification.Name("app.vexio.openNumberShapesSheet")
    static let vexioExportFormatRequested = Notification.Name("app.vexio.exportFormatRequested")
    static let vexioOpenExternalURL = Notification.Name("app.vexio.openExternalURL")
}

// MARK: - Left-Hand Vibrancy Stencil Navigator (All Visio Categories + Import Visio Stencil .vssx/.vss)

private struct VexioStencilNavigatorSidebar: View {
    @ObservedObject var session: VexioDocumentSession
    @ObservedObject private var i18n: VexioLocalization = .shared
    @Binding var searchQuery: String
    @Binding var selectedCategory: String
    let onRequestImportStencil: () -> Void
    let onRequestOpenStencilCatalog: () -> Void
    @Environment(\.undoManager) private var undoManager

    @State private var isStencilDropTargeted: Bool = false
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
                        .font(.system(size: 11, weight: .bold))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .fixedSize(horizontal: true, vertical: false)

                    Spacer(minLength: 4)

                    Button {
                        onRequestOpenStencilCatalog()
                    } label: {
                        Image(systemName: "icloud.and.arrow.down")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(Color.accentColor)
                            .frame(width: 24, height: 24)
                            .background(
                                RoundedRectangle(cornerRadius: 6, style: .continuous)
                                    .fill(Color.accentColor.opacity(0.14))
                            )
                    }
                    .buttonStyle(.plain)
                    .help("\(i18n.t("stencilCatalog")) — Download Official Microsoft 365, Azure & Awesome-Visio Packs")

                    Button {
                        session.addSelectionToMyShapes(undoManager: undoManager)
                        expandedCategories.insert("My Shapes")
                    } label: {
                        Image(systemName: "star.square.on.square")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(.primary)
                            .frame(width: 24, height: 24)
                            .background(
                                RoundedRectangle(cornerRadius: 6, style: .continuous)
                                    .fill(Color.primary.opacity(0.06))
                            )
                    }
                    .buttonStyle(.plain)
                    .disabled(session.selectedShapeId == 0)
                    .help("Add Selected Shape(s) to My Shapes Stencil")

                    Button {
                        exportMyShapesStencilSheet()
                    } label: {
                        Image(systemName: "square.and.arrow.up.on.square")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(.primary)
                            .frame(width: 24, height: 24)
                            .background(
                                RoundedRectangle(cornerRadius: 6, style: .continuous)
                                    .fill(Color.primary.opacity(0.06))
                            )
                    }
                    .buttonStyle(.plain)
                    .help("Export My Shapes Custom Stencil (.vssx)…")

                    Button {
                        onRequestImportStencil()
                    } label: {
                        Image(systemName: "plus.rectangle.on.folder")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(.primary)
                            .frame(width: 24, height: 24)
                            .background(
                                RoundedRectangle(cornerRadius: 6, style: .continuous)
                                    .fill(Color.primary.opacity(0.06))
                            )
                    }
                    .buttonStyle(.plain)
                    .help(i18n.t("importStencil"))

                    let allExpanded = expandedCategories.count >= session.stencilCategories.count
                    Button {
                        withAnimation(LiquidGlassPhysics.standardSpring) {
                            if allExpanded {
                                expandedCategories = ["Basic Shapes", "Basic Flowchart"]
                            } else {
                                expandedCategories = Set(session.stencilCategories)
                            }
                        }
                    } label: {
                        Image(systemName: allExpanded ? "rectangle.compress.vertical" : "rectangle.expand.vertical")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(.secondary)
                            .frame(width: 24, height: 24)
                            .background(
                                RoundedRectangle(cornerRadius: 6, style: .continuous)
                                    .fill(Color.primary.opacity(0.06))
                            )
                    }
                    .buttonStyle(.plain)
                    .help(allExpanded ? "Collapse All Stencil Packs" : "Expand All Stencil Packs")
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
                .fill(
                    session.highlightedStencilMasterId == master.id
                        ? Color.accentColor.opacity(0.14)
                        : Color.primary.opacity(0.045)
                )
        )
        .overlay(
            RoundedRectangle(cornerRadius: 9, style: .continuous)
                .strokeBorder(
                    session.highlightedStencilMasterId == master.id
                        ? Color.accentColor
                        : Color.primary.opacity(0.09),
                    lineWidth: session.highlightedStencilMasterId == master.id ? 1.5 : 0.8
                )
        )
        .contentShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
        .onTapGesture(count: 2) {
            session.instantiateStencilMasterAtViewportCenter(
                masterId: master.id,
                undoManager: undoManager
            )
        }
        .onTapGesture(count: 1) {
            session.highlightedStencilMasterId = master.id
        }
        .onDrag {
            session.activeDraggedStencilMasterId = master.id
            let provider = NSItemProvider(object: master.id as NSString)
            provider.suggestedName = master.name
            return provider
        }
        .contextMenu {
            Button("Add '\(master.name)' to Center of Page") {
                session.instantiateStencilMasterAtViewportCenter(
                    masterId: master.id,
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
            if master.category == "My Shapes" {
                Button("Delete '\(master.name)' from My Shapes", role: .destructive) {
                    session.deleteCustomStencilMaster(stencilTitle: "My Shapes", masterName: master.name)
                }
                Button("Export My Shapes (.vssx)…") {
                    exportMyShapesStencilSheet()
                }
                Divider()
            }
            Button("Copy Shape Name") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(master.name, forType: .string)
            }
        }
        .help(master.prompt.isEmpty ? "Click or drag '\(master.name)' onto canvas" : "\(master.name) — \(master.prompt)")
    }

    private func exportMyShapesStencilSheet() {
        let panel = NSSavePanel()
        panel.title = "Export My Shapes Stencil (.vssx)"
        panel.allowedContentTypes = [.visioStencil]
        panel.nameFieldStringValue = "My Shapes.vssx"
        panel.canCreateDirectories = true
        if let window = NSApp.keyWindow ?? NSApp.windows.first {
            panel.beginSheetModal(for: window) { response in
                guard response == .OK, let targetURL = panel.url else { return }
                Task { @MainActor in
                    do {
                        let bytes = try session.exportCustomStencilVssxData(stencilTitle: "My Shapes")
                        try bytes.write(to: targetURL)
                        session.statusMessage = "Exported My Shapes stencil to \(targetURL.lastPathComponent)"
                    } catch {
                        session.statusMessage = error.localizedDescription
                    }
                }
            }
        }
    }
}

// MARK: - Right-Hand 1:1 Microsoft Visio Object Inspector (Format Shape, Size & Position, Shape Data, Layers)

private struct VexioInspectorSidebar: View {
    @ObservedObject var session: VexioDocumentSession
    @ObservedObject private var i18n: VexioLocalization = .shared
    @Binding var selectedTab: VexioInspectorTab
    let onInspectCellInShapeSheet: (String) -> Void
    let onRequestDataVisualizerExport: () -> Void
    let onRequestOpenPageSetup: (UInt32) -> Void

    @Environment(\.undoManager) private var undoManager

    // Object identity drafts
    @State private var objectNameDraft: String = ""
    @State private var objectLabelDraft: String = ""
    @State private var pageNameDraft: String = ""

    // Collapsible Format Shape Disclosure states
    @State private var isFillExpanded: Bool = true
    @State private var isLineExpanded: Bool = true
    @State private var isTextExpanded: Bool = true
    @State private var isEffectsExpanded: Bool = false

    // Add Shape Data (`Prop.*`) drafts
    @State private var newPropName: String = ""
    @State private var newPropLabel: String = ""
    @State private var newPropType: UInt8 = 0
    @State private var newPropValue: String = ""

    // Add Layer draft
    @State private var newLayerNameDraft: String = ""

    private var activePageSummary: VexioPageSummary? {
        session.pages.first(where: { $0.id == session.activePageId }) ?? session.pages.first
    }

    var body: some View {
        VibrancySidebarContainer(material: .sidebar, blendingMode: .behindWindow) {
            VStack(alignment: .leading, spacing: 8) {
                Spacer().frame(height: 6)

                // Persistent Top MS Visio Object / Page Identity Card
                if let obj = session.inspectedObject, session.selectedShapeId != 0 {
                    selectedObjectHeaderCard(obj)
                        .padding(.horizontal, 10)
                } else {
                    pageInspectorHeaderCard
                        .padding(.horizontal, 10)
                }

                // 4-Pane MS Visio Task Pane Selector (Format | Size & Pos | Shape Data | Layers)
                HStack(spacing: 4) {
                    ForEach(VexioInspectorTab.allCases) { tab in
                        let isSelected = (selectedTab == tab)
                        Button {
                            withAnimation(LiquidGlassPhysics.standardSpring) {
                                selectedTab = tab
                            }
                        } label: {
                            VStack(spacing: 2.5) {
                                Image(systemName: tab.iconName)
                                    .font(.system(size: 12, weight: isSelected ? .semibold : .regular))
                                Text(tab.localizedTitle(i18n))
                                    .font(.system(size: 10, weight: isSelected ? .semibold : .medium))
                                    .lineLimit(1)
                                    .minimumScaleFactor(0.85)
                            }
                            .foregroundStyle(isSelected ? Color.accentColor : Color.secondary)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 5.5)
                            .padding(.horizontal, 3)
                            .background(
                                RoundedRectangle(cornerRadius: 8, style: .continuous)
                                    .fill(isSelected ? Color.accentColor.opacity(0.16) : Color.primary.opacity(0.03))
                            )
                        }
                        .buttonStyle(.plain)
                        .help(tab.tooltipDescription)
                    }
                }
                .padding(.horizontal, 10)

                Divider()

                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        if let obj = session.inspectedObject, session.selectedShapeId != 0 {
                            switch selectedTab {
                            case .format:
                                formatShapePane(obj)
                            case .sizeAndPos:
                                sizeAndPositionPane(obj)
                            case .shapeData:
                                shapeDataPane(obj)
                            case .layers:
                                layersPane(selectedObject: obj)
                            }
                        } else {
                            if selectedTab == .layers {
                                layersPane(selectedObject: nil)
                            } else if selectedTab == .shapeData {
                                VStack(alignment: .leading, spacing: 10) {
                                    dataGraphicsAndLegendCard
                                    externalDataRecordsetsCard(selectedObject: nil)
                                }
                            } else {
                                pagePropertiesInspectorPane
                            }
                        }
                    }
                    .padding(12)
                }
            }
        }
        .onAppear {
            syncHeaderDrafts()
        }
        .onChange(of: session.selectedShapeId) { _, _ in
            syncHeaderDrafts()
        }
        .onChange(of: session.inspectedObject?.name) { _, _ in
            if let obj = session.inspectedObject {
                objectNameDraft = obj.name
            }
        }
        .onChange(of: session.inspectedObject?.text) { _, _ in
            if let obj = session.inspectedObject {
                objectLabelDraft = obj.text
            }
        }
        .onChange(of: session.activePageId) { _, _ in
            syncHeaderDrafts()
        }
    }

    private func syncHeaderDrafts() {
        if let obj = session.inspectedObject {
            objectNameDraft = obj.name
            objectLabelDraft = obj.text
        }
        pageNameDraft = activePageSummary?.title ?? "Page-\(session.activePageId)"
    }

    // MARK: - Top Object Identity & Caption Header

    @ViewBuilder
    private func selectedObjectHeaderCard(_ obj: InspectedObjectState) -> some View {
        let iconName: String = {
            if obj.isGroup { return "square.3.layers.3d.down.right" }
            if obj.isConnector1D { return "point.topleft.down.curvedto.point.bottomright.up" }
            if obj.kind == "ForeignObject" { return "photo.artframe" }
            return "square.on.circle"
        }()

        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: iconName)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(Color.accentColor)
                    .frame(width: 22, height: 22)
                    .background(
                        RoundedRectangle(cornerRadius: 6, style: .continuous)
                            .fill(Color.accentColor.opacity(0.14))
                    )

                VStack(alignment: .leading, spacing: 1) {
                    HStack(spacing: 5) {
                        Text("Sheet.\(obj.id)")
                            .font(.system(size: 11, weight: .bold, design: .monospaced))
                        if session.selectedShapeIds.count > 1 {
                            Text("+\(session.selectedShapeIds.count - 1)")
                                .font(.system(size: 9.5, weight: .bold, design: .monospaced))
                                .padding(.horizontal, 5)
                                .padding(.vertical, 1)
                                .background(Capsule().fill(Color.accentColor.opacity(0.20)))
                        }
                    }
                    Text(obj.humanReadableTypeBadge)
                        .font(.system(size: 9.5))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }

                Spacer(minLength: 4)

                Button {
                    withAnimation(LiquidGlassPhysics.panelExpansionSpring) {
                        session.showShapeSheetDrawer.toggle()
                    }
                } label: {
                    Image(systemName: "function")
                        .font(.system(size: 11, weight: .semibold))
                        .padding(5)
                        .background(RoundedRectangle(cornerRadius: 6).fill(Color.primary.opacity(0.06)))
                }
                .buttonStyle(.plain)
                .help(i18n.t("openShapeSheet"))
            }

            HStack(spacing: 6) {
                Text(i18n.t("objectName"))
                    .font(.system(size: 10, weight: .medium))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .frame(minWidth: 50, alignment: .leading)
                    .fixedSize(horizontal: true, vertical: false)
                TextField("Sheet.\(obj.id)", text: $objectNameDraft)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: 11, weight: .medium))
                    .onSubmit {
                        session.renameSelectedShape(to: objectNameDraft, undoManager: undoManager)
                    }
                    .help("Visio Shape Name (press Return to apply)")
            }

            HStack(spacing: 6) {
                Text(i18n.t("textLabel"))
                    .font(.system(size: 10, weight: .medium))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .frame(minWidth: 50, alignment: .leading)
                    .fixedSize(horizontal: true, vertical: false)
                TextField(i18n.t("editLabelPlaceholder"), text: $objectLabelDraft)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: 11))
                    .onSubmit {
                        session.setSelectedShapeLabel(objectLabelDraft, undoManager: undoManager)
                    }
                    .help("Edit Shape Text / Caption (press Return to apply)")
            }
        }
        .padding(9)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(Color.primary.opacity(0.045))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.08), lineWidth: 0.8)
        )
    }

    private var pageInspectorHeaderCard: some View {
        let pageTitle = activePageSummary?.title ?? "Page-\(session.activePageId)"
        let sizeText = "\(i18n.unitPreference.formatFromInches(session.pageWidthInches)) × \(i18n.unitPreference.formatFromInches(session.pageHeightInches))"

        return HStack(spacing: 8) {
            Image(systemName: "doc.richtext")
                .font(.system(size: 13.5, weight: .semibold))
                .foregroundStyle(Color.accentColor)
                .frame(width: 26, height: 26)
                .background(RoundedRectangle(cornerRadius: 7).fill(Color.accentColor.opacity(0.14)))

            VStack(alignment: .leading, spacing: 1.5) {
                Text("\(i18n.t("pageInspector")) • \(pageTitle)")
                    .font(.system(size: 11.5, weight: .semibold))
                    .lineLimit(1)
                Text("\(sizeText) • \(session.activePageShapeCount) shapes")
                    .font(.system(size: 10))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
        }
        .padding(9)
        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color.primary.opacity(0.045)))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(Color.primary.opacity(0.08), lineWidth: 0.8))
    }

    // MARK: - Page Properties Pane (Shown when no object is selected on canvas)

    private var pagePropertiesInspectorPane: some View {
        VStack(alignment: .leading, spacing: 12) {
            pagePropertiesSizingCard
            pagePropertiesBackgroundCard
            layersPane(selectedObject: nil)
        }
    }

    private func convertUserDimensionToInches(_ raw: String, fallbackInches: Double) -> Double {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: ",", with: ".")
        guard let numeric = Double(trimmed), numeric > 0 else { return fallbackInches }
        switch i18n.unitPreference.resolvedEngineSlug {
        case "mm": return numeric / 25.4
        case "cm": return numeric / 2.54
        case "m": return numeric / 0.0254
        default: return numeric
        }
    }

    @ViewBuilder
    private var pagePropertiesSizingCard: some View {
        inspectorCard(title: i18n.t("pageInspector"), icon: "doc.text") {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 6) {
                    Text(i18n.t("objectName"))
                        .font(.system(size: 10.5, weight: .medium))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .fixedSize(horizontal: true, vertical: false)
                    TextField("Page-1", text: $pageNameDraft)
                        .textFieldStyle(.roundedBorder)
                        .font(.system(size: 11.5))
                        .onSubmit {
                            session.renamePage(pageId: session.activePageId, to: pageNameDraft, undoManager: undoManager)
                        }
                    Button {
                        onRequestOpenPageSetup(session.activePageId)
                    } label: {
                        Image(systemName: "slider.horizontal.3")
                            .font(.system(size: 11, weight: .semibold))
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .help(i18n.t("pageSetup"))
                }

                HStack(spacing: 5) {
                    ForEach([VexioPagePreset.a4, .a3, .a5, .letter], id: \.id) { preset in
                        Button(preset.shortCode) {
                            session.applyPagePreset(preset, landscape: session.isPageLandscape, undoManager: undoManager)
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .help(preset.displayLabel)
                    }

                    Spacer(minLength: 2)

                    Menu {
                        ForEach(VexioPagePreset.allCases) { preset in
                            Button(preset.displayLabel) {
                                session.applyPagePreset(preset, landscape: session.isPageLandscape, undoManager: undoManager)
                            }
                        }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                            .font(.system(size: 12))
                    }
                    .menuStyle(.borderlessButton)
                    .fixedSize()
                    .help("All ISO & ANSI Page Sizes")
                }

                HStack(spacing: 8) {
                    InspectorUnitField(
                        label: i18n.t("width"),
                        unitSuffix: i18n.unitPreference.resolvedEngineSlug,
                        displayValue: i18n.unitPreference.numericDisplayFromInches(session.pageWidthInches),
                        tooltip: "Page Width"
                    ) { committed in
                        let newW = convertUserDimensionToInches(committed, fallbackInches: session.pageWidthInches)
                        let summary = activePageSummary
                        session.applyCustomPageSetup(
                            pageId: session.activePageId,
                            name: nil,
                            widthInches: newW,
                            heightInches: session.pageHeightInches,
                            unitSlug: i18n.unitPreference.resolvedEngineSlug,
                            isBackground: summary?.isBackgroundPage ?? false,
                            backPageId: summary?.backPageId ?? 0,
                            pageColorHex: summary?.pageColorHex ?? "#FFFFFF",
                            undoManager: undoManager
                        )
                    }

                    InspectorUnitField(
                        label: i18n.t("height"),
                        unitSuffix: i18n.unitPreference.resolvedEngineSlug,
                        displayValue: i18n.unitPreference.numericDisplayFromInches(session.pageHeightInches),
                        tooltip: "Page Height"
                    ) { committed in
                        let newH = convertUserDimensionToInches(committed, fallbackInches: session.pageHeightInches)
                        let summary = activePageSummary
                        session.applyCustomPageSetup(
                            pageId: session.activePageId,
                            name: nil,
                            widthInches: session.pageWidthInches,
                            heightInches: newH,
                            unitSlug: i18n.unitPreference.resolvedEngineSlug,
                            isBackground: summary?.isBackgroundPage ?? false,
                            backPageId: summary?.backPageId ?? 0,
                            pageColorHex: summary?.pageColorHex ?? "#FFFFFF",
                            undoManager: undoManager
                        )
                    }
                }

                HStack(spacing: 6) {
                    Text(i18n.t("units"))
                        .font(.system(size: 10.5, weight: .medium))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .fixedSize(horizontal: true, vertical: false)
                    Picker("", selection: $i18n.unitPreference) {
                        Text("\(i18n.t("osStandard")) (\(VexioMeasurementUnit.osDefault.resolvedEngineSlug))").tag(VexioMeasurementUnit.osDefault)
                        Text("mm").tag(VexioMeasurementUnit.mm)
                        Text("cm").tag(VexioMeasurementUnit.cm)
                        Text("m").tag(VexioMeasurementUnit.m)
                        Text("in").tag(VexioMeasurementUnit.inch)
                    }
                    .labelsHidden()
                    .controlSize(.small)
                    .onChange(of: i18n.unitPreference) { _, newUnit in
                        session.applyMeasurementUnit(newUnit.resolvedEngineSlug, undoManager: undoManager)
                    }
                }

                HStack(spacing: 6) {
                    Button {
                        session.setPageOrientation(landscape: true, undoManager: undoManager)
                    } label: {
                        Label(i18n.t("landscape"), systemImage: "rectangle.portrait.rotate")
                            .font(.system(size: 11, weight: session.isPageLandscape ? .semibold : .regular))
                            .lineLimit(1)
                            .minimumScaleFactor(0.85)
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .tint(session.isPageLandscape ? .accentColor : nil)
                    .controlSize(.small)
                    .help("Set Landscape Page Orientation")

                    Button {
                        session.setPageOrientation(landscape: false, undoManager: undoManager)
                    } label: {
                        Label(i18n.t("portrait"), systemImage: "rectangle.portrait")
                            .font(.system(size: 11, weight: !session.isPageLandscape ? .semibold : .regular))
                            .lineLimit(1)
                            .minimumScaleFactor(0.85)
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .tint(!session.isPageLandscape ? .accentColor : nil)
                    .controlSize(.small)
                    .help("Set Portrait Page Orientation")
                }
            }
        }
    }

    @ViewBuilder
    private var pagePropertiesBackgroundCard: some View {
        let summary = activePageSummary
        let isBg = summary?.isBackgroundPage ?? false
        let currentColorHex = summary?.pageColorHex ?? "#FFFFFF"
        let bgPages = session.pages.filter { $0.isBackgroundPage && $0.id != session.activePageId }
        let swatches: [(String, String)] = [
            ("#FFFFFF", "Pure White"),
            ("#FAF8F5", "Warm Ivory"),
            ("#F1F5F9", "Soft Slate"),
            ("#EFF6FF", "Blueprint Tint"),
            ("#ECFDF5", "Engineering Mint"),
        ]

        inspectorCard(title: i18n.t("pageBackground"), icon: "paintbrush.pointed") {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 6) {
                    ForEach(swatches, id: \.0) { hex, label in
                        let selected = currentColorHex.caseInsensitiveCompare(hex) == .orderedSame
                        Button {
                            session.updatePageBackground(
                                pageId: session.activePageId,
                                isBackground: isBg,
                                backPageId: summary?.backPageId ?? 0,
                                pageColorHex: hex,
                                undoManager: undoManager
                            )
                        } label: {
                            Circle()
                                .fill(Color(vexioHex: hex))
                                .frame(width: 20, height: 20)
                                .overlay(
                                    Circle()
                                        .strokeBorder(selected ? Color.accentColor : Color.primary.opacity(0.22), lineWidth: selected ? 2.0 : 1.0)
                                )
                        }
                        .buttonStyle(.plain)
                        .help("Paper: \(label)")
                    }

                    Spacer(minLength: 4)

                    ColorPicker(
                        "",
                        selection: Binding(
                            get: { Color(vexioHex: currentColorHex) },
                            set: { picked in
                                session.updatePageBackground(
                                    pageId: session.activePageId,
                                    isBackground: isBg,
                                    backPageId: summary?.backPageId ?? 0,
                                    pageColorHex: picked.vexioHexRGBString,
                                    undoManager: undoManager
                                )
                            }
                        ),
                        supportsOpacity: false
                    )
                    .labelsHidden()
                    .help("Custom Page Paper Color")
                }

                Toggle(
                    "Mark as Background Page",
                    isOn: Binding(
                        get: { isBg },
                        set: { toggled in
                            session.updatePageBackground(
                                pageId: session.activePageId,
                                isBackground: toggled,
                                backPageId: toggled ? 0 : (summary?.backPageId ?? 0),
                                pageColorHex: currentColorHex,
                                undoManager: undoManager
                            )
                        }
                    )
                )
                .font(.system(size: 11.5))

                if !isBg && !bgPages.isEmpty {
                    HStack(spacing: 6) {
                        Text("Background")
                            .font(.system(size: 10.5, weight: .medium))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .fixedSize(horizontal: true, vertical: false)
                        Picker(
                            "",
                            selection: Binding(
                                get: { summary?.backPageId ?? 0 },
                                set: { chosenBgId in
                                    session.updatePageBackground(
                                        pageId: session.activePageId,
                                        isBackground: false,
                                        backPageId: chosenBgId,
                                        pageColorHex: currentColorHex,
                                        undoManager: undoManager
                                    )
                                }
                            )
                        ) {
                            Text("None").tag(UInt32(0))
                            ForEach(bgPages) { bgp in
                                Text(bgp.title).tag(bgp.id)
                            }
                        }
                        .labelsHidden()
                        .controlSize(.small)
                    }
                }

                Divider()

                HStack(spacing: 6) {
                    Button {
                        session.applyOrInsertHeading(.title, forceInsert: true, undoManager: undoManager)
                    } label: {
                        Label("Add Title", systemImage: "textformat.size.larger")
                            .font(.system(size: 11, weight: .medium))
                            .lineLimit(1)
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .help("Insert a Visio Diagram Title at the top of the page")

                    Button {
                        NotificationCenter.default.post(name: .vexioOpenBordersAndTitlesSheet, object: nil)
                    } label: {
                        Label(i18n.t("bordersAndTitles"), systemImage: "rectangle.topthird.inset.filled")
                            .font(.system(size: 11, weight: .medium))
                            .lineLimit(1)
                            .minimumScaleFactor(0.85)
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .help("Configure Visio Borders & Titles Header / Engineering Block")
                }

                Toggle("Smart Alignment Guides", isOn: $session.smartGuidesEnabled)
                    .font(.system(size: 11.5))
                    .help("Show dynamic alignment & equal-spacing guides while dragging shapes")

                Button {
                    session.fitViewportToActivePage()
                } label: {
                    Label("Fit Diagram to Window", systemImage: "arrow.up.left.and.down.right.magnifyingglass")
                        .lineLimit(1)
                        .minimumScaleFactor(0.85)
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
                .help("Zoom and center active page so the entire diagram fits on screen")
            }
        }
    }

    @ViewBuilder
    private var inspectorHeadingStyleStrip: some View {
        let quickPresets: [VexioTextHeadingPreset] = [.title, .subtitle, .h1, .h2, .h3, .sectionBanner, .body]
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 4) {
                ForEach(quickPresets) { preset in
                    Button(preset.shortChipLabel) {
                        session.applyOrInsertHeading(preset, forceInsert: false, undoManager: undoManager)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.mini)
                    .help("Apply \(preset.displayName)")
                }
            }
        }
    }

    // MARK: - Pane 1: MS Visio "Format Shape" (Fill, Line & Arrows, Text, Effects)

    @ViewBuilder
    private func formatShapePane(_ obj: InspectedObjectState) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            // 1A. FILL SECTION
            DisclosureGroup(isExpanded: $isFillExpanded) {
                VStack(alignment: .leading, spacing: 8) {
                    // Fill Pattern Mode (No Fill / Solid / Gradient)
                    HStack(spacing: 4) {
                        fillModeChip(title: i18n.t("noFill"), active: obj.fillPattern == 0, tip: "Transparent / No Fill (FillPattern = 0)") {
                            session.setCellForSelection(cellName: "FillPattern", formula: "0", actionLabel: "No Fill", undoManager: undoManager)
                        }
                        fillModeChip(title: i18n.t("solidFill"), active: obj.fillPattern == 1, tip: "Solid Color Fill (FillPattern = 1)") {
                            session.setCellForSelection(cellName: "FillPattern", formula: "1", actionLabel: "Solid Fill", undoManager: undoManager)
                        }
                        fillModeChip(title: i18n.t("gradientFill"), active: obj.fillPattern >= 25, tip: "Linear Gradient Fill") {
                            session.applyVisualPreset(.gradLinear, undoManager: undoManager)
                        }
                    }
                    .padding(.top, 4)

                    HStack {
                        ColorPicker(
                            i18n.t("fillColor"),
                            selection: Binding(
                                get: { Color(vexioHex: obj.fillColorHex) },
                                set: { session.applyFillColor($0, undoManager: undoManager) }
                            ),
                            supportsOpacity: false
                        )
                        .font(.system(size: 11.5, weight: .medium))
                        .help("Select foreground FillForegnd color")

                        Spacer()

                        if obj.fillPattern >= 25 {
                            ColorPicker(
                                "Stop 2",
                                selection: Binding(
                                    get: { Color(vexioHex: obj.fillBkgndHex) },
                                    set: { c in
                                        session.setCellForSelection(
                                            cellName: "FillBkgnd",
                                            formula: c.vexioRGBFormulaString,
                                            actionLabel: "Set Fill Background",
                                            undoManager: undoManager
                                        )
                                    }
                                ),
                                supportsOpacity: false
                            )
                            .font(.system(size: 11, weight: .medium))
                            .help("Secondary gradient color (FillBkgnd)")
                        }
                    }

                    // Visio Theme Quick Swatches
                    HStack(spacing: 6) {
                        ForEach(
                            [
                                ("Azure", "RGB(219,234,254)", "#DBEAFE"),
                                ("Mint", "RGB(220,252,231)", "#DCFCE7"),
                                ("Amber", "RGB(254,243,199)", "#FEF3C7"),
                                ("Rose", "RGB(255,228,230)", "#FFE4E6"),
                                ("Iris", "RGB(237,233,254)", "#EDE9FE"),
                                ("Slate", "RGB(241,245,249)", "#F1F5F9"),
                                ("White", "RGB(255,255,255)", "#FFFFFF"),
                            ],
                            id: \.0
                        ) { name, formula, hex in
                            Button {
                                session.setCellForSelection(cellName: "FillPattern", formula: "1", undoManager: undoManager)
                                session.setCellForSelection(cellName: "FillForegnd", formula: formula, actionLabel: "Fill \(name)", undoManager: undoManager)
                            } label: {
                                Circle()
                                    .fill(Color(vexioHex: hex))
                                    .overlay(Circle().strokeBorder(Color.primary.opacity(0.25), lineWidth: 0.9))
                                    .frame(width: 20, height: 20)
                            }
                            .buttonStyle(.plain)
                            .help("\(name) Fill (\(formula))")
                        }
                    }

                    // Fill Transparency Slider (0-100%)
                    VStack(alignment: .leading, spacing: 2) {
                        HStack {
                            Text(i18n.t("transparency"))
                                .font(.system(size: 11))
                                .foregroundStyle(.secondary)
                            Spacer()
                            Text("\(Int(obj.fillTransPct.rounded()))%")
                                .font(.system(size: 10.5, design: .monospaced))
                                .foregroundStyle(.secondary)
                        }
                        Slider(
                            value: Binding(
                                get: { obj.fillTransPct },
                                set: { pct in
                                    session.setCellForSelection(
                                        cellName: "FillForegndTrans",
                                        formula: "\(Int(pct.rounded()))%",
                                        actionLabel: "Fill Transparency",
                                        undoManager: undoManager
                                    )
                                }
                            ),
                            in: 0...100,
                            step: 5
                        )
                        .controlSize(.small)
                        .help("Adjust Fill Transparency (FillForegndTrans)")
                    }

                    if !obj.isConnector1D {
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Text(i18n.t("cornerRounding"))
                                    .font(.system(size: 11))
                                    .foregroundStyle(.secondary)
                                Spacer()
                                Text(i18n.unitPreference.formatFromInches(obj.roundingIn))
                                    .font(.system(size: 10.5, design: .monospaced))
                                    .foregroundStyle(.secondary)
                            }
                            HStack(spacing: 5) {
                                ForEach(
                                    i18n.unitPreference.isMetric
                                        ? [("0 mm", "0 mm"), ("2 mm", "2 mm"), ("4 mm", "4 mm"), ("8 mm", "8 mm")]
                                        : [("0 in", "0 in"), ("0.08 in", "0.08 in"), ("0.16 in", "0.16 in"), ("0.30 in", "0.30 in")],
                                    id: \.0
                                ) { label, formula in
                                    Button(label) {
                                        session.setCellForSelection(cellName: "Rounding", formula: formula, actionLabel: "Corner Rounding", undoManager: undoManager)
                                    }
                                    .buttonStyle(.bordered)
                                    .controlSize(.mini)
                                    .help("Set Corner Rounding to \(label)")
                                }
                            }
                        }
                    }
                }
            } label: {
                sectionHeaderLabel(i18n.t("fillSection"), icon: "drop.fill")
            }
            .padding(9)
            .background(RoundedRectangle(cornerRadius: 9).fill(Color.primary.opacity(0.035)))

            // 1B. LINE & ARROWS SECTION
            DisclosureGroup(isExpanded: $isLineExpanded) {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 4) {
                        fillModeChip(title: i18n.t("noLine"), active: obj.linePattern == 0, tip: "No Border / Line (LinePattern = 0)") {
                            session.setCellForSelection(cellName: "LinePattern", formula: "0", actionLabel: "No Line", undoManager: undoManager)
                        }
                        fillModeChip(title: i18n.t("solidLine"), active: obj.linePattern == 1, tip: "Solid Line (LinePattern = 1)") {
                            session.setCellForSelection(cellName: "LinePattern", formula: "1", actionLabel: "Solid Line", undoManager: undoManager)
                        }
                        fillModeChip(title: i18n.t("dashedLine"), active: obj.linePattern == 2, tip: "Dashed Line (LinePattern = 2)") {
                            session.setCellForSelection(cellName: "LinePattern", formula: "2", actionLabel: "Dashed Line", undoManager: undoManager)
                        }
                        fillModeChip(title: i18n.t("dottedLine"), active: obj.linePattern == 3, tip: "Dotted Line (LinePattern = 3)") {
                            session.setCellForSelection(cellName: "LinePattern", formula: "3", actionLabel: "Dotted Line", undoManager: undoManager)
                        }
                    }
                    .padding(.top, 4)

                    HStack {
                        ColorPicker(
                            i18n.t("lineColor"),
                            selection: Binding(
                                get: { Color(vexioHex: obj.lineColorHex) },
                                set: { session.applyLineColor($0, undoManager: undoManager) }
                            ),
                            supportsOpacity: false
                        )
                        .font(.system(size: 11.5, weight: .medium))
                        .help("Select LineColor stroke color")

                        Spacer()

                        Text(String(format: "%.2f pt", obj.lineWeightPt))
                            .font(.system(size: 10.5, design: .monospaced))
                            .foregroundStyle(.secondary)
                    }

                    HStack(spacing: 4) {
                        ForEach(["0.5 pt", "1 pt", "1.5 pt", "2.25 pt", "3.5 pt"], id: \.self) { wt in
                            Button(wt) {
                                session.setCellForSelection(cellName: "LineWeight", formula: wt, actionLabel: "Line Weight", undoManager: undoManager)
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.mini)
                            .help("Set LineWeight to \(wt)")
                        }
                    }

                    // Begin & End Arrowheads
                    HStack(spacing: 8) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(i18n.t("beginArrow"))
                                .font(.system(size: 10))
                                .foregroundStyle(.secondary)
                            Picker("", selection: Binding(
                                get: { obj.beginArrow },
                                set: { code in
                                    session.setCellForSelection(cellName: "BeginArrow", formula: "\(code)", actionLabel: "Begin Arrow", undoManager: undoManager)
                                }
                            )) {
                                Text("None").tag(0)
                                Text("Open ›").tag(1)
                                Text("Filled ▶").tag(4)
                                Text("Stealth ➤").tag(5)
                                Text("Circle ●").tag(10)
                                Text("Diamond ◆").tag(11)
                            }
                            .labelsHidden()
                            .controlSize(.small)
                            .help("Select BeginArrow style")
                        }

                        VStack(alignment: .leading, spacing: 2) {
                            Text(i18n.t("endArrow"))
                                .font(.system(size: 10))
                                .foregroundStyle(.secondary)
                            Picker("", selection: Binding(
                                get: { obj.endArrow },
                                set: { code in
                                    session.setCellForSelection(cellName: "EndArrow", formula: "\(code)", actionLabel: "End Arrow", undoManager: undoManager)
                                }
                            )) {
                                Text("None").tag(0)
                                Text("Open ›").tag(1)
                                Text("Filled ▶").tag(4)
                                Text("Stealth ➤").tag(5)
                                Text("Circle ●").tag(10)
                                Text("Diamond ◆").tag(11)
                            }
                            .labelsHidden()
                            .controlSize(.small)
                            .help("Select EndArrow style")
                        }
                    }

                    if obj.isConnector1D {
                        HStack(spacing: 4) {
                            fillModeChip(title: "Orthogonal", active: obj.routeStyle == 1 || obj.routeStyle == 16, tip: "Right-angle orthogonal connector routing") {
                                session.setCellForSelection(cellName: "ShapeRouteStyle", formula: "1", actionLabel: "Orthogonal Connector", undoManager: undoManager)
                            }
                            fillModeChip(title: "Straight", active: obj.routeStyle == 2, tip: "Direct straight-line connector") {
                                session.setCellForSelection(cellName: "ShapeRouteStyle", formula: "2", actionLabel: "Straight Connector", undoManager: undoManager)
                            }
                            fillModeChip(title: "Curved", active: obj.routeStyle == 3, tip: "Smooth Bézier curved connector") {
                                session.setCellForSelection(cellName: "ShapeRouteStyle", formula: "3", actionLabel: "Curved Connector", undoManager: undoManager)
                            }
                        }
                    }
                }
            } label: {
                sectionHeaderLabel(i18n.t("lineArrowsSection"), icon: "line.diagonal.arrow")
            }
            .padding(9)
            .background(RoundedRectangle(cornerRadius: 9).fill(Color.primary.opacity(0.035)))

            // 1C. TEXT & FONT SECTION
            DisclosureGroup(isExpanded: $isTextExpanded) {
                VStack(alignment: .leading, spacing: 8) {
                    inspectorHeadingStyleStrip
                        .padding(.top, 4)

                    HStack(spacing: 6) {
                        Picker("", selection: Binding(
                            get: { session.selectedFontFamily },
                            set: { session.applyTextStyle(fontFamily: $0, undoManager: undoManager) }
                        )) {
                            ForEach(VexioDocumentSession.installedSystemFontFamilies, id: \.self) { fam in
                                Text(fam).tag(fam)
                            }
                        }
                        .labelsHidden()
                        .controlSize(.small)
                        .help("Font Family")

                        Stepper(
                            value: Binding(
                                get: { session.selectedFontSizePt },
                                set: { session.applyTextStyle(fontSizePt: $0, undoManager: undoManager) }
                            ),
                            in: 6...96,
                            step: 1
                        ) {
                            Text("\(Int(session.selectedFontSizePt.rounded())) pt")
                                .font(.system(size: 11, design: .monospaced))
                                .frame(width: 38, alignment: .trailing)
                        }
                        .controlSize(.small)
                        .help("Font Size (pt)")
                    }

                    HStack(spacing: 5) {
                        inspectorSmallToggleButton("B", bold: true, active: (session.selectedCharStyle & 1) != 0, tip: "Bold (⌘B)") {
                            session.applyTextStyle(toggleStyleMask: 1, undoManager: undoManager)
                        }
                        inspectorSmallToggleButton("I", italic: true, active: (session.selectedCharStyle & 2) != 0, tip: "Italic (⌘I)") {
                            session.applyTextStyle(toggleStyleMask: 2, undoManager: undoManager)
                        }
                        inspectorSmallToggleButton("U", underline: true, active: (session.selectedCharStyle & 4) != 0, tip: "Underline (⌘U)") {
                            session.applyTextStyle(toggleStyleMask: 4, undoManager: undoManager)
                        }
                        Divider().frame(height: 16)
                        LiquidGlassIconButton(systemImage: "text.alignleft", title: "Align Left", isActive: session.selectedHorizAlign == 0) {
                            session.applyTextStyle(horizAlign: 0, undoManager: undoManager)
                        }
                        LiquidGlassIconButton(systemImage: "text.aligncenter", title: "Align Center", isActive: session.selectedHorizAlign == 1) {
                            session.applyTextStyle(horizAlign: 1, undoManager: undoManager)
                        }
                        LiquidGlassIconButton(systemImage: "text.alignright", title: "Align Right", isActive: session.selectedHorizAlign == 2) {
                            session.applyTextStyle(horizAlign: 2, undoManager: undoManager)
                        }
                        Spacer()
                        ColorPicker(
                            "",
                            selection: Binding(
                                get: { Color(vexioHex: session.selectedTextColorHex) },
                                set: { c in
                                    let hex = c.vexioHexRGBString
                                    session.selectedTextColorHex = hex
                                    session.applyTextStyle(textColorHex: hex, undoManager: undoManager)
                                }
                            ),
                            supportsOpacity: false
                        )
                        .labelsHidden()
                        .help("Text Color")
                    }

                    Toggle(
                        i18n.t("hideText"),
                        isOn: Binding(
                            get: { obj.hideText },
                            set: { hidden in
                                session.setCellForSelection(
                                    cellName: "HideText",
                                    formula: hidden ? "TRUE" : "FALSE",
                                    actionLabel: "Toggle HideText",
                                    undoManager: undoManager
                                )
                            }
                        )
                    )
                    .font(.system(size: 11))
                    .help("Hide text caption while keeping shape geometry visible (HideText)")
                }
            } label: {
                sectionHeaderLabel(i18n.t("font"), icon: "textformat")
            }
            .padding(9)
            .background(RoundedRectangle(cornerRadius: 9).fill(Color.primary.opacity(0.035)))

            // 1D. EFFECTS SECTION
            DisclosureGroup(isExpanded: $isEffectsExpanded) {
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
                                RoundedRectangle(cornerRadius: 7, style: .continuous)
                                    .fill(isSelectedPreset ? Color.accentColor.opacity(0.16) : Color.primary.opacity(0.045))
                            )
                        }
                        .buttonStyle(.plain)
                        .help("Apply \(preset.displayName) effect")
                    }
                }
                .padding(.top, 4)
            } label: {
                sectionHeaderLabel(i18n.t("effectsSection"), icon: "sparkles")
            }
            .padding(9)
            .background(RoundedRectangle(cornerRadius: 9).fill(Color.primary.opacity(0.035)))
        }
    }

    // MARK: - Pane 2: MS Visio "Size & Position" + Arrange + Protection

    @ViewBuilder
    private func sizeAndPositionPane(_ obj: InspectedObjectState) -> some View {
        let unit = i18n.unitPreference
        let unitSuffix = unit.resolvedEngineSlug

        VStack(alignment: .leading, spacing: 10) {
            inspectorCard(title: "\(i18n.t("sizeAndPosition")) (\(unitSuffix))", icon: "ruler") {
                VStack(alignment: .leading, spacing: 7) {
                    HStack(spacing: 8) {
                        InspectorUnitField(
                            label: "X",
                            unitSuffix: unitSuffix,
                            displayValue: unit.numericDisplayFromInches(obj.pinXIn),
                            tooltip: "PinX horizontal center position (\(unitSuffix))"
                        ) { raw in
                            session.setCellForSelection(
                                cellName: "PinX",
                                formula: unit.formulaFromUserInput(raw),
                                actionLabel: "Set X Position",
                                undoManager: undoManager
                            )
                        }

                        InspectorUnitField(
                            label: "Y",
                            unitSuffix: unitSuffix,
                            displayValue: unit.numericDisplayFromInches(obj.pinYIn),
                            tooltip: "PinY vertical center position (\(unitSuffix))"
                        ) { raw in
                            session.setCellForSelection(
                                cellName: "PinY",
                                formula: unit.formulaFromUserInput(raw),
                                actionLabel: "Set Y Position",
                                undoManager: undoManager
                            )
                        }
                    }

                    HStack(spacing: 8) {
                        InspectorUnitField(
                            label: i18n.t("width"),
                            unitSuffix: unitSuffix,
                            displayValue: unit.numericDisplayFromInches(obj.widthIn),
                            tooltip: "Shape Width (\(unitSuffix))"
                        ) { raw in
                            session.setCellForSelection(
                                cellName: "Width",
                                formula: unit.formulaFromUserInput(raw),
                                actionLabel: "Set Width",
                                undoManager: undoManager
                            )
                        }

                        InspectorUnitField(
                            label: i18n.t("height"),
                            unitSuffix: unitSuffix,
                            displayValue: unit.numericDisplayFromInches(obj.heightIn),
                            tooltip: "Shape Height (\(unitSuffix))"
                        ) { raw in
                            session.setCellForSelection(
                                cellName: "Height",
                                formula: unit.formulaFromUserInput(raw),
                                actionLabel: "Set Height",
                                undoManager: undoManager
                            )
                        }
                    }

                    HStack(spacing: 8) {
                        InspectorUnitField(
                            label: i18n.t("angle"),
                            unitSuffix: "deg",
                            displayValue: String(format: "%.1f", obj.angleDeg),
                            tooltip: "Rotation Angle in degrees"
                        ) { raw in
                            let clean = raw.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: ",", with: ".")
                            let formula = clean.hasPrefix("=") || clean.contains("deg") || clean.contains("rad")
                                ? clean
                                : "\(clean.isEmpty ? "0" : clean) deg"
                            session.setCellForSelection(
                                cellName: "Angle",
                                formula: formula,
                                actionLabel: "Set Angle",
                                undoManager: undoManager
                            )
                        }

                        VStack(alignment: .leading, spacing: 2) {
                            Text(i18n.t("pinAnchor"))
                                .font(.system(size: 10, weight: .medium))
                                .foregroundStyle(.secondary)
                            Menu {
                                Button("Center-Center (Width*0.5, Height*0.5)") {
                                    session.setCellForSelection(cellName: "LocPinX", formula: "=Width*0.5", undoManager: undoManager)
                                    session.setCellForSelection(cellName: "LocPinY", formula: "=Height*0.5", undoManager: undoManager)
                                }
                                Button("Bottom-Left (0, 0)") {
                                    session.setCellForSelection(cellName: "LocPinX", formula: "0 in", undoManager: undoManager)
                                    session.setCellForSelection(cellName: "LocPinY", formula: "0 in", undoManager: undoManager)
                                }
                                Button("Top-Left (0, Height)") {
                                    session.setCellForSelection(cellName: "LocPinX", formula: "0 in", undoManager: undoManager)
                                    session.setCellForSelection(cellName: "LocPinY", formula: "=Height*1", undoManager: undoManager)
                                }
                            } label: {
                                Label("Center", systemImage: "scope")
                                    .font(.system(size: 10.5))
                                    .frame(maxWidth: .infinity)
                            }
                            .controlSize(.small)
                            .help("Visio Pin Pos anchor (LocPinX / LocPinY)")
                        }
                        .frame(maxWidth: .infinity)
                    }

                    if obj.isConnector1D,
                       let bx = obj.beginXIn, let by = obj.beginYIn,
                       let ex = obj.endXIn, let ey = obj.endYIn {
                        Divider()
                        HStack(spacing: 8) {
                            InspectorUnitField(
                                label: "Begin X",
                                unitSuffix: unitSuffix,
                                displayValue: unit.numericDisplayFromInches(bx),
                                tooltip: "1D Connector BeginX"
                            ) { raw in
                                session.setCellForSelection(cellName: "BeginX", formula: unit.formulaFromUserInput(raw), undoManager: undoManager)
                            }
                            InspectorUnitField(
                                label: "Begin Y",
                                unitSuffix: unitSuffix,
                                displayValue: unit.numericDisplayFromInches(by),
                                tooltip: "1D Connector BeginY"
                            ) { raw in
                                session.setCellForSelection(cellName: "BeginY", formula: unit.formulaFromUserInput(raw), undoManager: undoManager)
                            }
                        }
                        HStack(spacing: 8) {
                            InspectorUnitField(
                                label: "End X",
                                unitSuffix: unitSuffix,
                                displayValue: unit.numericDisplayFromInches(ex),
                                tooltip: "1D Connector EndX"
                            ) { raw in
                                session.setCellForSelection(cellName: "EndX", formula: unit.formulaFromUserInput(raw), undoManager: undoManager)
                            }
                            InspectorUnitField(
                                label: "End Y",
                                unitSuffix: unitSuffix,
                                displayValue: unit.numericDisplayFromInches(ey),
                                tooltip: "1D Connector EndY"
                            ) { raw in
                                session.setCellForSelection(cellName: "EndY", formula: unit.formulaFromUserInput(raw), undoManager: undoManager)
                            }
                        }
                    }

                    Divider()

                    // Quick Transform & Z-Order Row
                    HStack(spacing: 4) {
                        LiquidGlassIconButton(systemImage: "rotate.left", title: i18n.t("rotateLeft")) {
                            session.rotateOrFlipSelectedShapes(deltaDegrees: 90.0, undoManager: undoManager)
                        }
                        LiquidGlassIconButton(systemImage: "rotate.right", title: i18n.t("rotateRight")) {
                            session.rotateOrFlipSelectedShapes(deltaDegrees: -90.0, undoManager: undoManager)
                        }
                        LiquidGlassIconButton(systemImage: "arrow.left.and.right.righttriangle.left.righttriangle.right", title: i18n.t("flipH"), isActive: obj.flipX) {
                            session.rotateOrFlipSelectedShapes(toggleFlipX: true, undoManager: undoManager)
                        }
                        LiquidGlassIconButton(systemImage: "arrow.up.and.down.righttriangle.up.righttriangle.down", title: i18n.t("flipV"), isActive: obj.flipY) {
                            session.rotateOrFlipSelectedShapes(toggleFlipY: true, undoManager: undoManager)
                        }
                        Divider().frame(height: 16)
                        LiquidGlassIconButton(systemImage: "square.2.layers.3d.top.filled", title: "Bring to Front") {
                            session.reorderSelectedShapes(mode: "front", undoManager: undoManager)
                        }
                        LiquidGlassIconButton(systemImage: "square.2.layers.3d.bottom.filled", title: "Send to Back") {
                            session.reorderSelectedShapes(mode: "back", undoManager: undoManager)
                        }
                        Spacer()
                        if session.canUngroupSelectedShapes {
                            Button("Ungroup") {
                                session.ungroupSelectedShapes(undoManager: undoManager)
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.mini)
                            .help("Ungroup Selected Group (⇧⌘G)")
                        } else if session.canGroupSelectedShapes {
                            Button("Group") {
                                session.groupSelectedShapes(undoManager: undoManager)
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.mini)
                            .help("Group Selected Objects (⌘G)")
                        }
                    }
                }
            }

            // Protection Section (MS Visio Developer -> Protection)
            inspectorCard(title: i18n.t("protection"), icon: "lock.shield") {
                VStack(alignment: .leading, spacing: 5) {
                    Toggle(i18n.t("lockAspect"), isOn: Binding(
                        get: { obj.lockAspect },
                        set: { v in session.setCellForSelection(cellName: "LockAspect", formula: v ? "TRUE" : "FALSE", forceSetF: true, undoManager: undoManager) }
                    ))
                    .help("Preserve Width:Height ratio when resizing (LockAspect)")

                    Toggle(i18n.t("lockSize"), isOn: Binding(
                        get: { obj.lockWidth && obj.lockHeight },
                        set: { v in
                            let f = v ? "TRUE" : "FALSE"
                            session.setCellForSelection(cellName: "LockWidth", formula: f, forceSetF: true, undoManager: undoManager)
                            session.setCellForSelection(cellName: "LockHeight", formula: f, forceSetF: true, undoManager: undoManager)
                        }
                    ))
                    .help("Prevent interactive Width/Height resize (LockWidth & LockHeight)")

                    Toggle(i18n.t("lockPosition"), isOn: Binding(
                        get: { obj.lockMoveX && obj.lockMoveY },
                        set: { v in
                            let f = v ? "TRUE" : "FALSE"
                            session.setCellForSelection(cellName: "LockMoveX", formula: f, forceSetF: true, undoManager: undoManager)
                            session.setCellForSelection(cellName: "LockMoveY", formula: f, forceSetF: true, undoManager: undoManager)
                        }
                    ))
                    .help("Lock PinX & PinY position on page (LockMoveX & LockMoveY)")

                    Toggle(i18n.t("lockRotation"), isOn: Binding(
                        get: { obj.lockRotate },
                        set: { v in session.setCellForSelection(cellName: "LockRotate", formula: v ? "TRUE" : "FALSE", forceSetF: true, undoManager: undoManager) }
                    ))
                    .help("Prevent interactive rotation (LockRotate)")

                    Toggle(i18n.t("lockDelete"), isOn: Binding(
                        get: { obj.lockDelete },
                        set: { v in session.setCellForSelection(cellName: "LockDelete", formula: v ? "TRUE" : "FALSE", forceSetF: true, undoManager: undoManager) }
                    ))
                    .help("Protect shape from accidental deletion (LockDelete)")
                }
                .toggleStyle(.checkbox)
                .font(.system(size: 11.5))
            }

            Button {
                onInspectCellInShapeSheet("Width")
            } label: {
                Label(i18n.t("openShapeSheet"), systemImage: "function")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            .help("Inspect all ShapeSheet formulas and GUARD expressions (⇧⌘F5)")
        }
    }

    // MARK: - Pane 3: MS Visio "Shape Data" (Prop.* Custom Properties & Data Graphics)

    @ViewBuilder
    private func shapeDataPane(_ obj: InspectedObjectState) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            inspectorCard(title: "\(i18n.t("inspectorData")) (Prop.*)", icon: "list.bullet.rectangle") {
                VStack(alignment: .leading, spacing: 8) {
                    if session.shapeDataProperties.isEmpty {
                        Text(i18n.t("noShapeData"))
                            .font(.system(size: 11))
                            .foregroundStyle(.secondary)
                            .padding(.vertical, 4)
                    } else {
                        ForEach(session.shapeDataProperties) { prop in
                            InspectorShapeDataRowEditor(
                                item: prop,
                                onCommitValue: { newVal in
                                    session.upsertSelectedShapeData(
                                        rowName: prop.rowName,
                                        label: prop.label,
                                        dataType: prop.dataTypeCode,
                                        value: newVal,
                                        undoManager: undoManager
                                    )
                                },
                                onDelete: {
                                    session.deleteSelectedShapeData(rowName: prop.rowName, undoManager: undoManager)
                                },
                                onInspectFormula: {
                                    onInspectCellInShapeSheet(prop.rowKey)
                                }
                            )
                        }
                    }

                    Divider()

                    // Define New Shape Data Field (MS Visio "Define Shape Data...")
                    Text(i18n.t("addDataField"))
                        .font(.system(size: 10.5, weight: .semibold))
                        .foregroundStyle(.secondary)

                    HStack(spacing: 6) {
                        TextField(i18n.t("fieldName"), text: $newPropName)
                            .textFieldStyle(.roundedBorder)
                            .font(.system(size: 11))
                            .help("Property key (e.g. Owner, AssetTag, Cost, IPAddress)")

                        Picker("", selection: $newPropType) {
                            Text("String").tag(UInt8(0))
                            Text("Number").tag(UInt8(2))
                            Text("Boolean").tag(UInt8(3))
                            Text("Date").tag(UInt8(5))
                            Text("Currency").tag(UInt8(7))
                        }
                        .labelsHidden()
                        .frame(width: 86)
                        .controlSize(.small)
                        .help("Visio Shape Data Type")
                    }

                    HStack(spacing: 6) {
                        TextField("Label (optional)", text: $newPropLabel)
                            .textFieldStyle(.roundedBorder)
                            .font(.system(size: 11))

                        TextField(i18n.t("fieldValue"), text: $newPropValue)
                            .textFieldStyle(.roundedBorder)
                            .font(.system(size: 11))
                            .onSubmit {
                                commitNewShapeDataField()
                            }
                    }

                    Button {
                        commitNewShapeDataField()
                    } label: {
                        Label(i18n.t("addDataField"), systemImage: "plus.circle.fill")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .disabled(newPropName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .help("Add custom Prop.* field to Sheet.\(obj.id)")
                }
            }

            dataGraphicsAndLegendCard
            externalDataRecordsetsCard(selectedObject: obj)
        }
    }

    @ViewBuilder
    private var dataGraphicsAndLegendCard: some View {
        inspectorCard(title: "Data Graphics & Legend", icon: "chart.bar.xaxis") {
            VStack(alignment: .leading, spacing: 7) {
                Toggle(
                    "Show Data Graphics Overlay",
                    isOn: Binding(
                        get: { session.dataGraphicConfig.isEnabled },
                        set: { val in
                            session.dataGraphicConfig.isEnabled = val
                            session.commitDataGraphicConfig(undoManager: undoManager)
                        }
                    )
                )
                .font(.system(size: 11.5, weight: .medium))

                dataGraphicFieldRow(
                    title: "Icon Set",
                    placeholder: "Status",
                    text: Binding(
                        get: { session.dataGraphicConfig.iconSetField },
                        set: { session.dataGraphicConfig.iconSetField = $0 }
                    )
                )

                dataGraphicFieldRow(
                    title: "Data Bar",
                    placeholder: "CompletionPct",
                    text: Binding(
                        get: { session.dataGraphicConfig.dataBarField },
                        set: { session.dataGraphicConfig.dataBarField = $0 }
                    )
                )

                dataGraphicFieldRow(
                    title: "Callout",
                    placeholder: "Owner",
                    text: Binding(
                        get: { session.dataGraphicConfig.textCalloutField },
                        set: { session.dataGraphicConfig.textCalloutField = $0 }
                    )
                )

                Toggle(
                    "Color by Value",
                    isOn: Binding(
                        get: { session.dataGraphicConfig.colorByValueEnabled },
                        set: { val in
                            session.dataGraphicConfig.colorByValueEnabled = val
                            session.commitDataGraphicConfig(undoManager: undoManager)
                        }
                    )
                )
                .font(.system(size: 11))

                if session.dataGraphicConfig.colorByValueEnabled {
                    dataGraphicFieldRow(
                        title: "Color Field",
                        placeholder: "Status",
                        text: Binding(
                            get: { session.dataGraphicConfig.colorByValueField },
                            set: {
                                session.dataGraphicConfig.colorByValueField = $0
                                session.dataGraphicConfig.legendField = $0
                            }
                        )
                    )
                }

                Divider()

                HStack(spacing: 6) {
                    Toggle(
                        "Live Legend",
                        isOn: Binding(
                            get: { session.dataGraphicConfig.showLegendOnPage },
                            set: { val in
                                session.dataGraphicConfig.showLegendOnPage = val
                                session.commitDataGraphicConfig(undoManager: undoManager)
                            }
                        )
                    )
                    .font(.system(size: 11))

                    Spacer()

                    Button("Apply") {
                        session.dataGraphicConfig.isEnabled = true
                        session.commitDataGraphicConfig(undoManager: undoManager)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .help("Apply Icon Set, Data Bar, Callout & Color-by-Value rules to active page")
                }
            }
        }
    }

    @ViewBuilder
    private func dataGraphicFieldRow(
        title: String,
        placeholder: String,
        text: Binding<String>
    ) -> some View {
        HStack(spacing: 4) {
            Text(title)
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
                .frame(width: 68, alignment: .leading)
            TextField(placeholder, text: text)
                .textFieldStyle(.roundedBorder)
                .font(.system(size: 11))
                .onSubmit {
                    session.commitDataGraphicConfig(undoManager: undoManager)
                }
            if !session.availablePropFields.isEmpty {
                Menu {
                    Button("None (Disabled)") {
                        text.wrappedValue = ""
                        session.commitDataGraphicConfig(undoManager: undoManager)
                    }
                    Divider()
                    ForEach(session.availablePropFields, id: \.self) { field in
                        Button(field) {
                            text.wrappedValue = field
                            session.commitDataGraphicConfig(undoManager: undoManager)
                        }
                    }
                } label: {
                    Image(systemName: "chevron.up.chevron.down")
                        .font(.system(size: 9.5, weight: .semibold))
                        .frame(width: 18, height: 18)
                }
                .menuStyle(.borderlessButton)
                .fixedSize()
                .help("Pick discovered Prop.* field")
            }
        }
    }

    @ViewBuilder
    private func externalDataRecordsetsCard(selectedObject: InspectedObjectState?) -> some View {
        inspectorCard(title: "External Data (.xlsx / .csv / .sqlite / .json)", icon: "tablecells") {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 5) {
                    Button {
                        session.presentImportExternalDataModal(undoManager: undoManager)
                    } label: {
                        Label("Link Data…", systemImage: "tray.and.arrow.down")
                            .font(.system(size: 11, weight: .medium))
                            .lineLimit(1)
                            .fixedSize(horizontal: true, vertical: false)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .help("Import & link .xlsx, .csv, .sqlite/.db, or .json recordset")

                    Button {
                        session.refreshExternalDataSources(undoManager: undoManager)
                    } label: {
                        Image(systemName: "arrow.clockwise")
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .help("Refresh linked External Data sources")

                    Button {
                        session.presentTwoWayWritebackModal(recordsetId: 0, undoManager: undoManager)
                    } label: {
                        Image(systemName: "arrow.up.doc")
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .help("Two-Way Writeback: Export updated Prop.* values to .xlsx / .csv / .sqlite / .json")

                    Spacer()

                    Button {
                        session.presentBomReportExportModal()
                    } label: {
                        Image(systemName: "doc.text.magnifyingglass")
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .help("Export BOM Shape Inventory Report (.xlsx)")
                }

                if session.externalRecordsets.isEmpty {
                    Text("No External Data linked yet. Import an Excel (.xlsx), CSV, SQLite (.sqlite), or JSON table to bind rows to shapes.")
                        .font(.system(size: 10.5))
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                } else {
                    ForEach(session.externalRecordsets) { rs in
                        VStack(alignment: .leading, spacing: 5) {
                            HStack {
                                Text("\(rs.name) (\(rs.sourceKind.uppercased()))")
                                    .font(.system(size: 11, weight: .semibold))
                                    .lineLimit(1)
                                Spacer()
                                Text("\(rs.rows.count) rows")
                                    .font(.system(size: 10, design: .monospaced))
                                    .foregroundStyle(.secondary)
                                Button {
                                    session.removeExternalRecordset(recordsetId: rs.id, undoManager: undoManager)
                                } label: {
                                    Image(systemName: "trash")
                                        .font(.system(size: 10))
                                        .foregroundStyle(.secondary)
                                }
                                .buttonStyle(.plain)
                                .help("Unlink '\(rs.name)'")
                            }

                            ForEach(Array(rs.rows.prefix(25).enumerated()), id: \.offset) { rowIdx, rowMap in
                                let keyVal = rowMap[rs.keyColumn] ?? "Row \(rowIdx + 1)"
                                let summaryPairs = rs.columns
                                    .filter { $0 != rs.keyColumn }
                                    .prefix(2)
                                    .compactMap { col -> String? in
                                        guard let v = rowMap[col], !v.isEmpty else { return nil }
                                        return "\(col): \(v)"
                                    }
                                    .joined(separator: " • ")
                                let boundIds = rs.boundShapeIdsByRow[rowIdx] ?? []

                                HStack(spacing: 6) {
                                    VStack(alignment: .leading, spacing: 1) {
                                        HStack(spacing: 4) {
                                            Text(keyVal)
                                                .font(.system(size: 11, weight: .medium))
                                                .lineLimit(1)
                                            if !boundIds.isEmpty {
                                                Text("✓ \(boundIds.count)")
                                                    .font(.system(size: 9.5, weight: .semibold, design: .monospaced))
                                                    .foregroundStyle(.green)
                                            }
                                        }
                                        if !summaryPairs.isEmpty {
                                            Text(summaryPairs)
                                                .font(.system(size: 9.5))
                                                .foregroundStyle(.secondary)
                                                .lineLimit(1)
                                        }
                                    }
                                    Spacer()
                                    Button(selectedObject != nil ? "Link" : "+ Stamp") {
                                        session.bindExternalRecordsetRow(
                                            recordsetId: rs.id,
                                            rowIndex: rowIdx,
                                            undoManager: undoManager
                                        )
                                    }
                                    .buttonStyle(.bordered)
                                    .controlSize(.mini)
                                    .help(
                                        selectedObject != nil
                                            ? "Bind row '\(keyVal)' onto selected shape(s)"
                                            : "Create & bind new shape on canvas for '\(keyVal)'"
                                    )
                                }
                                .padding(.horizontal, 6)
                                .padding(.vertical, 4)
                                .background(
                                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                                        .fill(Color.primary.opacity(0.04))
                                )
                            }
                        }
                        .padding(6)
                        .background(
                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                .fill(Color.primary.opacity(0.03))
                        )
                    }
                }
            }
        }
    }

    private func commitNewShapeDataField() {
        let trimmedKey = newPropName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedKey.isEmpty else { return }
        let label = newPropLabel.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? trimmedKey : newPropLabel
        session.upsertSelectedShapeData(
            rowName: trimmedKey,
            label: label,
            dataType: newPropType,
            value: newPropValue,
            undoManager: undoManager
        )
        newPropName = ""
        newPropLabel = ""
        newPropValue = ""
    }

    // MARK: - Pane 4: MS Visio "Layer Properties" & Object Layer Assignment

    @ViewBuilder
    private func layersPane(selectedObject: InspectedObjectState?) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            inspectorCard(
                title: selectedObject != nil ? i18n.t("layerMembership") : i18n.t("pageLayers"),
                icon: "square.3.layers.3d"
            ) {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(session.layers) { layer in
                        let isAssigned = selectedObject?.layerIndices.contains(layer.id) ?? false
                        HStack(spacing: 6) {
                            if selectedObject != nil {
                                Toggle(
                                    "",
                                    isOn: Binding(
                                        get: { isAssigned },
                                        set: { checked in
                                            session.setSelectedShapeLayerMembership(
                                                layerId: layer.id,
                                                assigned: checked,
                                                undoManager: undoManager
                                            )
                                        }
                                    )
                                )
                                .toggleStyle(.checkbox)
                                .labelsHidden()
                                .help(isAssigned ? "Remove object from '\(layer.name)'" : "Assign object to '\(layer.name)'")
                            }

                            Circle()
                                .fill(Color(vexioHex: layer.accentHex))
                                .frame(width: 8, height: 8)

                            Text(layer.name)
                                .font(.system(size: 11.5, weight: isAssigned ? .semibold : .regular))
                                .lineLimit(1)

                            Text("\(layer.memberCount)")
                                .font(.system(size: 9.5, weight: .medium, design: .monospaced))
                                .foregroundStyle(.secondary)
                                .padding(.horizontal, 4.5)
                                .padding(.vertical, 1)
                                .background(Capsule().fill(Color.primary.opacity(0.07)))
                                .help("\(layer.memberCount) object(s) on '\(layer.name)'")

                            Spacer(minLength: 2)

                            // Visible Toggle
                            Button {
                                session.updatePageLayerFlags(
                                    layerId: layer.id,
                                    visible: !layer.isVisible,
                                    locked: layer.isLocked,
                                    printable: layer.isPrintable,
                                    snapGlue: layer.isSnapGlueEnabled,
                                    undoManager: undoManager
                                )
                            } label: {
                                Image(systemName: layer.isVisible ? "eye" : "eye.slash")
                                    .font(.system(size: 11))
                                    .foregroundStyle(layer.isVisible ? Color.primary : Color.secondary)
                                    .frame(width: 22, height: 22)
                            }
                            .buttonStyle(.plain)
                            .help(layer.isVisible ? "Hide Layer '\(layer.name)'" : "Show Layer '\(layer.name)'")

                            // Lock Toggle
                            Button {
                                session.updatePageLayerFlags(
                                    layerId: layer.id,
                                    visible: layer.isVisible,
                                    locked: !layer.isLocked,
                                    printable: layer.isPrintable,
                                    snapGlue: layer.isSnapGlueEnabled,
                                    undoManager: undoManager
                                )
                            } label: {
                                Image(systemName: layer.isLocked ? "lock.fill" : "lock.open")
                                    .font(.system(size: 11))
                                    .foregroundStyle(layer.isLocked ? Color.orange : Color.secondary)
                                    .frame(width: 22, height: 22)
                            }
                            .buttonStyle(.plain)
                            .help(layer.isLocked ? "Unlock Layer '\(layer.name)'" : "Lock Layer '\(layer.name)'")

                            // Print Toggle
                            Button {
                                session.updatePageLayerFlags(
                                    layerId: layer.id,
                                    visible: layer.isVisible,
                                    locked: layer.isLocked,
                                    printable: !layer.isPrintable,
                                    snapGlue: layer.isSnapGlueEnabled,
                                    undoManager: undoManager
                                )
                            } label: {
                                Image(systemName: layer.isPrintable ? "printer.fill" : "printer")
                                    .font(.system(size: 11))
                                    .foregroundStyle(layer.isPrintable ? Color.primary : Color.secondary.opacity(0.5))
                                    .frame(width: 22, height: 22)
                            }
                            .buttonStyle(.plain)
                            .help(layer.isPrintable ? "Disable Printing for '\(layer.name)'" : "Enable Printing for '\(layer.name)'")

                            // Snap & Glue Toggle
                            Button {
                                session.updatePageLayerFlags(
                                    layerId: layer.id,
                                    visible: layer.isVisible,
                                    locked: layer.isLocked,
                                    printable: layer.isPrintable,
                                    snapGlue: !layer.isSnapGlueEnabled,
                                    undoManager: undoManager
                                )
                            } label: {
                                Image(systemName: "scope")
                                    .font(.system(size: 11))
                                    .foregroundStyle(layer.isSnapGlueEnabled ? Color.accentColor : Color.secondary.opacity(0.45))
                                    .frame(width: 22, height: 22)
                            }
                            .buttonStyle(.plain)
                            .help(layer.isSnapGlueEnabled ? "Disable Snap & Glue on '\(layer.name)'" : "Enable Snap & Glue on '\(layer.name)'")
                        }
                        .padding(.horizontal, 7)
                        .padding(.vertical, 5)
                        .background(
                            RoundedRectangle(cornerRadius: 7, style: .continuous)
                                .fill(isAssigned ? Color.accentColor.opacity(0.11) : Color.primary.opacity(0.035))
                        )
                    }

                    Divider()

                    HStack(spacing: 6) {
                        TextField(i18n.t("newLayerPlaceholder"), text: $newLayerNameDraft)
                            .textFieldStyle(.roundedBorder)
                            .font(.system(size: 11))
                            .onSubmit {
                                guard !newLayerNameDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
                                session.addPageLayer(name: newLayerNameDraft, undoManager: undoManager)
                                newLayerNameDraft = ""
                            }

                        Button {
                            session.addPageLayer(name: newLayerNameDraft, undoManager: undoManager)
                            newLayerNameDraft = ""
                        } label: {
                            Image(systemName: "plus")
                                .font(.system(size: 11, weight: .bold))
                        }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.small)
                        .help(i18n.t("addLayer"))
                    }
                }
            }
        }
    }

    // MARK: - Inspector Reusable Subcomponents

    @ViewBuilder
    private func sectionHeaderLabel(_ title: String, icon: String) -> some View {
        HStack(spacing: 6) {
            Image(systemName: icon)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(Color.accentColor)
            Text(title)
                .font(.system(size: 11.5, weight: .semibold))
        }
    }

    @ViewBuilder
    private func inspectorCard<Content: View>(title: String, icon: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            sectionHeaderLabel(title, icon: icon)
            content()
        }
        .padding(9)
        .background(RoundedRectangle(cornerRadius: 9, style: .continuous).fill(Color.primary.opacity(0.035)))
        .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(Color.primary.opacity(0.07), lineWidth: 0.8))
    }

    @ViewBuilder
    private func fillModeChip(title: String, active: Bool, tip: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .font(.system(size: 10.5, weight: active ? .semibold : .regular))
                .lineLimit(1)
                .minimumScaleFactor(0.85)
                .foregroundStyle(active ? Color.accentColor : Color.primary)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 4.5)
                .padding(.horizontal, 4)
                .background(
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(active ? Color.accentColor.opacity(0.18) : Color.primary.opacity(0.05))
                )
        }
        .buttonStyle(.plain)
        .help(tip)
    }

    @ViewBuilder
    private func inspectorSmallToggleButton(
        _ char: String,
        bold: Bool = false,
        italic: Bool = false,
        underline: Bool = false,
        active: Bool,
        tip: String,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Text(char)
                .font(.system(size: 11.5, weight: bold ? .bold : .semibold))
                .italic(italic)
                .underline(underline)
                .frame(width: 24, height: 24)
                .background(
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(active ? Color.accentColor.opacity(0.22) : Color.primary.opacity(0.05))
                )
        }
        .buttonStyle(.plain)
        .help(tip)
    }
}

private struct InspectorUnitField: View {
    let label: String
    let unitSuffix: String
    let displayValue: String
    let tooltip: String
    let onCommit: (String) -> Void

    @State private var draft: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 3) {
                Text(label)
                    .font(.system(size: 10, weight: .medium))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                Spacer(minLength: 0)
                Text(unitSuffix)
                    .font(.system(size: 9.5, design: .monospaced))
                    .foregroundStyle(.tertiary)
            }
            TextField(displayValue, text: $draft)
                .textFieldStyle(.roundedBorder)
                .font(.system(size: 11.5, design: .monospaced))
                .onSubmit {
                    onCommit(draft)
                }
        }
        .frame(maxWidth: .infinity)
        .help(tooltip)
        .onAppear { draft = displayValue }
        .onChange(of: displayValue) { _, updated in draft = updated }
    }
}

private struct InspectorShapeDataRowEditor: View {
    let item: ShapeDataPropertyItem
    let onCommitValue: (String) -> Void
    let onDelete: () -> Void
    let onInspectFormula: () -> Void

    @State private var draft: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 5) {
                Text(item.label)
                    .font(.system(size: 11.5, weight: .semibold))
                    .lineLimit(1)
                Text(item.dataType)
                    .font(.system(size: 9, weight: .medium))
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 4.5)
                    .padding(.vertical, 1)
                    .background(Capsule().fill(Color.primary.opacity(0.07)))
                Spacer(minLength: 2)
                Button("fx") {
                    onInspectFormula()
                }
                .buttonStyle(.borderless)
                .font(.system(size: 10, weight: .bold, design: .monospaced))
                .help("Inspect \(item.rowKey) in ShapeSheet")

                Button {
                    onDelete()
                } label: {
                    Image(systemName: "trash")
                        .font(.system(size: 10))
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .help("Delete \(item.rowKey)")
            }

            if item.dataTypeCode == 3 {
                Toggle(
                    item.rowKey,
                    isOn: Binding(
                        get: {
                            let lower = item.evaluatedValue.lowercased()
                            return lower == "true" || lower == "1"
                        },
                        set: { checked in
                            onCommitValue(checked ? "TRUE" : "FALSE")
                        }
                    )
                )
                .toggleStyle(.switch)
                .controlSize(.mini)
                .font(.system(size: 10.5, design: .monospaced))
            } else {
                HStack(spacing: 5) {
                    TextField(item.evaluatedValue, text: $draft)
                        .textFieldStyle(.roundedBorder)
                        .font(.system(size: 11))
                        .onSubmit {
                            onCommitValue(draft)
                        }
                    Button("Set") {
                        onCommitValue(draft)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.mini)
                    .help("Save \(item.rowKey)")
                }
            }
        }
        .padding(7)
        .background(RoundedRectangle(cornerRadius: 7, style: .continuous).fill(Color.primary.opacity(0.04)))
        .onAppear { draft = item.evaluatedValue }
        .onChange(of: item.evaluatedValue) { _, updated in draft = updated }
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

// MARK: - In-App Page Setup Modal Sheet (Size, Orientation, Units, Background & Paper Color)

private struct VexioPageSetupSheet: View {
    @ObservedObject var session: VexioDocumentSession
    @ObservedObject private var i18n: VexioLocalization = .shared
    let pageId: UInt32
    let onClose: () -> Void

    @Environment(\.undoManager) private var undoManager

    @State private var nameDraft: String = ""
    @State private var widthDraft: String = ""
    @State private var heightDraft: String = ""
    @State private var selectedUnit: VexioMeasurementUnit = .osDefault
    @State private var isLandscape: Bool = true
    @State private var isBackground: Bool = false
    @State private var assignedBackPageId: UInt32 = 0
    @State private var pageColorHex: String = "#FFFFFF"

    private var availableBackgroundPages: [VexioPageSummary] {
        session.pages.filter { $0.isBackgroundPage && $0.id != pageId }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Label(i18n.t("pageSetup"), systemImage: "doc.badge.gearshape")
                    .font(.system(size: 16, weight: .semibold))
                Spacer()
                Button(i18n.t("cancel"), action: onClose)
                    .keyboardShortcut(.cancelAction)
                Button(i18n.t("done"), action: applyAndClose)
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
            }

            Divider()

            pageIdentityAndPresetSection
            dimensionsAndUnitsSection
            backgroundSection
        }
        .padding(18)
        .frame(width: 490)
        .onAppear(perform: populateFromSession)
    }

    @ViewBuilder
    private var pageIdentityAndPresetSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                Text(i18n.t("objectName"))
                    .font(.system(size: 12, weight: .medium))
                    .frame(width: 96, alignment: .leading)
                TextField("Page-1", text: $nameDraft)
                    .textFieldStyle(.roundedBorder)
            }

            HStack(spacing: 10) {
                Text(i18n.t("pageSize"))
                    .font(.system(size: 12, weight: .medium))
                    .frame(width: 96, alignment: .leading)
                HStack(spacing: 5) {
                    ForEach([VexioPagePreset.a4, .a3, .a5, .letter, .legal, .tabloid], id: \.id) { preset in
                        Button(preset.shortCode) {
                            applyPresetToDrafts(preset)
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .help(preset.displayLabel)
                    }
                }
            }

            HStack(spacing: 10) {
                Text(i18n.t("orientation"))
                    .font(.system(size: 12, weight: .medium))
                    .frame(width: 96, alignment: .leading)
                HStack(spacing: 8) {
                    Button {
                        setDraftOrientation(landscape: true)
                    } label: {
                        Label(i18n.t("landscape"), systemImage: "rectangle.portrait.rotate")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .tint(isLandscape ? .accentColor : nil)

                    Button {
                        setDraftOrientation(landscape: false)
                    } label: {
                        Label(i18n.t("portrait"), systemImage: "rectangle.portrait")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .tint(!isLandscape ? .accentColor : nil)
                }
            }
        }
    }

    @ViewBuilder
    private var dimensionsAndUnitsSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                Text(i18n.t("units"))
                    .font(.system(size: 12, weight: .medium))
                    .frame(width: 96, alignment: .leading)
                Picker("", selection: $selectedUnit) {
                    Text("\(i18n.t("osStandard")) (\(VexioMeasurementUnit.osDefault.resolvedEngineSlug))").tag(VexioMeasurementUnit.osDefault)
                    Text("Millimeters (mm)").tag(VexioMeasurementUnit.mm)
                    Text("Centimeters (cm)").tag(VexioMeasurementUnit.cm)
                    Text("Meters (m)").tag(VexioMeasurementUnit.m)
                    Text("Inches (in)").tag(VexioMeasurementUnit.inch)
                }
                .labelsHidden()
                .onChange(of: selectedUnit) { oldUnit, newUnit in
                    let wIn = parseInches(widthDraft, unit: oldUnit, fallback: 11.6929)
                    let hIn = parseInches(heightDraft, unit: oldUnit, fallback: 8.2677)
                    widthDraft = newUnit.numericDisplayFromInches(wIn)
                    heightDraft = newUnit.numericDisplayFromInches(hIn)
                }
            }

            HStack(spacing: 10) {
                Text("\(i18n.t("width")) × \(i18n.t("height"))")
                    .font(.system(size: 12, weight: .medium))
                    .frame(width: 96, alignment: .leading)
                TextField("Width", text: $widthDraft)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: 12, design: .monospaced))
                Text("×")
                    .foregroundStyle(.secondary)
                TextField("Height", text: $heightDraft)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: 12, design: .monospaced))
                Text(selectedUnit.resolvedEngineSlug)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder
    private var backgroundSection: some View {
        let swatches = ["#FFFFFF", "#FAF8F5", "#F1F5F9", "#EFF6FF", "#ECFDF5"]
        VStack(alignment: .leading, spacing: 10) {
            Divider()
            HStack(spacing: 10) {
                Text(i18n.t("pageBackground"))
                    .font(.system(size: 12, weight: .medium))
                    .frame(width: 96, alignment: .leading)
                HStack(spacing: 6) {
                    ForEach(swatches, id: \.self) { hex in
                        let active = pageColorHex.caseInsensitiveCompare(hex) == .orderedSame
                        Button {
                            pageColorHex = hex
                        } label: {
                            Circle()
                                .fill(Color(vexioHex: hex))
                                .frame(width: 22, height: 22)
                                .overlay(
                                    Circle()
                                        .strokeBorder(active ? Color.accentColor : Color.primary.opacity(0.22), lineWidth: active ? 2.0 : 1.0)
                                )
                        }
                        .buttonStyle(.plain)
                    }
                    ColorPicker(
                        "",
                        selection: Binding(
                            get: { Color(vexioHex: pageColorHex) },
                            set: { pageColorHex = $0.vexioHexRGBString }
                        ),
                        supportsOpacity: false
                    )
                    .labelsHidden()
                }
            }

            Toggle("Mark as Background Page", isOn: $isBackground)
                .font(.system(size: 12))

            if !isBackground && !availableBackgroundPages.isEmpty {
                HStack(spacing: 10) {
                    Text("Assigned Bg")
                        .font(.system(size: 12, weight: .medium))
                        .frame(width: 96, alignment: .leading)
                    Picker("", selection: $assignedBackPageId) {
                        Text("None").tag(UInt32(0))
                        ForEach(availableBackgroundPages) { bgp in
                            Text(bgp.title).tag(bgp.id)
                        }
                    }
                    .labelsHidden()
                }
            }
        }
    }

    private func populateFromSession() {
        selectedUnit = i18n.unitPreference
        let summary = session.pages.first(where: { $0.id == pageId })
        nameDraft = summary?.title ?? "Page-\(pageId)"
        let wIn = summary?.widthInches ?? session.pageWidthInches
        let hIn = summary?.heightInches ?? session.pageHeightInches
        isLandscape = wIn >= hIn
        widthDraft = selectedUnit.numericDisplayFromInches(wIn)
        heightDraft = selectedUnit.numericDisplayFromInches(hIn)
        isBackground = summary?.isBackgroundPage ?? false
        assignedBackPageId = summary?.backPageId ?? 0
        pageColorHex = summary?.pageColorHex ?? "#FFFFFF"
    }

    private func presetInches(_ preset: VexioPagePreset) -> (Double, Double) {
        switch preset {
        case .a4: return (210.0 / 25.4, 297.0 / 25.4)
        case .a3: return (297.0 / 25.4, 420.0 / 25.4)
        case .a5: return (148.0 / 25.4, 210.0 / 25.4)
        case .a2: return (420.0 / 25.4, 594.0 / 25.4)
        case .a1: return (594.0 / 25.4, 841.0 / 25.4)
        case .a0: return (841.0 / 25.4, 1189.0 / 25.4)
        case .letter: return (8.5, 11.0)
        case .legal: return (8.5, 14.0)
        case .tabloid: return (11.0, 17.0)
        }
    }

    private func applyPresetToDrafts(_ preset: VexioPagePreset) {
        let (shortSide, longSide) = presetInches(preset)
        let wIn = isLandscape ? longSide : shortSide
        let hIn = isLandscape ? shortSide : longSide
        widthDraft = selectedUnit.numericDisplayFromInches(wIn)
        heightDraft = selectedUnit.numericDisplayFromInches(hIn)
    }

    private func setDraftOrientation(landscape: Bool) {
        let wIn = parseInches(widthDraft, unit: selectedUnit, fallback: 11.6929)
        let hIn = parseInches(heightDraft, unit: selectedUnit, fallback: 8.2677)
        let hi = max(wIn, hIn)
        let lo = min(wIn, hIn)
        isLandscape = landscape
        widthDraft = selectedUnit.numericDisplayFromInches(landscape ? hi : lo)
        heightDraft = selectedUnit.numericDisplayFromInches(landscape ? lo : hi)
    }

    private func parseInches(_ raw: String, unit: VexioMeasurementUnit, fallback: Double) -> Double {
        let cleaned = raw.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: ",", with: ".")
        guard let v = Double(cleaned), v > 0 else { return fallback }
        switch unit.resolvedEngineSlug {
        case "mm": return v / 25.4
        case "cm": return v / 2.54
        case "m": return v / 0.0254
        default: return v
        }
    }

    private func applyAndClose() {
        i18n.unitPreference = selectedUnit
        let wIn = parseInches(widthDraft, unit: selectedUnit, fallback: session.pageWidthInches)
        let hIn = parseInches(heightDraft, unit: selectedUnit, fallback: session.pageHeightInches)
        session.applyCustomPageSetup(
            pageId: pageId,
            name: nameDraft,
            widthInches: wIn,
            heightInches: hIn,
            unitSlug: selectedUnit.resolvedEngineSlug,
            isBackground: isBackground,
            backPageId: assignedBackPageId,
            pageColorHex: pageColorHex,
            undoManager: undoManager
        )
        onClose()
    }
}

// MARK: - Visio "Design -> Borders & Titles" Gallery & Metadata Customization Sheet

private struct VexioBordersAndTitlesSheet: View {
    @ObservedObject var session: VexioDocumentSession
    @ObservedObject private var i18n: VexioLocalization = .shared
    let onClose: () -> Void

    @Environment(\.undoManager) private var undoManager

    @State private var selectedStyle: VexioBorderTitleStyle = .modernBanner
    @State private var titleDraft: String = ""
    @State private var subtitleDraft: String = ""
    @State private var authorDraft: String = ""
    @State private var dateDraft: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Label(i18n.t("bordersAndTitles"), systemImage: "rectangle.topthird.inset.filled")
                    .font(.system(size: 16, weight: .semibold))
                Spacer()
                Button("Remove Border & Title", role: .destructive) {
                    session.applyBordersAndTitles(style: .none, undoManager: undoManager)
                    onClose()
                }
                .buttonStyle(.bordered)
                Button(i18n.t("cancel"), action: onClose)
                    .keyboardShortcut(.cancelAction)
                Button("Apply", action: applySelection)
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
            }

            Divider()

            stylePickerSection
            metadataFieldsSection
        }
        .padding(18)
        .frame(width: 510)
        .onAppear {
            titleDraft = session.documentTitle
            let formatter = DateFormatter()
            formatter.dateStyle = .medium
            dateDraft = formatter.string(from: Date())
            authorDraft = NSFullUserName()
        }
    }

    @ViewBuilder
    private var stylePickerSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Border & Title Style")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.secondary)

            ForEach(VexioBorderTitleStyle.allCases.filter { $0 != .none }) { style in
                let active = (selectedStyle == style)
                Button {
                    selectedStyle = style
                } label: {
                    HStack(spacing: 10) {
                        Image(systemName: active ? "checkmark.circle.fill" : "circle")
                            .foregroundStyle(active ? Color.accentColor : .secondary)
                        Text(style.displayName)
                            .font(.system(size: 12.5, weight: active ? .semibold : .regular))
                        Spacer()
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 7)
                    .background(
                        RoundedRectangle(cornerRadius: 8, style: .continuous)
                            .fill(active ? Color.accentColor.opacity(0.12) : Color.primary.opacity(0.04))
                    )
                }
                .buttonStyle(.plain)
            }
        }
    }

    @ViewBuilder
    private var metadataFieldsSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Divider()
            HStack(spacing: 10) {
                Text("Diagram Title")
                    .font(.system(size: 12, weight: .medium))
                    .frame(width: 110, alignment: .leading)
                TextField("System Architecture", text: $titleDraft)
                    .textFieldStyle(.roundedBorder)
            }
            HStack(spacing: 10) {
                Text("Subtitle / Code")
                    .font(.system(size: 12, weight: .medium))
                    .frame(width: 110, alignment: .leading)
                TextField("Phase 2 Overview / VEX-001", text: $subtitleDraft)
                    .textFieldStyle(.roundedBorder)
            }
            HStack(spacing: 10) {
                Text("Author")
                    .font(.system(size: 12, weight: .medium))
                    .frame(width: 110, alignment: .leading)
                TextField("Author", text: $authorDraft)
                    .textFieldStyle(.roundedBorder)
            }
            HStack(spacing: 10) {
                Text("Date / Revision")
                    .font(.system(size: 12, weight: .medium))
                    .frame(width: 110, alignment: .leading)
                TextField("Rev 1.0", text: $dateDraft)
                    .textFieldStyle(.roundedBorder)
            }
        }
    }

    private func applySelection() {
        session.applyBordersAndTitles(
            style: selectedStyle,
            title: titleDraft,
            subtitle: subtitleDraft,
            author: authorDraft,
            dateOrRev: dateDraft,
            undoManager: undoManager
        )
        onClose()
    }
}

// MARK: - v0.5.0-beta Diagram-as-Code Studio (Mermaid / PlantUML / Graphviz DOT) & Slide Snippets Sheets

private struct VexioMarkupStudioSheet: View {
    @ObservedObject var session: VexioDocumentSession
    let onClose: () -> Void
    @Environment(\.undoManager) private var undoManager

    @State private var selectedFormat: String = "mermaid"
    @State private var markupText: String = """
    flowchart TD
        subgraph Ingress ["API Gateway"]
            A(["Client Request"]) --> B{"Authenticated?"}
        end
        B -->|Yes| C[["Order Service"]]
        B -.->|No| D(["401 Unauthorized"])
        C --> E[("Orders PostgreSQL")]
    """

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Diagram-as-Code Studio — Mermaid · PlantUML · Graphviz DOT")
                    .font(.system(size: 14, weight: .semibold))
                Spacer()
                Button("Close", action: onClose)
            }

            HStack(spacing: 10) {
                Picker("Format", selection: $selectedFormat) {
                    Text("Mermaid (.mmd)").tag("mermaid")
                    Text("PlantUML (.puml)").tag("plantuml")
                    Text("Graphviz DOT (.dot)").tag("dot")
                }
                .pickerStyle(.segmented)
                .onChange(of: selectedFormat) { _, nextFmt in
                    let exported = session.exportDiagramMarkup(formatSlug: nextFmt)
                    if !exported.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        markupText = exported
                    }
                }

                Button("← Read Active Page") {
                    let exported = session.exportDiagramMarkup(formatSlug: selectedFormat)
                    if !exported.isEmpty {
                        markupText = exported
                    }
                }
            }

            TextEditor(text: $markupText)
                .font(.system(size: 12, design: .monospaced))
                .frame(minHeight: 240)
                .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.primary.opacity(0.14), lineWidth: 1))

            HStack {
                Spacer()
                Button("Cancel", action: onClose)
                Button("Build Visio Shapes on Page") {
                    session.importDiagramMarkup(markup: markupText, formatSlug: selectedFormat, undoManager: undoManager)
                    onClose()
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(18)
        .frame(width: 620, height: 420)
        .onAppear {
            if session.activePageShapeCount > 0 {
                let exported = session.exportDiagramMarkup(formatSlug: selectedFormat)
                if !exported.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    markupText = exported
                }
            }
        }
    }
}

private struct VexioSlideSnippetsSheet: View {
    @ObservedObject var session: VexioDocumentSession
    let onExportPptx: () -> Void
    let onClose: () -> Void
    @Environment(\.undoManager) private var undoManager

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Presentation Mode — Slide Snippets & Multi-Slide .pptx")
                    .font(.system(size: 14, weight: .semibold))
                Spacer()
                Button("Close", action: onClose)
            }

            HStack(spacing: 8) {
                Button("Auto-Generate Slide Snippets") {
                    session.autoGenerateSlideSnippets(undoManager: undoManager)
                }
                .buttonStyle(.borderedProminent)

                if !session.slideSnippets.isEmpty {
                    Button("Clear All") {
                        session.clearSlideSnippets(undoManager: undoManager)
                    }
                }
                Spacer()
                Button("Export Multi-Slide .pptx…", action: onExportPptx)
            }

            ScrollView {
                VStack(alignment: .leading, spacing: 6) {
                    if session.slideSnippets.isEmpty {
                        Text("Click 'Auto-Generate Slide Snippets' to create presenter frames from Containers, Swimlanes, or diagram stages.")
                            .font(.system(size: 12))
                            .foregroundStyle(.secondary)
                            .padding(.vertical, 16)
                    } else {
                        ForEach(Array(session.slideSnippets.enumerated()), id: \.element.id) { idx, snippet in
                            HStack {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text("#\(idx + 1) · \(snippet.title)")
                                        .font(.system(size: 12.5, weight: .semibold))
                                    if !snippet.notes.isEmpty {
                                        Text(snippet.notes)
                                            .font(.system(size: 11))
                                            .foregroundStyle(.secondary)
                                    }
                                }
                                Spacer()
                                Button("Present Frame ▶") {
                                    session.focusSlideSnippet(snippet)
                                    onClose()
                                }
                                .controlSize(.small)
                            }
                            .padding(8)
                            .background(RoundedRectangle(cornerRadius: 8).fill(Color.primary.opacity(0.045)))
                        }
                    }
                }
            }
            .frame(minHeight: 200)
        }
        .padding(18)
        .frame(width: 560, height: 380)
        .onAppear {
            session.refreshSlideSnippets()
        }
    }
}

// MARK: - v0.6.0-beta Sheets (VBA Macro IDE, Review Comments, Hyperlinks/ScreenTips, Domain Wizards & CAD Tools)

private struct VexioVbaMacroStudioSheet: View {
    @ObservedObject var session: VexioDocumentSession
    let onClose: () -> Void
    @Environment(\.undoManager) private var undoManager

    @State private var vbaScript: String = """
    Sub BuildArchitectureCluster()
        For i = 1 To 3
            Set shp = ActivePage.DrawRectangle(1.0 + i * 2.3, 4.2, 2.7 + i * 2.3, 5.2)
            shp.Text = "Service Node " & i
        Next i
    End Sub
    """
    @State private var executionOutput: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label("Sandboxed VBA Automation Studio", systemImage: "terminal")
                    .font(.system(size: 14, weight: .semibold))
                Spacer()
                Button("Close", action: onClose)
            }
            TextEditor(text: $vbaScript)
                .font(.system(size: 12, design: .monospaced))
                .frame(minHeight: 200)
                .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.primary.opacity(0.14), lineWidth: 1))
            if !executionOutput.isEmpty {
                Text(executionOutput)
                    .font(.system(size: 11.5, design: .monospaced))
                    .foregroundStyle(.secondary)
                    .padding(8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(RoundedRectangle(cornerRadius: 6).fill(Color.primary.opacity(0.05)))
            }
            HStack {
                Spacer()
                Button("Cancel", action: onClose)
                Button("Execute in Bounded Sandbox ▶") {
                    executionOutput = session.executeVbaMacroScript(vbaScript, undoManager: undoManager)
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(18)
        .frame(width: 600, height: 400)
    }
}

private struct VexioReviewCommentsSheet: View {
    @ObservedObject var session: VexioDocumentSession
    let onClose: () -> Void
    @Environment(\.undoManager) private var undoManager

    @State private var comments: [VexioReviewCommentItem] = []
    @State private var authorDraft: String = NSFullUserName()
    @State private var textDraft: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label("Review Comments & Annotations", systemImage: "text.bubble")
                    .font(.system(size: 14, weight: .semibold))
                Spacer()
                Button("Done", action: onClose)
            }
            HStack(spacing: 8) {
                TextField("Reviewer", text: $authorDraft)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 140)
                TextField("Add review note on selected shape or active page…", text: $textDraft)
                    .textFieldStyle(.roundedBorder)
                Button("Post") {
                    guard !textDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
                    session.addReviewComment(author: authorDraft, text: textDraft, undoManager: undoManager)
                    textDraft = ""
                    comments = session.fetchReviewComments()
                }
                .buttonStyle(.borderedProminent)
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 6) {
                    if comments.isEmpty {
                        Text("No review comments on this page yet.")
                            .font(.system(size: 12))
                            .foregroundStyle(.secondary)
                            .padding(.vertical, 12)
                    } else {
                        ForEach(comments) { c in
                            HStack {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text("\(c.author)\(c.shape_id != nil ? " · Sheet.\(c.shape_id!)" : "")\(c.resolved ? " ✓ Resolved" : "")")
                                        .font(.system(size: 11.5, weight: .semibold))
                                    Text(c.text)
                                        .font(.system(size: 12))
                                        .strikethrough(c.resolved)
                                }
                                Spacer()
                                Button(c.resolved ? "Reopen" : "Resolve") {
                                    session.toggleReviewCommentResolved(commentId: c.id, resolved: !c.resolved, undoManager: undoManager)
                                    comments = session.fetchReviewComments()
                                }
                                .controlSize(.small)
                                Button(role: .destructive) {
                                    session.deleteReviewComment(commentId: c.id, undoManager: undoManager)
                                    comments = session.fetchReviewComments()
                                } label: {
                                    Image(systemName: "trash")
                                }
                                .controlSize(.small)
                            }
                            .padding(8)
                            .background(RoundedRectangle(cornerRadius: 8).fill(Color.primary.opacity(0.045)))
                        }
                    }
                }
            }
            .frame(minHeight: 200)
        }
        .padding(18)
        .frame(width: 560, height: 370)
        .onAppear {
            comments = session.fetchReviewComments()
        }
    }
}

private struct VexioHyperlinkScreentipSheet: View {
    @ObservedObject var session: VexioDocumentSession
    let onClose: () -> Void
    @Environment(\.undoManager) private var undoManager

    @State private var descriptionDraft: String = "Documentation Runbook"
    @State private var addressDraft: String = "https://vexio-chromeos.web.app"
    @State private var subAddressDraft: String = ""
    @State private var screentipDraft: String = "Click or hover to inspect architecture SLA"

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Label("Shape Hyperlink & ScreenTip (⌘K)", systemImage: "link")
                    .font(.system(size: 14, weight: .semibold))
                Spacer()
                Button("Cancel", action: onClose)
            }
            TextField("Description / Label", text: $descriptionDraft)
                .textFieldStyle(.roundedBorder)
            TextField("External URL (https://…)", text: $addressDraft)
                .textFieldStyle(.roundedBorder)
            TextField("Internal Page SubAddress (e.g. Page-2, optional)", text: $subAddressDraft)
                .textFieldStyle(.roundedBorder)
            TextField("Hover ScreenTip Comment", text: $screentipDraft)
                .textFieldStyle(.roundedBorder)
            HStack {
                if let url = URL(string: addressDraft), !addressDraft.isEmpty {
                    Button("Open URL in Browser ↗") {
                        NSWorkspace.shared.open(url)
                    }
                }
                Spacer()
                Button("Apply to Selected Shape") {
                    session.applyHyperlinkAndScreentip(
                        description: descriptionDraft,
                        address: addressDraft,
                        subAddress: subAddressDraft,
                        screentip: screentipDraft,
                        undoManager: undoManager
                    )
                    onClose()
                }
                .buttonStyle(.borderedProminent)
                .disabled(session.selectedShapeId == 0)
            }
        }
        .padding(18)
        .frame(width: 480)
    }
}

private struct VexioDomainWizardsSheet: View {
    @ObservedObject var session: VexioDocumentSession
    let initialSlug: String
    let onClose: () -> Void
    @Environment(\.undoManager) private var undoManager

    @State private var wizardSlug: String = "gantt"
    @State private var specDraft: String = ""
    @State private var outputMessage: String = ""

    private static let defaultsBySlug: [String: String] = [
        "gantt": "ID,Task Name,StartDay,DurationDays,PercentComplete,DependsOn\n1,Architecture Spec,0,4,100,\n2,Rust ShapeSheet & CAD Core,4,7,100,1\n3,Multi-OS Native Viewport,10,6,85,2\n4,Enterprise QA & GA Ship,16,4,50,3",
        "timeline": "# Title: Product Release Roadmap | Horizon: 100\n10,Core Engine Lock,true\n35,CAD & Legacy VSD Parity,false\n65,Multi-OS Public Beta,true\n95,Enterprise v1.0 GA,true",
        "pert": "ID,Task Name,DurationDays,Predecessors\nA,System Architecture,3,\nB,Rust Core & DAG,6,A\nC,Stencil Library,3,A\nD,Multi-OS Shells,5,B;C\nE,Release Signoff,2,D",
        "calendar": "# Month: 2026-11 | Start: Monday\n2026-11-04|Architecture Review\n2026-11-12|CAD & AEC Freeze\n2026-11-20|Security Audit\n2026-11-27|v0.9.0-beta Release",
        "pivot_diagram": "Region,ProductLine,Revenue\nEMEA,Enterprise Cloud,420\nEMEA,Desktop Suite,260\nAmericas,Enterprise Cloud,610\nAmericas,Desktop Suite,340\nAPAC,Enterprise Cloud,310",
        "mind_map": "# Vexio 1.0 Strategy\n## Native Core\n### ShapeSheet DAG\n### OpenXML & Legacy VSD\n## AEC & CAD\n### Auto-Healing Walls\n### Associative Dimensions\n## Plan 2 Analytics\n### Lean VSM\n### Fault Tree MOCUS",
        "mind_map_radial": "# Product Architecture\n## Desktop Native\n### Windows DX12\n### macOS Metal\n## Mobile & Web\n### Googlebook Vulkan\n### ChromeOS Offline PWA",
        "vsm": "Step,CycleSec,LeadDays,UptimePct,InventoryUnits\nRaw Coil Stamping,42,3.5,96,450\nCNC Precision Machining,95,2.0,92,220\nAnodize & Cure,68,4.0,98,380\nFinal QA & Pack,35,1.0,99.5,90",
        "fta": "TOP,Data Center Outage,OR,0,G1;G2\nG1,Dual Utility Loss,AND,0,E1;E2\nG2,Cooling Failure,2/3,0,E3;E4;E5\nE1,Grid Feed A Fault,BASIC,0.02,\nE2,Backup Generator Fail,BASIC,0.015,\nE3,Chiller A Trip,BASIC,0.03,\nE4,Chiller B Trip,BASIC,0.03,\nE5,Standby Chiller C Trip,BASIC,0.04,",
        "sql_reverse": "CREATE TABLE customers (\n  id INT PRIMARY KEY,\n  email VARCHAR NOT NULL,\n  tier VARCHAR\n);\nCREATE TABLE orders (\n  id INT PRIMARY KEY,\n  customer_id INT NOT NULL REFERENCES customers(id),\n  total DECIMAL NOT NULL\n);",
        "org_chart": "Name,Title,Manager,Department\nAda Lovelace,CEO,,Executive\nGrace Hopper,VP Engineering,Ada Lovelace,Platform\nAlan Turing,Principal Architect,Grace Hopper,Core Engine",
        "rack_42u": "Device,StartU,HeightU\nTop-of-Rack Leaf Switch,41,2\nPerimeter Firewall Pair,38,2\nK8s Compute Blade Chassis,26,8\nNVMe SAN Storage Array,14,6\nRedundant UPS Battery,1,4",
        "arch_room": "Room,WidthIn,HeightIn\nMain Server Hall,3.4,2.4\nNOC Operations Suite,2.6,2.0\nMDF Telecom Closet,1.8,1.6",
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label("Enterprise Diagram Wizards — Parameterized Builders", systemImage: "wand.and.stars")
                    .font(.system(size: 14, weight: .semibold))
                Spacer()
                Button("Close", action: onClose)
            }
            Picker("Wizard", selection: $wizardSlug) {
                Text("Gantt Schedule").tag("gantt")
                Text("Project Timeline").tag("timeline")
                Text("PERT / Critical Path").tag("pert")
                Text("Monthly Calendar").tag("calendar")
                Text("PivotDiagram Rollup").tag("pivot_diagram")
                Text("Mind Map (Balanced)").tag("mind_map")
                Text("Mind Map (Radial Star)").tag("mind_map_radial")
                Text("Lean Value Stream (VSM)").tag("vsm")
                Text("Fault Tree Analysis (FTA)").tag("fta")
                Text("SQL ⇄ Crow's Foot ERD").tag("sql_reverse")
                Text("Organization Chart").tag("org_chart")
                Text("42U Rack Elevation").tag("rack_42u")
                Text("Architectural Rooms").tag("arch_room")
            }
            .pickerStyle(.menu)
            .onChange(of: wizardSlug) { _, next in
                specDraft = Self.defaultsBySlug[next] ?? ""
                outputMessage = ""
            }

            TextEditor(text: $specDraft)
                .font(.system(size: 12, design: .monospaced))
                .frame(minHeight: 220)
                .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.primary.opacity(0.14), lineWidth: 1))

            if !outputMessage.isEmpty {
                Text(outputMessage)
                    .font(.system(size: 11.5))
                    .foregroundStyle(.secondary)
            }

            HStack {
                if wizardSlug == "sql_reverse" {
                    Button("← Forward-Engineer Page to SQL DDL") {
                        let res = session.runEnterpriseDomainWizard(wizardSlug: "sql_forward", specText: "", createNewPage: false, undoManager: undoManager)
                        if !res.exportedText.isEmpty {
                            specDraft = res.exportedText
                        }
                        outputMessage = res.summary
                    }
                } else if wizardSlug.hasPrefix("mind_map") {
                    Button("← Export Page Mind Map to Markdown") {
                        let md = session.exportMindMapOutlineMarkdown()
                        if !md.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                            specDraft = md
                            outputMessage = "Exported hierarchical Mind Map outline from active page."
                        }
                    }
                }
                Spacer()
                Button("Build on Active Page") {
                    let res = session.runEnterpriseDomainWizard(wizardSlug: wizardSlug, specText: specDraft, createNewPage: false, undoManager: undoManager)
                    outputMessage = res.summary
                    onClose()
                }
                Button("+ Build on New Page") {
                    let res = session.runEnterpriseDomainWizard(wizardSlug: wizardSlug, specText: specDraft, createNewPage: true, undoManager: undoManager)
                    outputMessage = res.summary
                    onClose()
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(18)
        .frame(width: 650, height: 430)
        .onAppear {
            let clean = Self.defaultsBySlug.keys.contains(initialSlug) ? initialSlug : "gantt"
            wizardSlug = clean
            specDraft = Self.defaultsBySlug[clean] ?? ""
        }
    }
}

private struct VexioCadToolsSheet: View {
    @ObservedObject var session: VexioDocumentSession
    let onExportDwg: () -> Void
    let onClose: () -> Void
    @Environment(\.undoManager) private var undoManager

    @State private var scaleSlug: String = "1/4_in_ft"
    @State private var lockScale: Bool = true
    @State private var statusNote: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label("AutoCAD Interop — Scale Lock, Layer Convert & Binary .DWG", systemImage: "ruler")
                    .font(.system(size: 14, weight: .semibold))
                Spacer()
                Button("Close", action: onClose)
            }
            Picker("Drawing Scale", selection: $scaleSlug) {
                Text("Architectural 1/4\" = 1'-0\" (1:48)").tag("1/4_in_ft")
                Text("Architectural 1/8\" = 1'-0\" (1:96)").tag("1/8_in_ft")
                Text("Metric Engineering 1:50").tag("1:50")
                Text("Metric Floorplan 1:100").tag("1:100")
                Text("Metric Site Plan 1:200").tag("1:200")
                Text("Unscaled 1:1").tag("1:1")
            }
            Toggle("Protect PageScale & DrawingScale with GUARD() and lock CAD Background", isOn: $lockScale)
                .font(.system(size: 12))
            HStack {
                Button("Apply CAD Scale Lock") {
                    let ratio = session.applyCadScalePreset(scaleSlug: scaleSlug, locked: lockScale, undoManager: undoManager)
                    statusNote = String(format: "Applied scale %@ (ratio 1:%.0f).", scaleSlug, ratio)
                }
                Spacer()
                Button("Export Binary .DWG…", action: onExportDwg)
                Button("Convert All CAD Layers → Visio Shapes") {
                    let count = session.convertCadLayersToVisioShapes(layerNameOrAll: "*", undoManager: undoManager)
                    statusNote = "Converted \(count) CAD entity shape(s) to unlocked Visio shapes."
                }
                .buttonStyle(.borderedProminent)
            }
            if !statusNote.isEmpty {
                Text(statusNote)
                    .font(.system(size: 11.5))
                    .foregroundStyle(.secondary)
            }
        }
        .padding(18)
        .frame(width: 560)
    }
}

private struct VexioSmartActionsSheet: View {
    @ObservedObject var session: VexioDocumentSession
    let onClose: () -> Void
    @Environment(\.undoManager) private var undoManager

    @State private var rowName: String = "ToggleState"
    @State private var menuLabel: String = "Toggle Active Color"
    @State private var actionFormula: String = "=SETF(GetRef(FillForegnd), \"RGB(34,197,94)\")"
    @State private var checkedFormula: String = "FALSE"
    @State private var disabledFormula: String = "FALSE"
    @State private var beginGroup: Bool = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label("Smart-Shape Context Menu Actions (Actions.*)", systemImage: "bolt.badge.checkmark")
                    .font(.system(size: 14, weight: .semibold))
                Spacer()
                Button("Close", action: onClose)
            }

            if !session.selectedShapeActions.isEmpty {
                ScrollView {
                    VStack(spacing: 6) {
                        ForEach(session.selectedShapeActions) { act in
                            HStack {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text("\(act.checked ? "✓ " : "")\(act.menu) (Actions.\(act.name))")
                                        .font(.system(size: 12, weight: .semibold))
                                    Text(act.actionFormula)
                                        .font(.system(size: 11, design: .monospaced))
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                                Button("Run") {
                                    session.executeSmartAction(rowName: act.name, undoManager: undoManager)
                                }
                                Button("Delete", role: .destructive) {
                                    session.deleteSmartAction(rowName: act.name, undoManager: undoManager)
                                }
                            }
                            .padding(8)
                            .background(RoundedRectangle(cornerRadius: 7).fill(Color.primary.opacity(0.05)))
                        }
                    }
                }
                .frame(maxHeight: 150)
            }

            Grid(alignment: .leading, horizontalSpacing: 10, verticalSpacing: 8) {
                GridRow {
                    Text("Row Name:").font(.system(size: 12))
                    TextField("ToggleState", text: $rowName).textFieldStyle(.roundedBorder)
                }
                GridRow {
                    Text("Menu Label:").font(.system(size: 12))
                    TextField("Toggle Active Color", text: $menuLabel).textFieldStyle(.roundedBorder)
                }
                GridRow {
                    Text("Action Formula:").font(.system(size: 12))
                    TextField("=SETF(...)", text: $actionFormula).textFieldStyle(.roundedBorder)
                }
                GridRow {
                    Text("Checked Expr:").font(.system(size: 12))
                    TextField("=User.IsActive", text: $checkedFormula).textFieldStyle(.roundedBorder)
                }
                GridRow {
                    Text("Disabled Expr:").font(.system(size: 12))
                    TextField("FALSE", text: $disabledFormula).textFieldStyle(.roundedBorder)
                }
            }

            Toggle("Begin Menu Separator Group", isOn: $beginGroup)
                .font(.system(size: 12))

            HStack {
                Spacer()
                Button("Save Smart Action Row") {
                    session.upsertSmartAction(
                        rowName: rowName,
                        menu: menuLabel,
                        actionFormula: actionFormula,
                        checkedFormula: checkedFormula,
                        disabledFormula: disabledFormula,
                        beginGroup: beginGroup,
                        undoManager: undoManager
                    )
                }
                .buttonStyle(.borderedProminent)
                .disabled(session.selectedShapeId == 0)
            }
        }
        .padding(18)
        .frame(width: 560)
    }
}

// MARK: - Milestone 3 (`v0.8.0-beta`) Sheets: Insert Field, Find & Replace, Snap & Glue, Number Shapes

private struct VexioInsertFieldSheet: View {
    @ObservedObject var session: VexioDocumentSession
    @Environment(\.undoManager) private var undoManager
    var onClose: () -> Void
    @State private var preset: String = "width"
    @State private var prefixLabel: String = ""
    @State private var formatSpec: String = "0.00 in"

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Insert Dynamic Text Field (<fld>)").font(.system(size: 14, weight: .bold))
                Spacer()
                Button("Close", action: onClose)
            }
            Picker("Field Formula", selection: $preset) {
                Text("Geometry — Width (=Width)").tag("width")
                Text("Geometry — Height (=Height)").tag("height")
                Text("Geometry — Angle (=Angle)").tag("angle")
                Text("Page — Page Name (=PAGENAME())").tag("page_name")
                Text("Page — Page Number (=PAGENUMBER())").tag("page_number")
                Text("Page — Page Count (=PAGECOUNT())").tag("page_count")
                Text("Document — Title (=TITLE())").tag("doc_title")
                Text("Document — Creator (=CREATOR())").tag("creator")
                Text("Object Info — Name (=NAME())").tag("shape_name")
            }
            TextField("Prefix Label (e.g. W: )", text: $prefixLabel).textFieldStyle(.roundedBorder)
            TextField("Display Format (e.g. 0.00 in)", text: $formatSpec).textFieldStyle(.roundedBorder)
            HStack {
                Spacer()
                Button("Insert Field") {
                    session.insertDynamicTextField(presetOrFormula: preset, formatSpec: formatSpec, prefixLabel: prefixLabel, undoManager: undoManager)
                    onClose()
                }
                .buttonStyle(.borderedProminent)
                .disabled(session.selectedShapeId == 0)
            }
        }
        .padding(18)
        .frame(width: 460)
    }
}

private struct VexioFindReplaceSheet: View {
    @ObservedObject var session: VexioDocumentSession
    @Environment(\.undoManager) private var undoManager
    var onClose: () -> Void
    @State private var query: String = ""
    @State private var replacement: String = ""
    @State private var matchCase: Bool = false
    @State private var wholeWord: Bool = false
    @State private var allPages: Bool = true
    @State private var summary: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Find & Replace (⌘F)").font(.system(size: 14, weight: .bold))
                Spacer()
                Button("Close", action: onClose)
            }
            TextField("Find text, Prop.*, or shape name", text: $query).textFieldStyle(.roundedBorder)
            TextField("Replace with", text: $replacement).textFieldStyle(.roundedBorder)
            HStack(spacing: 14) {
                Toggle("Match Case", isOn: $matchCase)
                Toggle("Whole Word", isOn: $wholeWord)
                Toggle("All Pages", isOn: $allPages)
            }
            if !summary.isEmpty {
                Text(summary).font(.system(size: 11.5)).foregroundStyle(.secondary)
            }
            HStack {
                Spacer()
                Button("Find All") {
                    let hits = session.searchDocumentMatches(query: query, matchCase: matchCase, wholeWord: wholeWord, allPages: allPages)
                    summary = "Found \(hits.count) match(es)."
                    if let first = hits.first, let sid = first["shapeId"] as? Int, sid > 0 {
                        if let pid = first["pageId"] as? Int, pid > 0 {
                            session.activePageId = UInt32(pid)
                        }
                        session.selectSingleShape(UInt64(sid))
                    }
                }
                Button("Replace All") {
                    let n = session.replaceDocumentMatches(findStr: query, replaceStr: replacement, matchCase: matchCase, wholeWord: wholeWord, allPages: allPages, undoManager: undoManager)
                    summary = "Replaced \(n) occurrence(s)."
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(18)
        .frame(width: 480)
    }
}

private struct VexioSnapGlueSheet: View {
    @ObservedObject var session: VexioDocumentSession
    @Environment(\.undoManager) private var undoManager
    var onClose: () -> Void
    @State private var snapEnabled: Bool = true
    @State private var glueEnabled: Bool = true
    @State private var snapGrid: Bool = true
    @State private var snapGuides: Bool = true
    @State private var snapGeom: Bool = true
    @State private var snapAlignBox: Bool = true
    @State private var snapPorts: Bool = true
    @State private var gluePorts: Bool = true
    @State private var glueGuides: Bool = true
    @State private var glueGeom: Bool = true

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Snap & Glue Matrix").font(.system(size: 14, weight: .bold))
                Spacer()
                Button("Close", action: onClose)
            }
            HStack(spacing: 18) {
                Toggle("Enable Snap", isOn: $snapEnabled)
                Toggle("Enable Glue", isOn: $glueEnabled)
            }
            Divider()
            HStack(alignment: .top, spacing: 24) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Snap To").font(.system(size: 12, weight: .semibold))
                    Toggle("Grid", isOn: $snapGrid)
                    Toggle("Ruler Guides", isOn: $snapGuides)
                    Toggle("Shape Geometry", isOn: $snapGeom)
                    Toggle("Alignment Box", isOn: $snapAlignBox)
                    Toggle("Connection Points", isOn: $snapPorts)
                }
                VStack(alignment: .leading, spacing: 6) {
                    Text("Glue To").font(.system(size: 12, weight: .semibold))
                    Toggle("Connection Points", isOn: $gluePorts)
                    Toggle("Ruler Guides", isOn: $glueGuides)
                    Toggle("Shape Geometry", isOn: $glueGeom)
                }
            }
            HStack {
                Spacer()
                Button("Apply") {
                    var sMask: UInt32 = 0
                    if snapGrid { sMask |= 1 }
                    if snapGuides { sMask |= 4 }
                    if snapGeom { sMask |= 8 }
                    if snapAlignBox { sMask |= 16 }
                    if snapPorts { sMask |= 64 }
                    var gMask: UInt32 = 0
                    if gluePorts { gMask |= 1 }
                    if glueGuides { gMask |= 2 }
                    if glueGeom { gMask |= 4 }
                    session.applySnapGlueSettings(snapEnabled: snapEnabled, glueEnabled: glueEnabled, snapTargetsMask: sMask, glueTypesMask: gMask, undoManager: undoManager)
                    onClose()
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(18)
        .frame(width: 460)
        .onAppear {
            let (se, ge, st, gt) = session.readSnapGlueTuple()
            snapEnabled = se
            glueEnabled = ge
            snapGrid = (st & 1) != 0
            snapGuides = (st & 4) != 0
            snapGeom = (st & 8) != 0
            snapAlignBox = (st & 16) != 0
            snapPorts = (st & 64) != 0
            gluePorts = (gt & 1) != 0
            glueGuides = (gt & 2) != 0
            glueGeom = (gt & 4) != 0
        }
    }
}

private struct VexioNumberShapesSheet: View {
    @ObservedObject var session: VexioDocumentSession
    @Environment(\.undoManager) private var undoManager
    var onClose: () -> Void
    @State private var orderMode: String = "top_to_bottom_left_to_right"
    @State private var startNum: Int = 1
    @State private var interval: Int = 1
    @State private var prefix: String = ""
    @State private var targetMode: String = "both"

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Number Shapes Wizard").font(.system(size: 14, weight: .bold))
                Spacer()
                Button("Close", action: onClose)
            }
            Picker("Order", selection: $orderMode) {
                Text("Top-to-Bottom, Left-to-Right").tag("top_to_bottom_left_to_right")
                Text("Left-to-Right, Top-to-Bottom").tag("left_to_right_top_to_bottom")
                Text("Connected Flow (BFS)").tag("connected_flow")
                Text("Z-Order").tag("z_order")
            }
            Stepper("Start Number: \(startNum)", value: $startNum, in: 0...9999)
            Stepper("Interval: \(interval)", value: $interval, in: 1...100)
            TextField("Prefix (optional, e.g. P-)", text: $prefix).textFieldStyle(.roundedBorder)
            Picker("Output Target", selection: $targetMode) {
                Text("Shape Text + Prop.ShapeNumber").tag("both")
                Text("Shape Text Only").tag("text_prefix")
                Text("Prop.ShapeNumber Only").tag("shape_data")
            }
            HStack {
                Spacer()
                Button("Apply Numbering") {
                    session.numberShapesOnActivePage(orderMode: orderMode, startNum: Int32(startNum), interval: Int32(interval), prefix: prefix, targetMode: targetMode, undoManager: undoManager)
                    onClose()
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(18)
        .frame(width: 440)
    }
}
