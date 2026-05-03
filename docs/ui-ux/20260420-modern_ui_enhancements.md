# Modern UI Enhancements for Llama Monitor (April 2026)

**Document Version**: 5.0.0 (Phase 5 - In Progress)
**Date**: 2026-04-20
**Last Updated**: 2026-04-20
**Status**: Phase 1-4 Complete ✅ (UI/UX Modernization All Phases Implemented - Phase 5 In Progress)

---

## 🎯 Implementation Priority

### Phase 1: Quick Wins (Week 1) - CRITICAL ✅ COMPLETE
- ✅ Color system updates (CSS variables)
- ✅ Button modernization
- ✅ Card styling enhancements
- ✅ Loading state improvements

### Phase 2: Core Visuals (Week 2-3) - HIGH PRIORITY ✅ COMPLETE
- ✅ Typography system (Inter + Fira Code fonts, system variables)
- ✅ Icon library setup
- ✅ Glassmorphism panels (backdrop blur, semi-transparent backgrounds)
- ✅ Shadow hierarchy system (surface, elevated, floating levels)

### Phase 3: Interactive Elements (Week 4-5) - MEDIUM PRIORITY ✅ COMPLETE (CSS + JavaScript)
- ✅ Animation system (slide-up, fade-in, scale-in)
- ✅ Input field upgrades (focus states, validation colors)
- ✅ Modal improvements (overlay, header, body, buttons, fields, typography)
- ✅ Toast notifications (CSS styles, HTML structure, JavaScript functionality)

### Phase 4: Dashboard Redesign (Week 6-7) - MEDIUM PRIORITY ✅ COMPLETE
- ✅ Grid system (CSS variables, responsive breakpoints, utility classes)
- ✅ Dashboard layout (dashboard-grid, header, stats, main, sidebar)
- ✅ Card widgets (widget-card, widget-metric, widget-chart, widget-status)
- ✅ Responsive layout (media queries for all breakpoints)
- ✅ Updated HTML structure with grid classes

### Phase 5: UX Polish (Week 8+) - LOW PRIORITY - IN PROGRESS
- Customization (personalization, themes, user preferences)
- Keyboard shortcuts (accelerators, global shortcuts)
- Advanced features (analytics, reporting, export options)

---

## 📋 Executive Summary

This document provides **comprehensive implementation guidance for AI agents** to modernize the Llama Monitor UI/UX.

**Target**: Modern, premium aesthetic with glassmorphism, gradients, micro-interactions, and responsive design.

---

## 🏗️ Core Design Principles

1. **Premium Visual Quality** - Modern gradients, glassmorphism, micro-interactions
2. **Context-Aware UI** - Interface adapts to current state and user intent
3. **Instant Feedback** - No waiting, every action has immediate visual response
4. **Progressive Disclosure** - Show only what's needed, reveal more on demand
5. **Delightful Micro-Interactions** - Animations that feel natural and satisfying
6. **AI-Agent Ready** - Clear, specific instructions with code examples

---

## 🎨 Visual Modernization

### 1. Dark Theme Refinement

**Current**: Basic dark theme with limited depth  
**Target**: Modern dark theme with glassmorphism, gradients, and depth hierarchy

**Files to Modify**:
- `static/style.css` (add CSS variables, panel styles, card styles)

```css
/* ADD TO CSS VARIABLES SECTION */
:root {
  /* Palette */
  --color-bg: #0f1115;
  --color-bg-surface: #16191e;
  --color-bg-elevated: #1f232a;
  --color-bg-floating: #2a2f3a;
  
  /* Borders */
  --border-subtle: rgba(255, 255, 255, 0.05);
  --border-emphasis: rgba(255, 255, 255, 0.1);
  
  /* Shadows */
  --shadow-surface: 
    0 2px 8px rgba(0, 0, 0, 0.3),
    0 8px 24px rgba(0, 0, 0, 0.2);
  --shadow-elevated: 
    0 4px 12px rgba(0, 0, 0, 0.4),
    0 12px 32px rgba(0, 0, 0, 0.3);
  --shadow-floating: 
    0 8px 24px rgba(0, 0, 0, 0.5),
    0 24px 64px rgba(0, 0, 0, 0.4);
  
  /* Radius */
  --radius-base: 12px;
  --radius-card: 24px;
  
  /* Gradients */
  --gradient-primary: linear-gradient(160deg, #6366f1 0%, #8b5cf6 50%, #06b6d4 100%);
  --gradient-success: linear-gradient(135deg, #10b981 0%, #14b8a6 100%);
  --gradient-warning: linear-gradient(135deg, #f59e0b 0%, #f97316 100%);
  --gradient-error: linear-gradient(135deg, #f43f5e 0%, #ef4444 100%);
}

/* ADD PANEL STYLES */
.panel {
  background: rgba(30, 36, 43, 0.7);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-base);
  box-shadow: var(--shadow-surface);
}

/* ADD CARD STYLES */
.card {
  background: linear-gradient(145deg, rgba(40, 48, 58, 0.8), rgba(30, 36, 43, 0.9));
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.card:hover {
  transform: translateY(-4px);
  box-shadow: var(--shadow-elevated);
  border-color: rgba(99, 102, 241, 0.3);
}
```

**Testing Checklist**:
- [ ] Panels show backdrop blur effect
- [ ] Cards lift on hover with subtle gradient border
- [ ] Shadows match design specs
- [ ] Border radius consistent (12px panels, 24px cards)

---

### 2. Color System Overhaul

**Files to Modify**: `static/style.css`

```css
/* PRIMARY COLORS */
--color-primary: #6366f1;
--color-primary-dark: #4f46e5;
--color-primary-light: #818cf8;

/* GRADIENTS */
--gradient-primary: linear-gradient(160deg, #6366f1 0%, #8b5cf6 50%, #06b6d4 100%);
--gradient-primary-2: linear-gradient(160deg, #06b6d4 0%, #6366f1 100%);

--gradient-success: linear-gradient(135deg, #10b981 0%, #14b8a6 100%);
--gradient-warning: linear-gradient(135deg, #f59e0b 0%, #f97316 100%);
--gradient-error: linear-gradient(135deg, #f43f5e 0%, #ef4444 100%);
--gradient-neutral: linear-gradient(135deg, #6b7280 0%, #9ca3af 100%);

/* SEMANTIC COLORS */
--color-gpu: #10b981;
--color-cpu: #3b82f6;
--color-network: #8b5cf6;
--color-storage: #f59e0b;

/* NEUTRAL PALETTE */
--color-gray-50: #f9fafb;
--color-gray-100: #f3f4f6;
--color-gray-200: #e5e7eb;
--color-gray-300: #d1d5db;
--color-gray-400: #9ca3af;
--color-gray-500: #6b7280;
--color-gray-600: #4b5563;
--color-gray-700: #374151;
--color-gray-800: #1f2937;
--color-gray-900: #111827;
--color-gray-950: #030712;

/* TEXT COLORS */
--color-text-primary: #f9fafb;
--color-text-secondary: #d1d5db;
--color-text-muted: #9ca3af;
--color-text-on-primary: #ffffff;
--color-text-on-secondary: #1f2937;
```

**Usage Examples**:
```css
/* Button variants */
.btn-primary {
  background: var(--gradient-primary);
  color: var(--color-text-on-primary);
}

.btn-success {
  background: var(--gradient-success);
  color: var(--color-text-on-primary);
}

.text-gpu { color: var(--color-gpu); }
.text-cpu { color: var(--color-cpu); }

.border-gpu {
  border: 1px solid var(--color-gpu);
  box-shadow: 0 0 0 3px rgba(16, 185, 129, 0.1);
}
```

---

### 3. Typography Modernization

**Files to Modify**: `static/style.css`

