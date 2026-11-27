import AppKit
import SwiftUI

// MARK: - Chrome-Style Hover-Switching Menu Coordinator (Fixes Remark #12 / Screenshot 3)

@MainActor
public final class HoverSwitchMenuCoordinator: ObservableObject {
    @Published public var activeMenuID: String? = nil
    public init() {}
}

public struct HoverSwitchMenuButton<MenuContent: View>: View {
    public let id: String
    public let title: String
    public let systemImage: String
    public let tooltip: String
    @ObservedObject public var coordinator: HoverSwitchMenuCoordinator
    @ViewBuilder public let content: () -> MenuContent

    public init(
        id: String,
        title: String,
        systemImage: String,
        tooltip: String = "",
        coordinator: HoverSwitchMenuCoordinator,
        @ViewBuilder content: @escaping () -> MenuContent
    ) {
        self.id = id
        self.title = title
        self.systemImage = systemImage
        self.tooltip = tooltip
        self.coordinator = coordinator
        self.content = content
    }

    private var isOpen: Bool {
        coordinator.activeMenuID == id
    }

    public var body: some View {
        Button {
            if isOpen {
                coordinator.activeMenuID = nil
            } else {
                coordinator.activeMenuID = id
            }
        } label: {
            HStack(spacing: 4) {
                if !systemImage.isEmpty {
                    Image(systemName: systemImage)
                        .font(.system(size: 11.5, weight: .medium))
                }
                Text(title)
                    .font(.system(size: 11.5, weight: .medium))
                    .lineLimit(1)
                    .fixedSize(horizontal: true, vertical: false)
                Image(systemName: "chevron.down")
                    .font(.system(size: 8.5, weight: .bold))
                    .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4.5)
            .background(
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(isOpen ? Color.accentColor.opacity(0.18) : Color.primary.opacity(0.04))
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(isOpen ? "" : (tooltip.isEmpty ? title : tooltip))
        .onHover { hovering in
            // Chrome / native macOS menu bar behavior: if ANY sibling menu is currently open,
            // hovering onto another menu button immediately switches the open menu!
            if hovering, let openID = coordinator.activeMenuID, openID != id {
                coordinator.activeMenuID = id
            }
        }
        .popover(
            isPresented: Binding(
                get: { coordinator.activeMenuID == id },
                set: { shown in
                    if !shown && coordinator.activeMenuID == id {
                        coordinator.activeMenuID = nil
                    }
                }
            ),
            arrowEdge: .bottom
        ) {
            VStack(alignment: .leading, spacing: 2) {
                content()
            }
            .padding(8)
            .frame(minWidth: 205)
        }
    }
}

// MARK: - AppKit Auto-Select-All TextField for Double-Click Filename, Page & Inline Label Editing

public struct SelectAllInlineTextField: NSViewRepresentable {
    @Binding public var text: String
    public var placeholder: String
    public var fontFamily: String
    public var fontSize: CGFloat
    public var fontWeight: NSFont.Weight
    public var alignment: NSTextAlignment
    public var isCanvasInPlace: Bool
    public var onCommit: () -> Void
    public var onCancel: () -> Void

    public init(
        text: Binding<String>,
        placeholder: String = "",
        fontFamily: String = "",
        fontSize: CGFloat = 12.5,
        fontWeight: NSFont.Weight = .semibold,
        alignment: NSTextAlignment = .left,
        isCanvasInPlace: Bool = false,
        onCommit: @escaping () -> Void,
        onCancel: @escaping () -> Void
    ) {
        self._text = text
        self.placeholder = placeholder
        self.fontFamily = fontFamily
        self.fontSize = fontSize
        self.fontWeight = fontWeight
        self.alignment = alignment
        self.isCanvasInPlace = isCanvasInPlace
        self.onCommit = onCommit
        self.onCancel = onCancel
    }

    private func resolvedNSFont() -> NSFont {
        if !fontFamily.isEmpty, let custom = NSFont(name: fontFamily, size: fontSize) {
            return custom
        }
        return NSFont.systemFont(ofSize: fontSize, weight: fontWeight)
    }

    public func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    public func makeNSView(context: Context) -> NSTextField {
        let tf = NSTextField(string: text)
        tf.placeholderString = placeholder
        tf.font = resolvedNSFont()
        tf.alignment = alignment
        if isCanvasInPlace {
            tf.isBordered = false
            tf.drawsBackground = true
            tf.backgroundColor = NSColor.textBackgroundColor
            tf.focusRingType = .none
        } else {
            tf.isBordered = true
            tf.bezelStyle = .roundedBezel
            tf.focusRingType = .exterior
        }
        tf.delegate = context.coordinator
        DispatchQueue.main.async {
            tf.window?.makeFirstResponder(tf)
            tf.currentEditor()?.selectAll(nil)
        }
        return tf
    }

    public func updateNSView(_ nsView: NSTextField, context: Context) {
        nsView.alignment = alignment
        nsView.font = resolvedNSFont()
        if nsView.stringValue != text {
            nsView.stringValue = text
        }
    }

    public final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: SelectAllInlineTextField
        private var didFinish = false

        init(parent: SelectAllInlineTextField) {
            self.parent = parent
        }

        public func controlTextDidChange(_ obj: Notification) {
            guard let tf = obj.object as? NSTextField else { return }
            parent.text = tf.stringValue
        }

        public func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                didFinish = true
                parent.onCommit()
                return true
            } else if commandSelector == #selector(NSResponder.cancelOperation(_:)) {
                didFinish = true
                parent.onCancel()
                return true
            }
            return false
        }

        public func controlTextDidEndEditing(_ obj: Notification) {
            if !didFinish {
                didFinish = true
                parent.onCommit()
            }
        }
    }
}

// MARK: - Classic Visio Ribbon Tabs

public enum VexioRibbonTab: String, CaseIterable, Identifiable, Sendable {
    case home
    case insert
    case design
    case data
    case process
    case review
    case view
    case developer

    public var id: String { rawValue }
}

// MARK: - Unified Clean Top Header + Collapsible Visio Ribbon Strip (Fixes Remarks #3, #5, #6, #10-#14)

public struct VexioHeaderAndRibbonBar: View {
    @ObservedObject public var session: VexioDocumentSession
    @ObservedObject public var i18n: VexioLocalization
    @ObservedObject public var updater: VexioUpdateManager
    @Binding public var isBackstagePresented: Bool

    @Environment(\.undoManager) private var undoManager
    @AppStorage("vexio.appearanceMode") private var appearanceMode: String = "light"
    @AppStorage("vexio.ribbonCollapsed") private var isRibbonCollapsed: Bool = false

    @StateObject private var menuCoordinator = HoverSwitchMenuCoordinator()
    @State private var activeRibbonTab: VexioRibbonTab = .home
    @State private var isEditingTitle: Bool = false
    @State private var titleDraft: String = ""

