use openstrike_sim::grid::{Grid, CELLS, SIDE};
use openstrike_sim::vision::{cast, circle, refresh};

fn seen_cells(seen: &[u8], bit: u8) -> Vec<(i32, i32)> {
    (0..CELLS).filter(|&i| seen[i] & bit != 0).map(|i| (i as i32 % SIDE, i as i32 / SIDE)).collect()
}

fn grid_with(cells: &[((i32, i32), u32)]) -> Grid {
    let mut words = vec![0u32; 64 * 64];
    for &((x, y), w) in cells {
        words[(y * 64 + x) as usize] = w;
    }
    Grid::from_map(64, 64, &words)
}

#[test]
fn a_ray_marks_every_cell_up_to_its_end() {
    let mut seen = vec![0u8; CELLS];
    cast(&mut seen, &grid_with(&[]), 10, 10, 4, 0, 0x11);
    assert_eq!(seen_cells(&seen, 1), [(10, 10), (11, 10), (12, 10), (13, 10), (14, 10)]);
    assert_eq!(seen_cells(&seen, 0x10).len(), 5);
}

#[test]
fn obstacles_stop_rays_after_themselves() {
    let mut seen = vec![0u8; CELLS];
    cast(&mut seen, &grid_with(&[((12, 10), 0x1000)]), 10, 10, 4, 0, 1);
    assert_eq!(seen_cells(&seen, 1), [(10, 10), (11, 10), (12, 10)]);
    // over low ground (0x2000) only high ground (0x800) stops them
    let mut seen = vec![0u8; CELLS];
    let g = grid_with(&[((11, 10), 0x2000), ((12, 10), 0x1000), ((13, 10), 0x800)]);
    cast(&mut seen, &g, 10, 10, 4, 0, 1);
    assert_eq!(seen_cells(&seen, 1), [(10, 10), (11, 10), (12, 10), (13, 10)]);
}

#[test]
fn diagonal_rays_step_with_the_accumulator() {
    let mut seen = vec![0u8; CELLS];
    cast(&mut seen, &grid_with(&[]), 10, 10, 2, 1, 1);
    assert_eq!(seen_cells(&seen, 1), [(10, 10), (11, 10), (12, 11)]);
}

#[test]
fn sight_fades_at_the_second_refresh_without_a_new_look() {
    let ends = vec![vec![], vec![(1i8, 0i8), (-1, 0), (0, 1), (0, -1)]];
    let g = grid_with(&[]);
    let mut seen = vec![0u8; CELLS];
    circle(&mut seen, &g, &ends, 20, 20, 1, 2);
    assert_eq!(seen[20 * SIDE as usize + 21], 0x44);
    refresh(&mut seen, 64, 64);
    assert_eq!(seen[20 * SIDE as usize + 21], 0x04); // still seen
    refresh(&mut seen, 64, 64);
    assert_eq!(seen[20 * SIDE as usize + 21], 0); // gone
    // sides beyond 3 see nothing
    circle(&mut seen, &g, &ends, 20, 20, 1, 4);
    assert!(seen.iter().all(|&v| v == 0));
}
