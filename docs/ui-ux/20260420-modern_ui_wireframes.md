# Modern UI Enhancements - Detailed Wireframes

**Document Version**: 3.2 (AI-Agent Implementation Reference)
**Date**: 2026-04-20
**Last Updated**: 2026-04-20
**Status**: Phase 1 Completed - Phase 2 In Progress

---

## 📐 Wireframe Guidelines

### Grid System
- **Base unit**: 8px grid
- **Gaps**: 8px, 16px, 24px, 32px
- **Card sizes**: 280px min, 384px max
- **Border radius**: 8px (small), 12px (base), 24px (cards)

### Breakpoints
- **xs**: < 480px (mobile)
- **sm**: 480px - 767px (tablet)
- **md**: 768px - 1023px (laptop)
- **lg**: 1024px - 1279px (desktop)
- **xl**: >= 1280px (large desktop)

### Color Palette Reference
- **Primary**: `#6366f1` → `#8b5cf6` → `#06b6d4`
- **Success**: `#10b981` → `#14b8a6`
- **Warning**: `#f59e0b` → `#f97316`
- **Error**: `#f43f5e` → `#ef4444`
- **Background**: `#0f1115` (base), `#16191e` (surface), `#1f232a` (elevated)

---

## 📊 Dashboard Layout (Desktop)

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────┐
│ 🚀 Llama Monitor              🔍 Search...  ⚙️ Settings  👤 User ▼  📱 [+] [+ New Session]   │
├─────────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                                 │
│  [🏠 Home] [📂 Sessions] [🤖 Models] [🖥️ GPUs] [⚙️ Settings]                                   │
│                                                                                                 │
├─────────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                                 │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                           │
│  │ 📊 System    │ │ 🖥️ GPUs      │ │ 💾 Storage   │ │ 📡 Network   │                           │
│  │              │ │              │ │              │ │              │                           │
│  │ CPU: 45%     │ │ 🟢 RTX 4090  │ │ /data: 120   │ │ 📡 100 Mbps  │                           │
│  │ RAM: 62%     │ │ 24GB VRAM    │ │ 45% used     │ │ 12ms ping    │                           │
│  │ GPU: 38%     │ │ Temp: 65°C   │ │ 1.2 TB used  │ │ 500 KB/s     │                           │
│  │ Temp: 68°C   │ │ Fan: 45%     │ │ Free: 1.4 TB │ │ 80% Util     │                           │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘                           │
│                                                                                                 │
│  ┌──────────────────────────────────────────────────────────────────────────────────────────┐   │
│  │ Sessions (Active: 2)                               [➕ New Session] [⚙️ Configure]         │   │
│  ├──────────────────────────────────────────────────────────────────────────────────────────┤   │
│  │ ▶ llama-7b-chat  http://127.0.0.1:8001  4 GPUs  🟢 Online        [⋮] [⚙️] [➡️]        │   │
│  │ ─────────────────────────────────────────────────────────────────────────────────────    │   │
│  │ ⏸️ llama-13b    http://192.168.1.100:8001  8 GPUs  🟡 Offline  [⋮] [⚙️] [➡️]        │   │
│  └──────────────────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────────────────────┘
```

**Key Components**:
- Top navigation bar with search, settings, user profile
- 4-column grid for system metrics
- Sessions panel with expandable cards
- Floating action button for new session creation
- Sidebar navigation (260px, collapses to 80px on desktop)

---

## 📱 Dashboard Layout (Mobile)

```
┌────────────────────────────────────────────────────┐
│ 🚀 Llama Monitor         🔍  ⚙️  👤  [☰]         │
├────────────────────────────────────────────────────┤
│ ┌────────────────────────────────────────────────┐ │
│ │ 📊 CPU: 45%  RAM: 62%  GPU: 38%              │ │
│ │ 📈 Temp: 68°C  Fan: 45%                      │ │
│ └────────────────────────────────────────────────┘ │
│ ┌────────────────────────────────────────────────┐ │
│ │ 🖥️ RTX 4090  24GB VRAM  65°C  45%            │ │
│ └────────────────────────────────────────────────┘ │
│ ┌────────────────────────────────────────────────┐ │
│ │ 📡 100 Mbps  12ms  500 KB/s  80%             │ │
│ └────────────────────────────────────────────────┘ │
│ ┌────────────────────────────────────────────────┐ │
│ │ Sessions: 2 Active                             │ │
│ │ ─────────────────────────────────────────────  │ │
│ │ ▶ llama-7b-chat  🟢 8001  [⋮] [⚙️]           │ │
│ │ ⏸️ llama-13b  🟡 8001  [⋮] [⚙️]             │ │
│ └────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────┘
```

**Mobile Layout Features**:
- Single column stacking
- Collapsible sections
- Touch-friendly 48px minimum targets
- Bottom navigation on larger screens

---

## 🃏 Modernized Session Card

```
┌────────────────────────────────────────────────────────────────────────────────────────────┐
│ ▶ llama-7b-chat                                   [⋮] [⚙️] [➡️] [🔄 Restart] [🛑 Stop]     │
├────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                              │
│  Model: llama-7b.Q4_K_M.gguf                          🟢 384 MiB / 27.2 GiB                 │
│  Context: 4096 tokens  |  Temp: 0.7  |  Top P: 0.9                                        │
│  Repetition: 1.1  |  Top K: 40  |  Min P: 0.05  |  Batch: 512                             │
│                                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────────────────┐   │
│  │ ████████████████████████████░░░░░░░░░░░░  72%  🟢 Online                        │   │
│  └────────────────────────────────────────────────────────────────────────────────────┘   │
│  ETA: ≈ 2 min remaining                            [🚀 Expand]                               │
│                                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────────────────┐   │
│  │ GPU 0: 🟢 RTX 4090  24GB  65°C  45%  VRAM: 2.1/24 GB  |  GPU 1: 🟢 RTX 4090  24GB   │   │
│  │ GPU 2: 🟢 RTX 4090  24GB  63°C  42%  VRAM: 1.8/24 GB  |  GPU 3: 🟢 RTX 4090  24GB   │   │
│  └────────────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                              │
│  Ports: 8001 (API)  |  7779 (Metrics)  |  22 (SSH)                  [⚙️ Edit]             │
│  Threads: 12  |  Batch: 512  |  N-KV: 4  |  N-Seq: 1                                │
│                                                                                              │
└────────────────────────────────────────────────────────────────────────────────────────────┘
```

**Session Card Features**:
- Gradient header border on hover
- Expanded GPU details on expand
- Inline progress bar with ETA
- Action buttons in hover state
- Responsive text wrapping

---

## 🔲 Settings Modal (Glassmorphism)

```
┌───────────────────────────────────────────────────────────────────────────────────────────────┐
│  ⚙️ Settings                                    [✕]                                           │
├───────────────────────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────────────────────────────────┐  │
│  │  🔍 Search settings...                                                                  │  │
│  └─────────────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                               │
│  [Session] [GPU] [Models] [Appearance] [Advanced]                                           │
│                                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────────────────────┐  │
│  │  Session Management                                                                     │  │
│  ├─────────────────────────────────────────────────────────────────────────────────────────┤  │
│  │  Default context size: [4096 ▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇ 8192]                      │  │
│  │  ─────────────────────────────────────────────────────────────────────────────────────  │  │
│  │  Auto-save sessions: [✓] Enabled              [ℹ️]                                      │  │
│  │  Auto-cleanup stale sessions: [✓] 30 min      [ℹ️]                                      │  │
│  │  Default GPU count: [4] GPUs                  [ℹ️]                                      │  │
│  └─────────────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────────────────────┐  │
│  │  GPU Configuration                                                                      │  │
│  ├─────────────────────────────────────────────────────────────────────────────────────────┤  │
│  │  Enable GPU monitoring: [✓]                                                               │  │
│  │  Update interval: [2s ▇▇▇▇▇▇▇▇▇▇▇] 10s                                                  │  │
│  │  Temperature threshold: [75°C ▇▇▇▇▇▇▇▇▇▇] 90°C                                          │  │
│  └─────────────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────────────────────┐  │
│  │  Appearance                                                                             │  │
│  ├─────────────────────────────────────────────────────────────────────────────────────────┤  │
│  │  Theme: [Dark ▼] [Midnight] [Light] [Cyberpunk]                                        │  │
│  │  Font size: [Small] [Medium ▇▇▇] [Large]                                               │  │
│  │  Layout: [Balanced ▇▇▇] [Compact] [Spacious]                                           │  │
│  └─────────────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────────────────────┐  │
│  │  [Discard Changes]                               [💾 Save Changes]                      │  │
│  └─────────────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                               │
└───────────────────────────────────────────────────────────────────────────────────────────────┘
```

**Modal Features**:
- Glassmorphism backdrop blur (20px)
- Tabbed navigation for settings categories
- Range sliders with labels
- Theme presets with radio selection
- Save/Cancel actions at bottom

---

## 💬 Toast Notifications

### Success Toast
```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ ✅ Model installed successfully                                         [✕]              │
│   llama-7b.Q4_K_M.gguf (2.7 GB)                                                         │
│   [Undo] [View Logs] [Dismiss]                                                          │
│                                                                                         │
│  ▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇░░░░░░░░  100%                 │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