```css
/* FONTS */
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=Fira+Code:wght@400;500;600&display=swap');

:root {
  /* Font Families */
  --font-display: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
  --font-body: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
  --font-mono: 'Fira Code', 'Courier New', monospace;
  
  /* Font Weights */
  --font-normal: 400;
  --font-medium: 500;
  --font-semibold: 600;
  --font-bold: 700;
  --font-extrabold: 800;
  
  /* Font Sizes */
  --text-xs: 0.75rem;    /* 12px */
  --text-sm: 0.875rem;   /* 14px */
  --text-base: 1rem;     /* 16px */
  --text-lg: 1.125rem;   /* 18px */
  --text-xl: 1.25rem;    /* 20px */
  --text-2xl: 1.5rem;    /* 24px */
  --text-3xl: 1.875rem;  /* 30px */
  --text-4xl: 2.25rem;   /* 36px */
  
  /* Line Heights */
  --leading-tight: 1.25;
  --leading-normal: 1.5;
  --leading-relaxed: 1.625;
  
  /* Letter Spacing */
  --tracking-tight: -0.02em;
  --tracking-normal: 0em;
}

/* TYPOGRAPHY CLASSES */
h1, .text-display {
  font-family: var(--font-display);
  font-weight: var(--font-extrabold);
  font-size: var(--text-3xl);
  line-height: var(--leading-tight);
  letter-spacing: var(--tracking-tight);
  color: var(--color-text-primary);
}

h2, .text-headline {
  font-family: var(--font-display);
  font-weight: var(--font-bold);
  font-size: var(--text-2xl);
  line-height: var(--leading-tight);
  color: var(--color-text-primary);
}

h3, .text-title {
  font-family: var(--font-display);
  font-weight: var(--font-semibold);
  font-size: var(--text-xl);
  line-height: var(--leading-normal);
  color: var(--color-text-primary);
}

p, .text-body {
  font-family: var(--font-body);
  font-weight: var(--font-normal);
  font-size: var(--text-base);
  line-height: var(--leading-relaxed);
  color: var(--color-text-secondary);
}

.monospace, .text-mono {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  color: var(--color-text-secondary);
}

.code {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  background: rgba(0, 0, 0, 0.3);
  padding: 2px 6px;
  border-radius: 4px;
  color: var(--color-primary);
}
```

**Testing Checklist**:
- [ ] All fonts load correctly from Google Fonts
- [ ] Text sizes match specifications
- [ ] Line heights consistent
- [ ] Monospace fonts used for code/terminal

---

### 4. Icon System

**Files to Modify**: `static/index.html` (add icon library), `static/style.css` (icon styles)

```html
<!-- Add to static/index.html head section -->
<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.4.0/css/all.min.css">
```

```css
/* ICON STYLES */
i, .icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  font-size: 16px;
  color: var(--color-text-secondary);
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

i.large, .icon.large {
  width: 32px;
  height: 32px;
  font-size: 20px;
}

i.xlarge, .icon.xlarge {
  width: 48px;
  height: 48px;
  font-size: 28px;
}

i.primary, .icon.primary {
  color: var(--color-primary);
}

i.success, .icon.success {
  color: var(--color-gpu);
}

i.warning, .icon.warning {
  color: var(--color-warning);
}

i.error, .icon.error {
  color: var(--color-error);
}

/* ANIMATIONS */
@keyframes icon-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.icon-pulse {
  animation: icon-pulse 2s cubic-bezier(0.4, 0, 0.2, 1) infinite;
}

@keyframes icon-spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.icon-spin {
  animation: icon-spin 1s linear infinite;
}

.icon-bounce {
  animation: icon-pulse 0.6s cubic-bezier(0.68, -0.55, 0.27, 1.55) 2;
}

.icon-fade-in {
  animation: fade-in 0.3s ease-out;
}
```

**Icon Usage Examples**:
```html
<!-- System Status -->
<i class="fas fa-microchip icon" title="CPU"></i>

<!-- Status Indicators -->
<i class="fas fa-circle icon-primary icon-pulse" title="Online"></i>

<!-- Action Buttons -->
<button class="btn-icon">
  <i class="fas fa-wrench"></i>
</button>

<!-- Large Icons -->
<i class="fas fa-server icon xlarge primary"></i>
```

---

## 🧱 UI Component Modernization

### 1. Dashboard Cards

**Files to Modify**: `static/index.html`, `static/style.css`

```css
/* DASHBOARD CARD STYLES */
.card-dashboard {
  background: linear-gradient(145deg, rgba(40, 48, 58, 0.95), rgba(30, 36, 43, 1));
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  padding: 20px;
  position: relative;
  overflow: hidden;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.card-dashboard::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 2px;
  background: var(--gradient-primary);
  transform: scaleX(0);
  transition: transform 0.3s ease;
}

.card-dashboard:hover::before {
  transform: scaleX(1);
}

.card-dashboard:hover {
  transform: translateY(-4px);
  box-shadow: var(--shadow-elevated);
  border-color: rgba(99, 102, 241, 0.3);
}

/* CARD HEADER */
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border-subtle);
}

.card-title {
  font-weight: var(--font-semibold);
  color: var(--color-text-primary);
  font-size: var(--text-lg);
}

.card-action {
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: color 0.2s;
}

.card-action:hover {
  color: var(--color-primary);
}

/* CARD CONTENT */
.card-content {
  display: flex;
  align-items: center;
  gap: 16px;
}

.card-metric {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.metric-label {
  font-size: var(--text-xs);
  color: var(--color-text-muted);
  font-weight: var(--font-medium);
}

.metric-value {
  font-size: var(--text-2xl);
  font-weight: var(--font-extrabold);
  color: var(--color-text-primary);
  font-variant-numeric: tabular-nums;
}

.metric-trend {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
}

.metric-trend.up {
  color: var(--color-gpu);
}

.metric-trend.down {
  color: var(--color-error);
}

/* CARD FOOTER */
.card-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 16px;
  padding-top: 12px;
  border-top: 1px solid var(--border-subtle);
}

.card-stats {
  display: flex;
  gap: 16px;
}

.stat-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.stat-label {
  font-size: var(--text-xs);
  color: var(--color-text-muted);
}

.stat-value {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--color-text-primary);
}
```

**HTML Structure**:
```html
<div class="card-dashboard">
  <div class="card-header">
    <h3 class="card-title">System Monitor</h3>
    <button class="card-action" title="Expand">
      <i class="fas fa-expand"></i>
    </button>
  </div>
  
  <div class="card-content">
    <div class="card-metric">
      <span class="metric-label">CPU</span>
      <span class="metric-value">45%</span>
      <span class="metric-trend up">↑ 5%</span>
    </div>
    <div class="card-metric">
      <span class="metric-label">RAM</span>
      <span class="metric-value">6.2 GB</span>
      <span class="metric-trend down">↓ 1.2 GB</span>
    </div>
    <div class="card-metric">
      <span class="metric-label">GPU</span>
      <span class="metric-value">38%</span>
      <span class="metric-trend">—</span>
    </div>
  </div>
  
  <div class="card-footer">
    <div class="card-stats">
      <div class="stat-item">
        <span class="stat-label">Temp</span>
        <span class="stat-value">68°C</span>
      </div>
      <div class="card-stats">
        <div class="stat-item">
          <span class="stat-label">Fan</span>
          <span class="stat-value">45%</span>
        </div>
      </div>
    </div>
  </div>
</div>
```

---

### 2. Buttons

**Files to Modify**: `static/style.css`

```css
/* BUTTON VARIABLES */
:root {
  --btn-height: 44px;
  --btn-padding: 16px 24px;
  --btn-radius: 12px;
  --btn-icon-size: 20px;
}

/* BASE BUTTON */
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: var(--btn-padding);
  font-family: var(--font-body);
  font-weight: var(--font-medium);
  font-size: var(--text-base);
  line-height: 1.5;
  text-decoration: none;
  border: none;
  border-radius: var(--btn-radius);
  cursor: pointer;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  position: relative;
  overflow: hidden;
  user-select: none;
}

.btn:focus {
  outline: none;
  box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.3);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  transform: none !important;
}

/* BUTTON SIZES */
.btn-sm {
  padding: 8px 16px;
  font-size: var(--text-sm);
  border-radius: 8px;
  height: 36px;
}

.btn-lg {
  padding: 16px 32px;
  font-size: var(--text-lg);
  border-radius: 16px;
  height: 52px;
}

.btn-icon {
  width: 44px;
  height: 44px;
  padding: 0;
  border-radius: 50%;
}

.btn-icon-sm {
  width: 36px;
  height: 36px;
  padding: 0;
  border-radius: 50%;
}

.btn-icon-lg {
  width: 52px;
  height: 52px;
  padding: 0;
  border-radius: 50%;
}

/* BUTTON VARIANTS */

/* Primary - Gradient */
.btn-primary {
  background: var(--gradient-primary);
  color: var(--color-text-on-primary);
  box-shadow: 0 4px 12px rgba(99, 102, 241, 0.3);
}

.btn-primary:hover {
  background: var(--gradient-primary-2);
  transform: translateY(-2px);
  box-shadow: 0 8px 24px rgba(99, 102, 241, 0.4);
}

.btn-primary:active {
  transform: translateY(0);
  box-shadow: 0 2px 8px rgba(99, 102, 241, 0.3);
}

/* Success */
.btn-success {
  background: var(--gradient-success);
  color: var(--color-text-on-primary);
  box-shadow: 0 4px 12px rgba(16, 185, 129, 0.3);
}

/* Warning */
.btn-warning {
  background: var(--gradient-warning);
  color: var(--color-text-on-primary);
  box-shadow: 0 4px 12px rgba(245, 158, 11, 0.3);
}

/* Error */
.btn-error {
  background: var(--gradient-error);
  color: var(--color-text-on-primary);
  box-shadow: 0 4px 12px rgba(244, 63, 94, 0.3);
}

/* Ghost - Transparent with gradient hover */
.btn-ghost {
  background: transparent;
  color: var(--color-text-secondary);
  box-shadow: none;
}

.btn-ghost:hover {
  background: rgba(255, 255, 255, 0.05);
  color: var(--color-text-primary);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
}

.btn-ghost:active {
  background: rgba(255, 255, 255, 0.08);
}

/* Outline - Border with gradient on hover */
.btn-outline {
  background: transparent;
  border: 1px solid var(--border-subtle);
  color: var(--color-text-secondary);
  box-shadow: none;
}

.btn-outline:hover {
  border-color: rgba(99, 102, 241, 0.5);
  color: var(--color-primary);
  box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.1);
}

.btn-outline:active {
  border-color: var(--color-primary);
}

/* Button with Icon */
.btn i, .btn .icon {
  transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.btn:hover i, .btn:hover .icon {
  transform: translateX(4px);
}

.btn-icon .fas, .btn-icon .icon {
  margin: 0;
}
```

