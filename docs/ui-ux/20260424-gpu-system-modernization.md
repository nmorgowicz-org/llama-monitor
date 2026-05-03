# GPU & System Metrics Modernization

**Date:** 2026-04-24
**Author:** opencode
**Status:** Proposal — approved, ready for implementation

## Motivation

The inference metrics cards (Throughput, Generation, Context Window) have a premium, modern feel with animated rings, sparklines, token flow animations, progress bars, live chips, stage segments, and detail chips. By contrast, the GPU and System sections are flat tables with severity-colored text — functional but visually disconnected from the rest of the dashboard.

This document describes the chosen approach: wide GPU + System cards matching the inference row width, with a user-configurable visual presentation switcher that lets users toggle between multiple visualization styles per metric.

---

## Available Data

### GPU Metrics (`GpuMetrics` struct, per-card)

| Field | Type | Units | Notes |
|-------|------|-------|-------|
| `temp` | `f32` | °C | Temperature |
| `load` | `u32` | % (0-100) | GPU utilization |
| `power_consumption` | `f32` | Watts | Current power draw |
| `power_limit` | `u32` | Watts | Maximum power limit |
| `vram_used` | `u64` | MiB | VRAM currently used |
| `vram_total` | `u64` | MiB | Total VRAM available |
| `sclk_mhz` | `u32` | MHz | Shader/core clock |
| `mclk_mhz` | `u32` | MHz | Memory clock |

**Per-card key:** `BTreeMap<String, GpuMetrics>` keyed by card name (e.g. `"GPU0 NVIDIA GeForce RTX 4090"`)

**Backends:** NVIDIA (`nvidia-smi`), ROCm (`rocm-smi`), Apple (`mactop`), Dummy

### System Metrics (`SystemMetrics` struct, single)

| Field | Type | Units | Notes |
|-------|------|-------|-------|
| `cpu_name` | `String` | — | CPU brand/model string |
| `cpu_temp` | `f32` | °C | CPU temperature |
| `cpu_temp_available` | `bool` | — | Whether a sensor was found |
| `cpu_load` | `u32` | % (0-100) | Average CPU utilization |
| `cpu_clock_mhz` | `u32` | MHz | Maximum CPU core frequency |
| `ram_total_gb` | `f64` | GB | Total system RAM |
| `ram_used_gb` | `f64` | GB | Used system RAM |
| `motherboard` | `String` | — | Motherboard/product name |

**Platform sourcing:** Linux (`sysinfo` + sysfs), macOS (`sysctl`), Windows (WMI + LibreHardwareMonitor)

### Capability Flags

- `capabilities.gpu` / `capabilities.system` — boolean, controls section visibility
- `availability.gpu` / `availability.system` / `availability.cpu_temp` — reason strings for empty states

---

## Card Layout

Both cards span the full width of the inference row (same container width as the 3-column inference grid). GPU card sits first, System card below it.

### GPU Card

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│  GPU: NVIDIA GeForce RTX 4090              [LIVE]   ◉ 72°C           ⚙          │
│  (card topline: device name, live chip, radial temp gauge, visualization gear)   │
├──────────────────────────────────────────────────────────────────────────────────┤
│  ┌───────────────────┐  ┌───────────────────┐  ┌───────────────────┐  ┌────────┐│
│  │  LOAD             │  │  POWER            │  │  VRAM             │  │ CLOCKS ││
│  │                   │  │                   │  │                   │  │        ││
│  │  [visualization]  │  │  [visualization]  │  │  [visualization]  │  │[viz]   ││
│  │  85%              │  │  302W / 450W      │  │  14.2 / 24 GB     │  │1920/7001││
│  └───────────────────┘  └───────────────────┘  └───────────────────┘  └────────┘│
└──────────────────────────────────────────────────────────────────────────────────┘
```

Four equal-width metric blocks in a single row. Card height matches inference cards (~same visual weight).

### System Card

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│  SYSTEM: Intel Core i9-13900K / ROG MAXIMUS Z790        [LIVE]   ◉ 65°C    ⚙   │
├──────────────────────────────────────────────────────────────────────────────────┤
│  ┌───────────────────┐  ┌───────────────────┐  ┌─────────────────────────────┐  │
│  │  CPU LOAD         │  │  RAM              │  │  CLOCK                      │  │
│  │                   │  │                   │  │                             │  │
│  │  [visualization]  │  │  [visualization]  │  │  3.5 GHz                    │  │
│  │  34%              │  │  16.4 / 32 GB     │  │                             │  │
│  └───────────────────┘  └───────────────────┘  └─────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────────────┘
```

