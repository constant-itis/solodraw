""".monopic reader/writer written from SPEC.md."""
import gzip, json, uuid, copy

MAGIC = b"\xffMONOPIC\x01"
MODEL_VERSION = 5
NO_COLOR = 0x100000000

# ---- default-value elision table (SPEC §2.6) ----
SHAPE = {"cthrough": False, "name_edited": False, "origin": "0,0", "tag": 0, "hidden": False,
         "locked": False, "surface_pos_enabled": True, "attachable": True, "content_blended": True,
         "content_color": NO_COLOR, "shadow_enabled": False, "shadow_offset": "0,0", "shadow_tixel": "",
         "shadow_intensity": 4, "shadow_color": NO_COLOR, "subshape_refs": []}
PRIM = dict(SHAPE, background_type=2, background_enabled=True, background_tixel="",
            rectangular_border_enabled=True, mps_bckg_color=NO_COLOR, mps_border_color=NO_COLOR)
DASH0 = {"o": 0, "p": 0, "u": 0}
DEFAULTS = {
    0: {"alphabet": 1},
    1: dict(SHAPE, attachable=False, position_refs=[], shape_type=6),
    3: dict(SHAPE, shape_type=8, position_refs=[], attachable=False, subshape_selection=False),
    5: dict(SHAPE, shape_type=2, line_type=3, line_style=3, line_dash_enabled=False,
            line_dash=DASH0, head_notation=2),
    10: {"head_type": 0, "dirset": 0},
    11: {"dirset": 0},
    12: {"dirset": 0, "position_flags": 1},
    18: {"tixels": {"text_content": [], "text_points": []}},
    20: dict(PRIM, shape_type=4, rectangular_border=4, frame_offset="0,0", frame_corner=1,
             content_offset="0,0", border_dash=False, border_dash_enabled=DASH0,
             **{f"border_tixel_{k}": "" for k in ("tl", "tr", "bl", "br", "ts", "bs", "ls", "rs")}),
    21: dict(PRIM, shape_type=9, anchor_pos=1,
             **{k: "" for k in ("bt_tp", "bt_bp", "bt_lp", "bt_rp", "btd_ul", "btd_ur", "btd_ll", "btd_lr")}),
    27: {"enabled": True, "sweep": 0, "movement": 0, "alignment": 1, "position": 1, "inside_border": True,
         "color": NO_COLOR, "inset_top": 0, "inset_bottom": 0, "inset_left": 0, "inset_right": 0},
    28: dict(PRIM, shape_type=12, border_tixel="", text_container_control=0.25),
}

def _migrate_0_1(objs):
    retype = {1: (None, 6), 3: (None, 8), 4: (20, 1), 5: (None, 2), 8: (20, 5), 14: (20, 4)}
    for o in objs.values():
        t = o.get("type_id")
        if t not in retype: continue
        nt, st = retype[t]
        if nt is not None: o["type_id"] = nt
        o["shape_type"] = st
        if t == 14: o["text_enabled"] = True
        if o.get("rectangular_border") == 1: o["rectangular_border"] = 3
        else: o["rectangular_border_enabled"] = True
        bt = o.get("background_type")
        if isinstance(bt, int):
            if 5 <= bt <= 10: o["background_enabled"] = True; o["background_type"] = 4
            elif 2 <= bt <= 4: o["background_enabled"] = True
            elif bt == 1: o["background_type"] = 2

LEGACY_TEXT = [("text_enabled", "enabled"), ("text", "text"), ("text_sweep", "sweep"),
               ("text_movement", "movement"), ("text_alignment", "alignment"),
               ("text_position", "position"), ("text_inside", "inside_border")]
def _migrate_1_2(objs):
    for k in list(objs):
        o = objs[k]
        if o.get("type_id") != 20: continue
        t = {"type_id": 27, "doc_ref": o.get("doc_ref")}
        for old, new in LEGACY_TEXT: t[new] = o.get(old)
        if "lf_id" in o: t["letter_font_id"] = o["lf_id"]
        if "lf_name" in o: t["letter_font_name"] = o["lf_name"]
        nid = str(uuid.uuid4()).upper(); objs[nid] = t; o["model_text"] = nid

def _rewrite(v, m):
    if isinstance(v, str): return m.get(v, v)
    if isinstance(v, list): return [_rewrite(x, m) for x in v]
    if isinstance(v, dict): return {m.get(k, k): _rewrite(x, m) for k, x in v.items()}
    return v

def _migrate_3_4(doc):
    objs = doc.pop("objects")
    idmap = {k: str(uuid.uuid4()).upper() for k in objs}
    objs = _rewrite(objs, idmap)
    for k, o in objs.items():
        if "object_id" in o: raise ValueError("object_id already present")
        o["object_id"] = k
    doc["object_list"] = list(objs.values())

COLOR_KEYS = ("content_color", "shadow_color", "mps_bckg_color", "mps_border_color", "color")
def _migrate_4_5(lst):
    for o in lst:
        for k in COLOR_KEYS:
            if o.get(k) == 0xff000000: o[k] = NO_COLOR

def expand(lst):  # v5 default re-insertion
    for o in lst:
        for k, v in DEFAULTS.get(o.get("type_id"), {}).items():
            o.setdefault(k, copy.deepcopy(v))

def elide(lst):
    out = []
    for o in lst:
        d = DEFAULTS.get(o["type_id"], {})
        out.append({k: v for k, v in o.items() if not (k in d and d[k] == v)})
    return out

def load_json(doc):
    v = doc.get("header", {}).get("v", 0)
    if v > MODEL_VERSION: raise ValueError("file from newer Monodraw")
    if v < 3:
        steps = {0: _migrate_0_1, 1: _migrate_1_2, 2: lambda o: None}
        while v < 3: steps[v](doc["objects"]); v += 1
    if v == 3: _migrate_3_4(doc); v = 4
    lst = doc["object_list"]
    if v == 5: expand(lst)
    if v == 4: _migrate_4_5(lst); v = 5
    return {o["object_id"]: o for o in lst}

def load(path):
    b = open(path, "rb").read()
    if b.startswith(MAGIC): b = gzip.decompress(b[len(MAGIC):])
    return load_json(json.loads(b))

def save(objs, path, minify=False, pretty=False):
    lst = list(objs.values())
    if minify: lst = elide(lst)
    body = json.dumps({"header": {"v": MODEL_VERSION}, "object_list": lst},
                      indent=2 if pretty else None, ensure_ascii=False).encode()
    if path.endswith(".monojson"): open(path, "wb").write(body)
    else: open(path, "wb").write(MAGIC + gzip.compress(body, compresslevel=2))
