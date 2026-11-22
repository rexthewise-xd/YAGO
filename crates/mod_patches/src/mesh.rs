use crate::error::{Result, WeaverError};
use ini::IniCompiler;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeshPreviewData {
    pub positions: Vec<f32>,
    pub indices: Vec<u32>,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub source_files: Vec<String>,
}

#[derive(Debug, Clone)]
struct ResourceBufInfo {
    filename: String,
    stride: usize,
    format: Option<String>,
}

/// Extracts 3D mesh geometry (vertices and triangle indices) from 3DMigoto mod files (.buf and .ib)
/// for real-time 3D previewing, normalizing the mesh to fit a [-1, 1] unit bounding volume.
pub fn extract_mod_mesh_preview(mod_root: &Path, max_triangles: usize) -> Result<MeshPreviewData> {
    if !mod_root.exists() {
        return Err(WeaverError::Logic(format!(
            "Mod root does not exist: {:?}",
            mod_root
        )));
    }

    // 1. Scan INI files to discover [Resource*] definitions and their bindings
    let mut resource_map: HashMap<String, ResourceBufInfo> = HashMap::new();
    let ini_compiler = IniCompiler::default();

    for entry in WalkDir::new(mod_root)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".yago_backups")
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let name_low = entry.file_name().to_string_lossy().to_lowercase();
            if name_low.ends_with(".ini") || name_low.ends_with(".ini.yago_source") {
                if let Ok(doc) = ini_compiler.compile(entry.path()) {
                    for section in doc.sections {
                        let sec_low = section.name.to_lowercase();
                        if sec_low.starts_with("resource") {
                            let mut filename = None;
                            let mut stride = None;
                            let mut format = None;

                            for item in section.items {
                                if let ini::ast::IniItem::Pair { key, value } = item {
                                    let k = key.trim().to_lowercase();
                                    let v = value.trim().to_string();
                                    if k == "filename" {
                                        filename = Some(v);
                                    } else if k == "stride" {
                                        stride = v.parse::<usize>().ok();
                                    } else if k == "format" {
                                        format = Some(v);
                                    }
                                }
                            }

                            if let Some(f) = filename {
                                // Default stride: 40 for standard 3DMigoto position buffers, 12 if simple XYZ
                                let default_stride = if f.to_lowercase().contains("position") {
                                    40
                                } else {
                                    stride.unwrap_or(40)
                                };

                                resource_map.insert(
                                    section.name.clone(),
                                    ResourceBufInfo {
                                        filename: f,
                                        stride: stride.unwrap_or(default_stride),
                                        format,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Locate candidate .buf and .ib pairs
    // Prefer Position.buf files and matching .ib files
    let mut candidate_pairs: Vec<(PathBuf, usize, PathBuf, bool)> = Vec::new(); // (vb_path, stride, ib_path, is_r16)

    // Check mapping from resources
    let mut vb_resources = Vec::new();
    let mut ib_resources = Vec::new();

    for (_name, res) in &resource_map {
        let f_low = res.filename.to_lowercase();
        if f_low.ends_with(".buf")
            && (f_low.contains("position") || res.stride == 40 || res.stride == 12)
        {
            vb_resources.push(res);
        } else if f_low.ends_with(".ib") || res.format.as_deref().unwrap_or("").contains("UINT") {
            ib_resources.push(res);
        }
    }

    // Try pairing resources
    for vb_res in &vb_resources {
        if let Some(vb_path) = find_file_in_mod(mod_root, &vb_res.filename) {
            // Find best matching ib
            for ib_res in &ib_resources {
                if let Some(ib_path) = find_file_in_mod(mod_root, &ib_res.filename) {
                    let is_r16 = ib_res
                        .format
                        .as_deref()
                        .map(|fmt| fmt.to_uppercase().contains("R16"))
                        .unwrap_or(false);
                    candidate_pairs.push((vb_path.clone(), vb_res.stride, ib_path, is_r16));
                    break;
                }
            }
        }
    }

    // Fallback: direct file search in mod directory if INI resources were not found
    if candidate_pairs.is_empty() {
        let mut buf_files = Vec::new();
        let mut ib_files = Vec::new();

        for entry in WalkDir::new(mod_root)
            .into_iter()
            .filter_entry(|e| e.file_name() != ".yago_backups")
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let name_low = entry.file_name().to_string_lossy().to_lowercase();
                if name_low.ends_with(".buf")
                    && (name_low.contains("position")
                        || name_low.contains("head")
                        || name_low.contains("body"))
                {
                    buf_files.push(entry.path().to_path_buf());
                } else if name_low.ends_with(".ib") {
                    ib_files.push(entry.path().to_path_buf());
                }
            }
        }

        if buf_files.is_empty() {
            // Check any .buf file
            for entry in WalkDir::new(mod_root)
                .into_iter()
                .filter_entry(|e| e.file_name() != ".yago_backups")
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    let name_low = entry.file_name().to_string_lossy().to_lowercase();
                    if name_low.ends_with(".buf") {
                        buf_files.push(entry.path().to_path_buf());
                    }
                }
            }
        }

        for vb_path in buf_files {
            let vb_name = vb_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            // Find closest ib
            let matched_ib = ib_files
                .iter()
                .find(|ib| {
                    let ib_name = ib.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    ib_name == vb_name || vb_name.contains(ib_name) || ib_name.contains(vb_name)
                })
                .or_else(|| ib_files.first());

            if let Some(ib_path) = matched_ib {
                let is_r16 = check_if_r16_index_buffer(ib_path);
                candidate_pairs.push((vb_path, 40, ib_path.clone(), is_r16));
            }
        }
    }

    if candidate_pairs.is_empty() {
        return Err(WeaverError::Logic(
            "No 3D vertex (.buf) or index (.ib) geometry found in mod".to_string(),
        ));
    }

    // 3. Parse positions and indices from the candidate pairs
    let mut all_positions: Vec<f32> = Vec::new();
    let mut all_indices: Vec<u32> = Vec::new();
    let mut source_files: Vec<String> = Vec::new();

    for (vb_path, stride, ib_path, is_r16) in candidate_pairs {
        if all_indices.len() / 3 >= max_triangles {
            break;
        }

        let Ok(vb_bytes) = std::fs::read(&vb_path) else {
            continue;
        };
        let Ok(ib_bytes) = std::fs::read(&ib_path) else {
            continue;
        };

        if vb_bytes.len() < 12 || ib_bytes.is_empty() {
            continue;
        }

        let vertex_offset = (all_positions.len() / 3) as u32;

        // Parse vertex positions (3 x f32 at offset 0 of each stride block)
        let actual_stride = if stride >= 12 { stride } else { 40 };
        let num_vertices = vb_bytes.len() / actual_stride;

        for v_idx in 0..num_vertices {
            let byte_offset = v_idx * actual_stride;
            if byte_offset + 12 <= vb_bytes.len() {
                let x =
                    f32::from_le_bytes(vb_bytes[byte_offset..byte_offset + 4].try_into().unwrap());
                let y = f32::from_le_bytes(
                    vb_bytes[byte_offset + 4..byte_offset + 8]
                        .try_into()
                        .unwrap(),
                );
                let z = f32::from_le_bytes(
                    vb_bytes[byte_offset + 8..byte_offset + 12]
                        .try_into()
                        .unwrap(),
                );

                // Skip NaN or Inf
                if x.is_finite() && y.is_finite() && z.is_finite() {
                    all_positions.push(x);
                    all_positions.push(y);
                    all_positions.push(z);
                } else {
                    all_positions.push(0.0);
                    all_positions.push(0.0);
                    all_positions.push(0.0);
                }
            }
        }

        // Parse indices
        let max_v = (all_positions.len() / 3) as u32;
        if is_r16 {
            let num_indices = ib_bytes.len() / 2;
            for i in 0..num_indices {
                if all_indices.len() / 3 >= max_triangles {
                    break;
                }
                let idx_val =
                    u16::from_le_bytes(ib_bytes[i * 2..i * 2 + 2].try_into().unwrap()) as u32;
                let mapped = vertex_offset + idx_val;
                if mapped < max_v {
                    all_indices.push(mapped);
                }
            }
        } else {
            let num_indices = ib_bytes.len() / 4;
            for i in 0..num_indices {
                if all_indices.len() / 3 >= max_triangles {
                    break;
                }
                let idx_val = u32::from_le_bytes(ib_bytes[i * 4..i * 4 + 4].try_into().unwrap());
                let mapped = vertex_offset + idx_val;
                if mapped < max_v {
                    all_indices.push(mapped);
                }
            }
        }

        let vb_rel = vb_path
            .strip_prefix(mod_root)
            .unwrap_or(&vb_path)
            .to_string_lossy()
            .to_string();
        let ib_rel = ib_path
            .strip_prefix(mod_root)
            .unwrap_or(&ib_path)
            .to_string_lossy()
            .to_string();
        if !source_files.contains(&vb_rel) {
            source_files.push(vb_rel);
        }
        if !source_files.contains(&ib_rel) {
            source_files.push(ib_rel);
        }
    }

    if all_positions.is_empty() || all_indices.is_empty() {
        return Err(WeaverError::Logic(
            "Parsed mesh has no valid vertex or index buffer entries".to_string(),
        ));
    }

    // 4. Normalize vertices to unit bounding cube [-1, 1] centered at origin
    normalize_mesh_vertices(&mut all_positions);

    let vertex_count = all_positions.len() / 3;
    let triangle_count = all_indices.len() / 3;

    Ok(MeshPreviewData {
        positions: all_positions,
        indices: all_indices,
        vertex_count,
        triangle_count,
        source_files,
    })
}

fn find_file_in_mod(root: &Path, rel_or_name: &str) -> Option<PathBuf> {
    let direct = root.join(rel_or_name);
    if direct.exists() {
        return Some(direct);
    }

    let search_name = Path::new(rel_or_name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(rel_or_name)
        .to_lowercase();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".yago_backups")
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if entry.file_name().to_string_lossy().to_lowercase() == search_name {
                return Some(entry.path().to_path_buf());
            }
        }
    }

    None
}

