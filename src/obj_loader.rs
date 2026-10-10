use std::fs;
use std::path::Path;
use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};

/// Loads a standard .obj file into a Bevy Mesh with support for positions, UVs, and automatic planar texture mapping
pub fn load_obj_mesh(path: &Path) -> Result<Mesh, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let mut raw_positions: Vec<[f32; 3]> = Vec::new();
    let mut raw_uvs: Vec<[f32; 2]> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                let x: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                let y: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                let z: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                raw_positions.push([x, y, z]);
            }
            Some("vt") => {
                let u: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                let v: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                raw_uvs.push([u, v]);
            }
            Some("f") => {
                let vertex_tokens: Vec<&str> = parts.collect();
                if vertex_tokens.len() >= 3 {
                    let parse_token = |token: &str| -> (usize, Option<usize>) {
                        let segs: Vec<&str> = token.split('/').collect();
                        let v_idx = segs[0].parse::<usize>().unwrap_or(1).saturating_sub(1);
                        let vt_idx = if segs.len() > 1 && !segs[1].is_empty() {
                            segs[1].parse::<usize>().ok().map(|i| i.saturating_sub(1))
                        } else {
                            None
                        };
                        (v_idx, vt_idx)
                    };

                    for i in 1..vertex_tokens.len() - 1 {
                        let trio = [
                            parse_token(vertex_tokens[0]),
                            parse_token(vertex_tokens[i]),
                            parse_token(vertex_tokens[i + 1]),
                        ];

                        for &(v_idx, vt_idx) in &trio {
                            if v_idx < raw_positions.len() {
                                let base = positions.len() as u32;
                                let pos = raw_positions[v_idx];
                                positions.push(pos);

                                let uv = if let Some(vt) = vt_idx {
                                    if vt < raw_uvs.len() {
                                        raw_uvs[vt]
                                    } else {
                                        [pos[0] * 0.5 + 0.5, pos[2] * 0.5 + 0.5]
                                    }
                                } else {
                                    // Planar UV fallback projection
                                    [pos[0] * 0.5 + 0.5, pos[2] * 0.5 + 0.5]
                                };
                                uvs.push(uv);
                                indices.push(base);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();

    Ok(mesh)
}
