//! OpenStrike: the imperative shell. It reads devices into plain values, feeds commands to the
//! pure simulation at a fixed rate, and draws the result. No game logic lives here.


use std::collections::HashMap;

use openstrike_formats::rhomb::{Rect, rasterize_ground};
use openstrike_formats::sprite::Image as Sprite;
use openstrike_sim::grid::CELL_PX;
use openstrike_sim::{Command, Grid, Rules, SUBPIXEL, TICKS_PER_SECOND, UnitId, step};
use raylib::prelude::*;

use openstrike::assets;
use openstrike::camera::{self, Camera, CameraInput};
use openstrike::scene::{self, ObjectPart, UnitView};
use openstrike::chunks::{self, CHUNK, MAX_NEW_PER_FRAME};
use openstrike::mission;
use openstrike::options::{self, Options, Parsed};
use openstrike::{selection, terrain};

/// Quarter of a cell, for the blocked overlay.
const QUARTER: i32 = CELL_PX / 2;
const SCREENSHOT_FRAMES: u32 = 200;
/// Darkens the pixels under `mask` as the game does (`sprite::shade`, `cad800.dll` `0x10007938`
/// for the map's shadows, `0x10007672` for the units'), except where `skip_a` or `skip_b` is set
/// (the units' shadows skip pixels already in a map shadow, flag bit 0, and upright objects, whose
/// pixel word has bit 15): on the RGB565 colour, halve each field (lowest bit dropped) and add
/// `0x0085`, i.e. 4 to green and 5 to blue. The scene colours come from RGB565 (`palette::rgb565`).
const SHADOW_SHADER: &str = r#"#version 330
in vec2 fragTexCoord;
in vec4 fragColor;
uniform sampler2D texture0;
uniform sampler2D mask;
uniform sampler2D skip_a;
uniform sampler2D skip_b;
out vec4 finalColor;
void main() {
    vec4 c = texture(texture0, fragTexCoord);
    if (texture(mask, fragTexCoord).a > 0.5 && texture(skip_a, fragTexCoord).a < 0.5
        && texture(skip_b, fragTexCoord).a < 0.5) {
        vec3 range = vec3(31.0, 63.0, 31.0);
        vec3 f = floor(c.rgb * range + 0.5);
        f = floor(f / 2.0) + vec3(0.0, 4.0, 5.0);
        c.rgb = floor(f * 255.0 / range + 0.001) / 255.0;
    }
    finalColor = vec4(c.rgb, 1.0);
}
"#;
/// Records the depth of the upright objects (layer 7) per pixel, like the pixel word the game
/// writes (`cad800.dll` `0x10005e41`: `(sort y + 0x440) << 5`): the sprite's tint carries the
/// encoded sort y (see `depth_tint`), written wherever the sprite has a pixel.
const DEPTH_SHADER: &str = r#"#version 330
in vec2 fragTexCoord;
in vec4 fragColor;
uniform sampler2D texture0;
out vec4 finalColor;
void main() {
    if (texture(texture0, fragTexCoord).a == 0.0) discard;
    finalColor = vec4(fragColor.rgb, 1.0);
}
"#;
/// Draws a unit sprite like `cad800.dll` `0x10006f08`: a pixel only where no upright object at the
/// same or a greater depth is drawn (`0x1000713f`); with see-through (`0x100072a2`), every other
/// pixel in a checkerboard regardless. The tint carries the unit's encoded depth and, in alpha,
/// whether it sees through.
const UNIT_SHADER: &str = r#"#version 330
in vec2 fragTexCoord;
in vec4 fragColor;
uniform sampler2D texture0;
uniform sampler2D depth;
uniform vec2 size;
uniform vec3 view; // camera x, y and zoom: the checkerboard is in game pixels
out vec4 finalColor;
float decode(vec3 v) { return floor(v.r * 255.0 + 0.5) * 256.0 + floor(v.g * 255.0 + 0.5); }
void main() {
    vec4 c = texture(texture0, fragTexCoord);
    if (c.a == 0.0) discard;
    vec4 d = texture(depth, gl_FragCoord.xy / size);
    bool hidden = d.a > 0.5 && decode(d.rgb) >= decode(fragColor.rgb);
    vec2 p = floor(view.xy + vec2(gl_FragCoord.x, size.y - gl_FragCoord.y) / view.z);
    bool checker = (int(p.x + p.y) & 1) == 0;
    if (hidden && !(fragColor.a > 0.5 && checker)) discard;
    finalColor = c;
}
"#;
/// Screen size and camera for the unit shader.
///
/// # Safety
/// `shader` must be the active shader.
unsafe fn set_unit_uniforms(shader: raylib::ffi::Shader, size_loc: i32, view_loc: i32, size: (i32, i32), cam: &Camera) {
    use raylib::ffi::ShaderUniformDataType::{SHADER_UNIFORM_VEC2, SHADER_UNIFORM_VEC3};
    let (size, view) = ([size.0 as f32, size.1 as f32], [cam.x, cam.y, cam.zoom]);
    // SAFETY: the arrays outlive the calls and match the uniform types
    unsafe {
        raylib::ffi::SetShaderValue(shader, size_loc, size.as_ptr().cast(), SHADER_UNIFORM_VEC2 as i32);
        raylib::ffi::SetShaderValue(shader, view_loc, view.as_ptr().cast(), SHADER_UNIFORM_VEC3 as i32);
    }
}