fn check_if_r16_index_buffer(path: &Path) -> bool {
    if let Ok(meta) = path.metadata() {
        let len = meta.len();
        // If file length is not a multiple of 4, it must be 16-bit
        if len % 4 != 0 && len % 2 == 0 {
            return true;
        }
    }
    false
}

fn normalize_mesh_vertices(positions: &mut [f32]) {
    if positions.len() < 3 {
        return;
    }

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut min_z = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut max_z = f32::MIN;

    for i in (0..positions.len()).step_by(3) {
        let x = positions[i];
        let y = positions[i + 1];
        let z = positions[i + 2];

        if x < min_x {
            min_x = x;
        }
        if x > max_x {
            max_x = x;
        }
        if y < min_y {
            min_y = y;
        }
        if y > max_y {
            max_y = y;
        }
        if z < min_z {
            min_z = z;
        }
        if z > max_z {
            max_z = z;
        }
    }

    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;
    let center_z = (min_z + max_z) * 0.5;

    let extent_x = max_x - min_x;
    let extent_y = max_y - min_y;
    let extent_z = max_z - min_z;
    let max_extent = extent_x.max(extent_y).max(extent_z);

    let scale = if max_extent > 0.00001 {
        2.0 / max_extent // Scale to span [-1, 1]
    } else {
        1.0
    };

    for i in (0..positions.len()).step_by(3) {
        positions[i] = (positions[i] - center_x) * scale;
        positions[i + 1] = (positions[i + 1] - center_y) * scale;
        positions[i + 2] = (positions[i + 2] - center_z) * scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_extract_mod_mesh_preview_mock_triangle() {
        let dir = tempdir().unwrap();
        let mod_root = dir.path().join("MockMod");
        std::fs::create_dir_all(&mod_root).unwrap();

        // 1. Create a Position.buf with 3 vertices, stride = 40 (standard 3DMigoto format)
        // Vertex 0: (0.0, 0.0, 0.0)
        // Vertex 1: (10.0, 0.0, 0.0)
        // Vertex 2: (0.0, 10.0, 0.0)
        let mut vb_bytes = Vec::new();
        let make_vertex = |x: f32, y: f32, z: f32| {
            let mut v = Vec::with_capacity(40);
            v.extend_from_slice(&x.to_le_bytes());
            v.extend_from_slice(&y.to_le_bytes());
            v.extend_from_slice(&z.to_le_bytes());
            v.resize(40, 0u8); // 28 padding bytes for normals/uvs/tangents
            v
        };

        vb_bytes.extend(make_vertex(0.0, 0.0, 0.0));
        vb_bytes.extend(make_vertex(10.0, 0.0, 0.0));
        vb_bytes.extend(make_vertex(0.0, 10.0, 0.0));

        let vb_path = mod_root.join("Position.buf");
        std::fs::write(&vb_path, vb_bytes).unwrap();

        // 2. Create an Index buffer (.ib) with 1 triangle (indices: 0, 1, 2) as u32
        let mut ib_bytes = Vec::new();
        ib_bytes.extend_from_slice(&0u32.to_le_bytes());
        ib_bytes.extend_from_slice(&1u32.to_le_bytes());
        ib_bytes.extend_from_slice(&2u32.to_le_bytes());

        let ib_path = mod_root.join("Mesh.ib");
        std::fs::write(&ib_path, ib_bytes).unwrap();

        // 3. Create mod.ini pointing to resources
        let ini_path = mod_root.join("mod.ini");
        let ini_content = r#"
[ResourcePosition]
type = Buffer
stride = 40
filename = Position.buf

[ResourceIB]
type = Buffer
format = DXGI_FORMAT_R32_UINT
filename = Mesh.ib
"#;
        std::fs::write(&ini_path, ini_content).unwrap();

        // 4. Extract preview
        let preview = extract_mod_mesh_preview(&mod_root, 1000).unwrap();

        assert_eq!(preview.vertex_count, 3);
        assert_eq!(preview.triangle_count, 1);
        assert_eq!(preview.indices, vec![0, 1, 2]);

        // 5. Verify vertices are normalized within [-1, 1] unit bounds
        for pos in &preview.positions {
            assert!(
                *pos >= -1.0001 && *pos <= 1.0001,
                "Position coordinate {} must be within [-1, 1]",
                pos
            );
        }
    }
}
