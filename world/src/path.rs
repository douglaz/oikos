//! Deterministic breadth-first pathfinding around impassable terrain.
//!
//! Movement is 4-connected. The path is the shortest one a deterministic BFS
//! finds with a fixed tie-break: neighbours are expanded in up, left, right,
//! down order, and the first time the queue discovers a tile wins — so the same
//! `(grid, start, goal)` always yields the same path. An unreachable goal returns
//! `None` (the caller turns that into a `Blocked` status — never a panic). No
//! RNG: pathfinding is a pure function of the grid and endpoints.

use crate::grid::{Grid, Pos};

/// The shortest 4-connected path from `start` to `goal`, as the ordered list of
/// tiles to step onto (excluding `start`, ending at `goal`).
///
/// - `Some(vec![])` when `start == goal` (already there);
/// - `Some(path)` with `path.len()` == the step distance otherwise;
/// - `None` when `goal` is out of bounds, impassable, or unreachable from
///   `start` (or `start` itself is not passable).
pub fn shortest_path(grid: &Grid, start: Pos, goal: Pos) -> Option<Vec<Pos>> {
    if start == goal {
        return if grid.is_passable(start) {
            Some(Vec::new())
        } else {
            None
        };
    }
    if !grid.is_passable(start) || !grid.is_passable(goal) {
        return None;
    }

    let start_index = tile_index(grid, start)?;
    let goal_index = tile_index(grid, goal)?;
    let mut came_from = vec![usize::MAX; grid.tile_count()];
    let mut queue = Vec::new();
    let mut head = 0usize;
    came_from[start_index] = start_index;
    queue.push(start_index);

    while head < queue.len() {
        let current_index = queue[head];
        head += 1;
        let current = index_pos(grid, current_index);
        visit_passable_neighbor_indices(grid, current, |next_index| {
            if came_from[next_index] != usize::MAX {
                return;
            }
            came_from[next_index] = current_index;
            if next_index != goal_index {
                queue.push(next_index);
            }
        });
        if came_from[goal_index] != usize::MAX {
            return Some(reconstruct(grid, &came_from, start_index, goal_index));
        }
    }

    None
}

/// The step distance of the shortest path, or `None` if unreachable — the
/// travel-cost accessor the `World` exposes (and that G2b will read for
/// distance-affects-price).
///
/// Delegates to [`shortest_path`] so the distance is, by construction, the
/// length of the very path movement follows — a single BFS, no second
/// implementation to drift from it.
pub fn travel_cost(grid: &Grid, start: Pos, goal: Pos) -> Option<u32> {
    shortest_path(grid, start, goal).map(|path| path.len() as u32)
}

fn reconstruct(
    grid: &Grid,
    came_from: &[usize],
    start_index: usize,
    goal_index: usize,
) -> Vec<Pos> {
    let mut path = Vec::new();
    let mut node_index = goal_index;
    while node_index != start_index {
        path.push(index_pos(grid, node_index));
        node_index = came_from[node_index];
    }
    path.reverse();
    path
}

fn tile_index(grid: &Grid, pos: Pos) -> Option<usize> {
    if grid.in_bounds(pos) {
        Some(usize::from(pos.y) * usize::from(grid.width()) + usize::from(pos.x))
    } else {
        None
    }
}

fn index_pos(grid: &Grid, index: usize) -> Pos {
    let width = usize::from(grid.width());
    Pos::new((index % width) as u16, (index / width) as u16)
}

fn visit_passable_neighbor_indices<F>(grid: &Grid, pos: Pos, mut visit: F)
where
    F: FnMut(usize),
{
    // Route through the grid's single neighbour-order source so the BFS
    // tie-break can never diverge from `Grid::passable_neighbors`.
    grid.for_each_passable_neighbor(pos, |next| {
        let index = tile_index(grid, next)
            .expect("passable neighbor returned by the grid is always in bounds");
        visit(index);
    });
}

#[cfg(test)]
mod tests {
    use super::{shortest_path, travel_cost};
    use crate::grid::{Grid, Pos};

    #[test]
    fn straight_line_on_open_grid_has_distance_length() {
        let grid = Grid::new(5, 1);
        let path = shortest_path(&grid, Pos::new(0, 0), Pos::new(4, 0)).expect("reachable");
        assert_eq!(path.len(), 4);
        assert_eq!(*path.last().unwrap(), Pos::new(4, 0));
        // Each step is a single 4-connected move.
        let mut prev = Pos::new(0, 0);
        for step in &path {
            assert_eq!(prev.manhattan(*step), 1);
            prev = *step;
        }
    }

    #[test]
    fn same_start_and_goal_is_empty_path() {
        let grid = Grid::new(3, 3);
        assert_eq!(
            shortest_path(&grid, Pos::new(1, 1), Pos::new(1, 1)),
            Some(Vec::new())
        );
    }

    #[test]
    fn open_grid_travel_cost_equals_manhattan() {
        let grid = Grid::new(6, 6);
        for &(gx, gy) in &[(0u16, 0u16), (5, 0), (3, 4), (5, 5)] {
            let goal = Pos::new(gx, gy);
            assert_eq!(
                travel_cost(&grid, Pos::new(0, 0), goal),
                Some(Pos::new(0, 0).manhattan(goal))
            );
        }
    }

    #[test]
    fn path_routes_around_a_wall_through_the_gap() {
        // 5x5; a vertical wall at x=2 for rows 0..=3, leaving (2,4) as the gap.
        let mut grid = Grid::new(5, 5);
        for y in 0..4 {
            grid.set_impassable(Pos::new(2, y));
        }
        let path = shortest_path(&grid, Pos::new(0, 0), Pos::new(4, 0)).expect("reachable via gap");
        // The only crossing of column 2 is the gap.
        assert!(path.contains(&Pos::new(2, 4)));
        for step in &path {
            assert!(
                grid.is_passable(*step),
                "path stepped onto impassable {step:?}"
            );
        }
        // Shortest length: 6 to the gap + 6 back up = 12.
        assert_eq!(path.len(), 12);
    }

    #[test]
    fn pathfinding_is_reproducible() {
        let mut grid = Grid::new(6, 6);
        grid.set_impassable(Pos::new(3, 1));
        grid.set_impassable(Pos::new(3, 2));
        grid.set_impassable(Pos::new(3, 3));
        let a = shortest_path(&grid, Pos::new(0, 0), Pos::new(5, 2));
        let b = shortest_path(&grid, Pos::new(0, 0), Pos::new(5, 2));
        assert_eq!(a, b);
    }

    #[test]
    fn unreachable_goal_returns_none() {
        // Wall off (2,2) on all four sides.
        let mut grid = Grid::new(5, 5);
        grid.set_impassable(Pos::new(1, 2));
        grid.set_impassable(Pos::new(3, 2));
        grid.set_impassable(Pos::new(2, 1));
        grid.set_impassable(Pos::new(2, 3));
        assert_eq!(shortest_path(&grid, Pos::new(0, 0), Pos::new(2, 2)), None);
        assert_eq!(travel_cost(&grid, Pos::new(0, 0), Pos::new(2, 2)), None);
    }

    #[test]
    fn impassable_or_oob_goal_returns_none() {
        let mut grid = Grid::new(3, 3);
        grid.set_impassable(Pos::new(1, 1));
        assert_eq!(shortest_path(&grid, Pos::new(0, 0), Pos::new(1, 1)), None);
        assert_eq!(shortest_path(&grid, Pos::new(0, 0), Pos::new(9, 9)), None);
    }
}