### Error Toast
```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ ❌ Failed to start session                                              [✕]              │
│   Port 8001 is already in use                                                           │
│   [Retry] [Change Port] [Learn More]                                                    │
│                                                                                         │
│  ▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  68%                 │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

### Warning Toast
```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ ⚠️  High GPU temperature detected                                       [✕]              │
│   RTX 4090: 85°C (warning threshold)                                                    │
│   [View GPU Monitor] [Reduce Load]                                                      │
│                                                                                         │
│  ▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  47%                 │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

### Info Toast
```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ ℹ️  New update available                                                [✕]              │
│   Version 0.3.0 ready to install                                                        │
│   [View Changelog] [Install Update]                                                     │
│                                                                                         │
│  ▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇░░░░░░░░  100%                 │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

**Toast Features**:
- Auto-dismiss with countdown progress bar
- Dismissible with close button
- Action buttons for quick responses
- Color-coded by type (green/success, red/error, amber/warning, blue/info)

---

## 🔮 Modernized Modal

```
┌───────────────────────────────────────────────────────────────────────────────────────────────┐
│  🚀 New Session                                      [✕]                                     │
├───────────────────────────────────────────────────────────────────────────────────────────────┤
│  Model:                                                                                       │
│  ┌───────────────────────────────────────────────────────────────────────────────────────┐   │
│  │ ▼ llama-7b.Q4_K_M.gguf [1.2 GB]                          [🔍]                    ▼ │   │
│  └───────────────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                               │
│  Context:                                                                                     │
│  ┌───────────────────────────────────────────────────────────────────────────────────────┐   │
│  │ 4096 ▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇▇ 8192 tokens                                   │   │
│  └───────────────────────────────────────────────────────────────────────────────────────┘   │
│  4096                            8192                                                         │
│                                                                                               │
│  Parameters:                                                                                  │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐                              │
│  │ Temp: [0.7] │ │ Top P: [0.] │ │ Top K: [40] │ │ Rep: [1.1] │                              │
│  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘                              │
│                                                                                               │
│  ┌───────────────────────────────────────────────────────────────────────────────────────┐   │
│  │ 2 GPUs  |  12 Threads  |  512 Batch  |  4096 Context  |  4 GB  Estimated VRAM        │   │
│  └───────────────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                               │
│  ┌───────────────────────────────────────────────────────────────────────────────────────┐   │
│  │  [Cancel]                                            [🚀 Spawn Session]                │   │
│  └───────────────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                               │
└───────────────────────────────────────────────────────────────────────────────────────────────┘
```

**Modal Features**:
- Glassmorphism with backdrop blur
- Preview card showing resource estimates
- Range slider for context size
- Inline parameter inputs
- Gradient border on active tab

---

## ⏳ Loading States

### Spinner (0-100ms)
```
     ⚡
    ⚡ ⚡
   ⚡  ⚡
    ⚡ ⚡
     ⚡
