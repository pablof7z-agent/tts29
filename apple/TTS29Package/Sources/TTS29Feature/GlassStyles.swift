import SwiftUI

/// Progressive-enhancement Liquid Glass. On iOS 26 / macOS 26 these apply the
/// real dynamic glass material; below that they fall back to a system material
/// so the app still reads correctly. Glass is reserved for floating chrome
/// (mini-player, transport, pills) — in-content emphasis uses tinted fills.
struct GlassBackground<S: Shape>: ViewModifier {
    let shape: S
    var tint: Color?
    var interactive: Bool

    @available(iOS 26.0, macOS 26.0, *)
    private func makeGlass() -> Glass {
        var glass: Glass = .regular
        if let tint { glass = glass.tint(tint) }
        if interactive { glass = glass.interactive() }
        return glass
    }

    @ViewBuilder
    func body(content: Content) -> some View {
        if #available(iOS 26.0, macOS 26.0, *) {
            content.glassEffect(makeGlass(), in: shape)
        } else {
            content
                .background {
                    shape.fill(.regularMaterial)
                        .overlay { if let tint { shape.fill(tint.opacity(0.20)) } }
                }
                .overlay { shape.stroke(.white.opacity(0.10), lineWidth: 0.5) }
        }
    }
}

extension View {
    func glassSurface(
        in shape: some Shape,
        tint: Color? = nil,
        interactive: Bool = false
    ) -> some View {
        modifier(GlassBackground(shape: shape, tint: tint, interactive: interactive))
    }

    func glassCapsule(tint: Color? = nil, interactive: Bool = false) -> some View {
        glassSurface(in: Capsule(), tint: tint, interactive: interactive)
    }

    /// Assigns a morph identity when Liquid Glass is available; a no-op otherwise.
    @ViewBuilder
    func glassMorph(_ id: some Hashable & Sendable, in namespace: Namespace.ID) -> some View {
        if #available(iOS 26.0, macOS 26.0, *) {
            glassEffectID(id, in: namespace)
        } else {
            self
        }
    }
}

/// Groups glass elements so they blend and morph as one piece of material.
struct GlassContainer<Content: View>: View {
    var spacing: CGFloat = 12
    @ViewBuilder var content: Content

    var body: some View {
        if #available(iOS 26.0, macOS 26.0, *) {
            GlassEffectContainer(spacing: spacing) { content }
        } else {
            content
        }
    }
}
