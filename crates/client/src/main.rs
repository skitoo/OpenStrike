//! OpenStrike: the imperative shell. It reads devices into plain values, feeds commands to the
//! pure simulation at a fixed rate, and draws the result. No game logic lives here.

mod assets;
mod camera;
mod scene;

use std::collections::HashMap;
use std::path::PathBuf;

use openstrike_formats::rhomb::{Rect, rasterize_ground};
use openstrike_formats::sprite::Image as Sprite;
use openstrike_sim::{Command, Direction, Order, SUBPIXEL, TICKS_PER_SECOND, Unit, UnitId, Vec2, World, step};
use raylib::prelude::*;

use crate::assets::Assets;
use crate::camera::{Camera, CameraInput};
use crate::scene::{ObjectPart, UnitView};

const TANK: UnitId = UnitId(1);
const SCREENSHOT_FRAMES: u32 = 200;
/// Ground texture tiles, in screen pixels.
const CHUNK: i32 = 512;

struct Options {
    data: PathBuf,
    map: PathBuf,
    climate: String,
    screenshot: Option<PathBuf>,
    /// Map pixel to centre the camera on at start (defaults to the tank).
    look_at: Option<(i32, i32)>,
    zoom: f32,
}

fn options() -> Options {
    let mut args = std::env::args().skip(1);
    let mut o = Options {
        data: PathBuf::from("data/SuddenStrikeGold"),
        map: PathBuf::new(),
        climate: "summer".into(),
        screenshot: None,
        look_at: None,
        zoom: 1.0,
    };
    while let Some(a) = args.next() {
        match a.as_str() {
            "--data" => o.data = args.next().map(PathBuf::from).unwrap_or(o.data),
            "--climate" => o.climate = args.next().unwrap_or(o.climate),
            "--screenshot" => o.screenshot = args.next().map(PathBuf::from),
            "--zoom" => o.zoom = args.next().and_then(|v| v.parse().ok()).unwrap_or(o.zoom),
            "--look-at" => {
                o.look_at = args.next().and_then(|v| {
                    let (x, y) = v.split_once(',')?;
                    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
                })
            }
            _ => o.map = PathBuf::from(a),
        }
    }
    if o.map.as_os_str().is_empty() {
        o.map = o.data.join("Maps/single/'Act of Sabotage'.ssm");
    }
    o
}

fn upload(rl: &mut RaylibHandle, thread: &RaylibThread, w: u32, h: u32, rgba: &[u8]) -> Texture2D {
    let image = Image::gen_image_color(w as i32, h as i32, Color::BLANK);
    let mut texture = rl.load_texture_from_image(thread, &image).expect("texture upload");
    texture.update_texture(rgba).expect("texture size");
    texture.set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_POINT);
    texture
}

struct SpriteTexture {
    anchor: (i32, i32),
    size: (i32, i32),
    texture: Texture2D,
}

fn upload_sprite(rl: &mut RaylibHandle, thread: &RaylibThread, s: &Sprite) -> SpriteTexture {
    SpriteTexture {
        anchor: s.anchor,
        size: (s.width as i32, s.height as i32),
        texture: upload(rl, thread, s.width, s.height, &s.rgba),
    }
}

fn upload_strip(rl: &mut RaylibHandle, thread: &RaylibThread, sprites: &[Sprite]) -> Vec<SpriteTexture> {
    sprites.iter().map(|s| upload_sprite(rl, thread, s)).collect()
}

struct TankTextures {
    hull: Vec<SpriteTexture>,
    hull_shadow: Vec<SpriteTexture>,
    turret: Vec<SpriteTexture>,
    turret_shadow: Vec<SpriteTexture>,
    mounts: Vec<(i32, i32)>,
}

/// Screen rectangle currently in view.
fn view_rect(cam: &Camera, screen: (f32, f32)) -> Rect {
    let (x1, y1) = cam.screen_to_map(screen.0, screen.1);
    Rect { x: cam.x as i32, y: cam.y as i32, w: (x1 - cam.x) as i32 + 1, h: (y1 - cam.y) as i32 + 1 }
}

fn overlaps(view: Rect, x: i32, y: i32, w: i32, h: i32) -> bool {
    x < view.x + view.w && y < view.y + view.h && x + w > view.x && y + h > view.y
}

