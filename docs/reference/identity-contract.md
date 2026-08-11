# Local LLM Foundry identity contract

This is the human-readable companion to [`identity-contract.json`](identity-contract.json).
The production mark is the deterministic SVG at
[`assets/brand/token-ingot.svg`](../../assets/brand/token-ingot.svg). The PNG,
ICO, ICNS, tray, package, PWA, and social files are derivatives; none is a
source-of-truth brand master.

| Surface | 2.0 canonical | 2.x compatibility |
| --- | --- | --- |
| Product | Local LLM Foundry | Historical Llama Monitor references remain labeled; current UI uses Foundry. |
| Short name | Foundry | “Monitor” remains valid for technical monitoring actions. |
| Slug | `local-llm-foundry` | Accept `llama-monitor` aliases through 2.x. |
| Rust library namespace | `llama_monitor` | Retained internally through 2.x. |
| Unix/macOS root | `~/.config/local-llm-foundry` | Legacy root is preserved until explicit migration. |
| Windows root | `%APPDATA%\\local-llm-foundry` | Legacy roots are detected and never silently merged. |
| API routes | Existing routes | Stable throughout 2.x. |
| Browser storage | Existing keys | Stable throughout 2.x. |

Brand tokens are named separately from semantic status tokens: Foundry teal
(`--brand-foundry-teal`), deep teal (`--brand-foundry-teal-deep`), ingot copper
(`--brand-ingot-copper`), and forge charcoal (`--brand-forge-charcoal`).
