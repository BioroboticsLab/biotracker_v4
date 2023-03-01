use super::protocol::Arena;
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

pub fn px_to_cm(p: &Point2f, rectification_transform: &Mat) -> Result<Point2f> {
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
