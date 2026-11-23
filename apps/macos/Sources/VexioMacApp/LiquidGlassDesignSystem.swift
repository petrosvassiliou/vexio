import AppKit
import SwiftUI

// MARK: - Apple Liquid Glass Motion & Physics Tokens

public enum LiquidGlassPhysics {
    /// Canonical Apple Liquid Glass spring response for interactive lenses, tool switches, and floating palettes.
    public static let standardSpring = Animation.spring(response: 0.32, dampingFraction: 0.78)

    /// Micro-interaction spring for hover caustics and specular button press states.
    public static let microLensSpring = Animation.spring(response: 0.24, dampingFraction: 0.80)

    /// Panel expansion spring for the ShapeSheet formula drawer and inspector transitions.
    public static let panelExpansionSpring = Animation.spring(response: 0.36, dampingFraction: 0.82)
}

public enum LiquidGlassElevation: Sendable {
    case toolbarBar
    case floatingPalette
    case pillControl
    case quickPopover

    var shadowRadius: CGFloat {
        switch self {
        case .toolbarBar: return 14
        case .floatingPalette: return 22
        case .pillControl: return 12
        case .quickPopover: return 26
        }
    }

    var shadowYOffset: CGFloat {
        switch self {
        case .toolbarBar: return 5
        case .floatingPalette: return 10
        case .pillControl: return 6
        case .quickPopover: return 12
        }
    }

    var baseMaterial: Material {
        switch self {
        case .toolbarBar: return .thinMaterial
        case .floatingPalette, .pillControl: return .ultraThinMaterial
        case .quickPopover: return .regularMaterial
        }
    }
}

// MARK: - Translucent Adaptive Liquid Glass Surface & Specular Rim-Light Modifier

public struct LiquidGlassSurfaceModifier: ViewModifier {
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency

    let cornerRadius: CGFloat
    let elevation: LiquidGlassElevation
    let tintColor: Color?

    @State private var hoverPoint: CGPoint?
    @State private var viewSize: CGSize = CGSize(width: 180, height: 48)

    public init(
        cornerRadius: CGFloat = 16,
        elevation: LiquidGlassElevation = .floatingPalette,
        tintColor: Color? = nil
    ) {
        self.cornerRadius = cornerRadius
        self.elevation = elevation
        self.tintColor = tintColor
    }

    public func body(content: Content) -> some View {
        let shape = RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)

        content
            .background(
                GeometryReader { proxy in
                    ZStack {
                        if reduceTransparency {
                            shape.fill(Color(nsColor: .windowBackgroundColor))
                        } else {
                            // 1. Adaptive Apple translucent material over live Metal canvas
                            shape.fill(elevation.baseMaterial)

                            // 2. Subtle chromatic dispersion / tint refraction layer
                            shape.fill(
                                LinearGradient(
                                    colors: [
                                        (tintColor ?? Color.accentColor).opacity(colorScheme == .dark ? 0.11 : 0.07),
                                        Color.white.opacity(colorScheme == .dark ? 0.04 : 0.18),
                                        Color.cyan.opacity(colorScheme == .dark ? 0.05 : 0.04),
                                    ],
                                    startPoint: .topLeading,
                                    endPoint: .bottomTrailing
                                )
                            )

                            // 3. Interactive pointer-tracked refractive caustic highlight
                            if let hover = hoverPoint {
                                let normalizedX = min(max(hover.x / max(proxy.size.width, 1), 0), 1)
                                let normalizedY = min(max(hover.y / max(proxy.size.height, 1), 0), 1)
                                shape.fill(
                                    RadialGradient(
                                        colors: [
                                            Color.white.opacity(colorScheme == .dark ? 0.16 : 0.34),
                                            Color.clear,
                                        ],
                                        center: UnitPoint(x: normalizedX, y: normalizedY),
                                        startRadius: 4,
                                        endRadius: max(proxy.size.width, proxy.size.height) * 0.65
                                    )
                                )
                            }
                        }
                    }
                    .onAppear { viewSize = proxy.size }
                    .onChange(of: proxy.size) { _, updated in viewSize = updated }
                }
            )
            .overlay(
                // 4. Specular Rim-Light Depth Highlight (incident top-left optical bevel + counter-rim)
                shape
                    .strokeBorder(
                        LinearGradient(
                            stops: [
                                .init(color: Color.white.opacity(colorScheme == .dark ? 0.42 : 0.78), location: 0.0),
                                .init(color: Color.white.opacity(colorScheme == .dark ? 0.14 : 0.32), location: 0.38),
                                .init(color: Color.black.opacity(colorScheme == .dark ? 0.38 : 0.10), location: 0.74),
                                .init(color: Color.white.opacity(colorScheme == .dark ? 0.22 : 0.48), location: 1.0),
                            ],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        ),
                        lineWidth: 1.0
                    )
            )
            .shadow(
                color: Color.black.opacity(colorScheme == .dark ? 0.42 : 0.14),
                radius: elevation.shadowRadius,
                x: 0,
                y: elevation.shadowYOffset
            )
            .onContinuousHover { phase in
                switch phase {
                case .active(let location):
                    withAnimation(LiquidGlassPhysics.microLensSpring) {
                        hoverPoint = location
                    }
                case .ended:
                    withAnimation(LiquidGlassPhysics.standardSpring) {
                        hoverPoint = nil
                    }
                }
            }
    }
}