fn draw_sprite<D: RaylibDraw>(d: &mut D, s: &SpriteTexture, at: (i32, i32), view: Rect) {
    let (x, y) = (at.0 + s.anchor.0, at.1 + s.anchor.1);
    if overlaps(view, x, y, s.size.0, s.size.1) {
        d.draw_texture(&s.texture, x, y, Color::WHITE);
    }
}

fn draw_unit<D: RaylibDraw>(d: &mut D, tank: &TankTextures, u: &UnitView, view: Rect) {
    let mount = tank.mounts.get(u.frame).copied().unwrap_or_default();
    draw_sprite(d, &tank.hull[u.frame], u.screen, view);
    draw_sprite(d, &tank.turret[u.frame], (u.screen.0 + mount.0, u.screen.1 + mount.1), view);
}

fn read_input(rl: &RaylibHandle) -> CameraInput {
    let key = |k| if rl.is_key_down(k) { 1.0 } else { 0.0 };
    let scroll = (
        key(KeyboardKey::KEY_RIGHT) + key(KeyboardKey::KEY_D) - key(KeyboardKey::KEY_LEFT) - key(KeyboardKey::KEY_A),
        key(KeyboardKey::KEY_DOWN) + key(KeyboardKey::KEY_S) - key(KeyboardKey::KEY_UP) - key(KeyboardKey::KEY_W),
    );
    let drag = if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_MIDDLE) {
        let d = rl.get_mouse_delta();
        (d.x, d.y)
    } else {
        (0.0, 0.0)
    };
    let m = rl.get_mouse_position();
    CameraInput { scroll, drag, zoom_steps: rl.get_mouse_wheel_move(), zoom_anchor: (m.x, m.y) }
}