/// Offset that keeps the encoded depths positive.
const DEPTH_BIAS: i32 = 4096;

/// A screen y (and a flag in alpha) encoded in a tint, for the depth and unit shaders.
fn depth_tint(y: i32, flag: bool) -> Color {
    let v = (y + DEPTH_BIAS).clamp(0, 0xffff);
    Color::new((v >> 8) as u8, (v & 0xff) as u8, 0, if flag { 255 } else { 0 })
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
    // an empty sprite still gets a 1x1 transparent texture, so that part indices stay aligned
    let (w, h, rgba) = if s.width == 0 || s.height == 0 { (1, 1, &[0u8; 4][..]) } else { (s.width, s.height, &s.rgba[..]) };
    SpriteTexture { anchor: s.anchor, size: (w as i32, h as i32), texture: upload(rl, thread, w, h, rgba) }
}

fn upload_strip(rl: &mut RaylibHandle, thread: &RaylibThread, sprites: &[Sprite]) -> Vec<SpriteTexture> {
    sprites.iter().map(|s| upload_sprite(rl, thread, s)).collect()
}

struct TankTextures {
    hull: Vec<SpriteTexture>,
    tracks: Vec<SpriteTexture>,
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

fn draw_sprite_tinted<D: RaylibDraw>(d: &mut D, s: &SpriteTexture, at: (i32, i32), view: Rect, tint: Color) {
    let (x, y) = (at.0 + s.anchor.0, at.1 + s.anchor.1);
    if overlaps(view, x, y, s.size.0, s.size.1) {
        d.draw_texture(&s.texture, x, y, tint);
    }
}

/// Hull then turret (`Game_Dll.dll` `0x1000eeb0`, the third drawing pass).
fn draw_unit<D: RaylibDraw>(d: &mut D, tank: &TankTextures, u: &UnitView, view: Rect, tint: Color) {
    let mount = tank.mounts.get(u.frame).copied().unwrap_or_default();
    draw_sprite_tinted(d, &tank.hull[u.frame], u.screen, view, tint);
    draw_sprite_tinted(d, &tank.turret[u.turret_frame], (u.screen.0 + mount.0, u.screen.1 + mount.1), view, tint);
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
    let assets = assets::load(&opts.data, &opts.map, opts.climate.as_deref(), &[])?;
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
                tracks: upload_strip(&mut rl, &thread, &s.tracks),
                hull_shadow: upload_strip(&mut rl, &thread, &s.hull_shadow),
                turret: upload_strip(&mut rl, &thread, &s.turret),
                turret_shadow: upload_strip(&mut rl, &thread, &s.turret_shadow),
                mounts: s.mounts.clone(),
            }
        })
        .collect();
    // textures of the object parts this map uses
    let mut part_textures: HashMap<(usize, usize, usize), Vec<SpriteTexture>> = HashMap::new();
    for o in objects.ground.iter().chain(&objects.shadows).chain(&objects.upright) {
        part_textures.entry((o.set, o.frame, o.state)).or_insert_with(|| {
            let parts = assets.object_sets[o.set].frames[o.frame].parts(o.state);
            parts.iter().map(|p| upload_sprite(&mut rl, &thread, &p.image)).collect()
        });
    }
    let mut ground: HashMap<(i32, i32), Texture2D> = HashMap::new();

    let kinds = assets.vehicles.iter().enumerate().map(|(i, v)| (v.name.to_ascii_lowercase(), i)).collect();
    let start = mission::start(&assets.map, &assets.unit_classes, &kinds);
    if !start.skipped.is_empty() {
        eprintln!("openstrike: units of classes not simulated yet, left out: {:?}", start.skipped);
    }
    let mut world = start.world;
    // until the map's start view is read: the first unit of player 0, else the middle of the map
    let first = world.units.iter().rev().find(|u| u.player == 0).map(|u| u.position());
    let (mx, my) = first.map_or(((proj.width * 16) as i32, (proj.height * 16) as i32), |p| (p.0 / SUBPIXEL, p.1 / SUBPIXEL));
    let (lx, ly) = opts.look_at.unwrap_or((mx, my));
    let (tx, ty) = proj.to_screen(lx, ly);
    let mut cam = Camera { x: tx as f32 - 640.0 / opts.zoom, y: ty as f32 - 400.0 / opts.zoom, zoom: opts.zoom };
    let mut selected: Vec<UnitId> = Vec::new();
    let mut press: Option<(f32, f32)> = None;
    let mut pending: Vec<Command> = Vec::new();
    // Each frame, as the game composes the screen (`Game_Dll.dll` `0x100706d0`-`0x100707f9`): the
    // map (ground, layers 0-5, map shadows, upright objects with their depths), then the units in
    // three passes over all of them by depth: tracks, shadows, hull and turret.
    let mut targets: Option<[RenderTexture2D; 6]> = None;
    let mut shadow_shader = rl.load_shader_from_memory(&thread, None, Some(SHADOW_SHADER));
    let shadow_locs = ["mask", "skip_a", "skip_b"].map(|n| shadow_shader.get_shader_location(n));
    let mut depth_shader = rl.load_shader_from_memory(&thread, None, Some(DEPTH_SHADER));
    let mut unit_shader = rl.load_shader_from_memory(&thread, None, Some(UNIT_SHADER));
    let unit_depth_loc = unit_shader.get_shader_location("depth");
    let unit_size_loc = unit_shader.get_shader_location("size");
    let unit_view_loc = unit_shader.get_shader_location("view");
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
        // depth buckets, each filled by prepending while walking the unit list (`0x10070d00`)
        units.reverse();
        units.sort_by_key(UnitView::depth);
        let mouse_screen = (mouse_world.0 as i32, mouse_world.1 as i32);

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
        if targets.as_ref().is_none_or(|t| t[0].texture.width != sw || t[0].texture.height != sh) {
            targets = Some(std::array::from_fn(|_| rl.load_render_texture(&thread, sw as u32, sh as u32).expect("render target")));
        }
        let [scene, composed, map_mask, depth, unit_mask, blank] = targets.as_mut().unwrap();
        let camera2d = Camera2D { offset: Vector2::zero(), target: Vector2::new(cam.x, cam.y), rotation: 0.0, zoom: cam.zoom };
        let part = |p: &ObjectPart| &part_textures[&(p.set, p.frame, p.state)][p.part];
        let source = Rectangle::new(0.0, 0.0, sw as f32, -(sh as f32));
        {
            let mut t = rl.begin_texture_mode(&thread, blank);
            t.clear_background(Color::BLANK);
        }
        // map shadows (layer 6)
        {
            let mut t = rl.begin_texture_mode(&thread, map_mask);
            t.clear_background(Color::BLANK);
            let mut m = t.begin_mode2D(camera2d);
            for p in &objects.shadows {
                draw_sprite(&mut m, part(p), p.screen, view);
            }
        }
        // depths of the upright objects (layer 7), later ones over earlier ones
        {
            let mut t = rl.begin_texture_mode(&thread, depth);
            t.clear_background(Color::BLANK);
            let mut m = t.begin_mode2D(camera2d);
            let mut s = m.begin_shader_mode(&mut depth_shader);
            for p in &objects.upright {
                draw_sprite_tinted(&mut s, part(p), p.screen, view, depth_tint(p.sort_y, false));
            }
        }
        // ground and layers 0-5
        {
            let mut t = rl.begin_texture_mode(&thread, scene);
            t.clear_background(Color::BLACK);
            let mut m = t.begin_mode2D(camera2d);
            for ((cx, cy), texture) in &ground {
                if overlaps(view, cx * CHUNK, cy * CHUNK, CHUNK, CHUNK) {
                    m.draw_texture(texture, cx * CHUNK, cy * CHUNK, Color::WHITE);
                }
            }
            for p in &objects.ground {
                draw_sprite(&mut m, part(p), p.screen, view);
            }
        }
        let tint = |u: &UnitView| depth_tint(u.depth(), scene::see_through(selected.contains(&u.id), u.screen, mouse_screen));
        // map shadows applied, upright objects, then the units' first pass: their tracks
        {
            let mut t = rl.begin_texture_mode(&thread, composed);
            {
                let raw = *shadow_shader.as_ref();
                let mut b = t.begin_shader_mode(&mut shadow_shader);
                // SAFETY: binds render targets alive for the whole frame to the active shader
                unsafe {
                    raylib::ffi::SetShaderValueTexture(raw, shadow_locs[0], map_mask.texture);
                    raylib::ffi::SetShaderValueTexture(raw, shadow_locs[1], blank.texture);
                    raylib::ffi::SetShaderValueTexture(raw, shadow_locs[2], blank.texture);
                }
                b.draw_texture_rec(&*scene, source, Vector2::zero(), Color::WHITE);
            }
            let mut m = t.begin_mode2D(camera2d);
            for p in &objects.upright {
                draw_sprite(&mut m, part(p), p.screen, view);
            }
            let raw = *unit_shader.as_ref();
            let mut s = m.begin_shader_mode(&mut unit_shader);
            // SAFETY: as above
            unsafe {
                raylib::ffi::SetShaderValueTexture(raw, unit_depth_loc, depth.texture);
                set_unit_uniforms(raw, unit_size_loc, unit_view_loc, (sw, sh), &cam);
            }
            for u in &units {
                draw_sprite_tinted(&mut s, &vehicles[u.kind].tracks[u.frame], u.screen, view, tint(u));
            }
        }
        // second pass: the units' shadows
        {
            let mut t = rl.begin_texture_mode(&thread, unit_mask);
            t.clear_background(Color::BLANK);
            let mut m = t.begin_mode2D(camera2d);
            for u in &units {
                draw_unit_shadow(&mut m, &vehicles[u.kind], u, view);
            }
        }
        {
            let mut t = rl.begin_texture_mode(&thread, scene);
            let raw = *shadow_shader.as_ref();
            let mut b = t.begin_shader_mode(&mut shadow_shader);
            // SAFETY: as above
            unsafe {
                raylib::ffi::SetShaderValueTexture(raw, shadow_locs[0], unit_mask.texture);
                raylib::ffi::SetShaderValueTexture(raw, shadow_locs[1], map_mask.texture);
                raylib::ffi::SetShaderValueTexture(raw, shadow_locs[2], depth.texture);
            }
            b.draw_texture_rec(&*composed, source, Vector2::zero(), Color::WHITE);
        }
        // third pass: hull and turret
        {
            let mut t = rl.begin_texture_mode(&thread, scene);
            let mut m = t.begin_mode2D(camera2d);
            let raw = *unit_shader.as_ref();
            let mut s = m.begin_shader_mode(&mut unit_shader);
            // SAFETY: as above
            unsafe {
                raylib::ffi::SetShaderValueTexture(raw, unit_depth_loc, depth.texture);
                set_unit_uniforms(raw, unit_size_loc, unit_view_loc, (sw, sh), &cam);
            }
            for u in &units {
                draw_unit(&mut s, &vehicles[u.kind], u, view, tint(u));
            }
        }

        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        d.draw_texture_rec(&*scene, source, Vector2::zero(), Color::WHITE);
        {
            let mut m = d.begin_mode2D(camera2d);
            for u in units.iter().filter(|u| selected.contains(&u.id)) {
                m.draw_ellipse_lines(u.screen.0, u.screen.1, 22.0, 11.0, Color::LIME);
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
