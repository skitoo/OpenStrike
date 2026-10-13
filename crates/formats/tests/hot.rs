use openstrike_formats::hot::{frame_for_heading, parse};

#[test]
fn parses_point_pairs() {
    let data: Vec<u8> = [-1i32, -13, 7, 1].iter().flat_map(|v| v.to_le_bytes()).collect();
    assert_eq!(parse(&data).unwrap(), vec![(-1, -13), (7, 1)]);
    assert!(parse(&data[..6]).is_err());
}

#[test]
fn frames_follow_the_muzzle_ellipse() {
    // t-26.hot muzzle points: frame 28 east, 4 south, 12 west, 20 north
    assert_eq!(frame_for_heading(0), 28);
    assert_eq!(frame_for_heading(8), 4);
    assert_eq!(frame_for_heading(16), 12);
    assert_eq!(frame_for_heading(24), 20);
}