**HTML Examples**:
```html
<!-- Primary Button -->
<button class="btn btn-primary">
  <i class="fas fa-magic"></i>
  Install & Start
</button>

<!-- Ghost Button -->
<button class="btn btn-ghost">
  Cancel
</button>

<!-- Outline Button -->
<button class="btn btn-outline">
  <i class="fas fa-wrench"></i>
  Configure
</button>

<!-- Icon Button -->
<button class="btn btn-icon" title="Settings">
  <i class="fas fa-cog"></i>
</button>

<!-- Small Button -->
<button class="btn btn-sm btn-success">
  <i class="fas fa-check"></i>
  Save
</button>

<!-- Large Button -->
<button class="btn btn-lg btn-primary">
  <i class="fas fa-rocket"></i>
  Launch Model
</button>
```

---

### 3. Input Fields

**Files to Modify**: `static/style.css`

```css
/* INPUT VARIABLES */
:root {
  --input-height: 44px;
  --input-padding: 12px 16px;
  --input-radius: 12px;
  --input-border: 1px solid var(--border-subtle);
  --input-bg: rgba(30, 36, 43, 0.5);
}

/* INPUT BASE */
.input-wrapper {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.input-label {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--color-text-primary);
  display: flex;
  align-items: center;
  gap: 6px;
}

.input-label .required {
  color: var(--color-error);
  font-size: var(--text-base);
}

.input-field {
  display: flex;
  align-items: center;
  height: var(--input-height);
  padding: var(--input-padding);
  font-family: var(--font-body);
  font-size: var(--text-base);
  color: var(--color-text-primary);
  background: var(--input-bg);
  border: var(--input-border);
  border-radius: var(--radius-base);
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  outline: none;
}

.input-field:hover {
  border-color: rgba(255, 255, 255, 0.1);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
}

.input-field:focus {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.1);
}

.input-field::placeholder {
  color: var(--color-text-muted);
}

.input-field:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* INPUT WITH ICON */
.input-icon-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.input-icon-left i {
  color: var(--color-text-muted);
  pointer-events: none;
}

.input-icon-left input {
  padding-left: 40px;
}

.input-icon-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.input-icon-right input {
  padding-right: 40px;
}

.input-icon-right .action-icon {
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: color 0.2s;
  pointer-events: none;
}

.input-icon-right .action-icon:hover {
  color: var(--color-primary);
}

/* VALIDATION STATES */
.input-field.success {
  border-color: var(--color-gpu);
  box-shadow: 0 0 0 3px rgba(16, 185, 129, 0.1);
}

.input-field.success + .input-icon-right .check-icon {
  color: var(--color-gpu);
}

.input-field.error {
  border-color: var(--color-error);
  box-shadow: 0 0 0 3px rgba(244, 63, 94, 0.1);
}

.input-field.error + .input-icon-right .error-icon {
  color: var(--color-error);
}

.input-field.warning {
  border-color: var(--color-warning);
  box-shadow: 0 0 0 3px rgba(245, 158, 11, 0.1);
}

.input-field.warning + .input-icon-right .warning-icon {
  color: var(--color-warning);
}

/* ERROR MESSAGE */
.input-error-message {
  font-size: var(--text-xs);
  color: var(--color-error);
  display: flex;
  align-items: center;
  gap: 4px;
}

/* PASSWORD TOGGLE */
.input-password {
  padding-right: 48px !important;
}

.password-toggle {
  position: absolute;
  right: 12px;
  top: 50%;
  transform: translateY(-50%);
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: color 0.2s;
}

.password-toggle:hover {
  color: var(--color-primary);
}

/* CHAR COUNTER */
.input-counter {
  font-size: var(--text-xs);
  color: var(--color-text-muted);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.input-counter .current {
  font-weight: var(--font-medium);
  color: var(--color-text-primary);
}

/* TEXTAREA */
textarea.input-field {
  min-height: 120px;
  resize: vertical;
  padding: 12px 16px;
}

textarea.input-field::placeholder {
  color: var(--color-text-muted);
}
```

**HTML Examples**:
```html
<!-- Standard Input -->
<div class="input-wrapper">
  <label class="input-label">
    Model Path
    <span class="required">*</span>
  </label>
  <div class="input-wrapper">
    <div class="input-field input-icon-left">
      <i class="fas fa-folder"></i>
      <input type="text" placeholder="/home/user/models/llama-7b.Q4_K_M.gguf">
    </div>
  </div>
</div>

<!-- Input with Icons -->
<div class="input-wrapper">
  <label class="input-label">Context Size</label>
  <div class="input-wrapper">
    <div class="input-field input-icon-right">
      <input type="number" id="context-input" value="4096">
      <i class="fas fa-check check-icon" style="display: none;"></i>
      <i class="fas fa-xmark error-icon" style="display: none;"></i>
    </div>
    <div class="input-error-message" style="display: none;">
      <i class="fas fa-exclamation-circle"></i>
      Invalid value
    </div>
  </div>
</div>

<!-- Password Input -->
<div class="input-wrapper">
  <label class="input-label">SSH Password</label>
  <div class="input-wrapper">
    <div class="input-field input-password input-icon-right">
      <input type="password" id="ssh-password" placeholder="••••••••">
      <i class="fas fa-eye password-toggle" id="toggle-password"></i>
    </div>
  </div>
</div>

<!-- Input with Character Counter -->
<div class="input-wrapper">
  <label class="input-label">Description</label>
  <div class="input-wrapper">
    <textarea class="input-field" id="description" maxlength="200" placeholder="Enter model description..."></textarea>
    <div class="input-counter">
      <span>0 / 200 characters</span>
      <span class="current" id="char-count">0</span>
    </div>
  </div>
</div>
```

---

### 4. Modals & Overlays

**Files to Modify**: `static/style.css`

```css
/* MODAL OVERLAY */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  animation: fade-in 0.3s ease-out;
}

.modal-overlay.hidden {
  opacity: 0;
  pointer-events: none;
}

/* MODAL CONTENT */
.modal-content {
  background: rgba(30, 36, 43, 0.95);
  backdrop-filter: blur(24px);
  -webkit-backdrop-filter: blur(24px);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-elevated);
  max-width: 600px;
  width: 100%;
  max-height: 90vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  animation: slide-up 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

/* MODAL HEADER */
.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px;
  border-bottom: 1px solid var(--border-subtle);
}

.modal-title {
  font-size: var(--text-xl);
  font-weight: var(--font-semibold);
  color: var(--color-text-primary);
  margin: 0;
}

.modal-close {
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  cursor: pointer;
  width: 40px;
  height: 40px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
}

.modal-close:hover {
  background: rgba(255, 255, 255, 0.05);
  color: var(--color-text-primary);
}

.modal-close:active {
  background: rgba(255, 255, 255, 0.1);
}

/* MODAL BODY */
.modal-body {
  padding: 24px;
  flex: 1;
  overflow-y: auto;
  max-height: calc(90vh - 160px);
}

/* MODAL FOOTER */
.modal-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
  padding: 16px 24px;
  border-top: 1px solid var(--border-subtle);
  background: rgba(0, 0, 0, 0.2);
}

/* MODAL SIZES */
.modal-sm {
  max-width: 400px;
}

.modal-md {
  max-width: 600px;
}

.modal-lg {
  max-width: 800px;
}

.modal-xl {
  max-width: 1000px;
}

/* MODAL LAYERS */
.modal-layer-1 { z-index: 1000; }
.modal-layer-2 { z-index: 1100; }
.modal-layer-3 { z-index: 1200; }

/* ANIMATIONS */
@keyframes slide-up {
  from {
    opacity: 0;
    transform: translateY(24px) scale(0.95);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}
```

