# .monopic and Monodraw Behavior: Specification for solodraw

Status: **draft**, 2026-10-23. Target studied: Monodraw **1.7.1 (build 118)**.

## 0. Ground rules

* **Scope.** This document describes *behavior, formats, enums and data tables*, observed from
  running Monodraw 1.6/1.7 and from its file format, for the purpose of interoperability. The solodraw
  implementation must be written from this document. Data tables (key names, enum values, character
  tables) are interoperability facts and are reproduced.
* **Evidence.** Claims marked **[file]** were verified against the bundled `Monodraw-Intro.monopic`;
  **[inferred]** means reasoned, not directly observed.
* Architecture note: essentially all application logic (model, persistence, rendering, export, UI
  controllers) lives in a single shared component separate from the thin application executable.

---

## 1. File format (`.monopic` / `.monojson`)

### 1.1 Container

| Type | UTI | Bytes on disk |
|---|---|---|
| `.monopic` | `com.helftone.document.monopic` | 9-byte magic `FF 4D 4F 4E 4F 50 49 43 01` (`\xffMONOPIC` + `0x01`) followed by a **gzip** stream (written at deflate level 2) of the UTF-8 JSON document |
| `.monojson` | `com.helftone.document.monojson` | the JSON document directly, pretty-printed, no magic |

On save, the magic is written, then the stream is gzipped at deflate level 2; pretty-printing is used
only for monojson. On load, the magic is compared byte-for-byte and the rest gunzipped. Note: `file`,
`xz -t` and old p7zip misidentify these files — irrelevant to the format itself.

Reader rules: if the data starts with the magic → strip 9 bytes → gunzip → JSON. If the type is monojson →
JSON directly. Anything else → reject.

### 1.2 JSON envelope and versions

Current model version = **5**. Writers emit:

```json
{ "header": { "v": 5 }, "object_list": [ { "object_id": "<UUID>", "type_id": <int>, ... }, ... ] }
```

The persisted key vocabulary (234 entries) includes: `header`, `v`,
`object_list`, `objects` (legacy), `object_id`, `type_id`; the complete list is used in `crates/core/src`.

Version history as handled by the loader:

| v | Envelope | Notes |
|---|---|---|
| 0–2 | `{"header":{"v":n}, "objects": {"obj-N": {...}}}` | ids are `obj-N` dictionary keys; references are those strings |
| 3 | same as 2 | |
| 4 | `{"header":{"v":4}, "object_list": [...]}` | ids become uppercase UUID strings stored in `object_id` |
| 5 | same as 4 | color defaults normalized; optional *default elision* ("minification") |

If `header.v` > 5 the open fails with "You're trying to open a file created by a newer version of
Monodraw…". A missing/invalid header aborts migration.

### 1.3 Migration chain (load-time)

Applied in order until the document is v5. All operate on the raw JSON objects.

* **0 → 1**: retype legacy shapes and set `shape_type`:
  type 1→`shape_type` 6; 3→8; 5→2; **4→type 20**, shape_type 1; **8→type 20**, shape_type 5;
  **14→type 20**, shape_type 4 and `text_enabled=true`. For those retyped objects: if
  `rectangular_border == 1` set it to 3, otherwise set `rectangular_border_enabled=true`;
  `background_type` 5…10 → `background_enabled=true`, type 4; 2…4 → `background_enabled=true`;
  1 → type 2. Other types untouched.
* **1 → 2**: every type-20 object's text moves into a new **type 27 Text** object
  (new UUID key) referenced by `model_text`. Field renames: `text_enabled→enabled`, `text→text`,
  `text_sweep→sweep`, `text_movement→movement`, `text_alignment→alignment`,
  `text_position→position`, `text_inside→inside_border`, `lf_id→letter_font_id`,
  `lf_name→letter_font_name` (font keys copied only when present). `doc_ref` copied.
* **2 → 3**: no-op.
* **3 → 4**: generate a fresh uppercase UUID for every `objects` key; recursively rewrite
  **every string** in the whole object graph (dictionary keys *and* values, inside arrays) that exactly
  equals an old id to its new UUID; set `object_id` on each object (error if one already exists); replace
  `objects` with `object_list` = array of the objects. (Quirk: a user string that happens to equal an id
  such as `"obj-5"` is rewritten too.)
* **4 → 5**: for the color keys `content_color`, `shadow_color`, `mps_bckg_color`,
  `mps_border_color`, `color`: a value of `0xFF000000` becomes `0x100000000` ("no color / default").
* **v5 load**: re-insert elided defaults (§1.6). (v4 data is never elided.)

A loader built from this section (`tools/monopic.py`) migrates the bundled v2 intro document
to 1,443 objects with 3,219 UUID references and **0 dangling** references, and survives a minified v5
save/load round-trip with no value changes **[file]**.

### 1.4 Object graph

* Objects reference each other by id string (`*_ref`, `*_refs`). Every object carries `doc_ref` → the
  Document (type 0).
* Root: the single **Document** (type 0): `alphabet` (1 = Unicode, 2 = ASCII; each has a display
  name used for auto-naming), `canvas_refs` (ordered canvases),
  `shape_seq_numbers` (map `shape_type` → last auto-name counter; new shapes are named
  `"<DisplayName> <n+1>"`).
* Canvas (type 1) → `subshape_refs` (tree of shapes; groups nest further). Order of `subshape_refs` is
  the z/render order (see §4).
* Positions (types 11, 12, 16, 17, 19, 22, 23, 24) are standalone objects listed in a shape's
  `position_refs` and referenced by line ends (`position_ref`) and frames (`frame_anchor_ref`,
  `anchor_point`).
* Tixmaps (type 18) hold freehand/pasted character content (`content_tixels`, `border_tixels`,
  `content_tixmap_ref`).

### 1.5 `type_id` → class

| id | class | id | class |
|---|---|---|---|
| 0 | Document | 17 | RectangularContentPosition (`content_point`, `on_border`) |
| 1 | CanvasShape | 18 | Tixmap (`tixels`) |
| 3 | GroupShape | 19 | LineDiagramPosition (`line_dia_pos`, `line_dia_seg`) |
| 5 | LineShape | 20 | RectangularShape (rectangle / text field / surface) |
| 7 | TriangleShape (`triangle`) | 21 | DiamondShape |
| 9 | LineSegment (`direction`) | 22 | DiamondStandardPosition (`dpos`) |
| 10 | LineSegmentEnd (`position_ref`, `head_type`) | 23 | DiamondContentPosition (`dcpos_point`) |
| 11 | StaticPosition (`point`) | 24 | LinePointIndexPosition (`line_pt_idx`, `line_pt_rel`) |
| 12 | RectPosition (`rect_position`, `position_flags`) | 25 | ImageShape (`img_data`, `img_bounds`, `img_opacity`) |
| 16 | LineSegmentPosition (`line_seg_pos_type`, `line_seg_pos_shape_ref`) | 26 | TableShape |
| | | 27 | Text |
| | | 28 | EllipseShape |
| | | 29 | TableCell |

