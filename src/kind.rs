//! SDK 枚举值的 Rust 表示。

use std::fmt;

use crate::sys;

/// 图像格式。
///
/// SDK 输出可能包含本 crate 未列出的格式，因此用 newtype 保存原始值。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageType(i32);

impl ImageType {
    /// 未定义。
    pub const UNDEFINED: Self = Self(sys::ImageType_Undefined);
    /// 8 位单色。
    pub const MONO8: Self = Self(sys::ImageType_Mono8);
    /// 深度图，每像素 2 字节。
    pub const DEPTH: Self = Self(sys::ImageType_Depth);
    /// 轮廓数据，每点 6 字节。
    pub const PROFILE: Self = Self(sys::ImageType_Profile);
    /// 点云，每点 12 字节。
    pub const POINT_CLOUD: Self = Self(sys::ImageType_PointCloud);
    /// RGB24。
    pub const RGB24_PACKED: Self = Self(sys::ImageType_RGB24_Packed);
    /// JPEG 压缩数据。
    pub const JPEG: Self = Self(sys::ImageType_Jpeg);
    /// ABC32 轮廓数据，每点 12 字节。
    pub const PROFILE_ABC32: Self = Self(sys::ImageType_Profile_ABC32);

    /// 由 SDK 原始值构造。
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    /// 返回 SDK 原始值。
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// 非压缩格式的每像素字节数；压缩或未知格式返回 `None`。
    pub const fn bytes_per_pixel(self) -> Option<u64> {
        match self {
            Self::MONO8 => Some(1),
            Self::DEPTH => Some(2),
            Self::RGB24_PACKED => Some(3),
            Self::PROFILE => Some(6),
            Self::POINT_CLOUD | Self::PROFILE_ABC32 => Some(12),
            _ => None,
        }
    }
}

impl fmt::Debug for ImageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ImageType(0x{:08X})", self.0)
    }
}

/// [`Sdk::save`](crate::Sdk::save) 支持的文件格式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
#[repr(i32)]
pub enum IpConfigMode {
    /// 静态 IP。
    Static = sys::IpCfgMode_Static,
    /// DHCP。
    Dhcp = sys::IpCfgMode_DHCP,
    /// 链路本地地址（LLA）。
    LinkLocal = sys::IpCfgMode_LLA,
}

impl IpConfigMode {
    pub(crate) const fn from_raw(raw: i32) -> Option<Self> {
        match raw {
            sys::IpCfgMode_Static => Some(Self::Static),
            sys::IpCfgMode_DHCP => Some(Self::Dhcp),
            sys::IpCfgMode_LLA => Some(Self::LinkLocal),
            _ => None,
        }
    }
}
