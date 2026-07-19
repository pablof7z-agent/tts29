import SwiftUI

/// The read-along transcript block list. Without per-word timing we focus at
/// paragraph granularity: the block estimated to be spoken now is primary, the
/// rest recede. Transitions are opacity-only — honest about the approximation.
/// Scrolling and follow behaviour are owned by the enclosing surface.
struct TranscriptBlocks: View {
    let document: TranscriptDocument
    let focusedID: Int?
    let onSeek: (TranscriptBlock) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            ForEach(document.blocks) { block in
                TranscriptBlockView(block: block, focused: focusedID == nil || block.id == focusedID)
                    .id(block.id)
                    .contentShape(Rectangle())
                    .onTapGesture { onSeek(block) }
            }
        }
        .padding(.horizontal, 20)
    }
}

private struct TranscriptBlockView: View {
    let block: TranscriptBlock
    let focused: Bool

    var body: some View {
        content
            .foregroundStyle(focused ? AnyShapeStyle(.primary) : AnyShapeStyle(.secondary))
            .opacity(focused ? 1 : 0.5)
            .animation(.easeInOut(duration: 0.35), value: focused)
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityAddTraits(focused ? .isSelected : [])
    }

    @ViewBuilder
    private var content: some View {
        switch block.kind {
        case let .heading(level):
            Text(inline(block.text))
                .font(level <= 1 ? .title3.bold() : .headline)
                .padding(.top, 4)
        case .paragraph:
            Text(inline(block.text)).font(.body).lineSpacing(4)
        case .bullet:
            marker("•", block.text)
        case let .ordered(number):
            marker(number, block.text)
        case .quote:
            Text(inline(block.text))
                .font(.body.italic())
                .padding(.leading, 12)
                .overlay(alignment: .leading) {
                    Capsule().fill(Color.accentColor.opacity(0.5)).frame(width: 3)
                }
        case .code:
            Text(block.text)
                .font(.callout.monospaced())
                .padding(12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.primary.opacity(0.06), in: RoundedRectangle(cornerRadius: 10))
        }
    }

    private func marker(_ glyph: String, _ text: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(glyph).font(.body.monospacedDigit()).foregroundStyle(.secondary)
            Text(inline(text)).font(.body).lineSpacing(4)
        }
    }

    private func inline(_ text: String) -> AttributedString {
        let options = AttributedString.MarkdownParsingOptions(
            interpretedSyntax: .inlineOnlyPreservingWhitespace
        )
        return (try? AttributedString(markdown: text, options: options)) ?? AttributedString(text)
    }
}
