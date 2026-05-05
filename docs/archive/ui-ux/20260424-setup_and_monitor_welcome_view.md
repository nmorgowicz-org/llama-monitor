# Setup & Monitor Welcome View — Option C Design Spec

**Date:** 2026-04-24
**Status:** Design spec for future implementation
**Author:** AI agent (opencode)

## Overview

Split the Llama Monitor dashboard into two distinct views with a smooth animated transition:

- **Setup View** — Shown when no active session. Clean landing page with guided onboarding.
- **Monitor View** — Shown when a session is active. Full metrics dashboard (current layout).

The transition between views is a premium animated sequence that feels like the app is "coming alive."

## Architecture

### View Container Structure

```html
<div id="app-shell">
  <!-- Top bar (always visible, adapts content) -->
  <div class="endpoint-health-strip" id="top-bar">
    <!-- Dynamic content based on active view -->
  </div>

  <div class="main-layout">
    <!-- Sidebar (always visible) -->
    <nav class="sidebar-nav">...</nav>

    <!-- View container -->
    <main class="view-container">

      <!-- SETUP VIEW -->
      <div class="view view-setup" id="view-setup">
        <div class="setup-hero">
          <div class="setup-logo">
            <!-- Animated app logo with breathing glow -->
          </div>
          <h1 class="setup-title">Llama Monitor</h1>
          <p class="setup-subtitle">Real-time telemetry for your inference stack</p>
        </div>

        <div class="setup-actions">
          <!-- Path 1: Attach to remote endpoint -->
          <div class="setup-card setup-card-attach">
            <div class="setup-card-icon">
              <!-- Network/endpoint icon with pulse animation -->
            </div>
            <h2 class="setup-card-title">Attach to Endpoint</h2>
            <p class="setup-card-desc">Connect to a running llama.cpp server on any machine</p>
            <div class="setup-card-form">
              <input type="text" class="setup-input" id="setup-endpoint-url"
                     placeholder="http://127.0.0.1:8080">
              <button class="setup-btn setup-btn-primary" onclick="doAttachFromSetup()">
                <span class="btn-icon">⚡</span> Attach
              </button>
            </div>
            <div class="setup-card-footer">
              <a class="setup-link" onclick="openSessionModal()">Browse saved sessions →</a>
            </div>
          </div>

          <!-- Path 2: Spawn local server -->
          <div class="setup-card setup-card-spawn">
            <div class="setup-card-icon">
              <!-- Server/GPU icon with shimmer animation -->
            </div>
            <h2 class="setup-card-title">Spawn Local Server</h2>
            <p class="setup-card-desc">Start a llama.cpp server from a model preset</p>
            <div class="setup-card-form">
              <select class="setup-select" id="setup-preset-select">
                <option>Small Model 128K context</option>
                <option>Medium Model 32K context</option>
                <!-- Built-in presets -->
              </select>
              <button class="setup-btn setup-btn-secondary" onclick="doStartFromSetup()">
                <span class="btn-icon">▶</span> Start Server
              </button>
            </div>
            <div class="setup-card-footer">
              <a class="setup-link" onclick="openPresetManager()">Manage presets →</a>
            </div>
          </div>
        </div>

        <!-- Quick stats (optional, from last session) -->
        <div class="setup-stats" id="setup-stats" style="display:none;">
          <div class="setup-stat-item">
            <span class="setup-stat-value" id="setup-last-prompt-rate">—</span>
            <span class="setup-stat-label">Last prompt rate</span>
          </div>
          <div class="setup-stat-item">
            <span class="setup-stat-value" id="setup-last-gen-rate">—</span>
            <span class="setup-stat-label">Last gen rate</span>
          </div>
          <div class="setup-stat-item">
            <span class="setup-stat-value" id="setup-last-session">—</span>
            <span class="setup-stat-label">Last session</span>
          </div>
        </div>
      </div>

      <!-- MONITOR VIEW (existing dashboard) -->
      <div class="view view-monitor" id="view-monitor" style="display:none;">
        <div class="page" id="page-server">
          <!-- Model Preset section -->
          <section class="metric-section" id="preset-section">...</section>

          <!-- Inference Metrics section -->
          <section class="metric-section" id="inference-section">...</section>

          <!-- GPU section -->
          <section class="metric-section" id="gpu-section">...</section>

          <!-- System section -->
          <section class="metric-section" id="system-section">...</section>
        </div>
        <!-- Chat, Logs pages -->
        <div class="page" id="page-chat" style="display:none;">...</div>
        <div class="page" id="page-logs" style="display:none;">...</div>
      </div>

    </main>
  </div>
</div>
```

### Top Bar Adaptation