**HTML Examples**:
```html
<!-- Full Modal Structure -->
<div class="modal-overlay">
  <div class="modal-content modal-md">
    <div class="modal-header">
      <h2 class="modal-title">New Session</h2>
      <button class="modal-close" aria-label="Close">
        <i class="fas fa-times"></i>
      </button>
    </div>
    
    <div class="modal-body">
      <!-- Form content here -->
      <div class="form-group">
        <label>Model</label>
        <select class="input-field">
          <option>llama-7b.Q4_K_M.gguf</option>
          <option>llama-13b.Q4_K_M.gguf</option>
        </select>
      </div>
    </div>
    
    <div class="modal-footer">
      <button class="btn btn-ghost">Cancel</button>
      <button class="btn btn-primary">
        <i class="fas fa-rocket"></i>
        Start Session
      </button>
    </div>
  </div>
</div>

<!-- Modal Variants -->
<div class="modal-overlay">
  <div class="modal-content modal-lg">
    <!-- Large modal content -->
  </div>
</div>

<div class="modal-overlay modal-layer-2">
  <div class="modal-content modal-xl">
    <!-- XL modal content -->
  </div>
</div>
```

---

### 5. Toast Notifications

**Files to Modify**: `static/style.css`

```css
/* TOAST CONTAINER */
.toast-container {
  position: fixed;
  top: 24px;
  right: 24px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  z-index: 2000;
  max-width: 400px;
}

/* TOAST BASE */
.toast {
  background: linear-gradient(145deg, rgba(30, 36, 43, 0.95), rgba(40, 48, 58, 0.98));
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  border: 1px solid var(--border-subtle);
  border-radius: 16px;
  box-shadow: var(--shadow-elevated);
  padding: 16px 20px;
  min-width: 300px;
  animation: slide-in-right 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  position: relative;
  overflow: hidden;
}

.toast:hover {
  border-color: rgba(99, 102, 241, 0.3);
}

/* TOAST TYPES */
.toast-success {
  border-left: 4px solid var(--color-gpu);
}

.toast-success .toast-icon {
  color: var(--color-gpu);
}

.toast-error {
  border-left: 4px solid var(--color-error);
}

.toast-error .toast-icon {
  color: var(--color-error);
}

.toast-warning {
  border-left: 4px solid var(--color-warning);
}

.toast-warning .toast-icon {
  color: var(--color-warning);
}

.toast-info {
  border-left: 4px solid var(--color-primary);
}

.toast-info .toast-icon {
  color: var(--color-primary);
}

/* TOAST CONTENT */
.toast-content {
  display: flex;
  align-items: flex-start;
  gap: 12px;
}

.toast-icon {
  font-size: 24px;
  min-width: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.toast-body {
  flex: 1;
  min-width: 0;
}

.toast-title {
  font-weight: var(--font-semibold);
  color: var(--color-text-primary);
  margin-bottom: 4px;
  font-size: var(--text-base);
}

.toast-message {
  font-size: var(--text-sm);
  color: var(--color-text-secondary);
  line-height: 1.5;
}

.toast-meta {
  font-size: var(--text-xs);
  color: var(--color-text-muted);
  margin-top: 4px;
}

/* TOAST ACTIONS */
.toast-actions {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.toast-action {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--color-primary);
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 6px;
  transition: background 0.2s;
}

.toast-action:hover {
  background: rgba(99, 102, 241, 0.1);
}

.toast-close {
  position: absolute;
  top: 12px;
  right: 12px;
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  cursor: pointer;
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.2s;
}

.toast-close:hover {
  color: var(--color-text-primary);
}

/* TOAST PROGRESS BAR */
.toast-progress {
  position: absolute;
  bottom: 0;
  left: 0;
  height: 3px;
  background: var(--gradient-primary);
  width: 0%;
  animation: progress-indeterminate 1.5s linear infinite;
}

.toast-progress.determinate {
  animation: none;
  transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

/* TOAST STATES */
.toast.hidden {
  animation: slide-out-right 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  opacity: 0;
  pointer-events: none;
}

/* ANIMATIONS */
@keyframes slide-in-right {
  from {
    opacity: 0;
    transform: translateX(100%) scale(0.95);
  }
  to {
    opacity: 1;
    transform: translateX(0) scale(1);
  }
}

@keyframes slide-out-right {
  from {
    opacity: 1;
    transform: translateX(0);
  }
  to {
    opacity: 0;
    transform: translateX(100%);
  }
}

@keyframes progress-indeterminate {
  0% { width: 0%; }
  50% { width: 40%; }
  100% { width: 100%; }
}
```

**HTML Examples**:
```html
<!-- Success Toast -->
<div class="toast toast-success" data-auto-dismiss="4000">
  <div class="toast-content">
    <div class="toast-icon">
      <i class="fas fa-check-circle"></i>
    </div>
    <div class="toast-body">
      <div class="toast-title">Installation Complete</div>
      <div class="toast-message">llama-7b.Q4_K_M.gguf installed successfully</div>
      <div class="toast-meta">2.7 GB • 38 seconds</div>
    </div>
    <button class="toast-close" aria-label="Close">
      <i class="fas fa-times"></i>
    </button>
  </div>
  <div class="toast-progress determinate" style="width: 100%;"></div>
</div>

<!-- Error Toast -->
<div class="toast toast-error" data-auto-dismiss="6000">
  <div class="toast-content">
    <div class="toast-icon">
      <i class="fas fa-exclamation-circle"></i>
    </div>
    <div class="toast-body">
      <div class="toast-title">Session Failed</div>
      <div class="toast-message">Port 8001 is already in use</div>
      <div class="toast-actions">
        <button class="toast-action">Retry</button>
        <button class="toast-action">Change Port</button>
      </div>
    </div>
    <button class="toast-close" aria-label="Close">
      <i class="fas fa-times"></i>
    </button>
  </div>
  <div class="toast-progress"></div>
</div>

<!-- Warning Toast -->
<div class="toast toast-warning" data-auto-dismiss="8000">
  <div class="toast-content">
    <div class="toast-icon">
      <i class="fas fa-exclamation-triangle"></i>
    </div>
    <div class="toast-body">
      <div class="toast-title">High GPU Temperature</div>
      <div class="toast-message">RTX 4090: 85°C (warning threshold)</div>
      <div class="toast-actions">
        <button class="toast-action">Learn More</button>
      </div>
    </div>
    <button class="toast-close" aria-label="Close">
      <i class="fas fa-times"></i>
    </button>
  </div>
  <div class="toast-progress"></div>
</div>

<!-- Info Toast -->
<div class="toast toast-info" data-auto-dismiss="4000">
  <div class="toast-content">
    <div class="toast-icon">
      <i class="fas fa-info-circle"></i>
    </div>
    <div class="toast-body">
      <div class="toast-title">New Update Available</div>
      <div class="toast-message">Version 0.3.0 is ready to install</div>
      <div class="toast-actions">
        <button class="toast-action">View Changelog</button>
      </div>
    </div>
    <button class="toast-close" aria-label="Close">
      <i class="fas fa-times"></i>
    </button>
  </div>
  <div class="toast-progress determinate" style="width: 100%;"></div>
</div>
```

---

### 6. Progress Indicators

**Files to Modify**: `static/style.css`