Three blocks (load, RAM, clock). Temp in topline. CPU model + motherboard as detail chips in topline.

---

## Visualization Switcher

### Concept

A gear icon (⚙) in a small pill badge in the top-right of each card. Clicking it opens a popover panel where the user can pick a visualization style for each metric. Preferences persist to `localStorage`.

### Switcher Popover Layout

```
┌─────────────────────────────────────────────────────────────┐
│  GPU Visualization                     [Reset to defaults]  │
├─────────────────────────────────────────────────────────────┤
│  LOAD                    POWER                   VRAM       │
│  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐  │
│  │ ▓▓▓ │ │ ◉   │ │  ╱╲ │ │ ▓▓▓ │ │ ◉   │ │  ╱╲ │ │ ▓▓▓ │ │
│  │ bar │ │ ring│ │line │ │ bar │ │ ring│ │line │ │ stack│ │
│  └─────┘ └─────┘ └─────┘ └─────┘ └─────┘ └─────┘ └─────┘  │
│     ●                                     ●                    │
│                                             ▒                  │
│  TEMPERATURE              CLOCKS                            │
│  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐                  │
│  │ ◉   │ │ ▐   │ │ #   │ │ chips│ │ dual │                  │
│  │ ring│ │therm│ │num  │ │     │ │ ring │                  │
│  └─────┘ └─────┘ └─────┘ └─────┘ └─────┘                  │
│     ●                                                          │
│                                             ●                  │
└─────────────────────────────────────────────────────────────┘
```

Each metric type gets a row of selectable style thumbnails. Selected style is highlighted with a border + dot indicator. Clicking a thumbnail swaps the visualization live in the card.

### Persistence

```typescript
interface GpuVizPrefs {
  temp: 'ring' | 'thermometer' | 'numeric';
  load: 'bar' | 'ring' | 'sparkline';
  power: 'bar' | 'ring' | 'sparkline';
  vram: 'bar' | 'ring' | 'sparkline' | 'stacked';
  clocks: 'chips' | 'dual-ring' | 'numeric';
}

interface SystemVizPrefs {
  temp: 'ring' | 'thermometer' | 'numeric';
  cpuLoad: 'bar' | 'ring' | 'sparkline';
  ram: 'bar' | 'ring' | 'sparkline' | 'stacked';
  clock: 'chip' | 'numeric';
}
```

Stored in `localStorage` under keys `llama-monitor-gpu-viz` and `llama-monitor-system-viz`. Defaults to the "bar" style for everything.

---

## Visualization Styles (per metric)

### Temperature

| Style | Description | Visual |
|-------|-------------|--------|
| **Ring** (default) | Radial conic-gradient gauge, ~48px diameter. Color: teal < 60°C, amber 60-75°C, red > 75°C. Breathe animation when > 75°C. | `◉ 72°C` with colored arc |
| **Thermometer** | Vertical fill bar, narrow (~12px wide, ~60px tall). Gradient fill from bottom up. Severity color. | `▐` vertical bar with fill |
| **Numeric** | Compact monospace number with severity-colored dot indicator. Most space-efficient. | `● 72°C` |

### Load / CPU Load

| Style | Description | Visual |
|-------|-------------|--------|
| **Bar** (default) | Horizontal progress bar with gradient fill (green → amber → red based on percentage). Shimmer animation when > 90%. | `▓▓▓▓▓▓░░ 85%` |
| **Ring** | Radial conic-gradient gauge showing percentage. Same color scheme as temp. | `◉ 85%` circular |
| **Sparkline** | Mini SVG area chart showing load history (last ~30 data points, frontend ring buffer). Gradient fill. Peak dot. | `╱╲╱╲  85%` |

### Power

