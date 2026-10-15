use openstrike::fog::{EDGE, LIT, factor, light};

// centred on cell (64, 64), at screen (0, 2048)
const VIEW: (i32, i32) = (-400, 1750);
const SIZE: (i32, i32) = (800, 600);
const MAP: (i32, i32) = (128, 128);

#[test]
fn a_view_seen_everywhere_is_left_alone() {
    let l = light(VIEW, SIZE, MAP, |_, _| true);
    assert!(
        (0..SIZE.1)
            .step_by(7)
            .all(|y| (0..SIZE.0).step_by(13).all(|x| factor(&l, x, y) == 32))
    );
}

#[test]
fn a_view_seen_nowhere_is_halved() {
    let l = light(VIEW, SIZE, MAP, |_, _| false);
    // the last row of tiles is left out: the game's third pass stops one row short of its lower
    // corners, which keep some light (hidden under its panel)
    assert!(
        (0..SIZE.1 - 8)
            .step_by(7)
            .all(|y| (0..SIZE.0).step_by(13).all(|x| factor(&l, x, y) == 16))
    );
}

#[test]
fn the_edge_of_what_is_seen_is_shaded() {
    // the cells with x + y < 132 are seen: the line x + y = 70 runs across the screen (screen y = 32 * (x + y) / 2 ... )
    let l = light(VIEW, SIZE, MAP, |x, y| x + y < 132);
    let values: Vec<u8> = l.corners.clone();
    assert!(values.contains(&0) && values.contains(&LIT) && values.contains(&EDGE));
    // from the top of the screen (seen) to the bottom (not seen), it gets darker
    let top = factor(&l, 400, 4);
    let bottom = factor(&l, 400, 580);
    assert_eq!((top, bottom), (32, 16));
    let middle: Vec<i32> = (0..SIZE.1 - 8)
        .step_by(8)
        .map(|y| factor(&l, 400, y))
        .collect();
    // a gradient over a few tiles; interpolated tiles go down to 15 ((0 + 127) / 8), a hair darker
    // than the fully dark ones (16)
    assert!(middle.iter().all(|&k| (15..=32).contains(&k)), "{middle:?}");
    assert!(
        middle.iter().filter(|&&k| k > 16 && k < 32).count() >= 3,
        "{middle:?}"
    );
}
