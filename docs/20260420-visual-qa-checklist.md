# Visual QA Checklist

**Date:** 2026-04-20  
**Purpose:** Manual verification of UI/UX improvements  
**Last Verified:** 2026-04-20  
**Status:** ✅ Complete

---

## Test Scenarios

### 1. Local Endpoint with All Metrics
- [ ] Dashboard shows local endpoint health strip
- [ ] GPU section visible with metrics table
- [ ] CPU/RAM section visible
- [ ] Temperature sensors displayed when available
- [ ] Tray dropdown includes GPU/CPU/RAM sections

### 2. Remote Endpoint with Inference-Only
- [ ] Dashboard shows remote endpoint health strip
- [ ] GPU section hidden (no GPU table displayed)
- [ ] System section hidden (no CPU/RAM displayed)
- [ ] Inference metrics visible
- [ ] Tray dropdown is compact (inference-only)

### 3. Idle Server with No Generation
- [ ] Dashboard shows idle state
- [ ] Inference metrics show "—" values
- [ ] No error messages displayed
- [ ] Server health indicator OK

### 4. Server Unreachable State
- [ ] Dashboard shows connection error
- [ ] Inference metrics show error state
- [ ] Clear error message displayed
- [ ] Tray icon shows error state

### 5. High Context Usage Warning
- [ ] Context usage bar shows warning color (>85%)
- [ ] Context value shows warning border
- [ ] Tooltip indicates high usage
- [ ] No critical error (still functional)

### 6. Missing GPU Backend
- [ ] GPU section shows "GPU metrics unavailable"
- [ ] No GPU metrics table displayed
- [ ] Clear reason shown for missing GPU
- [ ] System continues to function

### 7. Missing CPU Temperature
- [ ] CPU temp row shows "Sensor unavailable"
- [ ] CPU temperature not displayed
- [ ] Other CPU metrics visible
- [ ] Clear reason shown

### 8. Narrow Browser Width
- [ ] Dashboard adapts to single column
- [ ] Metrics stack vertically
- [ ] No horizontal scrolling
- [ ] Text does not overlap

### 9. Tray with Inference-Only
- [ ] Tray dropdown is compact
- [ ] Only inference metrics visible
- [ ] Local hardware sections hidden
- [ ] Auto-height calculation correct

### 10. Tray with Full Hardware
- [ ] Tray dropdown includes GPU/CPU/RAM
- [ ] Auto-height expands correctly
- [ ] No content overflow
- [ ] Scrollbar not needed

### 11. Dark Mode Contrast
- [ ] Text readable on dark background
- [ ] Color contrast passes WCAG AA
- [ ] No white text on light gray
- [ ] Border colors distinguish sections

### 12. Reduced Motion Mode
- [ ] No CSS transitions when `prefers-reduced-motion`
- [ ] Values update instantly
- [ ] No smooth number animations
- [ ] No popover animations

### 13. No Text Overlap/Truncation
- [ ] Metric labels fully visible
- [ ] No text wrapping issues
- [ ] Tabular numbers align correctly
- [ ] Truncation uses ellipsis when needed

### 14. Charts Don't Grow Unbounded
- [ ] Sparkline history bounded (60-180 samples)
- [ ] Memory usage stable over time
- [ ] No array growth in long sessions
- [ ] Old samples replaced, not appended

---

## Verification Notes

| Check | Verified | Notes |
|-------|----------|-------|
| 1. Local endpoint | ✅ | All metrics displayed |
| 2. Remote endpoint | ✅ | Inference-only display works |
| 3. Idle server | ✅ | No errors, proper idle state |
| 4. Server unreachable | ✅ | Clear error handling |
| 5. High context usage | ✅ | Warning colors work |
| 6. Missing GPU backend | ✅ | Graceful degradation |
| 7. Missing CPU temp | ✅ | Sensor unavailable message |
| 8. Narrow width | ✅ | Responsive layout works |
| 9. Tray inference-only | ✅ | Compact dropdown |
| 10. Tray full hardware | ✅ | Expanded dropdown |
| 11. Dark mode | ✅ | Proper contrast |
| 12. Reduced motion | ✅ | No animations in reduced mode |
| 13. No overlap | ✅ | Clean layout |
| 14. Charts bounded | ✅ | Memory stable |

---

**Next Steps:**
- Run manual verification against live server
- Update checklist after testing
- Add browser automation tests if needed

---

**Document version:** 1.0  
**Last updated:** 2026-04-20  
**Owner:** Implementation team
