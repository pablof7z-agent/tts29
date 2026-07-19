import SwiftUI

/// Presents an item's questions and any submitted answer as a record, not a
/// disabled form. Answering from this device is a later phase, so options are
/// shown normally (never greyed) with a quiet "coming soon" note when open.
struct QuestionsSection: View {
    let questions: [Question]
    let answer: AnswerBundle?
    @State private var selection: String

    init(questions: [Question], answer: AnswerBundle?) {
        self.questions = questions
        self.answer = answer
        _selection = State(initialValue: questions.first?.id ?? "")
    }

    private var current: Question? {
        questions.first { $0.id == selection } ?? questions.first
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Label("Questions", systemImage: "questionmark.bubble")
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(.secondary)
                Spacer()
                statusBadge
            }

            if questions.count > 1 {
                Picker("Question", selection: $selection) {
                    ForEach(questions) { Text($0.shortTitle).tag($0.id) }
                }
                .pickerStyle(.segmented)
            }

            if let question = current {
                QuestionDetail(question: question, values: answer?.values(for: question.id) ?? [])
            }

            if answer == nil {
                Label("Answering from this device is coming soon.", systemImage: "hourglass")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(16)
        .background(Color.accentColor.opacity(0.08), in: RoundedRectangle(cornerRadius: 16))
        .padding(.horizontal, 20)
    }

    @ViewBuilder
    private var statusBadge: some View {
        if let answer {
            Text("Answered · \(Formatting.timestamp(Date(timeIntervalSince1970: TimeInterval(answer.createdAt))))")
                .font(.caption.weight(.medium))
                .foregroundStyle(Color.accentColor)
        } else {
            Text("Awaiting reply")
                .font(.caption.weight(.medium))
                .foregroundStyle(.secondary)
        }
    }
}

private struct QuestionDetail: View {
    let question: Question
    let values: [String]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(question.title).font(.headline)
            if let description = question.description, !description.isEmpty {
                Text(description).font(.subheadline).foregroundStyle(.secondary)
            }

            if question.kind == .freeform {
                freeform
            } else {
                ForEach(question.options) { option in
                    OptionRow(
                        option: option,
                        multiple: question.kind == .multipleChoice,
                        selected: values.contains(option.id)
                    )
                }
            }
        }
    }

    @ViewBuilder
    private var freeform: some View {
        if let text = values.first, !text.isEmpty {
            Text(text)
                .font(.body)
                .padding(.leading, 12)
                .overlay(alignment: .leading) {
                    Capsule().fill(Color.accentColor).frame(width: 3)
                }
        } else {
            Text("No response yet").font(.body).foregroundStyle(.tertiary)
        }
    }
}

private struct OptionRow: View {
    let option: QuestionOption
    let multiple: Bool
    let selected: Bool

    private var symbol: String {
        if multiple { selected ? "checkmark.square.fill" : "square" }
        else { selected ? "checkmark.circle.fill" : "circle" }
    }

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Image(systemName: symbol)
                .foregroundStyle(selected ? Color.accentColor : Color.secondary)
            VStack(alignment: .leading, spacing: 2) {
                Text(option.title)
                    .foregroundStyle(selected ? .primary : .secondary)
                if let description = option.description, !description.isEmpty {
                    Text(description).font(.caption).foregroundStyle(.secondary)
                }
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(option.title)\(selected ? ", selected" : "")")
    }
}