(360° rotation, 1s, linear timing)
```

### Progress Bar (100-500ms)
```
Install:  ██████████████████████████████████████████░░░░░  72%
         0%                                50%          100%
ETA: ≈ 2 min remaining                                   [Cancel]
```

### Skeleton (500ms+)
```
┌───────────────────────────────────────────────────────────────────────────────────────┐
│ ▇▇▇▇▇▇▇▇▇▇▇▇▇▇  ████████████████  ▇▇▇▇▇▇▇▇  ▇▇▇▇▇▇▇▇  ▇▇▇▇▇▇▇▇                      │
│ ▇▇▇▇▇▇▇▇  ████████████████████████  ▇▇▇▇▇▇  ▇▇▇▇▇▇▇▇  ▇▇▇▇▇▇▇▇  ▇▇▇▇▇▇              │
│ ▇▇▇▇▇▇▇▇  ██████████████              ▇▇▇▇  ▇▇▇▇▇▇▇▇  ▇▇▇▇▇▇▇▇  ▇▇▇▇▇▇              │
│ ▇▇▇▇▇▇▇▇  ████████████████████████  ▇▇▇▇▇▇  ▇▇▇▇▇▇▇▇  ▇▇▇▇▇▇▇▇  ▇▇▇▇▇▇              │
│ ▇▇▇▇▇▇▇▇  ████████████                  ▇▇  ▇▇▇▇▇▇▇▇  ▇▇▇▇▇▇▇▇  ▇▇▇▇▇▇              │
└───────────────────────────────────────────────────────────────────────────────────────┘
```

### Shimmer Effect (Content Loading)
```
┌───────────────────────────────────────────────────────────────────────────────────────┐
│ ▇▇▇▇▇▇▇▇  ██████████████  ▇▇▇▇▇▇▇▇  ██████████████  ▇▇▇▇▇▇▇▇  ██████████████        │
│ ▇▇▇▇▇▇▇▇  ████████████████████████  ▇▇▇▇▇▇  ██████████████  ▇▇▇▇▇▇▇▇  ▇▇▇▇▇▇        │
│ ▇▇▇▇▇▇▇▇  ██████████████              ▇▇▇▇  ██████████████  ▇▇▇▇▇▇▇▇  ▇▇▇▇▇▇        │
│ ▇▇▇▇▇▇▇▇  ████████████████████████  ▇▇▇▇▇▇  ██████████████  ▇▇▇▇▇▇▇▇  ▇▇▇▇▇▇        │
│ ▇▇▇▇▇▇▇▇  ████████████                  ▇▇  ██████████████  ▇▇▇▇▇▇▇▇  ▇▇▇▇▇▇        │
└───────────────────────────────────────────────────────────────────────────────────────┘
```

**Progressive Loading Strategy**:
- **0-100ms**: Show immediate visual feedback (spinner or shimmer)
- **100-500ms**: Continue spinner with optional ETA
- **500ms+**: Show skeleton with estimated time
- **2s+**: Show cancellable progress indicator

---

## 🔘 Button Variants

### Primary Gradient Button
```
[Gradient Active Button]
┌──────────────────────────────────────────────────────────────────────────────────────┐
│  ✨ Install & Start    ──→                                                         │
│  ← Gradient: 160°→240° (Blue→Purple→Cyan)                                          │
└──────────────────────────────────────────────────────────────────────────────────────┘
Hover: lift 4px, stronger shadow, gradient reverse
Active: scale 0.98, return shadow
```

### Ghost Button
```
[Ghost Button]
┌──────────────────────────────────────────────────────────────────────────────────────┐
│  Cancel                                                                              │
│  ← Transparent on hover with subtle background                                      │
└──────────────────────────────────────────────────────────────────────────────────────┘
Hover: background 5%, shadow 4px
```

### Icon Button
```
[Icon Button]
┌─────────────────┐
│  ⚙️            │  ← Circular, hover scale 1.1
└─────────────────┘
```

### Outline Button
```
[Outline Button]
┌──────────────────────────────────────────────────────────────────────────────────────┐
│  ⚪ 2 GPUs ▇▇▇▇▇▇▇ 12 Threads                                                        │
│  ← Border gradient on hover, fill on active                                         │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

