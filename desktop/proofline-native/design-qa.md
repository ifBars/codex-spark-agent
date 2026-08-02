# Proofline native spike design QA

Source visual truth: `C:\Users\ghost\AppData\Local\Temp\codex-clipboard-1f2076e5-0954-483d-8521-a2e48ed111b2.png`

Implementation screenshot: `C:\Users\ghost\AppData\Local\Temp\codex-shot-2026-08-01_18-51-14.png`

Viewport and density:

- source: 1487 x 1058 pixels at 96 DPI;
- implementation capture: 1390 x 850 pixels at 96 DPI;
- native client: 1375 x 812 physical pixels on a 120-DPI Windows display;
- Slint design surface: 1100 x 650 logical pixels; and
- comparison used the complete window at the selected completed-task state. The
  source was not downsampled because the full-view layout failure is visible at
  both native sizes; no pixel-perfect spacing claim is made.

## Findings

- **P1 - Anchored composer and status ribbon are clipped**
  - Location: bottom of the native window.
  - Evidence: the source keeps the entire composer and persistent branch,
    checkpoint, elapsed, token, pricing, and privacy ribbon visible. The native
    capture shows only the composer's upper border; its controls and the status
    ribbon are outside the rendered client area.
  - Impact: model, reasoning, workspace authority, pricing availability, and the
    required `Network gate pending` state are not inspectable. This changes the
    primary interaction hierarchy and blocks the spike.
  - Fix: replace the current fixed-position Slint surface with a DPI-correct root
    layout whose bottom regions participate in the same vertical layout. Re-test
    at 100%, 125%, 150%, and 200% Windows scaling before another fidelity pass.

- **P2 - Evidence document is materially smaller and more top-heavy**
  - Location: main completed-task document.
  - Evidence: the source gives the title, summary, actions, rows, validation, and
    activity disclosure a larger, calmer paper-like rhythm. The implementation
    compresses the evidence rows and leaves a large unused lower region before
    the clipped bottom surface.
  - Impact: the result reads like a dense table prototype instead of the selected
    evidence document.
  - Fix: restore the source's vertical rhythm after the DPI/layout blocker is
    resolved; keep the evidence rows readable without making the document feel
    like an operations grid.

- **P2 - Secondary actions and persistent state are missing**
  - Location: action row and footer.
  - Evidence: `Continue`, `Open files`, branch, checkpoint, elapsed, token,
    pricing, and network/privacy state are visible in the source but absent from
    the implementation capture.
  - Impact: the spike does not yet exercise the source's complete information
    architecture.
  - Fix: add the secondary actions only after the anchored regions render
    reliably; preserve `Pricing Unavailable` and `Network gate pending` rather
    than mock authority.

## Required fidelity surfaces

- Fonts and typography: the Georgia-like display title and compact mono evidence
  rows preserve the intended hierarchy, but the body and table type are too small
  at the captured density. Blocked by the DPI/layout issue.
- Spacing and layout rhythm: rail-to-document proportion is directionally close;
  vertical rhythm and the bottom anchors fail P1.
- Colors and visual tokens: warm white, quiet orange, green completion, red
  deletion, and gray evidence tones are directionally faithful. No gradient or
  unsupported decorative treatment was added.
- Image quality and asset fidelity: the existing Proofline mark is reused from
  `desktop/proofline/assets/proofline-mark.png` and is sharp. No replacement
  illustration or placeholder asset is present.
- Copy and content: selected-task, changed-file, validation, and collapsed-activity
  copy match the concept closely. Footer state copy is not visible and therefore
  fails the product requirement.

Focused-region comparison was not needed for this pass: the full-view comparison
shows a decisive P1 omission, and the dense evidence rows are readable enough to
classify the remaining P2 drift. A focused typography and composer comparison is
required after the P1 is fixed.

## Comparison history

1. Initial native capture showed only a blank white surface. The default FemtoVG
   path was replaced with the explicitly selected Slint software renderer.
2. The second pass rendered the rail and evidence hierarchy but clipped the
   composer and status ribbon at 125% Windows scaling. The window was resized
   from system DPI and the source mark and proportional native surface were added.
3. The current capture keeps the rendered evidence hierarchy but still clips the
   anchored bottom regions. No further visual claim is accepted.

## Implementation checklist

1. Falsify or fix Slint's DPI/root-layout behavior with a layout-only reproducer.
2. If the reproducer does not pass all four scale factors, start the WinUI 3
   fallback spike from the renderer decision record.
3. Re-capture the same completed-task state with the full composer and footer.
4. Test task selection and activity disclosure with keyboard and UI Automation.
5. Repeat full-view and focused-region design QA.

final result: blocked