public extension View {
    func liquidGlassSurface(
        cornerRadius: CGFloat = 16,
        elevation: LiquidGlassElevation = .floatingPalette,
        tint: Color? = nil
    ) -> some View {
        modifier(
            LiquidGlassSurfaceModifier(
                cornerRadius: cornerRadius,
                elevation: elevation,
                tintColor: tint
            )
        )
    }
}

// MARK: - Native AppKit Vibrancy Sidebar Container (NSVisualEffectView)

public struct VibrancySidebarContainer<Content: View>: NSViewRepresentable {
    public let material: NSVisualEffectView.Material
    public let blendingMode: NSVisualEffectView.BlendingMode
    private let content: Content

    public init(
        material: NSVisualEffectView.Material = .sidebar,
        blendingMode: NSVisualEffectView.BlendingMode = .behindWindow,
        @ViewBuilder content: () -> Content
    ) {
        self.material = material
        self.blendingMode = blendingMode
        self.content = content()
    }

    public func makeNSView(context: Context) -> NSVisualEffectView {
        let effectView = NSVisualEffectView()
        effectView.material = material
        effectView.blendingMode = blendingMode
        effectView.state = .followsWindowActiveState
        effectView.isEmphasized = true

        let hostingView = NSHostingView(rootView: content)
        hostingView.translatesAutoresizingMaskIntoConstraints = false
        effectView.addSubview(hostingView)

        NSLayoutConstraint.activate([
            hostingView.leadingAnchor.constraint(equalTo: effectView.leadingAnchor),
            hostingView.trailingAnchor.constraint(equalTo: effectView.trailingAnchor),
            hostingView.topAnchor.constraint(equalTo: effectView.topAnchor),
            hostingView.bottomAnchor.constraint(equalTo: effectView.bottomAnchor),
        ])

        context.coordinator.hostingView = hostingView
        return effectView
    }

    public func updateNSView(_ effectView: NSVisualEffectView, context: Context) {
        effectView.material = material
        effectView.blendingMode = blendingMode
        context.coordinator.hostingView?.rootView = content
    }

    public func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    public final class Coordinator {
        var hostingView: NSHostingView<Content>?
    }
}

// MARK: - Refractive Floating Tool Palette (16 Visio Drawing & Diagramming Tools)

public struct LiquidGlassFloatingPalette: View {
    @Binding var selectedTool: VexioCanvasTool
    @Namespace private var activeLensNamespace
    @State private var dragDisplacement: CGSize = .zero
    @State private var committedOffset: CGSize = .zero

    private let paletteColumns = [
        GridItem(.fixed(32), spacing: 4),
        GridItem(.fixed(32), spacing: 4),
    ]

    public init(selectedTool: Binding<VexioCanvasTool>) {
        self._selectedTool = selectedTool
    }

