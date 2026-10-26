//! Integer cell geometry (SPEC §4). x grows right, y grows down.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Point { x, y }
    }
    pub fn offset(self, dx: i32, dy: i32) -> Self {
        Point::new(self.x + dx, self.y + dy)
    }
    pub fn add(self, o: Point) -> Self {
        Point::new(self.x + o.x, self.y + o.y)
    }
    pub fn sub(self, o: Point) -> Self {
        Point::new(self.x - o.x, self.y - o.y)
    }
    /// Step one cell in a single direction bit (`DIR_UP` …).
    pub fn step(self, dir: u8, n: i32) -> Self {
        let (dx, dy) = dir_delta(dir);
        Point::new(self.x + dx * n, self.y + dy * n)
    }
}

/// Parse the on-disk `"x,y"` encoding (SPEC §1.6). Floats are truncated toward zero.
pub fn parse_point(s: &str) -> Option<Point> {
    let (a, b) = s.split_once(',')?;
    let f = |v: &str| -> Option<i32> {
        let v = v.trim();
        v.parse::<i32>().ok().or_else(|| v.parse::<f64>().ok().map(|f| f as i32))
    };
    Some(Point::new(f(a)?, f(b)?))
}

pub fn format_point(p: Point) -> String {
    format!("{},{}", p.x, p.y)
}

/// Parse the `"x,y,w,h"` rect encoding (image bounds).
pub fn parse_rect(s: &str) -> Option<Rect> {
    let v: Vec<i32> = s.split(',').map(|p| p.trim().parse::<i32>().ok()).collect::<Option<_>>()?;
    match v[..] {
        [x, y, w, h] => Some(Rect::new(x, y, w, h)),
        _ => None,
    }
}

pub fn format_rect(r: Rect) -> String {
    format!("{},{},{},{}", r.x, r.y, r.w, r.h)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect { x, y, w, h }
    }
    pub fn is_empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }
    /// Last column / row (inclusive).
    pub fn x1(&self) -> i32 {
        self.x + self.w - 1
    }
    pub fn y1(&self) -> i32 {
        self.y + self.h - 1
    }
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.x + self.w && p.y < self.y + self.h
    }
    pub fn on_border(&self, p: Point) -> bool {
        self.contains(p) && (p.x == self.x || p.y == self.y || p.x == self.x1() || p.y == self.y1())
    }
    pub fn inset(&self, n: i32) -> Rect {
        Rect::new(self.x + n, self.y + n, self.w - 2 * n, self.h - 2 * n)
    }
    pub fn translate(&self, d: Point) -> Rect {
        Rect::new(self.x + d.x, self.y + d.y, self.w, self.h)
    }
    pub fn union(&self, o: &Rect) -> Rect {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        let r = (self.x + self.w).max(o.x + o.w);
        let b = (self.y + self.h).max(o.y + o.h);
        Rect::new(x, y, r - x, b - y)
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        !self.is_empty()
            && !o.is_empty()
            && self.x < o.x + o.w
            && o.x < self.x + self.w
            && self.y < o.y + o.h
            && o.y < self.y + self.h
    }
    pub fn from_points(a: Point, b: Point) -> Rect {
        let x = a.x.min(b.x);
        let y = a.y.min(b.y);
        Rect::new(x, y, (a.x - b.x).abs() + 1, (a.y - b.y).abs() + 1)
    }

    /// Keypad position (SPEC §4): returns the point and the fixed exit dirset for
    /// outside positions 10–25 (0 for 1–9, whose dirset is stored on the position).
    pub fn keypad(&self, pos: i64) -> Option<(Point, u8)> {
        let (x, y, x1, y1) = (self.x, self.y, self.x1(), self.y1());
        let cx = self.x + self.w / 2;
        let cy = self.y + self.h / 2;
        let p = |x, y, d| Some((Point::new(x, y), d));
        match pos {
            1 => p(x, y, 0),
            2 => p(cx, y, 0),
            3 => p(x1, y, 0),
            4 => p(x, cy, 0),
            5 => p(cx, cy, 0),
            6 => p(x1, cy, 0),
            7 => p(x, y1, 0),
            8 => p(cx, y1, 0),
            9 => p(x1, y1, 0),
            10 => p(x, y - 1, DIR_UP),
            11 => p(x - 1, y, DIR_LEFT),
            12 => p(cx, y - 1, DIR_UP),
            13 => p(x1, y - 1, DIR_UP),
            14 => p(x1 + 1, y, DIR_RIGHT),
            15 => p(x - 1, cy, DIR_LEFT),
            16 => p(x1 + 1, cy, DIR_RIGHT),
            17 => p(x, y1 + 1, DIR_DOWN),
            18 => p(x - 1, y1, DIR_LEFT),
            19 => p(cx, y1 + 1, DIR_DOWN),
            20 => p(x1, y1 + 1, DIR_DOWN),
            21 => p(x1 + 1, y1, DIR_RIGHT),
            22 => p(x - 1, y - 1, DIR_UP | DIR_LEFT),
            23 => p(x1 + 1, y - 1, DIR_UP | DIR_RIGHT),
            24 => p(x - 1, y1 + 1, DIR_DOWN | DIR_LEFT),
            25 => p(x1 + 1, y1 + 1, DIR_DOWN | DIR_RIGHT),
            _ => None,
        }
    }

    /// Offset of keypad point 1–9 from the top-left corner for a rect of size (w,h).
    pub fn keypad_offset(pos: i64, w: i32, h: i32) -> Point {
        Rect::new(0, 0, w, h).keypad(pos).filter(|_| (1..=9).contains(&pos)).map(|(p, _)| p).unwrap_or_default()
    }
}

/// Direction masks (`dirset`): up 1, down 2, left 4, right 8; 0 = unconstrained.
pub const DIR_UP: u8 = 1;
pub const DIR_DOWN: u8 = 2;
pub const DIR_LEFT: u8 = 4;
pub const DIR_RIGHT: u8 = 8;
pub const DIR_ALL: u8 = 15;
pub const DIRS: [u8; 4] = [DIR_UP, DIR_DOWN, DIR_LEFT, DIR_RIGHT];

pub fn dir_delta(dir: u8) -> (i32, i32) {
    match dir {
        DIR_UP => (0, -1),
        DIR_DOWN => (0, 1),
        DIR_LEFT => (-1, 0),
        DIR_RIGHT => (1, 0),
        _ => (0, 0),
    }
}

pub fn dir_reverse(dir: u8) -> u8 {
    match dir {
        DIR_UP => DIR_DOWN,
        DIR_DOWN => DIR_UP,
        DIR_LEFT => DIR_RIGHT,
        DIR_RIGHT => DIR_LEFT,
        _ => 0,
    }
}

pub fn dir_is_horizontal(dir: u8) -> bool {
    dir == DIR_LEFT || dir == DIR_RIGHT
}

/// Direction of travel from `a` to `b` along one axis (0 if equal or diagonal).
pub fn dir_between(a: Point, b: Point) -> u8 {
    match ((b.x - a.x).signum(), (b.y - a.y).signum()) {
        (0, -1) => DIR_UP,
        (0, 1) => DIR_DOWN,
        (-1, 0) => DIR_LEFT,
        (1, 0) => DIR_RIGHT,
        _ => 0,
    }
}
