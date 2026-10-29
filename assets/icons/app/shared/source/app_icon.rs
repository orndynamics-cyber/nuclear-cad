//! Original Nuclear mark, shared by desktop, packaging and CLI: three electron orbits at 0, 60
//! and 120 degrees around a nucleus, on a transparent background. Drawn here from geometry, not
//! from an image, so every export (`cargo xtask app-icons`) stays in step.

/// Orbit half-axes and stroke, as shares of the icon's size.
const ORBIT_A: f32 = 0.46;
const ORBIT_B: f32 = 0.17;
const ORBIT_W: f32 = 0.045;
/// Nucleus radius, as a share of the icon's size.
const NUCLEUS_R: f32 = 0.11;
/// Orbit colours, back to front, and the nucleus colour.
const ORBITS: [[u8; 3]; 3] = [[21, 73, 158], [35, 121, 231], [69, 159, 255]];
const NUCLEUS: [u8; 3] = [255, 140, 26];

/// The colour of the sample at (x, y), in shares of the icon's size from its centre, if any.
fn sample(x: f32, y: f32) -> Option<[u8; 3]> {
    if x * x + y * y <= NUCLEUS_R * NUCLEUS_R {
        return Some(NUCLEUS);
    }
    let mut hit = None;
    for (k, col) in ORBITS.iter().enumerate() {
        let (sin, cos) = (k as f32 * std::f32::consts::PI / 3.0).sin_cos();
        let (u, v) = (x * cos + y * sin, -x * sin + y * cos);
        let (a2, b2) = (ORBIT_A * ORBIT_A, ORBIT_B * ORBIT_B);
        let f = u * u / a2 + v * v / b2 - 1.0;
        let g = ((2.0 * u / a2).powi(2) + (2.0 * v / b2).powi(2)).sqrt().max(1e-6);
        if (f / g).abs() <= ORBIT_W / 2.0 {
            hit = Some(*col);
        }
    }
    hit
}

/// This block is the icon image data
pub fn rgba(size: usize) -> Result<Vec<u8>, String> {
    if !(1..=2048).contains(&size) {
        return Err("icon size must be 1..=2048".into());
    }
    // 4x4 samples per pixel (2x2 for big sizes), averaged down to straight alpha.
    let ss = if size > 512 { 2 } else { 4 };
    let n = (size * ss) as f32;
    let mut rgba = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let (mut sum, mut cnt) = ([0u32; 3], 0u32);
            for j in 0..ss {
                for i in 0..ss {
                    let sx = ((x * ss + i) as f32 + 0.5) / n - 0.5;
                    let sy = ((y * ss + j) as f32 + 0.5) / n - 0.5;
                    if let Some(c) = sample(sx, sy) {
                        cnt += 1;
                        for k in 0..3 {
                            sum[k] += u32::from(c[k]);
                        }
                    }
                }
            }
            if cnt == 0 {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let a = 255 * cnt / (ss * ss) as u32;
                rgba.extend_from_slice(&[(sum[0] / cnt) as u8, (sum[1] / cnt) as u8, (sum[2] / cnt) as u8, a as u8]);
            }
        }
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_mark_has_transparency_and_original_colors_at_window_and_package_sizes() {
        for size in [16, 24, 32, 48, 64, 128, 256] {
            let pixels = super::rgba(size).expect("valid size");
            assert_eq!(pixels.len(), size * size * 4);
            assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 0));
            assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 255 && p[2] > p[0]));
        }
        assert!(super::rgba(0).is_err());
        assert!(super::rgba(2049).is_err());
    }

    /// The nucleus sits in the middle, the corners are see-through, and no opaque pixel is white.
    #[test]
    fn shared_mark_is_an_atom_on_a_clear_background() {
        let size = 256;
        let pixels = super::rgba(size).expect("valid size");
        let px = |x: usize, y: usize| pixels.as_chunks::<4>().0[y * size + x];
        assert_eq!(px(size / 2, size / 2), [255, 140, 26, 255], "the nucleus is at the centre");
        assert_eq!(px(2, 2)[3], 0, "the corners are transparent");
        assert!(pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 0 || p[0] < 200 || p[1] < 200), "no white edges");
    }
}