fn initial_world(assets: &Assets) -> World {
    let (w, h) = assets.map.pixel_size();
    let tank = Unit {
        id: TANK,
        kind: "t-26".into(),
        pos: Vec2::new(w as i32 / 2 * SUBPIXEL, h as i32 / 2 * SUBPIXEL),
        facing: Direction(0),
        speed: 3 * SUBPIXEL,
        order: Order::Idle,
    };
    World::new(Vec2::new(w as i32 * SUBPIXEL, h as i32 * SUBPIXEL), vec![tank])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts = options();
    let assets = assets::load(&opts.data, &opts.map, &opts.climate, "t-26")?;
    let proj = assets.projection;
    let (screen_w, screen_h) = proj.screen_size();
    let objects = scene::objects(&assets.map, &proj, &assets.object_sets);

    let (mut rl, thread) = raylib::init().size(1280, 800).title("OpenStrike").resizable().build();
    rl.set_target_fps(60);

    let tank = TankTextures {
        hull: upload_strip(&mut rl, &thread, &assets.tank.hull),
        hull_shadow: upload_strip(&mut rl, &thread, &assets.tank.hull_shadow),
        turret: upload_strip(&mut rl, &thread, &assets.tank.turret),
        turret_shadow: upload_strip(&mut rl, &thread, &assets.tank.turret_shadow),
        mounts: assets.tank.mounts.clone(),
    };
    // textures of the object parts this map uses
    let mut part_textures: HashMap<(usize, usize), Vec<SpriteTexture>> = HashMap::new();
    for o in objects.flat.iter().chain(&objects.shadows).chain(&objects.upright) {
        part_textures.entry((o.set, o.frame)).or_insert_with(|| {
            let parts = &assets.object_sets[o.set].frames[o.frame];
            parts.iter().map(|p| upload_sprite(&mut rl, &thread, &p.image)).collect()
        });
    }
    let mut ground: HashMap<(i32, i32), Texture2D> = HashMap::new();

    let mut world = initial_world(&assets);
    let tank_start = world.unit(TANK).map(|u| u.pos).unwrap_or_default();
    let (lx, ly) = opts.look_at.unwrap_or((tank_start.x / SUBPIXEL, tank_start.y / SUBPIXEL));
    let (tx, ty) = proj.to_screen(lx, ly);
    let mut cam = Camera { x: tx as f32 - 640.0 / opts.zoom, y: ty as f32 - 400.0 / opts.zoom, zoom: opts.zoom };
    let mut pending: Vec<Command> = Vec::new();
    if opts.screenshot.is_some() {
        pending.push(Command::Move { unit: TANK, to: tank_start + Vec2::new(-165 * SUBPIXEL, 325 * SUBPIXEL) });
    }
    let tick = 1.0 / TICKS_PER_SECOND as f32;
    let mut accumulator = 0.0f32;
    let mut frame = 0u32;

    while !rl.window_should_close() {
        let dt = rl.get_frame_time().min(0.25);
        let screen = (rl.get_screen_width() as f32, rl.get_screen_height() as f32);
        cam = camera::update(cam, &read_input(&rl), dt, (screen_w as f32, screen_h as f32), screen);
        if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_RIGHT) {
            let m = rl.get_mouse_position();
            let (sx, sy) = cam.screen_to_map(m.x, m.y);
            let (x, y) = proj.to_map(sx, sy);
            pending.push(Command::Move { unit: TANK, to: Vec2::new(x as i32 * SUBPIXEL, y as i32 * SUBPIXEL) });
        }
        accumulator += if opts.screenshot.is_some() { tick } else { dt };
        while accumulator >= tick {
            world = step(&world, &pending);
            pending.clear();
            accumulator -= tick;
        }

        let view = view_rect(&cam, screen);
        // rasterise the ground chunks that just came into view
        for cy in view.y.div_euclid(CHUNK)..=(view.y + view.h).div_euclid(CHUNK) {
            for cx in view.x.div_euclid(CHUNK)..=(view.x + view.w).div_euclid(CHUNK) {
                let outside = cx < 0 || cy < 0 || cx * CHUNK >= screen_w || cy * CHUNK >= screen_h;
                if outside || ground.contains_key(&(cx, cy)) {
                    continue;
                }
                let rect = Rect { x: cx * CHUNK, y: cy * CHUNK, w: CHUNK, h: CHUNK };
                let rgba = rasterize_ground(&assets.map, &assets.rhombs, &assets.rhomb_palettes, rect);
                ground.insert((cx, cy), upload(&mut rl, &thread, CHUNK as u32, CHUNK as u32, &rgba));
            }
        }
        let mut units: Vec<UnitView> = scene::units(&world, &proj);
        units.sort_by_key(|u| u.screen.1);

        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        {
            let target = Vector2::new(cam.x, cam.y);
            let camera2d = Camera2D { offset: Vector2::zero(), target, rotation: 0.0, zoom: cam.zoom };
            let mut m = d.begin_mode2D(camera2d);
            for ((cx, cy), texture) in &ground {
                if overlaps(view, cx * CHUNK, cy * CHUNK, CHUNK, CHUNK) {
                    m.draw_texture(texture, cx * CHUNK, cy * CHUNK, Color::WHITE);
                }
            }
            let part = |p: &ObjectPart| &part_textures[&(p.set, p.frame)][p.part];
            for p in objects.flat.iter().chain(&objects.shadows) {
                draw_sprite(&mut m, part(p), p.screen, view);
            }
            for u in &units {
                let mount = tank.mounts.get(u.frame).copied().unwrap_or_default();
                draw_sprite(&mut m, &tank.hull_shadow[u.frame], u.screen, view);
                draw_sprite(&mut m, &tank.turret_shadow[u.frame], (u.screen.0 + mount.0, u.screen.1 + mount.1), view);
            }
            // upright objects and units, back to front
            let mut units_left = units.iter().peekable();
            for p in &objects.upright {
                while let Some(u) = units_left.next_if(|u| u.screen.1 <= p.screen.1) {
                    draw_unit(&mut m, &tank, u, view);
                }
                draw_sprite(&mut m, part(p), p.screen, view);
            }
            for u in units_left {
                draw_unit(&mut m, &tank, u, view);
            }
        }
        d.draw_text("Right click: move   Arrows/WASD or middle drag: scroll   Wheel: zoom", 10, 10, 20, Color::RAYWHITE);
        d.draw_fps(10, 36);
        drop(d);

        frame += 1;
        if let Some(path) = &opts.screenshot
            && frame == SCREENSHOT_FRAMES
        {
            rl.load_image_from_screen(&thread).export_image(&path.to_string_lossy());
            break;
        }
    }
    Ok(())
}
