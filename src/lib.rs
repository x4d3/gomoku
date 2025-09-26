use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    window, CanvasRenderingContext2d, Element, HtmlCanvasElement, KeyboardEvent, PointerEvent,
    WheelEvent,
};

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let doc = window().unwrap().document().unwrap();
    let canvas: HtmlCanvasElement = doc
        .get_element_by_id("board")
        .unwrap()
        .dyn_into::<HtmlCanvasElement>()?;
    let ctx = canvas
        .get_context("2d")?
        .unwrap()
        .dyn_into::<CanvasRenderingContext2d>()?;

    let app = Rc::new(RefCell::new(App::new(canvas.clone(), ctx, Game::new())));
    App::attach_listeners(&app);
    app.borrow_mut().resize();
    app.borrow_mut().render();

    wasm_bindgen_futures::spawn_local(App::raf_loop(app));
    Ok(())
}

/* ---------- Model ---------- */

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Color {
    Black,
    White,
}
impl Color {
    fn other(self) -> Color {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
struct Pt {
    x: i32,
    y: i32,
}
impl Pt {
    #[inline]
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    #[inline]
    fn add(self, dx: i32, dy: i32) -> Self {
        Self { x: self.x + dx, y: self.y + dy }
    }
}
const DIRS: [Pt; 4] = [
    Pt { x: 1, y: 0 },
    Pt { x: 0, y: 1 },
    Pt { x: 1, y: 1 },
    Pt { x: 1, y: -1 },
];

#[derive(Clone)]
struct Game {
    cells: HashMap<Pt, Color>,   // empty = absent
    player: Color,               // whose turn
    winner: Option<Color>,       // None until someone wins
    last_move: Option<Pt>,
    frontier: HashSet<Pt>,       // candidate empty points near stones
}

impl Game {
    fn new() -> Self {
        let mut g = Self {
            cells: HashMap::new(),
            player: Color::Black,
            winner: None,
            last_move: None,
            frontier: HashSet::new(),
        };
        g.rebuild_frontier();
        g
    }

    fn reset(&mut self) {
        self.cells.clear();
        self.player = Color::Black;
        self.winner = None;
        self.last_move = None;
        self.frontier.clear();
        self.rebuild_frontier();
    }

    #[inline]
    fn color_at(&self, p: Pt) -> Option<Color> {
        self.cells.get(&p).copied()
    }
    #[inline]
    fn is_empty(&self, p: Pt) -> bool {
        !self.cells.contains_key(&p)
    }

    fn playable(&self, p: Pt) -> bool {
        self.winner.is_none() && self.is_empty(p)
    }

    fn play(&mut self, p: Pt) -> bool {
        if !self.playable(p) {
            return false;
        }
        self.cells.insert(p, self.player);
        self.last_move = Some(p);
        if self.check_win(p, self.player) {
            self.winner = Some(self.player);
        }
        self.player = self.player.other();
        self.rebuild_frontier();
        true
    }

    fn rebuild_frontier(&mut self) {
        self.frontier.clear();
        if self.cells.is_empty() {
            for dx in -2..=2 {
                for dy in -2..=2 {
                    self.frontier.insert(Pt::new(dx, dy));
                }
            }
            return;
        }
        for (&p, _) in self.cells.iter() {
            for dx in -2..=2 {
                for dy in -2..=2 {
                    let q = p.add(dx, dy);
                    if !self.cells.contains_key(&q) {
                        self.frontier.insert(q);
                    }
                }
            }
        }
    }

    fn check_win(&self, p: Pt, who: Color) -> bool {
        for d in DIRS {
            let mut count = 1;
            count += self.ray(p, d, who);
            count += self.ray(p, Pt::new(-d.x, -d.y), who);
            if count >= 5 {
                return true;
            }
        }
        false
    }

    fn ray(&self, mut p: Pt, d: Pt, who: Color) -> i32 {
        let mut c = 0;
        p = p.add(d.x, d.y);
        while self.color_at(p) == Some(who) {
            c += 1;
            p = p.add(d.x, d.y);
        }
        c
    }

    fn line_len_open(&self, p: Pt, d: Pt, who: Color) -> (i32, i32) {
        // forward
        let mut a = 0;
        let mut q = p.add(d.x, d.y);
        while self.color_at(q) == Some(who) {
            a += 1;
            q = q.add(d.x, d.y);
        }
        // backward
        let mut b = 0;
        let mut r = p.add(-d.x, -d.y);
        while self.color_at(r) == Some(who) {
            b += 1;
            r = r.add(-d.x, -d.y);
        }
        (a, b)
    }

    fn open_ends(&self, p: Pt, d: Pt, who: Color) -> i32 {
        let mut open = 0;
        // forward end
        let mut q = p.add(d.x, d.y);
        while self.color_at(q) == Some(who) {
            q = q.add(d.x, d.y);
        }
        if self.is_empty(q) {
            open += 1;
        }
        // backward end
        let mut r = p.add(-d.x, -d.y);
        while self.color_at(r) == Some(who) {
            r = r.add(-d.x, -d.y);
        }
        if self.is_empty(r) {
            open += 1;
        }
        open
    }

    fn score_point(&self, p: Pt, who: Color) -> i32 {
        if !self.is_empty(p) {
            return i32::MIN / 4;
        }
        let mut s = 0;
        for d in DIRS {
            let (a, b) = self.line_len_open(p, d, who);
            let len = a + 1 + b;
            let open = self.open_ends(p, d, who);
            s += match (len, open) {
                (l, _) if l >= 5 => 1_000_000,
                (4, 2) => 50_000,
                (4, 1) => 20_000,
                (3, 2) => 10_000,
                (3, 1) => 1_000,
                (2, 2) => 500,
                (2, 1) => 100,
                (1, 2) => 50,
                _ => 10,
            };

            // defend opponent
            let opp = who.other();
            let (oa, ob) = self.line_len_open(p, d, opp);
            let olen = oa + 1 + ob;
            let oopen = self.open_ends(p, d, opp);
            s += match (olen, oopen) {
                (l, _) if l >= 5 => 900_000,
                (4, 2) => 40_000,
                (4, 1) => 15_000,
                (3, 2) => 8_000,
                (3, 1) => 800,
                _ => 0,
            };
        }
        s
    }

    fn best_move(&self, who: Color) -> Option<(Pt, i32)> {
        let mut best: Option<(Pt, i32)> = None;
        for &p in &self.frontier {
            let sc = self.score_point(p, who);
            best = match best {
                None => Some((p, sc)),
                Some((_, bs)) if sc > bs => Some((p, sc)),
                other => other,
            };
        }
        best
    }
}

/* ---------- App / UI ---------- */

struct App {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    game: Game,
    cell_px: f64, // one cell in logical (CSS) px
    cam_x: f64,
    cam_y: f64,
    view_w: f64, // logical width (CSS px)
    view_h: f64, // logical height (CSS px)
    dirty: bool,
}

impl App {
    fn new(canvas: HtmlCanvasElement, ctx: CanvasRenderingContext2d, game: Game) -> Self {
        Self {
            canvas,
            ctx,
            game,
            cell_px: 36.0,
            cam_x: 0.0,
            cam_y: 0.0,
            view_w: 0.0,
            view_h: 0.0,
            dirty: true,
        }
    }

    fn attach_listeners(app: &Rc<RefCell<App>>) {
        // Pointer (tap/click) for placing stones
        {
            let app_rc = Rc::clone(app);
            let closure = Closure::<dyn FnMut(PointerEvent)>::new(move |e: PointerEvent| {
                e.prevent_default();
                app_rc.borrow_mut().on_pointer_down(e);
            });
            app.borrow()
                .canvas
                .add_event_listener_with_callback("pointerdown", closure.as_ref().unchecked_ref())
                .unwrap();
            closure.forget();
        }

        // Wheel for zoom + horizontal pan
        {
            let app_rc = Rc::clone(app);
            let closure = Closure::<dyn FnMut(WheelEvent)>::new(move |e: WheelEvent| {
                e.prevent_default();
                app_rc.borrow_mut().on_wheel(e);
            });
            app.borrow()
                .canvas
                .add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())
                .unwrap();
            closure.forget();
        }

        // Keyboard panning / reset
        {
            let app_rc = Rc::clone(app);
            let doc = window().unwrap().document().unwrap();
            let closure = Closure::<dyn FnMut(KeyboardEvent)>::new(move |e: KeyboardEvent| {
                app_rc.borrow_mut().on_key(e);
            });
            doc.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())
                .unwrap();
            closure.forget();
        }

        // Resize
        {
            let app_rc = Rc::clone(app);
            let closure = Closure::<dyn FnMut()>::new(move || {
                let mut a = app_rc.borrow_mut();
                a.resize();
                a.render();
            });
            window()
                .unwrap()
                .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
                .unwrap();
            closure.forget();
        }
    }

