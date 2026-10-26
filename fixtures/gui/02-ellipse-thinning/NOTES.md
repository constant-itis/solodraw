# Step 2 — Ellipse rasterization / thinning — COMPLETE

Black-box only: everything was produced by driving the Monodraw 1.6.1 (111) GUI and exporting with File > Export > Text.

## Coverage
- H = 3..20, W = 3..20: all 324 sizes in both styles.
- Files are `ellipse-HNN-WAA-BB-{line|star}.{monopic,txt}`. Each document contains every ellipse of height NN with widths AA..BB, left to right, separated by a 3-column gap. The width groups are 03-10, 11-16 and 17-20.
- 54 documents × 2 styles = 108 .monopic + 108 .txt.
- Automated check: in every .txt the non-blank rows equal H, and the column segments split by blank columns equal [AA..BB]. 0 failures.

## Styles
- `line` is the default ellipse border, Monodraw's own glyph choice (. ─ ' ` ( ) ╱ ╲ ; : │ , etc.).
- `star` uses a custom border: the inspector's Character field is set to `*`.
- The ellipse inspector has no border-type popup, only Border (on/off), Blended and Character. Blended wasn't varied because it only matters where shapes overlap and these ellipses are isolated.

## Method notes
- Each ellipse was created as a 3x3 drag, then given its W and H through the inspector Width/Height fields. The inspector resize is center-anchored, so the drag was centered where the final ellipse should sit.
- Each style was drawn in a fresh document, because Monodraw autosaves into the last-saved file.
