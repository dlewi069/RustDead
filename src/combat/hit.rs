use crate::math::{Position3D, AABB};
use super::ballistics::HitZone;

#[derive(Debug, Clone)]
pub struct TargetHitbox {
    pub entity_id: u32,
    pub head_box: AABB,
    pub torso_box: AABB,
    pub stomach_box: AABB,
    pub limbs_box: AABB,
}

impl TargetHitbox {
    pub fn for_humanoid(entity_id: u32, pos: Position3D) -> Self {
        // Humanoid height ~1.8m
        let head_box = AABB::from_center_size(
            Position3D::new(pos.x, pos.y + 1.6, pos.z),
            Position3D::new(0.2, 0.2, 0.2),
        );
        let torso_box = AABB::from_center_size(
            Position3D::new(pos.x, pos.y + 1.2, pos.z),
            Position3D::new(0.35, 0.25, 0.25),
        );
        let stomach_box = AABB::from_center_size(
            Position3D::new(pos.x, pos.y + 0.85, pos.z),
            Position3D::new(0.3, 0.15, 0.2),
        );
        let limbs_box = AABB::from_center_size(
            Position3D::new(pos.x, pos.y + 0.4, pos.z),
            Position3D::new(0.35, 0.4, 0.25),
        );

        Self {
            entity_id,
            head_box,
            torso_box,
            stomach_box,
            limbs_box,
        }
    }

    pub fn test_hit(&self, ray: &crate::math::Ray) -> Option<(HitZone, f32)> {
        let mut closest: Option<(HitZone, f32)> = None;

        let checks = [
            (HitZone::Head, ray.intersects_aabb(&self.head_box)),
            (HitZone::Torso, ray.intersects_aabb(&self.torso_box)),
            (HitZone::Stomach, ray.intersects_aabb(&self.stomach_box)),
            (HitZone::Limbs, ray.intersects_aabb(&self.limbs_box)),
        ];

        for (zone, hit_dist) in checks {
            if let Some(dist) = hit_dist {
                match closest {
                    Some((_, best_dist)) if dist < best_dist => {
                        closest = Some((zone, dist));
                    }
                    None => {
                        closest = Some((zone, dist));
                    }
                    _ => {}
                }
            }
        }

        closest
    }
}
