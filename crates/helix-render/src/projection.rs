//! Projection (MATH-3, ZB-1): reversed-Z with infinite far for the default
//! path; forward-Z fallback. z_ndc = near / (-z_view): 1.0 at the near
//! plane, -> 0 at infinity. Derived in spec §ZB; verified below.
use glam::{Mat4, Vec4};

pub fn perspective_rh_reverse_z_infinite(fov_y: f32, aspect: f32, z_near: f32) -> Mat4 {
    let f = 1.0 / (fov_y / 2.0).tan();
    Mat4::from_cols(
        Vec4::new(f / aspect, 0.0, 0.0, 0.0),
        Vec4::new(0.0, f, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, -1.0),
        Vec4::new(0.0, 0.0, z_near, 0.0),
    )
}

pub fn view_proj(fov_y: f32, aspect: f32, near: f32, far: f32, depth_reversed: bool) -> Mat4 {
    if depth_reversed {
        debug_assert!(far >= near);
        perspective_rh_reverse_z_infinite(fov_y, aspect, near)
    } else {
        Mat4::perspective_rh(fov_y, aspect, near, far)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn ndc_z(m: &Mat4, vz: f32) -> f32 {
        let c = *m * Vec4::new(0.0, 0.0, vz, 1.0);
        c.z / c.w
    }

    /// [T] ZB-1 — reversed-Z endpoints + monotonicity; forward-Z endpoints.
    #[test]
    fn endpoints() {
        let rz = perspective_rh_reverse_z_infinite(1.0, 1.0, 0.1);
        assert!((ndc_z(&rz, -0.1) - 1.0).abs() < 1e-6);
        assert!(ndc_z(&rz, -1e6) < 1e-4);
        assert!(ndc_z(&rz, -0.1) > ndc_z(&rz, -1.0));
        assert!(ndc_z(&rz, -1.0) > ndc_z(&rz, -100.0));

        let fw = Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0);
        assert!(ndc_z(&fw, -0.1).abs() < 1e-6);
        assert!((ndc_z(&fw, -100.0) - 1.0).abs() < 1e-5);
    }

    /// [T] REN-13/ZB-3 — culling correct under both depth conventions.
    #[test]
    fn frustum_both_conventions() {
        for reversed in [false, true] {
            let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y);
            let vp = view_proj(1.0, 1.0, 0.1, 100.0, reversed) * view;
            let f = crate::Frustum::from_view_proj(&vp, reversed);
            assert!(f.sphere_visible(Vec3::ZERO, 0.5));
            assert!(!f.sphere_visible(Vec3::new(0.0, 0.0, 200.0), 0.5)); // behind camera
        }
    }
}
