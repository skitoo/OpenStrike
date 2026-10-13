use openstrike::chunks::{CHUNK, keep, visible};
use openstrike_formats::rhomb::Rect;

#[test]
fn visible_chunks_stay_inside_the_world_nearest_first() {
    let view = Rect { x: -100, y: 600, w: 1280, h: 800 };
    let v = visible(view, (2048, 1024));
    assert!(v.iter().all(|&(x, y)| x >= 0 && y >= 0 && x * CHUNK < 2048 && y * CHUNK < 1024));
    assert_eq!(v.len(), 3); // x 0..=2, y 1 only (the world is two chunks high)
    assert_eq!(v[0], (1, 1));
}

#[test]
fn far_chunks_are_dropped() {
    let view = Rect { x: 0, y: 0, w: 1280, h: 800 };
    assert!(keep((0, 0), view));
    assert!(keep((4, 3), view));
    assert!(!keep((10, 0), view));
    assert!(!keep((0, -4), view));
}
