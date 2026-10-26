# 04 — PNG/SVG cell metrics (GUI, Monodraw 1.6.1)
Driven by screencapture + synthetic mouse/keyboard events (no binary inspection).

Setup: new document; Rectangle tool dragged to a 10x5 rect (inspector confirms Width 10, Height 5) with its top-left at canvas column 10, row 5 (1-based, per the rulers). Text enabled in the inspector and typed "Ag". Defaults: alignment left, position top, Font "None", Single (Light) border, opaque fill, padding 0.
Canvas font (Preferences > Text): Menlo Regular 11, anti-aliasing on, subpixel AA on.

## Results (File > Export, default options except Resolution)
The PNG screenshots are not included in this repository; the sizes below were measured from them.
| file | settings | size |
|---|---|---|
| rect10x5-Ag-1x.png | PNG Normal (1x) | 134x118 px, 72 dpi |
| rect10x5-Ag-2x.png | PNG Retina (2x) | 268x236 px |
| rect10x5-Ag-3x.png | PNG Retina HD (3x) | 402x354 px |
| rect10x5-Ag-default.svg | SVG | width="71" height="66" |
| rect10x5-Ag-1x-menlo14.png | font changed to Menlo 14, PNG 1x | 172x154 px |
| rect10x5-Ag-menlo14.svg | Menlo 14, SVG | width="91" height="86" |
| rect10x5-Ag.txt | Text export | the plain-text rendering |
| rect10x5-Ag.monopic | the document | |

Derived: Menlo 11 gives a 7x13 px cell; Menlo 14 gives 9x17. Size = cols*cw + 1 by rows*ch + 1.
- PNG defaults to Trim whitespace padding OFF, so the image starts at canvas origin: 19 cols (through the rect's right edge) x 9 rows -> 19*7+1=134, 9*13+1=118.
- SVG defaults to Trim ON: 10*7+1=71, 5*13+1=66.
- SVG: the root has width/height but NO viewBox, and there are NO <text> elements. Every glyph, including box-drawing chars and "A"/"g", is emitted as a filled <path> (fill="rgb(0, 0, 0)" fill-opacity="1.000000"), coordinates with 6 decimals. No font-family appears in the SVG.
- Font restored to Menlo 11 afterwards (verified in defaults).
