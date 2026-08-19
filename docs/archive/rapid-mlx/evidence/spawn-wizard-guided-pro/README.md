# Spawn Wizard Guided/Pro evidence

This directory contains the durable, checked-in evidence packet for the archived
Spawn Wizard Guided/Pro completion effort.

## Durable evidence

- `fixture-freeze.json` — representative fixture identities and metadata states.
- `capture-inventory.json` — registered capture scenarios and the historical G0
  output inventory. G0 entries are diagnostic history, not current screenshot
  acceptance.
- `route-inventory.json` — wizard-used API routes and their authentication
  requirements.
- `phase-0-receipt.md` — initial repository, harness, and evidence-freeze receipt.
- `phase-3-inference-inventory.md` — model-property provenance audit and closure
  boundary for the active wizard authority.

The raw `.log` files that may appear beside this packet are local capture
diagnostics and remain ignored by Git; they are not archival acceptance evidence.
Fresh screenshot receipts and promoted screenshots live under
`docs/screenshots/artifacts/` and `docs/screenshots/`, respectively.
