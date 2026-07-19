# Unresolved Decisions: Phase 1–3 Packets

Per execution companion §6.1 and comprehensive §8, these are the ONLY known consequential choices still open for Phases 1–3. All other A-items are accepted/frozen.

## Packet A: Foreground/Background Runtime Architecture (Phase 5/7/13)

Decision: Section 8.2 item 6 — how many runtimes, concurrency posture, credential lifecycle.

Current safe baseline: One runtime, one active generation where policy requires it, queue rare overlap.

Approach 1 — Single Runtime with Admission Queue
- Behavior: One Rapid/llama process per preset; overlapping requests queue or 503.
- Pros: simplest security/credential model; clear memory boundaries; easy restart/kill.
- Cons: background job blocks interactive coding if not carefully tuned; no priority scheduling.
- Phase impact: minimal for Phases 1–3; affects Phase 5 policy and Phase 7 UI.

Approach 2 — App-Owned Priority or Separate Background Runtime
- Behavior: separate runtime/port/credential for background jobs; app-level admission/priority.
- Pros: interactive coding protected; background jobs don't starve foreground.
- Cons: credential lifecycle complexity; memory overhead of second runtime; restart coordination.
- Phase impact: Phase 5 policy, Phase 7 credential UI, Phase 13 lifecycle docs.

Recommendation: Approach 1 (single runtime, queue). Defer Approach 2 until Phase 5 workload measurements justify added complexity.

Stop condition: Evidence justifies app-owned priority scheduling or a separate background runtime/port/credential lifecycle.

## Packet B: llama-server Built-in Tools (Phase 7/12/13)

Decision: A44/D26 — enable llama-server Web UI built-in tools?

Current safe baseline: Web UI controls allowed; MCP proxy Off; built-in tools absent.

Approach 1 — Maintain Current Baseline (No Built-in Tools)
- Behavior: Web UI openable/configured; no built-in tools/agents; MCP proxy Off unless explicit Experimental gate.
- Pros: clean security boundary; external MCP use doesn't depend on llama-server proxy.
- Cons: no local agent/scratchpad tools from llama-server.
- Phase impact: Phase 1 hard gate (no unconditional --webui-mcp-proxy), Phase 7 Web UI group.

Approach 2 — Enable Curated Tool Allowlist
- Behavior: small explicit tool allowlist behind Experimental security gate; threat model documented.
- Pros: local scratchpad/reasoning tooling available.
- Cons: security surface area; require allowlist, network design, sandboxing.
- Phase impact: Phase 7 Experimental section, Phase 12 security review.

Recommendation: Approach 1 (maintain baseline). A concrete allowlist/threat model/network design is required before enabling tools.

Stop condition: A concrete allowlist/threat model/network design proposes enabling tools.

## Measurement-Blocked Items (Not Decisions — [escalate→device])

These items are ACCEPTED in principle but require M5 Max measurements. Not decision packets — they resolve by evidence:

- A4/A5/A6: hybrid cache byte budget, response-cache behavior, cache recommendation
- A22: GPU memory utilization calibration
- A35/A41/A42/A48: TurboQuant savings, KV floors, calibration bounds
- A54/A58: memory availability snapshot, wired limit
- §8.3 envelope: all form the `[escalate→device]` measurement bucket

Phase 0 must not force premature answers on these. They enter their owning phases with conservative defaults.
