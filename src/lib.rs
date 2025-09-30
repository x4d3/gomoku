//! Infinite Gomoku (5-in-a-row) in Rust → WebAssembly.
//!
//! - Unbounded sparse board keyed by `Pt`.
//! - Choose Human/AI per color via on-canvas toggles.
//! - Mobile-friendly via Pointer Events; high-DPI aware canvas.
//! - Mouse/touchpad wheel: zoom toward cursor; horizontal pan.
//!
//! Controls
//! - Tap/click to place.
//! - Wheel up/down = zoom in/out (toward cursor).
//! - Shift+wheel or horizontal wheel = pan left/right.
//! - Arrow keys to pan; `R` to reset.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    window, CanvasRenderingContext2d, Element, HtmlCanvasElement, KeyboardEvent, PointerEvent,
    WheelEvent
};

/// Entry point invoked by the browser when the module loads.
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

    // Shared UI/application state.
    let app = Rc::new(RefCell::new(App::new(canvas.clone(), ctx, Game::new())));
    App::attach_listeners(&app);

    {
        let mut a = app.borrow_mut();
        a.resize();
        a.render();
        if a.is_ai_turn() {
            a.queue_ai_soon(120.0);
        }
    }

    wasm_bindgen_futures::spawn_local(App::raf_loop(app));
    Ok(())
}

/* ---------- Model ---------- */

/// The side (owner of a stone or current player).
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

