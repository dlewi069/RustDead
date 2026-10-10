use crate::math::Position3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PoiType {
    AbandonedGasStation,
    MilitaryOutpost,
    SurvivalCampsite,
    FieldHospital,
    RadioactiveCanyon,
    CivilianTown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointOfInterest {
    pub name: String,
    pub poi_type: PoiType,
    pub position: Position3D,
    pub radius_m: f32,
    pub danger_level: u8, // 1 to 5
    pub ambient_radiation: f32,
}

impl PointOfInterest {
    pub fn new(
        name: impl Into<String>,
        poi_type: PoiType,
        position: Position3D,
        radius_m: f32,
        danger_level: u8,
        ambient_radiation: f32,
    ) -> Self {
        Self {
            name: name.into(),
            poi_type,
            position,
            radius_m,
            danger_level,
            ambient_radiation,
        }
    }
}
