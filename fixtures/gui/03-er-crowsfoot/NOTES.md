# Step 3 — ER crow's-foot arrowheads — COMPLETE

Black-box only: everything was produced by driving the Monodraw 1.6.1 (111) GUI and exporting with File > Export > Text.

## Inspector observations (Line, Type: Diagram)
- The End Head popup lists, in order: None | ▶ (Smart), ▷ (Smart) | 8 triangles | squares, diamonds and circles | ER heads: One, Many, One Only, Zero or One, One or Many, Zero or Many.
- The Notation popup has Crow's Foot (Orthogonal), Crow's Foot (Diagonal) (the default) and Crow's Foot (Hat).

## Coverage
- 6 ER heads × 3 notations = 18 documents, `er-<head>-<notation>.{monopic,txt}`.
- Every document has the same 5 Diagram lines, each with Start Head None and End Head set to the ER head (cells are col,row):
  - right: (5,5)→(25,5)
  - left: (25,10)→(5,10)
  - down: (35,3)→(35,15)
  - up: (45,15)→(45,3)
  - bent: (55,3)→(75,15), auto-routed with two elbows and ending rightward
- The lines are free-floating, not attached to shapes.

## Findings
- Notation only affects heads that contain the "many" fork. The .txt exports of One, One Only and Zero or One are byte-identical across all 3 notations. The .monopic files still differ, so the notation is stored even when it has no visible effect.
- The Many / One or Many / Zero or Many heads differ per notation. Diagonal uses ╱ ╲ spanning the adjacent rows or columns; Orthogonal and Hat use other glyphs (see the files).
- Zero or Many with Hat renders as ○◀ (circle + filled triangle).
