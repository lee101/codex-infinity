# Tool-Backend Usage Guide

This note summarizes how to drive Codex as a backend for a custom surface (web UI, mobile app, CI bot, etc.) using the same server protocol the TUI uses.

## Start the backend server

Run the app-server sidecar and use the default stdio transport:

```bash
codex app-server --listen stdio://
```

After startup, send one JSON-RPC `initialize` request first, then a matching `initialized` notification. Subsequent calls on the same connection are rejected until this handshake completes.

## Minimal startup flow

1. `thread/start` (or `thread/resume`)
   - create a new conversation or reopen an existing one
   - response includes a `thread` with `threadId`
2. `turn/start`
   - send user input to begin the first model turn
   - response includes the `turn` object
   - notification stream starts with `turn/started`
3. Listen for stream notifications and render them in order

### Important events to subscribe to

- `turn/started`
- `item/started`
- `item/completed`
- `turn/completed`
- `enteredReviewMode`
- `exitedReviewMode`
- `tokenCount` updates (if your UI shows context window / usage)

These are the same events the TUI drives, so you can keep one event parser for both a skin and a custom client.

## Review mode

To run automated review in-band with a thread:

1. Call `review/start` with a review target
2. Watch for:
   - `enteredReviewMode` (review mode begins)
   - review item events (`item/started` / `item/completed` for `enteredReviewMode` / `exitedReviewMode`)
   - final review result message in item output

Because the TUI now queues `/review` requests while a turn is busy, external clients should do the same:

- keep a single in-memory queue per active thread
- when busy, push new `/review` and user-turn work into that queue instead of sending immediately
- when `turn/completed` arrives, send the next queued request
- if you need to cancel an in-flight turn, send `turn/interrupt`

## Interrupting and resumability

Use:

- `turn/interrupt` with `(threadId, turnId)` to cancel the active turn
- on interrupt, expect the active turn to finish with a status like interrupted
- then send the next queued item (user or `/review`) as part of your queue drain policy

### Suggested queue policy

- Serialize outbound commands for one thread.
- Never fire a new `turn/start` or `review/start` while another turn is in-flight.
- Drain queue only on terminal events (`turn/completed`, `turn/aborted`, explicit `turn/interrupt` completion).
- Keep user feedback in sync by mirroring the same state transitions the TUI uses (`enteredReviewMode`/`exitedReviewMode`, running indicator, etc.).
