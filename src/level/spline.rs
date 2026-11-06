//! `USplineMeshComponent` deformation, ported from UE's `CalcSliceTransform`: each vertex's
//! position along the forward axis picks a slice of a cubic Hermite spline, and its other two
//! coordinates are laid out across that slice. Done once on the CPU per spline instance.

use bevy::mesh::{Mesh, VertexAttributeValues};
use bevy::prelude::*;
use serde::Deserialize;

/// One end of a spline segment, in UE component space (cm, Z-up).
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct SplineEnd {
    pub pos: [f32; 3],
    pub tangent: [f32; 3],
    pub scale: [f32; 2],
    pub roll: f32,
    pub offset: [f32; 2],
}

#[derive(Deserialize, Clone, Debug)]
pub struct SplineMesh {
    pub mesh: String,
    /// Component-to-world, Bevy space, column-major 3x4.
    pub transform: [f32; 12],
    /// ESplineMeshAxis: 0 = X, 1 = Y, 2 = Z.
    pub axis: u8,
    pub up: [f32; 3],
    pub boundary: [f32; 2],
    pub smooth: bool,
    pub start: SplineEnd,
    pub end: SplineEnd,
}

fn v(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}

/// Bevy mesh space (m, Y-up) <-> UE (cm, Z-up): (x, y, z)_bevy = (X, Z, Y)_ue / 100.
fn to_ue(p: Vec3) -> Vec3 {
    Vec3::new(p.x, p.z, p.y) * 100.0
}
fn from_ue(p: Vec3) -> Vec3 {
    Vec3::new(p.x, p.z, p.y) / 100.0
}

fn axis_value(p: Vec3, axis: u8) -> f32 {
    match axis {
        0 => p.x,
        1 => p.y,
        _ => p.z,
    }
}

impl SplineMesh {
    fn eval_pos(&self, a: f32) -> Vec3 {
        let (a2, a3) = (a * a, a * a * a);
        let (p0, t0, p1, t1) = (v(self.start.pos), v(self.start.tangent), v(self.end.pos), v(self.end.tangent));
        p0 * (2.0 * a3 - 3.0 * a2 + 1.0) + t0 * (a3 - 2.0 * a2 + a) + t1 * (a3 - a2) + p1 * (-2.0 * a3 + 3.0 * a2)
    }

    fn eval_dir(&self, a: f32) -> Vec3 {
        let (p0, t0, p1, t1) = (v(self.start.pos), v(self.start.tangent), v(self.end.pos), v(self.end.tangent));
        let c = p0 * 6.0 + t0 * 3.0 + t1 * 3.0 - p1 * 6.0;
        let d = p0 * -6.0 - t0 * 4.0 - t1 * 2.0 + p1 * 6.0;
        (c * a * a + d * a + t0).normalize_or(Vec3::X)
    }

    /// (origin, axis across X, axis across Y, forward, scale) of the slice at `distance` along
    /// the forward axis, all in UE component space.
    fn slice(&self, distance: f32, min: f32, max: f32) -> (Vec3, Vec3, Vec3, Vec3, Vec2) {
        let alpha = if (max - min).abs() > f32::EPSILON { (distance - min) / (max - min) } else { 0.0 };
        let hermite = if self.smooth { alpha * alpha * (3.0 - 2.0 * alpha) } else { alpha };
        let mut pos = self.eval_pos(alpha);
        let dir = self.eval_dir(alpha);
        let base_x = v(self.up).cross(dir).normalize_or(Vec3::Y);
        let base_y = dir.cross(base_x).normalize_or(Vec3::Z);
        let off = Vec2::from(self.start.offset).lerp(Vec2::from(self.end.offset), hermite);
        pos += base_x * off.x + base_y * off.y;
        let roll = self.start.roll + (self.end.roll - self.start.roll) * hermite;
        let x = base_x * roll.cos() - base_y * roll.sin();
        let y = base_y * roll.cos() + base_x * roll.sin();
        let scale = Vec2::from(self.start.scale).lerp(Vec2::from(self.end.scale), hermite);
        (pos, x, y, dir, scale)
    }

    /// Component-to-world as an affine (from `transform`'s columns).
    pub fn affine(&self) -> bevy::math::Affine3A {
        let t = &self.transform;
        bevy::math::Affine3A::from_cols(Vec3::new(t[0], t[1], t[2]).into(), Vec3::new(t[3], t[4], t[5]).into(), Vec3::new(t[6], t[7], t[8]).into(), Vec3::new(t[9], t[10], t[11]).into())
    }

    /// Whether world point `p` lies in the deformed copy of a source mesh whose bounds (Bevy
    /// space) are `lo..hi`, as the mesh sweeps along the spline: the nearest of `SAMPLES`
    /// slices, then the point across it within the scaled cross-section.
    pub fn contains(&self, p: Vec3, lo: Vec3, hi: Vec3) -> bool {
        const SAMPLES: usize = 64;
        let local = to_ue(self.affine().inverse().transform_point3(p));
        let (ulo, uhi) = (to_ue(lo), to_ue(hi));
        let (ulo, uhi) = (ulo.min(uhi), ulo.max(uhi));
        let (min, max) = if (self.boundary[1] - self.boundary[0]).abs() > f32::EPSILON { (self.boundary[0], self.boundary[1]) } else { (axis_value(ulo, self.axis), axis_value(uhi, self.axis)) };
        let across = |v: Vec3| match self.axis {
            0 => Vec2::new(v.y, v.z),
            1 => Vec2::new(v.z, v.x),
            _ => Vec2::new(v.x, v.y),
        };
        let (alo, ahi) = (across(ulo), across(uhi));
        let nearest = (0..=SAMPLES)
            .map(|i| self.slice(min + (max - min) * i as f32 / SAMPLES as f32, min, max))
            .min_by(|a, b| (local - a.0).length_squared().total_cmp(&(local - b.0).length_squared()));
        let Some((origin, sx, sy, fwd, scale)) = nearest else { return false };
        let d = local - origin;
        // Between slices (or past an end by more than that): a slice's spacing along the chord.
        let spacing = (self.eval_pos(1.0) - self.eval_pos(0.0)).length() / SAMPLES as f32;
        let (x, y) = (d.dot(sx) / scale.x.abs().max(1e-6), d.dot(sy) / scale.y.abs().max(1e-6));
        d.dot(fwd).abs() <= spacing && (alo.x..=ahi.x).contains(&x) && (alo.y..=ahi.y).contains(&y)
    }

