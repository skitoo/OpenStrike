//! OpenStrike: the imperative shell. It reads devices into plain values, feeds commands to the
//! pure simulation at a fixed rate, and draws the result. No game logic lives here.


use std::collections::HashMap;

use openstrike_formats::rhomb::{Rect, rasterize_ground};
use openstrike_formats::sprite::Image as Sprite;
use openstrike_sim::grid::{CELL_PX, NO_VEHICLES, QUARTERS};
use openstrike_sim::{Command, Grid, Rules, SUBPIXEL, TICKS_PER_SECOND, Unit, UnitId, World, step};
use raylib::prelude::*;

use openstrike::assets;
use openstrike::camera::{self, Camera, CameraInput};
use openstrike::scene::{self, ObjectPart, UnitView};
use openstrike::chunks::{self, CHUNK, MAX_NEW_PER_FRAME};
use openstrike::options::{self, Options, Parsed};
use openstrike::{selection, terrain};

/// The demo platoon, by description name; vehicles that fail to load are skipped.
const PLATOON: [&str; 6] = ["t-26", "t-34", "kv-1", "tiger", "panther", "sherman5"];
/// Quarter of a cell, for the blocked overlay.
const QUARTER: i32 = CELL_PX / 2;
const SCREENSHOT_FRAMES: u32 = 200;

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
    // an empty sprite still gets a 1x1 transparent texture, so that part indices stay aligned
    let (w, h, rgba) = if s.width == 0 || s.height == 0 { (1, 1, &[0u8; 4][..]) } else { (s.width, s.height, &s.rgba[..]) };
    SpriteTexture { anchor: s.anchor, size: (w as i32, h as i32), texture: upload(rl, thread, w, h, rgba) }
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
    draw_sprite(d, &tank.turret[u.turret_frame], (u.screen.0 + mount.0, u.screen.1 + mount.1), view);
}

fn draw_unit_shadow<D: RaylibDraw>(d: &mut D, tank: &TankTextures, u: &UnitView, view: Rect) {
    let mount = tank.mounts.get(u.frame).copied().unwrap_or_default();
    draw_sprite(d, &tank.hull_shadow[u.frame], u.screen, view);
    draw_sprite(d, &tank.turret_shadow[u.turret_frame], (u.screen.0 + mount.0, u.screen.1 + mount.1), view);
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

/// The platoon starts on free cells around the middle of the map, two cells apart, facing east.
fn initial_world(grid: &Grid, count: usize) -> World {
    let (mx, my) = ((grid.width / 2) as i16, (grid.height / 2) as i16);
    let free = |(x, y): (i16, i16)| grid.at(x as i32, y as i32) & (QUARTERS | NO_VEHICLES) == 0;
    let cells: Vec<(i16, i16)> = (0..32i16)
        .flat_map(|r| (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| (mx + 2 * dx, my + 2 * dy))))
        .filter(|&c| free(c))
        .fold(Vec::new(), |mut acc, c| {
            if !acc.contains(&c) {
                acc.push(c);
            }
            acc
        });
    let units = cells.into_iter().take(count).enumerate().map(|(i, cell)| Unit::new(UnitId(i as u32 + 1), i, cell, 0)).collect();
    World::new(units, 0)
}

/// Cell under a map pixel.
fn cell_at(x: i32, y: i32) -> (i16, i16) {
    (x.div_euclid(CELL_PX) as i16, y.div_euclid(CELL_PX) as i16)
}

fn main() {
    let opts = match options::parse(std::env::args().skip(1)) {
        Ok(Parsed::Run(o)) => o,
        Ok(Parsed::Help) => {
            println!("{}", options::USAGE);
            return;
        }
        Err(e) => {
            eprintln!("openstrike: {e}");
            std::process::exit(2);
        }
    };
    if let Err(e) = options::check(&opts).map_err(|e| e.into()).and_then(|_| run(&opts)) {
        eprintln!("openstrike: {e}");
        std::process::exit(1);
    }
}