    public var body: some View {
        VStack(spacing: 5) {
            // Subtle tactile grab handle for repositioning over the Metal canvas
            Capsule()
                .fill(Color.secondary.opacity(0.38))
                .frame(width: 24, height: 4)
                .padding(.top, 6)
                .padding(.bottom, 1)
                .help("Drag floating Liquid Glass drawing tool palette")

            LazyVGrid(columns: paletteColumns, spacing: 4) {
                ForEach(VexioCanvasTool.allCases) { tool in
                    let isSelected = (selectedTool == tool)
                    Button {
                        withAnimation(LiquidGlassPhysics.standardSpring) {
                            selectedTool = tool
                        }
                    } label: {
                        ZStack {
                            if isSelected {
                                RoundedRectangle(cornerRadius: 9, style: .continuous)
                                    .fill(
                                        LinearGradient(
                                            colors: [
                                                Color.accentColor.opacity(0.90),
                                                Color.accentColor.opacity(0.70),
                                            ],
                                            startPoint: .topLeading,
                                            endPoint: .bottomTrailing
                                        )
                                    )
                                    .overlay(
                                        RoundedRectangle(cornerRadius: 9, style: .continuous)
                                            .strokeBorder(Color.white.opacity(0.55), lineWidth: 0.9)
                                    )
                                    .shadow(color: Color.accentColor.opacity(0.32), radius: 5, x: 0, y: 2)
                                    .matchedGeometryEffect(id: "ActiveToolGlassLens", in: activeLensNamespace)
                            }

                            Image(systemName: tool.symbolName)
                                .font(.system(size: 13.5, weight: isSelected ? .semibold : .medium))
                                .foregroundStyle(isSelected ? Color.white : Color.primary)
                                .frame(width: 32, height: 30)
                        }
                        .contentShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
                    }
                    .buttonStyle(.plain)
                    .help("\(tool.rawValue) (\(tool.shortcutHint))")
                }
            }
        }
        .padding(.horizontal, 7)
        .padding(.bottom, 8)
        .frame(width: 82)
        .liquidGlassSurface(cornerRadius: 16, elevation: .floatingPalette)
        .offset(
            x: committedOffset.width + dragDisplacement.width,
            y: committedOffset.height + dragDisplacement.height
        )
        .gesture(
            DragGesture()
                .onChanged { value in
                    dragDisplacement = value.translation
                }
                .onEnded { value in
                    withAnimation(LiquidGlassPhysics.standardSpring) {
                        committedOffset = CGSize(
                            width: min(max(committedOffset.width + value.translation.width, -12), 520),
                            height: min(max(committedOffset.height + value.translation.height, -120), 260)
                        )
                        dragDisplacement = .zero
                    }
                }
        )
    }
}

// MARK: - Native Retina Vector Stencil Preview Renderer (Zero WKWebView)

public struct StencilVectorPreviewCanvas: View {
    public let svgMarkup: String
    public let pngDataUrl: String
    public let fallbackSymbol: String

    public init(svgMarkup: String, pngDataUrl: String = "", fallbackSymbol: String) {
        self.svgMarkup = svgMarkup
        self.pngDataUrl = pngDataUrl
        self.fallbackSymbol = fallbackSymbol
    }

    public var body: some View {
        if let nsImg = Self.decodePngDataUrl(pngDataUrl) {
            Image(nsImage: nsImg)
                .resizable()
                .interpolation(.high)
                .antialiased(true)
                .scaledToFit()
                .frame(maxWidth: .infinity, minHeight: 44, maxHeight: 46)
        } else {
            let parsedShapes = Self.extractShapePaths(from: svgMarkup)
            if parsedShapes.isEmpty {
                Image(systemName: fallbackSymbol)
                    .font(.system(size: 21, weight: .regular))
                    .foregroundStyle(Color.accentColor)
                    .frame(maxWidth: .infinity, minHeight: 44)
            } else {
                Canvas { context, size in
                    let srcW: CGFloat = 144.0
                    let srcH: CGFloat = 96.0
                    let scale = min((size.width - 4.0) / srcW, (size.height - 4.0) / srcH)
                    let offsetX = (size.width - srcW * scale) * 0.5
                    let offsetY = (size.height - srcH * scale) * 0.5

                    var transform = CGAffineTransform(translationX: offsetX, y: offsetY)
                    transform = transform.scaledBy(x: scale, y: scale)

                    for item in parsedShapes {
                        let transformed = item.path.applying(transform)
                        if let fill = item.fillColor {
                            context.fill(transformed, with: .color(fill), style: FillStyle(eoFill: true))
                        }
                        if let stroke = item.strokeColor {
                            context.stroke(
                                transformed,
                                with: .color(stroke),
                                style: StrokeStyle(
                                    lineWidth: max(item.strokeWidth * scale, 0.78),
                                    lineCap: .round,
                                    lineJoin: .round
                                )
                            )
                        }
                    }
                }
                .frame(maxWidth: .infinity, minHeight: 46, maxHeight: 46)
            }
        }
    }

