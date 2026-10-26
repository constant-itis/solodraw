# Monodraw UI inventory (target for GTK parity)

Description of the Monodraw 1.7.1 user interface: layout, controls, labels and behaviour only,
observed for interoperability. No code or artwork is copied; icons are drawn fresh for this project
(icon names are listed only so we know which icons exist).

Sources: Monodraw's help documentation (14 pages), observation of the menu bar and document window layout,
every inspector panel, UI label strings, and the palette and snippet data files. Many popup menus are
filled at runtime, so their option lists come from observed label strings and `SPEC.md`. Those are marked
*(runtime)*.

Linux key mapping: **Cmd → Ctrl** and **Alt → Alt**. Mac-only chords are remapped in the "Linux" column, and
remaps that are decisions rather than mechanical translations are marked †.

---

## 1. Document window

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ Toolbar (customisable, icon + label)                                         │
├───────────────┬─────────────────────────────────────────┬────────────────────┤
│ Sidebar 222px │ Canvas (scrolling, infinite, origin TL) │ Assistant 246px    │
│ ┌───────────┐ │  rulers: line numbers (left),           │ [Inspector|Chars|  │
│ │utility bar│ │          column numbers (top)           │  Snippets] tabs    │
│ │ 30px high │ │  grid: vertical / horizontal lines      │                    │
│ ├───────────┤ │  page guide at column N                 │ stacked panels for │
│ │shape tree │ │                                         │ the current        │
│ │(outline)  │ │                                         │ selection          │
│ └───────────┘ │                                         │                    │
└───────────────┴─────────────────────────────────────────┴────────────────────┘
default window 1000×600; split = sidebar | canvas (530) | assistant
```

* Three-pane split view. Both side panes can be collapsed (View ▸ Show/Hide Sidebar ⇧⌘[, Show/Hide Assistant
  ⇧⌘]; the toolbar has toggle buttons for each). Collapsing is not animated (the "Animate sidebar/assistant" prefs animate shape-tree rows and assistant tab switches instead).
  Divider positions are saved per document.
* Saved per document: zoom, scroll point, selected and expanded sidebar items, line numbers, column numbers,
  and vertical and horizontal grid.
* Document tabs or pages: the model supports several canvases (`canvas_refs`), but the visible UI has no page
  switcher in 1.7.1. *Not in scope for phase 1.*

### 1.1 Toolbar items (identifier → label / tooltip / key)

| # | Label | Tooltip | Key | Kind |
|---|---|---|---|---|
| 1 | Select | Select tool (S) | S, Esc | tool |
| 2 | Pan | Pan tool (Space). Tooltip adds: "Press Space again to go back to previous tool." / "Hold Space to go back to previous tool on key release." | Space | tool |
| 3 | Rect | Rectangle tool (R) | R | tool |
| 4 | Table | Table tool (H) | H | tool (feature flag `table-shape`, off by default) |
| 5 | Diamond | Diamond tool (D) | D | tool |
| 6 | Line | Line tool (L) | L | tool |
| 7 | Ellipse | Ellipse tool (C) | C | tool |
| 8 | Text | Text tool (T) | T | tool |
| 9 | Surface | Surface tool (F) | F | tool |
| 10 | Pencil | Pencil tool (P) | P | tool |
| 11 | Bucket Fill | Bucket fill tool (B) | B | tool |
| 12 | Eraser | Eraser tool (E) | E | tool |
| 13 | Picker | Character Picker (I) | I | tool (eyedropper for the pencil character) |
| 14 | Group | Group shapes | — | button |
| 15 | Ungroup | Ungroup shapes | — | button |
| 16 | Edit | Edit Shape | — | toggle button (custom-content edit mode) |
| 17 | Image | Image overlay | — | button (insert image) |
| 18 | Backward | — | — | button (Alt: to back) |
| 19 | Forward | — | — | button (Alt: to front) |
| 20 | Export | Export as Text | — | button → export sheet |
| 21 | Zoom | — | — | zoom control showing `%.0f%%` |
| 22 | Chars | Characters | — | button → character picker window |
| 23 | Sidebar | Toggle Sidebar | — | toggle |
| 24 | Assistant | Toggle Assistant | — | toggle |

* Tool buttons are monochrome icons with "active" variants. The selected tool shows a pushed-in state.
* **Sticky tools:** pressing Shift while activating a tool keeps it active after a shape is created.
  Otherwise the tool reverts to Select after one shape.
* Toolbar can be hidden (⌥⌘T) and customised (View ▸ Customize Toolbar…).
* Single-key shortcuts have no modifier and only apply when the canvas has focus (not while typing text).

### 1.2 Sidebar (shape tree)

* Utility bar (30px) has four 20×20 icon buttons, left to right: **Group**, **Ungroup**, **Move Forwards**
  (Alt: to top), **Move Backwards** (Alt: to bottom).
* Outline view of the shape tree: one row per shape (17px), with an icon and a name. Groups expand and
  collapse. The list order is z-order, with the topmost shape first **[inferred]**.
* Each row has inline **lock** and **hide** toggles.
* Hovering a row draws a highlight overlay on that shape in the canvas.
* Double-clicking a row renames the shape (inline edit).
* Drag and drop reorders the rows and reparents shapes into or out of groups.
* Selection syncs both ways with the canvas, and multi-select works.
* Row context menu: Rename, Locate (scrolls the canvas to the shape), Lock/Unlock, Hide/Unhide, Copy Style,
  Paste Style, Duplicate, Delete, Reset Content, Move Forwards/Backwards/to Front/to Back, Group, Ungroup.
* Default names come from the shape type: Rectangle, Text Field, Surface, Ellipse, Diamond, Triangle, Line,
  Group, Image, Canvas. A text field's name follows its text until it is renamed. A duplicate gets `%@ Copy`
  or `%@ 2`.

### 1.3 Canvas

* Infinite grid of character cells with the origin at the top left. Drawn in the preference font
  (default Menlo/Monaco → we use a monospace fallback).
* Rulers: line numbers on the left and column numbers on the top. Each can be toggled.
* Vertical and horizontal grid lines can be toggled per document, with defaults from preferences. Optional
  page guide at column N.
* A character missing from the font draws as **`?` on a red background** (no font substitution).
* Movement alignment guides (View submenu) snap moving shapes to the edges and centres of other shapes.
* Colours (theme tokens): canvas background, line (grid), ruler separator, ruler background, ruler text, text.
* Zoom: ⌘+ / ⌘− / ⌘0, plus pinch and Ctrl+scroll.
* Selection visuals: selection frame and resize handles, line end and midpoint handles, attachment-point
  markers while dragging a line end, and a hover overlay driven from the sidebar.
* Canvas context menu:
  * On a line: **Add Midpoint** and **Delete Midpoint** (on a midpoint), **Add Label**, **Line Directions** ▸
    (Manual lines; per segment: Horizontal-Vertical / Vertical-Horizontal), and **Segment** ▸.
  * On a shape: **Add Attachment Point** and **Delete Attachment Point** (custom attachment points on the
    border or interior), plus the same shape items as the sidebar menu.
* Clipboard: copy puts shapes (internal type) **and** plain text on the clipboard. Plain text uses the
  Clipboard preferences (comment style, padding, trim trailing). Paste Raw pastes text into the canvas as a
  Surface.

### 1.4 Edit mode (custom content)

Shape ▸ Edit (⇧⌘E), or the Edit toolbar toggle, enters editing of the selected shape's custom content
(tixmap):

* The shape's standard content stays visible, and you type or draw characters on top.
* Text-field editor popover has **Raw Edit** and **Done**
  buttons. ⌘Return closes it.
* Edit mode has its own undo stack (preference `undo.levels.tixmap-editor`).
* Pencil, Eraser, Bucket Fill and Picker operate on the selected shape's custom content.

---

## 2. Menu bar

Mac-only items are dropped on Linux: Services, Hide/Hide Others/Show All, Spelling and Grammar, Substitutions,
Speech, Check for Updates, Register/License/Trial, and the Window ▸ Developer submenu. Each is listed once
here so the omission is deliberate.

### Monodraw (→ Linux: app menu / "Help" split)
| Item | Mac | Linux |
|---|---|---|
| About Monodraw | | About dialog |
| Command Line Tool… | | dialog explaining the CLI path (§5.7) |
| Preferences… | ⌘, | Ctrl+, |
| Quit Monodraw | ⌘Q | Ctrl+Q |

### File
| Item | Mac | Linux |
|---|---|---|
| New | ⌘N | Ctrl+N |
| Open… | ⌘O | Ctrl+O |
| Open Recent ▸ (… / Clear Menu) | | same |
| Close | ⌘W | Ctrl+W |
| Save… | ⌘S | Ctrl+S |
| Save As… | ⇧⌘S | Ctrl+Shift+S |
| Revert to Saved | | same |
| Export... | ⌘E | Ctrl+E |
| Import... | ⌘I | Ctrl+I (text file → Surface) |

### Edit
| Item | Mac | Linux |
|---|---|---|
| Undo / Redo | ⌘Z / ⇧⌘Z | Ctrl+Z / Ctrl+Shift+Z (also Ctrl+Y) |
| Cut / Copy / Paste | ⌘X/C/V | Ctrl+X/C/V |
| Paste Raw | | — |
| Paste and Match Style | ⌥⇧⌘V | Ctrl+Alt+Shift+V |
| Duplicate | ⌘D | Ctrl+D |
| Delete | ⌫ | Delete / BackSpace |
| Select All / Deselect All | ⌘A / ⇧⌘D | Ctrl+A / Ctrl+Shift+A † |
| Snippets ▸ Add Snippet | | opens Snippet Creation window |
| Find ▸ Assistant Search | ⌥⌘F | Ctrl+K † (focus the assistant search field) |
| Find ▸ Find… / Find Next / Previous / Use Selection / Jump to Selection | ⌘F … ⌘J | text-editing only, Ctrl+F / Ctrl+J |
| Transformations ▸ Make Upper/Lower Case, Capitalize | | text-editing only |

### Shape
| Item | Mac | Linux |
|---|---|---|
| Move Forwards | ⇧⌘F | Ctrl+Shift+F |
| Move Backwards | ⇧⌘B | Ctrl+Shift+B |
| Move to Front | ⌃⌘F | Ctrl+Alt+F † |
| Move to Back | ⌃⌘B | Ctrl+Alt+B † |
| Group / Ungroup | ⌘G / ⇧⌘G | Ctrl+G / Ctrl+Shift+G |
| Flatten | ⇧⌘\ | Ctrl+Shift+\ |
| Lock (toggle; label flips Lock/Unlock) | ⌘L | Ctrl+L |
| Hide (toggle; Hide/Unhide) | ⌃⌘H | Ctrl+Alt+H † |
| Copy Style / Paste Style | | Ctrl+Alt+C / Ctrl+Alt+V † |
| Locate | | — |
| Edit (toggle edit mode) | ⇧⌘E | Ctrl+Shift+E |
| Reset Content | | — |

### View
| Item | Mac | Linux |
|---|---|---|
| Zoom In / Out / to 100% | ⌘+ / ⌘− / ⌘0 | Ctrl+= (+) / Ctrl+- / Ctrl+0 |
| Show/Hide Sidebar | ⇧⌘[ | Ctrl+Shift+[ (also F9) |
| Show/Hide Assistant | ⇧⌘] | Ctrl+Shift+] |
| Show Shape Inspector | ⌘1 | Ctrl+1 |
| Show Character Palette | ⌘2 | Ctrl+2 |
| Show Templates (Snippets tab) | ⌘3 | Ctrl+3 |
| Show/Hide Line Numbers | | toggle |
| Show/Hide Column Numbers | | toggle |
| Show/Hide Vertical Canvas Grid | | toggle |
| Show/Hide Horizontal Canvas Grid | | toggle |
| Movement Alignment Guides ▸ | | submenu, filled at runtime: off / on (pref `canvas.guide.movement`) |
| Show Toolbar | ⌥⌘T | Ctrl+Alt+T would clash with GNOME Terminal → no shortcut † |
| Customize Toolbar… | | phase 6 (optional) |

Toggle items flip their label ("Show X" ↔ "Hide X").

### Window
| Item | Mac | Linux |
|---|---|---|
| Welcome to Monodraw | ⇧⌘1 | Ctrl+Shift+1 |
| Character Picker | ⇧⌘2 | Ctrl+Shift+2 |
| Minimize / Zoom / Bring All to Front | ⌘M | window-manager; drop |

### Extras
Reveal Fonts Folder…, Reveal Characters Folder…, Reveal User Snippets Folder…, Reveal Logs Folder… (each opens
the folder in the file manager, under `~/.local/share/monodraw/{fonts,char-palettes,snippets,logs}`).

### Help
Monodraw Help (⌘? → F1), Open Intro Document… (opens the bundled intro `.monopic`, which we would replace
with our own intro document).

Notes: the 2015 help pages give ⌘E for Shape ▸ Edit. In 1.7.1 that key is File ▸ Export and Edit is ⇧⌘E.
Follow 1.7.1. Assistant Search and Find and Replace both claim ⌥⌘F in the menu definition, so Assistant
Search gets its own key on Linux.

---

## 3. Tools (behaviour)

| Tool | Behaviour |
|---|---|
| **Select** | Click selects the topmost hit and Shift-click adds or removes shapes. Dragging on empty canvas draws a marquee. Dragging a shape moves it, and arrow keys nudge it 1 cell. Handles resize rects, ellipses and diamonds and move line ends and midpoints. Double-clicking a text-bearing shape edits its text. Esc from any tool returns to Select. A group selects as a whole unless "Allows sub-shape selection" is on. |
| **Pan** | Drag scrolls the canvas. Space toggles it (press: active until the next Space); holding Space pans temporarily and restores the previous tool on release. |
| **Rect / Text / Surface** | All three create the rectangular shape type with different defaults. Rect has a Single Light border. Text is a text field with "Text" centred, a border and a minimum of 6×3. Surface has no border, used for free custom content and fill. Drag sets the size; click creates a default size. |
| **Diamond** | Symmetric diamond from centre and radius (drag sets the radius). |
| **Ellipse** | Ellipse bounded by the dragged rect. |
| **Line** | Drag from point to point. Starting or ending over a shape attaches to the nearest attachment point (keypad positions and custom points, with markers shown while hovering). The default type is Diagram. While drawing, clicks add midpoints for Automatic, Manual and Step lines (not Staggered). Editing ends, midpoints and labels happens in Select. |
| **Table** | Feature-flagged grid shape (merge/unmerge cells). *Low priority.* |
| **Pencil** | Paints the current character (from the Characters tab or the Picker) into the custom content of the shape under the cursor. Right-drag erases. Switches the assistant to the Characters tab when the preference is on. |
| **Bucket Fill** | Fills a connected area of cells within the shape with the current character. |
| **Eraser** | Erases custom content under the cursor. |
| **Picker (I)** | Click a cell to set the pencil character. |
| **Image** (button) | Inserts an image overlay for tracing, with opacity and click-through. Not exported as text. |

A tool that has no target shape under the cursor (pencil, fill, eraser) uses the selected shape, or creates a
Surface for pencil.

---

## 4. Assistant (right pane)

A tab bar at the top has **Inspector** (⌘1), **Characters** (⌘2) and **Snippets** (⌘3), plus a search field
(Assistant Search). The Inspector is a vertical stack of collapsible sections chosen by selection type.
A mixed multi-selection shows only the shared sections, and differing values show "Multiple Values".

### 4.1 Which sections show

| Selection | Sections (top → bottom) |
|---|---|
| nothing | **Document** (Alphabet: Unicode / ASCII); "No Selection" |
| locked shape(s) | "The current selection is locked." + **Unlock** button |
| Rect / Text / Surface | Alignment*, Position, Geometry, Attachment, Border, Line Dash (border dash), Background (Fill), Shadow, Text, Content Color |
| Ellipse | Alignment*, Position, Geometry, Attachment, Ellipse Border, Ellipse Text Container, Background, Shadow, Text, Content Color |
| Diamond | Alignment*, Diamond Anchor, Diamond Border, Shadow, Text, Content Color |
| Line | Line, Line Dash, Shadow, Content Color |
| Group | Alignment*, Group, Shadow |
| Image | Image |

\* Alignment only when 2+ shapes are selected.

### 4.2 Sections and controls

**Document**: "Alphabet:" popup with Unicode and ASCII. Standard content only; custom characters are kept.

**Alignment**: 8 icon buttons: align left / centre-X / right, align top / centre-Y / bottom, distribute
horizontally, distribute vertically.

**Rectangular Position** ("Anchor Position"): a 3×3 anchor-corner picker (keypad positions, custom control
drawn with dots), **X:** and **Y:** steppers, and **Offset X / Y** used when the anchor is attached to a line.

**Rectangular Geometry**: **Width:** and **Height:** steppers.

**Rectangular Attachment** ("Attachment Points"): **Attachable** checkbox and a small diagram showing the
attachment points.

**Rectangular Border**: **Border** enable checkbox. **Type:** popup *(runtime)*: Transparent, Opaque,
Single (Light), Single (Heavy), Double, Dashed (Light), Dashed (Heavy). **Color:** well. **Blended**
checkbox (junction blending). Eight part-character pickers laid out around a box: TL, T, TR, L, R, BL, B, BR.
An empty part means the default glyph.

**Line Dash** ("Dash Pattern" checkbox): **Segment:**, **Gap:** and **Phase:** steppers. Used for lines and
rect borders.

**Rectangular Background**: **Fill** enable. **Type:** popup *(runtime)*: Opaque, Character, Block (legacy:
Crosshatch Diagonal/Orthogonal, Stripes Backwards/Forwards/Horizontal/Vertical). **Character** picker when the
type is Character. **Color:** well.

**Shadow**: **Shadow** enable. **Intensity:** popup *(runtime)*: Low, Medium, High, Full. **X:** and **Y:**
offset. **Character:** override picker. **Color:** well.

**Text**: **Text** enable. **Color:** well. **Alignment:** 3-segment (left/centre/right, icons).
**Position:** 3-segment (top/centre/bottom). **Sweep:** 2-segment (horizontal/vertical). **Movement:**
2-segment (down-right / up-left). **Text inside border** checkbox. **Padding:** Top/Bottom/Left/Right
steppers. **Font:** popup starting with None, then the FIGlet font list. **Preview** button shows the font
preview list (a table of font names rendered with the sample text).
Text content is edited in a popover (⌘Return closes).

**Ellipse Border**: **Border** enable, **Blended**, **Color:**, **Character:** override.

**Ellipse Text Container**: slider between **Horizontal** and **Vertical** (0…1, default 0.25).

**Diamond Anchor**: "Anchor Position:" popup *(runtime)*: Center, Top, Top (Outside), Bottom, Bottom
(Outside), Left, Left (Outside), Right, Right (Outside). **Attachable**.

**Diamond Border**: **Border** enable, **Color:**, and 8 part pickers (top, bottom, left, right and the four
edges).

**Line** ("Line"): **Type:** popup: Diagram, Automatic, Manual | Step, Staggered. **Style:** popup:
Transparent, Opaque, Single (Light), Single (Heavy), Double (legacy Dashed hidden). **Start Head:** and
**End Head:** popups *(runtime)*: None, smart and fixed arrows, squares, diamonds, circles, ER heads (One,
Many, One Only, Zero or One, One or Many, Zero or Many); each has a custom-character override. **Swap Heads**
button. **Notation:** popup: Crow's Foot (Diagonal) / (Orthogonal) / (Hat), shown for ER heads.
**Direction** popup (X-Y / Y-X) for the selected segment of a Manual line. **Attachable** and **Blended**
checkboxes. Smart arrows show "%@ (Smart)".

**Content Color**: "Content Color:" well (colour of custom content).

**Group**: "Group" title and **Allows sub-shape selection** checkbox.

**Image**: **Opacity** slider and **Clickthrough** checkbox.

**Colour wells** open a colour chooser. Colours are stored per element (border, background, shadow, text,
content).

### 4.3 Characters tab
* **Pencil:** shows the current pencil character.
* **Palette:** popup with Box Drawing (built in), ASCII (94 chars), Block Elements (32), Geometric Shapes
  (94), plus user palettes from the characters folder. Data format is `{id, name, characters:[{char,
  displayChar?}]}`.
* A grid of character buttons, where a click sets the pencil character.

### 4.4 Snippets tab
* Popup: All Snippets / User Snippets / Standard Snippets (default from preferences).
* A list with a group header per source. Each row shows a preview, a label and a "+" button that adds the
  snippet to the canvas. Drag and drop onto the canvas also works.
* **Add Snippet** (from the selection) and **Manage** (reveal the user snippets folder) buttons.
* The bundled standard snippets are Window (XS/S/M/L/XL) and Phone. Each `.monosnippet` folder holds
  `snippet-metadata.json` and `snippet-content.json`. We ship our own snippets, not theirs.

---

## 5. Secondary windows

### 5.1 Welcome
Borderless window. On the left: app icon, name, "Version x (y)", and three large buttons (**New**,
**Open**, **Intro**) plus social links (drop these). On the right: a recent documents list where each row
shows the name and path (`~/File/path`). When the list is empty it shows "No Recent Documents". A checkbox
at the bottom reads **Show this window when Monodraw launches**. A close button sits in the corner.

### 5.2 Export sheet
Sheet over the document with a format tab switcher (**Text | PNG | SVG**), an options pane per format, and
**Copy**, **Cancel** and **Export** buttons. The last-used format pane is remembered.
* **Text**: Include selected shapes only, Trim whitespace padding, Trim trailing whitespace. **Output
  style:** checkbox plus a wrap popup *(runtime)*: `<pre>`, `<pre><code>`, Shell (UTF-8), Ruby 2,
  Python 3. **Comment Style:** popup *(runtime)*: None, `//`, `///`, `#`, Header `/*`, Header `/**`,
  Vertical `/*`, Vertical `/**`, XML / HTML, Box, Box (Markdeep), Markdown (Tabs / Spaces / Backticks)
  (geometry in SPEC §8.1).
  **Comment Padding:** stepper. Checkbox **"Created with Monodraw" footer** (we rename ours).
