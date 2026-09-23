# Session protocol v3 uses transport-local request correlation

Historical decision record. The current rewrite contract is [target.md](../api/target.md).

## Status

Accepted.

This ADR was imported from the original `star_sim` workspace into the standalone `bug_hunter`
repository.

## Decision

`bug_hunter` protocol v3 allows several requests to remain outstanding and correlates each response
through a transport-local numeric request ID. The ID is unique among outstanding requests, carries
no ordering or domain meaning, and is never stored in Session Recordings, Reports, or other durable
artifacts. Requests start in arrival order, while responses may arrive in any order.

Rust keeps typed Command enums. `session::protocol` alone maps them to qualified Wire names such as
`input.keyboard.press` or `tick.warp.stop` plus an `arguments` object.

Malformed or unassignable input produces a `protocol_error` message when the JSONL connection can
continue. A protocol violation does not by itself terminate the Controlled Session. A message with
a known request ID completes that pending request with a protocol failure. Without a known ID,
other pending requests remain open and the Debug Host retains the message as session diagnostics.
If the transport ends, still-pending commands remain unanswered.

## Consequences

Protocol v3 replaces rather than extends the current protocol v2 and its duplicated serialization.
The Debug Host must retain outstanding requests until their responses arrive. Recording must join
each Command with its Outcome before discarding the request ID. The explicit Wire codec costs more
code than nested Serde enums, but gives the persisted contract one owner.
