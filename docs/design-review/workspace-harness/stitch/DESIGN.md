---
name: Atlas Engineering Core
colors:
  surface: '#111418'
  surface-dim: '#111418'
  surface-bright: '#36393e'
  surface-container-lowest: '#0b0e12'
  surface-container-low: '#191c20'
  surface-container: '#1d2024'
  surface-container-high: '#272a2e'
  surface-container-highest: '#323539'
  on-surface: '#e1e2e8'
  on-surface-variant: '#c7c4d7'
  inverse-surface: '#e1e2e8'
  inverse-on-surface: '#2e3135'
  outline: '#908fa0'
  outline-variant: '#464554'
  surface-tint: '#c0c1ff'
  primary: '#c0c1ff'
  on-primary: '#1000a9'
  primary-container: '#8083ff'
  on-primary-container: '#0d0096'
  inverse-primary: '#494bd6'
  secondary: '#7bd0ff'
  on-secondary: '#00354a'
  secondary-container: '#00a6e0'
  on-secondary-container: '#00374d'
  tertiary: '#ffb0cd'
  on-tertiary: '#640039'
  tertiary-container: '#f751a1'
  on-tertiary-container: '#570032'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#e1e0ff'
  primary-fixed-dim: '#c0c1ff'
  on-primary-fixed: '#07006c'
  on-primary-fixed-variant: '#2f2ebe'
  secondary-fixed: '#c4e7ff'
  secondary-fixed-dim: '#7bd0ff'
  on-secondary-fixed: '#001e2c'
  on-secondary-fixed-variant: '#004c69'
  tertiary-fixed: '#ffd9e4'
  tertiary-fixed-dim: '#ffb0cd'
  on-tertiary-fixed: '#3e0022'
  on-tertiary-fixed-variant: '#8c0053'
  background: '#111418'
  on-background: '#e1e2e8'
  surface-variant: '#323539'
typography:
  display:
    fontFamily: Geist
    fontSize: 1.75rem
    fontWeight: '600'
    lineHeight: 2.25rem
    letterSpacing: -0.025em
  headline-lg:
    fontFamily: Geist
    fontSize: 1.375rem
    fontWeight: '600'
    lineHeight: 1.75rem
    letterSpacing: -0.02em
  headline-sm:
    fontFamily: Geist
    fontSize: 1.125rem
    fontWeight: '500'
    lineHeight: 1.5rem
    letterSpacing: -0.015em
  body-lg:
    fontFamily: Geist
    fontSize: 0.9375rem
    fontWeight: '400'
    lineHeight: 1.375rem
    letterSpacing: -0.005em
  body-md:
    fontFamily: Geist
    fontSize: 0.8125rem
    fontWeight: '400'
    lineHeight: 1.25rem
    letterSpacing: 0em
  body-sm:
    fontFamily: Geist
    fontSize: 0.75rem
    fontWeight: '400'
    lineHeight: 1.125rem
    letterSpacing: 0.005em
  code-lg:
    fontFamily: JetBrains Mono
    fontSize: 0.875rem
    fontWeight: '400'
    lineHeight: 1.375rem
    letterSpacing: -0.01em
  code-md:
    fontFamily: JetBrains Mono
    fontSize: 0.75rem
    fontWeight: '400'
    lineHeight: 1.25rem
    letterSpacing: 0em
  code-sm:
    fontFamily: JetBrains Mono
    fontSize: 0.6875rem
    fontWeight: '400'
    lineHeight: 1rem
    letterSpacing: 0em
  label-md:
    fontFamily: Geist
    fontSize: 0.75rem
    fontWeight: '500'
    lineHeight: 1rem
    letterSpacing: 0.01em
  label-sm:
    fontFamily: JetBrains Mono
    fontSize: 0.625rem
    fontWeight: '500'
    lineHeight: 0.875rem
    letterSpacing: 0.04em
rounded:
  sm: 0.125rem
  DEFAULT: 0.25rem
  md: 0.375rem
  lg: 0.5rem
  xl: 0.75rem
  full: 9999px
spacing:
  gutter: 0.5rem
  margin: 0.75rem
  space-xs: 0.25rem
  space-sm: 0.375rem
  space-md: 0.625rem
  space-lg: 0.875rem
  space-xl: 1.25rem
---

## Brand & Style

This design system defines an AI engineering workspace engineered for technical rigor, extreme density, and sustained cognitive focus. Built for developers, AI researchers, and systems architects working within a high-throughput desktop client, the aesthetic synthesizes the architectural precision of Linear, the real-time context streaming of Cursor, and the calm typographic restraint of Claude with the terminal's raw efficiency.

The personality is hyper-focused, disciplined, and understated. It avoids decorative distractions, loud consumer gradients, and unnecessary animations. Instead, it prioritizes immediate information parsing, surgical telemetry, tactile mechanical responsiveness, and predictable ergonomics. 

