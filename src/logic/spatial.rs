//! Uniform spatial hash grid for fast neighbourhood queries.
//!
//! Survivor-likes routinely have well over a thousand enemies on screen. Checking
//! every projectile against every enemy is O(n * m); bucketing enemies into grid
//! cells reduces each query to the handful of cells around the query point.
//! The grid is rebuilt from scratch every frame, which is cheap and avoids having
//! to track movement between cells.

use std::collections::HashMap;

type Cell = (i32, i32);

/// A grid that maps world positions to items of type `T` (typically an `Entity`).
#[derive(Debug, Clone)]
pub struct SpatialGrid<T: Copy> {
    cell_size: f32,
    cells: HashMap<Cell, Vec<(T, [f32; 2])>>,
    len: usize,
}

impl<T: Copy> SpatialGrid<T> {
    /// Creates an empty grid. `cell_size` should be roughly the typical query radius.
    pub fn new(cell_size: f32) -> Self {
        assert!(cell_size > 0.0, "cell size must be positive");
        Self {
            cell_size,
            cells: HashMap::new(),
            len: 0,
        }
    }

    /// Removes all items but keeps allocations for the next frame.
    pub fn clear(&mut self) {
        // Drop buckets for cells that stayed empty last frame so the map does not
        // grow forever while the player walks across an unbounded world.
        self.cells.retain(|_, bucket| {
            let keep = !bucket.is_empty();
            bucket.clear();
            keep
        });
        self.len = 0;
    }

    /// Number of items in the grid.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the grid is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn cell_of(&self, x: f32, y: f32) -> Cell {
        (
            (x / self.cell_size).floor() as i32,
            (y / self.cell_size).floor() as i32,
        )
    }

    /// Inserts an item at a position.
    pub fn insert(&mut self, item: T, x: f32, y: f32) {
        let cell = self.cell_of(x, y);
        self.cells.entry(cell).or_default().push((item, [x, y]));
        self.len += 1;
    }

    /// Calls `f(item, x, y, distance_squared)` for every item within `radius` of `(x, y)`.
    pub fn for_each_in_radius(
        &self,
        x: f32,
        y: f32,
        radius: f32,
        mut f: impl FnMut(T, f32, f32, f32),
    ) {
        let (min_cx, min_cy) = self.cell_of(x - radius, y - radius);
        let (max_cx, max_cy) = self.cell_of(x + radius, y + radius);
        let r2 = radius * radius;
        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                let Some(bucket) = self.cells.get(&(cx, cy)) else {
                    continue;
                };
                for &(item, [ix, iy]) in bucket {
                    let dx = ix - x;
                    let dy = iy - y;
                    let d2 = dx * dx + dy * dy;
                    if d2 <= r2 {
                        f(item, ix, iy, d2);
                    }
                }
            }
        }
    }

    /// Collects all items within `radius`, sorted from nearest to farthest.
    pub fn query_sorted(&self, x: f32, y: f32, radius: f32) -> Vec<(T, f32)> {
        let mut found = Vec::new();
        self.for_each_in_radius(x, y, radius, |item, _, _, d2| found.push((item, d2)));
        found.sort_by(|a, b| a.1.total_cmp(&b.1));
        found
    }

    /// The nearest item within `radius`, with its distance.
    pub fn nearest(&self, x: f32, y: f32, radius: f32) -> Option<(T, f32)> {
        let mut best: Option<(T, f32)> = None;
        self.for_each_in_radius(x, y, radius, |item, _, _, d2| {
            if best.is_none_or(|(_, b)| d2 < b) {
                best = Some((item, d2));
            }
        });
        best.map(|(item, d2)| (item, d2.sqrt()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brute_force(points: &[(u32, f32, f32)], x: f32, y: f32, r: f32) -> Vec<u32> {
        let mut v: Vec<u32> = points
            .iter()
            .filter(|(_, px, py)| (px - x).powi(2) + (py - y).powi(2) <= r * r)
            .map(|(id, _, _)| *id)
            .collect();
        v.sort_unstable();
        v
    }

    #[test]
    fn radius_query_matches_brute_force() {
        let mut rng = crate::logic::rng::Rng::new(1234);
        let points: Vec<(u32, f32, f32)> = (0..2_000)
            .map(|i| {
                (
                    i,
                    rng.range_f32(-1_000.0, 1_000.0),
                    rng.range_f32(-1_000.0, 1_000.0),
                )
            })
            .collect();

        let mut grid = SpatialGrid::new(64.0);
        for &(id, x, y) in &points {
            grid.insert(id, x, y);
        }
        assert_eq!(grid.len(), points.len());

        for _ in 0..50 {
            let (x, y) = (
                rng.range_f32(-1_000.0, 1_000.0),
                rng.range_f32(-1_000.0, 1_000.0),
            );
            let r = rng.range_f32(1.0, 300.0);
            let mut got: Vec<u32> = grid
                .query_sorted(x, y, r)
                .into_iter()
                .map(|(id, _)| id)
                .collect();
            got.sort_unstable();
            assert_eq!(got, brute_force(&points, x, y, r));
        }
    }

    #[test]
    fn nearest_finds_closest_item() {
        let mut grid = SpatialGrid::new(32.0);
        grid.insert('a', 100.0, 0.0);
        grid.insert('b', -20.0, 5.0);
        grid.insert('c', 0.0, 300.0);
        let (item, dist) = grid.nearest(0.0, 0.0, 500.0).unwrap();
        assert_eq!(item, 'b');
        assert!((dist - (20.0f32 * 20.0 + 25.0).sqrt()).abs() < 1e-4);
        assert!(grid.nearest(0.0, 0.0, 10.0).is_none());
    }

    #[test]
    fn handles_negative_coordinates_and_cell_borders() {
        let mut grid = SpatialGrid::new(10.0);
        grid.insert(1, -0.1, -0.1);
        grid.insert(2, 0.0, 0.0);
        grid.insert(3, 9.99, 9.99);
        let found = grid.query_sorted(0.0, 0.0, 15.0);
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].0, 2);
    }

    #[test]
    fn clear_empties_grid() {
        let mut grid = SpatialGrid::new(10.0);
        grid.insert(1, 0.0, 0.0);
        grid.clear();
        assert!(grid.is_empty());
        assert!(grid.nearest(0.0, 0.0, 100.0).is_none());
    }
}
