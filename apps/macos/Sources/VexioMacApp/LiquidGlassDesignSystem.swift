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

// MARK: - Refractive Floating Tool Palette

public struct LiquidGlassFloatingPalette: View {
    @Binding var selectedTool: VexioCanvasTool
    @Namespace private var activeLensNamespace
    @State private var dragDisplacement: CGSize = .zero
    @State private var committedOffset: CGSize = .zero

    public init(selectedTool: Binding<VexioCanvasTool>) {
        self._selectedTool = selectedTool
    }

    public var body: some View {
        VStack(spacing: 6) {
            // Subtle tactile grab handle for repositioning over the Metal canvas
            Capsule()
                .fill(Color.secondary.opacity(0.38))
                .frame(width: 22, height: 4)
                .padding(.top, 6)
                .padding(.bottom, 2)
                .help("Drag floating Liquid Glass tool palette")

            ForEach(VexioCanvasTool.allCases) { tool in
                let isSelected = (selectedTool == tool)
                Button {
                    withAnimation(LiquidGlassPhysics.standardSpring) {
                        selectedTool = tool
                    }
                } label: {
                    ZStack {
                        if isSelected {
                            RoundedRectangle(cornerRadius: 11, style: .continuous)
                                .fill(
                                    LinearGradient(
                                        colors: [
                                            Color.accentColor.opacity(0.88),
                                            Color.accentColor.opacity(0.68),
                                        ],
                                        startPoint: .topLeading,
                                        endPoint: .bottomTrailing
                                    )
                                )
                                .overlay(
                                    RoundedRectangle(cornerRadius: 11, style: .continuous)
                                        .strokeBorder(Color.white.opacity(0.55), lineWidth: 0.9)
                                )
                                .shadow(color: Color.accentColor.opacity(0.35), radius: 6, x: 0, y: 3)
                                .matchedGeometryEffect(id: "ActiveToolGlassLens", in: activeLensNamespace)
                        }

                        Image(systemName: tool.symbolName)
                            .font(.system(size: 15, weight: isSelected ? .semibold : .medium))
                            .foregroundStyle(isSelected ? Color.white : Color.primary)
                            .frame(width: 36, height: 34)
                    }
                    .contentShape(RoundedRectangle(cornerRadius: 11, style: .continuous))
                }
                .buttonStyle(.plain)
                .help("\(tool.rawValue) (\(tool.shortcutHint))")
            }
        }
        .padding(.horizontal, 7)
        .padding(.bottom, 8)
        .liquidGlassSurface(cornerRadius: 18, elevation: .floatingPalette)
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
                            width: min(max(committedOffset.width + value.translation.width, -20), 520),
                            height: min(max(committedOffset.height + value.translation.height, -180), 280)
                        )
                        dragDisplacement = .zero
                    }
                }
        )
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
