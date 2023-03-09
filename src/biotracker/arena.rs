use super::{
    protocol::{Arena, Features, Point, Pose},
    undistort::UndistortMap,
};
use anyhow::Result;
use cv::{core::Point2f, imgproc::point_polygon_test, prelude::*, types::VectorOfPoint2f};

#[derive(Clone, Default)]
pub struct ArenaImpl {
    pub arena: Arena,
    pub rectification_transform: Mat,
    pub tracking_area_contour: VectorOfPoint2f,
}

impl ArenaImpl {
    pub fn new(arena: Arena) -> Result<Self> {
        let rectification_transform = arena.rectification_transform()?;
        let tracking_area_contour = arena.tracking_area_contour(&rectification_transform)?;
        Ok(Self {
            arena,
            rectification_transform,
            tracking_area_contour,
        })
    }

    pub fn features_to_poses(
        &self,
        features: &mut Features,
        undistortion: Option<UndistortMap>,
    ) -> Result<()> {
        if let Some(skeleton) = &features.skeleton {
            for feature in features.features.iter_mut() {
                let front = &feature.nodes[skeleton.front_index as usize];
                let center = &feature.nodes[skeleton.center_index as usize];
                let front = px_to_cm(
                    front.x,
                    front.y,
                    &self.rectification_transform,
                    &undistortion,
                )?;
                let center = px_to_cm(
                    center.x,
                    center.y,
                    &self.rectification_transform,
                    &undistortion,
                )?;

                let midline = front - center;
                let direction = midline / midline.norm() as f32;
                let mut orientation_rad =
                    direction.x.atan2(direction.y) + std::f32::consts::PI / 2.0;
                if orientation_rad.is_nan() {
                    // happens if center == front
                    orientation_rad = 0.0;
                }
                let out_of_bounds =
                    point_polygon_test(&self.tracking_area_contour, center, false)? < 0.0;
                feature.out_of_bounds = out_of_bounds;
                feature.pose = Some(Pose {
                    orientation_rad,
                    x_cm: center.x,
                    y_cm: center.y,
                });
            }
        }
        Ok(())
    }
}

impl Arena {
    pub fn rectification_transform(&self) -> Result<Mat> {
        let src_corners = to_vector_of_point2f(&self.rectification_corners);
        let x = self.width_cm as f32 / 2.0;
        let y = self.height_cm as f32 / 2.0;
        let dst_corners = VectorOfPoint2f::from_iter([
            Point2f::new(-x, y),
            Point2f::new(x, y),
            Point2f::new(x, -y),
            Point2f::new(-x, -y),
        ]);
        let src_mat = Mat::from_exact_iter(src_corners.iter()).unwrap();
        let dst_mat = Mat::from_exact_iter(dst_corners.iter()).unwrap();
        let mat = cv::calib3d::find_homography(&src_mat, &dst_mat, &mut Mat::default(), 0, 3.)?;
        Ok(mat)
    }

    pub fn tracking_area_contour(&self, rectification_transform: &Mat) -> Result<VectorOfPoint2f> {
        let mut area_contour_cm = VectorOfPoint2f::new();
        for p in &self.tracking_area_corners {
            let p_cm = px_to_cm(p.x, p.y, rectification_transform, &None)?;
            area_contour_cm.push(p_cm);
        }
        Ok(area_contour_cm)
    }
}

fn px_to_cm(
    mut x: f32,
    mut y: f32,
    rectification_transform: &Mat,
    undistortion: &Option<UndistortMap>,
) -> Result<Point2f> {
    if let Some(undistortion) = undistortion {
        let p = VectorOfPoint2f::from_iter([Point2f::new(x, y)]);
        let mut undistorted = VectorOfPoint2f::new();
        cv::calib3d::undistort_points(
            &p,
            &mut undistorted,
            &undistortion.camera_matrix,
            &undistortion.distortion_coefficients,
            &Mat::default(),
            &undistortion.new_camera_matrix,
        )?;
        let undistorted = undistorted.iter().next().unwrap();
        x = undistorted.x;
        y = undistorted.y;
    }
    let p = Mat::from_slice(&[x as f64, y as f64, 1.0])?;
    let mut rectified = p.clone();
    cv::core::gemm(
        &rectification_transform,
        &p.t()?,
        1.0,
        &Mat::default(),
        0.0,
        &mut rectified,
        0,
    )?;
    let x: f64 = *rectified.at(0).unwrap();
    let y: f64 = *rectified.at(1).unwrap();
    let z: f64 = *rectified.at(2).unwrap();
    Ok(Point2f::new((x / z) as f32, (y / z) as f32))
}

fn to_vector_of_point2f(points: &[Point]) -> VectorOfPoint2f {
    points
        .iter()
        .map(|p| Point2f::new(p.x as f32, p.y as f32))
        .collect()
}