```css
/* PROGRESS VARIABLES */
:root {
  --progress-height: 8px;
  --progress-radius: 4px;
}

/* LINEAR PROGRESS BAR */
.progress-bar {
  width: 100%;
  height: var(--progress-height);
  background: rgba(0, 0, 0, 0.3);
  border-radius: var(--progress-radius);
  overflow: hidden;
}

.progress-bar.fill {
  height: 100%;
  background: var(--gradient-primary);
  border-radius: var(--progress-radius);
  transition: width 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  position: relative;
  overflow: hidden;
}

.progress-bar.fill::before {
  content: '';
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  left: 0;
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(255, 255, 255, 0.3) 50%,
    transparent 100%
  );
  animation: shimmer 1.5s infinite linear;
}

@keyframes shimmer {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(100%); }
}

/* PROGRESS TEXT */
.progress-text {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 8px;
  font-size: var(--text-sm);
}

.progress-label {
  color: var(--color-text-secondary);
}

.progress-percentage {
  font-weight: var(--font-semibold);
  color: var(--color-text-primary);
}

.progress-eta {
  color: var(--color-text-muted);
  font-size: var(--text-xs);
}

/* CIRCULAR PROGRESS */
.circular-progress {
  width: 48px;
  height: 48px;
  position: relative;
}

.circular-progress svg {
  width: 100%;
  height: 100%;
  transform: rotate(-90deg);
}

.circular-progress circle {
  fill: none;
  stroke-width: 4;
  stroke-linecap: round;
}

.circular-progress circle.background {
  stroke: rgba(0, 0, 0, 0.3);
}

.circular-progress circle.foreground {
  stroke: var(--gradient-primary);
  stroke-dasharray: 120;
  stroke-dashoffset: 120;
  transition: stroke-dashoffset 0.4s cubic-bezier(0.4, 0, 0.2, 1);
}

.circular-progress-text {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  color: var(--color-text-primary);
}

/* SPINNER */
.spinner {
  display: inline-block;
  width: 24px;
  height: 24px;
  border: 3px solid rgba(0, 0, 0, 0.3);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* SKELETON LOADER */
.skeleton {
  background: linear-gradient(90deg, rgba(255, 255, 255, 0.05) 25%, rgba(255, 255, 255, 0.15) 50%, rgba(255, 255, 255, 0.05) 75%);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite linear;
  border-radius: 4px;
}

.skeleton.line {
  height: 12px;
  width: 100%;
}

.skeleton.circle {
  width: 48px;
  height: 48px;
  border-radius: 50%;
}

.skeleton.rect {
  height: 80px;
  border-radius: 8px;
}

/* SHIMMER EFFECT */
.shimmer {
  background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.1), transparent);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite linear;
}

/* RIPPLE EFFECT */
.ripple {
  position: relative;
  overflow: hidden;
}

.ripple::after {
  content: '';
  position: absolute;
  top: 50%;
  left: 50%;
  width: 0;
  height: 0;
  background: rgba(255, 255, 255, 0.3);
  border-radius: 50%;
  transform: translate(-50%, -50%);
  transition: width 0.6s, height 0.6s;
}

.ripple:active::after {
  width: 200%;
  height: 200%;
}
```

**HTML Examples**:
```html
<!-- Linear Progress Bar -->
<div class="progress-text">
  <span class="progress-label">Installing model...</span>
  <span class="progress-percentage">72%</span>
</div>
<div class="progress-bar">
  <div class="progress-bar fill" style="width: 72%;"></div>
</div>
<div class="progress-text" style="margin-top: 4px;">
  <span class="progress-eta">≈ 2 min remaining</span>
</div>

<!-- Circular Progress -->
<div class="circular-progress" data-progress="68">
  <svg>
    <circle class="background" cx="24" cy="24" r="18"></circle>
    <circle class="foreground" cx="24" cy="24" r="18"></circle>
  </svg>
  <span class="circular-progress-text">68%</span>
</div>

<!-- Spinner in Button -->
<button class="btn btn-primary">
  <div class="spinner"></div>
  <span>Processing...</span>
</button>

<!-- Skeleton Loader -->
<div class="skeleton rect"></div>
<div class="skeleton line" style="width: 80%; margin-top: 8px;"></div>
<div class="skeleton line" style="width: 60%; margin-top: 4px;"></div>

<!-- Shimmer Effect -->
<div class="shimmer" style="height: 44px; width: 100%; border-radius: 12px;"></div>
```

---

## 🎬 Interactive Elements

### 1. Animations System

**Files to Modify**: `static/style.css`

```css
/* ANIMATION VARIABLES */
:root {
  --duration-quick: 200ms;
  --duration-medium: 400ms;
  --duration-slow: 600ms;
  
  --timing-ease: cubic-bezier(0.4, 0, 0.2, 1);
  --timing-spring: cubic-bezier(0.16, 1, 0.3, 1);
  --timing-decel: cubic-bezier(0.0, 0, 0.2, 1);
}

/* FADE ANIMATIONS */
@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

@keyframes fade-out {
  from { opacity: 1; }
  to { opacity: 0; }
}

.animate-fade-in {
  animation: fade-in var(--duration-medium) var(--timing-decel);
}

.animate-fade-out {
  animation: fade-out var(--duration-medium) var(--timing-decel);
}

/* SLIDE ANIMATIONS */
@keyframes slide-up {
  from { transform: translateY(24px); opacity: 0; }
  to { transform: translateY(0); opacity: 1; }
}

@keyframes slide-down {
  from { transform: translateY(-24px); opacity: 0; }
  to { transform: translateY(0); opacity: 1; }
}

@keyframes slide-left {
  from { transform: translateX(24px); opacity: 0; }
  to { transform: translateX(0); opacity: 1; }
}

@keyframes slide-right {
  from { transform: translateX(-24px); opacity: 0; }
  to { transform: translateX(0); opacity: 1; }
}

.animate-slide-up {
  animation: slide-up var(--duration-medium) var(--timing-ease);
}

.animate-slide-down {
  animation: slide-down var(--duration-medium) var(--timing-ease);
}

/* SCALE ANIMATIONS */
@keyframes scale-in {
  from { transform: scale(0.95); opacity: 0; }
  to { transform: scale(1); opacity: 1; }
}

@keyframes scale-out {
  from { transform: scale(1); opacity: 1; }
  to { transform: scale(0.95); opacity: 0; }
}

.animate-scale-in {
  animation: scale-in var(--duration-medium) var(--timing-spring);
}

.animate-scale-out {
  animation: scale-out var(--duration-medium) var(--timing-decel);
}

/* ROTATE ANIMATIONS */
@keyframes rotate-in {
  from { transform: rotate(-10deg); opacity: 0; }
  to { transform: rotate(0); opacity: 1; }
}

.animate-rotate-in {
  animation: rotate-in var(--duration-medium) var(--timing-spring);
}

/* PULSE ANIMATIONS */
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.animate-pulse {
  animation: pulse 2s cubic-bezier(0.4, 0, 0.2, 1) infinite;
}

/* BOUNCE ANIMATIONS */
@keyframes bounce {
  0%, 20%, 50%, 80%, 100% { transform: translateY(0); }
  40% { transform: translateY(-12px); }
  60% { transform: translateY(-6px); }
}

.animate-bounce {
  animation: bounce 0.6s var(--timing-spring);
}

/* TRANSFORM UTILITIES */
.hover-lift:hover {
  transform: translateY(-4px);
  transition: transform 0.3s var(--timing-ease);
}

.hover-scale:hover {
  transform: scale(1.02);
  transition: transform 0.3s var(--timing-ease);
}

.hover-rotate:hover {
  transform: rotate(5deg);
  transition: transform 0.3s var(--timing-ease);
}

/* ANIMATION DELAYS */
.animate-delay-50 { animation-delay: 50ms; }
.animate-delay-100 { animation-delay: 100ms; }
.animate-delay-150 { animation-delay: 150ms; }
.animate-delay-200 { animation-delay: 200ms; }
.animate-delay-300 { animation-delay: 300ms; }
.animate-delay-500 { animation-delay: 500ms; }

/* STAGGER ANIMATION */
.stagger-children > * {
  animation: slide-up 0.4s var(--timing-ease);
  animation-fill-mode: both;
}

.stagger-children > *:nth-child(1) { animation-delay: 0ms; }
.stagger-children > *:nth-child(2) { animation-delay: 50ms; }
.stagger-children > *:nth-child(3) { animation-delay: 100ms; }
.stagger-children > *:nth-child(4) { animation-delay: 150ms; }
.stagger-children > *:nth-child(5) { animation-delay: 200ms; }
```

**Usage Examples**:
```html
<!-- Fade in on load -->
<div class="animate-fade-in">
  <h1>Welcome to Llama Monitor</h1>
</div>

<!-- Slide up with delay -->
<div class="animate-slide-up animate-delay-100">
  <p>System monitoring active</p>
</div>

<!-- Scale in for modal -->
<div class="modal-content animate-scale-in">
  <!-- Content -->
</div>

<!-- Hover effects -->
<button class="btn btn-primary hover-lift">
  Install & Start
</button>

<!-- Pulse status indicator -->
<span class="status-dot animate-pulse"></span>
```

