# Capability Atlas design QA

## Compared inputs

- Source visual truth: `C:\Users\ghost\.codex\generated_images\019f9d9a-89ae-7123-9f5b-029428bdcbde\call_QLfciZmU5VAyn5bBjUa0PlEo.png`
- Browser-rendered implementation: `web/design-qa-capability-atlas.png`
- Full-view comparison: `web/design-qa-capability-atlas-comparison.png`
- Focused controls/chart/ranking comparison: `web/design-qa-capability-atlas-focus.png`
- State: expanded reasoning suite, total API tokens, weighted quality, both runners, low/medium/high reasoning, confidence ranges visible

## Viewport and normalization

- Source pixels: 1487 × 1058.
- Browser CSS viewport: 1706 × 960 at device pixel ratio 0.9375.
- Raw browser capture: 2248 × 1280. The in-app browser capture contained repeated viewport tiles, so the authoritative first viewport was cropped to 1580 × 900.
- Full-view comparison normalized the north 1487 × 900 source crop and the 1580 × 900 implementation crop to equal 1487 × 900 frames before placing them side by side.
- Focused comparison normalized the source and implementation control/chart/ranking regions to equal 1260 × 470 frames.
- The selected visual target is desktop-only. Existing 840 px and 620 px responsive breakpoints were retained and updated for the atlas rail, one-column chart grid, stacked ranking ledger, and mobile controls; there was no separate mobile source frame to compare.

## Findings

- No actionable P0, P1, or P2 differences remain.
- Typography: the implementation preserves the mock's heavy editorial display face, neutral product body face, monospaced evidence labels, compact control type, and clear rank hierarchy. Real content wraps without clipping.
- Spacing and layout: the sticky benchmark rail, compact editorial header, shared controls, dominant overall chart, ranking ledger, and visible two-column category continuation match the selected Capability Atlas hierarchy. Thin rules and whitespace provide structure without dashboard-card nesting.
- Colors and tokens: the warm mineral canvas, near-black header, orange active state, blue Spark series, orange Codex series, and pale green ideal zone match the selected direction and existing product tokens.
- Image and asset fidelity: the selected design contains no raster imagery. GitHub and interface actions continue to use the existing Phosphor icon library; no placeholder illustration, custom SVG, CSS icon, or generated decorative asset was introduced.
- Copy and content: fictional mock values and rankings were intentionally replaced by the measured benchmark data. Category descriptions, scenario counts, run counts, confidence notes, and ranking uncertainty all come from the published dataset.
- Interaction: the benchmark rail navigates to category anchors. Shared runner/reasoning controls update every chart together; disabling High reduced visible chart points from 36 to 24. Switching to the pilot reduced the page to one overall chart, one rail entry, and six points, then restored the expanded atlas.
- Browser console: no warnings or errors were present after the final reload; only Vite debug and React development information messages were recorded.

## Comparison history

1. Initial implementation retained a single category tab switcher.
   - Finding: P1 information-architecture mismatch; users could not scan multiple benchmark charts together.
   - Fix: replaced the tab strip with the sticky atlas rail, one global control state, a dominant overall module, ranking ledger, and five concurrently rendered category charts.

2. First atlas render let the overall chart and ranking ledger consume the full initial viewport.
   - Finding: P2 density mismatch; category continuation was not visible above the fold.
   - Fix: tightened the editorial header, chart viewport, ranking rows, methodology block, and compact-chart heights until Coding and Math & data visibly begin in the first viewport.

3. Focused comparison exposed a letterboxed overall plot and uncertainty derived from a clipped quality range.
   - Finding: P2 chart fidelity and data-accuracy issue.
   - Fix: added a wide chart viewBox for the atlas module and made the ranking ledger use the published `qualityCi` value when available.

4. Final comparison:
   - Evidence: `web/design-qa-capability-atlas-comparison.png` and `web/design-qa-capability-atlas-focus.png`.
   - Result: no remaining P0, P1, or P2 issues.

## Follow-up polish

- P3: add scroll-spy state to the benchmark rail once the category set grows enough to make active-section tracking materially useful.
- P3: add a dedicated log-scale token axis only after documenting how it should behave for this narrow six-point reasoning matrix.

## Implementation checklist

- [x] Shared controls drive every chart.
- [x] Overall chart and measured ranking ledger are visible together.
- [x] Five category charts render concurrently.
- [x] Ideal zones and uncertainty ranges remain visible.
- [x] Historical datasets degrade to a single overall chart.
- [x] Browser interactions and console state are verified.
- [x] Lint, unit tests, production build, and static-host tests pass.

final result: passed
