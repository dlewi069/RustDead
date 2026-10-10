use super::poi::{PointOfInterest, PoiType};
use crate::math::Position3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Obstacle {
    pub id: u32,
    pub name: String,
    pub position: Position3D,
    pub radius_m: f32,
    pub blocks_los: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapWaypoint {
    pub position: Position3D,
    pub label: String,
    pub is_active: bool,
}

impl MapWaypoint {
    pub fn new(position: Position3D, label: impl Into<String>) -> Self {
        Self {
            position,
            label: label.into(),
            is_active: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldMap {
    pub width_m: f32,
    pub depth_m: f32,
    pub pois: Vec<PointOfInterest>,
    pub obstacles: Vec<Obstacle>,
    pub waypoint: Option<MapWaypoint>,
}

impl WorldMap {
    pub fn generate_default_valley() -> Self {
        let pois = vec![
            PointOfInterest::new(
                "Abandoned Red Rocket Gas Station",
                PoiType::AbandonedGasStation,
                Position3D::new(0.0, 0.0, 45.0),
                35.0,
                1,
                0.0,
            ),
            PointOfInterest::new(
                "Camp Timber Survior Outpost",
                PoiType::SurvivalCampsite,
                Position3D::new(-70.0, 0.0, -30.0),
                25.0,
                1,
                0.0,
            ),
            PointOfInterest::new(
                "FOB Bravo Military Checkpoint",
                PoiType::MilitaryOutpost,
                Position3D::new(120.0, 0.0, -90.0),
                50.0,
                4,
                5.0,
            ),
            PointOfInterest::new(
                "St. Jude Emergency Field Hospital",
                PoiType::FieldHospital,
                Position3D::new(-110.0, 0.0, 110.0),
                40.0,
                3,
                15.0,
            ),
            PointOfInterest::new(
                "Hazmat Containment Zone",
                PoiType::RadioactiveCanyon,
                Position3D::new(180.0, 0.0, 150.0),
                65.0,
                5,
                75.0,
            ),
        ];

        let mut obstacles = Vec::new();
        let mut obs_id = 1;

        // Populate natural tree clumps and boulders
        for i in -5..=5 {
            for j in -5..=5 {
                if (i * j) % 3 == 0 {
                    obstacles.push(Obstacle {
                        id: obs_id,
                        name: "Pine Tree".into(),
                        position: Position3D::new((i as f32) * 35.0 + 5.0, 0.0, (j as f32) * 35.0 - 4.0),
                        radius_m: 1.2,
                        blocks_los: true,
                    });
                    obs_id += 1;
                }
            }
        }

        Self {
            width_m: 500.0,
            depth_m: 500.0,
            pois,
            obstacles,
            waypoint: None,
        }
    }

    pub fn set_waypoint(&mut self, pos: Position3D, label: impl Into<String>) {
        self.waypoint = Some(MapWaypoint::new(pos, label));
    }

    pub fn clear_waypoint(&mut self) {
        self.waypoint = None;
    }

    /// Calculate military sector designation (e.g. "Sector B-3") based on world coords
    pub fn sector_name_at(&self, pos: &Position3D) -> String {
        let half_w = self.width_m * 0.5;
        let half_d = self.depth_m * 0.5;
        let norm_x = ((pos.x + half_w) / self.width_m).clamp(0.0, 0.999);
        let norm_z = ((pos.z + half_d) / self.depth_m).clamp(0.0, 0.999);

        let col_idx = (norm_x * 5.0) as usize; // A, B, C, D, E
        let row_idx = (norm_z * 5.0) as usize + 1; // 1, 2, 3, 4, 5

        let letter = match col_idx {
            0 => "Alpha",
            1 => "Bravo",
            2 => "Charlie",
            3 => "Delta",
            _ => "Echo",
        };

        format!("Sector {}-{}", letter, row_idx)
    }

    /// Checks if a position is near the meandering valley river (for freshwater fishing)
    pub fn is_near_water(&self, pos: &Position3D) -> bool {
        // River runs along z = -20.0 with gentle meander: z_river(x) = -20.0 + 8.0 * (x / 50.0).sin()
        let river_z = -20.0 + 6.0 * (pos.x * 0.03).sin();
        let dist = (pos.z - river_z).abs();
        dist <= 5.5
    }

    pub fn get_radiation_at(&self, pos: &Position3D) -> f32 {
        let mut max_rad = 0.0f32;
        for poi in &self.pois {
            if poi.ambient_radiation > 0.0 {
                let dist = pos.distance_2d(&poi.position);
                if dist < poi.radius_m {
                    let factor = 1.0 - (dist / poi.radius_m);
                    let rad = poi.ambient_radiation * factor;
                    if rad > max_rad {
                        max_rad = rad;
                    }
                }
            }
        }
        max_rad
    }
}
