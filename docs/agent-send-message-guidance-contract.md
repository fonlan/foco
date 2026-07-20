# Agent `send_message` Guidance Delivery Contract

`agent_send_message` always persists its message before attempting live delivery. The tool response includes the existing `delivery` value:

- `guidance`: exactly one active, guidance-accepting Agent run matched the workspace, team, receiving instance, and (when supplied) related task. The message was placed on that run's guidance channel.
- `queued`: no exact live target was available. This includes an idle or not-yet-registered receiver, an identity mismatch, more than one possible target, a completed run, or a channel-close race.

A queued message remains unread and is injected during a later attempt; sending it never wakes an idle Agent model.

A live delivery is not considered consumed merely because it entered the channel. Consumption occurs only when the run durably records a `guidanceApplied` event with `source: "agentMessage"` and the matching message ID. That same SQLite transaction persists the run event, sets `agent_messages.consumed_at`, and appends the `message_consumed` Agent audit event. Replays after consumption are no-ops.

The routing and consumption path is shared by the local runtime and the SSH sidecar's local Agent execution. No main-process broker special case is required, and the `agent_send_message` input schema and SSE event shapes remain unchanged.
