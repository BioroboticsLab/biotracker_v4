use super::protocol::{Arena, Features, Pose};
use anyhow::Result;
use cv::{core::Point2f, prelude::*, types::VectorOfPoint2f};

impl Arena {
    pub fn rectification_transform(&self) -> Result<Mat> {
        let src_corners: VectorOfPoint2f = self
            .rectification_corners
            .iter()
            .map(|c| Point2f::new(c.x as f32, c.y as f32))
            .collect();
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
}

pub fn rectify_features(features: &mut Features, rectification_transform: &Mat) -> Result<()> {
    if let Some(skeleton) = &features.skeleton {
        for feature in features.features.iter_mut() {
            let front = &feature.nodes[skeleton.front_index as usize];
            let center = &feature.nodes[skeleton.center_index as usize];
            let front = px_to_cm(
                &cv::core::Point2f::new(front.x, front.y),
                &rectification_transform,
            )?;
            let center = px_to_cm(
                &cv::core::Point2f::new(center.x, center.y),
                &rectification_transform,
            )?;

            let midline = front - center;
            let direction = midline / midline.norm() as f32;
            let mut orientation_rad = direction.x.atan2(direction.y) + std::f32::consts::PI / 2.0;
            if orientation_rad.is_nan() {
                // happens if center == front
                orientation_rad = 0.0;
            }
            feature.pose = Some(Pose {
                orientation_rad,
                x_cm: center.x,
                y_cm: center.y,
            });
        }
    }
    Ok(())
}

fn px_to_cm(p: &Point2f, rectification_transform: &Mat) -> Result<Point2f> {
    let p = cv::prelude::Mat::from_slice(&[p.x as f64, p.y as f64, 1.0])?;
    let mut rectified = p.clone();
    cv::core::gemm(
        &rectification_transform,
        &p.t()?,
        1.0,
        &cv::core::Mat::default(),
        0.0,
        &mut rectified,
        0,
    )?;
    let x: f64 = *rectified.at(0).unwrap();
    let y: f64 = *rectified.at(1).unwrap();
    let z: f64 = *rectified.at(2).unwrap();
    Ok(Point2f::new((x / z) as f32, (y / z) as f32))
}
