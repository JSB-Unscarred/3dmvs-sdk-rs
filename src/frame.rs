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

/// A frame whose payload and metadata are independent of SDK-owned memory.
///
/// Every buffer is copied before this value crosses the FFI boundary. The
/// frame therefore remains valid after another image is acquired or the
/// camera is stopped, cleared, or closed.
#[non_exhaustive]
pub struct OwnedFrame {
    pub image_type: ImageType,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub intensity_data: Option<Vec<u8>>,
    pub exposure_timestamps: Option<Vec<i64>>,
    pub frame_number: u32,
    pub device_timestamp: i64,
    pub valid: bool,
    pub x_scale: f32,
    pub y_scale: f32,
    pub z_scale: f32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub z_offset: i32,
}

impl OwnedFrame {
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
            x_scale: record.x_scale,
            y_scale: record.y_scale,
            z_scale: record.z_scale,
            x_offset: record.x_offset,
            y_offset: record.y_offset,
            z_offset: record.z_offset,
        }
    }
}

impl fmt::Debug for OwnedFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnedFrame")
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
            .field("x_scale", &self.x_scale)
            .field("y_scale", &self.y_scale)
            .field("z_scale", &self.z_scale)
            .field("x_offset", &self.x_offset)
            .field("y_offset", &self.y_offset)
            .field("z_offset", &self.z_offset)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageType, OwnedFrame};

    #[test]
    fn debug_reports_lengths_without_payload_contents() {
        let frame = OwnedFrame {
            image_type: ImageType::MONO8,
            width: 2,
            height: 1,
            data: vec![0xAA, 0xBB],
            intensity_data: Some(vec![0xCC, 0xDD]),
            exposure_timestamps: Some(vec![123_456_789]),
            frame_number: 7,
            device_timestamp: 42,
            valid: true,
            x_scale: 1.0,
            y_scale: 2.0,
            z_scale: 3.0,
            x_offset: 4,
            y_offset: 5,
            z_offset: 6,
        };

        let debug = format!("{frame:?}");

        assert!(debug.contains("data_len: 2"));
        assert!(debug.contains("intensity_data_len: Some(2)"));
        assert!(debug.contains("exposure_timestamps_len: Some(1)"));
        assert!(!debug.contains("170"));
        assert!(!debug.contains("187"));
        assert!(!debug.contains("123456789"));
    }
}