    /// Deformed copy of `src` (a static mesh in Bevy space), still in component space.
    pub fn deform(&self, src: &Mesh) -> Option<Mesh> {
        let Some(VertexAttributeValues::Float32x3(positions)) = src.attribute(Mesh::ATTRIBUTE_POSITION) else { return None };
        let ue: Vec<Vec3> = positions.iter().map(|p| to_ue(Vec3::from_array(*p))).collect();
        let (mut min, mut max) = (self.boundary[0], self.boundary[1]);
        if (max - min).abs() <= f32::EPSILON {
            // No explicit boundary: UE uses the mesh's bounds along the forward axis.
            min = ue.iter().map(|p| axis_value(*p, self.axis)).fold(f32::MAX, f32::min);
            max = ue.iter().map(|p| axis_value(*p, self.axis)).fold(f32::MIN, f32::max);
        }

        let normals = match src.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(VertexAttributeValues::Float32x3(n)) => Some(n.clone()),
            _ => None,
        };
        let mut out_pos = Vec::with_capacity(ue.len());
        let mut out_nrm = Vec::with_capacity(ue.len());
        for (i, p) in ue.iter().enumerate() {
            let (origin, sx, sy, fwd, scale) = self.slice(axis_value(*p, self.axis), min, max);
            // Which source axes lie across the slice (UE's FTransform(X, Y, Z) per ForwardAxis).
            let (across_x, across_y, map): (f32, f32, fn(Vec3) -> (f32, f32, f32)) = match self.axis {
                0 => (p.y, p.z, |n| (n.y, n.z, n.x)),
                1 => (p.z, p.x, |n| (n.z, n.x, n.y)),
                _ => (p.x, p.y, |n| (n.x, n.y, n.z)),
            };
            let world = origin + sx * across_x * scale.x + sy * across_y * scale.y;
            out_pos.push(from_ue(world).to_array());
            if let Some(ns) = &normals {
                let (a, b, f) = map(to_ue(Vec3::from_array(ns[i])) / 100.0);
                out_nrm.push(from_ue((sx * a + sy * b + fwd * f) * 100.0).normalize_or(Vec3::Y).to_array());
            }
        }
        let mut mesh = src.clone();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, out_pos);
        if normals.is_some() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, out_nrm);
        }
        // Tangents no longer match the bent surface; nothing here uses normal maps.
        mesh.remove_attribute(Mesh::ATTRIBUTE_TANGENT);
        Some(mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::PrimitiveTopology;

    fn straight(axis: u8) -> SplineMesh {
        let end = |x: f32| SplineEnd { pos: [x, 0.0, 0.0], tangent: [200.0, 0.0, 0.0], scale: [1.0, 1.0], roll: 0.0, offset: [0.0, 0.0] };
        SplineMesh { mesh: String::new(), transform: [0.0; 12], axis, up: [0.0, 0.0, 1.0], boundary: [0.0, 0.0], smooth: false, start: end(0.0), end: end(200.0) }
    }

    #[test]
    fn a_point_is_inside_the_swept_box() {
        // A 4 m cube (Bevy -2..2) swept 2 m along UE X, its cross-section scaled (20, 2):
        // 80 m across (UE Y), 8 m up (UE Z); the component at the origin.
        let mut s = straight(0);
        s.transform = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
        s.start.scale = [20.0, 2.0];
        s.end.scale = [20.0, 2.0];
        let (lo, hi) = (Vec3::splat(-2.0), Vec3::splat(2.0));
        // Bevy x = UE X (along), z = UE Y (across), y = UE Z (up).
        assert!(s.contains(Vec3::new(1.0, 0.0, 30.0), lo, hi));
        assert!(s.contains(Vec3::new(1.0, -3.9, -39.0), lo, hi));
        assert!(!s.contains(Vec3::new(1.0, 0.0, 41.0), lo, hi), "beyond the width");
        assert!(!s.contains(Vec3::new(1.0, 4.5, 0.0), lo, hi), "above the top");
        assert!(!s.contains(Vec3::new(3.0, 0.0, 0.0), lo, hi), "past the end");
    }

    #[test]
    fn straight_spline_along_x_keeps_cross_section() {
        // A 1 m long box (UE X 0..100 cm) stretched along a 2 m straight spline on X.
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0, 0.0, 0.0], [1.0, 0.5, 0.25], [0.5, 0.0, -0.25]]);
        let out = straight(0).deform(&m).unwrap();
        let Some(VertexAttributeValues::Float32x3(p)) = out.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
        // x stretched 0..1 -> 0..2, height (bevy y) and side (bevy z) preserved.
        assert!((Vec3::from(p[0]) - Vec3::new(0.0, 0.0, 0.0)).length() < 1e-3, "{:?}", p[0]);
        assert!((Vec3::from(p[1]) - Vec3::new(2.0, 0.5, 0.25)).length() < 1e-3, "{:?}", p[1]);
        assert!((Vec3::from(p[2]) - Vec3::new(1.0, 0.0, -0.25)).length() < 1e-3, "{:?}", p[2]);
    }
}
