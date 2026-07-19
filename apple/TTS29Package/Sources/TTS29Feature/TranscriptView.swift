import SwiftUI

/// The read-along transcript block list. Blocks focus at paragraph granularity
/// (opacity-only, since there is no per-word timing). Inline `[label](attachment:)`
/// references become tappable links, and referenced images render inline —
/// while the full attachments rail still appears at the end.
struct TranscriptBlocks: View {
    let document: TranscriptDocument
    let focusedID: Int?
    let attachments: [DurableArtifact]
    let onSeek: (TranscriptBlock) -> Void
    let onOpenAttachment: (DurableArtifact) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            ForEach(document.blocks) { block in
                TranscriptBlockView(
                    block: block,
                    focused: focusedID == nil || block.id == focusedID,
                    attachments: attachments,
                    onOpenAttachment: onOpenAttachment
                )
                .id(block.id)
                .contentShape(Rectangle())
                .onTapGesture { onSeek(block) }
            }
        }
        .padding(.horizontal, 20)
        .environment(\.openURL, OpenURLAction { url in
            guard url.scheme == AttachmentLink.scheme,
                  let index = Int(url.lastPathComponent),
                  attachments.indices.contains(index) else {
                return .systemAction
            }
            onOpenAttachment(attachments[index])
            return .handled
        })
    }
}

private struct TranscriptBlockView: View {
    let block: TranscriptBlock
    let focused: Bool
    let attachments: [DurableArtifact]
    let onOpenAttachment: (DurableArtifact) -> Void

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
            textWithInlineImages(block.text, font: .body)
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
            textWithInlineImages(text, font: .body)
        }
    }

    /// Renders text (with inline attachment links) followed by any image
    /// attachments it references, shown inline.
    @ViewBuilder
    private func textWithInlineImages(_ text: String, font: Font) -> some View {
        let referenced = AttachmentLink.referencedImages(in: text, attachments: attachments)
        VStack(alignment: .leading, spacing: 10) {
            Text(inline(text)).font(font).lineSpacing(4).tint(.accentColor)
            ForEach(referenced) { image in
                InlineAttachmentImage(attachment: image) { onOpenAttachment(image) }
            }
        }
    }

    private func inline(_ text: String) -> AttributedString {
        let rewritten = AttachmentLink.rewrite(text, attachments: attachments)
        let options = AttributedString.MarkdownParsingOptions(
            interpretedSyntax: .inlineOnlyPreservingWhitespace
        )
        return (try? AttributedString(markdown: rewritten, options: options)) ?? AttributedString(text)
    }
}

private struct InlineAttachmentImage: View {
    let attachment: DurableArtifact
    let onOpen: () -> Void

    var body: some View {
        Button(action: onOpen) {
            AsyncImage(url: URL(string: attachment.url)) { image in
                image.resizable().scaledToFit()
            } placeholder: {
                RoundedRectangle(cornerRadius: 12)
                    .fill(Color.secondary.opacity(0.12))
                    .frame(height: 160)
                    .overlay(ProgressView())
            }
            .frame(maxWidth: .infinity)
            .frame(maxHeight: 240)
            .clipShape(RoundedRectangle(cornerRadius: 12))
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Image attachment \(attachment.label ?? "")")
    }
}

/// Encodes and resolves inline `[label](attachment:)` references.
enum AttachmentLink {
    static let scheme = "ttsattach"

    /// Rewrites `[label](attachment:)` whose label matches an attachment into a
    /// resolvable custom-scheme link; unmatched references are left untouched.
    static func rewrite(_ text: String, attachments: [DurableArtifact]) -> String {
        matches(in: text).reversed().reduce(text) { current, match in
            guard let index = index(ofLabel: match.label, in: attachments) else { return current }
            let replacement = "[\(match.label)](\(scheme)://a/\(index))"
            return (current as NSString).replacingCharacters(in: match.range, with: replacement)
        }
    }

    static func referencedImages(in text: String, attachments: [DurableArtifact]) -> [DurableArtifact] {
        var seen = Set<Int>()
        var images: [DurableArtifact] = []
        for match in matches(in: text) {
            guard let index = index(ofLabel: match.label, in: attachments),
                  !seen.contains(index),
                  AttachmentKind(mediaType: attachments[index].mediaType) == .image else { continue }
            seen.insert(index)
            images.append(attachments[index])
        }
        return images
    }

    private static func index(ofLabel label: String, in attachments: [DurableArtifact]) -> Int? {
        attachments.firstIndex { $0.label == label }
    }

    private struct Match { let range: NSRange; let label: String }

    private static func matches(in text: String) -> [Match] {
        guard let regex = try? NSRegularExpression(pattern: "\\[([^\\]]+)\\]\\(attachment:\\)") else {
            return []
        }
        let ns = text as NSString
        return regex.matches(in: text, range: NSRange(location: 0, length: ns.length)).map {
            Match(range: $0.range, label: ns.substring(with: $0.range(at: 1)))
        }
    }
}