    public let onRequestOpenFile: () -> Void
    public let onRequestSaveFile: () -> Void
    public let onRequestSaveAsFile: () -> Void
    public let onRequestImportStencil: () -> Void
    public let onRequestExport: (VexioExportFormat) -> Void
    public let onRequestDataVisualizerExport: () -> Void
    public let onRequestOffscreenPreview: () -> Void

    private let fontSizes: [Double] = [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72]

    private var availableFontFamilies: [String] {
        var list = VexioDocumentSession.installedSystemFontFamilies
        if !session.selectedFontFamily.isEmpty && !list.contains(session.selectedFontFamily) {
            list.insert(session.selectedFontFamily, at: 0)
        }
        return list
    }

    private var resolvedFontSizes: [Double] {
        if fontSizes.contains(session.selectedFontSizePt) {
            return fontSizes
        }
        return (fontSizes + [session.selectedFontSizePt]).sorted()
    }

    private var isDarkMode: Bool {
        appearanceMode == "dark"
    }

    public var body: some View {
        VStack(spacing: 4) {
            topTitleAndTabStrip
                .frame(minHeight: 30)
            if !isRibbonCollapsed {
                ribbonBodyPanel
            }
        }
        .padding(.horizontal, 12)
        .padding(.top, 7)
        .padding(.bottom, 6)
        .background(.thinMaterial)
    }

    // MARK: - Row 1: Unified Window Header (Inline Traffic Lights) + Save / Save As + Editable Title + Ribbon Tabs

    private var topTitleAndTabStrip: some View {
        HStack(spacing: 8) {
            // Reserve 74pt inline for native macOS window traffic lights inside flush top bar so traffic lights never overlap any border!
            Spacer().frame(width: 74)

            // Left: Stencil toggle + Visio "File" Backstage button + Save / Save As / Undo / Redo + Editable Document Name
            HStack(spacing: 5) {
                LiquidGlassIconButton(
                    systemImage: "sidebar.left",
                    title: "Toggle Stencil Navigator (⌥⌘1)",
                    isActive: session.showStencilNavigator
                ) {
                    withAnimation(LiquidGlassPhysics.standardSpring) {
                        session.showStencilNavigator.toggle()
                    }
                }

                Button {
                    withAnimation(LiquidGlassPhysics.standardSpring) {
                        isBackstagePresented = true
                    }
                } label: {
                    Text(i18n.t("file"))
                        .font(.system(size: 12, weight: .bold))
                        .lineLimit(1)
                        .fixedSize(horizontal: true, vertical: false)
                        .foregroundStyle(.white)
                        .padding(.horizontal, 11)
                        .padding(.vertical, 4.5)
                        .background(Capsule().fill(Color.accentColor))
                }
                .buttonStyle(.plain)
                .help("\(i18n.t("file")) — New, Open, Save, Templates & Settings")

                LiquidGlassIconButton(systemImage: "square.and.arrow.down", title: "\(i18n.t("saveFile")) (⌘S)") {
                    onRequestSaveFile()
                }
                LiquidGlassIconButton(systemImage: "square.and.arrow.down.on.square", title: "\(i18n.t("saveAs")) (⇧⌘S)") {
                    onRequestSaveAsFile()
                }
                LiquidGlassIconButton(systemImage: "arrow.uturn.backward", title: "Undo (⌘Z)") {
                    session.performUndo(undoManager: undoManager)
                }
                LiquidGlassIconButton(systemImage: "arrow.uturn.forward", title: "Redo (⇧⌘Z)") {
                    session.performRedo(undoManager: undoManager)
                }

                Divider().frame(height: 16)

                // Double-click filename inline rename
                if isEditingTitle {
                    SelectAllInlineTextField(
                        text: $titleDraft,
                        placeholder: "Untitled Diagram",
                        fontSize: 12.5,
                        fontWeight: .semibold,
                        onCommit: {
                            session.renameDocument(to: titleDraft)
                            isEditingTitle = false
                        },
                        onCancel: {
                            isEditingTitle = false
                        }
                    )
                    .frame(width: 175, height: 22)
                } else {
                    VStack(alignment: .leading, spacing: 0.5) {
                        HStack(spacing: 5) {
                            Text(session.documentTitle)
                                .font(.system(size: 12.5, weight: .semibold))
                                .lineLimit(1)
                            Text(".\(session.sourceFormatExtension.uppercased())")
                                .font(.system(size: 9, weight: .bold, design: .monospaced))
                                .padding(.horizontal, 4.5)
                                .padding(.vertical, 1)
                                .background(Capsule().fill(Color.accentColor.opacity(0.16)))
                                .fixedSize()
                            if session.hasUnsavedChanges {
                                Text("• \(i18n.t("editedBadge"))")
                                    .font(.system(size: 9.5, weight: .medium))
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                                    .fixedSize()
                            }
                        }
                        Text(session.statusMessage)
                            .font(.system(size: 9.5))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                    .frame(maxWidth: 190, alignment: .leading)
                    .contentShape(Rectangle())
                    .help(i18n.t("doubleClickRenameHint"))
                    .onTapGesture(count: 2) {
                        titleDraft = session.documentTitle
                        isEditingTitle = true
                    }
                }
            }

            Spacer(minLength: 4)

            // Center: Non-Truncating Visio Ribbon Tab Strip
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 3) {
                    ForEach(VexioRibbonTab.allCases) { tab in
                        let isSelected = (activeRibbonTab == tab && !isRibbonCollapsed)
                        Button {
                            withAnimation(LiquidGlassPhysics.standardSpring) {
                                if activeRibbonTab == tab {
                                    isRibbonCollapsed.toggle()
                                } else {
                                    activeRibbonTab = tab
                                    isRibbonCollapsed = false
                                }
                            }
                        } label: {
                            Text(i18n.t(tab.rawValue))
                                .font(.system(size: 11.5, weight: isSelected ? .bold : .medium))
                                .lineLimit(1)
                                .fixedSize(horizontal: true, vertical: false)
                                .foregroundStyle(isSelected ? Color.accentColor : Color.primary)
                                .padding(.horizontal, 8.5)
                                .padding(.vertical, 4.5)
                                .background(
                                    RoundedRectangle(cornerRadius: 7, style: .continuous)
                                        .fill(isSelected ? Color.accentColor.opacity(0.16) : Color.clear)
                                )
                        }
                        .buttonStyle(.plain)
                        .help("\(i18n.t(tab.rawValue)) Ribbon Tab (click active tab to collapse/expand)")
                    }

                    Button {
                        withAnimation(LiquidGlassPhysics.standardSpring) {
                            isRibbonCollapsed.toggle()
                        }
                    } label: {
                        Image(systemName: isRibbonCollapsed ? "chevron.down" : "chevron.up")
                            .font(.system(size: 10, weight: .bold))
                            .foregroundStyle(.secondary)
                            .padding(5)
                    }
                    .buttonStyle(.plain)
                    .help(isRibbonCollapsed ? "Expand Visio Ribbon" : "Collapse Visio Ribbon")
                }
            }

            Spacer(minLength: 4)

            // Right: Hover-Switch Menus (Language, Units, Export) + Update + Theme + Panel Toggles
            HStack(spacing: 5) {
                HoverSwitchMenuButton(
                    id: "lang",
                    title: i18n.language.badgeCode,
                    systemImage: "globe",
                    coordinator: menuCoordinator
                ) {
                    ForEach(VexioAppLanguage.allCases) { lang in
                        Button {
                            i18n.language = lang
                            menuCoordinator.activeMenuID = nil
                        } label: {
                            HStack {
                                Text(lang.nativeDisplayName)
                                Spacer()
                                if i18n.language == lang {
                                    Image(systemName: "checkmark")
                                }
                            }
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 3)
                    }
                }

                HoverSwitchMenuButton(
                    id: "units",
                    title: i18n.unitPreference.resolvedEngineSlug,
                    systemImage: "ruler",
                    coordinator: menuCoordinator
                ) {
                    ForEach(VexioMeasurementUnit.allCases) { u in
                        Button {
                            i18n.unitPreference = u
                            session.applyMeasurementUnit(u.resolvedEngineSlug, undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        } label: {
                            HStack {
                                Text(u == .osDefault ? "\(i18n.t("osStandard")) (\(u.resolvedEngineSlug))" : u.rawValue.uppercased())
                                Spacer()
                                if i18n.unitPreference == u {
                                    Image(systemName: "checkmark")
                                }
                            }
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 3)
                    }
                }

                HoverSwitchMenuButton(
                    id: "export",
                    title: i18n.t("export"),
                    systemImage: "square.and.arrow.up",
                    coordinator: menuCoordinator
                ) {
                    ForEach(VexioExportFormat.allCases) { fmt in
                        Button(fmt.displayName) {
                            menuCoordinator.activeMenuID = nil
                            onRequestExport(fmt)
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2.5)
                    }
                    Divider()
                    Button("Data Visualizer (.xlsx)…") {
                        menuCoordinator.activeMenuID = nil
                        onRequestDataVisualizerExport()
                    }
                    .buttonStyle(.plain)
                    .padding(.vertical, 2.5)
                }

                LiquidGlassIconButton(
                    systemImage: updater.updateAvailable ? "arrow.down.circle.fill" : "arrow.triangle.2.circlepath",
                    title: i18n.t("checkForUpdates"),
                    isActive: updater.updateAvailable
                ) {
                    Task { await updater.checkForUpdates(userInitiated: true) }
                }

                LiquidGlassIconButton(
                    systemImage: isDarkMode ? "sun.max.fill" : "moon.stars",
                    title: isDarkMode ? "Switch to Light Mode" : "Switch to Dark Mode",
                    isActive: !isDarkMode
                ) {
                    let next = isDarkMode ? "light" : "dark"
                    appearanceMode = next
                    NSApp.appearance = NSAppearance(named: next == "dark" ? .darkAqua : .aqua)
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
            .fixedSize(horizontal: true, vertical: false)
        }
    }

    // MARK: - Row 2: Classic Multi-Tab Visio Ribbon Body

    @ViewBuilder
    private var ribbonBodyPanel: some View {
        VStack(spacing: 0) {
            Divider().padding(.bottom, 5)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 12) {
                    switch activeRibbonTab {
                    case .home:
                        homeRibbonGroups
                    case .insert:
                        insertRibbonGroups
                    case .design:
                        designRibbonGroups
                    case .data:
                        dataRibbonGroups
                    case .process:
                        processRibbonGroups
                    case .review:
                        reviewRibbonGroups
                    case .view:
                        viewRibbonGroups
                    case .developer:
                        developerRibbonGroups
                    }
                }
                .padding(.horizontal, 4)
                .padding(.vertical, 2)
            }
        }
    }

