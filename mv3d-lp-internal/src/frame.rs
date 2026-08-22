use std::fmt;

use crate::bindings;
use crate::bits::bit_newtype;

bit_newtype! {
    /// An image format reported by the SDK, preserving unknown 32-bit values.
    pub struct ImageType;
    UNDEFINED = bindings::ImageType_Undefined as u32 => "undefined",
    MONO8 = bindings::ImageType_Mono8 as u32 => "Mono8",
    DEPTH = bindings::ImageType_Depth as u32 => "depth",
    PROFILE = bindings::ImageType_Profile as u32 => "profile",
    POINT_CLOUD = bindings::ImageType_PointCloud as u32 => "point cloud",
    RGB24_PACKED = bindings::ImageType_RGB24_Packed as u32 => "RGB24 packed",
    JPEG = bindings::ImageType_Jpeg as u32 => "JPEG",
    PROFILE_ABC32 = bindings::ImageType_Profile_ABC32 as u32 => "profile ABC32",
}

/// Calibration metadata used when converting depth, profile, and point-cloud images.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ImageCalibration {
    pub x_scale: f32,
    pub y_scale: f32,
    pub z_scale: f32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub z_offset: i32,
}

/// A borrowed image view accepted by SDK image-processing helpers.
///
/// Known uncompressed formats require exactly their tightly packed pixel payload. Optional
/// intensity data contains one byte per pixel, and exposure timestamps contain one entry per row.
/// Unknown image types pass their complete non-empty payload to the SDK.
#[derive(Clone, Copy)]
pub struct ImageRef<'a> {
    pub image_type: ImageType,
    pub width: u32,
    pub height: u32,
    pub data: &'a [u8],
    pub intensity_data: Option<&'a [u8]>,
    pub exposure_timestamps: Option<&'a [i64]>,
    pub frame_number: u32,
    pub device_timestamp: i64,
    pub valid: bool,
    pub calibration: ImageCalibration,
}

impl fmt::Debug for ImageRef<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImageRef")
            .field("image_type", &self.image_type)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("data_len", &self.data.len())
            .field("intensity_data_len", &self.intensity_data.map(<[u8]>::len))
            .field(
                "exposure_timestamps_len",
                &self.exposure_timestamps.map(<[i64]>::len),
            )
            .field("frame_number", &self.frame_number)
            .field("device_timestamp", &self.device_timestamp)
            .field("valid", &self.valid)
            .field("calibration", &self.calibration)
            .finish()
    }
}

/// An acquired or processed image whose payload is owned by Rust.
#[derive(Clone, PartialEq)]
#[non_exhaustive]
pub struct Image {
    pub image_type: ImageType,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub intensity_data: Option<Vec<u8>>,
    pub exposure_timestamps: Option<Vec<i64>>,
    pub frame_number: u32,
    pub device_timestamp: i64,
    pub valid: bool,
    pub calibration: ImageCalibration,
}

impl fmt::Debug for Image {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Image")
            .field("image_type", &self.image_type)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("data_len", &self.data.len())
            .field(
                "intensity_data_len",
                &self.intensity_data.as_ref().map(Vec::len),
            )
            .field(
                "exposure_timestamps_len",
                &self.exposure_timestamps.as_ref().map(Vec::len),
            )
            .field("frame_number", &self.frame_number)
            .field("device_timestamp", &self.device_timestamp)
            .field("valid", &self.valid)
            .field("calibration", &self.calibration)
            .finish()
    }
}

impl Image {
    /// Deep-copies an [`ImageRef`] so the result is independent of its source buffers.
    #[must_use]
    pub fn from_image_ref(image: ImageRef<'_>) -> Self {
        Self {
            image_type: image.image_type,
            width: image.width,
            height: image.height,
            data: image.data.to_vec(),
            intensity_data: image.intensity_data.map(<[u8]>::to_vec),
            exposure_timestamps: image.exposure_timestamps.map(<[i64]>::to_vec),
            frame_number: image.frame_number,
            device_timestamp: image.device_timestamp,
            valid: image.valid,
            calibration: image.calibration,
        }
    }

    /// Borrows this image as an image-processing input.
    #[must_use]
    pub fn as_image_ref(&self) -> ImageRef<'_> {
        ImageRef {
            image_type: self.image_type,
            width: self.width,
            height: self.height,
            data: &self.data,
            intensity_data: self.intensity_data.as_deref(),
            exposure_timestamps: self.exposure_timestamps.as_deref(),
            frame_number: self.frame_number,
            device_timestamp: self.device_timestamp,
            valid: self.valid,
            calibration: self.calibration,
        }
    }
}

/// A file representation supported by the vendor image writer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(i32)]
#[non_exhaustive]
pub enum ImageFileFormat {
    Ply = bindings::FileType_PLY,
    Csv = bindings::FileType_CSV,
    Obj = bindings::FileType_OBJ,
    Bmp = bindings::FileType_BMP,
    Jpeg = bindings::FileType_JPG,
    Tiff = bindings::FileType_TIFF,
    TiffU16 = bindings::FileType_TIFF_U16,
    TiffF32 = bindings::FileType_TIFF_F32,
    PlyBinary = bindings::FileType_PLY_BINARY,
    PlyTexture = bindings::FileType_PLY_TEXTURE,
    Hibag = bindings::FileType_HIBAG,
}
