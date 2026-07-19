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

/// The accent-tinted attachments rail at the end of the transcript. It shows
/// every attachment; opening is delegated so inline references and the rail
/// share one handler and preview surface.
struct AttachmentsRail: View {
    let attachments: [DurableArtifact]
    let onOpen: (DurableArtifact) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Label("Attachments", systemImage: "paperclip")
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 20)

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 12) {
                    ForEach(attachments) { attachment in
                        AttachmentCard(attachment: attachment) { onOpen(attachment) }
                    }
                }
                .padding(.horizontal, 20)
            }
        }
    }
}

/// Narrated child branches — each is a full spoken item that plays in the same
/// player. Tapping opens the branch; `< back` returns to this update.
struct NarratedBranchesRail: View {
    let children: [SpokenItem]
    let onOpen: (SpokenItem) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Label("Narrated branches", systemImage: "waveform")
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 20)

            VStack(spacing: 8) {
                ForEach(children) { child in
                    Button { onOpen(child) } label: {
                        HStack(spacing: 12) {
                            Image(systemName: "play.circle.fill")
                                .font(.title2)
                                .foregroundStyle(Color.accentColor)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(child.attach?.label ?? child.subject)
                                    .font(.subheadline.weight(.medium))
                                    .foregroundStyle(.primary)
                                    .lineLimit(1)
                                Text(child.subject)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                            Spacer()
                            Image(systemName: "chevron.forward")
                                .font(.caption).foregroundStyle(.tertiary)
                        }
                        .padding(12)
                        .background(Color.accentColor.opacity(0.10), in: RoundedRectangle(cornerRadius: 14))
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Play narrated branch \(child.attach?.label ?? child.subject)")
                }
            }
            .padding(.horizontal, 20)
        }
    }
}

/// Resolves how an attachment opens: images and text preview in-app; audio and
/// other files hand off to the system.
enum AttachmentOpener {
    static func opensInApp(_ attachment: DurableArtifact) -> Bool {
        switch AttachmentKind(mediaType: attachment.mediaType) {
        case .image, .text: true
        case .audio, .other: false
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
