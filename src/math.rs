use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Position3D {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn distance_to(&self, other: &Position3D) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn distance_2d(&self, other: &Position3D) -> f32 {
        let dx = self.x - other.x;
        let dz = self.z - other.z;
        (dx * dx + dz * dz).sqrt()
    }

    pub fn to_vec3(&self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    pub fn from_vec3(v: Vec3) -> Self {
        Self { x: v.x, y: v.y, z: v.z }
    }

    pub fn lerp(&self, other: &Position3D, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
            z: self.z + (other.z - self.z) * t,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rotation3D {
    pub yaw: f32,   // Horizontal look angle in degrees
    pub pitch: f32, // Vertical look angle in degrees
    pub roll: f32,
}

impl Rotation3D {
    pub const ZERO: Self = Self { yaw: 0.0, pitch: 0.0, roll: 0.0 };

    pub fn new(yaw: f32, pitch: f32) -> Self {
        Self { yaw, pitch, roll: 0.0 }
    }

    pub fn forward_vector(&self) -> Vec3 {
        let yaw_rad = self.yaw.to_radians();
        let pitch_rad = self.pitch.to_radians();
        Vec3::new(
            yaw_rad.sin() * pitch_rad.cos(),
            pitch_rad.sin(),
            yaw_rad.cos() * pitch_rad.cos(),
        ).normalize_or_zero()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AABB {
    pub min: Position3D,
    pub max: Position3D,
}

impl AABB {
    pub fn new(min: Position3D, max: Position3D) -> Self {
        Self { min, max }
    }

    pub fn from_center_size(center: Position3D, half_extents: Position3D) -> Self {
        Self {
            min: Position3D::new(
                center.x - half_extents.x,
                center.y - half_extents.y,
                center.z - half_extents.z,
            ),
            max: Position3D::new(
                center.x + half_extents.x,
                center.y + half_extents.y,
                center.z + half_extents.z,
            ),
        }
    }

    pub fn contains(&self, p: &Position3D) -> bool {
        p.x >= self.min.x && p.x <= self.max.x
            && p.y >= self.min.y && p.y <= self.max.y
            && p.z >= self.min.z && p.z <= self.max.z
    }

    pub fn intersects(&self, other: &AABB) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
            && self.min.z <= other.max.z && self.max.z >= other.min.z
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Position3D,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Position3D, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize_or_zero(),
        }
    }

    pub fn at(&self, t: f32) -> Position3D {
        Position3D::new(
            self.origin.x + self.direction.x * t,
            self.origin.y + self.direction.y * t,
            self.origin.z + self.direction.z * t,
        )
    }

    pub fn intersects_aabb(&self, aabb: &AABB) -> Option<f32> {
        let mut tmin = (aabb.min.x - self.origin.x) / self.direction.x;
        let mut tmax = (aabb.max.x - self.origin.x) / self.direction.x;

        if tmin > tmax {
            std::mem::swap(&mut tmin, &mut tmax);
        }

        let mut tymin = (aabb.min.y - self.origin.y) / self.direction.y;
        let mut tymax = (aabb.max.y - self.origin.y) / self.direction.y;

        if tymin > tymax {
            std::mem::swap(&mut tymin, &mut tymax);
        }

        if (tmin > tymax) || (tymin > tmax) {
            return None;
        }

        if tymin > tmin {
            tmin = tymin;
        }

        if tymax < tmax {
            tmax = tymax;
        }

        let mut tzmin = (aabb.min.z - self.origin.z) / self.direction.z;
        let mut tzmax = (aabb.max.z - self.origin.z) / self.direction.z;

        if tzmin > tzmax {
            std::mem::swap(&mut tzmin, &mut tzmax);
        }

        if (tmin > tzmax) || (tzmin > tmax) {
            return None;
        }

        if tzmin > tmin {
            tmin = tzmin;
        }

        if tmin < 0.0 {
            if tmax < 0.0 {
                return None;
            }
            return Some(tmax);
        }

        Some(tmin)
    }
}
