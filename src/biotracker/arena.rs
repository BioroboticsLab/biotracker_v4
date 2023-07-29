use super::{
    protocol::{Arena, Features, SkeletonDescriptor},
    undistort::UndistortMap,
    VideoInfo,
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
    pub fn new(arena: Arena, video_info: &Option<VideoInfo>) -> Result<Self> {
        let (px_width, px_height) = match video_info {
            Some(video_info) => (video_info.width, video_info.height),
            None => (1024, 1024),
        };

        let rectification_transform = arena.rectification_transform(px_width, px_height)?;
        let tracking_area_contour =
            arena.tracking_area_contour(&rectification_transform, px_width, px_height)?;
        Ok(Self {
            arena,
            rectification_transform,
            tracking_area_contour,
        })
    }

    pub fn features_to_world(
        &self,
        features: &mut Features,
        skeleton: &SkeletonDescriptor,
        undistortion: Option<UndistortMap>,
    ) -> Result<()> {
        for feature in features.features.iter_mut() {
            feature.world_nodes = feature.image_nodes.clone();
            for (i, node) in feature.image_nodes.iter().enumerate() {
                let (x, y) = (node.x, node.y);
                let cm_pos = px_to_cm(x, y, &self.rectification_transform, &undistortion)?;
                let world_node = &mut feature.world_nodes[i];
                world_node.x = cm_pos.x;
                world_node.y = cm_pos.y;
                // Features containing NaN nodes are technically out of bounds, but we treat them
                // separately: They may still contain other correct nodes, it is up to the plugins
                // to decide what to do with them.
                if i == skeleton.center_index as usize && !cm_pos.x.is_nan() && !cm_pos.y.is_nan() {
                    let out_of_bounds =
                        point_polygon_test(&self.tracking_area_contour, cm_pos, false)? < 0.0;
                    feature.out_of_bounds = Some(out_of_bounds);
                }
            }
        }
        Ok(())
    }
}

impl Arena {
    pub fn rectification_transform(&self, px_width: u32, px_height: u32) -> Result<Mat> {
        // Rectification corners are stored in relative coordinates in range [0.0, 1.0]. We
        // transform these to pixel coordinates here. This is necessary, so that the rectification
        // / tracking areas are independent of video resolution.
        let src_corners: VectorOfPoint2f = self
            .rectification_corners
            .iter()
            .map(|p| Point2f::new(p.x * px_width as f32, p.y * px_height as f32))
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

    pub fn tracking_area_contour(
        &self,
        rectification_transform: &Mat,
        px_width: u32,
        px_height: u32,
    ) -> Result<VectorOfPoint2f> {
        let mut area_contour_cm = VectorOfPoint2f::new();
        for p in &self.tracking_area_corners {
            // again, corners are stored in relative form in range [0.0, 1.0], so that they are
            // decoupled from video resolution.
            let p_cm = px_to_cm(
                p.x * px_width as f32,
                p.y * px_height as f32,
                rectification_transform,
                &None,
            )?;
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
