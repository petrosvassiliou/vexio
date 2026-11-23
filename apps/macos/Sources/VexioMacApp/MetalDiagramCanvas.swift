import AppKit
import Metal
import QuartzCore
import SwiftUI
import VexioCoreFFI

// MARK: - SwiftUI Bridge for CAMetalLayer-Backed Vexio Viewport

public struct MetalDiagramCanvas: NSViewRepresentable {
    @ObservedObject public var session: VexioDocumentSession
    @Environment(\.undoManager) private var undoManager

    public init(session: VexioDocumentSession) {
        self.session = session
    }

    public func makeNSView(context: Context) -> VexioMetalNSView {
        let view = VexioMetalNSView(frame: .zero)
        view.session = session
        view.activeUndoManager = undoManager
        view.configureGestureRecognizers()
        return view
    }

    public func updateNSView(_ nsView: VexioMetalNSView, context: Context) {
        nsView.session = session
        nsView.activeUndoManager = undoManager
        nsView.syncViewportDrawableIfNeeded()
        nsView.refreshActiveCursor()
    }

    public static func dismantleNSView(_ nsView: VexioMetalNSView, coordinator: ()) {
        nsView.session?.detachViewport()
        nsView.session = nil
    }
}

// MARK: - Native AppKit + CAMetalLayer Canvas View

@MainActor
public final class VexioMetalNSView: NSView {
    public weak var session: VexioDocumentSession?
    public weak var activeUndoManager: UndoManager?

    private var trackingArea: NSTrackingArea?
    private var lastBoundPixelSize: CGSize = .zero
    private var lastBoundScale: CGFloat = 0.0
    private var forceTouchLatchedInCurrentPress: Bool = false
    private var activeCursor: NSCursor = .arrow

    public var metalBackingLayer: CAMetalLayer? {
        layer as? CAMetalLayer
    }

