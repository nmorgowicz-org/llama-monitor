# Test Coverage Analysis for Implementation Plan (Tasks 1.1-4.4)

**Date:** 2026-04-20  
**Status:** Comprehensive gap analysis

---

## Executive Summary

| Task | Has Tests? | Test Name | Coverage Status |
|------|-----------|-----------|-----------------|
| **Phase 1** | | | |
| 1.1 MetricsCapabilities struct | ❌ NO | - | **MISSING** |
| 1.2 AvailabilityReason enum | ❌ NO | - | **MISSING** |
| 1.3 AppState capability fields | ❌ NO | - | **MISSING** |
| 1.4 Capability calculation logic | ❌ NO | - | **MISSING** |
| 1.5 AppState::new() initialization | ❌ NO | - | **MISSING** |
| 1.6 Capability API endpoint | ❌ NO | - | **MISSING** |
| 1.7 WS availability payload | ❌ NO | - | **MISSING** |
| 1.8 Endpoint locality helper | ❌ NO | - | **MISSING** |
| **Phase 2** | | | |
| 2.1 Endpoint health strip (UI) | ❌ NO | - | **MISSING** |
| 2.2 Dashboard capability handling | ❌ NO | - | **MISSING** |
| 2.3 Capability-aware empty states | ❌ NO | - | **MISSING** |
| 2.4 Severity indicators (CSS) | ❌ NO | - | **MISSING** |
| 2.5 Context usage display | ❌ NO | - | **MISSING** |
| 2.6 Mode badge (UI) | ❌ NO | - | **MISSING** |
| **Phase 3** | | | |
| 3.1 Compact HTML structure | ❌ NO | - | **MISSING** |
| 3.2 Compact.js capability handling | ❌ NO | - | **MISSING** |
| 3.3 Tray auto-height logic | ❌ NO | - | **MISSING** |
| 3.4 Tray tooltip endpoint info | ❌ NO | - | **MISSING** |
| **Phase 4** | | | |
| 4.1 CLI flags (headless/no-tray) | ❌ NO | - | **MISSING** |
| 4.2 should_start_tray() logic | ❌ NO | - | **MISSING** |
| 4.3 Mode log messages | ❌ NO | - | **MISSING** |
| 4.4 Tray startup failure message | ❌ NO | - | **MISSING** |

**Overall: 0/28 tasks have tests**

---

## Detailed Analysis

### Phase 1: Backend Capability Model

#### Task 1.1: MetricsCapabilities struct
**File:** `src/state.rs:1`  
**Status:** ✅ IMPLEMENTED  
**Test Coverage:** ❌ MISSING

**Issues:**
- No unit tests for struct construction
- No serialization/deserialization tests
- No field validation tests

**Required tests:**
```rust
#[test]
fn metrics_capabilities_default_values() { ... }
#[test]
fn metrics_capabilities_serialization() { ... }
#[test]
fn metrics_capabilities_deserialization() { ... }
```

---

#### Task 1.2: AvailabilityReason enum
**File:** `src/state.rs:1`  
**Status:** ✅ IMPLEMENTED  
**Test Coverage:** ❌ MISSING

**Issues:**
- No tests for each variant
- No serde serialization tests
- No variant comparison tests

**Required tests:**
```rust
#[test]
fn availability_reason_variants() { ... }
#[test]
fn availability_reason_serialization() { ... }
#[test]
fn availability_reason_deserialization() { ... }
```

---

#### Task 1.3: AppState capability fields
**File:** `src/state.rs:1`  
**Status:** ✅ IMPLEMENTED  
**Test Coverage:** ❌ MISSING

**Issues:**
- No tests for field initialization
- No tests for Arc<Mutex<>> wrapper accessibility
- No concurrent access tests

**Required tests:**
```rust
#[test]
fn app_state_initializes_capabilities() { ... }
#[test]
fn app_state_capabilities_are_thread_safe() { ... }
```

---

#### Task 1.4: Capability calculation logic
**File:** `src/state.rs:1`  
**Status:** ✅ IMPLEMENTED  
**Test Coverage:** ❌ MISSING

**Issues:**
- No tests for `calculate_capabilities()` logic paths
- No tests for `calculate_availability_reasons()` return values
- Missing scenarios:
  - Local spawn → all metrics available
  - Local attach → inference only
  - Remote attach → inference only
  - Headless mode → tray unavailable

**Required tests:**
```rust
#[test]
fn calculate_capabilities_local_spawn() { ... }
#[test]
fn calculate_capabilities_local_attach() { ... }
#[test]
fn calculate_capabilities_remote_attach() { ... }
#[test]
fn calculate_capabilities_headless() { ... }

#[test]
fn calculate_availability_reasons_system() { ... }
#[test]
fn calculate_availability_reasons_gpu() { ... }
#[test]
fn calculate_availability_reasons_cpu_temp() { ... }
```

---

