import SwiftUI

/// A modal preview for an attachment: a zoomable image or a fetched text /
/// Markdown document. Anything else is handed to the system before this opens.
struct AttachmentPreview: View {
    let attachment: DurableArtifact
    @Environment(\.dismiss) private var dismiss
    @Environment(\.openURL) private var openURL

    private var title: String {
        attachment.label ?? URL(string: attachment.url)?.lastPathComponent ?? "Attachment"
    }

    var body: some View {
        NavigationStack {
            Group {
                if AttachmentKind(mediaType: attachment.mediaType) == .image {
                    ZoomableImage(url: URL(string: attachment.url))
                } else {
                    TextDocumentView(
                        url: URL(string: attachment.url),
                        rendersMarkdown: attachment.mediaType.lowercased().contains("markdown")
                    )
                }
            }
            .navigationTitle(title)
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
                ToolbarItem(placement: .cancellationAction) {
                    Button("Open", systemImage: "arrow.up.forward.app") {
                        if let url = URL(string: attachment.url) { openURL(url) }
                    }
                }
            }
        }
    }
}

private struct ZoomableImage: View {
    let url: URL?
    @State private var scale: CGFloat = 1

    var body: some View {
        AsyncImage(url: url) { phase in
            switch phase {
            case let .success(image):
                image.resizable().scaledToFit()
                    .scaleEffect(scale)
                    .gesture(MagnifyGesture()
                        .onChanged { scale = max(1, $0.magnification) }
                        .onEnded { _ in withAnimation(.snappy) { scale = 1 } })
            case .failure:
                ContentUnavailableView("Couldn't load image", systemImage: "photo.badge.exclamationmark")
            default:
                ProgressView()
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct TextDocumentView: View {
    let url: URL?
    let rendersMarkdown: Bool
    @State private var text: String?
    @State private var failed = false

    var body: some View {
        ScrollView {
            if let text {
                Group {
                    if rendersMarkdown, let attributed = try? AttributedString(
                        markdown: text,
                        options: .init(interpretedSyntax: .full)
                    ) {
                        Text(attributed)
                    } else {
                        Text(text).font(.callout.monospaced())
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .textSelection(.enabled)
                .padding(20)
            } else if failed {
                ContentUnavailableView("Couldn't load document", systemImage: "doc.badge.exclamationmark")
                    .padding(.top, 60)
            } else {
                ProgressView().padding(.top, 60)
            }
        }
        .task { await load() }
    }

    private func load() async {
        guard let url else { failed = true; return }
        do {
            let (data, _) = try await URLSession.shared.data(from: url)
            text = String(decoding: data, as: UTF8.self)
        } catch {
            failed = true
        }
    }
}
