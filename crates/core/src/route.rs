//! Line routing and rasterization (SPEC §5.2–5.4). Clean-room: written from SPEC.md and the
//! golden data only.

use crate::geom::*;
use crate::tixel::{SEG_D, SEG_DL, SEG_DR, SEG_L, SEG_R, SEG_U, SEG_UL, SEG_UR};

/// One end of a line as seen by the routers.
#[derive(Clone, Copy, Debug)]
pub struct End {
    pub p: Point,
    /// Allowed exit directions (0 = unconstrained).
    pub dirs: u8,
    /// The rect of the shape the end is attached to, if any.
    pub rect: Option<Rect>,
}

/// Orthogonal segment between two points: straight, or one L.
/// `direction` 1 = horizontal first (corner (b.x, a.y)), 2 = vertical first (corner (a.x, b.y)).
pub fn l_corner(a: Point, b: Point, direction: i64) -> Option<Point> {
    if a.x == b.x || a.y == b.y {
        return None;
    }
    Some(if direction == 2 { Point::new(a.x, b.y) } else { Point::new(b.x, a.y) })
}

/// Drop repeated and collinear interior points.
pub fn simplify(pts: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(pts.len());
    for &p in pts {
        if out.last() == Some(&p) {
            continue;
        }
        while out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            let between = |a: i32, b: i32, c: i32| (a <= b && b <= c) || (a >= b && b >= c);
            let collinear = (a.x == b.x && b.x == p.x && between(a.y, b.y, p.y))
                || (a.y == b.y && b.y == p.y && between(a.x, b.x, p.x));
            if collinear {
                out.pop();
            } else {
                break;
            }
        }
        out.push(p);
    }
    out
}

/// Cells of one straight orthogonal run from `a` to `b`, both ends included.
fn run(a: Point, b: Point, out: &mut Vec<Point>) {
    let (dx, dy) = ((b.x - a.x).signum(), (b.y - a.y).signum());
    let mut p = a;
    out.push(p);
    while p != b {
        // A diagonal pair (not expected) is walked horizontally first.
        if p.x != b.x {
            p.x += dx;
        } else {
            p.y += dy;
        }
        out.push(p);
    }
}

/// Cells along an orthogonal polyline.
pub fn ortho_cells(pts: &[Point]) -> Vec<Point> {
    let mut out = Vec::new();
    match pts.len() {
        0 => {}
        1 => out.push(pts[0]),
        n => {
            for i in 0..n - 1 {
                run(pts[i], pts[i + 1], &mut out);
                if i + 2 < n {
                    out.pop();
                }
            }
        }
    }
    out
}

