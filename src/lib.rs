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

    let win = window().unwrap();
    let doc = win.document().unwrap();
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

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Cell {
    Empty,
    Black,
    White,
}
impl Cell {
    fn other(self) -> Cell {
        match self {
            Cell::Black => Cell::White,
            Cell::White => Cell::Black,
            _ => Cell::Empty,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
struct Pt {
    x: i32,
    y: i32,
}

#[derive(Clone)]
struct Game {
    cells: HashMap<(i32, i32), Cell>,
    player: Cell,
    winner: Cell,
    last_move: Option<Pt>,
    frontier: HashSet<(i32, i32)>,
}

impl Game {
    fn new() -> Self {
        let mut g = Self {
            cells: HashMap::new(),
            player: Cell::Black,
            winner: Cell::Empty,
            last_move: None,
            frontier: HashSet::new(),
        };
        g.rebuild_frontier();
        g
    }

    fn reset(&mut self) {
        self.cells.clear();
        self.player = Cell::Black;
        self.winner = Cell::Empty;
        self.last_move = None;
        self.frontier.clear();
        self.rebuild_frontier();
    }

    fn rebuild_frontier(&mut self) {
        if self.cells.is_empty() {
            for dx in -2..=2 {
                for dy in -2..=2 {
                    if dx != 0 || dy != 0 {
                        self.frontier.insert((dx, dy));
                    }
                }
            }
            self.frontier.insert((0, 0));
            return;
        }
        self.frontier.clear();
        for (&(x, y), &c) in self.cells.iter() {
            if c == Cell::Empty {
                continue;
            }
            for dx in -2..=2 {
                for dy in -2..=2 {
                    let p = (x + dx, y + dy);
                    if !self.cells.contains_key(&p) {
                        self.frontier.insert(p);
                    }
                }
            }
        }
    }

    fn get(&self, x: i32, y: i32) -> Cell {
        *self.cells.get(&(x, y)).unwrap_or(&Cell::Empty)
    }
    fn set(&mut self, x: i32, y: i32, c: Cell) {
        if c == Cell::Empty {
            self.cells.remove(&(x, y));
        } else {
            self.cells.insert((x, y), c);
        }
    }

    fn playable(&self, x: i32, y: i32) -> bool {
        self.winner == Cell::Empty && self.get(x, y) == Cell::Empty
    }

    fn play(&mut self, x: i32, y: i32) -> bool {
        if !self.playable(x, y) {
            return false;
        }
        self.set(x, y, self.player);
        self.last_move = Some(Pt { x, y });
        if self.check_win(x, y, self.player) {
            self.winner = self.player;
        }
        self.player = self.player.other();
        self.rebuild_frontier();
        true
    }

    fn dirs() -> &'static [(i32, i32); 4] {
        &[(1, 0), (0, 1), (1, 1), (1, -1)]
    }

    fn check_win(&self, x: i32, y: i32, who: Cell) -> bool {
        for &(dx, dy) in Self::dirs() {
            let mut count = 1;
            count += self.ray(x, y, dx, dy, who);
            count += self.ray(x, y, -dx, -dy, who);
            if count >= 5 {
                return true;
            }
        }
        false
    }

    fn ray(&self, x: i32, y: i32, dx: i32, dy: i32, who: Cell) -> i32 {
        let mut c = 0;
        let mut cx = x + dx;
        let mut cy = y + dy;
        while self.get(cx, cy) == who {
            c += 1;
            cx += dx;
            cy += dy;
        }
        c
    }

    fn score_point(&self, x: i32, y: i32, who: Cell) -> i32 {
        if self.get(x, y) != Cell::Empty {
            return i32::MIN / 4;
        }
        let mut s = 0;
        for &(dx, dy) in Self::dirs() {
            let (a, b) = self.line_len_open(x, y, dx, dy, who);
            let len = a + 1 + b;
            let open = self.open_ends(x, y, dx, dy, who);
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
            let (oa, ob) = self.line_len_open(x, y, dx, dy, opp);
            let olen = oa + 1 + ob;
            let oopen = self.open_ends(x, y, dx, dy, opp);
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

    fn line_len_open(&self, x: i32, y: i32, dx: i32, dy: i32, who: Cell) -> (i32, i32) {
        let mut a = 0;
        let mut cx = x + dx;
        let mut cy = y + dy;
        while self.get(cx, cy) == who {
            a += 1;
            cx += dx;
            cy += dy;
        }
        let mut b = 0;
        let mut cx2 = x - dx;
        let mut cy2 = y - dy;
        while self.get(cx2, cy2) == who {
            b += 1;
            cx2 -= dx;
            cy2 -= dy;
        }
        (a, b)
    }

    fn open_ends(&self, x: i32, y: i32, dx: i32, dy: i32, who: Cell) -> i32 {
        let mut open = 0;
        // forward
        let mut cx = x + dx;
        let mut cy = y + dy;
        while self.get(cx, cy) == who {
            cx += dx;
            cy += dy;
        }
        if self.get(cx, cy) == Cell::Empty {
            open += 1;
        }
        // backward
        let mut cx2 = x - dx;
        let mut cy2 = y - dy;
        while self.get(cx2, cy2) == who {
            cx2 -= dx;
            cy2 -= dy;
        }
        if self.get(cx2, cy2) == Cell::Empty {
            open += 1;
        }
        open
    }

    fn best_move(&self, who: Cell) -> Option<(i32, i32, i32)> {
        let mut best: Option<(i32, i32, i32)> = None;
        for &(x, y) in self.frontier.iter() {
            let sc = self.score_point(x, y, who);
            if let Some((_, _, bs)) = best {
                if sc > bs {
                    best = Some((x, y, sc));
                }
            } else {
                best = Some((x, y, sc));
            }
        }
        best
    }
}

struct App {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    game: Game,
    cell_px: f64, // size of one cell in logical (CSS) px
    cam_x: f64,
    cam_y: f64,
    view_w: f64, // canvas logical width (CSS px)
    view_h: f64, // canvas logical height (CSS px)
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
        // Compute CSS (logical) size
        let rect = self
            .canvas
            .unchecked_ref::<Element>()
            .get_bounding_client_rect();
        let css_w = rect.width();
        let css_h = rect.height();
        self.view_w = css_w;
        self.view_h = css_h;

        // Backing store size in device pixels
        let dpr = window().unwrap().device_pixel_ratio();
        self.canvas.set_width((css_w * dpr) as u32);
        self.canvas.set_height((css_h * dpr) as u32);

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
    fn screen_to_cell(&self, sx: f64, sy: f64) -> (i32, i32) {
        let (x, y) = self.screen_to_cell_f64(sx, sy);
        (x.round() as i32, y.round() as i32)
    }

    fn on_pointer_down(&mut self, e: PointerEvent) {
        // Logical pointer coords
        let rect = self
            .canvas
            .unchecked_ref::<Element>()
            .get_bounding_client_rect();
        let sx = e.client_x() as f64 - rect.left();
        let sy = e.client_y() as f64 - rect.top();

        let (x, y) = self.screen_to_cell(sx, sy);
        if self.game.play(x, y) {
            self.dirty = true;
            if self.game.winner == Cell::Empty {
                if let Some((ax, ay, _)) = self.game.best_move(self.game.player) {
                    self.game.play(ax, ay);
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
        for (&(x, y), &c) in self.game.cells.iter() {
            let sx = (x as f64 - self.cam_x) * self.cell_px + w / 2.0;
            let sy = (y as f64 - self.cam_y) * self.cell_px + h / 2.0;
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
                Cell::Black => self.ctx.set_fill_style_str("#e6edf3"),
                Cell::White => self.ctx.set_fill_style_str("#38bdf8"),
                _ => {}
            }
            self.ctx.fill();
        }

        // HUD
        self.ctx.set_fill_style_str("#e5e7eb");
        self.ctx
            .set_font("14px ui-sans-serif, system-ui, -apple-system");
        let turn = match self.game.player {
            Cell::Black => "Your turn (X)",
            Cell::White => "AI thinking (O)",
            _ => "",
        };
        let status = if self.game.winner == Cell::Empty {
            turn
        } else if self.game.winner == Cell::Black {
            "You win! Press R to restart."
        } else {
            "AI wins! Press R to restart."
        };
        let _ = self.ctx.fill_text(status, 12.0, 22.0);
    }
}
