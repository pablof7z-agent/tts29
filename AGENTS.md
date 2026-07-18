# TTS29 contributor rules

Every non-trivial change must start from a GitHub issue describing the user or
product need. Deliver changes through a pull request and merge that pull request
before considering the work complete.

TTS29 consumes NMP through supported public APIs. Do not modify, fork, patch, or
reach into NMP mechanism crates to make application work pass.

The app-specific Rust kernel owns product state, protocol interpretation,
ordering, policy, persistence decisions, and lifecycle. Apple code renders
bounded projections, dispatches semantic actions, and executes Apple platform
capabilities. It must not implement Nostr, retry, routing, or product policy.

Keep hand-maintained code files below 300 lines when practical and below 600
lines without exception. Generated files and machine-produced project metadata
are exempt.

Use XcodeBuildMCP CLI commands for Apple builds, tests, simulator runs, and UI
inspection. Do not use raw xcodebuild, xcrun, or simctl commands.
