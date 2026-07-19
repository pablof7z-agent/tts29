import SwiftUI

/// The item surface — preview and player are one surface. Identity and subject
/// sit above a read-along transcript (with inline attachment links and images)
/// that flows into the attachments rail and questions, with a compact transport
/// fixed at the base. Speed lives in the toolbar; the back control badges how
/// many items would autoplay next.
struct NowPlayingView: View {
    let item: SpokenItem
    let playback: PlaybackController
    private let document: TranscriptDocument
    @State private var following = true
    @State private var previewedAttachment: DurableArtifact?
    @Environment(\.openURL) private var openURL
    @Environment(\.dismiss) private var dismiss

    init(item: SpokenItem, playback: PlaybackController) {
        self.item = item
        self.playback = playback
        document = TranscriptDocument(item.body)
    }

    private var identity: AgentIdentity { AgentIdentity(item) }
    private var isActive: Bool { playback.isActive(item) }
    private var focusedID: Int? { isActive ? playback.focusedBlockID(in: document) : nil }

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    header
                    Text(item.subject).font(.title2.bold()).lineLimit(3).padding(.horizontal, 20)
                    if !document.isEmpty {
                        TranscriptBlocks(
                            document: document,
                            focusedID: focusedID,
                            attachments: item.attachments,
                            onSeek: seek,
                            onOpenAttachment: openAttachment
                        )
                    }
                    if item.hasAttachments {
                        AttachmentsRail(attachments: item.attachments, onOpen: openAttachment)
                    }
                    if item.hasQuestions {
                        QuestionsSection(questions: item.questions, answer: item.answer)
                    }
                    Color.clear.frame(height: 12)
                }
                .padding(.vertical, 12)
            }
            .simultaneousGesture(DragGesture(minimumDistance: 12).onChanged { _ in
                if following { following = false }
            })
            .onChange(of: focusedID) { _, id in
                guard following, let id else { return }
                withAnimation(.easeInOut(duration: 0.4)) { proxy.scrollTo(id, anchor: .center) }
            }
            .overlay(alignment: .bottom) { followingPill(proxy) }
        }
        .safeAreaInset(edge: .bottom) {
            if isActive || playback.selectedItem?.id == item.id {
                TransportCluster(playback: playback)
            } else {
                startButton
            }
        }
        .toolbar { toolbar }
        #if os(iOS)
        .navigationBarTitleDisplayMode(.inline)
        .navigationBarBackButtonHidden(true)
        #endif
        .sheet(item: $previewedAttachment) { AttachmentPreview(attachment: $0) }
        .accessibilityIdentifier("tts29.nowplaying")
    }

    @ToolbarContentBuilder
    private var toolbar: some ToolbarContent {
        #if os(iOS)
        ToolbarItem(placement: .topBarLeading) { backButton }
        #endif
        ToolbarItem(placement: .primaryAction) { SpeedControl(playback: playback) }
    }

    private var backButton: some View {
        let upNext = playback.upNextCount(after: item)
        return Button { dismiss() } label: {
            HStack(spacing: 5) {
                Image(systemName: "chevron.backward").fontWeight(.semibold)
                if upNext > 0 {
                    Text("\(upNext)")
                        .font(.caption2.weight(.bold).monospacedDigit())
                        .padding(.horizontal, 6).padding(.vertical, 1)
                        .background(Color.accentColor, in: Capsule())
                        .foregroundStyle(.white)
                }
            }
        }
        .accessibilityLabel(upNext > 0 ? "Back, \(upNext) queued next" : "Back")
        .accessibilityIdentifier("tts29.nowplaying.back")
    }

    private var header: some View {
        HStack(spacing: 12) {
            AgentAvatar(identity: identity, size: 40)
            VStack(alignment: .leading, spacing: 2) {
                Text(identity.displayName).font(.subheadline.weight(.medium)).lineLimit(1)
                Text(Formatting.timestamp(item.createdDate)).font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            ForEach(item.reactions.prefix(3)) { ReactionChip(reaction: $0) }
        }
        .padding(.horizontal, 20)
    }

    private var startButton: some View {
        Button { playback.toggle(item) } label: {
            Label(playback.isActive(item) && playback.phase == .failed ? "Retry" : "Play",
                  systemImage: "play.fill")
                .font(.headline)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 14)
                .glassSurface(in: Capsule(), tint: .accentColor, interactive: true)
        }
        .buttonStyle(.plain)
        .foregroundStyle(Color.accentColor)
        .disabled(item.playableURL == nil)
        .padding(.horizontal, 20)
        .padding(.bottom, 8)
        .accessibilityIdentifier("tts29.nowplaying.start")
    }

    @ViewBuilder
    private func followingPill(_ proxy: ScrollViewProxy) -> some View {
        if !following, isActive, playback.isPlaying {
            Button {
                withAnimation { following = true }
                if let id = focusedID {
                    withAnimation(.easeInOut(duration: 0.4)) { proxy.scrollTo(id, anchor: .center) }
                }
            } label: {
                Label("Following", systemImage: "waveform")
                    .font(.caption.weight(.medium))
                    .padding(.horizontal, 14).padding(.vertical, 8)
                    .glassCapsule(interactive: true)
            }
            .buttonStyle(.plain)
            .padding(.bottom, 10)
            .transition(.move(edge: .bottom).combined(with: .opacity))
        }
    }

    private func seek(to block: TranscriptBlock) {
        if !isActive { playback.toggle(item) }
        following = true
        playback.seek(toFraction: block.startFraction)
    }

    private func openAttachment(_ attachment: DurableArtifact) {
        if AttachmentOpener.opensInApp(attachment) {
            previewedAttachment = attachment
        } else if let url = URL(string: attachment.url) {
            openURL(url)
        }
    }
}