Unknown/absent type ids decode to nothing (ids < 30 present in a
fixed class table decode normally, else nil). Ids 2, 4, 6, 8,
13, 14, 15 only exist in legacy files and are migrated away (§1.3).

`shape_type` enum: 1 Rectangle, 2 Line, 3 Triangle, 4 Text Field, 5 Surface, 6 Canvas,
8 Group, 9 Diamond, 10 Image, 11 Table, 12 Ellipse. (Type-20 objects use 1/4/5.)

### 1.6 Value encodings and default elision

* Points/sizes are strings `"x,y"` (e.g. `origin`, `frame_size "17,7"`, `shadow_offset`).
* Characters ("tixels") are strings of one character (1–2 UTF-16 units); `""` = unset/default.
* Colors are unsigned integers; **`4294967296` (`0x100000000`) = no explicit color** (a missing key
  decodes to the same value).
* Dash patterns are `{"o":int,"p":int,"u":int}` (see §5).
* **On-disk quirk (verified [file]):** on rectangles the key `border_dash` holds the *boolean* enable flag and
  `border_dash_enabled` holds the *dash dictionary* — the names are swapped relative to their meaning.
  Lines use `line_dash_enabled` (bool) / `line_dash` (dict) normally.

**Default elision** (v5, optional; enabled by the hidden preference
`monodraw.feature-flags.json-minification-enabled`, default off): when writing, any key
whose value equals the per-type default below is omitted; when reading a v5 file, missing keys are
re-filled with these defaults. Readers MUST implement
re-filling; writers MAY skip elision.

Base *shape* defaults (all shapes): `cthrough false, name_edited false, origin "0,0", tag 0, hidden false,
locked false, surface_pos_enabled true, attachable true, content_blended true, content_color 0x100000000,
shadow_enabled false, shadow_offset "0,0", shadow_tixel "", shadow_intensity 4, shadow_color 0x100000000,
subshape_refs []`.
*Primitive* shapes (20, 21, 28) add: `background_type 2, background_enabled true, background_tixel "",
rectangular_border_enabled true, mps_bckg_color 0x100000000, mps_border_color 0x100000000`.

| type | additional / overriding defaults |
|---|---|
| 0 Document | `alphabet 1` |
| 1 Canvas | shape base + `attachable false, position_refs [], shape_type 6` |
| 3 Group | shape base + `shape_type 8, position_refs [], attachable false, subshape_selection false` |
| 5 Line | shape base + `shape_type 2, line_type 3, line_style 3, line_dash_enabled false, line_dash {o0,p0,u0}, head_notation 2` |
| 10 SegmentEnd | `head_type 0, dirset 0` |
| 11 StaticPosition | `dirset 0` |
| 12 RectPosition | `dirset 0, position_flags 1` |
| 18 Tixmap | `tixels {"text_content":[], "text_points":[]}` |
| 20 Rectangular | primitive + `shape_type 4, rectangular_border 4, frame_offset "0,0", frame_corner 1, content_offset "0,0", border_tixel_{tl,tr,bl,br,ts,bs,ls,rs} "", border_dash false, border_dash_enabled {o0,p0,u0}` |
| 21 Diamond | primitive + `shape_type 9, anchor_pos 1, bt_{tp,bp,lp,rp} "", btd_{ul,ur,ll,lr} ""` |
| 27 Text | `enabled true, sweep 0, movement 0, alignment 1, position 1, inside_border true, color 0x100000000, inset_{top,bottom,left,right} 0` |
| 28 Ellipse | primitive + `shape_type 12, border_tixel "", text_container_control 0.25` |

Elision deliberately has no entries for other types; their objects are always written in full.

The complete key vocabulary (every persisted key and its constant name) is used in `crates/core/src`.

### 1.7 Tixmaps (type 18)
`tixels = {"text_content": [c…], "text_points": ["x,y"…]}`: parallel arrays, one entry per non-null cell, row-major.
Chars are 1–2 UTF-16 units; `""`/≥3 units → null. Mismatched lengths → ignore the whole map. Segments and colours are
not persisted. Placement: rectangle `content_tixels` at the frame's **anchor point** (keypad `frame_corner`, not
necessarily top-left) + `content_offset` + point — verified on the user's real files; RectangularContentPosition
(type 17) `content_point` uses the same origin; rectangle
`border_tixels` point = (part 0–15, offset) overriding border cells: part 0 top, 1 bottom, 2 left, 3 right side,
offset along the side measured from the same content origin (x for top/bottom, y for left/right); 4–7 the
TL/TR/BL/BR corner; 8–15 the same parts on the ring one cell outside the frame. A type-17 position with
`on_border` uses this (part, offset) encoding for its `content_point` (§5.6); diamond `content_tixels` at the diamond rect origin;
line `content_tixmap_ref` x = index into the line's path cells. Content cells are drawn with replace blending.

---

## 2. The character cell ("tixel") model

Everything drawn on a canvas is a grid of **tixels** (text pixels). A tixel is a 64-bit value:

| bits | meaning |
|---|---|
| 0–7 | **segment presence** mask (8 half-segments from the cell centre) |
| 8–31 | 3-bit **style** per segment *s* at bits `8+3s … 10+3s` |
| 32–63 | the **character**: one or two UTF-16 code units (surrogate pair) |

Segment indices (derived from the glyph table in §3):
`0 up, 1 down, 2 left, 3 right, 4 up-left, 5 up-right, 6 down-left, 7 down-right`.

Segment styles: `1` = hidden/eraser, `2` light, `3` heavy, `4` double. Line style → style: the tixel
line style 0…4 maps to a "box-draw variant" `[1,0,3,4,5]`, and variants 3/4/5 give segment styles
2/3/4 (anything else 1).

A tixel with segments set is a **line-drawing tixel**: its visible character is *derived* from the
segments at output time (§3), so overlapping lines can merge into correct junctions. A tixel with no
segments is a plain character. Null tixel = transparent/empty.

The 13 named box-draw symbols (used by the character palette / pencil) are fixed segment masks:
1 `L+R`, 2 `U+D`, 3 `UR+DL` (╱), 4 `UL+DR` (╲), 5 `U+D+L+R`,
6 `D+R`, 7 `D+L`, 8 `U+R`, 9 `U+L`, 10 `U+L+R`, 11 `D+L+R`, 12 `U+D+L`, 13 `U+D+R`.

### 2.1 Compositing (blend modes)

Writing a tixel into a buffer takes a blend mode:

* **0 — merge** (used for "blended" content): if the incoming tixel is empty, nothing changes. If both the
  existing and the incoming tixel are line-drawing tixels, the incoming segments are OR-ed in, each
  incoming segment overwriting that segment's style — except an incoming segment of style 1 *removes*
  that segment. Otherwise the incoming tixel replaces the cell (character tixels overwrite).
* **1 — replace**: cell := incoming.
* **2 — erase**: if both are line tixels, remove the incoming segments from the cell; if both are plain
  and identical, clear the cell.

Every write also stores the cell's color and marks the cell as written.

---

## 3. Junction characters (where lines meet)

After all shapes are composited into a buffer (with merge blending, so segments from different lines
accumulate in shared cells), each line-drawing tixel is converted to a character by the
subtixel-to-text conversion. Algorithm:

1. Drop every segment whose style is 1 (hidden).
2. Effective style = **maximum** style over the cell's segments (so light ⊕ heavy → heavy).
3. Look up the remaining mask:
   * **Orthogonal only** (bits 0–3):
     * Unicode: pick from the light / heavy / double box-drawing set by effective style
       (style ≤ 1 → empty cell). `U`, `D`, `U+D` → `│┃║`; `L`, `R`, `L+R` → `─━═`; corners
       `┌┐└┘` / `┏┓┗┛` / `╔╗╚╝`; tees `├┤┬┴` / `┣┫┳┻` / `╠╣╦╩`; cross `┼╋╬`.
     * ASCII: vertical-only `|`, horizontal-only `-`, anything else (corner/tee/cross) `+`.
   * **Diagonal / mixed**: `/`-family → `╱` (Unicode) or `/`; `\`-family → `╲` or `\`; specials:
     `UL+UR` → `V`, `DL+DR` → `Λ` (Unicode) / `X` (ASCII), `UL+DL` → `▏` / `|`, `UR+DR` → `▕` / `|`;
     every other diagonal combination → `╳` / `X`. (Diagonal glyphs ignore style.)
4. Write the character into bits 32–63 (segments kept).

The full 255-row table (mask → ASCII, Unicode light/heavy/double) generated from the observed behavior
is implemented in `crates/core/src/junction_table.rs`.

Consequences worth preserving:
* A document's alphabet only affects this final conversion; switching alphabet re-renders everything
  without touching the model (shapes listen for an alphabet-change notification).
* Junctions form only between content drawn with merge blending: blended lines, blended rectangles/ellipses
  *with a border*, and tables. Everything else overwrites cells (see §4.1).

---

Sections 4–8 condense the area reports. The exhaustive tables and open
questions not covered elsewhere are noted inline in the relevant subsections below.

## 4. Geometry conventions and render pipeline

* Integer cell grid, **x right, y down**. Rects are `(x, y, w, h)`.
* **Keypad positions** (used by `frame_corner`, `rect_position`, watermark…):
  1 TL `(x,y)`, 2 top-centre `(x+⌊w/2⌋,y)`, 3 TR `(x+w−1,y)`, 4 left-mid, 5 centre, 6 right-mid, 7 BL `(x,y+h−1)`,
  8 bottom-centre, 9 BR. Values 10–25 are points one cell **outside** the rect, each with a fixed exit direction:
  10 `(x,y−1)`↑, 11 `(x−1,y)`←, 12 `(cx,y−1)`↑, 13 `(X1,y−1)`↑, 14 `(X1+1,y)`→, 15 `(x−1,cy)`←, 16 `(X1+1,cy)`→,
  17 `(x,Y1+1)`↓, 18 `(x−1,Y1)`←, 19 `(cx,Y1+1)`↓, 20 `(X1,Y1+1)`↓, 21 `(X1+1,Y1)`→, 22 `(x−1,y−1)`↑←,
  23 `(X1+1,y−1)`↑→, 24 `(x−1,Y1+1)`↓←, 25 `(X1+1,Y1+1)`↓→ (X1 = x+w−1, Y1 = y+h−1). Attachment points are
  recomputed from the live frame on every read, so attached line ends follow moves/resizes.
* **Direction masks** (`dirset`): up 1, down 2, left 4, right 8; 0 = unconstrained.

### 4.1 Compositing order
1. Shapes are drawn into one memory tixmap. The canvas render tree is visited **pre-order with children in
   reverse**: a parent draws before (under) its children, and `subshape_refs[0]` is drawn **last → on top**.
   Hidden shapes are skipped.
2. Each shape writes its cells with its **content blend mode**:

   | shape | blend mode |
   |---|---|
   | Line | 0 merge if `content_blended`, else 1 replace |
   | Rectangle / Ellipse | 0 if `content_blended` **and** border enabled, else 1 |
   | Table | always 0 |
   | everything else (text, groups, canvas, surface, diamond, triangle, image) | 1 |

   So lines merge with each other and with bordered blended boxes (giving `┬ ├ ┼` joins); text and other
   content overwrite.
3. **Once**, after all shapes: convert line-drawing tixels to glyphs (§3) using the alphabet.
4. Then comment decoration and watermark (§8) — drawn after conversion, so they never form junctions.

Within a rectangle the draw order is shadow → background → border → text → content tixels → positions.
With the border enabled, content tixels are clipped to the interior (frame inset 1): a
content cell on the border is not drawn (round-trip test, `'///` vs `|///`). Within a line: body → heads → content tixels → positions.

### 4.2 Text output
* Export rect = union of the shapes' content extents (canvas coordinates). Option bits:
  **1** anchor at canvas origin → rect becomes `(0,0,maxX,maxY)` (leading blank rows/cols kept). Real v3 files place
  content at negative coordinates and still export fully, so this implementation uses `(min(0,minX), min(0,minY))`
  as the top-left (verified against the fixture: identical placement);
  **2** trim padding → drop the origin anchoring only: the rect is the shapes' extent, so an empty shape (a bare
  1×1 surface) still widens it (golden round-trip: WesASCIIcard, Emily1; was inferred as "box of non-null cells");
  **4** trim trailing whitespace per line.
  Whole-canvas export uses option 1 (+2 if "trim padding"); "selected shapes only" uses 0 (+2).
* Rows are joined with `\n`, **no trailing newline**. A cell with neither character nor segments → one space;
  otherwise its 1–2 UTF-16 units. No double-width compensation. Trailing-whitespace trimming treats null, TAB and
  SPACE as whitespace; blank rows remain as empty lines; leading whitespace is never trimmed. Output is UTF-8.
* ASCII vs Unicode only changes the generated line/junction glyphs; user-typed characters pass through unchanged.

## 5. Lines and connectors

### 5.1 Model
A line (type 5) = ordered `segment_end_refs` (n+1 ends, each with a `position_ref` and `head_type`) and
`segment_refs` (n segments, each with `direction`). `position_refs` additionally lists the line's own
attachable positions (segment midpoints/corners, diagram waypoints). Missing end position → line undefined.