| Style | Description | Visual |
|-------|-------------|--------|
| **Bar** (default) | Horizontal progress bar showing consumption vs limit. Limit shown as a thin marker line at the right. Red pulse animation when at cap. | `▓▓▓▓▓░ 302W / 450W` |
| **Ring** | Radial gauge showing percentage of power limit. | `◉ 67%` circular |
| **Sparkline** | Mini SVG area chart showing power history. Limit shown as a horizontal dashed line. | `╱╲╱  302W` with limit line |

### VRAM / RAM

| Style | Description | Visual |
|-------|-------------|--------|
| **Bar** (default) | Horizontal progress bar with gradient fill. Used/total text overlay. Severity coloring. | `▓▓▓▓░░ 14.2 / 24 GB` |
| **Ring** | Radial gauge showing percentage of total. | `◉ 59%` circular |
| **Sparkline** | Mini SVG area chart showing usage history over time. | `╱╲╱╲  14.2 GB` |
| **Stacked** | Two-tone bar: used portion in primary color, free portion in dimmed color. Shows both values. | `▓▓▓▓░░░░` with labels |

### Clocks (SCLK / MCLK)

| Style | Description | Visual |
|-------|-------------|--------|
| **Chips** (default) | Detail-chip style pills showing both values with labels. Compact and clean. | `SCLK 1920 MHz` `MCLK 7001 MHz` |
| **Dual Ring** | Two small radial gauges side by side, each showing current clock as percentage of known max (or just raw value ring). | `◉ 1920` `◉ 7001` |
| **Numeric** | Clean monospace readout, two lines. Most space-efficient. | `1920 / 7001 MHz` |

### System Clock

| Style | Description | Visual |
|-------|-------------|--------|
| **Chip** (default) | Single detail-chip pill with GHz value. | `3.5 GHz` |
| **Numeric** | Clean monospace readout. | `3500 MHz` |

---

## Sparkline History (Frontend Ring Buffer)

For sparkline visualizations, we need historical data. Rather than adding backend complexity, we'll track history in a frontend ring buffer:

```typescript
interface MetricHistory {
  gpu: {
    temp: number[];       // last 60 readings (~1 minute at 1s poll)
    load: number[];
    power: number[];
    vramPct: number[];
    sclk: number[];
    mclk: number[];
  };
  system: {
    cpuTemp: number[];
    cpuLoad: number[];
    ramPct: number[];
  };
}
```

Each array maintains a fixed-size ring buffer (60 entries). On each WebSocket message, push the new value. The sparkline renderer reads the buffer and draws the SVG area chart. This is the same pattern used for the throughput sparklines in the inference cards.

---

## Shared Visual Components

### 1. Hardware Card Base

```css
.hardware-card {
  background: linear-gradient(145deg, rgba(40, 48, 58, 0.86), rgba(30, 36, 43, 0.94));
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-surface);
  padding: 22px;
  transition: all 300ms ease;
}
.hardware-card.is-live {
  border-color: rgba(136, 192, 209, 0.2);
  box-shadow: var(--shadow-surface),
    0 0 0 1px rgba(136, 192, 209, 0.1),
    0 0 40px rgba(136, 192, 209, 0.06);
}
```

### 2. Metric Block (Reusable Container)

```css
.metric-block {
  flex: 1;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 12px;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}
.metric-block-label {
  font-size: var(--text-3xs);
  font-weight: 700;
  color: var(--color-text-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.metric-block-value {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  font-weight: 700;
  color: var(--color-text-primary);
}
```

### 3. Radial Mini-Gauge (Ring Visualization)

```css
.hw-gauge {
  width: 48px;
  height: 48px;
  border-radius: 50%;
  background:
    radial-gradient(circle, rgba(26, 27, 38, 0.98) 0 54%, transparent 55%),
    conic-gradient(from -90deg, var(--gauge-color) calc(var(--pct) * 1%),
      rgba(255, 255, 255, 0.06) 0);
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
  transition: background 300ms ease;
}
.hw-gauge.is-warming {
  animation: ring-breathe 2.4s ease-in-out infinite;
}
```

### 4. Progress Bar (Bar Visualization)

