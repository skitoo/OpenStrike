use openstrike_sim::geom::{Direction, Vec2, isqrt};

#[test]
fn isqrt_is_exact() {
    for n in [0, 1, 2, 3, 4, 15, 16, 17, 1 << 40, (1 << 40) - 1, i32::MAX as i64 * 3] {
        let r = isqrt(n);
        assert!(r * r <= n && (r + 1) * (r + 1) > n, "{n}");
    }
}

#[test]
fn headings_point_where_expected() {
    assert_eq!(Direction::towards(Vec2::new(10, 0)), Direction(0));
    assert_eq!(Direction::towards(Vec2::new(0, 10)), Direction(8));
    assert_eq!(Direction::towards(Vec2::new(-10, 0)), Direction(16));
    assert_eq!(Direction::towards(Vec2::new(0, -10)), Direction(24));
    assert_eq!(Direction::towards(Vec2::new(10, 10)), Direction(4));
    assert_eq!(Direction::towards(Vec2::new(1000, 200)), Direction(1));
}
