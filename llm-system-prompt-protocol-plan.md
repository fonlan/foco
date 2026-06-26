# LLM System Prompt Protocol Mapping Plan

## Phase 1: Provider-native system field mapping

- [x] Update `providers/lib.rs::genai_chat_request()` to extract leading contiguous `NeutralChatRole::System` messages.
- [x] Merge extracted system messages with blank-line separators and set them on `genai::ChatRequest.system` via `with_system(...)`.
- [x] Keep non-leading `System` messages inline in `ChatRequest.messages` so runtime-injected system context retains its relative position.
- [x] Preserve existing tool, attachment, reasoning, and tool-state conversion behavior.
- [x] Add focused provider tests for leading system extraction and non-leading system preservation.
- [x] Run provider tests and record the result: `cargo test -p foco-providers` passed, 21 tests.

## Phase 2: XML-structured prompt content

- [ ] Define the XML-like section schema for system prompt content, including `system_prompt`, `skills_instructions`, `memory_context`, `environment_context`, and related runtime sections.
- [ ] Update base system prompt assembly to emit structured plain text inside the provider-native system field.
- [ ] Audit current context roles for skills, prompt files, AGENTS.md, environment context, memories, hook feedback, and task state.
- [ ] Decide which currently user-role context messages should remain user context and which should move to system/developer-level instructions.
- [ ] Escape or fence untrusted user-provided content to avoid accidental XML tag boundary confusion.
- [ ] Add prompt assembly tests or snapshot-style assertions for the structured sections.
- [ ] Run app/provider tests and document any intentional prompt behavior changes.

## Notes

- API role and provider-native fields define instruction priority; XML tags only structure content inside those fields.
- Phase 1 is a protocol-shape fix and should not change prompt assembly semantics beyond using `ChatRequest.system` for leading system messages.
- Phase 2 is a prompt behavior change and should be evaluated separately.
