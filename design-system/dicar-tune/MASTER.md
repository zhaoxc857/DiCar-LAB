# DiCar Tune Design System

> This file is the source of truth for the desktop UI. Page files under pages may override layout only; semantic colors, accessibility, and interaction rules remain global.

## Design character

- Modern dark engineering instrument panel.
- Variance 4/10, motion 3/10, density 9/10.
- High information density, low decoration, clear operational hierarchy.
- No neon glow, ambient blobs, decorative glass blur, haptics, marketing hero layouts, or continuous animation.

## Semantic colors

| Role | Value |
| --- | --- |
| background | #071018 |
| surface | #0B1620 |
| surface-raised | #101F2B |
| surface-hover | #152836 |
| border | #263746 |
| text | #F2F7FA |
| text-muted | #A8BAC7 |
| interactive | #38BDF8 |
| interactive-strong | #0EA5E9 |
| success | #34D399 |
| warning | #FBBF24 |
| danger | #FB7185 |
| focus-ring | #38BDF8 |

Interactive cyan never means success. Green is success only, amber is warning only, and rose is danger only. Every status also has text or an icon.

## Typography

- UI: Inter, Noto Sans SC, Segoe UI, Microsoft YaHei, sans-serif.
- Data: JetBrains Mono, Cascadia Mono, Consolas, monospace.
- Use tabular figures for telemetry, timestamps, units, revisions, and diagnostics.
- Body text is never smaller than 12 px on desktop; form controls use 13 or 14 px.

## Geometry

- shadcn style: New York.
- Base radius: 6 px.
- Spacing rhythm: 2, 4, 8, 12, 16, 24, 32 px.
- Desktop control height: 32 or 36 px; icon hit target is at least 40 x 40 px.
- Borders separate dense regions; shadows are reserved for overlays.
- Z-index scale: 0, 10, 20, 40, 60, 100.

## Components

- Cards do not move or scale on hover; border and surface color may transition.
- Buttons use one primary action per region. Flash or commit actions are visually separated from ordinary writes.
- Inputs always have visible labels, units, ranges, and nearby error text.
- Read-only values are semantically readonly, never presented as disabled editable controls.
- Dialogs trap focus, close on Escape when safe, and return focus to their trigger.
- Skeletons appear for waits over 300 ms; progress buttons prevent duplicate submission.
- Toasts use polite live regions and never replace field-level errors.

## Workbench layout

- At 1280 px and above: 264 px parameter navigation, minmax(420 px, 1fr) editor, minmax(440 px, 1.15fr) waveform.
- At 1024 to 1279 px: navigation drawer plus editor and waveform columns.
- Below 1024 px: parameter and waveform tabs, persistent bottom change bar.
- Fixed bars reserve content space and never cover scrolling content.

## Waveform

- Canvas rendering with bounded buffers and min/max-per-pixel downsampling.
- Maximum eight visible channels.
- Each channel uses color plus line style.
- Visible legend, units, current values, pause, cursor, time window, and text/table alternative.
- Empty, loading, paused, disconnected, and error states are explicit.

## Accessibility

- WCAG AA: text 4.5:1, large graphics 3:1.
- Visible 2 px cyan focus ring and logical keyboard order.
- Skip link to main content.
- Icon-only buttons have accessible names.
- Dynamic status uses aria-live; urgent errors use role alert.
- UI works at 200 percent zoom.
- prefers-reduced-motion reduces transitions to effectively zero.

## Motion

- 150 to 220 ms, ease-out on entry and ease-in on exit.
- Only opacity and transform.
- Motion communicates cause and effect; no decorative looping.

## Icons

- Use Phosphor outline icons consistently.
- Never use emoji as structural icons.

## Required visual verification

- 1280 x 720 desktop.
- 1024 x 768 compact desktop.
- 768 x 1024 tablet-shaped viewport.
- Keyboard-only navigation.
- Reduced-motion mode.
- 200 percent browser zoom.