**Setup mode:**
```
[Logo] Llama Monitor                                    [Settings] [User]
```
- Clean, minimal, no status pills
- Subtle gradient top border (same as current)
- Settings and User buttons remain for access to preferences

**Transitioning (attach/start in progress):**
```
[Logo] Llama Monitor              [Connecting...]        [Settings] [User]
```
- Center shows animated "Connecting..." with spinning dots
- Background subtly darkens

**Monitor mode (existing):**
```
[Remote] http://192... [Detach] [Inference only] [Remote Agent] [GPU 61C]
```
- Full status strip with all pills

### Transition Animation Sequence

**Setup → Monitor (on attach/start success):**

1. **Frame 0ms** — Setup view visible, monitor view hidden
2. **Frame 0ms** — Top bar transitions to "Connecting..." state
3. **Frame 100ms** — Setup view begins exit animation:
   - Hero logo scales up 1.2x with glow intensify
   - Action cards fade out with stagger (100ms apart)
   - Overall opacity goes to 0
4. **Frame 400ms** — Setup view removed from DOM
5. **Frame 400ms** — Monitor view enters:
   - Background: subtle flash of light (white overlay, opacity 0.08 → 0)
   - Cards enter with staggered slide-up (120ms apart per card)
   - Each card: `transform: translateY(20px) → translateY(0)`, `opacity: 0 → 1`
   - Ease: `cubic-bezier(0.16, 1, 0.3, 1)` (spring-like)
   - Duration: 500ms per card
6. **Frame 900ms** — Top bar transitions to full status strip
7. **Frame 1200ms** — All cards visible, first data refresh begins

**Monitor → Setup (on detach):**
1. **Frame 0ms** — Cards begin exit animation:
   - Staggered fade-out from bottom (cards closest to bottom exit first)
   - `transform: translateY(0) → translateY(16px)`, `opacity: 1 → 0`
   - Duration: 300ms, stagger 60ms
2. **Frame 600ms** — Monitor view removed from DOM
3. **Frame 600ms** — Setup view enters:
   - Hero logo: scale from 0.8 → 1, opacity 0 → 1
   - Action cards: slide-up with stagger (80ms apart)
   - Duration: 400ms per card
4. **Frame 1200ms** — Setup view fully visible

### CSS Keyframes

```css
/* View transitions */
.view-setup.entering {
  animation: setup-enter 0.6s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

.view-setup.exiting {
  animation: setup-exit 0.4s cubic-bezier(0.55, 0, 1, 0.45) forwards;
}

.view-monitor.entering {
  animation: monitor-enter 0.5s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

.view-monitor.exiting {
  animation: monitor-exit 0.4s cubic-bezier(0.55, 0, 1, 0.45) forwards;
}

@keyframes setup-enter {
  0% { opacity: 0; transform: scale(0.96); }
  100% { opacity: 1; transform: scale(1); }
}

@keyframes setup-exit {
  0% { opacity: 1; transform: scale(1); }
  100% { opacity: 0; transform: scale(1.04); }
}

@keyframes monitor-enter {
  0% { opacity: 0; }
  100% { opacity: 1; }
}

@keyframes monitor-exit {
  0% { opacity: 1; }
  100% { opacity: 0; }
}

/* Card entrance (monitor view) */
.widget-card.entrance {
  opacity: 0;
  transform: translateY(20px);
}

.widget-card.entrance.active {
  animation: card-entrance 0.5s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

@keyframes card-entrance {
  0% { opacity: 0; transform: translateY(20px) scale(0.97); }
  100% { opacity: 1; transform: translateY(0) scale(1); }
}

/* Setup card entrance */
.setup-card.entrance {
  opacity: 0;
  transform: translateY(16px);
}

.setup-card.entrance.active {
  animation: setup-card-entrance 0.4s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

@keyframes setup-card-entrance {
  0% { opacity: 0; transform: translateY(16px); }
  100% { opacity: 1; transform: translateY(0); }
}

/* Connecting state */
.connecting-dots::after {
  content: '';
  animation: dots 1.5s steps(4, end) infinite;
}

@keyframes dots {
  0% { content: ''; }
  25% { content: '.'; }
  50% { content: '..'; }
  75% { content: '...'; }
}

/* Flash overlay on monitor enter */
.view-flash {
  position: fixed;
  inset: 0;
  background: radial-gradient(ellipse at center, rgba(143, 188, 187, 0.08), transparent 70%);
  pointer-events: none;
  z-index: 1000;
  animation: flash-fade 0.8s ease-out forwards;
}

@keyframes flash-fade {
  0% { opacity: 1; }
  100% { opacity: 0; }
}
```