#### Task 1.5: AppState::new() initialization
**File:** `src/state.rs:1`  
**Status:** ✅ IMPLEMENTED  
**Test Coverage:** ❌ MISSING

**Issues:**
- No tests for default capability values
- No tests for endpoint_kind initialization
- No tests for session_kind and tray_mode defaults

**Required tests:**
```rust
#[test]
fn state_new_initializes_capabilities() { ... }
#[test]
fn state_new_sets_default_endpoint_kind() { ... }
#[test]
fn state_new_sets_default_session_kind() { ... }
#[test]
fn state_new_sets_default_tray_mode() { ... }
```

---

#### Task 1.6: Capability API endpoint
**File:** `src/web/api.rs:1`  
**Status:** ✅ IMPLEMENTED  
**Test Coverage:** ❌ MISSING

**Issues:**
- No unit tests for `api_get_capabilities()`
- No integration tests for `/api/capabilities` route
- No response format validation

**Required tests:**
```rust
#[test]
fn api_get_capabilities_returns_json() { ... }
#[test]
fn api_get_capabilities_includes_all_fields() { ... }
#[test]
fn api_get_capabilities_format() { ... }
```

---

#### Task 1.7: WS availability payload
**File:** `src/web/ws.rs:1`  
**Status:** ✅ IMPLEMENTED  
**Test Coverage:** ❌ MISSING

**Issues:**
- No tests for WebSocket message payload structure
- No tests for capabilities in WS messages
- No tests for availability reasons in WS messages

**Required tests:**
```rust
#[test]
fn ws_capabilities_payload_structure() { ... }
#[test]
fn ws_availability_reasons_included() { ... }
```

---

#### Task 1.8: Endpoint locality helper
**File:** `src/web/api.rs:1`  
**Status:** ✅ IMPLEMENTED  
**Test Coverage:** ❌ MISSING

**Issues:**
- No unit tests for `get_endpoint_kind()`
- No tests for edge cases (empty string, invalid URLs)

**Required tests:**
```rust
#[test]
fn get_endpoint_kind_local() { ... }
#[test]
fn get_endpoint_kind_remote() { ... }
#[test]
fn get_endpoint_kind_edge_cases() { ... }
```

---

### Phase 2: UI Updates

#### Tasks 2.1-2.6
**Status:** ✅ IMPLEMENTED (UI files)  
**Test Coverage:** ❌ MISSING

**Issues:**
- **No browser-based tests** for UI rendering
- **No visual QA tests** for capability-aware rendering
- Missing test files:
  - `tests/ui/capability-rendering.test.js`
  - `tests/ui/endpoint-health.test.js`
  - `tests/ui/tray-compact.test.js`

**Required tests (javascript):**
```javascript
test('GPU section hidden for remote endpoint', async () => { ... });
test('CPU temp shows sensor unavailable when missing', async () => { ... });
test('Endpoint health strip shows correct mode', async () => { ... });
test('Mode badge shows Spawn vs Attach', async () => { ... });
test('Severity indicators applied correctly', async () => { ... });
```

---

### Phase 3: Tray Updates

#### Tasks 3.1-3.4
**Status:** ✅ IMPLEMENTED  
**Test Coverage:** ❌ MISSING

**Issues:**
- No tests for compact HTML structure
- No tests for capability-based section visibility
- No tests for tray auto-height calculation
- No tests for tray tooltip endpoint info

**Required tests (Rust + JS):**
```rust
#[test]
fn tray_auto_height_includes_visible_sections() { ... }
#[test]
fn tray_auto_height_clamps_min_max() { ... }

#[test]
fn tray_tooltip_shows_endpoint_mode() { ... }
```

```javascript
test('Compact tray hides GPU for remote', async () => { ... });
test('Compact tray hides CPU/RAM when unavailable', async () => { ... });
```

---

### Phase 4: CLI Headless Mode

#### Task 4.1: CLI flags
**File:** `src/cli.rs:1`  
**Status:** ✅ IMPLEMENTED  
**Test Coverage:** ❌ MISSING

**Issues:**
- No tests for `--headless` flag parsing
- No tests for `--no-tray` flag parsing
- No tests for combined flags

**Required tests:**
```rust
#[test]
fn cli_headless_flag_parsed() { ... }
#[test]
fn cli_no_tray_flag_parsed() { ... }
#[test]
fn cli_flags_combined() { ... }
```

---

#### Task 4.2: should_start_tray() logic
**File:** `src/main.rs:1`  
**Status:** ✅ IMPLEMENTED  
**Test Coverage:** ❌ MISSING

**Issues:**
- No tests for `--headless` override
- No tests for `--no-tray` override
- No tests for Linux DISPLAY detection
- No tests for macOS/Windows defaults

**Required tests:**
```rust
#[test]
fn should_start_tray_headless_override() { ... }
#[test]
fn should_start_tray_no_tray_override() { ... }

#[cfg(target_os = "linux")]
#[test]
fn should_start_tray_linux_with_display() { ... }

#[cfg(target_os = "linux")]
#[test]
fn should_start_tray_linux_without_display() { ... }

#[cfg(not(target_os = "linux"))]
#[test]
fn should_start_tray_non_linux_default() { ... }
```

