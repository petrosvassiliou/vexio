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
        registerForDraggedTypes([.string])
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

        session.attachMetalLayer(
            metalLayer,
            widthPx: UInt32(pixelWidth),
            heightPx: UInt32(pixelHeight),
            dpiScale: Float(scale)
        )
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
        forceTouchLatchedInCurrentPress = false
        forwardPointer(event: event, kind: 2)
        session?.reloadSelectedShapeSheetCells()
    }

    override public func rightMouseDown(with event: NSEvent) {
        forwardPointer(event: event, kind: 1)
    }

    override public func rightMouseDragged(with event: NSEvent) {
        forwardPointer(event: event, kind: 0)
    }

    override public func rightMouseUp(with event: NSEvent) {
        forwardPointer(event: event, kind: 2)
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

        forwardPointer(event: event, kind: 3)
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
                    pressure: event.stageTransition > 0 ? event.stageTransition : event.pressure
                )
            }
        }
    }

    // MARK: - Stencil Drag-and-Drop Destination (NSDraggingDestination)

    override public func draggingEntered(_ sender: any NSDraggingInfo) -> NSDragOperation {
        if sender.draggingPasteboard.canReadObject(forClasses: [NSString.self], options: nil) {
            return .copy
        }
        return []
    }

    override public func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
        guard let session,
              let masterId = sender.draggingPasteboard.string(forType: .string),
              VexioDocumentSession.builtInStencilCatalog.contains(where: { $0.id == masterId })
        else {
            return false
        }

        let dropPointInView = convert(sender.draggingLocation, from: nil)
        let canvasXInches = max((dropPointInView.x - session.panOffset.width) / (96.0 * max(session.zoomScale, 0.1)), 0.25)
        let canvasYInches = max((dropPointInView.y - session.panOffset.height) / (96.0 * max(session.zoomScale, 0.1)), 0.25)

        session.instantiateStencilMaster(
            masterId: masterId,
            atCanvasInches: CGPoint(x: canvasXInches, y: canvasYInches),
            undoManager: activeUndoManager
        )
        return true
    }
}
