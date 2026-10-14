use openstrike_formats::palette::{AlphaPalette, blend_alpha};
use openstrike_formats::sprite::{KIND_ALPHA, decode_alpha};

fn palette(entries: &[(u8, u32)]) -> AlphaPalette {
    let mut p = AlphaPalette { entries: [0x00f8_0000; 256] };
    for &(i, v) in entries {
        p.entries[i as usize] = v;
    }
    p
}

#[test]
fn blending_weights_the_background_per_field_then_adds_the_colour() {
    // k = 16: half the background, plus a weighted colour
    let p = palette(&[(1, 0x0080_0821), (2, 0x00f8_ffff), (3, 0x0000_1234)]);
    assert_eq!(p.background_weight(1), 16);
    // white 0xffff halves per field: R 31 -> 15, G 63 -> 31, B 31 -> 15
    assert_eq!(blend_alpha(&p, 1, 0xffff), Some(((15 << 11) | (31 << 5) | 15) + 0x0821));
    // k = 31 leaves the pixel alone
    assert_eq!(blend_alpha(&p, 2, 0xffff), None);
    // k = 0 replaces it
    assert_eq!(blend_alpha(&p, 3, 0xffff), Some(0x1234));
}

#[test]
fn alpha_frames_decode_to_premultiplied_pixels() {
    let p = palette(&[(1, 0x0080_f800), (2, 0x0000_001f)]);
    // 3x1: copy entries 1 and 2, then entry 0 (k = 31)
    let mut frame = vec![0, 0, 0, 0, 3, 0, 1, 0, KIND_ALPHA];
    frame.extend_from_slice(&[4, 0, 0x03, 1, 2, 0]);
    let image = decode_alpha(&frame, &p).unwrap();
    assert_eq!(&image.rgba[0..4], &[255, 0, 0, 127]);
    assert_eq!(&image.rgba[4..8], &[0, 0, 255, 255]);
    assert_eq!(&image.rgba[8..12], &[0, 0, 0, 0]);
}