### 5.2 `line_type`
| v | name | path |
|---|---|---|
| 1 | Step | free polyline, 4-connected staircase raster |
| 2 | Automatic | orthogonal, per-segment L orientation solved |
| 3 | **Diagram** (default) | orthogonal, auto-routed from the two ends around the attached shapes |
| 4 | Manual | orthogonal, per-segment `direction` stored and user-editable |
| 5 | Staggered | free polyline, 8-connected raster |

Inspector order: Diagram, Automatic, Manual | Step, Staggered.

### 5.3 Routing
* Every orthogonal segment is a straight run or **one L**. `direction` 1 = horizontal first (corner at
  `(end.x, start.y)`), 2 = vertical first (corner at `(start.x, end.y)`).
* **Manual:** use stored directions.
* **Automatic:** depth-first over all 1/2 assignments. A choice is "usable" unless it leaves a
  waypoint back along the leg it arrived on; dead-end assignments become fallbacks. Rank solutions by
  (a) more ends whose used exits lie in their `dirset`, (b) fewer leg-rectangle intersections, (c) more interior
  waypoints continuing on the same axis; first best wins, else first fallback. (Tie-break direction of each
  key is **[inferred]**.)
* **Diagram** (the default router, implemented in `crates/core/src/route.rs`; verified 128/128 against
  synthetic GUI captures, `fixtures/gui/01-diagram-router`). Only the first and last ends matter, each with a point, its owner shape's rect
  (empty when free) and an **ordered** exit list. Direction2D: 1 up, 2 down, 3 left, 4 right; leg orientation
  1 = horizontal first, 2 = vertical first.
  0. Equal points → one point. Missing exit sets: both missing → each end gets the dominant direction toward
     the other (|dx| ≥ |dy| → horizontal); one missing → the first of the other end's exits (order left, right,
     up, down) in which this end lies strictly ahead, reversed. Masks expand to lists in the order up, down,
     left, right.
  1. Sanitise: an end on its own rect's border exits through the first of left/right/up/down that is both a
     border side and an allowed exit; that side becomes the list's head and the rect loses that row/column. An
     end inside its rect (not on a usable border side) drops the rect.
  2. Single L: orientation 1 then 2, accepted if its first leg leaves through a start exit and the reverse L
     from the end leaves through an end exit.
  3. Else for each start exit a × end exit b (list order), build a candidate (d0,d1,d2, waypoints A, B):
     * **a = b** (U route). Vertical: same row → A = start + 2a, legs (1,1). Otherwise F = the end farther
       along a, P1 = F + 2a, M = (other.x, upper.y + |dy|/2) (x − 2 if both share x). If either leg
       other→M, M→P1 touches F's rect and the other end sits > 1 row beyond that rect's near edge (and F is
       outside it), M.y moves halfway between the other end and the rect; if M.x then lies over the rect it
       jumps to 2 cells past the nearer side (avoiding the end's own column). If the end lies ahead of the
       start along a: (2,2,2) A = M, B = P1; else (1,1,1) A = P1, B = M. Horizontal: transpose (orientations
       swapped: reachable → 1, else 2; same column → A = start + 2a, legs (2,2)).
     * **Opposite horizontal** (Z/S): if the left-exiting end is ≥ 2 columns right of the right-exiting one →
       one middle leg at x = halfway (A = (midX, midY), legs (1,2); same row → straight). Else an S: stubs 2
       cells out, middle row halfway between the upper shape's bottom row and the lower shape's top; if that
       gap is < 2 or the middle leg hits either rect, go around above/below the union of both rects
       (2 cells clear, whichever side is nearer) and push each stub out of the *other* end's rect. Legs (1,1,2).
     * **Opposite vertical**: transpose — if the down-exiting end is > 1 row above the up-exiting one,
       A = (midX, midY) legs (2,1); else the S around left/right, legs (2,2,1).
     * **Perpendicular**: if each end lies strictly ahead of the other along its exit → one L (orientation 1
       if a is horizontal). Else a corner P = start + offset(a) on a's axis and end + offset(b) on b's, where
       offset = max(2, half the free distance toward the other point, stopping a cell before the other end's
       rect); P is pushed out of the end rect (start leg) then the start rect (end leg); if both legs would
       leave P the same way, P moves 1 cell along whichever exit is perpendicular to them. Legs (o, o).
     Score a candidate with A by the first leg start→A and the reversed leg end→A: same direction 2, either
     missing or perpendicular 1, opposite 0. First 0 wins; a candidate with no waypoint wins unless a 0
     already exists; otherwise lowest score, first found.
  4. No candidate → retry with the end's exits complemented, then the start's, then both; final fallback
     one L with orientation 1.
  *Inferred (which rect each test uses, and the retry order), confirmed only by the 128 captures:*
  the U-route touch test uses F's rect, the offset uses the other end's rect, the S middle leg is tested against
  both rects.
  **Observed details** (`fixtures/gui/01-diagram-router`, 128/128; these pin down rounding and tests the prose above leaves open):
  * Halfway points use the lower coordinate plus half the absolute distance, truncated:
    `min(a,b) + |a−b|/2`. This holds for M in the U route and for the Z/S midpoint (diagTL-AT-BB: ends on rows 12
    and 5 → middle row 8, not 9).
  * U-route touch test: both legs (other end → M, M → P1) are drawn axis-of-a first (vertical first for an up/down
    U). When it fires, M moves to halfway between the other end and the first free row/column outside F's rect:
    `n + (other − n)/2` with `n` = rect edge ± 1 (overlapY-AL-BL → column 20, overlapY-AR-BR → column 17).
  * S go-around: "nearer side" compares the distances of the two sides from the ends' mean coordinate
    (overlapX-AT-BB: ends at x 5 and 10, rect union x 0…14 → goes right at x 16).
  * Perpendicular: P is pushed when the *leg* start→P (drawn with the route's orientation) touches the end rect: P's
    coordinate on a's axis jumps to 2 cells past the far side of that rect along a (overlapX-AR-BB: x 12 → 16).
    The end leg P→end against the start rect is pushed the same way along b **[inferred, not in the captures]**.
  * Candidate score directions are the first step of the leg start→A and the first step from the end back along
    the last leg (each L drawn with its orientation).
  * In the 128 captures every end has a single exit, so the exit-pair loop, the score and the complement retries are
    exercised only by the CLI round-trip test documents.
  The result is ≤ 3 segments with waypoints A, B. `LineDiagramPosition` objects (`line_dia_seg` 0 = middle,
  1 = first, 2 = last; `line_dia_pos` 1 centre / 2 waypoint / 3 leg midpoint / 4 corner) expose those points
  as attachable positions.

### 5.4 Rasterization
* Orthogonal L: horizontal run along the start row up to the corner column, then vertical to the end (mirrored
  for direction 2). Every segment except the last omits its end point so shared corners are drawn once.
* Step: exact 45° → diagonal; axis-aligned → straight; otherwise a 4-connected staircase with evenly spread
  steps — M major, m minor delta, `q,r = divmod(M−m+1, m+1)`; at major offset `(k+1)q + k + min(k+1,r)`
  (k = 0…m−1) emit the cell plus one extra cell shifted on the minor axis (M+m+1 cells).
* Staggered: ⌈m/2⌉ diagonal moves, then M−m straight moves, then ⌊m/2⌋ diagonal moves (M+1 cells).
* Each cell gets a segment bit toward its previous and its next cell (8-neighbour deltas → bits 0–7 as in §2);
  an isolated cell gets left+right. Glyphs then come from §3.
* Orthogonal corner lists are simplified before rasterizing: repeated points and interior points that lie between
  their neighbours on one axis are dropped (a point where the path doubles back is kept). Consecutive duplicate
  cells are dropped before the segment bits are assigned.

### 5.5 Style and dash
`line_style`: 1 Opaque (cells become spaces), 2 Transparent (body not drawn),
**3 Single Light** (default), 4 Single Heavy, 5 Double; 6/7 legacy dashed draw nothing.
Dash `{p,u,o}`: active only if `line_dash_enabled` and p>0 and u>0. A counter i runs over every
cell of the whole line; period = p+u; phase′ = o if o ≤ 0 else (o mod period) − period; cell painted iff
`((i − phase′) mod period) < p`. Unpainted cells are empty.

### 5.6 Attachment
* `rect_position` = keypad positions of §4 on the target shape's frame; the sample's 12/15/16/19 are the
  four "outside mid-side" points.
* `position_flags` bit 0 = anchor; other bits are fixed per position kind (StaticPosition 0x2C,
  LinePointIndex 0x15, RectangularContent 0x15 on border / 0x35 interior…) — writers should emit these constants.
* `line_seg_pos_type` (attach to another line's segment): 1 midpoint of first leg, 2 corner, 3 midpoint of second
  leg, 4 middle of the rasterized range (free lines).
* `LinePointIndexPosition`: `line_pt_idx` if valid, else `round(line_pt_rel·(count−1))`.
* `RectangularContentPosition` (type 17), owned by the shape listing it in `position_refs`: without `on_border`,
  `content_point` is a cell of the owner's content grid (content origin + point, §1.7). With `on_border` it is
  **(border part, offset)**, the `border_tixels` encoding of §1.7, not a cell: `"0,34"` on a 97×9 rect with
  `frame_corner` 1 and `content_offset` `-13,-15` is the top border at x = frame.x − 13 + 34. Exit direction
  (when `dirset` is 0) is the side's outward one. Verified by round-trip test documents with line
  ends on all four sides (parts 0–3). Top/bottom offsets count from the content origin's x, left/right from its y
  (content origin = keypad `frame_corner` point + `content_offset`). Corner and outer-ring parts (4–15) are unobserved;
  they follow border_tixels by analogy.
* Shapes with `attachable=false` don't accept attachments.

### 5.7 Heads
Heads occupy the end cell, need ≥ 2 cells in the terminal segment, and point from the previous cell toward the
end. Glyphs (Unicode / ASCII; arrows listed right, left, down, up):

| head_type | Unicode | ASCII |
|---|---|---|
| 0 | none | none |
| 1 smart filled arrow | ▶◀▼▲ | > < v ^ (document alphabet, not export override: §8.4) |
| 2 smart outline arrow | ▷◁▽△ | > < v ^ |
| 3/4/5 | ■ □ ▣ | # |
| 6/7/8 | ◆ ◇ ◈ | * |
| 9–13 | ● ○ ◌ ◎ ◍ | . |
| 14–19 ER: One, Many, One Only, Zero or One, One or Many, Zero or Many | crow's-foot drawing | crow's-foot drawing |
| 20–23 fixed filled | ▲ ▼ ◀ ▶ | ^ v < > |
| 24–27 fixed outline | △ ▽ ◁ ▷ | ^ v < > |

Smart arrows (1, 2) re-aim at the side of the attached shape they touch. `line_head_start_tixel` /
`line_head_end_tixel` override with a custom character. `head_notation` (1 Orthogonal, **2 Diagonal** default,
3 Hat) only changes the "many" fork; One / One Only / Zero or One look identical in all three
(`fixtures/gui/03-er-crowsfoot`).
ER layout, k = cells back from the tip (bar `┼`/`|`/`-`, circle `○`/`o`, line glyph elsewhere):

| head | marks | fork at the tip |
|---|---|---|
| 14 One | bar k=0 | – |
| 16 One Only | bars k=0,1 | – |
| 17 Zero or One | bar k=0, circle k=1 | – |
| 15 Many | – | yes |
| 18 One or Many | bar k=1 (Diagonal on a horizontal end: k=2) | yes |
| 19 Zero or Many | circle k=1 | yes |

Fork per notation: **Diagonal** puts `╱`/`╲` in the two cells beside the tip, spreading toward it (rightward
end: `╱` above, `╲` below; downward end: `╱│╲`; leftward and upward mirror these), the tip keeps the
line glyph. **Orthogonal** draws `┼` on the tip with corner arms opening away from the line (rightward: `┌` above,
`└` below; leftward `┐`/`┘`; downward `┌┼┐`; upward `└┼┘`). **Hat** replaces the tip with a filled triangle
pointing back along the line (rightward end → `◀`), no arms. Bent lines use the last segment's direction.

## 6. Shapes

### 6.1 Rectangle family (type 20; shape_type 1 Rectangle, 4 Text Field, 5 Surface)
**Frame**: size `frame_size`; the keypad point `frame_corner` of the frame is placed at
(anchor point of `frame_anchor_ref` + `frame_offset`). `frame_corner` is an anchoring choice, not a visual style.

**Border** `rectangular_border`, drawn only if
`rectangular_border_enabled`, w ≥ 2, h ≥ 2:

| v | name | cells |
|---|---|---|
| 2 | Transparent | nothing drawn, but the ring still insets text/background |
| 3 | Opaque | spaces |
| 4 | Single Light (default) | light segments |
| 5 | Single Heavy | heavy segments |
| 6 | Double | double segments |
| 1, 7, 8 | None / legacy Dashed | nothing |

Corners get the two inward segments (TL D+R, TR D+L, BL U+R, BR U+L); sides get L+R or U+D. Glyphs come from §3,
so a line merged into a border produces tees automatically. Each of `border_tixel_{tl,tr,bl,br,ts,bs,ls,rs}`,
when non-empty, replaces that part with a literal character (this is how ╭╮╰╯ rounded corners are made; there
is no built-in rounded style). When the background is enabled, side cells additionally carry a hidden segment
pointing inward. Dashing uses the §5.5 rule over a clockwise perimeter index starting at TL = 0
(remember the swapped key names, §1.6). Border color: `mps_border_color`.

**Background** (drawn if `background_enabled`, inside the border ring): `background_type` 1 None, **2 Opaque
(spaces, default)**, 3 Character (`background_tixel`), 4 Block (`█` / ASCII `@`); 5–10 (stripes/crosshatch) are
legacy names that draw nothing. Color `mps_bckg_color`.

**Shadow** (`shadow_enabled`): the whole frame shifted by `shadow_offset`, drawn first (under the shape) in
`shadow_color`. The shape is composed over its own shadow before it lands on the canvas, so the shadow never
reaches a cell the shape itself writes: there the border merges with what was below the shape (a shadowed
text field on another rect's border gives `░┤`), while dash gaps and transparent parts still show
the shadow; `shadow_tixel` overrides, else by `shadow_intensity`: 1 `░`/`.`, 2 `▒`/`:`, 3 `▓`/ASCII
direction-aware `|` (dx=0), `-` (dy=0), `\` (dx,dy same sign) or `/`, **4 `█`/`*`** (default).
The glyph set follows the **document's** alphabet, not the export's (like arrowheads): a Unicode document
exported as ASCII keeps `█` (verified by round-trip test).

**Text field** (shape_type 4): a rectangle created with border 4, text "Text", centred, min 6×3; it does not
auto-size; its name follows its text unless renamed.

### 6.2 Ellipse (type 28)
Hit-cell generation (outline sampling, trimming and the minimum-size fix-up described below):
* **h = 3 or w = 3** (checked in that order): fixed shape, no sampling. h = 3: (0,1), (w−1,1) and rows 0 and 2
  for x = 1…w−2; w = 3 is the transpose.
* Otherwise: graphics rect of the w×h cells in **y-up** space (cell size = canvas font cell); a = (Wpx − cellW)/2,
  b = (Hpx − cellH)/2 (need ≥ 1); 5,760 samples t = i·2π/5760 of the **polar** form r = ab/√((b cos t)² +
  (a sin t)²), point = centre + r(cos t, sin t); cell = (⌊x/cellW⌋, ⌊−y/cellH⌋).
* Trim, two passes over horizontal runs (rows top→bottom, x left→right, each pass seeing earlier removals):
  pass 1 for run (y, x0, x1): drop (x0,y+1) if (x0−1,y+1) is hit; drop (x1,y+1) if (x1+1,y+1) is hit. Pass 2:
  drop (x0,y) if (x0+1,y) and (x0,y±1) are hit; same for (x1,y) with (x1−1,y) — checked below first, then above.
  A row's runs are taken when the pass reaches that row (`fixtures/gui/02-ellipse-thinning` confirms this reading). Frames under 3 cells
  in either direction: every cell is outline **[implementation choice]**.

Glyphs per run (draw block 3, later writes win; A(x)/B(x) = hit at (x, y∓1)):
* single cell, first match: A(x+1)∧B(x−1) `╱`; A(x−1)∧B(x+1) `╲`; A(x)∧B(x) `│` (hidden side segment toward
  the inside); A(x)∧B(x−1) or A(x+1)∧B(x) `;`; A(x)∧B(x+1) or A(x−1)∧B(x) `:`; A(x+1)∧B(x+1) `(`;
  A(x−1)∧B(x−1) `)`; else fall through to the run rules (so a lone bottom cap is `'`, a lone top cap `.`).
* run rules use only the diagonal cells beyond the ends: rising = A(x1+1)∧B(x0−1), falling = A(x0−1)∧B(x1+1).
  2 cells: rising `,'`, falling `` `. ``; 3 cells: rising `,─'`, falling `'─.`; otherwise rising `_.─…─'`,
  falling `` `─…─. ``, B(x0−1)∧B(x1+1) `.─…─.`, A(x0−1)∧A(x1+1) `` `─…─' ``, else all `.`.
* `─` carries a hidden segment pointing inward (down in the top half, y ≤ h/2) so it never merges with lines.
* `─`, `│`, `╱`, `╲` are line tixels (L+R, U+D, UR+DL, UL+DR, light), so the alphabet picks the glyph
  (`-`, `|`, `/`, `\` in ASCII); the other glyphs are plain characters **[implementation choice: the GUI
  captures are Unicode only]**. `│` carries its hidden segment toward the centre column.
* `border_tixel`, when set, replaces every outline cell with that literal character (`fixtures/gui/02-ellipse-thinning`,
  the `star` case: the same cells as the default glyphs).

**Cell size.** The sampling uses a 13 pt row (Menlo 11 line height, `fixtures/gui/04-cell-metrics`) and a fitted column width of
**6.625 pt**. Observed on the 324 GUI sizes (h, w = 3…20, `fixtures/gui/02-ellipse-thinning`): with the sampling above, any width of about
6.6241…6.6266 reproduces 323 of 324 outlines; Menlo 11's own advance (≈6.6226, also 6.623) gives 322 (13×17 fails).
The one size no width fixes without breaking another is **13×16**: Monodraw shows cells (5,11) and (10,11) (so the
bottom rows read `` `.    ,' `` / `` `──' ``), because the outline passes a few thousandths of a cell above the corner of
cell (5,12) and Monodraw's samples land in (5,11), ours step over it. Computing in canvas coordinates, pixel-rounding
the frame, other sample counts and the parametric form were tried: none matches all 324.

**Shape (observed / inferred).** Content blend mode as §4.1 (merge only when blended and the border is on). Draw
order shadow → background → outline → text. Shadow = outline and interior cells shifted by `shadow_offset`, glyph as
§6.1 from the **document's** alphabet (as for rectangles). Background (§6.1 `background_type`) = the cells of each
row strictly between its leftmost and rightmost hit cell that are not hits **[inferred: no captured sample with a
visible fill]**.

**Text rect** (`model_text_ref`, `text_container_control` c ∈ 0…1, default 0.25; 0 = Horizontal = wide/short,
1 = Vertical = tall/narrow) **[inferred: no captured sample contains text in an ellipse]**: in the pixel space above, the
rect inscribed in the sampling ellipse with its corner at angle θ = c·π/2, i.e. half-sizes a·cos θ and b·sin θ, each
at least half a cell. It takes the columns and rows whose cell centres lie inside it, then is clipped so it never
covers the outline: for every row used, the columns stay strictly inside the gap between the hits around the
centre column; if a row has no such gap or the column range becomes empty, the top and bottom rows are dropped
together until it fits (none left = no text). `inset_*` then apply and the text is laid out as §7 (alignment,
position, wrapping, FIGlet). For a 20×9 ellipse at 0.25 this gives the 16×2 block on rows 3–4.

### 6.3 Diamond (type 21)
Centre + radius r; points at (0,±r), (±r,0), 45° edges of r−1 cells. Defaults (Unicode / ASCII):
top `Λ`/`X`, bottom `V`/`V`, left `▕`/`|`, right `▏`/`|`, edges UL `╱`/`/`, UR `╲`/`\`, LL `╲`/`\`, LR `╱`/`/`;
`bt_*`/`btd_*` override. Plain characters (no merging). Anchor `anchor_pos` 1 centre, 2/4/6/8 top/bottom/left/right
point, 3/5/7/9 the same one cell outside.

### 6.4 Triangle (type 7)
`triangle` = `{type, origin_x, origin_y, length}` (length floored to even, sign kept). Types 0/1: base row
`*-…-*` with `/`,`\` diagonals rising to the apex above (0) or below (1); types 2/3: base column of `|` with
diagonals to the left/right. ASCII characters only.

### 6.5 Groups, canvas, images, tables
Groups (type 3) only order and transform children (`subshape_selection` = inspector behavior). Canvas (type 1)
is the root of each page (`canvas_refs`); export uses the active canvas. **Table (26/29)**: an unfinished prototype in Monodraw — the
`table-shape` feature flag is hard-wired off, so users can't create one, but files may contain them. Type 26 =
rect-positioned frame shape, `shape_type 11`, `rows`/`cols` = integer ratio arrays (default 9×9 of 1), `cells` =
dict `"(row<<32)|col"` → TableCell ref (sparse). Type 29 = `cell_index` (same key string) + `text_ref` (type 27).
Per axis interior = size − (N+1) split by ratio (greedy one-unit distribution); cell rect = interior + 2 so
borders are shared; text in the cell inset by 1; borders drawn per cell, blend 0 → junctions. Merges are
view-only, never saved. Border-style/edge-flag meaning and merge rules are unknown. Implementation choices for
the unknowns (key set, all-zero ratios, no elision, no merges) are implemented in `crates/core/src` and noted
where relevant above.

**Image overlay (25)**: shape base + `shape_type 10`, `img_data` = base64 of the
image file bytes, `img_bounds` = `"x,y,w,h"` integer cell rect (relative to the shape's translation),
`img_opacity` double (insert default 0.5). Draws **no tixels** (never in text/PNG/SVG export); shown stretched
over its bounds at its opacity. Hit only when not `cthrough` (Clickthrough). Free resize. Insert: size =
image points ÷ cell size at 100%, rounded; centred in the visible canvas; top of the z-order.

## 7. Text in shapes (type 27)

* Text rect = frame, inset by 1 if border enabled **and** `inside_border`, minus `inset_*`; empty → no text.
* `sweep` 0 horizontal (rows) / 1 vertical (each line is a column); `movement` 0 down/right, 1 up/left
  (reverses paragraphs and lines); `alignment` 0 left(top) / **1 centre** / 2 right(bottom);
  `position` (block placement across lines) 0 top(left) / **1 centre** / 2 bottom(right): offset 0, ⌊(E−L)/2⌋
  or E−L when the L lines are fewer than the available E rows; overflow is clipped **[inferred]**.
* Typesetting: **every character is one cell** (no East-Asian/emoji double width). Tab = advance to the next
  multiple of 4. Word wrap: break before a word if the line is full, or non-empty and the word doesn't fit;
  words longer than the line are hard-split; spaces never force a break (they clamp to the line end).
  Centre shifts by ⌊(len − last non-space end)/2⌋; right aligns the last non-space glyph to the end.
  Spaces between words are **drawn** (opaque: they hide a line or tixel underneath); leading and trailing
  spaces are not **[verified against a test document]**.
* Text color `color`.

### 7.1 FIGlet letter fonts
When `letter_font_id` is set the text is rendered through an embedded FIGlet library instead of the typesetter:
plain `figlet` invocation (no `-k/-W/-S`, i.e. each `.flf`'s own default layout), justification = `alignment`,
output width = text-rect width, vertical placement by `position`. Placement is of the *inked* block, not the raw
FIGlet cells **[verified against several test documents]**: each FIGlet output line (all its rows move
together) is aligned on its width up to the last non-blank column (trailing blank columns ignored, leading ones kept);
centre pad = ⌈(W − ink)/2⌉ (rounds **up**, unlike plain text), right pad = W − ink. Blank rows at the top and bottom
of the whole block (a glyph's empty descender row, the empty line after a trailing `\n`) are dropped, then the block
is placed by `position` as for plain text: ⌊(H − rows)/2⌋ (rounds **down**). Left/top alignment of blocks with
leading blank columns is not covered by the verified set **[inferred]**. Fonts come from `fonts.mflist` manifests
(JSON `{version:1, fonts:[{id,name,type:1,path}]}`) in `Resources/figlet-fonts` (15) and
`figlet-fonts-contrib` (133), plus user `*.mflist` files. Ids: `com.helftone.monodraw.figlet-font.<base>` and
`…figlet-font.contrib.<base>` → `<base>.flf`. Unknown ids render nothing. The `.flf` files themselves are the
standard FIGlet distribution — use an existing FIGlet library. Full id/name list is in this section and the
font manifests above.

## 8. Export, clipboard, CLI

### 8.1 Plain text
Options: alphabet (document's), trim padding, trim trailing whitespace, comment style + padding, watermark,
HTML/code wrap. Comment styles (all overwrite cells; p = padding): 0 None, 1 `//`, 5 `///`, 9 `#`,
2 Header `/*…*/`, 3 Header `/**…*/`, 7 Vertical `/* * */`, 8 Vertical `/* ** */`, 6 XML/HTML `<!-- -->`,
4 Box (`*` border, `/\` corners), 13 Box (Markdeep, all `*`), 10 Markdown tabs, 11 Markdown 4 spaces,
12 Markdown ```` ``` ```` fences. Watermark "Created with Monodraw" adds a blank row + text row (right-aligned,
width ≥ 21, for PNG/SVG). Wrap (`export.html-wrap.tag`): 0 `<pre>`, 2 `<pre><code>`, 3 Shell (`echo "…";` per line,
bytes outside safe printable ASCII as `\xHH`), 4 Ruby (`puts "…";`, `\u{X}`), 5 Python 3 (`print("…");`,
`\uXXXX`/`\UXXXXXXXX`). Exact geometry per style is covered above.

### 8.2 PNG / SVG
Font: the canvas font pref at its point size, default **Menlo 11 pt** (fallback Monaco 11), 1 pt = 1 px at 1×.
Observed (`fixtures/gui/04-cell-metrics`, Monodraw 1.6.1): **cell = ⌈advance⌉ × ⌈line height⌉** — Menlo 11 → 7×13 px,
Menlo 14 → 9×17; image = cols·cw + 1 by rows·ch + 1 px (× the 1×/2×/3× scale). On Linux, DejaVu Sans Mono (same
metrics as Menlo) with unhinted Pango metrics reproduces both sizes. PNG: scale 1×/2×/3×, optional vertical/
horizontal grid lines, background on/off + color (default white), grid color (white 0.9), text color (black);
cells keep their own colors. SVG: `width/height` header with **no viewBox**, optional `<g id="Grid">` of rects,
**one filled `<path>` per glyph** (`fill="rgb(r, g, b)" fill-opacity="1.000000"`, 6-decimal coordinates; box
drawing is drawn as rectangles, not font glyphs), no `<text>`, no font-family, no background.

SVG file structure observed byte-for-byte (`fixtures/gui/04-cell-metrics`, `rect10x5-Ag-default.svg` / `-menlo14.svg`):
* Line 1 is exactly `<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="71" height="66">` (integer
  pixels, no XML declaration, no DOCTYPE, no `<defs>`/`<g>`/`<style>`). The file ends with `</svg>` and **no**
  trailing newline.
* Then one line per non-blank cell: `<path d="…" fill="rgb(0, 0, 0)" fill-opacity="1.000000" />` (space before
  `/>`; colour as integer `rgb(r, g, b)` with ", " separators). Blank cells and the rectangle's opaque white fill
  emit nothing (no background rect).
* Path data: absolute commands `M x y`, `L x y`, `Q cx cy x y`, `Z`, every token followed by one space (so `d`
  ends in `"Z "`), numbers `%f`-style with 6 decimals. Each glyph's subpaths are in one element (e.g. "A" is
  `M … Z M … Z` — counter then outline). Menlo's outlines are quadratic, hence `Q`.
* Box drawing (Menlo 11, Single/Light border): each piece is a closed axis-aligned polygon starting bottom-left
  and going up: `─`/`│` one rectangle spanning the whole cell plus a small overshoot (│ in a 13 px row covers
  y −0.70…+0.32 beyond it; ─ covers x −0.11…+6.73 of its 7 px cell, leaving 0.16 px seams; lines ≈0.86 px
  (vertical) and ≈0.92 px (horizontal) thick, slightly off-centre);
  corners `┌┐└┘` are a single 6-point L-shaped polygon (the union of their two arms), not two rects. These are
  Menlo's own glyph outlines, which happen to be rectangles.
* Element order is **not** row-major: in the 10×5 capture it runs left column top→bottom, bottom row left→right,
  then "A", parts of the right column, "g", the rest of the right column, the top row, and `┐` last. It looks
  like an unordered (hash/glyph-run) traversal; nothing depends on it.

*Ours:* the same header, path line format, number formatting and trailing `</svg>`; elements in row-major order.
Glyph outlines come from Pango (`layout_path` + `copy_path`, unhinted) and are cubic (`C`) rather than Menlo's
quadratic `Q`. Box drawing uses our own geometry (the same quads the canvas fills): a straight run is one
rectangle edge to edge with half-line-width overshoot, a corner/tee is one rectangle per axis in the same path
(overlapping, same winding, nonzero fill), double lines two parallel rectangles, diagonals a quadrilateral.
Shade blocks `░▒▓█` are one cell-sized rectangle with `fill-opacity` 0.25/0.5/0.75/1. Grid lines (unconfirmed
format) are 1 px `<rect>`s inside `<g id="Grid">` before the glyphs; the footer is glyph paths.

Export sheet defaults (first open): Text — trim trailing whitespace **on**, trim padding **on**, output wrap off
(`<pre><code>`), footer off, comment None / padding 0. PNG — Normal (1×), trim padding **off** (keeps the canvas
origin), grid lines off, footer off, background on (white). SVG — trim padding **on**, grid lines off, footer
off. The sheet remembers the last tab; an empty document doesn't open it.

### 8.3 Clipboard
Copy writes plain text (rendered with the pasteboard comment prefs, trim trailing only, no watermark) plus the
selected shapes as a JSON persistence graph (root shape list, root shapes in canvas coordinates with
`canvas_origin=true`). Style copy uses a separate private type. Pasting plain text into the surface editor turns
it into a tixmap at the cursor. A Linux port can use a private MIME type with the same JSON.

### 8.4 CLI (offline strings of the bundled `monodraw` tool)
`monodraw [--unicode] [--trim-whitespace] [--skip-newline] [--json] file.monopic` → prints the rendered canvas
to stdout (+ newline unless `--skip-newline`; `--json` → `{"output": "…"}`). Also `--help`, `--version`,
`--no-<opt>`. Worth reproducing as-is: it gives a scriptable regression oracle.

Observed against real Monodraw 1.6.1 (black-box comparison exports):
* Padding is **always trimmed**: the canvas-origin margin never appears, but blank cells inside the shapes' extent
  (empty shapes included) do.
* An empty drawing prints the literal `(null)` with **no** trailing newline.
* Without `--unicode` lines and borders are ASCII, but **arrowheads keep the document's alphabet** (head type 1
  stays ▶◀▼▲ in a Unicode document). Only Unicode-alphabet documents with heads were available, so whether an
  ASCII-alphabet document gets `> < v ^` (table in §5.7) is unconfirmed.

## 9. Known gaps / approximations

| gap | impact | suggested approach |
|---|---|---|
| Ellipse float ties (13×16, 13×17) | one cell on two sizes | exact Menlo cell width + Apple libm; low value |
| SVG structure (cairo symbols + viewBox vs per-glyph paths) | cosmetic | own path writer if anyone needs byte-level SVG |
| Outline animation, edit-mode undo | not captured on the reference install | capture with the GUI toolkit, same approach as the other `fixtures/gui` steps |
| Table edge flags / merge rules | only matters for merged cells, which Monodraw never saves | not needed unless merges get persisted |

The ellipse rasterizer and glyph rules (`crates/core/src/ellipse.rs`) and the Diagram router
(`crates/core/src/route.rs`) are implemented from §5.3, §6.2 and the measurements in this document, and
re-verified against the recorded GUI behavior: router 128/128, ER 18/18, ellipse 105/107 (13×16 only).

The bundled CLI cannot run on Linux, but a licensed install (if available) can produce `.txt` reference
outputs for a set of test `.monopic` files. Those, plus `tools/monopic.py` round-trips, are the recommended
conformance suite.

## Appendix: feature checklist (inspector panels)
Rectangular border (enable, type, color, blend content, 8 part characters) · line dash (painted/unpainted/phase)
· background (enable, type, character, color) · shadow (enable, intensity, offset X/Y, character, color) · text
(enable, color, alignment, position, sweep, movement, inside border, 4 insets, letter font + preview) · ellipse
border + text-container slider · diamond border (8 parts) + anchor · rectangular position (anchor corner, X/Y),
geometry (W/H), attachment (attachable) · line (type, style, heads, notation, segment direction for Manual) ·
alignment/distribute · color · document (alphabet) · group · image · locked · character palette (box drawing,
`char-palette.{ascii,block-elements,geometric-shapes}.json`) · snippets library · FIGlet font picker.