    async fn raf_loop(app: Rc<RefCell<App>>) {
        loop {
            app.borrow_mut().render();
            let _ = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut |resolve, _| {
                window()
                    .unwrap()
                    .request_animation_frame(resolve.unchecked_ref())
                    .unwrap();
            }))
                .await;
        }
    }

    fn resize(&mut self) {
        // Logical CSS size
        let rect = self
            .canvas
            .unchecked_ref::<Element>()
            .get_bounding_client_rect();
        self.view_w = rect.width();
        self.view_h = rect.height();

        // Backing store size in device pixels
        let dpr = window().unwrap().device_pixel_ratio();
        self.canvas.set_width((self.view_w * dpr) as u32);
        self.canvas.set_height((self.view_h * dpr) as u32);

        // Draw in logical pixels by scaling the context
        let _ = self.ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        let _ = self.ctx.scale(dpr, dpr);

        self.dirty = true;
    }

    // Mapping using logical sizes
    fn screen_to_cell_f64(&self, sx: f64, sy: f64) -> (f64, f64) {
        let x = (sx - self.view_w / 2.0) / self.cell_px + self.cam_x;
        let y = (sy - self.view_h / 2.0) / self.cell_px + self.cam_y;
        (x, y)
    }
    fn screen_to_cell(&self, sx: f64, sy: f64) -> Pt {
        let (x, y) = self.screen_to_cell_f64(sx, sy);
        Pt::new(x.round() as i32, y.round() as i32)
    }

    fn on_pointer_down(&mut self, e: PointerEvent) {
        let rect = self
            .canvas
            .unchecked_ref::<Element>()
            .get_bounding_client_rect();
        let sx = e.client_x() as f64 - rect.left();
        let sy = e.client_y() as f64 - rect.top();

        let p = self.screen_to_cell(sx, sy);
        if self.game.play(p) {
            self.dirty = true;
            if self.game.winner.is_none() {
                if let Some((ai_p, _)) = self.game.best_move(self.game.player) {
                    self.game.play(ai_p);
                }
            }
        }
    }

    fn on_wheel(&mut self, e: WheelEvent) {
        let rect = self
            .canvas
            .unchecked_ref::<Element>()
            .get_bounding_client_rect();
        let sx = e.client_x() as f64 - rect.left();
        let sy = e.client_y() as f64 - rect.top();

        let dx = e.delta_x();
        let dy = e.delta_y();

        if e.shift_key() || dx.abs() > dy.abs() {
            // Horizontal pan by trackpad
            let pan_cells = dx / self.cell_px.max(1.0);
            self.cam_x += pan_cells;
            self.dirty = true;
            return;
        }

        // Zoom toward cursor
        let zoom_step = 1.1_f64;
        let old = self.cell_px;
        let mut new = if dy < 0.0 { old * zoom_step } else { old / zoom_step };
        new = new.clamp(12.0, 80.0);
        if (new - old).abs() < f64::EPSILON {
            return;
        }

        let (cell_x, cell_y) = self.screen_to_cell_f64(sx, sy);
        self.cell_px = new;
        self.cam_x = cell_x - (sx - self.view_w / 2.0) / self.cell_px;
        self.cam_y = cell_y - (sy - self.view_h / 2.0) / self.cell_px;

        self.dirty = true;
    }

    fn on_key(&mut self, e: KeyboardEvent) {
        match e.key().as_str() {
            "ArrowLeft" => {
                self.cam_x -= 3.0;
                self.dirty = true;
            }
            "ArrowRight" => {
                self.cam_x += 3.0;
                self.dirty = true;
            }
            "ArrowUp" => {
                self.cam_y -= 3.0;
                self.dirty = true;
            }
            "ArrowDown" => {
                self.cam_y += 3.0;
                self.dirty = true;
            }
            "-" => {
                self.cell_px = (self.cell_px * 0.9).max(12.0);
                self.dirty = true;
            }
            "+" | "=" => {
                self.cell_px = (self.cell_px * 1.1).min(80.0);
                self.dirty = true;
            }
            "r" | "R" => {
                self.game.reset();
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn render(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let w = self.view_w;
        let h = self.view_h;

        // background
        self.ctx.set_fill_style_str("#0b0d11");
        self.ctx.fill_rect(0.0, 0.0, w, h);

        // grid
        self.ctx.set_stroke_style_str("#20242b");
        self.ctx.set_line_width(1.0);
        let half_w = (w / 2.0) / self.cell_px;
        let half_h = (h / 2.0) / self.cell_px;
        let min_x = (self.cam_x - half_w - 1.0).floor() as i32;
        let max_x = (self.cam_x + half_w + 1.0).ceil() as i32;
        let min_y = (self.cam_y - half_h - 1.0).floor() as i32;
        let max_y = (self.cam_y + half_h + 1.0).ceil() as i32;

        for gx in min_x..=max_x {
            let sx = (gx as f64 - self.cam_x) * self.cell_px + w / 2.0;
            self.ctx.begin_path();
            self.ctx.move_to(sx, 0.0);
            self.ctx.line_to(sx, h);
            self.ctx.stroke();
        }
        for gy in min_y..=max_y {
            let sy = (gy as f64 - self.cam_y) * self.cell_px + h / 2.0;
            self.ctx.begin_path();
            self.ctx.move_to(0.0, sy);
            self.ctx.line_to(w, sy);
            self.ctx.stroke();
        }

        // stones
        for (&p, &c) in self.game.cells.iter() {
            let sx = (p.x as f64 - self.cam_x) * self.cell_px + w / 2.0;
            let sy = (p.y as f64 - self.cam_y) * self.cell_px + h / 2.0;
            if sx < -self.cell_px
                || sx > w + self.cell_px
                || sy < -self.cell_px
                || sy > h + self.cell_px
            {
                continue;
            }
            let r = self.cell_px * 0.4;
            self.ctx.begin_path();
            let _ = self.ctx.arc(sx, sy, r, 0.0, std::f64::consts::TAU);
            match c {
                Color::Black => self.ctx.set_fill_style_str("#e6edf3"),
                Color::White => self.ctx.set_fill_style_str("#38bdf8"),
            }
            self.ctx.fill();
        }

        // HUD
        self.ctx.set_fill_style_str("#e5e7eb");
        self.ctx
            .set_font("14px ui-sans-serif, system-ui, -apple-system");
        let turn = match self.game.player {
            Color::Black => "Your turn (X)",
            Color::White => "AI thinking (O)",
        };
        let status = match self.game.winner {
            None => turn,
            Some(Color::Black) => "You win! Press R to restart.",
            Some(Color::White) => "AI wins! Press R to restart.",
        };
        let _ = self.ctx.fill_text(status, 12.0, 22.0);
    }
}