Visual design pillars:
- **Calibrated Density:** Compact geometry, low padding thresholds, and tight typographic baselines designed for 1440px+ multi-panel desktop environments.
- **Instrumental Tactility:** Subtle surface layering, razor-sharp 1px border dividers, and deliberate micro-interactions that feel closer to high-end hardware instruments than standard web wrappers.
- **Cognitive Serenity:** Ultra-deep slate backdrops reduce luminous fatigue during multi-hour terminal sessions, punctuated only by precision-grade status optics.

## Colors

The system uses an ultra-deep, cold slate foundation paired with restrained luminescent accents. Surface levels are defined through chromatic black layering rather than artificial elevation tints.

### Foundation Tiers
- **Void Canvas (`#0b0d10`):** Core application frame, backdrop beneath floating splits, and inactive window frames.
- **Base Surface (`#101317`):** Primary panel workspace, code editor canvas, and root terminal container.
- **Layer 1 Surface (`#15191e`):** Sidebars, collapsed trees, inspect panes, and tool drawers.
- **Layer 2 Surface (`#1c2128`):** Elevated cards, contextual popovers, active tabs, and input focus surfaces.
- **Layer 3 Surface (`#242c37`):** Hover states, active drag targets, and elevated modal panels.

### Structural Borders
- **Subtle / Division (`#242c37`):** Standard 1px panel boundaries, non-interactive column borders, and table rows.
- **Interactive / Highlight (`#2d3748`):** Focus boundaries, tab separators, and button perimeters.
- **High-Contrast Stroke (`#3b4554`):** Keyboard-focused inputs and active panel accents.

### Text & Glyph Hierarchy
- **Text High-Contrast (`#f1f5f9`):** Command palette queries, terminal outputs, active values, and primary headers.
- **Text Standard (`#e2e8f0`):** File names, UI navigation labels, default code tokens, and body copy.
- **Text Muted (`#94a3b8`):** Metadata, line numbers, path segments, and secondary parameters.
- **Text Dim (`#64748b`):** Timestamp watermarks, empty-state hints, and disabled actions.

### Accents & Semantic Telemetry
Accents are applied with surgical economy (maximum 5% viewport area):
- **Core Accent (`#6366f1` / `#4f46e5`):** Agent execution commands, primary actions, focused selections.
- **Auxiliary Accent (`#38bdf8`):** Streamed LLM inference spans, diff additions, telemetry traces.
- **Status: Running (`#3b82f6` / `#60a5fa`):** Active process indicators paired with subtle 1.5s harmonic pulses.
- **Status: Success (`#10b981`):** Operational health checks, clean exit codes, committed changes.
- **Status: Warning / Blocked (`#f59e0b`):** Pipeline pauses, rate-limit warnings, unreviewed assumptions.
- **Status: Error (`#ef4444`):** Fatal exceptions, broken dependencies, rejected permissions.
- **Status: Permission / Policy (`#ec4899`):** Sandboxed shell escalation, API token write requests, irreversible mutations.
- **Status: Idle (`#64748b`):** Stale threads, suspended containers, inactive agents.

## Typography

The typographic engine uses a dual-axis strategy: **Geist** handles spatial UI controls, toolbars, and contextual dialogs; **JetBrains Mono** serves as the operative engine for code diffs, logs, REPL inputs, and metric telemetry.

Rules for application:
- **Fixed Vertical Cadence:** All monospace components (logs, terminals, diff viewports) lock strictly to an 8px vertical baseline grid to maintain alignment across split panels.
- **Tabular Numerics:** Monospace digits are mandatory for all file sizes, line indicators, execution latencies, and git SHA hashes.
- **Optical Kerning:** UI headers in Geist adopt negative tracking (`-0.02em`) to prevent optical spread on retina displays.
- **Context Monospace:** UI labels referencing system state (e.g., branch names, memory allocations, environment tags) automatically inherit `label-sm` in JetBrains Mono.

## Layout & Spacing

Atlas is built as an ultra-dense, non-scrolling desktop workspace designed natively for 1440px and wider monitors. The layout philosophy relies on a **Fluid Multi-Pane Dock** where viewport verticality is preserved and horizontal space is partitioned into resizable, collapsible columns.

### Panel Architecture
- **Global Control Ribbon:** Left-anchored fixed rail (44px) housing workspace switcher, core engine state, and global telemetry.
- **Navigator Pane:** Fluid 220px–360px collapsible panel for file trees, active AI agents, and workflow sessions.
- **Editor / Stage:** Main viewport occupying remaining horizontal width, split horizontally or vertically into arbitrary code, terminal, and graph views.
- **Inspector / AI Context Rail:** Right-anchored panel (320px–480px) for ongoing model streaming, tool calls, and diff reviews.