## Setup View Design

### Hero Section

```
                 [Animated Logo]
              (breathing glow pulse)

              Llama Monitor
         Real-time telemetry for your inference stack
```

- Logo: Same SVG as current, 80x80, centered
- Breathing animation: subtle scale (1 → 1.05 → 1) and glow pulse, 4s cycle
- Title: 32px, weight 800, white, tight letter-spacing
- Subtitle: 14px, weight 400, muted color, 2px letter-spacing

### Action Cards

Two cards side by side (responsive: stack on mobile):

```
┌──────────────────────────┐  ┌──────────────────────────┐
│        ⚡                │  │        ▶                 │
│   Attach to Endpoint     │  │   Spawn Local Server     │
│                          │  │                          │
│   Connect to a running   │  │   Start a llama.cpp      │
│   llama.cpp server       │  │   server from preset     │
│                          │  │                          │
│  [http://127.0.0.1:8080]│  │  [Select preset    ▼]   │
│                          │  │                          │
│     [  ⚡ Attach  ]      │  │     [  ▶ Start  ]       │
│                          │  │                          │
│   Browse saved sessions →│  │   Manage presets →       │
└──────────────────────────┘  └──────────────────────────┘
```

**Card styling:**
- Width: 340px each, gap 24px
- Background: `rgba(31, 35, 42, 0.6)` with `backdrop-filter: blur(20px)`
- Border: `1px solid rgba(255, 255, 255, 0.08)`
- Border-radius: `var(--radius-card)` (20px)
- Padding: 32px
- Hover: border brightens to `rgba(143, 188, 187, 0.3)`, subtle lift (`translateY(-2px)`)
- Hover box-shadow: `0 12px 40px rgba(0, 0, 0, 0.4), 0 0 30px rgba(143, 188, 187, 0.08)`

**Primary button (Attach):**
- Gradient: `linear-gradient(135deg, #88c0d1, #8fbcbb)`
- Text: dark, weight 700
- Hover: glow intensifies, subtle scale (1.02)
- Active: scale (0.98)

**Secondary button (Start):**
- Border: `1px solid rgba(143, 188, 187, 0.4)`
- Background: `rgba(143, 188, 187, 0.08)`
- Text: `#8fbcbb`, weight 700
- Hover: background brightens to `rgba(143, 188, 187, 0.16)`

**Input styling:**
- Background: `rgba(255, 255, 255, 0.04)`
- Border: `1px solid rgba(255, 255, 255, 0.1)`
- Border-radius: 10px
- Padding: 10px 14px
- Font: mono, 13px
- Focus: border `rgba(143, 188, 187, 0.5)`, glow `0 0 16px rgba(143, 188, 187, 0.12)`

### Ambient Background

- Subtle animated gradient mesh behind setup view
- Two large radial gradients, slow drift animation (20s cycle)
- Colors: `rgba(143, 188, 187, 0.03)` and `rgba(180, 142, 173, 0.02)`
- Creates depth without distraction

```css
.setup-bg {
  position: fixed;
  inset: 0;
  z-index: -1;
  overflow: hidden;
}

.setup-bg::before,
.setup-bg::after {
  content: '';
  position: absolute;
  width: 600px;
  height: 600px;
  border-radius: 50%;
  filter: blur(80px);
  animation: bg-drift 20s ease-in-out infinite;
}

.setup-bg::before {
  background: rgba(143, 188, 187, 0.03);
  top: -200px;
  left: -100px;
}

.setup-bg::after {
  background: rgba(180, 142, 173, 0.02);
  bottom: -200px;
  right: -100px;
  animation-delay: -10s;
}

@keyframes bg-drift {
  0%, 100% { transform: translate(0, 0) scale(1); }
  33% { transform: translate(40px, 30px) scale(1.1); }
  66% { transform: translate(-30px, -20px) scale(0.95); }
}
```

### Icon Animations

**Attach card icon (network):**
- SVG network/endpoint icon
- Continuous pulse: scale 1 → 1.1 → 1, 3s cycle
- Glow: drop-shadow that pulses with scale

**Spawn card icon (server):**
- SVG server/GPU icon
- Shimmer sweep across icon: linear gradient mask, 4s cycle
- Subtle rotation: -2° → 2° → -2°, 6s cycle

### Quick Stats (Last Session)

Below the action cards, if previous session data exists:

```
┌─────────────────────────────────────────────┐
│   1240 t/s        52 t/s          Local     │
│   Last prompt   Last gen rate   Last session│
└─────────────────────────────────────────────┘
```

