use super::protocol::{CameraConfig, VideoInfo};
use anyhow::Result;
use cv::{core::Size, prelude::*};
use std::convert::TryFrom;

#[derive(Default, Clone)]
pub struct UndistortMap {
    pub camera_matrix: Mat,
    pub new_camera_matrix: Mat,
    pub distortion_coefficients: Mat,
    pub map1: Mat,
    pub map2: Mat,
}

impl UndistortMap {
    pub fn undistort(&self, src: &Mat, dst: &mut Mat) -> Result<()> {
        cv::imgproc::remap(
            src,
            dst,
            &self.map1,
            &self.map2,
            cv::imgproc::INTER_LINEAR,
            cv::core::BORDER_CONSTANT,
            cv::core::Scalar::default(),
        )?;
        Ok(())
    }
}

impl TryFrom<(&CameraConfig, &VideoInfo)> for UndistortMap {
    type Error = anyhow::Error;

    fn try_from(args: (&CameraConfig, &VideoInfo)) -> Result<Self> {
        let (config, info) = args;
        let camera_matrix = Mat::from_slice_rows_cols(&config.camera_matrix, 3, 3)?;
        let distortion_coefficients = Mat::from_slice(&config.distortion_coefficients)?;
        let image_size = Size::new(info.width as i32, info.height as i32);
        let new_camera_matrix = cv::calib3d::get_optimal_new_camera_matrix(
            &camera_matrix,
            &distortion_coefficients,
            image_size,
            1.0,
            image_size,
            None,
            false,
        )?;
        let mut res = Self {
            camera_matrix: camera_matrix.clone(),
            new_camera_matrix: new_camera_matrix.clone(),
            distortion_coefficients: distortion_coefficients.clone(),
            ..Default::default()
        };
        if config.fisheye {
            cv::calib3d::fisheye_init_undistort_rectify_map(
                &camera_matrix,
                &distortion_coefficients,
                &Mat::default(),
                &new_camera_matrix,
                image_size,
                cv::core::CV_32FC1,
                &mut res.map1,
                &mut res.map2,
            )?;
            unimplemented!()
        } else {
            cv::calib3d::init_undistort_rectify_map(
                &camera_matrix,
                &distortion_coefficients,
                &Mat::default(),
                &new_camera_matrix,
                image_size,
                cv::core::CV_32FC1,
                &mut res.map1,
                &mut res.map2,
            )?;
        }
        Ok(res)
    }
}
