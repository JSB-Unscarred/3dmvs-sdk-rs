use crate::{Error, Image, ImageRef, ImageType, Operation, Result};

/// A file representation supported by the vendor image writer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(i32)]
#[non_exhaustive]
pub enum ImageFileFormat {
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

impl ImageFileFormat {
    /// Maps the public format to the internal SDK representation.
    fn to_internal(self) -> mv3d_lp_internal::ImageFileFormatRecord {
        match self {
            Self::Ply => mv3d_lp_internal::ImageFileFormatRecord::Ply,
            Self::Csv => mv3d_lp_internal::ImageFileFormatRecord::Csv,
            Self::Obj => mv3d_lp_internal::ImageFileFormatRecord::Obj,
            Self::Bmp => mv3d_lp_internal::ImageFileFormatRecord::Bmp,
            Self::Jpeg => mv3d_lp_internal::ImageFileFormatRecord::Jpeg,
            Self::Tiff => mv3d_lp_internal::ImageFileFormatRecord::Tiff,
            Self::TiffU16 => mv3d_lp_internal::ImageFileFormatRecord::TiffU16,
            Self::TiffF32 => mv3d_lp_internal::ImageFileFormatRecord::TiffF32,
            Self::PlyBinary => mv3d_lp_internal::ImageFileFormatRecord::PlyBinary,
            Self::PlyTexture => mv3d_lp_internal::ImageFileFormatRecord::PlyTexture,
            Self::Hibag => mv3d_lp_internal::ImageFileFormatRecord::Hibag,
        }
    }
}

/// An owned token for the process-wide LPSDK image-processing functions.
///
/// `ImageProcessor` does not borrow [`crate::Sdk`] and is `Send + Sync`. Each call verifies that
/// its session is active, delegates input validation to the internal FFI boundary, and copies SDK
/// output into Rust-owned storage. Calls from the same session are serialized through the native
/// call and immediate output copy, so another call cannot replace transient SDK output during the
/// copy. Successful [`crate::Sdk::shutdown`] invalidates tokens from that session.
///
/// # Native contract
///
/// For the audited LPSDK 1.3.3.3 runtime, the wrapper assumes that input payloads marked `[IN]`
/// are not modified or retained after the synchronous call, and that SDK-owned outputs remain
/// readable and unchanged during the immediate copy. These are disclosed project assumptions,
/// not separate written vendor guarantees.
pub struct ImageProcessor {
    pub(crate) inner: mv3d_lp_internal::Runtime,
}

impl ImageProcessor {
    /// Converts one depth image to a point cloud.
    pub fn depth_to_point_cloud(&self, input: ImageRef<'_>) -> Result<Image> {
        self.inner
            .map_depth_to_point_cloud(input.to_internal())
            .map(Image::from_internal)
            .map_err(Error::map_internal_error)
    }

    /// Converts multiple depth images to one round point cloud.
    pub fn depth_to_round_point_cloud(&self, inputs: &[ImageRef<'_>]) -> Result<Image> {
        let inputs = prepare_multi(Operation::MapDepthToPointCloudRound, inputs)?;
        self.inner
            .map_depth_to_point_cloud_round(&inputs)
            .map(Image::from_internal)
            .map_err(Error::map_internal_error)
    }

    /// Converts an image to a vendor-supported target type.
    pub fn convert(&self, input: ImageRef<'_>, target: ImageType) -> Result<Image> {
        self.inner
            .convert_image(
                input.to_internal(),
                mv3d_lp_internal::ImageTypeRecord::from_bits(target.bits()),
            )
            .map(Image::from_internal)
            .map_err(Error::map_internal_error)
    }

    /// Mosaics multiple depth images.
    pub fn mosaic_depth(&self, inputs: &[ImageRef<'_>]) -> Result<Image> {
        let inputs = prepare_multi(Operation::DepthMosaic, inputs)?;
        self.inner
            .mosaic_depth(&inputs)
            .map(Image::from_internal)
            .map_err(Error::map_internal_error)
    }

    /// Saves an image using the vendor encoder.
    ///
    /// `file_name` is passed as original narrow-string bytes because the SDK
    /// does not document whether paths use UTF-8 or the Windows code page.
    pub fn save(
        &self,
        input: ImageRef<'_>,
        format: ImageFileFormat,
        file_name: &[u8],
    ) -> Result<()> {
        self.inner
            .save_image(input.to_internal(), format.to_internal(), file_name)
            .map_err(Error::map_internal_error)
    }
}

/// Builds borrowed internal descriptors; the FFI boundary validates their contents.
fn prepare_multi<'a>(
    operation: Operation,
    inputs: &[ImageRef<'a>],
) -> Result<Vec<mv3d_lp_internal::ImageInput<'a>>> {
    let mut internal = Vec::new();
    internal
        .try_reserve_exact(inputs.len())
        .map_err(|_| Error::AllocationFailed { operation })?;
    internal.extend(inputs.iter().copied().map(ImageRef::to_internal));
    Ok(internal)
}