### Button Sizes
```
[Small Button]
┌────────────┐
│  Text      │  ← Height: 36px, Padding: 8px 16px
└────────────┘

[Medium Button] (Default)
┌────────────────────────┐
│  Text                │  ← Height: 44px, Padding: 12px 20px
└────────────────────────┘

[Large Button]
┌─────────────────────────────────┐
│  Text                         │  ← Height: 52px, Padding: 16px 32px
└─────────────────────────────────┘
```

---

## 🟢 Status Indicators

```
🟢 Online  (Success)       #10b981
🟡 Offline (Warning)       #f59e0b
🔴 Error   (Error)         #f43f5e
⚪ Unknown (Neutral)       #6b7280
🔵 Processing (Info)       #6366f1
```

### Animated Status Dots
```
[Status Dot - Pulse]
● ────○─────●─────○─────●
(800ms cycle, opacity 0.5→1→0.5)

[Status Dot - Bounce]
●  ●   ●
   ↓
  ●
   ↑
  ●
(300ms spring animation)

[Status Dot - Spin]
↻
(1s rotation, continuous)
```

---

## 📝 Input Fields

### Standard Input (Unfocused)
```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ Model Path                                                                           │
│ ┌──────────────────────────────────────────────────────────────────────────────────┐ │
│ │ /home/user/models/llama-7b.Q4_K_M.gguf       📂  ❌                              │ │
│ └──────────────────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

### Focused Input
```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ Model Path                                                                           │
│ ┌──────────────────────────────────────────────────────────────────────────────────┐ │
│ │ /home/user/models/llama-7b.Q4_K_M.gguf       📂  ❌                              │ │
│ └──────────────────────────────────────────────────────────────────────────────────┘ │
│  ▲ Gradient Border (#6366f1→#8b5cf6, 3px)                                           │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

### Success Input
```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ Port                                                                                 │
│ ┌──────────────────────────────────────────────────────────────────────────────────┐ │
│ │ 8001                                        ✓                                    │ │
│ └──────────────────────────────────────────────────────────────────────────────────┘ │
│  ▲ Green Gradient Border (#10b981→#14b8a6, 3px)                                     │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

### Error Input
```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ SSH Target                                                                           │
│ ┌──────────────────────────────────────────────────────────────────────────────────┐ │
│ │ user@remote                                 ✗                                    │ │
│ └──────────────────────────────────────────────────────────────────────────────────┘ │
│  ▲ Red Gradient Border (#f43f5e→#ef4444, 3px)                                       │
│ Error: Invalid SSH target                                                            │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

### Input with Icons
```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ Search                                                                               │
│ ┌──────────────────────────────────────────────────────────────────────────────────┐ │
│ │ 🔍 Search...                                 🎯  🔊  🔍  ⚙️                       │ │
│ └──────────────────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 📈 Progress Indicators

### Spinner
```
    ⚡
   ⚡ ⚡
  ⚡  ⚡
   ⚡ ⚡
    ⚡
(360° rotation, 1s linear)
```

### Linear Progress
```
Install:  ██████████████████████████████████████████████████░░░░░  72%
         0%                                                50%   100%
ETA: ≈ 2 min remaining
```

### Circular Progress
```
           12 o'clock
         ↺
     9         3
         ↻
           6
(Gradient stroke, 12px width, 18px radius)
```

### Progress with Status
```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ Installing model...                                                                  │
│                                                                                      │
│ ████████████████████████████████████████████░░░░░░░░░░░░░░░░  68%                  │
│                                                                                      │
│ Process: llama-server                                                               │
│ GPU: 0, 1, 2, 3                                                                     │
│ ETA: ≈ 2 min remaining                            [X] Cancel                          │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 🎨 Theme Presets

### Dark (Default)
```css
--bg-primary: #0f1115
--bg-surface: #16191e
--bg-elevated: #1f232a
--bg-floating: #2a2f3a
--primary-gradient: linear-gradient(160deg, #6366f1, #8b5cf6, #06b6d4)
--border-subtle: rgba(255,255,255,0.05)
--border-emphasis: rgba(255,255,255,0.1)
```

### Midnight
```css
--bg-primary: #0a0e14
--bg-surface: #131821
--bg-elevated: #1b212c
--primary-gradient: linear-gradient(160deg, #2563eb, #3b82f6, #0ea5e9)
```

### Light
```css
--bg-primary: #f8fafc
--bg-surface: #ffffff
--bg-elevated: #ffffff
--primary-gradient: linear-gradient(160deg, #6366f1, #8b5cf6, #06b6d4)
--text-primary: #1f2937
--text-secondary: #4b5563
```

### Cyberpunk
```css
--bg-primary: #0d0d0d
--bg-surface: #1a1a1a
--bg-elevated: #242424
--primary-gradient: linear-gradient(160deg, #00f3ff, #ff00ff, #bc13fe)
--accent-color: #00f3ff
```

---

## 🎬 Animated Icons

### Status Dot (Pulse)
```
● ────○─────●─────○─────●
(800ms cycle, opacity animate 1→0.5→1)
```

### Checkmark (Bounce)
```
✓  ✓   ✓
   ↓
  ✓
   ↑
 ✓
(300ms spring animation, cubic-bezier(0.68,-0.55,0.27,1.55))
```

### Warning (Blink)
```
⚠️  ⚠️   ⚠️
    ↓
   ⚠️
(1s cycle, opacity 0.3→1→0.3)
```

### Loading (Spin)
```
↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻↻
(1s rotation, linear timing)
```

---

## 📏 Component尺寸参考

### Spacing
- **4px**: tight spacing, micro-animations
- **8px**: base unit, tight gaps
- **12px**: component padding
- **16px**: standard spacing, modal content
- **20px**: card padding
- **24px**: gap between sections
- **32px**: large gaps, section breaks

### Border Radius
- **4px**: small buttons, tags
- **8px**: inputs, pills
- **12px**: panels, base radius
- **16px**: modals
- **24px**: cards, large containers

### Shadows
- **Surface**: `0 2px 8px rgba(0,0,0,0.3), 0 8px 24px rgba(0,0,0,0.2)`
- **Elevated**: `0 4px 12px rgba(0,0,0,0.4), 0 12px 32px rgba(0,0,0,0.3)`
- **Floating**: `0 8px 24px rgba(0,0,0,0.5), 0 24px 64px rgba(0,0,0,0.4)`

### Animation Timing
- **Quick**: 200ms `cubic-bezier(0.4, 0, 0.2, 1)`
- **Medium**: 400ms `cubic-bezier(0.4, 0, 0.2, 1)`
- **Slow**: 600ms `cubic-bezier(0.4, 0, 0.2, 1)`
- **Spring**: `cubic-bezier(0.16, 1, 0.3, 1)`

---

## 🚀 Implementation Guide

### Phase 1: Core Visuals (Priority: HIGH)
1. Add CSS variables to `:root`
2. Add panel and card styles
3. Add button variants
4. Add input field styles

### Phase 2: Layout System (Priority: MEDIUM)
1. Add grid system
2. Add navigation styles
3. Add responsive utilities

### Phase 3: Interactive Elements (Priority: MEDIUM)
1. Add animation system
2. Add hover states
3. Add loading states

### Phase 4: JavaScript Integration (Priority: HIGH)
1. Add toast notification system
2. Add modal system
3. Add progress indicator helpers

---

## 📚 Additional Reference

### File Locations
- `static/style.css` - CSS styles
- `static/index.html` - HTML structure
- `static/app.js` - JavaScript logic
- `docs/20260420-modern_ui_enhancements.md` - Implementation guide

### Design Principles
1. **Glassmorphism** for depth and modern feel
2. **Gradient accents** for visual interest
3. **Animations** for delight and feedback
4. **Context-aware UI** that adapts to user state
5. **Consistent design language** across all components

---

## ✅ Testing Checklist

### Visual Testing
- [ ] All CSS variables defined
- [ ] Panels show glassmorphism
- [ ] Cards lift on hover
- [ ] Gradients render correctly
- [ ] Shadows match specs
- [ ] Border radius consistent

### Component Testing
- [ ] All button variants work
- [ ] Input fields show validation
- [ ] Modals open/close smoothly
- [ ] Toasts appear/disappear
- [ ] Progress bars animate

### Responsive Testing
- [ ] Grid adjusts at breakpoints
- [ ] Sidebar collapses on mobile
- [ ] Touch targets 48px minimum
- [ ] Text doesn't overflow

### Accessibility Testing
- [ ] Keyboard navigation works
- [ ] ARIA labels present
- [ ] Focus indicators visible
- [ ] Color contrast meets WCAG AA

---

## 🤝 Contributing

When implementing these changes:
1. Follow the priority order
2. Test each component thoroughly
3. Maintain backward compatibility
4. Document any changes
5. Update this document as needed

---

**Happy Coding!** 🚀
