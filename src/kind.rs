//! SDK 枚举值的 Rust 表示。

use std::fmt;

use crate::sys;

/// 图像格式码，编码与 `GigE` Vision 像素格式相同。
///
/// 未列出的格式可以用 [`ImageType::from_raw`] 表示。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageType(u32);

impl ImageType {
    /// 未定义。
    pub const UNDEFINED: Self = Self(sys::ImageType_Undefined.cast_unsigned());
    /// 8 位单色。
    pub const MONO8: Self = Self(sys::ImageType_Mono8.cast_unsigned());
    /// 深度图，每像素 16 位。
    pub const DEPTH: Self = Self(sys::ImageType_Depth.cast_unsigned());
    /// 轮廓数据，每点 48 位。
    pub const PROFILE: Self = Self(sys::ImageType_Profile.cast_unsigned());
    /// 点云，每点 96 位。
    pub const POINT_CLOUD: Self = Self(sys::ImageType_PointCloud.cast_unsigned());
    /// RGB24。
    pub const RGB24_PACKED: Self = Self(sys::ImageType_RGB24_Packed.cast_unsigned());
    /// JPEG 压缩数据。
    pub const JPEG: Self = Self(sys::ImageType_Jpeg.cast_unsigned());
    /// ABC32 轮廓数据，每点 96 位。
    pub const PROFILE_ABC32: Self = Self(sys::ImageType_Profile_ABC32.cast_unsigned());

    /// 由 SDK 原始值构造。
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// 返回 SDK 原始值。
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// 格式码中编码的每像素位数；对 JPEG 等压缩格式与 [`ImageType::UNDEFINED`] 没有意义。
    pub const fn bits_per_pixel(self) -> u32 {
        (self.0 >> 16) & 0xFF
    }
}

impl fmt::Debug for ImageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ImageType(0x{:08X})", self.0)
    }
}

/// [`Sdk::save_image`](crate::Sdk::save_image) 支持的文件格式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[repr(i32)]
pub enum ImageFileFormat {
    /// ASCII PLY 点云。
    Ply = sys::FileType_PLY,
    /// CSV 文本。
    Csv = sys::FileType_CSV,
    /// OBJ 网格。
    Obj = sys::FileType_OBJ,
    /// BMP 位图。
    Bmp = sys::FileType_BMP,
    /// JPEG 图像。
    Jpeg = sys::FileType_JPG,
    /// TIFF 图像。
    Tiff = sys::FileType_TIFF,
    /// 16 位无符号 TIFF。
    TiffU16 = sys::FileType_TIFF_U16,
    /// 32 位浮点 TIFF。
    TiffF32 = sys::FileType_TIFF_F32,
    /// 二进制 PLY 点云。
    PlyBinary = sys::FileType_PLY_BINARY,
    /// 带纹理的 PLY 点云。
    PlyTexture = sys::FileType_PLY_TEXTURE,
    /// 厂商私有 HIBAG 容器。
    Hibag = sys::FileType_HIBAG,
}

/// 设备的 IP 配置方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum IpConfigMode {
    /// 静态 IP。
    Static,
    /// DHCP。
    Dhcp,
    /// 链路本地地址（LLA）。
    LinkLocal,
    /// 头文件未定义的配置方式。
    Other(i32),
}

impl IpConfigMode {
    /// 由 SDK 原始值构造，未定义的值保存在 [`IpConfigMode::Other`]。
    pub(crate) const fn from_raw(raw: i32) -> Self {
        match raw {
            sys::IpCfgMode_Static => Self::Static,
            sys::IpCfgMode_DHCP => Self::Dhcp,
            sys::IpCfgMode_LLA => Self::LinkLocal,
            other => Self::Other(other),
        }
    }
}