```css
.hw-bar-bg {
  height: 6px;
  border-radius: 3px;
  background: rgba(255, 255, 255, 0.06);
  overflow: hidden;
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.3);
}
.hw-bar-fill {
  height: 100%;
  border-radius: 3px;
  background: linear-gradient(90deg, var(--bar-start), var(--bar-end));
  transition: width 300ms ease, background 300ms ease;
}
.hw-bar-fill.is-hot {
  animation: bar-shimmer 2s ease-in-out infinite;
}
```

### 5. Sparkline (Sparkline Visualization)

Reuses the existing sparkline rendering from the inference cards. Each metric block can contain a small SVG sparkline (~80px × 20px) with gradient fill and optional peak dot.

### 6. Visualization Switcher Popover

```css
.viz-switcher {
  position: absolute;
  top: 100%;
  right: 0;
  z-index: 100;
  background: linear-gradient(145deg, rgba(40, 48, 58, 0.98), rgba(30, 36, 43, 0.99));
  border: 1px solid var(--border-subtle);
  border-radius: 16px;
  padding: 16px;
  min-width: 320px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(12px);
}
.viz-switcher-option {
  cursor: pointer;
  border-radius: 8px;
  padding: 8px;
  border: 2px solid transparent;
  transition: all 150ms ease;
}
.viz-switcher-option:hover {
  border-color: rgba(136, 192, 209, 0.3);
  background: rgba(136, 192, 209, 0.06);
}
.viz-switcher-option.active {
  border-color: rgba(136, 192, 209, 0.6);
  background: rgba(136, 192, 209, 0.1);
}
```

### 7. Gear Button (Pill Badge)

```css
.viz-gear-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  cursor: pointer;
  transition: all 150ms ease;
  color: var(--color-text-muted);
  font-size: 14px;
}
.viz-gear-btn:hover {
  background: rgba(136, 192, 209, 0.1);
  border-color: rgba(136, 192, 209, 0.2);
  color: #88c0d1;
}
```

---

## Implementation Plan

### Phase 1: Card Structure & Default Visualizations

1. Replace GPU table with wide GPU card HTML structure
2. Replace System table with wide System card HTML structure
3. Implement default "bar" visualizations for all metrics
4. Implement radial temp gauge in card topline
5. Wire up live data from WebSocket
6. Add card states (live/idle/dormant/unavailable), live chip, empty states

**Files:** `static/index.html`, `static/style.css`, `static/app.js`

### Phase 2: Visualization Switcher

1. Build gear button + popover component
2. Implement all visualization style variants per metric
3. Wire up style selection with live preview swap
4. Add `localStorage` persistence
5. Add "Reset to defaults" button

**Files:** `static/index.html`, `static/style.css`, `static/app.js`

### Phase 3: Sparkline History

1. Add frontend ring buffer for GPU + system metric history
2. Implement sparkline rendering for load, power, VRAM/RAM metrics
3. Add peak dot + limit line overlays

**Files:** `static/app.js`

### Phase 4: Polish

1. Severity animations (breathe, shimmer, pulse)
2. Smooth transitions between visualization swaps
3. Responsive adjustments for narrow viewports
4. Empty state handling

---

## Implementation Scope Estimate

| Phase | HTML/CSS | JS Changes | Rust Changes | Complexity |
|-------|----------|------------|--------------|------------|
| Phase 1: Cards + defaults | ~250 lines | ~120 lines | 0 | Medium |
| Phase 2: Viz switcher | ~200 lines | ~200 lines | 0 | Medium-High |
| Phase 3: Sparklines | ~20 lines | ~80 lines | 0 | Low |
| Phase 4: Polish | ~50 lines | ~50 lines | 0 | Low |
| **Total** | **~520 lines** | **~450 lines** | **0** | **Medium** |

**No Rust/backend changes required.** All data is already available via the existing WebSocket push. Sparkline history is tracked in a frontend ring buffer.

---

## Previous Options (Superseded)

The following options were considered but superseded by the chosen approach above:

- **Option A (original):** Per-GPU cards in a responsive grid — rejected in favor of single wide card
- **Option B (original):** Enhanced table with inline bars — rejected for being too conservative
- **Option C (original):** Side-by-side GPU + System in a split layout — rejected in favor of stacked full-width cards
