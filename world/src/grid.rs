//! `Pos`, `Terrain`, and the row-major `Grid` — the bare spatial substrate.
//!
//! The grid is integer and finite: a `width × height` rectangle of tiles, each
//! either `Passable` or `Impassable`. Nothing here knows about economics; a tile
//! is a place, not a market. `Impassable` blocks movement (pathfinding routes
//! around it) and cannot host a node/stockpile/agent that must be stood on — the
//! `World` rejects such placements at construction (see `world.rs`).

/// An integer tile coordinate. `Ord` is derived so positions can be ordered
/// deterministically when exposed or stored; no `HashMap` is used in `world`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pos {
    pub x: u16,
    pub y: u16,
}

impl Pos {
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }

    /// 4-connected Manhattan distance — the cheap, obstacle-blind separation
    /// estimate. On an open grid this equals the true travel cost; with
    /// obstacles it is a lower bound. Monotone in grid separation (the property
    /// G2b leans on for "distance affects price").
    pub fn manhattan(self, other: Pos) -> u32 {
        let dx = u32::from(self.x.abs_diff(other.x));
        let dy = u32::from(self.y.abs_diff(other.y));
        dx + dy
    }
}

/// The fixed 4-neighbour scan order — up, left, right, down (lowest delta-index
/// first). This single constant is the deterministic tie-break contract for
/// **both** neighbour enumeration ([`Grid::passable_neighbors`]) and BFS
/// pathfinding (`path::shortest_path`); routing both through it keeps the two
/// from silently drifting apart. Do not reorder.
pub(crate) const NEIGHBOR_DELTAS: [(i32, i32); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];

/// Whether a tile can be entered. Pure spatial — no cost, no economics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Terrain {
    Passable,
    Impassable,
}

impl Terrain {
    /// Stable one-byte tag for canonical serialization / determinism hashing.
    pub(crate) fn tag(self) -> u8 {
        match self {
            Terrain::Passable => 0,
            Terrain::Impassable => 1,
        }
    }
}

/// A finite integer tile grid, terrain stored row-major (`index = y*width + x`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    width: u16,
    height: u16,
    terrain: Vec<Terrain>,
}

impl Grid {
    /// A grid of the given size with every tile `Passable`.
    pub fn new(width: u16, height: u16) -> Self {
        let tiles = usize::from(width) * usize::from(height);
        Self {
            width,
            height,
            terrain: vec![Terrain::Passable; tiles],
        }
    }