/// Integer grid point. Keys the sparse board and frontier.
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
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
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
    cells: HashMap<Pt, Color>,
    player: Color,
    winner: Option<Color>,
    last_move: Option<Pt>,
    frontier: HashSet<Pt>,
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
    fn color_at(&self, p: Pt) -> Option<&Color> {
        self.cells.get(&p)
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
        while self.color_at(p) == Some(&who) {
            c += 1;
            p = p.add(d.x, d.y);
        }
        c
    }

    fn line_len_open(&self, p: Pt, d: Pt, who: Color) -> (i32, i32) {
        let mut a = 0;
        let mut q = p.add(d.x, d.y);
        while self.color_at(q) == Some(&who) {
            a += 1;
            q = q.add(d.x, d.y);
        }
        let mut b = 0;
        let mut r = p.add(-d.x, -d.y);
        while self.color_at(r) == Some(&who) {
            b += 1;
            r = r.add(-d.x, -d.y);
        }
        (a, b)
    }

    fn open_ends(&self, p: Pt, d: Pt, who: Color) -> i32 {
        let mut open = 0;
        let mut q = p.add(d.x, d.y);
        while self.color_at(q) == Some(&who) {
            q = q.add(d.x, d.y);
        }
        if self.is_empty(q) {
            open += 1;
        }
        let mut r = p.add(-d.x, -d.y);
        while self.color_at(r) == Some(&who) {
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

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Controller {
    Human,
    AI,
}

struct App {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    game: Game,

    ctrl_black: Controller,
    ctrl_white: Controller,

    cell_px: f64,
    cam_x: f64,
    cam_y: f64,
    view_w: f64,
    view_h: f64,

    want_ai: bool,
    next_ai_at_ms: f64,

    btn_black: (f64, f64, f64, f64),
    btn_white: (f64, f64, f64, f64),

    dirty: bool,
}

impl App {
    fn new(canvas: HtmlCanvasElement, ctx: CanvasRenderingContext2d, game: Game) -> Self {
        Self {
            canvas,
            ctx,
            game,
            ctrl_black: Controller::Human,
            ctrl_white: Controller::AI,
            cell_px: 36.0,
            cam_x: 0.0,
            cam_y: 0.0,
            view_w: 0.0,
            view_h: 0.0,
            want_ai: false,
            next_ai_at_ms: 0.0,
            btn_black: (0.0, 0.0, 0.0, 0.0),
            btn_white: (0.0, 0.0, 0.0, 0.0),
            dirty: true,
        }
    }

    fn attach_listeners(app: &Rc<RefCell<App>>) {
        // Pointer
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
        // Wheel
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
        // Keyboard
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
            {
                let mut a = app.borrow_mut();
                a.maybe_ai_step();
                a.render();
            }
            let _ =
                wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut |resolve, _| {
                    window()
                        .unwrap()
                        .request_animation_frame(resolve.unchecked_ref())
                        .unwrap();
                }))
                    .await;
        }
    }

    fn resize(&mut self) {
        let rect = self
            .canvas
            .unchecked_ref::<Element>()
            .get_bounding_client_rect();
        self.view_w = rect.width();
        self.view_h = rect.height();

        let dpr = window().unwrap().device_pixel_ratio();
        self.canvas.set_width((self.view_w * dpr) as u32);
        self.canvas.set_height((self.view_h * dpr) as u32);

        let _ = self.ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        let _ = self.ctx.scale(dpr, dpr);

        self.dirty = true;
    }

    fn screen_to_cell_f64(&self, sx: f64, sy: f64) -> (f64, f64) {
        let x = (sx - self.view_w / 2.0) / self.cell_px + self.cam_x;
        let y = (sy - self.view_h / 2.0) / self.cell_px + self.cam_y;
        (x, y)
    }
    fn screen_to_cell(&self, sx: f64, sy: f64) -> Pt {
        let (x, y) = self.screen_to_cell_f64(sx, sy);
        Pt::new(x.round() as i32, y.round() as i32)
    }

    fn is_human(&self, side: Color) -> bool {
        match side {
            Color::Black => self.ctrl_black == Controller::Human,
            Color::White => self.ctrl_white == Controller::Human,
        }
    }
    fn is_ai(&self, side: Color) -> bool {
        !self.is_human(side)
    }
    fn is_ai_turn(&self) -> bool {
        self.game.winner.is_none() && self.is_ai(self.game.player)
    }

    fn queue_ai_soon(&mut self, delay_ms: f64) {
        let now = window().unwrap().performance().unwrap().now();
        self.want_ai = true;
        self.next_ai_at_ms = now + delay_ms;
    }
    fn maybe_ai_step(&mut self) {
        if !self.is_ai_turn() || !self.want_ai {
            return;
        }
        let now = window().unwrap().performance().unwrap().now();
        if now < self.next_ai_at_ms {
            return;
        }
        if let Some((ai_p, _)) = self.game.best_move(self.game.player) {
            self.game.play(ai_p);
            self.dirty = true;
            if self.is_ai_turn() {
                self.queue_ai_soon(120.0);
            } else {
                self.want_ai = false;
            }
        } else {
            self.want_ai = false;
        }
    }

    fn on_pointer_down(&mut self, e: PointerEvent) {
        let rect = self
            .canvas
            .unchecked_ref::<Element>()
            .get_bounding_client_rect();
        let sx = e.client_x() as f64 - rect.left();
        let sy = e.client_y() as f64 - rect.top();

        if self.game.winner.is_some() {
            self.game.reset();
            self.dirty = true;
            if self.is_ai_turn() {
                self.queue_ai_soon(120.0);
            } else {
                self.want_ai = false;
            }
            return;
        }

        // Toggle pills first.
        if self.hit_btn(self.btn_black, sx, sy) {
            self.ctrl_black = if self.ctrl_black == Controller::Human {
                Controller::AI
            } else {
                Controller::Human
            };
            self.dirty = true;
            if self.game.player == Color::Black && self.is_ai(Color::Black) {
                self.queue_ai_soon(80.0);
            }
            return;
        }
        if self.hit_btn(self.btn_white, sx, sy) {
            self.ctrl_white = if self.ctrl_white == Controller::Human {
                Controller::AI
            } else {
                Controller::Human
            };
            self.dirty = true;
            if self.game.player == Color::White && self.is_ai(Color::White) {
                self.queue_ai_soon(80.0);
            }
            return;
        }

        // Board play (human-only)
        if !self.is_human(self.game.player) {
            return;
        }
        let p = self.screen_to_cell(sx, sy);
        if self.game.play(p) {
            self.dirty = true;
            if self.is_ai_turn() {
                self.queue_ai_soon(120.0);
            }
        }
    }

    fn hit_btn(&self, btn: (f64, f64, f64, f64), sx: f64, sy: f64) -> bool {
        let (x, y, w, h) = btn;
        sx >= x && sx <= x + w && sy >= y && sy <= y + h
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
            let pan_cells = dx / self.cell_px.max(1.0);
            self.cam_x += pan_cells;
            self.dirty = true;
            return;
        }

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
                if self.is_ai_turn() {
                    self.queue_ai_soon(120.0);
                } else {
                    self.want_ai = false;
                }
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

        // HUD: controller pills only; no turn text.
        self.draw_controller_pills();

        // Winner overlay (centered) unchanged
        if let Some(winner) = self.game.winner {
            let msg = match winner {
                Color::Black => "You win!",
                Color::White => "AI wins!",
            };
            let sub = "Click or Press R to play again";

            let w2 = w / 2.0;
            let h2 = h / 2.0;

            self.ctx
                .set_font("bold 36px ui-sans-serif, system-ui, -apple-system");
            let msg_w = self
                .ctx
                .measure_text(msg)
                .ok()
                .map(|m| m.width())
                .unwrap_or(0.0);

            self.ctx
                .set_font("16px ui-sans-serif, system-ui, -apple-system");
            let sub_w = self
                .ctx
                .measure_text(sub)
                .ok()
                .map(|m| m.width())
                .unwrap_or(0.0);

            let pad = 24.0;
            let box_w = msg_w.max(sub_w) + pad * 2.0;
            let box_h = 36.0 + 8.0 + 16.0 + pad * 2.0;

            self.ctx.set_fill_style_str("rgba(0,0,0,0.55)");
            self.ctx
                .fill_rect(w2 - box_w / 2.0, h2 - box_h / 2.0, box_w, box_h);

            self.ctx.set_text_align("center");
            self.ctx.set_text_baseline("middle");

            self.ctx.set_fill_style_str("#e6edf3");
            self.ctx
                .set_font("bold 36px ui-sans-serif, system-ui, -apple-system");
            let _ = self.ctx.fill_text(msg, w2, h2 - 10.0);

            self.ctx.set_fill_style_str("#cbd5e1");
            self.ctx
                .set_font("16px ui-sans-serif, system-ui, -apple-system");
            let _ = self.ctx.fill_text(sub, w2, h2 + 24.0);

            self.ctx.set_text_align("left");
            self.ctx.set_text_baseline("alphabetic");
        }

        // Build timestamp HUD
        let ts = env!("BUILD_TS_UNIX");
        self.ctx.set_text_align("left");
        self.ctx.set_text_baseline("alphabetic");
        self.ctx
            .set_font("14px ui-sans-serif, system-ui, -apple-system");
        let _ = self.ctx.fill_text(ts, 12.0, h - 22.0);
    }

    /// Draw the Human/AI toggle pills and store their hitboxes.
    /// The pill for the **current turn** is highlighted with a bright outline.
    fn draw_controller_pills(&mut self) {
        let pad_x = 12.0;
        let y = 44.0;
        let gap = 10.0;
        let pill_h = 26.0;

        self.ctx
            .set_font("12px ui-sans-serif, system-ui, -apple-system");

        let fmt = |c: Controller| match c {
            Controller::Human => "Human",
            Controller::AI => "AI",
        };

        let b_label = format!("Black: {}", fmt(self.ctrl_black));
        let w_label = format!("White: {}", fmt(self.ctrl_white));

        let b_w = self
            .ctx
            .measure_text(&b_label)
            .ok()
            .map(|m| m.width())
            .unwrap_or(80.0)
            + 20.0;
        let w_w = self
            .ctx
            .measure_text(&w_label)
            .ok()
            .map(|m| m.width())
            .unwrap_or(80.0)
            + 20.0;

        let x0 = pad_x;
        let x1 = x0 + b_w + gap;

        self.btn_black = (x0, y - pill_h + 8.0, b_w, pill_h);
        self.btn_white = (x1, y - pill_h + 8.0, w_w, pill_h);

        // Helper: draw pill with fill driven by controller, and outline if current player's pill.
        let draw_pill = |x: f64, text: &str, is_current: bool, is_ai: bool, w: f64| {
            // Fill indicates Human/AI (subtle)
            self.ctx
                .set_fill_style_str(if is_ai { "#111827" } else { "#1f2937" });
            self.ctx.begin_path();
            let r = 13.0;
            let y0 = y - pill_h + 8.0;
            let x1 = x + w;
            let y1 = y0 + pill_h;
            self.ctx.move_to(x + r, y0);
            self.ctx.line_to(x1 - r, y0);
            let _ = self
                .ctx
                .arc(x1 - r, y0 + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
            self.ctx.line_to(x1, y1 - r);
            let _ = self
                .ctx
                .arc(x1 - r, y1 - r, r, 0.0, std::f64::consts::FRAC_PI_2);
            self.ctx.line_to(x + r, y1);
            let _ = self
                .ctx
                .arc(x + r, y1 - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
            self.ctx.line_to(x, y0 + r);
            let _ = self
                .ctx
                .arc(x + r, y0 + r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
            self.ctx.close_path();
            self.ctx.fill();

            // Outline: bright if current turn, muted otherwise.
            if is_current {
                self.ctx.set_stroke_style_str("#38bdf8"); // highlight
                self.ctx.set_line_width(2.0);
            } else {
                self.ctx.set_stroke_style_str("#374151");
                self.ctx.set_line_width(1.0);
            }
            self.ctx.stroke();

            self.ctx.set_fill_style_str("#e5e7eb");
            self.ctx.set_text_align("left");
            self.ctx.set_text_baseline("alphabetic");
            let _ = self.ctx.fill_text(text, x + 10.0, y);
        };

        draw_pill(
            x0,
            &b_label,
            self.game.player == Color::Black,
            self.ctrl_black == Controller::AI,
            b_w,
        );
        draw_pill(
            x1,
            &w_label,
            self.game.player == Color::White,
            self.ctrl_white == Controller::AI,
            w_w,
        );
    }
}
