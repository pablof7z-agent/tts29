import SwiftUI

/// The signature expand: a queue row morphs into the item surface via the
/// system zoom navigation transition on iOS 18+. The zoom transition is an
/// iOS capability, so both helpers are no-ops elsewhere and fall back to the
/// standard push.
extension View {
    @ViewBuilder
    func zoomSource(_ id: some Hashable, in namespace: Namespace.ID) -> some View {
        #if os(iOS)
        if #available(iOS 18.0, *) {
            matchedTransitionSource(id: id, in: namespace)
        } else {
            self
        }
        #else
        self
        #endif
    }

    @ViewBuilder
    func zoomDestination(_ id: some Hashable, in namespace: Namespace.ID) -> some View {
        #if os(iOS)
        if #available(iOS 18.0, *) {
            navigationTransition(.zoom(sourceID: id, in: namespace))
        } else {
            self
        }
        #else
        self
        #endif
    }
}