    override public init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        commonInit()
    }

    public required init?(coder: NSCoder) {
        super.init(coder: coder)
        commonInit()
    }

    private func commonInit() {
        wantsLayer = true
        layerContentsRedrawPolicy = .duringViewResize
        layerContentsPlacement = .scaleAxesIndependently
        allowedTouchTypes = [.indirect, .direct]
        wantsRestingTouches = true
        registerForDraggedTypes([
            .string,
            NSPasteboard.PasteboardType("public.utf8-plain-text"),
            NSPasteboard.PasteboardType("public.plain-text")
        ])
    }

    override public var acceptsFirstResponder: Bool {
        true
    }

    override public var isFlipped: Bool {
        true
    }

    override public func makeBackingLayer() -> CALayer {
        let metalLayer = CAMetalLayer()
        metalLayer.device = MTLCreateSystemDefaultDevice()
        metalLayer.pixelFormat = .bgra8Unorm
        metalLayer.framebufferOnly = false
        metalLayer.presentsWithTransaction = false
        metalLayer.isOpaque = true
        metalLayer.colorspace = CGColorSpace(name: CGColorSpace.sRGB)
        metalLayer.contentsScale = NSScreen.main?.backingScaleFactor ?? 2.0
        return metalLayer
    }

    // MARK: - Gesture Recognizers (Trackpad Pinch-to-Zoom & Two-Finger Rotate)

    public func configureGestureRecognizers() {
        guard gestureRecognizers.isEmpty else { return }

        let magnificationRecognizer = NSMagnificationGestureRecognizer(
            target: self,
            action: #selector(handleMagnificationGesture(_:))
        )
        addGestureRecognizer(magnificationRecognizer)

        let rotationRecognizer = NSRotationGestureRecognizer(
            target: self,
            action: #selector(handleRotationGesture(_:))
        )
        addGestureRecognizer(rotationRecognizer)
    }

    @objc private func handleMagnificationGesture(_ recognizer: NSMagnificationGestureRecognizer) {
        guard recognizer.state == .changed else { return }
        let anchor = recognizer.location(in: self)
        applyZoomDelta(magnification: recognizer.magnification, anchorInView: anchor)
        recognizer.magnification = 0
    }

    @objc private func handleRotationGesture(_ recognizer: NSRotationGestureRecognizer) {
        guard recognizer.state == .changed else { return }
        let deltaDegrees = Double(-recognizer.rotationInDegrees)
        if abs(deltaDegrees) > 0.05 {
            session?.rotateSelectedShape(byDegrees: deltaDegrees, undoManager: activeUndoManager)
            recognizer.rotation = 0
        }
    }

    // MARK: - Window, Backing Scale & Metal Surface Attachment

    override public func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil {
            session?.detachViewport()
            lastBoundPixelSize = .zero
        } else {
            window?.acceptsMouseMovedEvents = true
            syncViewportDrawableIfNeeded(forceRebind: true)
        }
    }

    override public func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        syncViewportDrawableIfNeeded(forceRebind: true)
    }

    override public func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        syncViewportDrawableIfNeeded()
    }

    public func syncViewportDrawableIfNeeded(forceRebind: Bool = false) {
        guard window != nil,
              let session,
              let metalLayer = metalBackingLayer
        else {
            return
        }

        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        let pixelWidth = max( floor(bounds.width * scale), 1.0 )
        let pixelHeight = max( floor(bounds.height * scale), 1.0 )
        let targetPixelSize = CGSize(width: pixelWidth, height: pixelHeight)

        guard forceRebind || targetPixelSize != lastBoundPixelSize || scale != lastBoundScale else {
            return
        }

        metalLayer.contentsScale = scale
        metalLayer.drawableSize = targetPixelSize
        lastBoundPixelSize = targetPixelSize
        lastBoundScale = scale

        if !forceRebind && session.hasAttachedMetalViewport() {
            session.resizeMetalViewport(
                widthPx: UInt32(pixelWidth),
                heightPx: UInt32(pixelHeight),
                dpiScale: Float(scale)
            )
        } else {
            session.attachMetalLayer(
                metalLayer,
                widthPx: UInt32(pixelWidth),
                heightPx: UInt32(pixelHeight),
                dpiScale: Float(scale)
            )
        }
    }

    // MARK: - Tracking Areas & Dynamic NSCursor Management

    override public func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let existing = trackingArea {
            removeTrackingArea(existing)
        }
        let options: NSTrackingArea.Options = [
            .activeInKeyWindow,
            .mouseMoved,
            .mouseEnteredAndExited,
            .cursorUpdate,
            .inVisibleRect,
        ]
        let created = NSTrackingArea(rect: bounds, options: options, owner: self, userInfo: nil)
        addTrackingArea(created)
        trackingArea = created
    }

    override public func cursorUpdate(with event: NSEvent) {
        activeCursor.set()
    }

    override public func resetCursorRects() {
        discardCursorRects()
        addCursorRect(bounds, cursor: activeCursor)
    }

    public func refreshActiveCursor() {
        guard let session else { return }
        let updated = session.resolveNSCursor(for: session.activeCursorKind)
        if updated != activeCursor {
            activeCursor = updated
            window?.invalidateCursorRects(for: self)
            activeCursor.set()
        }
    }

    // MARK: - Pointer, Tablet & Stylus Event Translation -> VexioPointerEvent

    private func buildPointerEvent(from event: NSEvent, kind: UInt32) -> VexioPointerEvent {
        let localPoint = convert(event.locationInWindow, from: nil)
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0

        let pressedMask = UInt32(NSEvent.pressedMouseButtons)
        var buttons: UInt32 = 0
        if (pressedMask & 0x1) != 0 { buttons |= 1 }   // Primary
        if (pressedMask & 0x2) != 0 { buttons |= 2 }   // Secondary
        if (pressedMask & 0x4) != 0 { buttons |= 4 }   // Middle

        var resolvedKind = kind
        var tiltXDeg: Float = 0.0
        var tiltYDeg: Float = 0.0
        var normalizedPressure: Float = (buttons != 0) ? max(event.pressure, 0.5) : 0.0

        if event.subtype == .tabletPoint || event.subtype == .tabletProximity {
            normalizedPressure = min(max(event.pressure, 0.0), 1.0)
            tiltXDeg = Float(event.tilt.x * 90.0)
            tiltYDeg = Float(event.tilt.y * 90.0)
            if event.pointingDeviceType == .eraser {
                buttons |= 32
            }
            if kind == 0 {
                resolvedKind = 4 // StylusMove
            }
        }

        var modifiers: UInt32 = 0
        let flags = event.modifierFlags
        if flags.contains(.shift) { modifiers |= 1 }
        if flags.contains(.control) { modifiers |= 2 }
        if flags.contains(.option) { modifiers |= 4 }
        if flags.contains(.command) { modifiers |= 8 }

        return VexioPointerEvent(
            kind: resolvedKind,
            buttons: buttons,
            modifiers: modifiers,
            x_px: Double(localPoint.x * scale),
            y_px: Double(localPoint.y * scale),
            pressure: normalizedPressure,
            tilt_x_deg: tiltXDeg,
            tilt_y_deg: tiltYDeg,
            timestamp_us: UInt64(max(event.timestamp, 0.0) * 1_000_000.0)
        )
    }

    private func forwardPointer(event: NSEvent, kind: UInt32) {
        guard let session else { return }
        let pointerEvent = buildPointerEvent(from: event, kind: kind)
        let cursor = session.dispatchPointerEvent(pointerEvent)
        if cursor != activeCursor {
            activeCursor = cursor
            window?.invalidateCursorRects(for: self)
        }
        activeCursor.set()
    }

    override public func mouseDown(with event: NSEvent) {
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        if session?.activeInlineTextEdit != nil {
            // Resigning first responder triggers SelectAllInlineTextField.controlTextDidEndEditing,
            // committing the user's edited draft cleanly before processing the new canvas click.
            window?.makeFirstResponder(self)
        }
        if event.clickCount == 2, let session {
            let localPoint = convert(event.locationInWindow, from: nil)
            if session.handleCanvasDoubleClick(
                xPx: Double(localPoint.x * scale),
                yPx: Double(localPoint.y * scale),
                backingScale: scale
            ) {
                return
            }
        }
        window?.makeFirstResponder(self)
        forceTouchLatchedInCurrentPress = false
        if session?.forceTouchQuickInspect != nil {
            session?.forceTouchQuickInspect = nil
        }
        forwardPointer(event: event, kind: 1)
    }

    override public func mouseDragged(with event: NSEvent) {
        forwardPointer(event: event, kind: 0)
    }

    override public func mouseUp(with event: NSEvent) {
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        forceTouchLatchedInCurrentPress = false
        forwardPointer(event: event, kind: 2)
        session?.reloadSelectedShapeSheetCells()
        session?.syncToolAndInlineTextAfterPointerUp(backingScale: scale)
        if session?.activeTool == .pointer {
            activeCursor = .arrow
            window?.invalidateCursorRects(for: self)
            activeCursor.set()
        }
    }

    private var lastContextCanvasPoint: CGPoint = CGPoint(x: 4.5, y: 4.25)

    private func canvasInchesPoint(from event: NSEvent) -> CGPoint {
        guard let session else { return CGPoint(x: 4.5, y: 4.25) }
        let pt = convert(event.locationInWindow, from: nil)
        let xIn = max((pt.x - session.panOffset.width) / (96.0 * max(session.zoomScale, 0.1)), 0.35)
        let rawTopDownY = max((pt.y - session.panOffset.height) / (96.0 * max(session.zoomScale, 0.1)), 0.35)
        let yIn = min(max(8.5 - rawTopDownY, 0.45), 8.05)
        return CGPoint(x: min(xIn, 10.5), y: yIn)
    }

    override public func rightMouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        let localPoint = convert(event.locationInWindow, from: nil)
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        session?.selectShapeOnRightClickIfNeeded(
            xPx: Double(localPoint.x * scale),
            yPx: Double(localPoint.y * scale)
        )
        lastContextCanvasPoint = canvasInchesPoint(from: event)
        forwardPointer(event: event, kind: 0)
        super.rightMouseDown(with: event)
    }

    override public func menu(for event: NSEvent) -> NSMenu? {
        guard let session else { return nil }
        let localPoint = convert(event.locationInWindow, from: nil)
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        session.selectShapeOnRightClickIfNeeded(
            xPx: Double(localPoint.x * scale),
            yPx: Double(localPoint.y * scale)
        )
        lastContextCanvasPoint = canvasInchesPoint(from: event)

        let menu = NSMenu(title: "Canvas Context Menu")
        menu.autoenablesItems = false

        let editLabelItem = NSMenuItem(title: "Edit Text Label…", action: #selector(ctxEditSelectedLabel(_:)), keyEquivalent: "\r")
        editLabelItem.keyEquivalentModifierMask = []
        editLabelItem.target = self
        menu.addItem(editLabelItem)

        let rotLeftItem = NSMenuItem(title: "Rotate 90° Left (↺)", action: #selector(ctxRotateLeft90(_:)), keyEquivalent: "l")
        rotLeftItem.target = self
        menu.addItem(rotLeftItem)

        let rotRightItem = NSMenuItem(title: "Rotate 90° Right (↻)", action: #selector(ctxRotateRight90(_:)), keyEquivalent: "r")
        rotRightItem.target = self
        menu.addItem(rotRightItem)

        menu.addItem(.separator())

        let dupItem = NSMenuItem(title: "Duplicate Selection", action: #selector(ctxDuplicateSelection(_:)), keyEquivalent: "d")
        dupItem.target = self
        menu.addItem(dupItem)

        let copyItem = NSMenuItem(title: "Copy", action: #selector(ctxCopySelection(_:)), keyEquivalent: "c")
        copyItem.target = self
        menu.addItem(copyItem)

        let pasteItem = NSMenuItem(title: "Paste Here", action: #selector(ctxPasteHere(_:)), keyEquivalent: "v")
        pasteItem.target = self
        menu.addItem(pasteItem)

        let deleteItem = NSMenuItem(title: "Delete", action: #selector(ctxDeleteSelection(_:)), keyEquivalent: "\u{8}")
        deleteItem.keyEquivalentModifierMask = []
        deleteItem.target = self
        menu.addItem(deleteItem)

        menu.addItem(.separator())

        let groupItem = NSMenuItem(title: "Group Selection", action: #selector(ctxGroupSelection(_:)), keyEquivalent: "g")
        groupItem.keyEquivalentModifierMask = [.command]
        groupItem.target = self
        groupItem.isEnabled = session.canGroupSelectedShapes
        menu.addItem(groupItem)

        let ungroupItem = NSMenuItem(title: "Ungroup", action: #selector(ctxUngroupSelection(_:)), keyEquivalent: "g")
        ungroupItem.keyEquivalentModifierMask = [.command, .shift]
        ungroupItem.target = self
        ungroupItem.isEnabled = session.canUngroupSelectedShapes
        menu.addItem(ungroupItem)

        menu.addItem(.separator())

        let frontItem = NSMenuItem(title: "Bring to Front", action: #selector(ctxBringToFront(_:)), keyEquivalent: "")
        frontItem.target = self
        menu.addItem(frontItem)

        let forwardItem = NSMenuItem(title: "Bring Forward", action: #selector(ctxBringForward(_:)), keyEquivalent: "")
        forwardItem.target = self
        menu.addItem(forwardItem)

        let backwardItem = NSMenuItem(title: "Send Backward", action: #selector(ctxSendBackward(_:)), keyEquivalent: "")
        backwardItem.target = self
        menu.addItem(backwardItem)

        let backItem = NSMenuItem(title: "Send to Back", action: #selector(ctxSendToBack(_:)), keyEquivalent: "")
        backItem.target = self
        menu.addItem(backItem)

        menu.addItem(.separator())

        // Insert Shape Here Submenu
        let insertSubmenu = NSMenu(title: "Insert Shape Here")
        let quickPrimitives: [(String, String)] = [
            ("Rectangle", "rect"),
            ("Rounded Rectangle", "round_rect"),
            ("Circle / Ellipse", "ellipse"),
            ("Diamond (Decision)", "diamond"),
            ("Triangle", "triangle"),
            ("Hexagon", "hexagon"),
            ("5-Point Star", "star"),
            ("Straight Line", "line"),
            ("Text Block", "text"),
        ]
        for (label, kind) in quickPrimitives {
            let item = NSMenuItem(title: label, action: #selector(ctxInsertPrimitive(_:)), keyEquivalent: "")
            item.representedObject = kind
            item.target = self
            insertSubmenu.addItem(item)
        }
        let insertParent = NSMenuItem(title: "Insert Shape Here", action: nil, keyEquivalent: "")
        insertParent.submenu = insertSubmenu
        menu.addItem(insertParent)

        // Connector Routing Submenu
        let routingSubmenu = NSMenu(title: "Connector Routing")
        for style in ConnectorRoutingStyle.allCases {
            let item = NSMenuItem(title: style.rawValue, action: #selector(ctxSelectRouting(_:)), keyEquivalent: "")
            item.representedObject = style.rawValue
            item.state = (session.connectorRouting == style) ? .on : .off
            item.target = self
            routingSubmenu.addItem(item)
        }
        let routingParent = NSMenuItem(title: "Connector Routing", action: nil, keyEquivalent: "")
        routingParent.submenu = routingSubmenu
        menu.addItem(routingParent)

        // Auto-Layout Submenu
        let layoutSubmenu = NSMenu(title: "Auto-Layout Page")
        for preset in AutoLayoutPreset.allCases {
            let item = NSMenuItem(title: preset.rawValue, action: #selector(ctxApplyLayout(_:)), keyEquivalent: "")
            item.representedObject = preset.rawValue
            item.target = self
            layoutSubmenu.addItem(item)
        }
        let layoutParent = NSMenuItem(title: "Auto-Layout Page", action: nil, keyEquivalent: "")
        layoutParent.submenu = layoutSubmenu
        menu.addItem(layoutParent)

        menu.addItem(.separator())

        let inspectItem = NSMenuItem(title: "Inspect ShapeSheet Formula…", action: #selector(ctxInspectShapeSheet(_:)), keyEquivalent: "")
        inspectItem.target = self
        menu.addItem(inspectItem)

        return menu
    }

    @objc private func ctxEditSelectedLabel(_ sender: NSMenuItem) {
        guard let session, session.selectedShapeId != 0 else { return }
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        session.beginInlineEditing(shapeId: session.selectedShapeId, backingScale: scale)
    }

    @objc private func ctxRotateLeft90(_ sender: NSMenuItem) {
        session?.rotateOrFlipSelectedShapes(deltaDegrees: 90.0, undoManager: activeUndoManager)
    }

    @objc private func ctxRotateRight90(_ sender: NSMenuItem) {
        session?.rotateOrFlipSelectedShapes(deltaDegrees: -90.0, undoManager: activeUndoManager)
    }

    @objc private func ctxDuplicateSelection(_ sender: NSMenuItem) {
        session?.duplicateSelectedShapes(undoManager: activeUndoManager)
    }

    @objc private func ctxCopySelection(_ sender: NSMenuItem) {
        session?.copySelectionToClipboard()
    }

    @objc private func ctxPasteHere(_ sender: NSMenuItem) {
        session?.pasteClipboardAt(canvasInches: lastContextCanvasPoint, undoManager: activeUndoManager)
    }

    @objc private func ctxDeleteSelection(_ sender: NSMenuItem) {
        session?.deleteSelectedShapes(undoManager: activeUndoManager)
    }

    @objc private func ctxGroupSelection(_ sender: NSMenuItem) {
        session?.groupSelectedShapes(undoManager: activeUndoManager)
    }

    @objc private func ctxUngroupSelection(_ sender: NSMenuItem) {
        session?.ungroupSelectedShapes(undoManager: activeUndoManager)
    }

    @objc private func ctxBringToFront(_ sender: NSMenuItem) {
        session?.reorderSelectedShapes(mode: "front", undoManager: activeUndoManager)
    }

    @objc private func ctxBringForward(_ sender: NSMenuItem) {
        session?.reorderSelectedShapes(mode: "forward", undoManager: activeUndoManager)
    }

    @objc private func ctxSendBackward(_ sender: NSMenuItem) {
        session?.reorderSelectedShapes(mode: "backward", undoManager: activeUndoManager)
    }

    @objc private func ctxSendToBack(_ sender: NSMenuItem) {
        session?.reorderSelectedShapes(mode: "back", undoManager: activeUndoManager)
    }

    @objc private func ctxInsertPrimitive(_ sender: NSMenuItem) {
        guard let kind = sender.representedObject as? String else { return }
        session?.drawQuickPrimitive(kind: kind, atCanvasInches: lastContextCanvasPoint, undoManager: activeUndoManager)
    }

    @objc private func ctxSelectRouting(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String,
              let style = ConnectorRoutingStyle(rawValue: raw) else { return }
        session?.setConnectorRoutingStyle(style, undoManager: activeUndoManager)
    }

    @objc private func ctxApplyLayout(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String,
              let preset = AutoLayoutPreset(rawValue: raw) else { return }
        session?.applyAutoLayout(preset, undoManager: activeUndoManager)
    }

    @objc private func ctxInspectShapeSheet(_ sender: NSMenuItem) {
        withAnimation(LiquidGlassPhysics.panelExpansionSpring) {
            session?.showShapeSheetDrawer = true
        }
    }

    override public func keyDown(with event: NSEvent) {
        guard let session else {
            super.keyDown(with: event)
            return
        }
        // Delete (51) or Forward Delete (117)
        if event.keyCode == 51 || event.keyCode == 117 {
            session.deleteSelectedShapes(undoManager: activeUndoManager)
            return
        }
        // Escape returns to Select tool
        if event.keyCode == 53 {
            session.activeTool = .pointer
            return
        }
        // Command+G -> Group Shapes; Shift+Command+G -> Ungroup Shapes
        let cmdFlags = event.modifierFlags.intersection([.command, .shift, .control, .option])
        if let chars = event.charactersIgnoringModifiers?.lowercased(), chars == "g" {
            if cmdFlags == [.command] {
                session.groupSelectedShapes(undoManager: activeUndoManager)
                return
            } else if cmdFlags == [.command, .shift] {
                session.ungroupSelectedShapes(undoManager: activeUndoManager)
                return
            }
        }
        if event.modifierFlags.intersection([.command, .control]).isEmpty,
           let chars = event.charactersIgnoringModifiers?.lowercased() {
            switch chars {
            case "v": session.activeTool = .pointer; return
            case "h": session.activeTool = .pan; return
            case "c": session.activeTool = .connector; return
            case "l": session.activeTool = .line; return
            case "a": session.activeTool = .arc; return
            case "r": session.activeTool = .rectangle; return
            case "o": session.activeTool = .ellipse; return
            case "d": session.activeTool = .diamond; return
            case "x": session.activeTool = .hexagon; return
            case "s": session.activeTool = .star; return
            case "t": session.activeTool = .text; return
            default: break
            }
        }
        super.keyDown(with: event)
    }

    override public func otherMouseDown(with event: NSEvent) {
        forwardPointer(event: event, kind: 1)
    }

    override public func otherMouseDragged(with event: NSEvent) {
        guard let session else { return }
        // Middle-button pan convenience on desktop mice
        session.panOffset.width += event.deltaX
        session.panOffset.height += event.deltaY
        activeCursor = .closedHand
        activeCursor.set()
        forwardPointer(event: event, kind: 0)
    }

    override public func otherMouseUp(with event: NSEvent) {
        forwardPointer(event: event, kind: 2)
        refreshActiveCursor()
    }

    override public func mouseMoved(with event: NSEvent) {
        forwardPointer(event: event, kind: 0)
    }

    override public func tabletPoint(with event: NSEvent) {
        forwardPointer(event: event, kind: 4)
    }

    // MARK: - Multi-Touch Trackpad Pinch, Inertial Two-Finger Pan, Rotate & Force Touch

    private func applyZoomDelta(magnification: CGFloat, anchorInView: CGPoint) {
        guard let session else { return }
        let previousZoom = session.zoomScale
        let clampedZoom = min(max(previousZoom * (1.0 + magnification), 0.10), 16.0)
        let ratio = clampedZoom / max(previousZoom, 0.001)

        // Preserve focal point under the user's fingers during pinch-to-zoom
        let updatedPanX = anchorInView.x - (anchorInView.x - session.panOffset.width) * ratio
        let updatedPanY = anchorInView.y - (anchorInView.y - session.panOffset.height) * ratio

        session.zoomScale = clampedZoom
        session.panOffset = CGSize(width: updatedPanX, height: updatedPanY)
    }

    override public func magnify(with event: NSEvent) {
        let anchor = convert(event.locationInWindow, from: nil)
        applyZoomDelta(magnification: event.magnification, anchorInView: anchor)
    }

    override public func scrollWheel(with event: NSEvent) {
        guard let session else { return }

        // Option / Command + Scroll Wheel performs smooth focal zoom for external mice
        if event.modifierFlags.contains(.option) || event.modifierFlags.contains(.command) {
            let anchor = convert(event.locationInWindow, from: nil)
            let zoomFactor = event.scrollingDeltaY * (event.hasPreciseScrollingDeltas ? 0.006 : 0.04)
            applyZoomDelta(magnification: zoomFactor, anchorInView: anchor)
        } else {
            // Two-finger Trackpad inertial pan (includes momentumPhase automatically)
            let multiplier: CGFloat = event.hasPreciseScrollingDeltas ? 1.0 : 12.0
            session.panOffset.width += event.scrollingDeltaX * multiplier
            session.panOffset.height += event.scrollingDeltaY * multiplier
        }
    }

    @objc public func changeFont(_ sender: Any?) {
        guard let session else { return }
        let fm = NSFontManager.shared
        let baseFont = NSFont(name: session.selectedFontFamily, size: CGFloat(session.selectedFontSizePt))
            ?? NSFont.systemFont(ofSize: CGFloat(session.selectedFontSizePt))
        let converted = fm.convert(baseFont)
        let traits = fm.traits(of: converted)
        var toggleMask: UInt8 = 0
        let wantsBold = traits.contains(.boldFontMask)
        let wantsItalic = traits.contains(.italicFontMask)
        if wantsBold != ((session.selectedCharStyle & 1) != 0) { toggleMask |= 1 }
        if wantsItalic != ((session.selectedCharStyle & 2) != 0) { toggleMask |= 2 }
        session.applyTextStyle(
            fontFamily: converted.familyName ?? converted.fontName,
            fontSizePt: Double(converted.pointSize),
            toggleStyleMask: toggleMask == 0 ? nil : toggleMask,
            undoManager: activeUndoManager
        )
    }

    @objc public func changeColor(_ sender: Any?) {
        guard let session else { return }
        let nsCol = (sender as? NSColorPanel)?.color ?? NSColorPanel.shared.color
        guard let srgb = nsCol.usingColorSpace(.sRGB) else { return }
        let hex = String(
            format: "#%02X%02X%02X",
            Int((srgb.redComponent * 255.0).rounded()),
            Int((srgb.greenComponent * 255.0).rounded()),
            Int((srgb.blueComponent * 255.0).rounded())
        )
        session.applyTextStyle(textColorHex: hex, undoManager: activeUndoManager)
    }

    override public func rotate(with event: NSEvent) {
        let deltaDegrees = Double(-event.rotation)
        if abs(deltaDegrees) > 0.05 {
            session?.rotateSelectedShape(byDegrees: deltaDegrees, undoManager: activeUndoManager)
        }
    }

    override public func pressureChange(with event: NSEvent) {
        guard let session else { return }

        // Force Touch Stage 2 deep-click summons the ShapeSheet Quick-Inspect Popover
        if event.stage >= 2 && !forceTouchLatchedInCurrentPress {
            forceTouchLatchedInCurrentPress = true
            NSHapticFeedbackManager.defaultPerformer.perform(.generic, performanceTime: .now)
            let viewLocation = convert(event.locationInWindow, from: nil)
            session.reloadSelectedShapeSheetCells()
            withAnimation(LiquidGlassPhysics.standardSpring) {
                session.forceTouchQuickInspect = ForceTouchQuickInspectContext(
                    shapeId: session.selectedShapeId,
                    viewLocation: viewLocation,
                    pressure: event.stageTransition > 0 ? Float(event.stageTransition) : event.pressure
                )
            }
        }
    }

    // MARK: - Stencil Drag-and-Drop Destination (NSDraggingDestination)

    override public func draggingEntered(_ sender: any NSDraggingInfo) -> NSDragOperation {
        if sender.draggingPasteboard.canReadObject(forClasses: [NSString.self], options: nil)
            || session?.activeDraggedStencilMasterId != nil
        {
            return .copy
        }
        return []
    }

    override public func draggingUpdated(_ sender: any NSDraggingInfo) -> NSDragOperation {
        if sender.draggingPasteboard.canReadObject(forClasses: [NSString.self], options: nil)
            || session?.activeDraggedStencilMasterId != nil
        {
            return .copy
        }
        return []
    }

    override public func draggingEnded(_ sender: any NSDraggingInfo) {
        session?.activeDraggedStencilMasterId = nil
    }

    override public func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
        guard let session else { return false }
        let pb = sender.draggingPasteboard
        let resolvedMasterId = pb.string(forType: .string)
            ?? pb.string(forType: NSPasteboard.PasteboardType("public.utf8-plain-text"))
            ?? pb.string(forType: NSPasteboard.PasteboardType("public.plain-text"))
            ?? (pb.readObjects(forClasses: [NSString.self], options: nil)?.first as? String)
            ?? session.activeDraggedStencilMasterId
        session.activeDraggedStencilMasterId = nil

        guard let masterId = resolvedMasterId else {
            return false
        }
        session.ensureStencilsLoadedIfNeeded()
        guard session.stencilCatalog.contains(where: { $0.id == masterId })
            || VexioDocumentSession.builtInStencilCatalog.contains(where: { $0.id == masterId })
        else {
            return false
        }

        let dropPointInView = convert(sender.draggingLocation, from: nil)
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        let pagePoint = session.screenPxToPageInches(
            xPx: Double(dropPointInView.x * scale),
            yPx: Double(dropPointInView.y * scale)
        )

        session.instantiateStencilMaster(
            masterId: masterId,
            atCanvasInches: pagePoint,
            undoManager: activeUndoManager
        )
        return true
    }
}
