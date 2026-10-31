# Nuclear interface: themes, toolbar density, tabs and recommended tools

This page records how the "Interface Refinement, Themes, and Adaptive Toolbar" specification
(2026-10-29) is implemented, the tunable defaults it chose, and what is still open. Screenshots
in the pull request are real captures of the app.

## Themes (Preferences › Interface › Appearance)

| Preset | File | Notes |
|---|---|---|
| Nuclear Graphite (default) | `assets/styles/ui/themes/nuclear-graphite.json` | Window `#191C20`, panels `#23272D`, raised `#2D333B`, hover `#39424D`, text `#F1F3F5` / `#B8C0CB`, accent `#FF7A32`, borders `#414A56` |
| Nuclear Carbon | `nuclear-carbon.json` | Near-black surfaces, amber-orange accent `#FF8F3A` |
| Nuclear Light | `nuclear-light.json` | Warm light grey surfaces, dark text, burnt-orange accent `#B8480F` |
| Follow System | the `system` mode | Nuclear Graphite on a dark system, Nuclear Light on a light one (not the OS accent) |

- The presets are ordinary theme files over the shared `theme::Tokens`; no widget has a new
  literal colour. The accent replaces blue in window chrome and in the command icons' "solid"
  ramp (light neutral geometry, orange on the affected part).
- Model and sketch colours are left out of the files on purpose, so selection, hover, previews,
  sketch states, constraints, axes, analyses and the per-tab accents keep the built-in Dark and
  Light meanings. Warning and error colours are unchanged.
- The classic Dark and Light themes and every other theme stay available.
- Migration: saved preferences from before (`nuclear_appearance` 0) that used `dark` or `light`
  move once to Nuclear Graphite or Nuclear Light; any later choice stays. Imported and other
  built-in themes are untouched.
- **Restore appearance defaults** resets the theme, the 3D view's colours and background, the
  logo colours and the toolbar density and labels only. Projects, pins, shortcuts and other
  preferences stay.

## 3D view background

`viewport_background`: From theme (default), Studio gradient, Graphite, Black, Light grey,
Custom colour (`viewport_custom`, `#rrggbb`, applied as soon as it reads, with "Restore default
colour"). The view's dark or light side (edges, selection, sketch colours, triad) follows the
background's brightness, so a dark window can show a light view and the reverse; the grid is
derived from the background. Only tokens change: no geometry is rebuilt or retessellated, and
materials are untouched.

## Toolbar density

`toolbar_density`: Comfortable (default) or Compact (the previous ribbon).

| | Comfortable | Compact |
|---|---|---|
| Command artwork | 30 pt | 26 pt |
| Labelled button height | 66 pt | 60 pt |
| Icon-only target | 46 × 46 pt | 40 × 36 pt |
| Labelled ribbon height | 123 pt | 117 pt |

Button widths are unchanged, so translated labels wrap and fit exactly as before (the existing
every-language label test covers both densities' label position). Labels on/off stays its own
setting. Tooltips already show the command name, its shortcut and one explanatory sentence.

## Document tabs

- The application bar is 44 pt; tabs are 38 pt tall with 14 pt names, a 2.5 pt accent line under
  the active tab on its lifted surface, a filled dot for unsaved changes (the × replaces it on
  hover) and a 24 pt close target.
- Tabs never shrink below 120 pt; past that the strip keeps the active tab in view and an
  **Open documents** button (count, accented when some tabs are hidden) lists every design with
  where it is saved. It is shown whenever more than one design is open.
- Designs with the same name are told apart by their folder, or by a number.
- Switching, closing, reordering and the save prompts are unchanged.

## Recommended tools

Code: `crates/ui-egui/src/recommend.rs`. A "Recommended" panel after the tab's panels (before the
Sketch exit group) with up to three of the tab's own commands.

- It only takes room the other panels leave: pinned buttons never narrow or move for it, and it
  goes to the » menu when there is no room. A command already on the toolbar is never recommended.
- Counting: one count per deliberate start of a tool or dialog (button, menu, shortcut, command
  search), keyed by tab. Starts within 2 s of a control-channel request (scripts, agents, MCP)
  are not counted; hovers, redraws, previews and value changes are not starts.
- Stored: `tool_usage` (`<tab>/<command id>` → uses, score, launch), `usage_sessions`,
  `adaptive_shown`, `adaptive_hidden`. No geometry, file names, values or design contents, and
  nothing leaves the computer.
- Controls: Preferences › Interface › Recommended tools (on/off, Freeze, Reset learned usage);
  right-click a recommended tool for Pin to Toolbar (the existing pin), Hide Recommendation,
  Freeze Recommendations and Turn Off Recommendations. Off hides the area; pins are unaffected.

Tunable defaults (constants in `recommend.rs`):

| Constant | Default | Meaning |
|---|---|---|
| `SLOTS` | 3 | Most recommendations per tab |
| `MIN_SESSIONS` | 2 | Launches observed before any recommendation |
| `MIN_TAB_USES` | 15 | Uses in a tab before it gets recommendations |
| `MIN_COMMAND_USES` | 3 | Uses a command needs to be recommended |
| `SESSION_CAP` | 4 | Most one launch adds to a score (a burst cannot dominate) |
| `DECAY` | 0.8 | Share of a score kept per launch (recency) |
| `REPLACE_MARGIN` | 1.25 | A newcomer must beat the weakest shown one by 25 % plus one use |
| `SCRIPTED_QUIET` | 2 s | Window after a control request in which starts are not counted |

Recommendations change only at an idle boundary (a launch, or entering a tab) when no tool,
dialog, pointer press or drag is active, and a replacement keeps the other tools in place.

## Still open

- Icon artwork: the command icons are larger and take the orange accent, but their silhouettes
  were not redrawn. New masters go through the icon review in `assets/icons/ui/AGENTS.md` and
  need the owner's acceptance there.
- Keyboard access to the Open documents list uses egui's focus order (Tab, Enter); there is no
  dedicated shortcut for it yet.
- High-DPI and Windows-specific rendering were checked only through the interface scale on Linux.
