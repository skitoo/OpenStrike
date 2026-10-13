use openstrike_sim::Terrain;
use openstrike_sim::path::{find, line_of_sight, nearest_passable, smooth};

#[test]
fn straight_paths_on_open_ground() {
    let t = Terrain::open(10, 10);
    assert_eq!(find(&t, (0, 0), (3, 0)).unwrap(), vec![(1, 0), (2, 0), (3, 0)]);
    assert_eq!(find(&t, (0, 0), (2, 2)).unwrap(), vec![(1, 1), (2, 2)]);
    assert_eq!(find(&t, (4, 4), (4, 4)).unwrap(), vec![]);
}

#[test]
fn no_corner_cutting_between_blocked_tiles() {
    let t = Terrain::open(3, 3).with_blocked([(1, 0), (0, 1)]);
    assert_eq!(find(&t, (0, 0), (1, 1)), None);
}

#[test]
fn paths_avoid_blocked_tiles() {
    let t = Terrain::open(10, 10).with_blocked((0..9).map(|y| (5, y)));
    let path = find(&t, (0, 0), (9, 0)).unwrap();
    assert!(path.iter().all(|&(x, y)| t.passable(x, y)));
    assert!(path.contains(&(5, 9)));
}

#[test]
fn smoothing_keeps_only_corners() {
    let t = Terrain::open(10, 10).with_blocked((0..9).map(|y| (5, y)));
    let path = find(&t, (0, 0), (9, 0)).unwrap();
    let smoothed = smooth(&t, (0, 0), &path);
    assert!(smoothed.len() < path.len());
    assert_eq!(smoothed.last(), Some(&(9, 0)));
    let mut from = (0, 0);
    for &p in &smoothed {
        assert!(line_of_sight(&t, from, p), "{from:?} -> {p:?}");
        from = p;
    }
}

#[test]
fn nearest_passable_finds_the_edge_of_an_obstacle() {
    let t = Terrain::open(10, 10).with_blocked((3..6).flat_map(|x| (3..6).map(move |y| (x, y))));
    let p = nearest_passable(&t, (4, 4)).unwrap();
    assert!(t.passable(p.0, p.1));
    assert!((2..=6).contains(&p.0) && (2..=6).contains(&p.1));
    assert_eq!(nearest_passable(&t, (0, 0)), Some((0, 0)));
}
