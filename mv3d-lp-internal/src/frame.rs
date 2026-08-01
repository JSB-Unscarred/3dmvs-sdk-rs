/// The image type bit pattern reported by the SDK.
///
/// This remains a newtype instead of an enum so newer SDK image types can pass through without
/// being rejected or losing their original value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageTypeRecord(u32);

impl ImageTypeRecord {
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw as u32)
    }

    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn raw(self) -> i32 {
        self.0 as i32
    }
}

/// A borrowed image descriptor passed to the SDK image-processing helpers.
///
/// The payload remains owned by the public wrapper. Native descriptors are
/// assembled only for the duration of one serialized SDK call.
#[derive(Clone, Copy, Debug)]
pub struct ImageInput<'a> {
    pub image_type: ImageTypeRecord,
    pub width: u32,
    pub height: u32,
    pub data: &'a [u8],
    pub intensity_data: Option<&'a [u8]>,
    pub exposure_timestamps: Option<&'a [i64]>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ImageFileFormatRecord {
    Ply = 1,
    Csv = 2,
    Obj = 3,
    Bmp = 4,
    Jpeg = 5,
    Tiff = 6,
    TiffU16 = 7,
    TiffF32 = 8,
    PlyBinary = 9,
    PlyTexture = 10,
    Hibag = 11,
}

/// A completely owned copy of one image returned by the SDK.
///
/// No field in this record borrows SDK memory, so it remains valid after later SDK calls and
/// after the originating device has been stopped or closed.
#[derive(Debug, PartialEq)]
pub struct FrameRecord {
    pub image_type: ImageTypeRecord,
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
