# Phase 13 Windows/macOS visual parity ledger

Status: provisional; fresh isolated Windows recapture required before visual
acceptance.

## Evidence contract

- Compare release-built captures from the same source commit.
- Use `1440x900` at device scale factor 1 for desktop pairs and the same
  reduced-motion/narrow viewport for responsive pairs.
- Keep Windows artifacts under the ignored `docs/screenshots/artifacts/windows/`
  staging tree; never promote them automatically.
- Compare screenshots by scenario and state, not merely by filename. Record
  dimensions, hashes, missing outputs, and the exact capture command.
- Treat font rasterization, native scrollbars, GPU labels, memory totals, and
  OS-specific controls as expected differences only after confirming that no
  wrapping, clipping, hierarchy, or control-reachability regression exists.

## Current evidence

| Surface | Current result | Disposition |
|---|---|---|
| Settings modal | Windows and macOS shells are both `1440x900`; preliminary inspection shows the Windows navigation/content split is about 15 px wider, with no observed clipping. | Provisional typography/layout observation; recheck after isolated recapture. |
| Appearance palette | Same overall hierarchy and controls; Windows text metrics change the navigation width and card widths slightly. | Provisional; compare computed boxes after recapture. |
| Welcome, dashboard, chat, sidebar | Windows batch inherited the real application home, including an attached server and model cards; macOS and Windows states are not equivalent. | Invalid parity input; recapture both from disposable seeded homes. |
| Rapid-MLX wizard, preset editor, runtime manager, live telemetry | Local Rapid-MLX is Apple-Silicon/macOS-only; Windows scenarios are platform-skipped. | No Windows pair by design. Review macOS Rapid captures separately for product correctness. |

The initial Windows `config`/`core` batch is retained only for debugging and is
not acceptance evidence. The capture harness fix in the current branch passes
an explicit temporary config root and isolated Windows profile variables.

## Required next pass

1. Sync the current branch to Windows and verify the checkout is clean.
2. Run the `config` and `core` capture groups sequentially with the release
   binary and the documented disposable profile command.
3. Confirm no startup log references the real user application home and that
   fresh/seeded fixture state matches the macOS run.
4. Build a same-scenario manifest of Windows/macOS filenames, dimensions,
   hashes, and missing outputs.
5. Inspect high-impact pairs first: welcome, launch grid, settings modal,
   appearance palette, preset editor, wizard review, dashboard, chat, sidebar,
   and narrow/reduced-motion variants.
6. Classify each difference as `pass`, `expected-platform`, `needs-fix`, or
   `blocked-input`. Any wrapping, clipping, hierarchy, contrast, focus, or
   reachability defect is a Phase 13 blocker.
7. Run `scripts/check-unused-screenshots.sh`; promote nothing unless a current
   screenshot is intentionally referenced by documentation.
