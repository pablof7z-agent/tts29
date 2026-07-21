import SwiftUI

struct QuestionsSection: View {
    let questions: [Question]
    let answer: AnswerBundle?
    let identity: IdentitySnapshot
    let submission: AnswerSubmission?
    let onLogin: () -> Void
    let onSubmit: ([QuestionAnswer]) -> Void
    @State private var selectedQuestion: String
    @State private var choices: [String: Set<String>] = [:]
    @State private var freeform: [String: String] = [:]

    init(
        questions: [Question],
        answer: AnswerBundle?,
        identity: IdentitySnapshot,
        submission: AnswerSubmission?,
        onLogin: @escaping () -> Void,
        onSubmit: @escaping ([QuestionAnswer]) -> Void
    ) {
        self.questions = questions
        self.answer = answer
        self.identity = identity
        self.submission = submission
        self.onLogin = onLogin
        self.onSubmit = onSubmit
        _selectedQuestion = State(initialValue: questions.first?.id ?? "")
    }

    private var current: Question? {
        questions.first { $0.id == selectedQuestion } ?? questions.first
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            header
            if questions.count > 1 {
                Picker("Question", selection: $selectedQuestion) {
                    ForEach(questions) { Text($0.shortTitle).tag($0.id) }
                }
                .pickerStyle(.segmented)
            }
            if let question = current {
                if let answer {
                    AnsweredQuestion(question: question, values: answer.values(for: question.id))
                } else if identity.phase == .signedIn {
                    editable(question)
                } else {
                    signedOutPrompt
                }
            }
            submissionStatus
            if answer == nil, identity.phase == .signedIn {
                submitButton
            }
        }
        .padding(16)
        .background(Color.accentColor.opacity(0.08), in: RoundedRectangle(cornerRadius: 16))
        .padding(.horizontal, 20)
    }

    private var header: some View {
        HStack {
            Label("Questions", systemImage: "questionmark.bubble")
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(.secondary)
            Spacer()
            if let answer {
                Text("Answered · \(Formatting.timestamp(answerDate(answer)))")
                    .font(.caption.weight(.medium))
                    .foregroundStyle(Color.accentColor)
            } else {
                Text("Awaiting reply")
                    .font(.caption.weight(.medium))
                    .foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder
    private func editable(_ question: Question) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            questionHeader(question)
            switch question.kind {
            case .freeform:
                TextEditor(text: freeformBinding(question.id))
                    .frame(minHeight: 84)
                    .padding(8)
                    .background(.background, in: RoundedRectangle(cornerRadius: 10))
                    .accessibilityIdentifier("tts29.answer.freeform.\(question.id)")
            case .singleChoice, .multipleChoice:
                ForEach(question.options) { option in
                    Button {
                        toggle(option.id, for: question)
                    } label: {
                        OptionRow(
                            option: option,
                            multiple: question.kind == .multipleChoice,
                            selected: choices[question.id, default: []].contains(option.id)
                        )
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("tts29.answer.option.\(question.id).\(option.id)")
                }
            }
        }
    }

    private var signedOutPrompt: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Log in with your Nostr key to answer as yourself.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
            Button("Log In to Answer", systemImage: "key.fill", action: onLogin)
                .accessibilityIdentifier("tts29.answer.login")
        }
    }

    @ViewBuilder
    private var submissionStatus: some View {
        if let submission {
            switch submission.phase {
            case .sending:
                Label("Publishing answer…", systemImage: "arrow.up.circle")
                    .foregroundStyle(.secondary)
            case .published where answer == nil:
                Label("Answer published", systemImage: "checkmark.circle.fill")
                    .foregroundStyle(.green)
            case .failed:
                Label(submission.error ?? "The answer could not be published.",
                      systemImage: "exclamationmark.triangle.fill")
                    .foregroundStyle(.red)
            default:
                EmptyView()
            }
        }
    }

    private var submitButton: some View {
        Button {
            onSubmit(draftAnswers)
        } label: {
            Label("Send Answer", systemImage: "paperplane.fill")
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent)
        .disabled(draftAnswers.isEmpty || submission?.phase == .sending)
        .accessibilityIdentifier("tts29.answer.submit")
    }

    private var draftAnswers: [QuestionAnswer] {
        questions.compactMap { question in
            let values: [String]
            if question.kind == .freeform {
                let value = freeform[question.id, default: ""]
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                values = value.isEmpty ? [] : [value]
            } else {
                let selected = choices[question.id, default: []]
                values = question.options.map(\.id).filter(selected.contains)
            }
            return values.isEmpty ? nil : QuestionAnswer(questionId: question.id, values: values)
        }
    }

    private func toggle(_ optionID: String, for question: Question) {
        if question.kind == .singleChoice {
            choices[question.id] = [optionID]
        } else if choices[question.id, default: []].contains(optionID) {
            choices[question.id]?.remove(optionID)
        } else {
            choices[question.id, default: []].insert(optionID)
        }
    }

    private func freeformBinding(_ questionID: String) -> Binding<String> {
        Binding(
            get: { freeform[questionID, default: ""] },
            set: { freeform[questionID] = $0 }
        )
    }
}

private struct AnsweredQuestion: View {
    let question: Question
    let values: [String]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            questionHeader(question)
            if question.kind == .freeform {
                Text(values.first ?? "No response")
                    .font(.body)
                    .padding(.leading, 12)
                    .overlay(alignment: .leading) {
                        Capsule().fill(Color.accentColor).frame(width: 3)
                    }
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
}

private struct OptionRow: View {
    let option: QuestionOption
    let multiple: Bool
    let selected: Bool

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Image(systemName: symbol)
                .foregroundStyle(selected ? Color.accentColor : Color.secondary)
            VStack(alignment: .leading, spacing: 2) {
                Text(option.title).foregroundStyle(selected ? .primary : .secondary)
                if let description = option.description, !description.isEmpty {
                    Text(description).font(.caption).foregroundStyle(.secondary)
                }
            }
        }
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(option.title)\(selected ? ", selected" : "")")
    }

    private var symbol: String {
        if multiple { selected ? "checkmark.square.fill" : "square" }
        else { selected ? "checkmark.circle.fill" : "circle" }
    }
}

private func questionHeader(_ question: Question) -> some View {
    VStack(alignment: .leading, spacing: 4) {
        Text(question.title).font(.headline)
        if let description = question.description, !description.isEmpty {
            Text(description).font(.subheadline).foregroundStyle(.secondary)
        }
    }
}

private func answerDate(_ answer: AnswerBundle) -> Date {
    Date(timeIntervalSince1970: TimeInterval(answer.createdAt))
}
