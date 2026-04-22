# Visual QA Checklist - Phase 5 UI Implementation

**Date**: 2026-04-20  
**Version**: Phase 5  
**Status**: In Progress

---

## ✅ Testing Checklist

### Local Endpoint (Full Metrics)
- [ ] Server tab displays all metrics (CPU, RAM, GPU, Temp, Fan, Clock)
- [ ] Inference metrics show real-time data
- [ ] GPU table shows all detected GPUs
- [ ] Start/Stop buttons functional
- [ ] Preset dropdown populated
- [ ] No console errors

### Remote Endpoint (Inference Only)
- [ ] GPU section hidden when attached to remote
- [ ] System metrics hidden when attached to remote
- [ ] Only inference metrics displayed
- [ ] Clear "Remote Agent" indicator visible
- [ ] No GPU temperature display

### Server States
- [ ] Idle server shows "Online" with no inference data
- [ ] Server unreachable shows error state
- [ ] High context usage shows warning
- [ ] Missing GPU backend shows fallback message

### UI Components
- [ ] Top navigation bar (search, settings, user profile)
- [ ] Sidebar navigation (Server, Chat, Logs, Sessions, Models, Settings)
- [ ] Session cards display correctly
- [ ] Settings modal tabs functional
- [ ] Analytics dashboard renders
- [ ] Export modal options visible
- [ ] Keyboard shortcuts modal displays
- [ ] User preferences modal functional

### Responsive Design
- [ ] Desktop (1920px+): 12-column grid works
- [ ] Laptop (1024px-1279px): Sidebar visible
- [ ] Tablet (768px-1023px): Layout adjusts
- [ ] Mobile (< 768px): Single column stacking

### Theme System
- [ ] Light mode renders correctly
- [ ] Dark mode renders correctly
- [ ] Theme switcher works
- [ ] Auto mode detects system preference
- [ ] No text contrast issues

### Advanced Features
- [ ] Sessions panel shows expandable cards
- [ ] Settings tabs switch content
- [ ] Analytics charts render
- [ ] Export format options work
- [ ] Keyboard shortcuts displayed
- [ ] User preferences save correctly

---

## 📋 Known Limitations

- Analytics dashboard requires JavaScript for chart rendering
- Browser-based tests require Chrome/Edge
- Visual QA should be run on actual hardware for GPU metrics

---

## 🚀 Next Steps

1. Run `cargo run -- --headless --port 9999` for remote testing
2. Open browser at `http://localhost:9999`
3. Test each checklist item
4. Report any issues found

---

**Last Updated**: 2026-04-20
