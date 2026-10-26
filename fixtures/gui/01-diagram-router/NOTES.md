# 01 — Diagram router (GUI, Monodraw 1.6.1) — COMPLETE: 128 of 128 cases
How each case was made: new document, then
- box A: Rectangle tool, 10x5 at top-left col 40,row 20 (1-based canvas cells)
- box B: 10x5 at the layout position below
- Line tool dragged from A's attachment point on side s1 to B's attachment point on side s2. Default line Type = "Diagram", Style Single (Light), Start head None, End head "▶ (Smart)".
- Attachment points sit one cell OUTSIDE the edge midpoints: for a 10x5 box at (c,r): R=(c+10,r+2) L=(c-1,r+2) T=(c+5,r-1) B=(c+5,r+5).
- Attachment was verified once by moving B with the arrow keys: the line rerouted, so the ends really were attached (and weren't just drawn to those points).
- Saved NAME.monopic, then File > Export > Text with the default options (trim trailing whitespace ON, trim padding ON) → NAME.txt.
Names: <layout>-A<s1>-B<s2>.

Layouts (B top-left col,row; A at 40,20):
side 70,20 | stacked 40,35 | diagBR 65,33 | diagBL 15,33 | diagTR 65,7 | diagTL 15,7 | overlapX 45,33 | overlapY 65,22

DONE: all 8 layouts x 16 side combos (128 cases), each with .monopic + .txt. Every .txt contains an arrowhead.
