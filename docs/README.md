# Quadrant Project Memory

The `docs/` directory is **persistent development memory for Codex and other coding agents**. It exists so a new conversation/window can recover the state of the project without re-reading the entire Git history or rediscovering decisions from source code.

`AGENTS.md` requires agents to read and maintain these files.

## File roles

| File | Purpose | Update style |
|---|---|---|
| `00_PROJECT_MEMORY.md` | Compact stable truth about the project right now | Edit in place, keep short |
| `01_ARCHITECTURE.md` | Detailed architecture/ownership boundaries | Update when structure changes |
| `02_UI_UPSTREAM.md` | wsl-dashboard derivation rules and UI design memory | Update on UI/upstream changes |
| `03_DOMAIN_DATA.md` | Domain model, invariants, DB schema/migrations | Update with domain/storage changes |
| `04_PLATFORM_RUNTIME.md` | Runtime/concurrency/platform contracts | Update with platform/runtime changes |
| `05_DECISIONS.md` | Durable decision log / lightweight ADRs | Append or mark superseded; do not erase history |
| `06_PROGRESS.md` | Current milestone checklist | Update task status |
| `07_BACKLOG.md` | Ordered future work/dependencies | Reorder as priorities change |
| `08_SESSION_HANDOFF.md` | Exact short-term handoff to next Codex window | **Always rewrite/update after substantive work** |
| `09_SOURCE_MAP.md` | Legacy and upstream source -> new Rust/Slint mapping, licenses | Update whenever code is ported/derived |
| `10_LICENSE_RELEASE.md` | GPL, attribution, packaging/release memory | Update with licensing/release changes |

## Memory discipline

### What belongs here

- accepted architecture
- current implementation state
- durable product rules
- unresolved blockers
- exact next steps
- upstream provenance
- migration/schema facts
- commands required to reproduce an unresolved failure

### What does not belong here

- long terminal transcripts
- every commit message
- speculative brainstorming that was never accepted
- duplicate copies of source code
- stale TODO lists left after tasks finish

## Session lifecycle

At session start:

```text
AGENTS.md
  -> SPEC.md
  -> 00_PROJECT_MEMORY
  -> 05_DECISIONS
  -> 06_PROGRESS
  -> 08_SESSION_HANDOFF
  -> subsystem docs
```

At session end:

```text
implementation complete
  -> update decision(s) if needed
  -> update subsystem memory
  -> update progress/backlog
  -> write exact SESSION_HANDOFF
```

The best handoff is short enough to read immediately and precise enough that the next agent can begin work without asking what happened in the previous window.
