use cgmath::{vec2, vec3, Vector2, Vector3};

pub struct Cube {
    pub center: Vector3<f32>,
    pub x_extent: f32,
    pub y_extent: f32,
    pub z_extent: f32,
}

impl Cube {
    #[rustfmt::skip]
    pub fn corners(&self) -> Vec<Vector3<f32>> {
        let axes = [
            Vector3::unit_x() * self.x_extent,
            Vector3::unit_y() * self.y_extent,
            Vector3::unit_z() * self.z_extent,
        ];
        vec![
            self.center - axes[0] - axes[1] - axes[2],
            self.center + axes[0] - axes[1] - axes[2],
            self.center + axes[0] + axes[1] - axes[2],
            self.center - axes[0] + axes[1] - axes[2],
            self.center + axes[0] - axes[1] + axes[2],
            self.center - axes[0] - axes[1] + axes[2],
            self.center + axes[0] + axes[1] + axes[2],
            self.center - axes[0] + axes[1] + axes[2],
        ]
    }

    #[rustfmt::skip]
    pub fn positions(&self) -> Vec<Vector3<f32>> {
        let v = self.corners();
        vec![
            vec3(v[0].x, v[0].y, v[0].z), vec3(v[1].x, v[1].y, v[1].z), vec3(v[2].x, v[2].y, v[2].z), vec3(v[3].x, v[3].y, v[3].z), // back
            vec3(v[1].x, v[1].y, v[1].z), vec3(v[4].x, v[4].y, v[4].z), vec3(v[6].x, v[6].y, v[6].z), vec3(v[2].x, v[2].y, v[2].z), // right
            vec3(v[4].x, v[4].y, v[4].z), vec3(v[5].x, v[5].y, v[5].z), vec3(v[7].x, v[7].y, v[7].z), vec3(v[6].x, v[6].y, v[6].z), // front
            vec3(v[5].x, v[5].y, v[5].z), vec3(v[0].x, v[0].y, v[0].z), vec3(v[3].x, v[3].y, v[3].z), vec3(v[7].x, v[7].y, v[7].z), // left
            vec3(v[2].x, v[2].y, v[2].z), vec3(v[6].x, v[6].y, v[6].z), vec3(v[7].x, v[7].y, v[7].z), vec3(v[3].x, v[3].y, v[3].z), // top
            vec3(v[0].x, v[0].y, v[0].z), vec3(v[5].x, v[5].y, v[5].z), vec3(v[4].x, v[4].y, v[4].z), vec3(v[1].x, v[1].y, v[1].z), // bottom
        ]
    }

    #[rustfmt::skip]
    pub fn indices(&self) -> &'static [u16] {
        &[
            2,   1,  0,  3,  2,  0, // back
            6,   5,  4,  7,  6,  4, // right
            10,  9,  8, 11, 10,  8, // front
            14, 13, 12, 15, 14, 12, // left
            18, 17, 16, 19, 18, 16, // top
            22, 21, 20, 23, 22, 20  // bottom
        ]
    }

    #[rustfmt::skip]
    pub fn normals(&self) -> Vec<Vector3<f32>> {
        vec![
            vec3( 0.0,  0.0, -1.0), vec3( 0.0,  0.0, -1.0), vec3( 0.0,  0.0, -1.0), vec3( 0.0,  0.0, -1.0), // back
            vec3( 1.0,  0.0,  0.0), vec3( 1.0,  0.0,  0.0), vec3( 1.0,  0.0,  0.0), vec3( 1.0,  0.0,  0.0), // right
            vec3( 0.0,  0.0,  1.0), vec3( 0.0,  0.0,  1.0), vec3( 0.0,  0.0,  1.0), vec3( 0.0,  0.0,  1.0), // front
            vec3(-1.0,  0.0,  0.0), vec3(-1.0,  0.0,  0.0), vec3(-1.0,  0.0,  0.0), vec3(-1.0,  0.0,  0.0), // left
            vec3( 0.0,  1.0,  0.0), vec3( 0.0,  1.0,  0.0), vec3( 0.0,  1.0,  0.0), vec3( 0.0,  1.0,  0.0), // top
            vec3( 0.0, -1.0,  0.0), vec3( 0.0, -1.0,  0.0), vec3( 0.0, -1.0,  0.0), vec3( 0.0, -1.0,  0.0), // bottom
        ]
    }

    #[rustfmt::skip]
    pub fn texcoords(&self) -> Vec<Vector2<f32>> {
        vec![
            vec2(1.0, 0.0), vec2(0.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 1.0), // back
            vec2(1.0, 0.0), vec2(0.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 1.0), // right
            vec2(1.0, 0.0), vec2(0.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 1.0), // front
            vec2(1.0, 0.0), vec2(0.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 1.0), // left
            vec2(1.0, 0.0), vec2(0.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 1.0), // top
            vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0), vec2(0.0, 0.0), // bottom
        ]
    }
}