* **PNG**: Include selected only, Trim padding, Vertical and Horizontal grid lines. **Resolution:**
  Normal (1x) / Retina (2x) / Retina HD (3x). **Text color**, **Background color** (checkbox + well),
  **Grid color**, **Reset Colors**, footer checkbox.
* **SVG**: Include selected only, Trim padding, Vertical and Horizontal grid lines, **Grid color**,
  **Reset Colors**, footer checkbox.

### 5.3 Preferences (tabbed)
* **Text:** font label + **Change Font**, **Anti-aliasing**, **Subpixel anti-aliasing**, **Default
  alphabet:** Unicode / ASCII.
* **Clipboard:** **Copy with comment style:** popup, **Comment style padding:**, **Trim trailing whitespace**.
* **Canvas:** **Show vertical lines for new documents**, **Show horizontal lines for new documents**,
  **Page guide at column** [N], **Show line numbers for new documents**, **Show line columns for new
  documents**, **Apply selection shape style on create**.
* **Interface:** **Default Snippet List:** (All / User / Standard), **Appearance:** System / Light / Dark,
  **Use dark icon in dark mode**, **Animate sidebar**, **Animate assistant**, **Show characters tab when
  using the pencil**.
* **Update:** drop on Linux.

Preference keys (`monodraw.prefs.*`) map one-to-one onto a GSettings or keyfile schema. The full list is
used in `crates/core/src`.

