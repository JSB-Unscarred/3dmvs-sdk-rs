use std::fmt;

/// An image format reported by the SDK, preserving unknown 32-bit values.
///
/// This is deliberately a newtype rather than a Rust enum so that image
/// formats introduced by a newer runtime remain representable.
#[repr(transparent)]
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct ImageType(u32);

impl ImageType {
    pub const UNDEFINED: Self = Self(0xFFFF_FFFF);
    pub const MONO8: Self = Self(0x0108_0001);
    pub const DEPTH: Self = Self(0x0110_00B8);
    pub const PROFILE: Self = Self(0x0230_00B9);
    pub const POINT_CLOUD: Self = Self(0x0260_00C0);
    pub const RGB24_PACKED: Self = Self(0x0218_0014);
    pub const JPEG: Self = Self(0x8018_0001);
    pub const PROFILE_ABC32: Self = Self(0x8260_3001);

    #[must_use]
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw as u32)
    }

    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn raw(self) -> i32 {
        self.0 as i32
    }

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn name(self) -> Option<&'static str> {
        match self.0 {
            0xFFFF_FFFF => Some("undefined"),
            0x0108_0001 => Some("Mono8"),
            0x0110_00B8 => Some("depth"),
            0x0230_00B9 => Some("profile"),
            0x0260_00C0 => Some("point cloud"),
            0x0218_0014 => Some("RGB24 packed"),
            0x8018_0001 => Some("JPEG"),
            0x8260_3001 => Some("profile ABC32"),
            _ => None,
        }
    }
}

/// Calibration metadata used when converting depth, profile, and point-cloud images.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageCalibration {
    pub x_scale: f32,
    pub y_scale: f32,
    pub z_scale: f32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub z_offset: i32,
}

impl Default for ImageCalibration {
    fn default() -> Self {
        Self {
            x_scale: 0.0,
            y_scale: 0.0,
            z_scale: 0.0,
            x_offset: 0,
            y_offset: 0,
            z_offset: 0,
        }
    }
}

/// A borrowed image view accepted by [`crate::ImageProcessor`].
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

impl<'a> ImageRef<'a> {
    pub(crate) fn to_internal(self) -> mv3d_lp_internal::ImageInput<'a> {
        mv3d_lp_internal::ImageInput {
            image_type: mv3d_lp_internal::ImageTypeRecord::from_bits(self.image_type.bits()),
            width: self.width,
            height: self.height,
            data: self.data,
            intensity_data: self.intensity_data,
            exposure_timestamps: self.exposure_timestamps,
            frame_number: self.frame_number,
            device_timestamp: self.device_timestamp,
            valid: self.valid,
            x_scale: self.calibration.x_scale,
            y_scale: self.calibration.y_scale,
            z_scale: self.calibration.z_scale,
            x_offset: self.calibration.x_offset,
            y_offset: self.calibration.y_offset,
            z_offset: self.calibration.z_offset,
        }
    }
}

/// An acquired or processed image whose payload is owned by Rust.
///
/// SDK payloads are copied before return, so the image remains valid after later calls, buffer
/// clearing, stopping, or closing the device. Cloning deep-copies all payload buffers.
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

    /// Moves validated internal fields into the public image without copying payload buffers.
    pub(crate) fn from_internal(record: mv3d_lp_internal::FrameRecord) -> Self {
        Self {
            image_type: ImageType::from_bits(record.image_type.bits()),
            width: record.width,
            height: record.height,
            data: record.data,
            intensity_data: record.intensity_data,
            exposure_timestamps: record.exposure_timestamps,
            frame_number: record.frame_number,
            device_timestamp: record.device_timestamp,
            valid: record.valid,
            calibration: ImageCalibration {
                x_scale: record.x_scale,
                y_scale: record.y_scale,
                z_scale: record.z_scale,
                x_offset: record.x_offset,
                y_offset: record.y_offset,
                z_offset: record.z_offset,
            },
        }
    }
}

impl fmt::Debug for ImageType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => write!(formatter, "ImageType({name}, 0x{:08X})", self.0),
            None => write!(formatter, "ImageType(0x{:08X})", self.0),
        }
    }
}

impl fmt::Display for ImageType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => formatter.write_str(name),
            None => write!(formatter, "unknown image type 0x{:08X}", self.0),
        }
    }
}

/// Acquired-frame name for the shared owned image representation.
pub type Frame = Image;
