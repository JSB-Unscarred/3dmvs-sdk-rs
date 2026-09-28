//! 图像：SDK 输出的拥有副本，以及图像处理接口的输入校验。

use std::fmt;
use std::ptr;
use std::slice;

use crate::{Error, ImageType, Result, sys};

/// 深度图、轮廓与点云转换所需的标定参数。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ImageCalibration {
    /// X 方向缩放，单位 mm/LSB。
    pub x_scale: f32,
    /// Y 方向缩放，单位 mm/LSB。
    pub y_scale: f32,
    /// Z 方向缩放，单位 mm/LSB。
    pub z_scale: f32,
    /// X 方向偏移，单位 mm。
    pub x_offset: i32,
    /// Y 方向偏移，单位 mm。
    pub y_offset: i32,
    /// Z 方向偏移，单位 mm。
    pub z_offset: i32,
}

/// 一帧图像，像素由 Rust 拥有。
///
/// 取流与图像处理的输出都在返回前从 SDK buffer复制。作为图像处理的输入时，
/// 非压缩格式要求数据长度与宽高、格式严格对应。
#[derive(Clone, PartialEq)]
pub struct Image {
    /// 图像格式。
    pub image_type: ImageType,
    /// 宽度，单位像素。
    pub width: u32,
    /// 高度，单位像素；轮廓数据为行数。
    pub height: u32,
    /// 主数据。
    pub data: Vec<u8>,
    /// 亮度数据，每像素 1 字节。
    pub intensity_data: Option<Vec<u8>>,
    /// 每行的曝光时间戳。
    pub exposure_timestamps: Option<Vec<i64>>,
    /// 帧号。
    pub frame_number: u32,
    /// 设备时间戳。
    pub device_timestamp: i64,
    /// SDK 是否判定该帧有效。
    pub valid: bool,
    /// 标定参数。
    pub calibration: ImageCalibration,
}

impl Image {
    /// 复制 SDK 输出。
    ///
    /// # Safety
    ///
    /// `raw` 来自成功的 SDK 调用，其中非空指针在本次调用期间指向声明长度的有效数据；
    /// 曝光时间戳指针指向 `nHeight` 个 `i64`。
    pub(crate) unsafe fn from_raw(raw: &sys::MV3D_LP_IMAGE_DATA) -> Self {
        // SAFETY: 见函数的 Safety 约定。
        unsafe {
            Self {
                image_type: ImageType::from_raw(raw.enImageType.cast_unsigned()),
                width: raw.nWidth,
                height: raw.nHeight,
                data: copy(raw.pData, raw.nDataLen).unwrap_or_default(),
                intensity_data: copy(raw.pIntensityData, raw.nIntensityDataLen),
                exposure_timestamps: copy(raw.pExposureTimeStamp, raw.nHeight),
                frame_number: raw.nFrameNum,
                device_timestamp: raw.nTimeStamp,
                valid: raw.bValid != 0,
                calibration: ImageCalibration {
                    x_scale: raw.fXScale,
                    y_scale: raw.fYScale,
                    z_scale: raw.fZScale,
                    x_offset: raw.nXOffset,
                    y_offset: raw.nYOffset,
                    z_offset: raw.nZOffset,
                },
            }
        }
    }

