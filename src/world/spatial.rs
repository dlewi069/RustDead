use crate::math::Position3D;
use std::collections::HashMap;

/// Spatial hash grid for high-speed spatial partitioning
pub struct SpatialGrid<T> {
    pub cell_size: f32,
    pub cells: HashMap<(i32, i32), Vec<(u32, Position3D, T)>>,
}

impl<T> SpatialGrid<T> {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
        }
    }

    fn cell_coord(&self, pos: &Position3D) -> (i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.z / self.cell_size).floor() as i32,
        )
    }

    pub fn clear(&mut self) {
        self.cells.clear();
    }

    pub fn insert(&mut self, id: u32, pos: Position3D, item: T) {
        let coord = self.cell_coord(&pos);
        self.cells.entry(coord).or_default().push((id, pos, item));
    }

    pub fn query_radius(&self, center: &Position3D, radius: f32) -> Vec<(u32, Position3D, &T)> {
        let mut results = Vec::new();
        let min_coord = self.cell_coord(&Position3D::new(center.x - radius, center.y, center.z - radius));
        let max_coord = self.cell_coord(&Position3D::new(center.x + radius, center.y, center.z + radius));
        let r2 = radius * radius;

        for cx in min_coord.0..=max_coord.0 {
            for cz in min_coord.1..=max_coord.1 {
                if let Some(list) = self.cells.get(&(cx, cz)) {
                    for (id, pos, item) in list {
                        let dx = pos.x - center.x;
                        let dz = pos.z - center.z;
                        if (dx * dx + dz * dz) <= r2 {
                            results.push((*id, *pos, item));
                        }
                    }
                }
            }
        }

        results
    }
}
