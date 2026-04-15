# Lattice UI - Current State (2026-04-14 19:50 CDT)

## ✅ PRODUCTION BUILD - LOCKED

**Commit:** `fff0e15` - Make 0.15 the center of slider range  
**Status:** Stable, working, DO NOT BREAK  
**URL:** http://localhost:5174

---

## What's Built

### Core Visualization
- **Engine:** THREE.js + React
- **Data:** 3570 shards (GN compression data)
- **Layout:** Fibonacci sphere (radius 130)
- **Rendering:** EffectComposer with UnrealBloomPass

### Visual Features
- ✅ Crystal geometry (VTC-based)
- ✅ Bloom effect
- ✅ Orbit controls (scroll/drag)
- ✅ Click-to-select with metadata panel
- ✅ Rotation toggle (enable/disable)
- ✅ Size slider (0.5x - 2.0x, centered at 1.0 = 0.15 base scale)

### Menu System
- ✅ ? - Show/hide menu
- ✅ F - Filters (shard type)
- ✅ M - Metrics
- ✅ Playhead scrubber (temporal navigation)
- ✅ Size slider (bottom-left)
- ✅ Selection info panel

### Data
- **Graph:** `/lattice.json` (3570 shards with pairs, next pointers, types)
- **Compression:** Glasik GN notation (token/literal pairs)
- **Flow:** Arrows connecting shards (temporal/causal)

---

## Current Sizing

**Default Scale:** 0.15 (75% reduction from original 0.6)

Formula: `mesh.scale.setScalar((scale * crystalSize * 0.15))`
- `scale`: Log-based per shard (0.9 + Math.log2(count+1) * 0.6)
- `crystalSize`: Slider value (0.5 = 50%, 1.0 = 100%, 2.0 = 200%)
- Base multiplier: **0.15** (this is the center point)

**Result at 100% slider:**
- Shards are very small but individually visible
- Entire lattice fits on screen
- Data flow patterns readable

---

## Key Files

| File | Purpose | Status |
|------|---------|--------|
| `src/App.jsx` | Main component (GOLD) | ✅ Locked |
| `src/main.jsx` | Entry point | ✅ Locked |
| `public/lattice.json` | Shard data (3570) | ✅ Current |
| `index.html` | HTML shell | ✅ Locked |
| `vite.config.js` | Build config | ✅ Locked |

---

## Recent Changes (This Session)

1. ✅ Restored original App.jsx from commit d0ab429
2. ✅ Reduced shard size 70% (0.6 → 0.3)
3. ✅ Reduced further 25% (0.3 → 0.225)
4. ✅ Reduced further 60% (0.225 → 0.084)
5. ✅ Settled on 0.15 as new default (cleaner formula)
6. ✅ Made 0.15 the center of slider range
7. ❌ Removed AppWithTabs wrapper (was breaking things)
8. ❌ Removed Tab 2 / AppLattice (complexity not needed)

---

## What NOT to Do

- ❌ Don't modify App.jsx visually (menu, controls work)
- ❌ Don't change the slider range (0.5-2.0 is correct)
- ❌ Don't touch the crystal geometry calculation
- ❌ Don't add tabs/wrappers without testing
- ❌ Don't change the bloom effect settings
- ❌ Don't swap out the lattice.json (3570 shards is correct)

---

## Next Steps (If Needed)

1. **Add search/filter:** Implement shard search by type/content
2. **Export:** Screenshot or JSON export of visible shards
3. **Performance:** If slow with 3570, optimize renderer or reduce sphere radius
4. **Metadata:** Show compression stats (original/compressed size, ratio) on hover
5. **Connections:** Wire up edge flow visualization properly

---

## Testing Checklist

- [ ] Load on fresh browser (F5)
- [ ] Rotate shards (enable rotation toggle)
- [ ] Adjust size slider (0.5x → 2.0x)
- [ ] Click shard (shows metadata)
- [ ] Toggle menu (?)
- [ ] Check bloom effect (shards should glow)
- [ ] Orbit controls (scroll to zoom, drag to rotate)

---

## Git State

```
Branch: master
Last commit: fff0e15 - "fix: make 0.15 (current size) the center of slider range"
Remote: github.com:atomsrkuul/glasik-core.git
Status: Clean, all changes pushed
```

---

## Critical Context

- **User:** Robert
- **Project:** OpenClaw control UI with Glasik lattice visualization
- **Philosophy:** Direct work, no hanging. Keep it working.
- **Constraint:** Don't break the visualization. Test thoroughly.

---

**Built with:** React + THREE.js + Vite  
**Deployed to:** localhost:5174  
**Data source:** GN compression shards + VTC identity  
**Last updated:** 2026-04-14 19:50 CDT
