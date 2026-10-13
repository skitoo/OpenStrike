use openstrike_formats::palette::{Palette, rgb565};
use openstrike_formats::sprite::{self, ALPHA_BLEND, ALPHA_OPAQUE, ALPHA_SHADOW, ALPHA_SHADOW_EDGE, Image, KIND_SHADOW};

fn grey_palette() -> Palette {
    let mut p = Palette::from_rgb565(&[0; 256]);
    for (i, c) in p.colours.iter_mut().enumerate() {
        *c = [i as u8; 3];
    }
    p
}

fn frame(kind: u8, w: u16, rows: &[&[u8]]) -> Vec<u8> {
    let mut f = Vec::new();
    f.extend((-2i16).to_le_bytes());
    f.extend((-1i16).to_le_bytes());
    f.extend(w.to_le_bytes());
    f.extend((rows.len() as u16).to_le_bytes());
    f.push(kind);
    for row in rows {
        f.extend((row.len() as u16).to_le_bytes());
        f.extend(*row);
    }
    f
}

fn pixel(img: &Image, x: usize, y: usize) -> [u8; 4] {
    img.rgba[(y * img.width as usize + x) * 4..][..4].try_into().unwrap()
}

#[test]
fn expands_rgb565_to_full_range() {
    assert_eq!(rgb565(0xffff), [255, 255, 255]);
    assert_eq!(rgb565(0xf800), [255, 0, 0]);
    assert_eq!(rgb565(0x07e0), [0, 255, 0]);
    assert_eq!(rgb565(0x001f), [0, 0, 255]);
}

#[test]
fn decodes_all_four_colour_commands() {
    // skip 1, copy 1 (10), blend 1 (20), repeat 2 x (30)
    let f = frame(0xa2, 6, &[&[0xC1, 0x01, 10, 0x41, 20, 0x82, 30]]);
    let img = sprite::decode(&f, &grey_palette()).unwrap();
    assert_eq!(img.anchor, (-2, -1));
    assert_eq!(pixel(&img, 0, 0), [0, 0, 0, 0]);
    assert_eq!(pixel(&img, 1, 0), [10, 10, 10, ALPHA_OPAQUE]);
    assert_eq!(pixel(&img, 2, 0), [20, 20, 20, ALPHA_BLEND]);
    assert_eq!(pixel(&img, 3, 0), [30, 30, 30, ALPHA_OPAQUE]);
    assert_eq!(pixel(&img, 4, 0), [30, 30, 30, ALPHA_OPAQUE]);
    assert_eq!(pixel(&img, 5, 0), [0, 0, 0, 0]);
}

#[test]
fn shadow_commands_carry_no_colour_byte() {
    let f = frame(KIND_SHADOW, 5, &[&[0x81, 0x02, 0x41, 0xC1]]);
    let img = sprite::decode(&f, &grey_palette()).unwrap();
    assert_eq!(pixel(&img, 0, 0)[3], 0);
    assert_eq!(pixel(&img, 1, 0), [0, 0, 0, ALPHA_SHADOW]);
    assert_eq!(pixel(&img, 3, 0), [0, 0, 0, ALPHA_SHADOW_EDGE]);
    assert_eq!(pixel(&img, 4, 0)[3], 0);
}

#[test]
fn rejects_rows_wider_than_the_frame() {
    let f = frame(0xa2, 2, &[&[0x83, 1]]);
    assert!(sprite::decode(&f, &grey_palette()).is_err());
}

#[test]
fn splits_both_pck_layouts() {
    // offsets from byte 0: [12, 14, 16]
    let mut a: Vec<u8> = [12u32, 14, 16].iter().flat_map(|v| v.to_le_bytes()).collect();
    a.extend([1, 2, 3, 4]);
    assert_eq!(sprite::pck_frames(&a).unwrap(), vec![&[1u8, 2][..], &[3, 4][..]]);
    // header, then offsets from byte 4: [12, 14]
    let mut b: Vec<u8> = [1u32, 12, 14].iter().flat_map(|v| v.to_le_bytes()).collect();
    b.extend([5, 6]);
    assert_eq!(sprite::pck_frames(&b).unwrap(), vec![&[5u8, 6][..]]);
}

#[test]
fn splits_rs2_and_keeps_empty_frames() {
    // count 2, offsets [16, 16, 18]
    let mut d: Vec<u8> = [2u32, 16, 16, 18].iter().flat_map(|v| v.to_le_bytes()).collect();
    d.extend([7, 8]);
    assert_eq!(sprite::rs2_frames(&d).unwrap(), vec![&[][..], &[7u8, 8][..]]);
}

#[test]
fn colour_key_entries_stay_transparent() {
    let mut values = [0u16; 256];
    values[5] = openstrike_formats::palette::COLOUR_KEY;
    let f = frame(0xa1, 2, &[&[0x02, 5, 0]]);
    let img = sprite::decode(&f, &Palette::from_rgb565(&values)).unwrap();
    assert_eq!(pixel(&img, 0, 0)[3], 0);
    assert_eq!(pixel(&img, 1, 0)[3], ALPHA_OPAQUE);
}

#[test]
fn frame_len_finds_exact_frames_only() {
    let f = frame(0xa2, 6, &[&[0xC1, 0x01, 10, 0x41, 20, 0x82, 30]]);
    let mut data = vec![9, 9, 9];
    data.extend(&f);
    assert_eq!(sprite::frame_len(&data, 3), Some(f.len()));
    assert_eq!(sprite::frame_len(&data, 0), None);
    assert_eq!(sprite::frame_len(&data, 4), None);
}

#[test]
fn a_frame_count_that_is_a_multiple_of_4_is_not_taken_for_an_offset() {
    // count 4, then offsets [24, 26, 28, 30, 32] from byte 4, then 4 frames of 2 bytes
    let mut d: Vec<u8> = [4u32, 24, 26, 28, 30, 32].iter().flat_map(|v| v.to_le_bytes()).collect();
    d.extend([1, 1, 2, 2, 3, 3, 4, 4]);
    let frames = sprite::pck_frames(&d).unwrap();
    assert_eq!(frames, vec![&[1u8, 1][..], &[2, 2][..], &[3, 3][..], &[4, 4][..]]);
}
