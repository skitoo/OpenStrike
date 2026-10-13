//! Which ground chunks to keep on the GPU and which to rasterise next.

use openstrike_formats::rhomb::Rect;

/// Side of a ground chunk, in screen pixels.
pub const CHUNK: i32 = 512;
/// Chunks within this many chunks of the view are kept; further ones are dropped.
pub const KEEP_MARGIN: i32 = 2;
/// New chunks rasterised per frame at most, so that a fast zoom-out does not stall a frame.
pub const MAX_NEW_PER_FRAME: usize = 4;

/// Chunks touching `view`, nearest to its centre first, within a world of `world` screen pixels.
pub fn visible(view: Rect, world: (i32, i32)) -> Vec<(i32, i32)> {
    let (cx0, cy0) = (view.x.div_euclid(CHUNK).max(0), view.y.div_euclid(CHUNK).max(0));
    let cx1 = (view.x + view.w).div_euclid(CHUNK).min((world.0 - 1).div_euclid(CHUNK));
    let cy1 = (view.y + view.h).div_euclid(CHUNK).min((world.1 - 1).div_euclid(CHUNK));
    let centre = (view.x + view.w / 2, view.y + view.h / 2);
    let mut out: Vec<(i32, i32)> = (cy0..=cy1).flat_map(|y| (cx0..=cx1).map(move |x| (x, y))).collect();
    out.sort_by_key(|&(x, y)| {
        let (dx, dy) = (x * CHUNK + CHUNK / 2 - centre.0, y * CHUNK + CHUNK / 2 - centre.1);
        (dx as i64 * dx as i64 + dy as i64 * dy as i64, y, x)
    });
    out
}

/// Whether a loaded chunk is close enough to the view to keep.
pub fn keep(chunk: (i32, i32), view: Rect) -> bool {
    let margin = KEEP_MARGIN * CHUNK;
    let (x, y) = (chunk.0 * CHUNK, chunk.1 * CHUNK);
    x < view.x + view.w + margin && y < view.y + view.h + margin && x + CHUNK > view.x - margin && y + CHUNK > view.y - margin
}