### Density & Rhythms
- **Base Grid:** 4px micro-grid. All internal paddings resolve to `0.25rem` (4px), `0.375rem` (6px), `0.625rem` (10px), or `0.875rem` (14px).
- **Splitter Rails:** 1px visual border bounded by a 5px invisible interactive grab handle to allow frictionless re-sizing without visual bulk.
- **Panel Gutter:** 0px between adjacent panes; panes are delineated solely through `#242c37` 1px strokes.

## Elevation & Depth

Visual hierarchy is communicated via **Tonal Layering and Sharp Inset Borders** rather than diffuse drop shadows. This reflects the terminal/editor ethos, keeping rendering lightweight and rendering crisply on high-DPI displays.

### Layer Tiers
- **Tier 0 (Base Canvas - `#0b0d10`):** Outer perimeter and collapsed panel backdrops.
- **Tier 1 (Panels - `#101317`):** Primary code editing area, standard terminal shell. Inset 1px stroke `#242c37`.
- **Tier 2 (Sub-panels & Flyouts - `#15191e`):** Sidebars, log monitors, and active tool drawer surfaces.
- **Tier 3 (Floating Overlays - `#1c2128`):** Command palettes, code action hovercards, and quick-pick dialogs. 
  - *Border:* 1px solid `#2d3748`.
  - *Shadow:* `0 12px 32px -4px rgba(0, 0, 0, 0.65), 0 2px 6px 0 rgba(0, 0, 0, 0.4)`.

### Depth Accents
Overlays utilize a subtle backdrop-filter blur (`12px`) combined with an 85% opacity surface to maintain situational awareness of the background code while preventing visual clutter. Modals never use bright ambient glows; focus is retained via a 40% pitch-black scrim.

## Shapes

Atlas embraces a sharp, engineered silhouette (`roundedness: 1`). Large radii waste screen real estate and soften the technical character of a developer client.

- **Micro Controls (Inputs, Inline Badges, Menu Items):** `0.25rem` (4px).
- **Surface Containers (Panels, Hovercards, Code Diff Blocks):** `0.375rem` (6px).
- **Floating Modals & Command Palette:** `0.5rem` (8px).
- **Window Level:** Completely sharp `0px` when tiled; system-native window border rounding when untiled.

## Components

### Buttons & Segmented Controls
- **Primary Action:** Background `#6366f1`, hover `#4f46e5`, text `#f1f5f9`, border none. Height: 26px (compact) or 30px (standard). Padding: `0 10px`. Font: `label-md`.
- **Secondary Action:** Background `#15191e`, border 1px solid `#242c37`, hover border `#2d3748`, text `#e2e8f0`.
- **Ghost/Icon Button:** Background transparent, text `#94a3b8`, hover text `#f1f5f9`, hover background `#1c2128`.
- **Segmented Switcher:** Contained inside a `#0b0d10` track. Inactive items use text `#94a3b8`; active tab has background `#1c2128`, border 1px solid `#2d3748`, and text `#f1f5f9`.

### Command Palette (`Cmd+K`)
- Center-pinned top-aligned overlay (width: 640px). Surface `#1c2128` with 1px `#3b4554` border.
- **Input:** 42px height, no outer border, `#f1f5f9` text, JetBrains Mono prefix glyph (`>`), Geist input font.
- **Item Row:** 32px height, dual-line support. Active item receives `#242c37` background and 2px left indicator in `#6366f1`.
- **Shortcut Hints:** Right-aligned inline pill badges (`#101317` surface, `#64748b` text, 1px border `#242c37`, font: `code-sm`).

### Terminal & Code Diff Blocks
- **Terminal Shell:** Background `#0b0d10`. JetBrains Mono typography at 12px/18px line height. Custom scrollbars: 4px width, `#1c2128` rail, `#2d3748` thumb.
- **Code Diff Block:** Unified or split.
  - *Addition:* Line background `rgba(16, 185, 129, 0.08)`, border-left 2px solid `#10b981`, token text highlighted green.
  - *Deletion:* Line background `rgba(239, 68, 68, 0.08)`, border-left 2px solid `#ef4444`, token text highlighted red.
  - *Diff Gutter:* Monospace line numbers in `#64748b` with right-aligned 8px padding.

### Security & Permission Badges
- High-visibility notification banners for elevated terminal access, web requests, and local filesystem modifications.
- Magenta/Rose accent (`#ec4899`) background tint `rgba(236, 72, 153, 0.1)`, perimeter border 1px solid `rgba(236, 72, 153, 0.3)`.
- Includes action approval pairing: `Approve once`, `Always allow`, `Deny`.

### Activity & Agent Timeline
- Vertical timeline thread showing AI agent thought chains, shell execution, and diff staging.
- Connected via a continuous 1px `#242c37` trace line.
- Nodes use multi-modal status chips:
  - **Running:** Spin indicator icon + `#3b82f6` color badge + elapsed timer.
  - **Review Needed:** Amber icon (`#f59e0b`) + interactive inline prompt response input.