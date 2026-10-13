//! OpenStrike: the imperative shell. It reads devices into plain values, feeds commands to the
//! pure simulation at a fixed rate, and draws the result. No game logic lives here.

mod assets;
mod camera;

use std::path::PathBuf;

use openstrike_formats::hot::frame_for_heading;
use openstrike_formats::rhomb::Rect;
use openstrike_formats::sprite::Image as Sprite;
use openstrike_sim::{Command, Direction, Order, SUBPIXEL, TICKS_PER_SECOND, Unit, UnitId, Vec2, World, step};
use raylib::prelude::*;

use crate::assets::Assets;
use crate::camera::{Camera, CameraInput};

const TANK: UnitId = UnitId(1);
const SCREENSHOT_FRAMES: u32 = 120;

struct Options {
    data: PathBuf,
    map: PathBuf,
    climate: String,
    screenshot: Option<PathBuf>,
}

fn options() -> Options {
    let mut args = std::env::args().skip(1);
    let mut o = Options {
        data: PathBuf::from("data/SuddenStrikeGold"),
        map: PathBuf::new(),
        climate: "summer".into(),
        screenshot: None,
    };
    while let Some(a) = args.next() {
        match a.as_str() {
            "--data" => o.data = args.next().map(PathBuf::from).unwrap_or(o.data),
            "--climate" => o.climate = args.next().unwrap_or(o.climate),
            "--screenshot" => o.screenshot = args.next().map(PathBuf::from),
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
    texture: Texture2D,
}

struct TankTextures {
    hull: Vec<SpriteTexture>,
    hull_shadow: Vec<SpriteTexture>,
    turret: Vec<SpriteTexture>,
    turret_shadow: Vec<SpriteTexture>,
    mounts: Vec<(i32, i32)>,
}

fn upload_sprites(rl: &mut RaylibHandle, thread: &RaylibThread, sprites: &[Sprite]) -> Vec<SpriteTexture> {
    sprites
        .iter()
        .map(|s| SpriteTexture { anchor: s.anchor, texture: upload(rl, thread, s.width, s.height, &s.rgba) })
        .collect()
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

fn visible(rect: Rect, cam: &Camera, screen: (f32, f32)) -> bool {
    let (x1, y1) = cam.screen_to_map(screen.0, screen.1);
    (rect.x as f32) < x1 && (rect.y as f32) < y1 && ((rect.x + rect.w) as f32) > cam.x && ((rect.y + rect.h) as f32) > cam.y
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts = options();
    let assets = assets::load(&opts.data, &opts.map, &opts.climate, "t-26")?;
    let (map_w, map_h) = assets.map.pixel_size();

    let (mut rl, thread) = raylib::init().size(1280, 800).title("OpenStrike").resizable().build();
    rl.set_target_fps(60);
    let ground: Vec<(Rect, Texture2D)> = assets
        .ground
        .iter()
        .map(|c| (c.rect, upload(&mut rl, &thread, c.rect.w as u32, c.rect.h as u32, &c.rgba)))
        .collect();
    let tank = TankTextures {
        hull: upload_sprites(&mut rl, &thread, &assets.tank.hull),
        hull_shadow: upload_sprites(&mut rl, &thread, &assets.tank.hull_shadow),
        turret: upload_sprites(&mut rl, &thread, &assets.tank.turret),
        turret_shadow: upload_sprites(&mut rl, &thread, &assets.tank.turret_shadow),
        mounts: assets.tank.mounts.clone(),
    };

    let mut world = initial_world(&assets);
    let tank_start = world.unit(TANK).map(|u| u.pos).unwrap_or_default();
    let mut cam = Camera { x: tank_start.x as f32 / SUBPIXEL as f32 - 640.0, y: tank_start.y as f32 / SUBPIXEL as f32 - 400.0, zoom: 1.0 };
    let mut pending: Vec<Command> = Vec::new();
    if opts.screenshot.is_some() {
        let to = tank_start + Vec2::new(-200 * SUBPIXEL, -60 * SUBPIXEL);
        pending.push(Command::Move { unit: TANK, to });
    }
    let tick = 1.0 / TICKS_PER_SECOND as f32;
    let mut accumulator = 0.0f32;
    let mut frame = 0u32;

    while !rl.window_should_close() {
        let dt = rl.get_frame_time().min(0.25);
        let screen = (rl.get_screen_width() as f32, rl.get_screen_height() as f32);
        cam = camera::update(cam, &read_input(&rl), dt, (map_w as f32, map_h as f32), screen);
        if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_RIGHT) {
            let m = rl.get_mouse_position();
            let (x, y) = cam.screen_to_map(m.x, m.y);
            pending.push(Command::Move { unit: TANK, to: Vec2::new(x as i32 * SUBPIXEL, y as i32 * SUBPIXEL) });
        }
        accumulator += if opts.screenshot.is_some() { tick } else { dt };
        while accumulator >= tick {
            world = step(&world, &pending);
            pending.clear();
            accumulator -= tick;
        }

        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        {
            let camera2d = Camera2D { offset: Vector2::zero(), target: Vector2::new(cam.x, cam.y), rotation: 0.0, zoom: cam.zoom };
            let mut m = d.begin_mode2D(camera2d);
            for (rect, texture) in ground.iter().filter(|(r, _)| visible(*r, &cam, screen)) {
                m.draw_texture(texture, rect.x, rect.y, Color::WHITE);
            }
            for unit in &world.units {
                let (x, y) = (unit.pos.x / SUBPIXEL, unit.pos.y / SUBPIXEL);
                // the turret follows the hull until turrets get their own heading in the simulation
                let k = frame_for_heading(unit.facing.0);
                let (mx, my) = tank.mounts.get(k).copied().unwrap_or_default();
                for (sprite, (dx, dy)) in [
                    (&tank.hull_shadow[k], (0, 0)),
                    (&tank.turret_shadow[k], (mx, my)),
                    (&tank.hull[k], (0, 0)),
                    (&tank.turret[k], (mx, my)),
                ] {
                    m.draw_texture(&sprite.texture, x + dx + sprite.anchor.0, y + dy + sprite.anchor.1, Color::WHITE);
                }
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