- Background: `rgba(255, 255, 255, 0.02)`
- Border: `1px solid rgba(255, 255, 255, 0.04)`
- Border-radius: 12px
- Values: mono, 18px, weight 700, primary color
- Labels: 11px, weight 500, muted color
- Entrance: fade-in with 200ms delay after cards

## Monitor View (Existing)

No changes to the existing monitor view layout. The current dashboard sections remain:
- Model Preset
- Inference Metrics (Throughput, Generation, Context)
- GPU
- System

### Card Entrance Animation

When transitioning from setup → monitor, each card enters with staggered animation:

```javascript
function animateCardsEnter() {
  const cards = document.querySelectorAll('.widget-card');
  cards.forEach((card, i) => {
    card.classList.add('entrance');
    setTimeout(() => card.classList.add('active'), 120 * i);
  });
}
```

Stagger order (by visual priority):
1. Throughput card (0ms)
2. Generation card (120ms)
3. Context card (240ms)
4. GPU card (360ms)
5. System card (480ms)

### Card Exit Animation

When transitioning from monitor → setup:

```javascript
function animateCardsExit() {
  const cards = [...document.querySelectorAll('.widget-card')].reverse();
  cards.forEach((card, i) => {
    card.style.transition = `opacity 0.3s ease ${60 * i}ms, transform 0.3s ease ${60 * i}ms`;
    card.style.opacity = '0';
    card.style.transform = 'translateY(16px)';
  });
  // Remove view after animation
  setTimeout(() => switchView('setup'), 600);
}
```

## State Management

### View State

```javascript
const appState = {
  view: 'setup' | 'monitor' | 'transitioning',
  sessionActive: false,
  lastSessionData: null  // For quick stats on setup view
};

function switchView(targetView) {
  if (appState.view === 'transitioning') return;
  appState.view = 'transitioning';

  const currentView = document.getElementById('view-' + appState.view);
  const targetViewEl = document.getElementById('view-' + targetView);

  if (targetView === 'monitor') {
    // Exit setup, enter monitor
    currentView.classList.add('exiting');
    setTimeout(() => {
      currentView.style.display = 'none';
      currentView.classList.remove('exiting');
      targetViewEl.style.display = '';
      targetViewEl.classList.add('entering');
      animateCardsEnter();
      // Show flash overlay
      showFlashOverlay();
      setTimeout(() => {
        targetViewEl.classList.remove('entering');
        appState.view = 'monitor';
      }, 500);
    }, 400);
  } else {
    // Exit monitor, enter setup
    animateCardsExit();
    setTimeout(() => {
      currentView.style.display = 'none';
      currentView.classList.remove('exiting');
      targetViewEl.style.display = '';
      targetViewEl.classList.add('entering');
      animateSetupCardsEnter();
      setTimeout(() => {
        targetViewEl.classList.remove('entering');
        appState.view = 'setup';
      }, 400);
    }, 600);
  }
}
```

### Persistence

- Last session data saved to `localStorage` key `llama-monitor-last-session`
- Includes: last prompt rate, last gen rate, last session name, timestamp
- Used to populate quick stats on setup view
- Expires after 24 hours (check timestamp on load)

## Responsive Design

### Desktop (> 900px)
- Two action cards side by side, centered
- Hero section above cards
- Quick stats below cards

### Tablet (600–900px)
- Two action cards side by side, narrower (280px each)
- Hero section compacted (smaller logo, tighter spacing)

### Mobile (< 600px)
- Action cards stacked vertically, full width
- Hero section minimal (logo + title only)
- Quick stats in horizontal scroll

## Accessibility

- Setup view: `role="main"`, `aria-label="Setup"`
- Monitor view: `role="main"`, `aria-label="Dashboard"`
- View transition: `aria-live="polite"` announcement
- Focus management: on view switch, focus moves to first interactive element
- Keyboard: Enter on input triggers action, Tab order follows visual flow

## Implementation Checklist

- [ ] Create `view-setup` HTML structure
- [ ] Create `view-monitor` wrapper around existing dashboard
- [ ] Write CSS for setup view (hero, cards, inputs, buttons)
- [ ] Write CSS for ambient background
- [ ] Write CSS for icon animations
- [ ] Write CSS for view transitions
- [ ] Write CSS for card entrance/exit animations
- [ ] Implement `switchView()` function
- [ ] Implement `animateCardsEnter()` / `animateCardsExit()`
- [ ] Implement `animateSetupCardsEnter()`
- [ ] Wire up top bar adaptation per view
- [ ] Implement last session data persistence
- [ ] Implement quick stats rendering
- [ ] Add connecting state animation
- [ ] Add flash overlay
- [ ] Responsive breakpoints
- [ ] Accessibility attributes
- [ ] Test transition timing
- [ ] Test focus management