---

### 2. Hover States

**Files to Modify**: `static/style.css`

```css
/* HOVER STATE VARIABLES */
:root {
  --hover-bg-light: 8%;
  --hover-border-opacity: 0.15;
  --hover-shadow: 4px;
  --hover-scale: 0.98;
}

/* BUTTON HOVER */
.btn:hover {
  transform: translateY(-2px);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
}

.btn:hover::before {
  border-color: rgba(99, 102, 241, var(--hover-border-opacity));
}

/* CARD HOVER */
.card:hover {
  transform: translateY(-4px);
  box-shadow: var(--shadow-elevated);
  border-color: rgba(99, 102, 241, 0.3);
}

/* INPUT HOVER */
.input-field:hover {
  border-color: rgba(255, 255, 255, 0.1);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
}

/* ICON HOVER */
i:hover, .icon:hover {
  color: var(--color-primary);
  transform: scale(1.1);
  transition: all 0.2s ease;
}

/* LINK HOVER */
a:hover {
  color: var(--color-primary);
  text-decoration: underline;
  text-underline-offset: 4px;
}

/* HOVER UTILITY */
.hover-primary:hover {
  color: var(--color-primary);
}

.hover-success:hover {
  color: var(--color-gpu);
}

.hover-error:hover {
  color: var(--color-error);
}

.hover-raise:hover {
  transform: translateY(-4px);
  transition: transform 0.3s var(--timing-ease);
}

.hover-grow:hover {
  transform: scale(1.05);
  transition: transform 0.3s var(--timing-ease);
}
```

---

### 3. Loading States

**Files to Modify**: `static/style.css`

```css
/* PROGRESSIVE LOADING STATES */

/* Instant (0-100ms): Immediate visual feedback */
.loading-indicator {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: var(--color-text-primary);
}

.loading-indicator .spinner {
  width: 20px;
  height: 20px;
  border-width: 2px;
}

/* Brief (100-500ms): Show spinner */
.loading-text {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--color-text-secondary);
}

/* Extended (500ms+): Show skeleton + ETA */
.loading-full {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 20px;
}

.loading-full .skeleton {
  width: 100%;
  margin-bottom: 8px;
}

.loading-full .eta {
  font-size: var(--text-xs);
  color: var(--color-text-muted);
  text-align: center;
}

/* Long (2s+): Show cancellable progress */
.loading-blocking {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.8);
  backdrop-filter: blur(8px);
  z-index: 9999;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 24px;
}

.loading-blocking .progress-bar {
  width: 300px;
  height: 10px;
}

.loading-blocking .cancel-btn {
  margin-top: 16px;
}
```

---

## 📊 Dashboard Layout Enhancements

### 1. Grid System

**Files to Modify**: `static/style.css`

```css
/* GRID VARIABLES */
:root {
  --grid-gap: 24px;
  --grid-gap-sm: 16px;
  --grid-gap-lg: 32px;
}

/* RESPONSIVE GRID CONTAINER */
.grid-container {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: var(--grid-gap);
  padding: 24px;
}

/* GRID BREAKPOINTS */
@media (min-width: 480px) {
  .grid-xs {
    grid-template-columns: repeat(1, 1fr);
  }
}

@media (min-width: 768px) {
  .grid-sm {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (min-width: 1024px) {
  .grid-md {
    grid-template-columns: repeat(3, 1fr);
  }
}

@media (min-width: 1280px) {
  .grid-lg {
    grid-template-columns: repeat(4, 1fr);
  }
}

@media (min-width: 1536px) {
  .grid-xl {
    grid-template-columns: repeat(5, 1fr);
  }
}

/* GRID ITEMS */
.grid-item {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* AUTO-FIT GRID */
.grid-auto-fit {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: var(--grid-gap);
}

/* GRID SPAN */
.span-2 { grid-column: span 2; }
.span-3 { grid-column: span 3; }
.span-4 { grid-column: span 4; }

/* ASPECT RATIO */
.aspect-1\/1 { aspect-ratio: 1/1; }
.aspect-16\/9 { aspect-ratio: 16/9; }
.aspect-4\/3 { aspect-ratio: 4/3; }

/* GAP UTILITIES */
.gap-8 { gap: 8px; }
.gap-12 { gap: 12px; }
.gap-16 { gap: 16px; }
.gap-20 { gap: 20px; }
.gap-24 { gap: 24px; }
.gap-32 { gap: 32px; }
```

---

### 2. Dashboard Widgets

**Files to Modify**: `static/style.css`

```css
/* WIDGET BASE */
.widget {
  background: linear-gradient(145deg, rgba(40, 48, 58, 0.8), rgba(30, 36, 43, 0.9));
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  padding: 20px;
  transition: all 0.3s var(--timing-ease);
}

.widget:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-elevated);
  border-color: rgba(99, 102, 241, 0.2);
}

/* METRIC WIDGET */
.widget-metric {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.widget-metric .metric-value {
  font-size: var(--text-3xl);
  font-weight: var(--font-extrabold);
}

.widget-metric .metric-label {
  font-size: var(--text-xs);
  color: var(--color-text-muted);
}

/* STATUS WIDGET */
.widget-status {
  display: flex;
  align-items: center;
  gap: 12px;
}

.status-indicator {
  width: 12px;
  height: 12px;
  border-radius: 50%;
}

.status-indicator.online {
  background: var(--color-gpu);
  box-shadow: 0 0 0 4px rgba(16, 185, 129, 0.2);
}

.status-indicator.offline {
  background: var(--color-warning);
}

.status-indicator.error {
  background: var(--color-error);
}

.status-indicator.unknown {
  background: var(--color-gray-500);
}

.status-text {
  font-weight: var(--font-medium);
}

/* CHART WIDGET */
.widget-chart {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.chart-container {
  height: 120px;
  position: relative;
}

.chart-title {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--color-text-primary);
}

/* ACTION WIDGET */
.widget-action {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.action-icon {
  width: 48px;
  height: 48px;
  border-radius: 12px;
  background: rgba(99, 102, 241, 0.1);
  color: var(--color-primary);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 28px;
  transition: all 0.2s;
}

.action-icon:hover {
  background: rgba(99, 102, 241, 0.2);
  transform: scale(1.1);
}

/* INFO WIDGET */
.widget-info {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.info-text {
  font-size: var(--text-sm);
  color: var(--color-text-secondary);
  line-height: 1.5;
}
```

---

### 3. Sidebar Navigation

**Files to Modify**: `static/style.css`

```css
/* SIDEBAR BASE */
.sidebar {
  width: 260px;
  height: 100vh;
  background: rgba(15, 17, 21, 0.98);
  backdrop-filter: blur(12px);
  border-right: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  position: fixed;
  left: 0;
  top: 0;
  z-index: 100;
  transition: all 0.3s var(--timing-ease);
}

.sidebar.collapsed {
  width: 80px;
}

/* SIDEBAR HEADER */
.sidebar-header {
  padding: 20px;
  border-bottom: 1px solid var(--border-subtle);
}

.sidebar-logo {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--text-2xl);
  font-weight: var(--font-extrabold);
  color: var(--color-text-primary);
  text-decoration: none;
}

.sidebar-logo i {
  color: var(--color-primary);
}

/* SIDEBAR NAVIGATION */
.sidebar-nav {
  flex: 1;
  padding: 16px 0;
  overflow-y: auto;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  color: var(--color-text-secondary);
  text-decoration: none;
  border-radius: 8px;
  margin: 4px 8px;
  transition: all 0.2s;
  position: relative;
}

.nav-item:hover {
  background: rgba(255, 255, 255, 0.05);
  color: var(--color-text-primary);
}

.nav-item.active {
  background: rgba(99, 102, 241, 0.1);
  color: var(--color-primary);
  border-left: 4px solid var(--color-primary);
}

.nav-item i {
  width: 24px;
  text-align: center;
  font-size: 18px;
}

.nav-item-label {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
}

/* SIDEBAR COLLAPSED STATE */
.sidebar.collapsed .sidebar-logo {
  justify-content: center;
}

.sidebar.collapsed .nav-item-label {
  display: none;
}

.sidebar.collapsed .nav-item {
  justify-content: center;
  padding: 16px;
}

/* SIDEBAR FOOTER */
.sidebar-footer {
  padding: 16px;
  border-top: 1px solid var(--border-subtle);
}

.sidebar-footer .nav-item {
  justify-content: center;
}

/* MOBILE DRAWER */
.sidebar-drawer {
  position: fixed;
  top: 0;
  left: 0;
  height: 100vh;
  width: 280px;
  z-index: 200;
  transform: translateX(-100%);
  transition: transform 0.3s var(--timing-ease);
  box-shadow: var(--shadow-elevated);
}

.sidebar-drawer.open {
  transform: translateX(0);
}
```