### 5.4 Character Picker window (⇧⌘2)
Floating utility window with a palette popup and a character grid (the same component as the Characters
tab). Clicking a character sets the pencil character.

### 5.5 Snippet Creation
Fields: **Name:** (Required), **Identifier:** (Optional, reverse DNS notation), **Tags:** (Optional, space
separated), **Author:**, **Website:**, **Email:**, **Description:**. Buttons **Add Snippet** and
**Cancel**. Saves to the user snippets folder.

### 5.6 Letter-font preview
A popover table listing every FIGlet font rendered with the current text. Choosing a row sets the font.

### 5.7 Command Line Tool
Explains the CLI and shows **Tool Path:** and **Link Path:** with **Copy Tool Path**, **Copy Link Path**
and **Create Link** buttons. On Linux it shows `~/.local/bin/monodraw`.

### 5.8 Dropped (Mac/commercial)
License registration, license viewer, trial expiration, acknowledgements (replace with our About and
licences), and Sparkle updates.

---

## 6. Visual language (to reproduce, not copy)

* Monochrome line icons in the toolbar, with a filled or pushed variant for the active tool.
* Assistant has a light grey background and thin separators between sections. Section headers are
  checkboxes that enable or disable the feature, and the section body dims when disabled.
* The anchor picker is a 3×3 dot grid with the active dot filled. "Discrete rect" diagrams show attachment
  points.