    private static func decodePngDataUrl(_ raw: String) -> NSImage? {
        let prefix = "data:image/png;base64,"
        guard raw.hasPrefix(prefix) else { return nil }
        let b64 = String(raw.dropFirst(prefix.count))
        guard let data = Data(base64Encoded: b64), !data.isEmpty else { return nil }
        return NSImage(data: data)
    }

    private struct ParsedStencilPath {
        let path: Path
        let fillColor: Color?
        let strokeColor: Color?
        let strokeWidth: CGFloat
    }

    private static func extractAttribute(_ name: String, in tag: Substring) -> String? {
        let needle = "\(name)=\""
        guard let startRange = tag.range(of: needle) else { return nil }
        let rest = tag[startRange.upperBound...]
        guard let endQuote = rest.firstIndex(of: "\"") else { return nil }
        return String(rest[..<endQuote])
    }

    private static func parseHexColor(_ raw: String?) -> Color? {
        guard let raw else { return nil }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty || trimmed == "none" {
            return nil
        }
        if trimmed.hasPrefix("url(") {
            return Color(red: 0.90, green: 0.95, blue: 1.0)
        }
        var hex = trimmed
        if hex.hasPrefix("#") {
            hex.removeFirst()
        }
        guard hex.count == 6, let rgb = UInt32(hex, radix: 16) else {
            return Color.accentColor.opacity(0.18)
        }
        let r = Double((rgb >> 16) & 0xFF) / 255.0
        let g = Double((rgb >> 8) & 0xFF) / 255.0
        let b = Double(rgb & 0xFF) / 255.0
        return Color(red: r, green: g, blue: b)
    }

    private static func extractShapePaths(from svg: String) -> [ParsedStencilPath] {
        guard let groupRange = svg.range(of: "data-shape-id=") else { return [] }
        let body = svg[groupRange.lowerBound...]
        var results: [ParsedStencilPath] = []

        var cursor = body.startIndex
        while cursor < body.endIndex,
              let pathStart = body[cursor...].range(of: "<path ") {
            let afterPath = body[pathStart.upperBound...]
            guard let tagEnd = afterPath.firstIndex(of: ">") else { break }
            let tagSlice = afterPath[..<tagEnd]
            if let dStr = extractAttribute("d", in: tagSlice) {
                let built = buildSwiftUIPath(from: dStr)
                if !built.isEmpty {
                    let fill = parseHexColor(extractAttribute("fill", in: tagSlice))
                    let stroke = parseHexColor(extractAttribute("stroke", in: tagSlice))
                    let sw = CGFloat(Double(extractAttribute("stroke-width", in: tagSlice) ?? "1.2") ?? 1.2)
                    results.append(
                        ParsedStencilPath(
                            path: built,
                            fillColor: fill,
                            strokeColor: stroke,
                            strokeWidth: sw
                        )
                    )
                }
            }
            cursor = body.index(after: tagEnd)
        }
        return results
    }