    /// Build from an explicit row-major terrain vector. `None` if the length does
    /// not match `width × height` (a deterministic rejection, never a panic).
    pub fn with_terrain(width: u16, height: u16, terrain: Vec<Terrain>) -> Option<Self> {
        if terrain.len() != usize::from(width) * usize::from(height) {
            return None;
        }
        Some(Self {
            width,
            height,
            terrain,
        })
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    /// Total tile count.
    pub fn tile_count(&self) -> usize {
        self.terrain.len()
    }

    /// Whether `pos` lies inside the grid rectangle.
    pub fn in_bounds(&self, pos: Pos) -> bool {
        pos.x < self.width && pos.y < self.height
    }

    fn index(&self, pos: Pos) -> Option<usize> {
        if self.in_bounds(pos) {
            Some(usize::from(pos.y) * usize::from(self.width) + usize::from(pos.x))
        } else {
            None
        }
    }

    /// The terrain at `pos`, or `None` if out of bounds.
    pub fn terrain_at(&self, pos: Pos) -> Option<Terrain> {
        self.index(pos).map(|i| self.terrain[i])
    }

    /// Whether an agent may stand on / move onto `pos`: in bounds and `Passable`.
    /// Out-of-bounds tiles are impassable by definition.
    pub fn is_passable(&self, pos: Pos) -> bool {
        matches!(self.terrain_at(pos), Some(Terrain::Passable))
    }

    /// Set the terrain at `pos`. Returns `false` (no-op) if out of bounds.
    pub fn set_terrain(&mut self, pos: Pos, terrain: Terrain) -> bool {
        match self.index(pos) {
            Some(i) => {
                self.terrain[i] = terrain;
                true
            }
            None => false,
        }
    }

    /// Mark `pos` impassable. Returns `false` (no-op) if out of bounds.
    pub fn set_impassable(&mut self, pos: Pos) -> bool {
        self.set_terrain(pos, Terrain::Impassable)
    }

    /// Invoke `visit` for each in-bounds, passable 4-neighbour of `pos`, in the
    /// fixed [`NEIGHBOR_DELTAS`] order. Allocation-free — the single neighbour
    /// scan both `passable_neighbors` and the pathfinder flow through, so the
    /// determinism contract lives in exactly one place.
    pub(crate) fn for_each_passable_neighbor(&self, pos: Pos, mut visit: impl FnMut(Pos)) {
        for (dx, dy) in NEIGHBOR_DELTAS {
            let nx = i32::from(pos.x) + dx;
            let ny = i32::from(pos.y) + dy;
            if nx < 0 || ny < 0 {
                continue;
            }
            let (Ok(nx), Ok(ny)) = (u16::try_from(nx), u16::try_from(ny)) else {
                continue;
            };
            let next = Pos::new(nx, ny);
            if self.is_passable(next) {
                visit(next);
            }
        }
    }

    /// The in-bounds, passable 4-neighbours of `pos`, collected in the fixed
    /// tie-break order (up, left, right, down). A thin `Vec` wrapper over
    /// `Grid::for_each_passable_neighbor`.
    pub fn passable_neighbors(&self, pos: Pos) -> Vec<Pos> {
        let mut out = Vec::with_capacity(4);
        self.for_each_passable_neighbor(pos, |next| out.push(next));
        out
    }

    /// Append a canonical byte serialization of the grid (dimensions + terrain
    /// tags in row-major order) for determinism hashing.
    pub(crate) fn write_canonical(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        for terrain in &self.terrain {
            out.push(terrain.tag());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Grid, Pos, Terrain};

    #[test]
    fn new_grid_is_all_passable_and_sized() {
        let grid = Grid::new(4, 3);
        assert_eq!(grid.width(), 4);
        assert_eq!(grid.height(), 3);
        assert_eq!(grid.tile_count(), 12);
        for y in 0..3 {
            for x in 0..4 {
                assert!(grid.is_passable(Pos::new(x, y)));
            }
        }
    }

    #[test]
    fn out_of_bounds_is_not_passable() {
        let grid = Grid::new(2, 2);
        assert!(!grid.in_bounds(Pos::new(2, 0)));
        assert!(!grid.is_passable(Pos::new(2, 0)));
        assert!(!grid.is_passable(Pos::new(0, 9)));
        assert_eq!(grid.terrain_at(Pos::new(5, 5)), None);
    }

    #[test]
    fn set_impassable_blocks_the_tile() {
        let mut grid = Grid::new(3, 3);
        assert!(grid.set_impassable(Pos::new(1, 1)));
        assert!(!grid.is_passable(Pos::new(1, 1)));
        assert_eq!(grid.terrain_at(Pos::new(1, 1)), Some(Terrain::Impassable));
        // Out of bounds is a deterministic no-op, never a panic.
        assert!(!grid.set_impassable(Pos::new(9, 9)));
    }

    #[test]
    fn with_terrain_rejects_length_mismatch() {
        assert!(Grid::with_terrain(2, 2, vec![Terrain::Passable; 3]).is_none());
        assert!(Grid::with_terrain(2, 2, vec![Terrain::Passable; 4]).is_some());
    }

    #[test]
    fn neighbors_follow_fixed_order_and_skip_blocked() {
        let mut grid = Grid::new(3, 3);
        // Block the up-neighbour of the centre; order becomes left, right, down.
        grid.set_impassable(Pos::new(1, 0));
        let neighbors = grid.passable_neighbors(Pos::new(1, 1));
        assert_eq!(
            neighbors,
            vec![Pos::new(0, 1), Pos::new(2, 1), Pos::new(1, 2)]
        );
    }

    #[test]
    fn corner_neighbors_drop_out_of_bounds() {
        let grid = Grid::new(3, 3);
        // From (0,0): up and left are out of bounds → only right and down.
        assert_eq!(
            grid.passable_neighbors(Pos::new(0, 0)),
            vec![Pos::new(1, 0), Pos::new(0, 1)]
        );
    }

    #[test]
    fn manhattan_is_symmetric_and_grows_with_separation() {
        let origin = Pos::new(0, 0);
        assert_eq!(origin.manhattan(Pos::new(3, 4)), 7);
        assert_eq!(Pos::new(3, 4).manhattan(origin), 7);
        assert!(origin.manhattan(Pos::new(2, 0)) < origin.manhattan(Pos::new(3, 0)));
    }
}