    // 1. HOME RIBBON: Clipboard | Native OS Font & Native OS ColorPickers | Paragraph | Shape Styles | Tools | Arrange
    private var homeRibbonGroups: some View {
        HStack(spacing: 10) {
            ribbonGroup(title: i18n.t("clipboard")) {
                HStack(spacing: 4) {
                    ribbonCompactButton("doc.on.clipboard", "Paste") {
                        session.pasteClipboardAt(canvasInches: CGPoint(x: 4.5, y: 4.5), undoManager: undoManager)
                    }
                    ribbonCompactButton("doc.on.doc", "Copy") {
                        session.copySelectionToClipboard()
                    }
                    ribbonCompactButton("plus.square.on.square", "Dup") {
                        session.duplicateSelectedShapes(undoManager: undoManager)
                    }
                    ribbonCompactButton("trash", "Del") {
                        session.deleteSelectedShapes(undoManager: undoManager)
                    }
                }
            }

            Divider().frame(height: 44)

            // Native OS Installed Fonts + Visio Headings + Native macOS ColorPicker (NSColorPanel)
            ribbonGroup(title: i18n.t("font")) {
                HStack(spacing: 5) {
                    HoverSwitchMenuButton(
                        id: "home.headings",
                        title: i18n.t("headings"),
                        systemImage: "textformat.size",
                        tooltip: "Apply Visio Title / Heading style to selection (or insert onto page)",
                        coordinator: menuCoordinator
                    ) {
                        ForEach(VexioTextHeadingPreset.allCases) { preset in
                            Button(preset.displayName) {
                                session.applyOrInsertHeading(preset, forceInsert: false, undoManager: undoManager)
                                menuCoordinator.activeMenuID = nil
                            }
                            .buttonStyle(.plain)
                            .padding(.vertical, 2.5)
                        }
                    }

                    Picker("", selection: Binding(
                        get: { session.selectedFontFamily },
                        set: { session.applyTextStyle(fontFamily: $0, undoManager: undoManager) }
                    )) {
                        ForEach(availableFontFamilies, id: \.self) { f in
                            Text(f).tag(f)
                        }
                    }
                    .labelsHidden()
                    .frame(width: 148)
                    .controlSize(.small)
                    .help("Installed macOS System Fonts")

                    LiquidGlassIconButton(systemImage: "textformat", title: "\(i18n.t("fontPanel")) (⌘T)") {
                        NSFontManager.shared.orderFrontFontPanel(nil)
                    }

                    Picker("", selection: Binding(
                        get: { session.selectedFontSizePt },
                        set: { session.applyTextStyle(fontSizePt: $0, undoManager: undoManager) }
                    )) {
                        ForEach(resolvedFontSizes, id: \.self) { sz in
                            Text("\(Int(sz.rounded())) pt").tag(sz)
                        }
                    }
                    .labelsHidden()
                    .frame(width: 70)
                    .controlSize(.small)

                    LiquidGlassIconButton(systemImage: "textformat.size.larger", title: "Grow Font (+2 pt)") {
                        session.applyTextStyle(sizeDeltaPt: 2.0, undoManager: undoManager)
                    }
                    LiquidGlassIconButton(systemImage: "textformat.size.smaller", title: "Shrink Font (-2 pt)") {
                        session.applyTextStyle(sizeDeltaPt: -2.0, undoManager: undoManager)
                    }

                    Divider().frame(height: 20)

                    styleToggleButton("B", bold: true, active: (session.selectedCharStyle & 1) != 0, tip: "Bold (⌘B)") {
                        session.applyTextStyle(toggleStyleMask: 1, undoManager: undoManager)
                    }
                    styleToggleButton("I", italic: true, active: (session.selectedCharStyle & 2) != 0, tip: "Italic (⌘I)") {
                        session.applyTextStyle(toggleStyleMask: 2, undoManager: undoManager)
                    }
                    styleToggleButton("U", underline: true, active: (session.selectedCharStyle & 4) != 0, tip: "Underline (⌘U)") {
                        session.applyTextStyle(toggleStyleMask: 4, undoManager: undoManager)
                    }
                    styleToggleButton("S", strike: true, active: (session.selectedCharStyle & 8) != 0, tip: "Strikethrough") {
                        session.applyTextStyle(toggleStyleMask: 8, undoManager: undoManager)
                    }

                    Divider().frame(height: 20)

                    // Native macOS NSColorPanel Picker for Text Color
                    HStack(spacing: 3) {
                        Image(systemName: "textformat.abc")
                            .font(.system(size: 10.5, weight: .semibold))
                            .foregroundStyle(.secondary)
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
                        .help("Text Color (Native macOS Color Picker)")
                    }

                    // Native macOS NSColorPanel Picker for Highlight + Clear Button
                    HStack(spacing: 3) {
                        Image(systemName: "highlighter")
                            .font(.system(size: 10.5, weight: .semibold))
                            .foregroundStyle(.secondary)
                        ColorPicker(
                            "",
                            selection: Binding(
                                get: {
                                    session.selectedTextBgColorHex == "none"
                                        ? Color(vexioHex: "#FEF08A")
                                        : Color(vexioHex: session.selectedTextBgColorHex)
                                },
                                set: { c in
                                    let hex = c.vexioHexRGBString
                                    session.selectedTextBgColorHex = hex
                                    session.applyTextStyle(textBgColorHex: hex, undoManager: undoManager)
                                }
                            ),
                            supportsOpacity: false
                        )
                        .labelsHidden()
                        .help("Text Highlight Color (Native macOS Color Picker)")

                        if session.selectedTextBgColorHex != "none" {
                            LiquidGlassIconButton(systemImage: "xmark.circle", title: i18n.t("clearHighlight")) {
                                session.selectedTextBgColorHex = "none"
                                session.applyTextStyle(textBgColorHex: "none", undoManager: undoManager)
                            }
                        }
                    }
                }
            }

            Divider().frame(height: 44)

            // Shape Fill & Line Native macOS ColorPickers
            ribbonGroup(title: i18n.t("shapeStyles")) {
                HStack(spacing: 6) {
                    HStack(spacing: 3) {
                        Text(i18n.t("fillColor"))
                            .font(.system(size: 10.5, weight: .medium))
                            .lineLimit(1)
                            .fixedSize()
                        ColorPicker(
                            "",
                            selection: Binding(
                                get: { Color(vexioHex: session.selectedFillColorHex) },
                                set: { session.applyFillColor($0, undoManager: undoManager) }
                            ),
                            supportsOpacity: false
                        )
                        .labelsHidden()
                        .help("Shape Fill Color (Native macOS Color Picker)")
                    }

                    HStack(spacing: 3) {
                        Text(i18n.t("lineColor"))
                            .font(.system(size: 10.5, weight: .medium))
                            .lineLimit(1)
                            .fixedSize()
                        ColorPicker(
                            "",
                            selection: Binding(
                                get: { Color(vexioHex: session.selectedLineColorHex) },
                                set: { session.applyLineColor($0, undoManager: undoManager) }
                            ),
                            supportsOpacity: false
                        )
                        .labelsHidden()
                        .help("Line / Border Color (Native macOS Color Picker)")
                    }
                }
            }

            Divider().frame(height: 44)

            // Paragraph Alignment & Bullets Group
            ribbonGroup(title: i18n.t("paragraph")) {
                HStack(spacing: 4) {
                    LiquidGlassIconButton(systemImage: "text.alignleft", title: "Align Left", isActive: session.selectedHorizAlign == 0) {
                        session.applyTextStyle(horizAlign: 0, undoManager: undoManager)
                    }
                    LiquidGlassIconButton(systemImage: "text.aligncenter", title: "Align Center", isActive: session.selectedHorizAlign == 1) {
                        session.applyTextStyle(horizAlign: 1, undoManager: undoManager)
                    }
                    LiquidGlassIconButton(systemImage: "text.alignright", title: "Align Right", isActive: session.selectedHorizAlign == 2) {
                        session.applyTextStyle(horizAlign: 2, undoManager: undoManager)
                    }

                    Divider().frame(height: 18)

                    LiquidGlassIconButton(systemImage: "align.vertical.top", title: "Top Align", isActive: session.selectedVertAlign == 0) {
                        session.applyTextStyle(vertAlign: 0, undoManager: undoManager)
                    }
                    LiquidGlassIconButton(systemImage: "align.vertical.center", title: "Middle Align", isActive: session.selectedVertAlign == 1) {
                        session.applyTextStyle(vertAlign: 1, undoManager: undoManager)
                    }
                    LiquidGlassIconButton(systemImage: "align.vertical.bottom", title: "Bottom Align", isActive: session.selectedVertAlign == 2) {
                        session.applyTextStyle(vertAlign: 2, undoManager: undoManager)
                    }

                    Divider().frame(height: 18)

                    LiquidGlassIconButton(systemImage: "list.bullet", title: "Bullet List", isActive: session.selectedBulletStyle == 1) {
                        let next: UInt8 = session.selectedBulletStyle == 1 ? 0 : 1
                        session.applyTextStyle(bulletStyle: next, undoManager: undoManager)
                    }
                    LiquidGlassIconButton(systemImage: "list.number", title: "Numbered List", isActive: session.selectedBulletStyle == 2) {
                        let next: UInt8 = session.selectedBulletStyle == 2 ? 0 : 2
                        session.applyTextStyle(bulletStyle: next, undoManager: undoManager)
                    }
                }
            }

            Divider().frame(height: 44)

            // Tools Group
            ribbonGroup(title: i18n.t("tools")) {
                HStack(spacing: 4) {
                    LiquidGlassIconButton(systemImage: "cursorarrow", title: "Select (⌘1)", isActive: session.activeTool == .pointer) {
                        session.activeTool = .pointer
                    }
                    LiquidGlassIconButton(systemImage: "point.topleft.down.curvedto.point.bottomright.up", title: "Connector (⌘2)", isActive: session.activeTool == .connector) {
                        session.activeTool = .connector
                    }
                    LiquidGlassIconButton(systemImage: "character.textbox", title: "Text Tool (⌘3)", isActive: session.activeTool == .text) {
                        session.activeTool = .text
                    }
                    LiquidGlassIconButton(systemImage: "rectangle", title: "Rectangle (⌘4)", isActive: session.activeTool == .rectangle) {
                        session.activeTool = .rectangle
                    }
                    LiquidGlassIconButton(systemImage: "oval", title: "Ellipse (⌘5)", isActive: session.activeTool == .ellipse) {
                        session.activeTool = .ellipse
                    }
                }
            }

            Divider().frame(height: 44)

            // Arrange, Align, Distribute & Group for ALL Stencils
            ribbonGroup(title: i18n.t("arrange")) {
                HStack(spacing: 4) {
                    HoverSwitchMenuButton(
                        id: "home.align",
                        title: "Align",
                        systemImage: "align.horizontal.left",
                        coordinator: menuCoordinator
                    ) {
                        Button("Align Left") {
                            session.alignSelectedShapes(mode: "left", undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2)
                        Button("Align Center") {
                            session.alignSelectedShapes(mode: "center", undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2)
                        Button("Align Right") {
                            session.alignSelectedShapes(mode: "right", undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2)
                        Divider()
                        Button("Align Top") {
                            session.alignSelectedShapes(mode: "top", undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2)
                        Button("Align Middle") {
                            session.alignSelectedShapes(mode: "middle", undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2)
                        Button("Align Bottom") {
                            session.alignSelectedShapes(mode: "bottom", undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2)
                    }

                    HoverSwitchMenuButton(
                        id: "home.distribute",
                        title: "Distribute",
                        systemImage: "distribute.horizontal",
                        coordinator: menuCoordinator
                    ) {
                        Button("Distribute Horizontally") {
                            session.distributeSelectedShapes(axis: "horizontal", undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2)
                        Button("Distribute Vertically") {
                            session.distributeSelectedShapes(axis: "vertical", undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2)
                    }

                    ribbonCompactButton("square.3.layers.3d", "Group", tip: "Group Selected Shapes (⌘G)") {
                        session.groupSelectedShapes(undoManager: undoManager)
                    }
                    .disabled(!session.canGroupSelectedShapes)
                    ribbonCompactButton("square.stack.3d.up.slash", "Ungroup", tip: "Ungroup Selected Group (⇧⌘G)") {
                        session.ungroupSelectedShapes(undoManager: undoManager)
                    }
                    .disabled(!session.canUngroupSelectedShapes)

                    ribbonCompactButton("rotate.left", "↺ 90°", tip: i18n.t("rotateLeft")) {
                        session.rotateOrFlipSelectedShapes(deltaDegrees: 90.0, undoManager: undoManager)
                    }
                    ribbonCompactButton("rotate.right", "↻ 90°", tip: i18n.t("rotateRight")) {
                        session.rotateOrFlipSelectedShapes(deltaDegrees: -90.0, undoManager: undoManager)
                    }
                    LiquidGlassIconButton(systemImage: "arrow.left.and.right.righttriangle.left.righttriangle.right", title: i18n.t("flipH")) {
                        session.rotateOrFlipSelectedShapes(toggleFlipX: true, undoManager: undoManager)
                    }
                    LiquidGlassIconButton(systemImage: "arrow.up.and.down.righttriangle.up.righttriangle.down", title: i18n.t("flipV")) {
                        session.rotateOrFlipSelectedShapes(toggleFlipY: true, undoManager: undoManager)
                    }
                    LiquidGlassIconButton(systemImage: "square.2.layers.3d.top.filled", title: "Bring to Front") {
                        session.reorderSelectedShapes(mode: "front", undoManager: undoManager)
                    }
                    LiquidGlassIconButton(systemImage: "square.2.layers.3d.bottom.filled", title: "Send to Back") {
                        session.reorderSelectedShapes(mode: "back", undoManager: undoManager)
                    }
                }
            }
        }
    }

    // 2. INSERT RIBBON: Pages | Stencils & Catalog | Objects
    private var insertRibbonGroups: some View {
        HStack(spacing: 12) {
            ribbonGroup(title: "Pages") {
                HStack(spacing: 5) {
                    ribbonCompactButton("doc.badge.plus", "New Page") {
                        session.addNewPage()
                    }
                    ribbonCompactButton("doc.badge.gearshape", i18n.t("pageSetup")) {
                        NotificationCenter.default.post(name: .vexioOpenPageSetupSheet, object: session.activePageId)
                    }
                }
            }
            Divider().frame(height: 44)
            ribbonGroup(title: i18n.t("stencils")) {
                HStack(spacing: 5) {
                    ribbonCompactButton("icloud.and.arrow.down", i18n.t("stencilCatalog")) {
                        NotificationCenter.default.post(name: .vexioOpenStencilCatalogSheet, object: nil)
                    }
                    ribbonCompactButton("square.grid.3x1.folder.badge.plus", i18n.t("importStencil")) {
                        onRequestImportStencil()
                    }
                }
            }
            Divider().frame(height: 44)
            ribbonGroup(title: "Diagram Objects") {
                HStack(spacing: 6) {
                    HoverSwitchMenuButton(
                        id: "insert.headings",
                        title: i18n.t("insertTitleHeading"),
                        systemImage: "textformat.alt",
                        tooltip: "Insert a Visio Title, Subtitle, Heading (H1–H3), or Section Banner",
                        coordinator: menuCoordinator
                    ) {
                        ForEach([
                            VexioTextHeadingPreset.title,
                            .subtitle,
                            .h1,
                            .h2,
                            .h3,
                            .sectionBanner,
                            .calloutTitle,
                            .caption,
                        ]) { preset in
                            Button("Insert \(preset.displayName)") {
                                session.applyOrInsertHeading(preset, forceInsert: true, undoManager: undoManager)
                                menuCoordinator.activeMenuID = nil
                            }
                            .buttonStyle(.plain)
                            .padding(.vertical, 2.5)
                        }
                    }

                    ribbonCompactButton("character.textbox", "Insert Text Box") {
                        session.insertTextBoxAndBeginEditing(atCanvasInches: CGPoint(x: 4.5, y: 4.5), undoManager: undoManager)
                    }
                    ribbonCompactButton("macwindow", "Container") {
                        session.insertContainerBox(undoManager: undoManager)
                    }
                    ribbonCompactButton("rectangle.split.1x2", "Swimlanes") {
                        session.insertSwimlanePool(undoManager: undoManager)
                    }
                    ribbonCompactButton("bubble.left", "Callout") {
                        session.attachCalloutNote(undoManager: undoManager)
                    }
                    ribbonCompactButton("rectangle.3.group", "Subprocess") {
                        session.createSubprocessDrilldown(undoManager: undoManager)
                    }
                    ribbonCompactButton("link", "Hyperlink…", tip: "Attach External URL, Page Link & Hover ScreenTip (⌘K)") {
                        NotificationCenter.default.post(name: .vexioOpenHyperlinkSheet, object: nil)
                    }
                    ribbonCompactButton("wand.and.stars", "Wizards…", tip: "SQL ⇄ ERD, Gantt, Timeline, PERT/CPM, Org Chart, 42U Rack & Room Wizards") {
                        NotificationCenter.default.post(name: .vexioOpenDomainWizardsSheet, object: nil)
                    }
                    ribbonCompactButton("ruler", "CAD Tools…", tip: "Architectural Scale Lock, Convert CAD Layers & Export Binary .DWG") {
                        NotificationCenter.default.post(name: .vexioOpenCadToolsSheet, object: nil)
                    }
                }
            }
        }
    }

    // 3. DESIGN RIBBON: Page Setup & Sizes | Orientation | Background & Themes | Routing | Auto-Layout
    private var designRibbonGroups: some View {
        HStack(spacing: 12) {
            ribbonGroup(title: i18n.t("pageSize")) {
                HStack(spacing: 5) {
                    ribbonCompactButton("doc.badge.gearshape", i18n.t("pageSetup"), tip: "Open Page Setup (⇧⌘P)") {
                        NotificationCenter.default.post(name: .vexioOpenPageSetupSheet, object: session.activePageId)
                    }

                    ForEach([VexioPagePreset.a4, .a3, .a5, .letter], id: \.id) { preset in
                        Button(preset.shortCode) {
                            session.applyPagePreset(preset, landscape: session.isPageLandscape, undoManager: undoManager)
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .help(preset.displayLabel)
                    }

                    HoverSwitchMenuButton(
                        id: "design.morepages",
                        title: "More Sizes",
                        systemImage: "doc.plaintext",
                        coordinator: menuCoordinator
                    ) {
                        ForEach(VexioPagePreset.allCases) { preset in
                            Button(preset.displayLabel) {
                                session.applyPagePreset(preset, landscape: session.isPageLandscape, undoManager: undoManager)
                                menuCoordinator.activeMenuID = nil
                            }
                            .buttonStyle(.plain)
                            .padding(.vertical, 2.5)
                        }
                    }
                }
            }

            Divider().frame(height: 44)

            ribbonGroup(title: i18n.t("orientation")) {
                HStack(spacing: 5) {
                    ribbonCompactButton("rectangle.portrait.rotate", i18n.t("landscape")) {
                        session.setPageOrientation(landscape: true, undoManager: undoManager)
                    }
                    ribbonCompactButton("rectangle.portrait", i18n.t("portrait")) {
                        session.setPageOrientation(landscape: false, undoManager: undoManager)
                    }
                }
            }

            Divider().frame(height: 44)

            ribbonGroup(title: "Themes & Titles") {
                HStack(spacing: 5) {
                    HoverSwitchMenuButton(
                        id: "design.borders",
                        title: i18n.t("bordersAndTitles"),
                        systemImage: "rectangle.topthird.inset.filled",
                        tooltip: "Apply Visio Page Borders & Title Blocks",
                        coordinator: menuCoordinator
                    ) {
                        ForEach(VexioBorderTitleStyle.allCases.filter { $0 != .none }) { style in
                            Button(style.displayName) {
                                session.applyBordersAndTitles(style: style, undoManager: undoManager)
                                menuCoordinator.activeMenuID = nil
                            }
                            .buttonStyle(.plain)
                            .padding(.vertical, 2.5)
                        }
                        Divider()
                        Button("Customize Title & Metadata…") {
                            menuCoordinator.activeMenuID = nil
                            NotificationCenter.default.post(name: .vexioOpenBordersAndTitlesSheet, object: nil)
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2.5)
                        Button("None (Remove Border & Title)") {
                            session.applyBordersAndTitles(style: .none, undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2.5)
                    }

                    HoverSwitchMenuButton(
                        id: "design.themes",
                        title: "Themes",
                        systemImage: "paintpalette",
                        coordinator: menuCoordinator
                    ) {
                        ForEach(VexioThemePreset.allCases) { theme in
                            Button(theme.displayName) {
                                session.applyThemeAndVariant(themeSlug: theme.rawValue, variantIndex: 0, undoManager: undoManager)
                                menuCoordinator.activeMenuID = nil
                            }
                            .buttonStyle(.plain)
                            .padding(.vertical, 2.5)
                        }
                    }

                    HoverSwitchMenuButton(
                        id: "design.background",
                        title: i18n.t("pageBackground"),
                        systemImage: "paintbrush.pointed",
                        coordinator: menuCoordinator
                    ) {
                        let activeSummary = session.pages.first(where: { $0.id == session.activePageId })
                        let isBg = activeSummary?.isBackgroundPage ?? false
                        Button(isBg ? "✓ Background Page" : "Mark as Background Page") {
                            session.updatePageBackground(pageId: session.activePageId, isBackground: !isBg, undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2.5)
                        Divider()
                        Button("Paper: Pure White") {
                            session.updatePageBackground(pageId: session.activePageId, isBackground: isBg, pageColorHex: "#FFFFFF", undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2.5)
                        Button("Paper: Warm Ivory") {
                            session.updatePageBackground(pageId: session.activePageId, isBackground: isBg, pageColorHex: "#FAF8F5", undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2.5)
                        Button("Paper: Soft Slate") {
                            session.updatePageBackground(pageId: session.activePageId, isBackground: isBg, pageColorHex: "#F1F5F9", undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2.5)
                        Button("Paper: Blueprint Tint") {
                            session.updatePageBackground(pageId: session.activePageId, isBackground: isBg, pageColorHex: "#EFF6FF", undoManager: undoManager)
                            menuCoordinator.activeMenuID = nil
                        }
                        .buttonStyle(.plain)
                        .padding(.vertical, 2.5)
                    }
                }
            }

            Divider().frame(height: 44)

            ribbonGroup(title: "\(i18n.t("routing")) & \(i18n.t("autoLayout"))") {
                HStack(spacing: 6) {
                    HoverSwitchMenuButton(
                        id: "design.routing",
                        title: i18n.t("routing"),
                        systemImage: "point.topleft.down.curvedto.point.bottomright.up",
                        coordinator: menuCoordinator
                    ) {
                        ForEach(ConnectorRoutingStyle.allCases) { style in
                            Button(style.rawValue) {
                                session.setConnectorRoutingStyle(style, undoManager: undoManager)
                                menuCoordinator.activeMenuID = nil
                            }
                            .buttonStyle(.plain)
                            .padding(.vertical, 2.5)
                        }
                    }

                    HoverSwitchMenuButton(
                        id: "design.autolayout",
                        title: i18n.t("autoLayout"),
                        systemImage: "rectangle.3.group",
                        coordinator: menuCoordinator
                    ) {
                        ForEach(AutoLayoutPreset.allCases) { preset in
                            Button(preset.rawValue) {
                                session.applyAutoLayout(preset, undoManager: undoManager)
                                menuCoordinator.activeMenuID = nil
                            }
                            .buttonStyle(.plain)
                            .padding(.vertical, 2.5)
                        }
                    }
                }
            }
        }
    }

    // 4. DATA RIBBON
    private var dataRibbonGroups: some View {
        HStack(spacing: 12) {
            ribbonGroup(title: "Two-Way External Data") {
                HStack(spacing: 6) {
                    ribbonCompactButton(
                        "tray.and.arrow.down",
                        "Link Data…",
                        tip: "Link Excel (.xlsx), CSV, SQLite (.sqlite/.db), or JSON to shapes"
                    ) {
                        session.presentImportExternalDataModal(undoManager: undoManager)
                    }
                    ribbonCompactButton(
                        "arrow.clockwise",
                        "Refresh",
                        tip: "Refresh all linked External Data recordsets on active page"
                    ) {
                        session.refreshExternalDataSources(undoManager: undoManager)
                    }
                    ribbonCompactButton(
                        "arrow.up.doc",
                        "Writeback…",
                        tip: "Push edited Shape Data (Prop.*) back to .xlsx / .csv / .sqlite / .json"
                    ) {
                        session.presentTwoWayWritebackModal(recordsetId: 0, undoManager: undoManager)
                    }
                }
            }

            Divider().frame(height: 44)

            ribbonGroup(title: "Data Graphics & Legends") {
                HStack(spacing: 6) {
                    LiquidGlassIconButton(
                        systemImage: "chart.bar.xaxis",
                        title: "Toggle Live Data Graphics Overlay (Icon Sets, Data Bars, Callouts, Color by Value)",
                        isActive: session.dataGraphicConfig.isEnabled
                    ) {
                        session.dataGraphicConfig.isEnabled.toggle()
                        session.commitDataGraphicConfig(undoManager: undoManager)
                    }
                    ribbonCompactButton(
                        "list.bullet.rectangle.portrait",
                        "Insert Legend",
                        tip: "Insert or update live Color-by-Value / Category Legend on page"
                    ) {
                        session.dataGraphicConfig.showLegendOnPage = true
                        session.insertOrRefreshPageLegend(undoManager: undoManager)
                    }
                    ribbonCompactButton(
                        "doc.text.magnifyingglass",
                        "BOM Report…",
                        tip: "Export Bill of Materials (BOM) Shape Inventory (.xlsx / .csv / .json / .sqlite)"
                    ) {
                        session.presentBomReportExportModal()
                    }
                }
            }

            Divider().frame(height: 44)

            ribbonGroup(title: "Plan 2 Discovery") {
                HStack(spacing: 6) {
                    ribbonCompactButton("tablecells", "Data Visualizer", tip: "Generate Cross-Functional Flowchart") {
                        session.importDataVisualizerSample(undoManager: undoManager)
                    }
                    ribbonCompactButton("cloud", "Cloud Topology", tip: "Import AWS / Azure / GCP Topology") {
                        session.importCloudTopologySample(undoManager: undoManager)
                    }
                    ribbonCompactButton("person.3", "Entra / LDIF Org", tip: "Import Entra ID / LDIF Org Chart") {
                        session.importEntraLdifSample(undoManager: undoManager)
                    }
                    ribbonCompactButton("square.and.arrow.up", "Export .xlsx", tip: "Export Data Visualizer Workbook") {
                        onRequestDataVisualizerExport()
                    }
                }
            }
        }
    }

    // 5. PROCESS RIBBON (Relocated Validate from Screenshot 4)
    private var processRibbonGroups: some View {
        HStack(spacing: 12) {
            ribbonGroup(title: i18n.t("validate")) {
                HStack(spacing: 6) {
                    ForEach(ValidationStandardKind.allCases) { std in
                        ribbonCompactButton("checkmark.shield", std.rawValue) {
                            session.runDiagramValidation(standard: std)
                        }
                    }
                }
            }
            Divider().frame(height: 44)
            ribbonGroup(title: "Subprocess & Schedules") {
                HStack(spacing: 6) {
                    ribbonCompactButton("arrow.turn.down.right", "Link Subprocess") {
                        session.createSubprocessDrilldown(undoManager: undoManager)
                    }
                    ribbonCompactButton("calendar.day.timeline.left", "Gantt / PERT / Timeline…") {
                        NotificationCenter.default.post(name: .vexioOpenDomainWizardsSheet, object: "gantt")
                    }
                }
            }
        }
    }

    // 6. REVIEW RIBBON
    private var reviewRibbonGroups: some View {
        HStack(spacing: 12) {
            ribbonGroup(title: "Diagram Compare & Notes") {
                HStack(spacing: 6) {
                    ribbonCompactButton("pin", "Pin Baseline") {
                        session.captureComparisonBaseline()
                    }
                    ribbonCompactButton("arrow.left.arrow.right.square", "Compare") {
                        session.compareWithSnapshot()
                    }
                    ribbonCompactButton("text.bubble", "Comments…", tip: "Threaded Review Notes") {
                        NotificationCenter.default.post(name: .vexioOpenCommentsSheet, object: nil)
                    }
                }
            }
            Divider().frame(height: 44)
            ribbonGroup(title: "Markup Ink") {
                HStack(spacing: 6) {
                    ribbonCompactButton("pencil.tip.crop.circle", "Pen Ink") {
                        session.activeTool = .ink
                    }
                    ribbonCompactButton("highlighter", "Highlighter") {
                        session.activeTool = .highlighter
                    }
                    ribbonCompactButton("eraser", "Eraser") {
                        session.activeTool = .inkEraser
                    }
                    ribbonCompactButton("trash", "Clear Ink") {
                        session.clearActivePageInk(undoManager: undoManager)
                    }
                }
            }
        }
    }

    // 7. VIEW RIBBON
    private var viewRibbonGroups: some View {
        HStack(spacing: 12) {
            ribbonGroup(title: "Workstation Panes") {
                HStack(spacing: 6) {
                    ribbonCompactButton("sidebar.left", i18n.t("stencils")) {
                        session.showStencilNavigator.toggle()
                    }
                    ribbonCompactButton("sidebar.right", "Inspector") {
                        session.showInspectorSidebar.toggle()
                    }
                    ribbonCompactButton("function", i18n.t("shapeSheet")) {
                        session.showShapeSheetDrawer.toggle()
                    }
                    LiquidGlassIconButton(
                        systemImage: "ruler",
                        title: "Smart Alignment & Equal-Spacing Guides",
                        isActive: session.smartGuidesEnabled
                    ) {
                        session.smartGuidesEnabled.toggle()
                    }
                }
            }
            Divider().frame(height: 44)
            ribbonGroup(title: "Zoom & Preview") {
                HStack(spacing: 6) {
                    ribbonCompactButton("arrow.up.left.and.down.right.magnifyingglass", "Fit Page", tip: "Fit Diagram to Window") {
                        session.fitViewportToActivePage()
                    }
                    ribbonCompactButton("1.magnifyingglass", "100%", tip: "Reset Zoom to 100%") {
                        session.setManualZoomScale(1.0)
                        session.panOffset = .zero
                    }
                    ribbonCompactButton("plus.magnifyingglass", "Zoom In", tip: "Zoom In (+25%)") {
                        session.setManualZoomScale(session.zoomScale * 1.25)
                    }
                    ribbonCompactButton("minus.magnifyingglass", "Zoom Out", tip: "Zoom Out (-20%)") {
                        session.setManualZoomScale(session.zoomScale / 1.25)
                    }
                    ribbonCompactButton("eye", "Preview", tip: "High-DPI Print / Render Preview") {
                        onRequestOffscreenPreview()
                    }
                    ribbonCompactButton("play.rectangle", "Slides…", tip: "Presentation Mode Slide Snippets & Multi-Slide .pptx Deck") {
                        NotificationCenter.default.post(name: .vexioOpenSlideSnippetsSheet, object: nil)
                    }
                }
            }
        }
    }

    // 8. DEVELOPER RIBBON (Relocated Boolean Shape Ops + ShapeSheet + Diagram-as-Code)
    private var developerRibbonGroups: some View {
        HStack(spacing: 12) {
            ribbonGroup(title: i18n.t("shapeOps")) {
                HStack(spacing: 5) {
                    ForEach(VexioBooleanOperation.allCases) { op in
                        ribbonCompactButton(op.symbolName, op.displayName) {
                            session.applyBooleanOperation(op, undoManager: undoManager)
                        }
                    }
                }
            }
            Divider().frame(height: 44)
            ribbonGroup(title: i18n.t("shapeSheet")) {
                HStack(spacing: 5) {
                    ribbonCompactButton("function", "Formula Editor") {
                        withAnimation(LiquidGlassPhysics.panelExpansionSpring) {
                            session.showShapeSheetDrawer.toggle()
                        }
                    }
                    ribbonCompactButton("diamond", "+ Control Handle", tip: "Attach Parametric Yellow Controls.Row_1 Diamond") {
                        session.ensureSelectedControlHandle(undoManager: undoManager)
                    }
                    ribbonCompactButton("point.topleft.down.curvedto.point.bottomright.up", "Reset Route", tip: "Restore Automatic A* Orthogonal Routing") {
                        session.resetSelectedConnectorRouting(undoManager: undoManager)
                    }
                    ribbonCompactButton("point.3.connected.trianglepath.dotted", "Edit Vertices", tip: "Drag Vertices, Option-Click to Insert, Right-Click to Delete") {
                        session.activeTool = (session.activeTool == .editVertices) ? .pointer : .editVertices
                    }
                    ribbonCompactButton("plus.circle.dashed", "Connection Points", tip: "Connection Point Authoring Tool (⌃⇧1) — Click Shape to Add, Drag to Move, Right-Click to Delete") {
                        session.activeTool = (session.activeTool == .connectionPoint) ? .pointer : .connectionPoint
                    }
                    ribbonCompactButton("bolt.badge.checkmark", "Smart Actions…", tip: "Author Right-Click Smart-Shape Actions.* Context Menu Rows") {
                        NotificationCenter.default.post(name: .vexioOpenSmartActionsSheet, object: nil)
                    }
                }
            }
            Divider().frame(height: 44)
            ribbonGroup(title: "Studios & Automation") {
                HStack(spacing: 5) {
                    ribbonCompactButton("chevron.left.forwardslash.chevron.right", "Mermaid / DOT…", tip: "Bidirectional Mermaid (.mmd), PlantUML (.puml) & Graphviz DOT (.dot) Studio") {
                        NotificationCenter.default.post(name: .vexioOpenMarkupStudioSheet, object: nil)
                    }
                    ribbonCompactButton("terminal", "VBA Macros…", tip: "Sandboxed VBA Automation Studio (⌥⌘V)") {
                        NotificationCenter.default.post(name: .vexioOpenVbaStudioSheet, object: nil)
                    }
                    ribbonCompactButton("cylinder.split.1x2", "SQL ⇄ ERD…", tip: "Reverse & Forward SQL Schema Engineering") {
                        NotificationCenter.default.post(name: .vexioOpenDomainWizardsSheet, object: "sql_reverse")
                    }
                }
            }
        }
    }

    // MARK: - Ribbon Building Blocks

    @ViewBuilder
    private func ribbonGroup<Content: View>(title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(spacing: 3) {
            content()
                .frame(height: 30)
            Text(title.uppercased())
                .font(.system(size: 8.5, weight: .bold))
                .lineLimit(1)
                .fixedSize(horizontal: true, vertical: false)
                .foregroundStyle(.secondary)
        }
        .fixedSize(horizontal: true, vertical: false)
    }

    @ViewBuilder
    private func ribbonCompactButton(_ icon: String, _ label: String, tip: String = "", action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 4) {
                Image(systemName: icon)
                    .font(.system(size: 11.5, weight: .medium))
                Text(label)
                    .font(.system(size: 11, weight: .medium))
                    .lineLimit(1)
                    .fixedSize(horizontal: true, vertical: false)
            }
            .padding(.horizontal, 7)
            .padding(.vertical, 4)
            .background(RoundedRectangle(cornerRadius: 6, style: .continuous).fill(Color.primary.opacity(0.055)))
        }
        .buttonStyle(.plain)
        .help(tip.isEmpty ? label : tip)
    }

    @ViewBuilder
    private func styleToggleButton(
        _ char: String,
        bold: Bool = false,
        italic: Bool = false,
        underline: Bool = false,
        strike: Bool = false,
        active: Bool,
        tip: String,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Text(char)
                .font(.system(size: 12, weight: bold ? .bold : .semibold))
                .italic(italic)
                .underline(underline)
                .strikethrough(strike)
                .frame(width: 24, height: 24)
                .background(
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(active ? Color.accentColor.opacity(0.24) : Color.primary.opacity(0.05))
                )
        }
        .buttonStyle(.plain)
        .help(tip)
    }
}