/// Cells of a free (non-orthogonal) segment; `kind` per SPEC §5.4.
pub fn free_cells(a: Point, b: Point, kind: u8) -> Vec<Point> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (sx, sy) = (dx.signum(), dy.signum());
    let x_major = dx.abs() >= dy.abs();
    let (big, small) = if x_major { (dx.abs(), dy.abs()) } else { (dy.abs(), dx.abs()) };
    // Cell at major offset `t`, minor offset `u`.
    let at = |t: i32, u: i32| if x_major { Point::new(a.x + t * sx, a.y + u * sy) } else { Point::new(a.x + u * sx, a.y + t * sy) };
    let mut out = Vec::new();
    if small == 0 || small == big {
        for t in 0..=big {
            out.push(at(t, if small == 0 { 0 } else { t }));
        }
        return out;
    }
    if kind == 2 {
        // Staggered: half the diagonal moves, the straight run, the other half.
        let first = (small + 1) / 2;
        let (mut t, mut u) = (0, 0);
        out.push(at(t, u));
        for _ in 0..first {
            t += 1;
            u += 1;
            out.push(at(t, u));
        }
        for _ in 0..big - small {
            t += 1;
            out.push(at(t, u));
        }
        for _ in 0..small - first {
            t += 1;
            u += 1;
            out.push(at(t, u));
        }
        return out;
    }
    // Step: 4-connected staircase with evenly spread steps.
    let (q, r) = ((big - small + 1) / (small + 1), (big - small + 1) % (small + 1));
    let steps: Vec<i32> = (0..small).map(|k| (k + 1) * q + k + (k + 1).min(r)).collect();
    let mut u = 0;
    for t in 0..=big {
        out.push(at(t, u));
        if steps.contains(&t) {
            u += 1;
            out.push(at(t, u));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Diagram router (SPEC §5.3)

fn delta(d: u8) -> Point {
    let (x, y) = dir_delta(d);
    Point::new(x, y)
}

/// `to` lies strictly ahead of `from` along `d`.
fn ahead(from: Point, to: Point, d: u8) -> bool {
    match d {
        DIR_UP => to.y < from.y,
        DIR_DOWN => to.y > from.y,
        DIR_LEFT => to.x < from.x,
        DIR_RIGHT => to.x > from.x,
        _ => false,
    }
}

/// Dominant direction from `a` toward `b` (horizontal on ties).
fn dominant(a: Point, b: Point) -> u8 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if dx.abs() >= dy.abs() && dx != 0 {
        if dx > 0 { DIR_RIGHT } else { DIR_LEFT }
    } else if dy > 0 {
        DIR_DOWN
    } else {
        DIR_UP
    }
}

fn mask_list(m: u8) -> Vec<u8> {
    DIRS.iter().copied().filter(|d| m & d != 0).collect()
}

/// Direction of the first step of the leg `a`→`b` drawn with orientation `o`.
fn leg_first_dir(a: Point, b: Point, o: i64) -> u8 {
    match l_corner(a, b, o) {
        Some(c) => dir_between(a, c),
        None => dir_between(a, b),
    }
}

/// Direction of the first step from `b` back along the leg `a`→`b` drawn with orientation `o`.
fn leg_back_dir(a: Point, b: Point, o: i64) -> u8 {
    match l_corner(a, b, o) {
        Some(c) => dir_between(b, c),
        None => dir_between(b, a),
    }
}

/// Does the cell run `a`→`b` (axis aligned) touch `r`?
fn run_hits(a: Point, b: Point, r: &Rect) -> bool {
    Rect::from_points(a, b).intersects(r)
}

/// Does the L leg `a`→`b` with orientation `o` touch `r`?
fn leg_hits(a: Point, b: Point, o: i64, r: &Rect) -> bool {
    match l_corner(a, b, o) {
        Some(c) => run_hits(a, c, r) || run_hits(c, b, r),
        None => run_hits(a, b, r),
    }
}

#[derive(Clone, Debug)]
struct DEnd {
    p: Point,
    rect: Option<Rect>,
    exits: Vec<u8>,
}

#[derive(Clone, Debug)]
struct Cand {
    wps: Vec<Point>,
    legs: Vec<i64>,
}

/// Step 1: an end on its own rect's border exits through that side (first of left/right/up/down
/// that is allowed) and the rect loses that row/column; an end inside its rect drops the rect.
fn sanitise(e: &mut DEnd) {
    let Some(r) = e.rect else { return };
    if !r.contains(e.p) {
        return;
    }
    for d in [DIR_LEFT, DIR_RIGHT, DIR_UP, DIR_DOWN] {
        let on_side = match d {
            DIR_LEFT => e.p.x == r.x,
            DIR_RIGHT => e.p.x == r.x1(),
            DIR_UP => e.p.y == r.y,
            _ => e.p.y == r.y1(),
        };
        if on_side && e.exits.contains(&d) {
            e.exits.retain(|&x| x != d);
            e.exits.insert(0, d);
            e.rect = Some(match d {
                DIR_LEFT => Rect::new(r.x + 1, r.y, r.w - 1, r.h),
                DIR_RIGHT => Rect::new(r.x, r.y, r.w - 1, r.h),
                DIR_UP => Rect::new(r.x, r.y + 1, r.w, r.h - 1),
                _ => Rect::new(r.x, r.y, r.w, r.h - 1),
            })
            .filter(|r| !r.is_empty());
            return;
        }
    }
    e.rect = None;
}

/// Transpose helpers so the vertical cases can be written once.
fn tp(p: Point) -> Point {
    Point::new(p.y, p.x)
}
fn tr(r: Rect) -> Rect {
    Rect::new(r.y, r.x, r.h, r.w)
}
fn td(d: u8) -> u8 {
    match d {
        DIR_UP => DIR_LEFT,
        DIR_DOWN => DIR_RIGHT,
        DIR_LEFT => DIR_UP,
        DIR_RIGHT => DIR_DOWN,
        x => x,
    }
}
fn to(o: i64) -> i64 {
    3 - o
}
fn tend(e: &DEnd) -> DEnd {
    DEnd { p: tp(e.p), rect: e.rect.map(tr), exits: e.exits.iter().map(|&d| td(d)).collect() }
}
fn tcand(c: Cand) -> Cand {
    Cand { wps: c.wps.into_iter().map(tp).collect(), legs: c.legs.into_iter().map(to).collect() }
}

/// Free distance from `from` toward `to` along `d`, stopping a cell before `rect`; offset =
/// max(2, half of it).
fn offset(from: Point, d: u8, to: Point, rect: Option<Rect>) -> i32 {
    let along = |p: Point| match d {
        DIR_UP => -p.y,
        DIR_DOWN => p.y,
        DIR_LEFT => -p.x,
        _ => p.x,
    };
    let mut dist = along(to) - along(from);
    if let Some(r) = rect {
        // Near edge of the rect seen from `from` along `d`.
        let near = match d {
            DIR_UP => -r.y1(),
            DIR_DOWN => r.y,
            DIR_LEFT => -r.x1(),
            _ => r.x,
        };
        if near > along(from) {
            dist = dist.min(near - 1 - along(from));
        }
    }
    (dist / 2).max(2)
}

/// U route for a vertical exit `a` shared by both ends (horizontal: transpose).
fn u_route(s: &DEnd, e: &DEnd, a: u8) -> Cand {
    if !matches!(a, DIR_UP | DIR_DOWN) {
        return tcand(u_route(&tend(s), &tend(e), td(a)));
    }
    let da = delta(a);
    if s.p.y == e.p.y {
        return Cand { wps: vec![s.p.add(Point::new(2 * da.x, 2 * da.y))], legs: vec![1, 1] };
    }
    let start_far = if a == DIR_UP { s.p.y < e.p.y } else { s.p.y > e.p.y };
    let (f, o) = if start_far { (s, e) } else { (e, s) };
    let p1 = f.p.add(Point::new(0, 2 * da.y));
    let upper = s.p.y.min(e.p.y);
    let mut m = Point::new(o.p.x, upper + (s.p.y - e.p.y).abs() / 2);
    if s.p.x == e.p.x {
        m.x -= 2;
    }
    if let Some(fr) = f.rect {
        let touches = leg_hits(o.p, m, 2, &fr) || leg_hits(m, p1, 2, &fr);
        if touches && !fr.contains(f.p) {
            if o.p.y > fr.y1() + 1 {
                let near = fr.y1() + 1;
                m.y = near + (o.p.y - near) / 2;
            } else if o.p.y < fr.y - 1 {
                let near = fr.y - 1;
                m.y = near - (near - o.p.y) / 2;
            } else {
                return finish_u(s, e, a, m, p1);
            }
            if m.x >= fr.x && m.x <= fr.x1() {
                let left = fr.x - 2;
                let right = fr.x1() + 2;
                m.x = if m.x - fr.x < fr.x1() - m.x { left } else { right };
                if m.x == o.p.x {
                    m.x = if m.x == left { right } else { left };
                }
            }
        }
    }
    finish_u(s, e, a, m, p1)
}

fn finish_u(s: &DEnd, e: &DEnd, a: u8, m: Point, p1: Point) -> Cand {
    if ahead(s.p, e.p, a) {
        Cand { wps: vec![m, p1], legs: vec![2, 2, 2] }
    } else {
        Cand { wps: vec![p1, m], legs: vec![1, 1, 1] }
    }
}

/// Opposite horizontal exits (vertical: transpose).
fn z_route(s: &DEnd, e: &DEnd, a: u8) -> Cand {
    if !matches!(a, DIR_LEFT | DIR_RIGHT) {
        return tcand(z_route(&tend(s), &tend(e), td(a)));
    }
    let (r, l) = if a == DIR_RIGHT { (s, e) } else { (e, s) };
    if l.p.x - r.p.x >= 2 {
        if s.p.y == e.p.y {
            return Cand { wps: vec![], legs: vec![1] };
        }
        let mid = Point::new(s.p.x.min(e.p.x) + (s.p.x - e.p.x).abs() / 2, s.p.y.min(e.p.y) + (s.p.y - e.p.y).abs() / 2);
        return Cand { wps: vec![mid], legs: vec![1, 2] };
    }
    let rect_of = |x: &DEnd| x.rect.unwrap_or(Rect::new(x.p.x, x.p.y, 1, 1));
    let (sr, er) = (rect_of(s), rect_of(e));
    let (up, lo) = if s.p.y <= e.p.y { (sr, er) } else { (er, sr) };
    let gap = lo.y - up.y1();
    let mut ym = up.y1() + gap / 2;
    let da = if a == DIR_RIGHT { 2 } else { -2 };
    let mut sx = s.p.x + da;
    let mut ex = e.p.x - da;
    let hits = |y: i32, sx: i32, ex: i32| {
        let (pa, pb) = (Point::new(sx, y), Point::new(ex, y));
        s.rect.is_some_and(|r| run_hits(pa, pb, &r)) || e.rect.is_some_and(|r| run_hits(pa, pb, &r))
    };
    if gap < 2 || hits(ym, sx, ex) {
        let u = sr.union(&er);
        let above = u.y - 2;
        let below = u.y1() + 2;
        let sum = s.p.y + e.p.y;
        ym = if (sum - 2 * above).abs() <= (2 * below - sum).abs() { above } else { below };
        if let Some(r) = e.rect {
            if sx >= r.x && sx <= r.x1() {
                sx = if da > 0 { r.x1() + 2 } else { r.x - 2 };
            }
        }
        if let Some(r) = s.rect {
            if ex >= r.x && ex <= r.x1() {
                ex = if da > 0 { r.x - 2 } else { r.x1() + 2 };
            }
        }
    }
    Cand { wps: vec![Point::new(sx, ym), Point::new(ex, ym)], legs: vec![1, 1, 2] }
}

fn perp_route(s: &DEnd, e: &DEnd, a: u8, b: u8) -> Cand {
    let o = if dir_is_horizontal(a) { 1 } else { 2 };
    if ahead(s.p, e.p, a) && ahead(e.p, s.p, b) {
        return Cand { wps: vec![], legs: vec![o] };
    }
    let oa = offset(s.p, a, e.p, e.rect);
    let ob = offset(e.p, b, s.p, s.rect);
    let pa = s.p.add(Point::new(delta(a).x * oa, delta(a).y * oa));
    let pb = e.p.add(Point::new(delta(b).x * ob, delta(b).y * ob));
    let mut p = if dir_is_horizontal(a) { Point::new(pa.x, pb.y) } else { Point::new(pb.x, pa.y) };
    // Push out of the end rect along a, then out of the start rect along b.
    if let Some(r) = e.rect {
        if leg_hits(s.p, p, o, &r) {
            p = push_out(p, a, &r);
        }
    }
    if let Some(r) = s.rect {
        if leg_hits(p, e.p, o, &r) {
            p = push_out(p, b, &r);
        }
    }
    let d1 = leg_back_dir(s.p, p, o);
    let d2 = leg_first_dir(p, e.p, o);
    if d1 != 0 && d1 == d2 {
        for x in [a, b] {
            if dir_is_horizontal(x) != dir_is_horizontal(d1) {
                p = p.add(delta(x));
                break;
            }
        }
    }
    Cand { wps: vec![p], legs: vec![o, o] }
}

fn push_out(p: Point, d: u8, r: &Rect) -> Point {
    match d {
        DIR_UP => Point::new(p.x, r.y - 2),
        DIR_DOWN => Point::new(p.x, r.y1() + 2),
        DIR_LEFT => Point::new(r.x - 2, p.y),
        _ => Point::new(r.x1() + 2, p.y),
    }
}

fn candidate(s: &DEnd, e: &DEnd, a: u8, b: u8) -> Option<Cand> {
    if a == b {
        Some(u_route(s, e, a))
    } else if a == dir_reverse(b) {
        Some(z_route(s, e, a))
    } else {
        Some(perp_route(s, e, a, b))
    }
}

fn score(s: &DEnd, e: &DEnd, c: &Cand) -> i32 {
    let a = c.wps[0];
    let d1 = leg_first_dir(s.p, a, c.legs[0]);
    let last = *c.wps.last().unwrap();
    let d2 = leg_back_dir(last, e.p, *c.legs.last().unwrap());
    if d1 == 0 || d2 == 0 {
        1
    } else if d1 == d2 {
        2
    } else if d1 == dir_reverse(d2) {
        0
    } else {
        1
    }
}

fn solve(s: &DEnd, e: &DEnd) -> Option<Cand> {
    let mut best: Option<(i32, Cand)> = None;
    for &a in &s.exits {
        for &b in &e.exits {
            let Some(c) = candidate(s, e, a, b) else { continue };
            if c.wps.is_empty() {
                return Some(c);
            }
            let sc = score(s, e, &c);
            if sc == 0 {
                return Some(c);
            }
            if best.as_ref().is_none_or(|(bs, _)| sc < *bs) {
                best = Some((sc, c));
            }
        }
    }
    best.map(|(_, c)| c)
}

/// Corner points of a Diagram-routed line (SPEC §5.3).
pub fn diagram_route(start: End, end: End) -> Vec<Point> {
    if start.p == end.p {
        return vec![start.p];
    }
    let (mut sd, mut ed) = (start.dirs & DIR_ALL, end.dirs & DIR_ALL);
    let derive = |o: Point, om: u8, x: Point| {
        [DIR_LEFT, DIR_RIGHT, DIR_UP, DIR_DOWN]
            .into_iter()
            .find(|&d| om & d != 0 && ahead(o, x, d))
            .map(dir_reverse)
            .unwrap_or_else(|| dominant(x, o))
    };
    if sd == 0 && ed == 0 {
        sd = dominant(start.p, end.p);
        ed = dominant(end.p, start.p);
    } else if sd == 0 {
        sd = derive(end.p, ed, start.p);
    } else if ed == 0 {
        ed = derive(start.p, sd, end.p);
    }
    let mut s = DEnd { p: start.p, rect: start.rect.filter(|r| !r.is_empty()), exits: mask_list(sd) };
    let mut e = DEnd { p: end.p, rect: end.rect.filter(|r| !r.is_empty()), exits: mask_list(ed) };
    sanitise(&mut s);
    sanitise(&mut e);

    let legs_to_points = |c: &Cand| {
        let mut pts = vec![s.p];
        let mut all = vec![s.p];
        all.extend(c.wps.iter().copied());
        all.push(e.p);
        for i in 0..all.len() - 1 {
            if let Some(k) = l_corner(all[i], all[i + 1], c.legs[i]) {
                pts.push(k);
            }
            pts.push(all[i + 1]);
        }
        simplify(&pts)
    };

    // Step 2: a single L that leaves through allowed exits at both ends.
    for o in [1, 2] {
        if s.exits.contains(&leg_first_dir(s.p, e.p, o)) && e.exits.contains(&leg_back_dir(s.p, e.p, o)) {
            return legs_to_points(&Cand { wps: vec![], legs: vec![o] });
        }
    }
    // Steps 3–4.
    let comp = |x: &DEnd| DEnd { exits: DIRS.iter().copied().filter(|d| !x.exits.contains(d)).collect(), ..x.clone() };
    let tries = [(s.clone(), e.clone()), (s.clone(), comp(&e)), (comp(&s), e.clone()), (comp(&s), comp(&e))];
    for (ts, te) in &tries {
        if let Some(c) = solve(ts, te) {
            return legs_to_points(&c);
        }
    }
    legs_to_points(&Cand { wps: vec![], legs: vec![1] })
}

// ---------------------------------------------------------------------------
// Automatic (SPEC §5.3)

/// Per-segment L directions (1/2) for an Automatic line.
pub fn automatic_directions(ends: &[End]) -> Vec<i64> {
    let n = ends.len().saturating_sub(1);
    if n == 0 {
        return Vec::new();
    }
    let pts: Vec<Point> = ends.iter().map(|e| e.p).collect();
    let mut best: Option<((i32, i32, i32), Vec<i64>)> = None;
    let mut fallback: Option<Vec<i64>> = None;
    let mut cur = Vec::with_capacity(n);
    auto_dfs(ends, &pts, &mut cur, &mut best, &mut fallback);
    best.map(|(_, d)| d).or(fallback).unwrap_or_else(|| vec![1; n])
}

fn auto_dfs(
    ends: &[End],
    pts: &[Point],
    cur: &mut Vec<i64>,
    best: &mut Option<((i32, i32, i32), Vec<i64>)>,
    fallback: &mut Option<Vec<i64>>,
) {
    let n = pts.len() - 1;
    if cur.len() == n {
        let k = auto_key(ends, pts, cur);
        if best.as_ref().is_none_or(|(bk, _)| k > *bk) {
            *best = Some((k, cur.clone()));
        }
        return;
    }
    let i = cur.len();
    for o in [1, 2] {
        cur.push(o);
        // Unusable if the leg into waypoint i leaves it back along the leg it arrived on.
        let usable = i == 0 || {
            let arrive = leg_back_dir(pts[i - 1], pts[i], cur[i - 1]);
            let leave = leg_first_dir(pts[i], pts[i + 1], o);
            arrive == 0 || leave != arrive
        };
        if usable {
            auto_dfs(ends, pts, cur, best, fallback);
        } else if fallback.is_none() {
            let mut f = cur.clone();
            f.resize(n, 1);
            *fallback = Some(f);
        }
        cur.pop();
    }
}

fn auto_key(ends: &[End], pts: &[Point], dirs: &[i64]) -> (i32, i32, i32) {
    let n = pts.len() - 1;
    let first = leg_first_dir(pts[0], pts[1], dirs[0]);
    let last = leg_back_dir(pts[n - 1], pts[n], dirs[n - 1]);
    let mut ok = 0;
    if ends[0].dirs & first != 0 {
        ok += 1;
    }
    if ends[n].dirs & last != 0 {
        ok += 1;
    }
    let mut hits = 0;
    for i in 0..n {
        for e in ends {
            if let Some(r) = e.rect.filter(|r| !r.is_empty()) {
                if leg_hits(pts[i], pts[i + 1], dirs[i], &r) {
                    hits += 1;
                }
            }
        }
    }
    let mut straight = 0;
    for i in 1..n {
        let arrive = leg_back_dir(pts[i - 1], pts[i], dirs[i - 1]);
        let leave = leg_first_dir(pts[i], pts[i + 1], dirs[i]);
        if arrive != 0 && leave == dir_reverse(arrive) {
            straight += 1;
        }
    }
    (ok, -hits, straight)
}

// ---------------------------------------------------------------------------
// Segment masks (SPEC §5.4)

/// Segment bit for the step from a to b.
pub fn seg_bit(a: Point, b: Point) -> u8 {
    match ((b.x - a.x).signum(), (b.y - a.y).signum()) {
        (0, -1) => SEG_U,
        (0, 1) => SEG_D,
        (-1, 0) => SEG_L,
        (1, 0) => SEG_R,
        (-1, -1) => SEG_UL,
        (1, -1) => SEG_UR,
        (-1, 1) => SEG_DL,
        (1, 1) => SEG_DR,
        _ => 0,
    }
}

/// Segment mask per cell of a path.
pub fn path_masks(cells: &[Point]) -> Vec<u8> {
    let n = cells.len();
    (0..n)
        .map(|i| {
            let mut m = 0;
            if i > 0 {
                m |= seg_bit(cells[i], cells[i - 1]);
            }
            if i + 1 < n {
                m |= seg_bit(cells[i], cells[i + 1]);
            }
            if m == 0 { SEG_L | SEG_R } else { m }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i32, y: i32) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn step_staircase_spreads_steps() {
        // M = 5, m = 2: q, r = divmod(4, 3) = (1, 1); steps at major offsets 2 and 4.
        let c = free_cells(p(0, 0), p(5, 2), 1);
        assert_eq!(c, vec![p(0, 0), p(1, 0), p(2, 0), p(2, 1), p(3, 1), p(4, 1), p(4, 2), p(5, 2)]);
    }

    #[test]
    fn staggered_splits_diagonals() {
        let c = free_cells(p(0, 0), p(5, 3), 2);
        assert_eq!(c, vec![p(0, 0), p(1, 1), p(2, 2), p(3, 2), p(4, 2), p(5, 3)]);
    }

    #[test]
    fn simplify_and_cells() {
        let s = simplify(&[p(0, 0), p(0, 0), p(2, 0), p(4, 0), p(4, 2)]);
        assert_eq!(s, vec![p(0, 0), p(4, 0), p(4, 2)]);
        assert_eq!(ortho_cells(&s).len(), 7);
        assert_eq!(path_masks(&[p(1, 1)]), vec![SEG_L | SEG_R]);
    }

    #[test]
    fn opposite_exits_far_apart_make_a_z() {
        let a = End { p: p(10, 2), dirs: DIR_RIGHT, rect: Some(Rect::new(0, 0, 10, 5)) };
        let b = End { p: p(24, 15), dirs: DIR_LEFT, rect: Some(Rect::new(25, 13, 10, 5)) };
        assert_eq!(diagram_route(a, b), vec![p(10, 2), p(17, 2), p(17, 15), p(24, 15)]);
    }
}