* Canvas is white with faint grid lines. Rulers have a grey background and small grey numbers.
* Dark mode: every colour above has a dark token (Appearance preference: System/Light/Dark).
* Icons needed (our own drawings, `crates/gui/icons/*.svg`): select, pan, rect, table, diamond, line,
  ellipse, text, surface, pencil, bucket, eraser, picker, group, ungroup, edit, image, backward, forward,
  export, chars, sidebar, assistant, eight alignment icons, text alignment/position/sweep/movement segments,
  lock, eye, and add-snippet.

---

## 7. Current GTK editor vs target (gap list)

| Area | Have (crates/gui) | Missing |
|---|---|---|
| Layout | single window, toolbar of 5 text buttons, inspector on the right | sidebar tree, assistant tabs, rulers, menu bar, welcome |
| Tools | Select, Rect, Line, Text, Pencil | Pan, Surface, Diamond, Ellipse, Bucket, Eraser, Picker, Image, sticky Shift; key map should be S not V for Select |
| Selection | single select, move, corner resize | marquee, multi-select, all handles, line point editing, midpoints, labels, attachment points |
| Inspector | ad-hoc rows | every section in §4.2 |
| Edit ops | undo/redo, copy text | shape clipboard, duplicate, group/ungroup, z-order, lock/hide, flatten, copy/paste style, reset content, import |
| Export | text dialog | Text/PNG/SVG sheet with all options |
| Prefs | none | Preferences window + persistence |
| Look | basic | grid, rulers, page guide, missing-glyph marker, dark mode, icons |