---

### 4. Top Navigation

**Files to Modify**: `static/style.css`

```css
/* TOP NAVIGATION */
.top-nav {
  height: 64px;
  background: rgba(30, 36, 43, 0.8);
  backdrop-filter: blur(12px);
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 24px;
  position: fixed;
  top: 0;
  right: 0;
  left: 260px;
  z-index: 90;
  transition: all 0.3s var(--timing-ease);
}

.top-nav.collapsed {
  left: 80px;
}

/* NAV SEARCH */
.nav-search {
  flex: 1;
  max-width: 400px;
  margin-right: 24px;
}

.nav-search input {
  width: 100%;
  background: rgba(0, 0, 0, 0.3);
  border: 1px solid var(--border-subtle);
  color: var(--color-text-primary);
  padding: 8px 16px;
  border-radius: 20px;
  font-size: var(--text-sm);
}

.nav-search input:focus {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.1);
}

/* NAV ACTIONS */
.nav-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.nav-action-btn {
  width: 40px;
  height: 40px;
  border-radius: 8px;
  background: transparent;
  border: 1px solid var(--border-subtle);
  color: var(--color-text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
}

.nav-action-btn:hover {
  background: rgba(255, 255, 255, 0.05);
  color: var(--color-text-primary);
  border-color: rgba(255, 255, 255, 0.1);
}

.nav-action-btn .badge {
  position: absolute;
  top: 8px;
  right: 8px;
  width: 8px;
  height: 8px;
  background: var(--color-error);
  border-radius: 50%;
  border: 2px solid rgba(30, 36, 43, 0.9);
}

/* USER PROFILE */
.user-profile {
  display: flex;
  align-items: center;
  gap: 12px;
  cursor: pointer;
  padding: 4px;
  border-radius: 8px;
  transition: background 0.2s;
}

.user-profile:hover {
  background: rgba(255, 255, 255, 0.05);
}

.user-avatar {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background: var(--gradient-primary);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-on-primary);
  font-weight: var(--font-semibold);
}

.user-info {
  display: flex;
  flex-direction: column;
}

.user-name {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--color-text-primary);
}

.user-role {
  font-size: var(--text-xs);
  color: var(--color-text-muted);
}

/* FLOATING ACTION BUTTON */
.fab {
  position: fixed;
  bottom: 32px;
  right: 32px;
  width: 56px;
  height: 56px;
  border-radius: 50%;
  background: var(--gradient-primary);
  border: none;
  box-shadow: 0 8px 24px rgba(99, 102, 241, 0.4);
  color: var(--color-text-on-primary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 28px;
  transition: all 0.3s var(--timing-ease);
  z-index: 1000;
}

.fab:hover {
  transform: scale(1.1);
  box-shadow: 0 12px 32px rgba(99, 102, 241, 0.5);
}

.fab:active {
  transform: scale(0.95);
}
```

---

## 🎯 User Experience Improvements

### 1. Smart Defaults

**Files to Modify**: `static/app.js`

```javascript
// Add to app.js
function smartDetectDefaults() {
  // Detect GPU count
  const gpuCount = detectGPUCount();
  document.getElementById('gpu-select').value = Math.min(gpuCount, 4);
  
  // Detect available memory
  const availableMemory = detectAvailableMemory();
  const maxContext = Math.min(Math.floor(availableMemory / 2) * 1024, 8192);
  document.getElementById('context-slider').value = Math.floor(maxContext / 2);
  
  // Set temperature default
  document.getElementById('temp-input').value = 0.7;
  
  // Set top_p default
  document.getElementById('top-p-input').value = 0.9;
  
  // Set top_k default
  document.getElementById('top-k-input').value = 40;
}

function detectGPUCount() {
  // Implementation to detect available GPUs
  return 1;
}

function detectAvailableMemory() {
  // Implementation to detect available GPU memory
  return 24;
}
```

---

### 2. Contextual Help

**Files to Modify**: `static/style.css`

```css
/* CONTEXTUAL HELP */
.help-icon {
  color: var(--color-text-muted);
  cursor: help;
  transition: all 0.2s;
}

.help-icon:hover {
  color: var(--color-primary);
}

.help-tooltip {
  position: absolute;
  bottom: 100%;
  left: 50%;
  transform: translateX(-50%);
  background: var(--color-bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  padding: 12px;
  width: 250px;
  box-shadow: var(--shadow-elevated);
  font-size: var(--text-sm);
  color: var(--color-text-secondary);
  z-index: 100;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.2s;
}

.help-icon:hover + .help-tooltip {
  opacity: 1;
  pointer-events: auto;
}

/* IN-APP TOUR */
.tour-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.6);
  z-index: 5000;
  display: flex;
  align-items: center;
  justify-content: center;
}

.tour-card {
  background: var(--color-bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: 24px;
  padding: 32px;
  max-width: 400px;
  text-align: center;
  box-shadow: var(--shadow-elevated);
}

.tour-step {
  margin: 24px 0;
}

.tour-step-number {
  font-size: var(--text-4xl);
  font-weight: var(--font-extrabold);
  color: var(--color-primary);
  margin-bottom: 16px;
}

.tour-step-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-semibold);
  color: var(--color-text-primary);
  margin-bottom: 8px;
}

.tour-step-desc {
  font-size: var(--text-base);
  color: var(--color-text-secondary);
  line-height: 1.6;
}
```

---

### 3. Keyboard Accessibility

**Files to Modify**: `static/app.js`

```javascript
// Global keyboard shortcuts
function setupKeyboardShortcuts() {
  // Global search (Cmd/Ctrl + K)
  document.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault();
      document.querySelector('.nav-search input').focus();
    }
    
    // Quick actions (Cmd/Ctrl + 1-9)
    if ((e.metaKey || e.ctrlKey) && /^[1-9]$/.test(e.key)) {
      e.preventDefault();
      const index = parseInt(e.key) - 1;
      handleQuickAction(index);
    }
    
    // Close dialogs (Esc)
    if (e.key === 'Escape') {
      closeActiveDialog();
    }
  });
}

function handleQuickAction(index) {
  const actions = [
    () => document.querySelector('.btn-primary').click(),
    () => openSettings(),
    () => openHelp(),
    () => createNewSession(),
    // Add more quick actions...
  ];
  
  if (index < actions.length) {
    actions[index]();
  }
}
```

---

## 🚀 Advanced Features

### 1. Customization

**Files to Modify**: `static/style.css`

```css
/* THEME PRESETS */
:root[data-theme="dark"] {
  --bg-primary: #0f1115;
  --bg-surface: #16191e;
  --bg-elevated: #1f232a;
  --primary-gradient: linear-gradient(160deg, #6366f1 0%, #8b5cf6 50%, #06b6d4 100%);
}

:root[data-theme="midnight"] {
  --bg-primary: #0a0e14;
  --bg-surface: #131821;
  --bg-elevated: #1b212c;
  --primary-gradient: linear-gradient(160deg, #2563eb 0%, #3b82f6 50%, #0ea5e9 100%);
}

:root[data-theme="light"] {
  --bg-primary: #f8fafc;
  --bg-surface: #ffffff;
  --bg-elevated: #ffffff;
  --primary-gradient: linear-gradient(160deg, #6366f1 0%, #8b5cf6 50%, #06b6d4 100%);
}

:root[data-theme="cyberpunk"] {
  --bg-primary: #0d0d0d;
  --bg-surface: #1a1a1a;
  --bg-elevated: #242424;
  --primary-gradient: linear-gradient(160deg, #00f3ff 0%, #ff00ff 50%, #bc13fe 100%);
  --accent-color: #00f3ff;
}

/* LAYOUT PRESETS */
:root[data-layout="compact"] {
  --grid-gap: 16px;
  --btn-padding: 8px 16px;
  --input-height: 36px;
}

:root[data-layout="balanced"] {
  --grid-gap: 24px;
  --btn-padding: 12px 20px;
  --input-height: 40px;
}

:root[data-layout="spacious"] {
  --grid-gap: 32px;
  --btn-padding: 16px 24px;
  --input-height: 48px;
}
```

---

### 2. Animations & Micro-Interactions

**Files to Modify**: `static/style.css`