    private static func buildSwiftUIPath(from d: String) -> Path {
        var path = Path()
        let tokens = tokenizePath(d)
        var idx = 0
        var current = CGPoint.zero

        func nextCGFloat() -> CGFloat? {
            guard idx < tokens.count, let val = Double(tokens[idx]) else { return nil }
            idx += 1
            return CGFloat(val)
        }

        while idx < tokens.count {
            let cmd = tokens[idx]
            idx += 1
            switch cmd {
            case "M":
                if let x = nextCGFloat(), let y = nextCGFloat() {
                    current = CGPoint(x: x, y: y)
                    path.move(to: current)
                }
            case "L":
                if let x = nextCGFloat(), let y = nextCGFloat() {
                    current = CGPoint(x: x, y: y)
                    path.addLine(to: current)
                }
            case "H":
                if let x = nextCGFloat() {
                    current = CGPoint(x: x, y: current.y)
                    path.addLine(to: current)
                }
            case "V":
                if let y = nextCGFloat() {
                    current = CGPoint(x: current.x, y: y)
                    path.addLine(to: current)
                }
            case "Q":
                if let cx = nextCGFloat(), let cy = nextCGFloat(),
                   let x = nextCGFloat(), let y = nextCGFloat() {
                    current = CGPoint(x: x, y: y)
                    path.addQuadCurve(to: current, control: CGPoint(x: cx, y: cy))
                }
            case "C":
                if let c1x = nextCGFloat(), let c1y = nextCGFloat(),
                   let c2x = nextCGFloat(), let c2y = nextCGFloat(),
                   let x = nextCGFloat(), let y = nextCGFloat() {
                    current = CGPoint(x: x, y: y)
                    path.addCurve(
                        to: current,
                        control1: CGPoint(x: c1x, y: c1y),
                        control2: CGPoint(x: c2x, y: c2y)
                    )
                }
            case "A":
                if let rx = nextCGFloat(), let ry = nextCGFloat(),
                   let _ = nextCGFloat(), let _ = nextCGFloat(),
                   let sweep = nextCGFloat(),
                   let x = nextCGFloat(), let y = nextCGFloat() {
                    let target = CGPoint(x: x, y: y)
                    let midX = (current.x + target.x) * 0.5
                    let midY = (current.y + target.y) * 0.5
                    let dx = target.x - current.x
                    let dy = target.y - current.y
                    let chord = hypot(dx, dy)
                    if chord > 0.001 {
                        let radius = max((rx + ry) * 0.5, chord * 0.5)
                        let sagitta = radius - sqrt(max(radius * radius - (chord * chord * 0.25), 0))
                        let sign: CGFloat = sweep > 0.5 ? -1.0 : 1.0
                        let nx = -dy / chord
                        let ny = dx / chord
                        let ctrl = CGPoint(
                            x: midX + nx * sagitta * 2.0 * sign,
                            y: midY + ny * sagitta * 2.0 * sign
                        )
                        path.addQuadCurve(to: target, control: ctrl)
                    } else {
                        path.addLine(to: target)
                    }
                    current = target
                }
            case "Z", "z":
                path.closeSubpath()
            default:
                break
            }
        }
        return path
    }

    private static func tokenizePath(_ d: String) -> [String] {
        var out: [String] = []
        var buffer = ""
        for ch in d {
            if ch.isWhitespace || ch == "," {
                if !buffer.isEmpty {
                    out.append(buffer)
                    buffer.removeAll(keepingCapacity: true)
                }
            } else if ch.isLetter && ch != "e" && ch != "E" {
                if !buffer.isEmpty {
                    out.append(buffer)
                    buffer.removeAll(keepingCapacity: true)
                }
                out.append(String(ch))
            } else {
                buffer.append(ch)
            }
        }
        if !buffer.isEmpty {
            out.append(buffer)
        }
        return out
    }
}

// MARK: - Specular Liquid Glass Pill Cluster & Action Button

public struct LiquidGlassPillCluster<Content: View>: View {
    private let cornerRadius: CGFloat
    private let content: Content

    public init(cornerRadius: CGFloat = 14, @ViewBuilder content: () -> Content) {
        self.cornerRadius = cornerRadius
        self.content = content()
    }

    public var body: some View {
        HStack(spacing: 8) {
            content
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .liquidGlassSurface(cornerRadius: cornerRadius, elevation: .pillControl)
    }
}

public struct LiquidGlassIconButton: View {
    let systemImage: String
    let title: String
    let isActive: Bool
    let action: () -> Void

    @State private var isPressed = false

    public init(
        systemImage: String,
        title: String,
        isActive: Bool = false,
        action: @escaping () -> Void
    ) {
        self.systemImage = systemImage
        self.title = title
        self.isActive = isActive
        self.action = action
    }

    public var body: some View {
        Button {
            withAnimation(LiquidGlassPhysics.standardSpring) {
                action()
            }
        } label: {
            Image(systemName: systemImage)
                .font(.system(size: 13.5, weight: isActive ? .semibold : .regular))
                .foregroundStyle(isActive ? Color.accentColor : Color.primary)
                .frame(width: 28, height: 26)
                .background(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(isActive ? Color.accentColor.opacity(0.16) : Color.clear)
                )
                .scaleEffect(isPressed ? 0.92 : 1.0)
        }
        .buttonStyle(.plain)
        .help(title)
        .simultaneousGesture(
            DragGesture(minimumDistance: 0)
                .onChanged { _ in
                    if !isPressed {
                        withAnimation(LiquidGlassPhysics.microLensSpring) { isPressed = true }
                    }
                }
                .onEnded { _ in
                    withAnimation(LiquidGlassPhysics.standardSpring) { isPressed = false }
                }
        )
    }
}
