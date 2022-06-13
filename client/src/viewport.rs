use crate::{
    bridge::{Context, EventType, JsError, MouseButton},
    client::Client,
    interactor::Interactor,
};
use scene::{Rect, ScenePoint};

#[derive(Clone, Copy, Debug)]
pub struct ViewportPoint {
    x: f32,
    y: f32,
}

impl ViewportPoint {
    pub fn new(x: i32, y: i32) -> Self {
        ViewportPoint {
            x: x as f32,
            y: y as f32,
        }
    }

    fn scene_point(&self, viewport: Rect, grid_zoom: f32) -> ScenePoint {
        ScenePoint::new(
            (self.x / grid_zoom) + viewport.x,
            (self.y / grid_zoom) + viewport.y,
        )
    }
}

pub struct Viewport {
    pub scene: Interactor,

    context: Context,

    // Measured in scene units (tiles)
    viewport: Rect,

    // Size to render a scene unit, in pixels
    grid_zoom: f32,

    // Current grab for dragging on the viewport
    grabbed_at: Option<ViewportPoint>,

    // Flag set true whenever something changes
    redraw_needed: bool,
}

impl Viewport {
    const BASE_GRID_ZOOM: f32 = 50.0;

    pub fn new(client: Option<Client>) -> Result<Self, JsError> {
        let mut vp = Viewport {
            context: Context::new()?,
            scene: Interactor::new(client),
            viewport: Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            grid_zoom: Viewport::BASE_GRID_ZOOM,
            grabbed_at: None,
            redraw_needed: true,
        };

        vp.update_viewport();
        vp.centre_viewport();

        Ok(vp)
    }

    fn update_viewport(&mut self) {
        let (w, h) = self.context.viewport_size();
        let w = w as f32 / self.grid_zoom;
        let h = h as f32 / self.grid_zoom;

        if w != self.viewport.w || h != self.viewport.h {
            self.viewport = Rect {
                x: self.viewport.x,
                y: self.viewport.y,
                w,
                h,
            };
            self.redraw_needed = true;
        }
    }

    fn centre_viewport(&mut self) {
        let scene_size = self.scene.dimensions();
        self.viewport.x = (scene_size.w / 2.0 - self.viewport.w / 2.0).round();
        self.viewport.y = (scene_size.h / 2.0 - self.viewport.h / 2.0).round();
        self.redraw_needed = true;
    }

    fn grab(&mut self, at: ViewportPoint) {
        if self.grabbed_at.is_none() {
            self.grabbed_at = Some(at);
        }
    }

    fn handle_mouse_down(&mut self, at: ViewportPoint, button: MouseButton) {
        match button {
            MouseButton::Left => self
                .scene
                .grab(at.scene_point(self.viewport, self.grid_zoom)),
            MouseButton::Right => self.grab(at),
            _ => {}
        };
    }

    fn release_grab(&mut self) {
        self.grabbed_at = None;
    }

    fn handle_mouse_up(&mut self, alt: bool, button: MouseButton) {
        match button {
            MouseButton::Left => self.scene.release(!alt),
            MouseButton::Right => self.release_grab(),
            MouseButton::Middle => self.centre_viewport(),
            _ => {}
        };
    }

    fn handle_mouse_move(&mut self, at: ViewportPoint) {
        self.scene
            .drag(at.scene_point(self.viewport, self.grid_zoom));
        if let Some(from) = self.grabbed_at {
            self.viewport.x += (from.x - at.x) / self.grid_zoom;
            self.viewport.y += (from.y - at.y) / self.grid_zoom;
            self.grabbed_at = Some(at);
            self.redraw_needed = true;
        }
    }

    fn handle_scroll(&mut self, at: ViewportPoint, delta: f32, shift: bool, ctrl: bool) {
        const SCROLL_COEFFICIENT: f32 = 0.5;
        const ZOOM_COEFFICIENT: f32 = 3.0 / Viewport::BASE_GRID_ZOOM;
        const ZOOM_MIN: f32 = Viewport::BASE_GRID_ZOOM / 2.0;
        const ZOOM_MAX: f32 = Viewport::BASE_GRID_ZOOM * 5.0;

        // We want shift + scroll to scroll horizontally but browsers (Firefox
        // anyway) only do this when the page is wider than the viewport, which
        // it never is in this case. Thus this check for shift. Likewise for
        // ctrl + scroll and zooming.
        if shift {
            self.viewport.x += SCROLL_COEFFICIENT * delta / self.grid_zoom;
        } else if ctrl {
            // Need to calculate these before changing the zoom level
            let scene_point = at.scene_point(self.viewport, self.grid_zoom);
            let fraction_x = at.x / (self.viewport.w * self.grid_zoom);
            let fraction_y = at.y / (self.viewport.h * self.grid_zoom);

            // Zoom in
            self.grid_zoom = (self.grid_zoom - ZOOM_COEFFICIENT * delta).clamp(ZOOM_MIN, ZOOM_MAX);
            self.update_viewport();

            // Update viewport such that the mouse is at the same scene
            // coordinate as before zooming.
            self.viewport.x = scene_point.x - self.viewport.w * fraction_x;
            self.viewport.y = scene_point.y - self.viewport.h * fraction_y;
        } else {
            self.viewport.y += SCROLL_COEFFICIENT * delta / self.grid_zoom;
        }

        self.redraw_needed = true;

        // Update the held object details for the scene for the new cursor
        // position.
        self.scene
            .drag(at.scene_point(self.viewport, self.grid_zoom));
    }

    fn process_ui_events(&mut self) {
        let events = match self.context.events() {
            Some(e) => e,
            None => return,
        };

        for event in &events {
            match event.event_type {
                EventType::MouseDown => self.handle_mouse_down(event.at, event.button),
                EventType::MouseLeave => self.handle_mouse_up(event.alt, event.button),
                EventType::MouseMove => self.handle_mouse_move(event.at),
                EventType::MouseUp => self.handle_mouse_up(event.alt, event.button),
                EventType::MouseWheel(delta) => {
                    self.handle_scroll(event.at, delta, event.shift, event.ctrl)
                }
            };
        }
    }

    fn redraw(&mut self) {
        let vp = Rect::scaled_from(self.viewport, self.grid_zoom);

        self.context.clear(vp);

        let mut background_drawn = false;
        for layer in self.scene.layers().iter().rev() {
            if !background_drawn && layer.z >= 0 {
                self.context
                    .draw_grid(vp, self.scene.dimensions(), self.grid_zoom);
                background_drawn = true;
            }

            if layer.visible {
                self.context
                    .draw_sprites(vp, &layer.sprites, self.grid_zoom);
            }
        }

        if !background_drawn {
            self.context
                .draw_grid(vp, self.scene.dimensions(), self.grid_zoom);
        }

        for rect in self.scene.selections() {
            self.context
                .draw_outline(vp, Rect::scaled_from(rect, self.grid_zoom));
        }
    }

    pub fn animation_frame(&mut self) {
        self.process_ui_events();
        self.scene.process_server_events();
        self.update_viewport();
        if self.redraw_needed || self.context.load_texture_queue() || self.scene.handle_change() {
            self.redraw();
            self.redraw_needed = false;
        }
    }
}