```css
/* BUTTON RIPPLE */
.ripple-effect {
  position: relative;
  overflow: hidden;
}

.ripple-effect::after {
  content: '';
  position: absolute;
  top: 50%;
  left: 50%;
  width: 0;
  height: 0;
  background: rgba(255, 255, 255, 0.3);
  border-radius: 50%;
  transform: translate(-50%, -50%);
  transition: width 0.6s, height 0.6s;
}

.ripple-effect:active::after {
  width: 200%;
  height: 200%;
}

/* ICON TRANSITIONS */
.icon-morph {
  transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
}

.icon-morph:hover {
  transform: rotate(180deg) scale(1.1);
}

/* STATUS ANIMATIONS */
@keyframes status-pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.7; transform: scale(0.9); }
}

.status-dot.pulse {
  animation: status-pulse 2s cubic-bezier(0.4, 0, 0.2, 1) infinite;
}

@keyframes status-bounce {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-6px); }
}

.status-dot.bounce {
  animation: status-bounce 0.6s cubic-bezier(0.68, -0.55, 0.27, 1.55) 2;
}

/* LOADING SKELETON */
@keyframes skeleton-shimmer {
  0% { background-position: 200% 0; }
  100% { background-position: -200% 0; }
}

.skeleton {
  background: linear-gradient(90deg, rgba(255,255,255,0.05) 25%, rgba(255,255,255,0.15) 50%, rgba(255,255,255,0.05) 75%);
  background-size: 200% 100%;
  animation: skeleton-shimmer 1.5s linear infinite;
}

/* CARD FLIP */
.card-flip-container {
  perspective: 1000px;
}

.card-flip {
  transition: transform 0.6s cubic-bezier(0.4, 0, 0.2, 1);
  transform-style: preserve-3d;
}

.card-flip.flipped {
  transform: rotateY(180deg);
}

.card-front, .card-back {
  backface-visibility: hidden;
  transform: rotateY(0deg);
}

.card-back {
  transform: rotateY(180deg);
}
```

---

### 3. Data Visualization

**Files to Modify**: `static/app.js` (add chart functionality)

```javascript
// Simple chart rendering
function renderChart(canvasId, data, options = {}) {
  const canvas = document.getElementById(canvasId);
  const ctx = canvas.getContext('2d');
  const width = canvas.width;
  const height = canvas.height;
  
  // Clear canvas
  ctx.clearRect(0, 0, width, height);
  
  // Draw line
  ctx.strokeStyle = getComputedStyle(document.documentElement).getPropertyValue('--color-primary');
  ctx.lineWidth = 2;
  ctx.beginPath();
  
  const maxData = Math.max(...data);
  const minData = Math.min(...data);
  
  data.forEach((value, index) => {
    const x = (index / (data.length - 1)) * width;
    const y = height - ((value - minData) / (maxData - minData)) * (height - 40) - 20;
    
    if (index === 0) {
      ctx.moveTo(x, y);
    } else {
      ctx.lineTo(x, y);
    }
  });
  
  ctx.stroke();
  
  // Draw gradient fill
  const gradient = ctx.createLinearGradient(0, 0, 0, height);
  gradient.addColorStop(0, getComputedStyle(document.documentElement).getPropertyValue('--color-primary').replace(')', ', 0.2)').replace('rgb', 'rgba'));
  gradient.addColorStop(1, 'rgba(0, 0, 0, 0)');
  
  ctx.fillStyle = gradient;
  ctx.lineTo(width, height);
  ctx.lineTo(0, height);
  ctx.fill();
}

// Sparkline component
function renderSparkline(elementId, data) {
  const canvas = document.getElementById(elementId);
  if (!canvas) return;
  
  const ctx = canvas.getContext('2d');
  const width = canvas.width;
  const height = canvas.height;
  
  ctx.clearRect(0, 0, width, height);
  
  // Draw sparkline
  ctx.strokeStyle = getComputedStyle(document.documentElement).getPropertyValue('--color-gpu');
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  
  const maxData = Math.max(...data);
  const minData = Math.min(...data);
  
  data.forEach((value, index) => {
    const x = (index / (data.length - 1)) * width;
    const y = height - ((value - minData) / (maxData - minData)) * (height - 4) - 2;
    
    if (index === 0) {
      ctx.moveTo(x, y);
    } else {
      ctx.lineTo(x, y);
    }
  });
  
  ctx.stroke();
  
  // Draw points
  ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--color-bg');
  data.forEach((value, index) => {
    const x = (index / (data.length - 1)) * width;
    const y = height - ((value - minData) / (maxData - minData)) * (height - 4) - 2;
    
    ctx.beginPath();
    ctx.arc(x, y, 2, 0, Math.PI * 2);
    ctx.fill();
  });
  
  // Draw current value dot
  if (data.length > 0) {
    const lastX = width;
    const lastY = height - ((data[data.length - 1] - minData) / (maxData - minData)) * (height - 4) - 2;
    
    ctx.beginPath();
    ctx.arc(lastX, lastY, 4, 0, Math.PI * 2);
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--color-primary');
    ctx.fill();
  }
}
```

---

## 📦 Implementation Guide

### File Structure

```
llama-monitor/
├── static/
│   ├── style.css          # Main stylesheet (MODIFY)
│   ├── index.html         # HTML structure (MODIFY)
│   └── app.js             # JavaScript logic (MODIFY)
├── src/
│   ├── web/api.rs         # API endpoints
│   └── agent.rs           # Backend agent
└── docs/
    ├── 20260420-modern_ui_enhancements.md
    └── 20260420-modern_ui_wireframes.md
```

### Implementation Steps

1. **Phase 1 - CSS Variables** (30 minutes)
   - Add color system to `:root`
   - Add spacing and sizing variables
   - Add animation timing variables

2. **Phase 2 - Base Components** (2 hours)
    - Add panel and card styles
    - Add button variants
    - Add input field styles
    - Add modal styles with typography

3. **Phase 3 - Layout System** (1 hour)
   - Add grid system
   - Add responsive utilities
   - Add navigation styles

4. **Phase 4 - Interactive Elements** (2 hours)
   - Add animation system
   - Add hover states
   - Add loading states

5. **Phase 5 - JavaScript Integration** (2 hours)
   - Add animation helpers
   - Add keyboard shortcuts
   - Add toast notifications

6. **Phase 6 - Testing** (1 hour)
   - Test all components
   - Test responsive design
   - Test accessibility

---

## 🧪 Testing Checklist

### Visual Testing
- [ ] All CSS variables are defined
- [ ] Panels show glassmorphism effect
- [ ] Cards lift on hover
- [ ] Gradients render correctly
- [ ] Shadows match design specs
- [ ] Border radius consistent

### Component Testing
- [ ] All button variants work
- [ ] Input fields show validation states
- [ ] Modals open/close smoothly
- [ ] Toasts appear/disappear correctly
- [ ] Progress bars animate

### Responsive Testing
- [ ] Grid adjusts at 768px breakpoint
- [ ] Sidebar collapses on mobile
- [ ] Touch targets are 48px minimum
- [ ] Text doesn't overflow on small screens

### Accessibility Testing
- [ ] Keyboard navigation works
- [ ] ARIA labels present
- [ ] Focus indicators visible
- [ ] Color contrast meets WCAG AA
- [ ] Screen reader compatible

### Performance Testing
- [ ] Animations run at 60fps
- [ ] No layout shifts on load
- [ ] Images lazy load
- [ ] Critical CSS inlined

---

## 📚 Additional Resources

### CSS Frameworks
- [Tailwind CSS](https://tailwindcss.com/) - Utility-first framework
- [UnoCSS](https://unocss.dev/) - Atomic CSS engine

### Libraries
- [Framer Motion](https://www.framer.com/motion/) - Animations
- [Interact.js](https://interactjs.io/) - Drag and drop
- [Heroicons](https://heroicons.com/) - Icon library

### Design Systems
- [Material Design 3](https://m3.material.io/) - Design guidelines
- [Interact](https://interactjs.io/) - UI interactions

---

## 🎓 Learning Resources

For AI agents implementing these changes:
1. Read CSS Custom Properties spec
2. Understand CSS Grid and Flexbox
3. Learn CSS animations and transitions
4. Study glassmorphism techniques
5. Practice progressive enhancement

---

## 📝 Version History

- **v1.0** (2026-04-20): Initial release
- **v2.0** (2026-04-20): AI-Agent Ready update with implementation details
- **v3.0** (2026-04-20): Full Implementation Guide
- **v3.1** (2026-04-20): Phase 1 implementation begun (CSS variables, colors, panels, cards)
- **v3.2.1** (2026-04-20): Phase 3 complete (Toast CSS, HTML, JavaScript)

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
