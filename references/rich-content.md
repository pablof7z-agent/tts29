# Rich content

Read this reference when an update contains local files, Markdown attachment
links, or narrated supporting branches.

## Choose the correct boundary

- Use ordinary `--message` for one spoken item without local attachments.
- Use `--tree` for local file attachments or narrated child items.
- Use raw `ProducerRequest.attachments` only for artifacts that are already
  durable at public HTTPS URLs with known SHA-256, media type, and byte count.
  Do not put local paths there.

Use supporting material when it helps the user inspect evidence or explore the
update in a useful direction: screenshots, mockups, proposals, detailed
findings, or decision context. Keep the root focused on the outcome and organize
narrated branches to fit the material, such as progressive disclosure,
per-decision context, chapters, or a walkthrough. Do not flatten every detail
into the primary root.

## Spoken tree

A `SpokenTree` points to local files. The daemon reads them, synthesizes spoken
nodes, uploads every artifact, publishes the root first, and then publishes
narrated children with durable parent links.

```bash
<skill-dir>/scripts/tts --tree /tmp/tts29-tree.json --agent-id "Codex"
```

Example tree:

```json
{
  "request_id": "architecture-20260720",
  "group_id": "tts",
  "title": "Architecture ready",
  "summary": "The proposal and diagram capture the verified architecture.",
  "message": "/tmp/architecture.md",
  "questions": [],
  "attachments": [{
    "label": "System diagram",
    "file": "/tmp/system.png"
  }, {
    "label": "Detailed reasoning",
    "message": "/tmp/reasoning.md",
    "questions": [],
    "attachments": []
  }]
}
```

Each attachment has exactly one of:

- `file`: upload the bytes as a file attachment on the current item;
- `message`: synthesize and publish a narrated child item.

Give attachments short human labels instead of exposing raw filenames when a
clearer label exists. Use narrated children for explanation worth hearing. Use
file attachments for screenshots, mockups, source files, raw logs, and
diagnostic captures that should remain inspectable without being read aloud.

The optional `questions` array on the root or a narrated child uses the schema
in [asking-questions.md](asking-questions.md). Tree submission does not open an
answer wait.

## Markdown attachment links

Reference a narrated child from its parent Markdown with an exact-label link:

```markdown
Open the [Detailed reasoning](attachment:) for the full tradeoff analysis.
```

The visible label must exactly equal the narrated attachment's `label`. The
child uses that label as its title. File attachments do not need this link.

Keep nesting within three levels and no more than twelve children per node.
Prefer a concise root update with only the branches that materially help the
user. Keep each narrated child focused; split long material into clearly
labeled siblings instead of creating another wall of speech. Do not attach
routine logs, duplicate the main message, or create supplemental files merely
to make an update look substantial.

Use short Markdown headings and lists when they make a narrated transcript
easier to scan. Describe code in plain language and attach the source when the
literal code would be painful to hear.

## Result

A successful tree response contains `root_event_id` and ordered
`child_event_ids`. Preserve all IDs. Publication of the tree still does not
prove that a device played any node.
