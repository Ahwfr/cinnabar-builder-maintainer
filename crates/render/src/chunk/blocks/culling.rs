//! Skips hologram sections that the camera cannot see.

use bevy::math::{Mat4, Vec4};

/// Blocks of slack around a section so models and outlines that reach past its cube stay drawn.
const MARGIN: f32 = 1.0;

/// Whether the sub-chunk cube at `origin` (grid coordinates) may touch the view frustum.
///
/// `clip_from_grid` is the projection times the grid-to-view matrix. Only the side planes and the
/// camera plane are tested: depth is reverse-Z with a far plane the other passes already own, so
/// the test stays conservative. A section is dropped only when all eight corners lie outside the
/// same plane. Non-finite matrices keep everything visible.
pub(super) fn section_visible(clip_from_grid: Mat4, origin: [i32; 3]) -> bool {
    if !clip_from_grid.is_finite() {
        return true;
    }
    let side = world::SUB_CHUNK_SIDE as f32;
    let low = origin.map(|value| value as f32 - MARGIN);
    let high = origin.map(|value| value as f32 + side + MARGIN);
    // Bit `n` of each outside mask means every corner so far is outside plane `n`.
    let mut outside = 0b11_1111u8;
    for corner in 0..8 {
        let point = Vec4::new(
            if corner & 1 == 0 { low[0] } else { high[0] },
            if corner & 2 == 0 { low[1] } else { high[1] },
            if corner & 4 == 0 { low[2] } else { high[2] },
            1.0,
        );
        let clip = clip_from_grid * point;
        let planes = [
            clip.x < -clip.w,
            clip.x > clip.w,
            clip.y < -clip.w,
            clip.y > clip.w,
            clip.w <= 0.0,
            // Clip z outside 0..w is left to the depth test: the far plane may be infinite.
            false,
        ];
        let mut mask = 0u8;
        for (bit, is_outside) in planes.into_iter().enumerate() {
            if is_outside {
                mask |= 1 << bit;
            }
        }
        outside &= mask;
        if outside == 0 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec3;

    /// Camera at the origin looking down -Z with a 90 degree reverse-Z infinite projection.
    fn camera() -> Mat4 {
        Mat4::perspective_infinite_reverse_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.1)
    }

    #[test]
    fn a_section_in_front_is_visible() {
        assert!(section_visible(camera(), [-8, -8, -40]));
    }

    #[test]
    fn a_section_behind_the_camera_is_culled() {
        assert!(!section_visible(camera(), [-8, -8, 20]));
    }

    #[test]
    fn a_section_far_to_the_side_is_culled() {
        assert!(!section_visible(camera(), [200, -8, -40]));
        assert!(!section_visible(camera(), [-216, -8, -40]));
        assert!(!section_visible(camera(), [-8, 200, -40]));
    }

    #[test]
    fn a_section_straddling_an_edge_stays_visible() {
        // The frustum edge at depth 20 is x = 20, and this cube spans x 12..28.
        assert!(section_visible(camera(), [12, -8, -36]));
    }

    #[test]
    fn a_section_containing_the_camera_is_visible() {
        assert!(section_visible(camera(), [-8, -8, -8]));
    }

    #[test]
    fn the_grid_transform_moves_the_section() {
        let shifted = camera() * Mat4::from_translation(Vec3::new(-300., 0., 0.));
        assert!(!section_visible(shifted, [-8, -8, -40]));
        assert!(section_visible(shifted, [292, -8, -40]));
    }

    #[test]
    fn a_non_finite_matrix_keeps_everything() {
        assert!(section_visible(Mat4::NAN, [0, 0, 0]));
    }
}