    /// 借用本图像构造 SDK 输入。
    ///
    /// SDK 按宽高与格式读取buffer，长度不足会越界读，因此只放行本 crate 能校验长度的格式：
    /// 非压缩格式要求长度与宽高、位数严格对应，JPEG 只要求非空，其余格式返回
    /// [`Error::InvalidInput`]。返回值借用 `self` 的buffer，调用方须在 `self` 存活期间使用。
    pub(crate) fn to_raw(&self) -> Result<sys::MV3D_LP_IMAGE_DATA> {
        let pixels = u64::from(self.width) * u64::from(self.height);
        let data_matches = match self.image_type {
            ImageType::JPEG => !self.data.is_empty(),
            ImageType::MONO8
            | ImageType::DEPTH
            | ImageType::RGB24_PACKED
            | ImageType::PROFILE
            | ImageType::POINT_CLOUD
            | ImageType::PROFILE_ABC32 => {
                pixels.checked_mul(u64::from(self.image_type.bits_per_pixel()))
                    == (self.data.len() as u64).checked_mul(8)
            }
            _ => return Err(Error::InvalidInput("unsupported image type")),
        };
        if !data_matches {
            return Err(Error::InvalidInput(
                "image data length does not match width, height and image type",
            ));
        }
        if self
            .intensity_data
            .as_ref()
            .is_some_and(|data| data.len() as u64 != pixels)
        {
            return Err(Error::InvalidInput(
                "intensity data length does not match the pixel count",
            ));
        }
        if self
            .exposure_timestamps
            .as_ref()
            .is_some_and(|stamps| stamps.len() as u64 != u64::from(self.height))
        {
            return Err(Error::InvalidInput(
                "exposure timestamp count does not match the row count",
            ));
        }
        let too_large = || Error::InvalidInput("image data exceeds 4 GiB");

        Ok(sys::MV3D_LP_IMAGE_DATA {
            enImageType: self.image_type.raw().cast_signed(),
            nWidth: self.width,
            nHeight: self.height,
            pData: self.data.as_ptr().cast_mut(),
            nDataLen: u32::try_from(self.data.len()).map_err(|_| too_large())?,
            pIntensityData: self
                .intensity_data
                .as_ref()
                .map_or(ptr::null_mut(), |data| data.as_ptr().cast_mut()),
            nIntensityDataLen: self
                .intensity_data
                .as_ref()
                .map_or(Ok(0), |data| u32::try_from(data.len()))
                .map_err(|_| too_large())?,
            nFrameNum: self.frame_number,
            nTimeStamp: self.device_timestamp,
            bValid: sys::BOOL::from(self.valid),
            fXScale: self.calibration.x_scale,
            fYScale: self.calibration.y_scale,
            fZScale: self.calibration.z_scale,
            nXOffset: self.calibration.x_offset,
            nYOffset: self.calibration.y_offset,
            nZOffset: self.calibration.z_offset,
            pExposureTimeStamp: self
                .exposure_timestamps
                .as_ref()
                .map_or(ptr::null_mut(), |stamps| stamps.as_ptr().cast_mut()),
            nReserved: [0; 12],
        })
    }
}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Image")
            .field("image_type", &self.image_type)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("data_len", &self.data.len())
            .field("frame_number", &self.frame_number)
            .field("valid", &self.valid)
            .finish_non_exhaustive()
    }
}

/// 复制 SDK buffer；空指针或零长度视为不存在。
///
/// # Safety
///
/// 非空的 `data` 指向 `len` 个有效元素。
unsafe fn copy<T: Copy>(data: *const T, len: u32) -> Option<Vec<T>> {
    if data.is_null() || len == 0 {
        return None;
    }
    // SAFETY: 见函数的 Safety 约定。
    Some(unsafe { slice::from_raw_parts(data, len as usize) }.to_vec())
}

#[cfg(test)]
mod tests {
    use super::Image;
    use crate::{Error, ImageType, sys};

    // 作为 SDK 输入时只放行能校验长度的格式：非压缩格式按宽高与位数严格校验，JPEG 只要求非空，
    // 未知格式被拒绝，避免 SDK 越界读。
    #[test]
    fn input_length_is_checked_before_calling_the_sdk() {
        let mut data = [1_u8, 2];
        let raw = sys::MV3D_LP_IMAGE_DATA {
            enImageType: sys::ImageType_Mono8,
            nWidth: 2,
            nHeight: 1,
            pData: data.as_mut_ptr(),
            nDataLen: 2,
            ..Default::default()
        };
        // SAFETY: pData 指向上面的 2 字节。
        let image = unsafe { Image::from_raw(&raw) };
        let accepted = |image: Image| !matches!(image.to_raw(), Err(Error::InvalidInput(_)));

        assert!(accepted(image.clone()));
        assert!(!accepted(Image {
            data: vec![1],
            ..image.clone()
        }));
        assert!(!accepted(Image {
            image_type: ImageType::from_raw(0x0108_0002),
            ..image.clone()
        }));
        assert!(accepted(Image {
            image_type: ImageType::JPEG,
            data: vec![0xFF],
            ..image
        }));
    }
}