---

#### Task 4.3: Mode log messages
**File:** `src/main.rs:1`  
**Status:** ✅ IMPLEMENTED  
**Test Coverage:** ❌ MISSING

**Issues:**
- No tests for log output verification
- No tests for different mode messages

**Required tests:**
```rust
#[test]
fn log_headless_mode_message() { ... }
#[test]
fn log_no_tray_message() { ... }
#[test]
fn log_tray_enabled_message() { ... }
#[test]
fn log_tray_disabled_message() { ... }
```

---

#### Task 4.4: Tray startup failure message
**File:** `src/main.rs:1`  
**Status:** ✅ IMPLEMENTED  
**Test Coverage:** ❌ MISSING

**Issues:**
- No tests for tray failure scenario
- No tests for error message format
- Requires mock/failure simulation

**Required tests:**
```rust
#[test]
fn tray_failure_logs_warning() { ... }
#[test]
fn tray_failure_continues_headless() { ... }
```

---

## Consolidation Opportunities

### Potential Test Merges

1. **MetricsCapabilities + AvailabilityReason tests**
   - Can share serde test helpers
   - Can consolidate serialization/deserialization into single test module

2. **AppState::new() + Capability initialization**
   - Can combine into single `state_initialization` test module

3. **API endpoint + WS payload tests**
   - Can share test fixtures for capability JSON structure
   - Can use common response validation helpers

4. **CLI flag parsing tests**
   - All 4 CLI tasks (4.1-4.4) can use same test setup
   - Can consolidate into `tests/cli/mod.rs`

---

## Priority Gaps (Critical)

### Must-Have Tests (Block Release)

1. **Task 1.4: Capability calculation** - Core business logic
   - Missing 4 critical scenarios per acceptance criteria
   
2. **Task 4.2: should_start_tray()** - CLI behavior critical
   - Linux DISPLAY detection logic untested
   
3. **Task 1.6: Capability API endpoint** - Public API
   - No validation of JSON response format

4. **Task 4.3: Mode log messages** - User visibility
   - Cannot verify correct messages shown

---

## New Feature Gaps (Phase 1-4)

### MetricsCapabilities
- ❌ Default values not tested
- ❌ Serialization not validated
- ❌ Field access not verified

### AvailabilityReason
- ❌ All variants not exhaustively tested
- ❌ JSON mapping not validated

### CLI Flags
- ❌ `--headless` parsing untested
- ❌ `--no-tray` parsing untested
- ❌ Flag combination behavior untested
- ❌ Override logic not verified

### Capability API Endpoint
- ❌ Route registration not tested
- ❌ Response structure not validated
- ❌ Empty state handling not tested

### Tray Auto-Height
- ❌ Visible section counting not tested
- ❌ Min/max clamping not tested
- ❌ Remote vs local height差异 not verified

---

## Recommendations

### Immediate Actions

1. **Create test directory structure:**
   ```
   tests/
   ├── unit/
   │   ├── state.rs
   │   ├── cli.rs
   │   ├── web_api.rs
   │   └── web_ws.rs
   ├── integration/
   │   └── capabilities.rs
   └── ui/
       └── capability-rendering.test.js
   ```

2. **Add unit tests for Phase 1 core logic** (Tasks 1.1-1.8)
   - Focus on capability calculation logic first
   - Add CLI flag tests for Tasks 4.1-4.4
   - Add API endpoint tests for Task 1.6

3. **Add browser-based UI tests** (Tasks 2.1-3.4)
   - Add `tests/ui/` directory
   - Use Jest or similar for JS tests
   - Test capability-based hiding/showing

4. **Add integration tests** (Task 6.1 from plan)
   - Create `tests/integration/capabilities.rs`
   - Test all 4 scenarios from Task 1.4

### Long-Term Improvements

1. **Add test helpers for:**
   - JSON response validation
   - WebSocket message structure
   - CLI argument parsing
   - AppState mocking

2. **Add CI integration:**
   - Run `cargo test` on all PRs
   - Run `cargo test --test capabilities` for feature branches
   - Add browser tests to CI pipeline

3. **Add coverage reporting:**
   - Use `tarpaulin` for Rust coverage
   - Use `nyc` for JavaScript coverage
   - Set minimum thresholds (80% Rust, 70% JS)

---

## Summary

- **Total tasks analyzed:** 28 (1.1-4.4)
- **Tasks with tests:** 0
- **Critical gaps (must fix):** 4
- **High priority gaps:** 12
- **UI test coverage:** 0% (browser tests needed)
- **Integration test coverage:** 0%

**Bottom line:** All Phase 1-4 tasks lack test coverage. Prioritize Tasks 1.4, 4.2, 1.6, and 4.3 for immediate test addition.
