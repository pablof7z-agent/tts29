# Asking questions

Read this reference before publishing questions or requesting an answer wait.

## Contents

- [Request shape](#request-shape)
- [Question rules](#question-rules)
- [Bounded answer wait](#bounded-answer-wait)
- [Result semantics](#result-semantics)

## Request shape

Use `--request` with a complete `ProducerRequest`. Keep signer material only in
the environment; never add it to this JSON.

```bash
<skill-dir>/scripts/tts --request - --wait-seconds 300 <<'JSON'
{
  "request_id": "release-choice-20260720",
  "group_id": "tts",
  "voice": "af_heart",
  "agent_name": "Codex",
  "subject": "Release choice",
  "summary": "One release decision remains before delivery.",
  "body": "The release candidate passed. Please choose the rollout approach.",
  "attachments": [],
  "questions": [{
    "id": "rollout",
    "kind": "single_choice",
    "short_title": "Rollout",
    "title": "Which rollout should I use?",
    "description": "Choose the initial exposure level.",
    "options": [{
      "id": "progressive",
      "title": "Progressive",
      "description": "Start with a narrow cohort."
    }, {
      "id": "all_at_once",
      "title": "All at once",
      "description": "Release to everyone immediately."
    }]
  }]
}
JSON
```

Replace the example request ID with one stable for the actual operation. Use
the configured group unless the user explicitly selects another one.

Make the spoken body provide the outcome and context the user needs before
answering. Do not read the question list aloud or repeat structured question
text in the body.

## Question rules

An item may contain at most three questions.

- Use `single_choice` for exactly one option, `multiple_choice` for several,
  and `freeform` for one text answer.
- Give every question a stable `id`, short tab label in `short_title`, and full
  natural question in `title`.
- Give choice questions one to eight options with stable IDs. Give freeform
  questions no options.
- Keep question and option IDs to 1-64 ASCII letters, digits, `_`, or `-`.
- Use choice questions only when the options represent real alternatives. Use
  freeform when the answer space is genuinely open.
- Keep option titles short. Use option descriptions to explain consequences or
  tradeoffs rather than restating the title.
- Use question descriptions only when they clarify why the decision matters or
  provide context not already present in the spoken body.

Do not add a catch-all “Other” option unless the product decision genuinely
requires it; freeform questions already represent open text.

## Bounded answer wait

Add `--wait-seconds <1-300>` only when observing an immediate answer is useful.
The daemon publishes first, then opens one bounded NMP observation for a valid
answer related to that exact item. The wait never controls publication.

When the answer is not required to continue useful work, omit the wait and
report the item event ID. When it is required, choose the smallest realistic
bound and handle timeout explicitly.

## Result semantics

`answer_wait.status` is one of:

- `answered`: includes the related answer event, author, timestamp, and
  question values;
- `timed_out`: the item remains durably published, but no valid answer arrived
  before the bound;
- `unavailable`: answer observation failed with a code and message;
- `not_requested`: no wait was opened.

Never republish after a timeout. Reuse the same item event for later answer
observation through a supported consumer.
