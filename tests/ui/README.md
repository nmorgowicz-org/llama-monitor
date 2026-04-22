# UI Test Documentation

## Setup

```bash
cd tests/ui
npm install
```

## Run Tests

```bash
# Run all tests
npm test

# Run specific test file
npm test capability-rendering.spec.js

# Run with browser visible
npm test -- --headless=false

# Debug mode
npm test -- --debug
```

## Test Coverage

- ✅ Top navigation bar rendering
- ✅ Sidebar navigation rendering  
- ✅ Dashboard grid rendering
- ✅ Inference metrics rendering
- ✅ GPU table rendering
- ✅ Chat and Logs tabs
- ✅ Session modal structure
- ✅ Settings modal with tabs
- ✅ Analytics modal structure
- ✅ Export modal structure
- ✅ Keyboard shortcuts modal
- ✅ User preferences modal
- ✅ Theme system (light/dark)
- ✅ Responsive design (desktop/laptop/mobile)
- ✅ Remote agent mode
- ✅ Error states
- ✅ Keyboard shortcuts
- ✅ Accessibility features

## Manual Testing

```bash
# Start server with headless mode
cargo run -- --headless --port 9999

# Open browser
open http://localhost:9999

# Test each UI component
# 1. Navigate through all tabs
# 2. Test Start/Stop server
# 3. Test Settings modal
# 4. Test Analytics modal
# 5. Test Export modal
# 6. Test keyboard shortcuts (Ctrl+? or Shift+/)
# 7. Test theme toggle
```

---

**Last Updated**: 2026-04-20
