//! Camera: a pure value updated from the frame's input.

/// What the player did to the camera this frame, already read from the devices by the shell.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CameraInput {
    /// Keyboard / screen-edge scrolling, each axis in -1..=1.
    pub scroll: (f32, f32),
    /// Mouse drag this frame, in screen pixels.
    pub drag: (f32, f32),
    /// Wheel steps this frame.
    pub zoom_steps: f32,
    /// Screen point the zoom is anchored on.
    pub zoom_anchor: (f32, f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Map pixel at the top-left of the screen.
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

pub const SCROLL_SPEED: f32 = 900.0;
pub const MIN_ZOOM: f32 = 0.5;
pub const MAX_ZOOM: f32 = 3.0;

impl Camera {
    pub fn screen_to_map(&self, sx: f32, sy: f32) -> (f32, f32) {
        (self.x + sx / self.zoom, self.y + sy / self.zoom)
    }
}

/// Next camera from the previous one, the input, the frame time, and the map and screen sizes.
pub fn update(cam: Camera, input: &CameraInput, dt: f32, map: (f32, f32), screen: (f32, f32)) -> Camera {
    let zoom = (cam.zoom * 1.15f32.powf(input.zoom_steps)).clamp(MIN_ZOOM, MAX_ZOOM);
    // keep the map point under the anchor fixed while zooming
    let (ax, ay) = cam.screen_to_map(input.zoom_anchor.0, input.zoom_anchor.1);
    let x = ax - input.zoom_anchor.0 / zoom + input.scroll.0 * SCROLL_SPEED * dt / zoom - input.drag.0 / zoom;
    let y = ay - input.zoom_anchor.1 / zoom + input.scroll.1 * SCROLL_SPEED * dt / zoom - input.drag.1 / zoom;
    let (max_x, max_y) = ((map.0 - screen.0 / zoom).max(0.0), (map.1 - screen.1 / zoom).max(0.0));
    Camera { x: x.clamp(0.0, max_x), y: y.clamp(0.0, max_y), zoom }
}
