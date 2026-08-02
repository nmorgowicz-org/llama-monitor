# Memory Management

> **Status: Shipped.** Phase 5b complete. Wired-limit reserves, Metal GPU cap with reclaim guidance, and MemoryAvailabilitySnapshot.

Llama Monitor manages system memory to ensure stable operation during inference. This covers wired memory limits on macOS, Metal GPU memory on Apple Silicon, and reclaim guidance for freeing unused memory.

## MemoryAvailabilitySnapshot

The system captures a `MemoryAvailabilitySnapshot` that includes:

| Field | Description |
|-------|-------------|
| `total_memory_bytes` | Total system memory |
| `available_memory_bytes` | Available memory (includes inactive pages on macOS) |
| `used_memory_bytes` | Memory currently in use |
| `memory_pressure` | Current memory pressure state |
| `wired_limit_bytes` | Recommended wired memory limit |
| `hard_limit_bytes` | 95% hard ceiling for wired memory |
| `reclaim_bytes` | Recommended memory to reclaim |

### macOS memory calculation

On macOS, `available_memory_bytes` uses `sysinfo` which includes inactive pages. This is different from "free memory" which only counts truly unused pages. The distinction matters because macOS can quickly reactivate inactive pages when needed.

## Wired-limit reserves

Wired memory is memory that cannot be paged out. On macOS, the system has a wired memory limit that, if exceeded, can cause instability.

### Tiered reserves (Phase 5b)

The system uses tiered wired memory limits:

1. **Base limit**: The minimum wired memory reservation
2. **Tiered reserves**: Additional reserves based on memory pressure
3. **95% hard ceiling**: Never exceed 95% of total wired memory

### Implementation

- `src/llama/memory/` — Memory management modules
- `wired_limit.rs` — Wired limit calculations
- `memory_snapshot.rs` — Memory availability snapshot

### Behavior

- On open: The system captures a fresh memory snapshot
- On inference start: The wired limit is set based on available memory
- On memory pressure: Reclaim guidance is shown to the user
- The 95% hard ceiling is never exceeded

## Metal GPU cap (Phase 5b)

On Apple Silicon, Metal GPU memory is part of unified memory. The system tracks Metal GPU usage and provides a cap to prevent exceeding available resources.

### Metal GPU cap row

The welcome page and spawn wizard include a Metal GPU cap row with:

- **Current Metal limit**: Shows the current Metal GPU memory limit
- **Increase button**: Allows the user to increase the limit (with confirmation)
- **Reclaim guidance**: Suggests reclaiming memory when close to the limit

### Implementation

- `src/web/api/memory/metal_cap.rs` — Metal cap API endpoint
- `src/llama/memory/metal_cap.rs` — Metal cap calculations
- `src/llama/memory/metal_reclaim.rs` — Metal reclaim guidance

### Behavior

- On open: The system queries the current Metal GPU limit
- On inference start: The Metal cap is set based on available GPU memory
- On Metal cap exceeded: The user is shown a warning with reclaim options

## Reclaim guidance (Phase 5b)

When memory pressure is high, the system provides reclaim guidance:

### Sources of reclaimable memory

1. **Inactive pages**: macOS can reactivate these, but they're not currently used
2. **Wired memory**: Memory that cannot be paged out (requires application restart)
3. **Metal cache**: GPU cache can be flushed

### Guidance display

- Reclaim guidance is shown in the welcome page when memory is low
- The spawn wizard shows reclaim guidance during hardware step
- The preset editor shows reclaim guidance in the advanced section

### Reclaim flow

1. User clicks "Reclaim" button
2. System attempts to reclaim inactive pages
3. Result is shown to the user (success/failure with details)
4. Memory snapshot is refreshed

## Memory policy

The overall memory policy is:

1. **Estimate mandatory memory**: weights + active KV/context + runtime overhead + reserve
2. **Present optional growth**: Rapid retained cache as optional growth on top of fit
3. **Recommend baseline**: 8 GiB retained cache when it fits
4. **Expose retention option**: 16 GiB as "retain more branches" option
5. **Don't recommend disk checkpoints**: For normal interactive agents
6. **Use workload-scoped llama host cache**: Keep `0` for one linear active
   conversation; use a bounded 2 GiB cap for the measured parallel-1 main plus
   one sequential child profile through 32K when unified-memory headroom permits