fn run(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    let assets = assets::load(&opts.data, &opts.map, &opts.climate, &PLATOON)?;
    if assets.vehicles.is_empty() {
        return Err("no vehicle could be loaded".into());
    }
    let proj = assets.projection;
    let (screen_w, screen_h) = proj.screen_size();
    let objects = scene::objects(&assets.map, &proj, &assets.object_sets);
    let rules = Rules {
        grid: terrain::build(&assets.map),
        curves: assets.curves.clone(),
        types: assets.vehicles.iter().map(|v| v.unit_type.clone()).collect(),
    };
    let mut show_blocked = opts.show_blocked;

    let (mut rl, thread) = raylib::init().size(1280, 800).title("OpenStrike").resizable().build();
    rl.set_target_fps(60);

    let vehicles: Vec<TankTextures> = assets
        .vehicles
        .iter()
        .map(|v| {
            let s = &v.sprites;
            TankTextures {
                hull: upload_strip(&mut rl, &thread, &s.hull),
                hull_shadow: upload_strip(&mut rl, &thread, &s.hull_shadow),
                turret: upload_strip(&mut rl, &thread, &s.turret),
                turret_shadow: upload_strip(&mut rl, &thread, &s.turret_shadow),
                mounts: s.mounts.clone(),
            }
        })
        .collect();
    // textures of the object parts this map uses
    let mut part_textures: HashMap<(usize, usize, usize), Vec<SpriteTexture>> = HashMap::new();
    for o in objects.flat.iter().chain(&objects.shadows).chain(&objects.upright) {
        part_textures.entry((o.set, o.frame, o.state)).or_insert_with(|| {
            let parts = assets.object_sets[o.set].frames[o.frame].parts(o.state);
            parts.iter().map(|p| upload_sprite(&mut rl, &thread, &p.image)).collect()
        });
    }
    let mut ground: HashMap<(i32, i32), Texture2D> = HashMap::new();

    let mut world = initial_world(&rules.grid, assets.vehicles.len());
    let first = world.units[0].position();
    let (lx, ly) = opts.look_at.unwrap_or((first.0 / SUBPIXEL, first.1 / SUBPIXEL));
    let (tx, ty) = proj.to_screen(lx, ly);
    let mut cam = Camera { x: tx as f32 - 640.0 / opts.zoom, y: ty as f32 - 400.0 / opts.zoom, zoom: opts.zoom };
    let mut selected: Vec<UnitId> = Vec::new();
    let mut press: Option<(f32, f32)> = None;
    let mut pending: Vec<Command> = Vec::new();
    if opts.screenshot.is_some() {
        selected = world.units.iter().map(|u| u.id).collect();
        pending.push(Command::Move { units: selected.clone(), to: cell_at(first.0 / SUBPIXEL - 165, first.1 / SUBPIXEL + 325) });
    }
    // shadows are drawn into a mask, then applied once per pixel (see `sprite::shade`)
    let mut shadow_mask: Option<RenderTexture2D> = None;
    let tick = 1.0 / TICKS_PER_SECOND as f32;
    let mut accumulator = 0.0f32;
    let mut frame = 0u32;

    while !rl.window_should_close() {
        let dt = rl.get_frame_time().min(0.25);
        let screen = (rl.get_screen_width() as f32, rl.get_screen_height() as f32);
        cam = camera::update(cam, &read_input(&rl), dt, (screen_w as f32, screen_h as f32), screen);
        let mouse = rl.get_mouse_position();
        let mouse_world = cam.screen_to_map(mouse.x, mouse.y);
        let mut units: Vec<UnitView> = scene::units(&world, &proj);
        units.sort_by_key(|u| u.screen.1);

        if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
            press = Some(mouse_world);
        }
        if rl.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT)
            && let Some(start) = press.take()
        {
            let add = rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT) || rl.is_key_down(KeyboardKey::KEY_RIGHT_SHIFT);
            selected = selection::select(&selected, &units, start, mouse_world, add);
        }
        if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_RIGHT) && !selected.is_empty() {
            let (x, y) = proj.to_map(mouse_world.0, mouse_world.1);
            pending.push(Command::Move { units: selected.clone(), to: cell_at(x as i32, y as i32) });
        }
        if rl.is_key_pressed(KeyboardKey::KEY_B) {
            show_blocked = !show_blocked;
        }
        accumulator += if opts.screenshot.is_some() { tick } else { dt };
        while accumulator >= tick {
            world = step(&world, &rules, &pending);
            pending.clear();
            accumulator -= tick;
        }
        units = scene::units(&world, &proj);
        units.sort_by_key(|u| u.screen.1);

        let view = view_rect(&cam, screen);
        // drop the ground chunks far from the view, then rasterise a few missing ones, nearest first
        ground.retain(|&c, _| chunks::keep(c, view));
        let missing: Vec<(i32, i32)> =
            chunks::visible(view, (screen_w, screen_h)).into_iter().filter(|c| !ground.contains_key(c)).take(MAX_NEW_PER_FRAME).collect();
        for (cx, cy) in missing {
            let rect = Rect { x: cx * CHUNK, y: cy * CHUNK, w: CHUNK, h: CHUNK };
            let rgba = rasterize_ground(&assets.map, &assets.rhombs, &assets.rhomb_palettes, rect);
            ground.insert((cx, cy), upload(&mut rl, &thread, CHUNK as u32, CHUNK as u32, &rgba));
        }

        let (sw, sh) = (screen.0 as i32, screen.1 as i32);
        if shadow_mask.as_ref().is_none_or(|t| t.texture.width != sw || t.texture.height != sh) {
            shadow_mask = Some(rl.load_render_texture(&thread, sw as u32, sh as u32).expect("shadow mask"));
        }
        let mask = shadow_mask.as_mut().unwrap();
        let camera2d = Camera2D { offset: Vector2::zero(), target: Vector2::new(cam.x, cam.y), rotation: 0.0, zoom: cam.zoom };
        let part = |p: &ObjectPart| &part_textures[&(p.set, p.frame, p.state)][p.part];
        {
            let mut t = rl.begin_texture_mode(&thread, mask);
            t.clear_background(Color::BLANK);
            let mut m = t.begin_mode2D(camera2d);
            for p in &objects.shadows {
                draw_sprite(&mut m, part(p), p.screen, view);
            }
            for u in &units {
                draw_unit_shadow(&mut m, &vehicles[u.kind], u, view);
            }
        }

        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        {
            let mut m = d.begin_mode2D(camera2d);
            for ((cx, cy), texture) in &ground {
                if overlaps(view, cx * CHUNK, cy * CHUNK, CHUNK, CHUNK) {
                    m.draw_texture(texture, cx * CHUNK, cy * CHUNK, Color::WHITE);
                }
            }
            for p in &objects.flat {
                draw_sprite(&mut m, part(p), p.screen, view);
            }
        }
        {
            // shadowed pixels: 0.64 of the lit colour, measured on a screenshot of the original and
            // matching how the game is remembered; the cad*.dll code reads as half plus a blue-green
            // tint instead (see docs/formats.md)
            let source = Rectangle::new(0.0, 0.0, sw as f32, -(sh as f32));
            let mut b = d.begin_blend_mode(BlendMode::BLEND_MULTIPLIED);
            b.draw_texture_rec(&*mask, source, Vector2::zero(), Color::new(163, 163, 163, 255));
        }
        {
            let mut m = d.begin_mode2D(camera2d);
            for u in units.iter().filter(|u| selected.contains(&u.id)) {
                m.draw_ellipse_lines(u.screen.0, u.screen.1, 22.0, 11.0, Color::LIME);
            }
            // upright objects and units, back to front
            let mut units_left = units.iter().peekable();
            for p in &objects.upright {
                while let Some(u) = units_left.next_if(|u| u.screen.1 <= p.screen.1) {
                    draw_unit(&mut m, &vehicles[u.kind], u, view);
                }
                draw_sprite(&mut m, part(p), p.screen, view);
            }
            for u in units_left {
                draw_unit(&mut m, &vehicles[u.kind], u, view);
            }
            if show_blocked {
                draw_blocked(&mut m, &rules.grid, &proj, view);
            }
            if let Some(start) = press {
                let (x0, y0) = (start.0.min(mouse_world.0), start.1.min(mouse_world.1));
                let (w, h) = ((start.0 - mouse_world.0).abs(), (start.1 - mouse_world.1).abs());
                if w.max(h) >= selection::CLICK_SLOP {
                    m.draw_rectangle_lines(x0 as i32, y0 as i32, w as i32, h as i32, Color::LIME);
                }
            }
        }
        d.draw_text(
            "Left click / drag: select (Shift adds)   Right click: move   Arrows/WASD, middle drag: scroll   Wheel: zoom   B: blocked",
            10,
            10,
            20,
            Color::RAYWHITE,
        );
        d.draw_text(&format!("{} selected", selected.len()), 10, 36, 20, Color::LIME);
        d.draw_fps(10, 62);
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

/// Debug overlay: blocked cell quarters as red rhombs.
fn draw_blocked<D: RaylibDraw>(d: &mut D, grid: &Grid, proj: &openstrike_formats::iso::Projection, view: Rect) {
    let colour = Color::new(255, 0, 0, 90);
    for ty in 0..grid.height * 2 {
        for tx in 0..grid.width * 2 {
            if !grid.quarter_blocked(tx, ty) {
                continue;
            }
            let corner = |x: i32, y: i32| {
                let (sx, sy) = proj.to_screen(x * QUARTER, y * QUARTER);
                Vector2::new(sx as f32, sy as f32)
            };
            let (top, right, bottom, left) = (corner(tx, ty), corner(tx + 1, ty), corner(tx + 1, ty + 1), corner(tx, ty + 1));
            if !overlaps(view, left.x as i32, top.y as i32, (right.x - left.x) as i32, (bottom.y - top.y) as i32) {
                continue;
            }
            // raylib wants counter-clockwise triangles
            d.draw_triangle(top, left, right, colour);
            d.draw_triangle(right, left, bottom, colour);
        }
    }
}
