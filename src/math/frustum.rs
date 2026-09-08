use macroquad::prelude::*;

/// A conservative frustum extracted from the same view/projection matrices used
/// by [`super::intersection::world_to_screen`]. This lets renderers skip labels
/// that cannot appear on screen without paying for a full matrix projection.
#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    /// Builds the frustum that `world_to_screen` uses for the given camera.
    pub fn from_camera(camera: &Camera3D, aspect: f32) -> Self {
        let view = Mat4::look_at_rh(camera.position, camera.target, camera.up);
        let proj = Mat4::perspective_rh_gl(camera.fovy.to_radians(), aspect, 0.1, 1000.0);
        let clip = proj * view;

        let r0 = clip.row(0);
        let r1 = clip.row(1);
        let r2 = clip.row(2);
        let r3 = clip.row(3);

        Self {
            planes: [
                r0 + r3, // left:   x >= -w
                r3 - r0, // right:  x <=  w
                r1 + r3, // bottom: y >= -w
                r3 - r1, // top:    y <=  w
                r2 + r3, // near:   z >= -w
                r3 - r2, // far:    z <=  w
            ],
        }
    }

    pub fn contains_point(&self, point: Vec3) -> bool {
        self.planes
            .iter()
            .all(|plane| point.x * plane.x + point.y * plane.y + point.z * plane.z + plane.w >= 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Frustum;
    use macroquad::prelude::*;

    fn camera_at_origin_looking_forward() -> Camera3D {
        Camera3D {
            position: Vec3::new(0.0, 0.0, -10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            // The project's world_to_screen treats Camera3D.fovy as degrees.
            fovy: 45.0,
            projection: Projection::Perspective,
            ..Default::default()
        }
    }

    #[test]
    fn points_in_front_are_visible() {
        let frustum = Frustum::from_camera(&camera_at_origin_looking_forward(), 1.0);
        assert!(frustum.contains_point(Vec3::new(0.0, 0.0, 0.0)));
        assert!(frustum.contains_point(Vec3::new(1.0, 1.0, 5.0)));
    }

    #[test]
    fn points_behind_the_camera_are_culled() {
        let frustum = Frustum::from_camera(&camera_at_origin_looking_forward(), 1.0);
        assert!(!frustum.contains_point(Vec3::new(0.0, 0.0, -20.0)));
    }

    #[test]
    fn points_far_off_screen_are_culled() {
        let frustum = Frustum::from_camera(&camera_at_origin_looking_forward(), 1.0);
        assert!(!frustum.contains_point(Vec3::new(100.0, 0.0, 0.0)));
    }
}
