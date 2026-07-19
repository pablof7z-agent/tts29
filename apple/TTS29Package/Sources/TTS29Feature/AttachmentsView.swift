import SwiftUI

enum AttachmentKind {
    case image, text, audio, other

    init(mediaType: String) {
        let type = mediaType.lowercased()
        if type.hasPrefix("image/") { self = .image }
        else if type.hasPrefix("audio/") { self = .audio }
        else if type.hasPrefix("text/") || type.contains("markdown") || type.contains("json") { self = .text }
        else { self = .other }
    }

    var symbol: String {
        switch self {
        case .image: "photo"
        case .text: "doc.text"
        case .audio: "waveform"
        case .other: "doc"
        }
    }
}

/// The accent-tinted attachments rail at the end of the transcript. Images and
/// text open in-app; audio and other files hand off to the system.
struct AttachmentsRail: View {
    let attachments: [DurableArtifact]
    @State private var preview: DurableArtifact?
    @Environment(\.openURL) private var openURL

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Label("Attachments", systemImage: "paperclip")
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 20)

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 12) {
                    ForEach(attachments) { attachment in
                        AttachmentCard(attachment: attachment) { open(attachment) }
                    }
                }
                .padding(.horizontal, 20)
            }
        }
        .sheet(item: $preview) { AttachmentPreview(attachment: $0) }
    }

    private func open(_ attachment: DurableArtifact) {
        switch AttachmentKind(mediaType: attachment.mediaType) {
        case .image, .text:
            preview = attachment
        case .audio, .other:
            if let url = URL(string: attachment.url) { openURL(url) }
        }
    }
}

private struct AttachmentCard: View {
    let attachment: DurableArtifact
    let action: () -> Void

    private var kind: AttachmentKind { AttachmentKind(mediaType: attachment.mediaType) }
    private var title: String {
        attachment.label ?? URL(string: attachment.url)?.lastPathComponent ?? "Attachment"
    }

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 8) {
                if kind == .image {
                    AsyncImage(url: URL(string: attachment.url)) { image in
                        image.resizable().aspectRatio(contentMode: .fill)
                    } placeholder: {
                        Image(systemName: "photo").font(.title2).foregroundStyle(.secondary)
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                    .frame(height: 74)
                    .frame(maxWidth: .infinity)
                    .clipped()
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                } else {
                    Image(systemName: kind.symbol)
                        .font(.title2)
                        .foregroundStyle(Color.accentColor)
                        .frame(height: 74)
                        .frame(maxWidth: .infinity)
                }
                Text(title)
                    .font(.caption.weight(.medium))
                    .lineLimit(1)
                    .foregroundStyle(.primary)
                Text(Formatting.byteCount(attachment.byteCount))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .padding(10)
            .frame(width: 148)
            .background(Color.accentColor.opacity(0.12), in: RoundedRectangle(cornerRadius: 14))
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(title), \(Formatting.byteCount(attachment.byteCount))")
    }
}
