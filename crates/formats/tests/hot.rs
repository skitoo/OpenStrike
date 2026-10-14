use openstrike_formats::hot::parse;

#[test]
fn parses_point_pairs() {
    let data: Vec<u8> = [-1i32, -13, 7, 1].iter().flat_map(|v| v.to_le_bytes()).collect();
    assert_eq!(parse(&data).unwrap(), vec![(-1, -13), (7, 1)]);
    assert!(parse(&data[..6]).is_err());
}
