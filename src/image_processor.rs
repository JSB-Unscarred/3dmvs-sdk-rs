use std::marker::PhantomData;
use std::rc::Rc;

use crate::{Error, ImageRef, ImageType, InputViolation, Operation, OwnedImage, Result};

const MAX_MULTI_IMAGE_COUNT: usize = 8;
const MAX_IMAGE_BYTES: usize = 512 * 1024 * 1024;

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

/// Safe access to the process-global LPSDK image-processing functions.
///
/// Each output is validated and copied into Rust-owned storage before the
/// process-wide SDK call lock is released.
///
/// # Native contract
///
/// For the audited LPSDK 1.3.3.3 runtime, the wrapper assumes that input payloads marked `[IN]`
/// are not modified or retained after the synchronous call, and that SDK-owned outputs remain
/// readable and unchanged during the immediate copy. These are disclosed project assumptions,
/// not separate written vendor guarantees.
pub struct ImageProcessor<'sdk> {
    pub(crate) inner: &'sdk mv3d_lp_internal::Runtime,
    pub(crate) _not_send_or_sync: PhantomData<Rc<()>>,
}

impl ImageProcessor<'_> {
    pub fn depth_to_point_cloud(&self, input: ImageRef<'_>) -> Result<OwnedImage> {
        require_type(Operation::MapDepthToPointCloud, input, ImageType::DEPTH)?;
        validate_layout(Operation::MapDepthToPointCloud, input)?;
        validate_expected_output(
            Operation::MapDepthToPointCloud,
            input.width,
            input.height,
            12,
        )?;
        self.inner
            .map_depth_to_point_cloud(input.to_internal())
            .map(OwnedImage::from_internal)
            .map_err(Error::from)
    }

    pub fn depth_to_round_point_cloud(&self, inputs: &[ImageRef<'_>]) -> Result<OwnedImage> {
        let inputs = prepare_multi(Operation::MapDepthToPointCloudRound, inputs)?;
        self.inner
            .map_depth_to_point_cloud_round(&inputs)
            .map(OwnedImage::from_internal)
            .map_err(Error::from)
    }

    pub fn convert(&self, input: ImageRef<'_>, target: ImageType) -> Result<OwnedImage> {
        validate_layout(Operation::ImageConvert, input)?;
        if !conversion_supported(input.image_type, target) {
            return Err(invalid(
                Operation::ImageConvert,
                InputViolation::UnsupportedImageConversion {
                    source: input.image_type.bits(),
                    target: target.bits(),
                },
            ));
        }
        if let Some(bytes_per_pixel) = known_bytes_per_pixel(target) {
            validate_expected_output(
                Operation::ImageConvert,
                input.width,
                input.height,
                bytes_per_pixel,
            )?;
        }
        self.inner
            .convert_image(
                input.to_internal(),
                mv3d_lp_internal::ImageTypeRecord::from_bits(target.bits()),
            )
            .map(OwnedImage::from_internal)
            .map_err(Error::from)
    }

    pub fn mosaic_depth(&self, inputs: &[ImageRef<'_>]) -> Result<OwnedImage> {
        let inputs = prepare_multi(Operation::DepthMosaic, inputs)?;
        self.inner
            .mosaic_depth(&inputs)
            .map(OwnedImage::from_internal)
            .map_err(Error::from)
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
        validate_layout(Operation::SaveImage, input)?;
        if !file_format_supported(input.image_type, format) {
            return Err(invalid(
                Operation::SaveImage,
                InputViolation::UnsupportedImageFileFormat {
                    image_type: input.image_type.bits(),
                    file_format: format as i32,
                },
            ));
        }
        self.inner
            .save_image(input.to_internal(), format.to_internal(), file_name)
            .map_err(Error::from)
    }
}

fn prepare_multi<'a>(
    operation: Operation,
    inputs: &[ImageRef<'a>],
) -> Result<Vec<mv3d_lp_internal::ImageInput<'a>>> {
    if !(1..=MAX_MULTI_IMAGE_COUNT).contains(&inputs.len()) {
        return Err(invalid(
            operation,
            InputViolation::ImageCount {
                minimum: 1,
                maximum: MAX_MULTI_IMAGE_COUNT,
                actual: inputs.len(),
            },
        ));
    }
    let mut internal = Vec::new();
    internal
        .try_reserve_exact(inputs.len())
        .map_err(|_| Error::AllocationFailed { operation })?;
    let mut aggregate_input_bytes = 0usize;
    let mut predicted_output_bytes = 0usize;
    for input in inputs {
        require_type(operation, *input, ImageType::DEPTH)?;
        validate_layout(operation, *input)?;
        aggregate_input_bytes = aggregate_input_bytes
            .checked_add(payload_bytes(operation, *input)?)
            .ok_or_else(|| invalid_layout(operation, "aggregate input length"))?;
        let output_contribution = if operation == Operation::MapDepthToPointCloudRound {
            usize::try_from(input.width)
                .unwrap_or(usize::MAX)
                .checked_mul(usize::try_from(input.height).unwrap_or(usize::MAX))
                .and_then(|pixels| pixels.checked_mul(12))
        } else {
            input
                .data
                .len()
                .checked_add(input.intensity_data.map_or(0, <[u8]>::len))
        }
        .ok_or_else(|| invalid_layout(operation, "predicted output length"))?;
        predicted_output_bytes = predicted_output_bytes
            .checked_add(output_contribution)
            .ok_or_else(|| invalid_layout(operation, "predicted output length"))?;
        internal.push(input.to_internal());
    }
    if aggregate_input_bytes > MAX_IMAGE_BYTES {
        return Err(invalid(
            operation,
            InputViolation::TooLong {
                max: MAX_IMAGE_BYTES,
                actual: aggregate_input_bytes,
            },
        ));
    }
    if predicted_output_bytes > MAX_IMAGE_BYTES {
        return Err(invalid(
            operation,
            InputViolation::TooLong {
                max: MAX_IMAGE_BYTES,
                actual: predicted_output_bytes,
            },
        ));
    }
    Ok(internal)
}

fn require_type(operation: Operation, image: ImageRef<'_>, expected: ImageType) -> Result<()> {
    if image.image_type == expected {
        Ok(())
    } else {
        Err(invalid(
            operation,
            InputViolation::UnexpectedImageType {
                expected: expected.bits(),
                actual: image.image_type.bits(),
            },
        ))
    }
}

pub(crate) fn validate_layout(operation: Operation, image: ImageRef<'_>) -> Result<()> {
    if image.width == 0 {
        return Err(invalid(
            operation,
            InputViolation::InvalidImageLayout { field: "width" },
        ));
    }
    if image.height == 0 {
        return Err(invalid(
            operation,
            InputViolation::InvalidImageLayout { field: "height" },
        ));
    }
    if image.data.is_empty() {
        return Err(invalid(
            operation,
            InputViolation::InvalidImageLayout {
                field: "data length",
            },
        ));
    }
    let pixels = usize::try_from(image.width)
        .unwrap_or(usize::MAX)
        .checked_mul(usize::try_from(image.height).unwrap_or(usize::MAX))
        .ok_or_else(|| {
            invalid(
                operation,
                InputViolation::InvalidImageLayout {
                    field: "dimensions",
                },
            )
        })?;
    if let Some(bytes_per_pixel) = known_bytes_per_pixel(image.image_type) {
        let expected = pixels.checked_mul(bytes_per_pixel).ok_or_else(|| {
            invalid(
                operation,
                InputViolation::InvalidImageLayout {
                    field: "data length",
                },
            )
        })?;
        if image.data.len() != expected {
            return Err(invalid(
                operation,
                InputViolation::InvalidImageLayout {
                    field: "data length",
                },
            ));
        }
    }
    if image.data.len() > u32::MAX as usize {
        return Err(invalid(
            operation,
            InputViolation::TooLong {
                max: u32::MAX as usize,
                actual: image.data.len(),
            },
        ));
    }
    if let Some(intensity) = image.intensity_data {
        if intensity.len() != pixels {
            return Err(invalid(
                operation,
                InputViolation::InvalidImageLayout {
                    field: "intensity data length",
                },
            ));
        }
    }
    if let Some(timestamps) = image.exposure_timestamps {
        if timestamps.len() != usize::try_from(image.height).unwrap_or(usize::MAX) {
            return Err(invalid(
                operation,
                InputViolation::InvalidImageLayout {
                    field: "exposure timestamp count",
                },
            ));
        }
    }
    if !image.calibration.x_scale.is_finite()
        || !image.calibration.y_scale.is_finite()
        || !image.calibration.z_scale.is_finite()
    {
        return Err(invalid(
            operation,
            InputViolation::InvalidImageLayout {
                field: "calibration scale",
            },
        ));
    }
    let aggregate = payload_bytes(operation, image)?;
    if aggregate > MAX_IMAGE_BYTES {
        return Err(invalid(
            operation,
            InputViolation::TooLong {
                max: MAX_IMAGE_BYTES,
                actual: aggregate,
            },
        ));
    }
    Ok(())
}

fn payload_bytes(operation: Operation, image: ImageRef<'_>) -> Result<usize> {
    let intensity = image.intensity_data.map_or(0, <[u8]>::len);
    let exposure = image.exposure_timestamps.map_or(Ok(0), |timestamps| {
        timestamps
            .len()
            .checked_mul(std::mem::size_of::<i64>())
            .ok_or_else(|| invalid_layout(operation, "exposure timestamp bytes"))
    })?;
    image
        .data
        .len()
        .checked_add(intensity)
        .and_then(|bytes| bytes.checked_add(exposure))
        .ok_or_else(|| invalid_layout(operation, "aggregate input length"))
}

fn invalid_layout(operation: Operation, field: &'static str) -> Error {
    invalid(operation, InputViolation::InvalidImageLayout { field })
}

fn validate_expected_output(
    operation: Operation,
    width: u32,
    height: u32,
    bytes_per_pixel: usize,
) -> Result<()> {
    let expected = usize::try_from(width)
        .unwrap_or(usize::MAX)
        .checked_mul(usize::try_from(height).unwrap_or(usize::MAX))
        .and_then(|pixels| pixels.checked_mul(bytes_per_pixel))
        .ok_or_else(|| {
            invalid(
                operation,
                InputViolation::InvalidImageLayout {
                    field: "expected output length",
                },
            )
        })?;
    if expected > MAX_IMAGE_BYTES {
        return Err(invalid(
            operation,
            InputViolation::TooLong {
                max: MAX_IMAGE_BYTES,
                actual: expected,
            },
        ));
    }
    Ok(())
}

fn known_bytes_per_pixel(image_type: ImageType) -> Option<usize> {
    match image_type {
        ImageType::MONO8 => Some(1),
        ImageType::DEPTH => Some(2),
        ImageType::PROFILE => Some(6),
        ImageType::POINT_CLOUD | ImageType::PROFILE_ABC32 => Some(12),
        ImageType::RGB24_PACKED => Some(3),
        _ => None,
    }
}

fn conversion_supported(source: ImageType, target: ImageType) -> bool {
    matches!(
        (source, target),
        (ImageType::DEPTH, ImageType::MONO8)
            | (ImageType::DEPTH, ImageType::RGB24_PACKED)
            | (ImageType::PROFILE, ImageType::POINT_CLOUD)
            | (ImageType::PROFILE, ImageType::PROFILE_ABC32)
            | (ImageType::PROFILE_ABC32, ImageType::POINT_CLOUD)
            | (ImageType::POINT_CLOUD, ImageType::PROFILE_ABC32)
    )
}

fn file_format_supported(image_type: ImageType, format: ImageFileFormat) -> bool {
    match format {
        ImageFileFormat::Bmp => matches!(
            image_type,
            ImageType::MONO8 | ImageType::DEPTH | ImageType::RGB24_PACKED
        ),
        ImageFileFormat::Jpeg => matches!(
            image_type,
            ImageType::DEPTH | ImageType::JPEG | ImageType::RGB24_PACKED
        ),
        ImageFileFormat::Tiff => {
            matches!(image_type, ImageType::DEPTH | ImageType::RGB24_PACKED)
        }
        ImageFileFormat::TiffU16 | ImageFileFormat::TiffF32 | ImageFileFormat::Hibag => {
            image_type == ImageType::DEPTH
        }
        ImageFileFormat::Ply | ImageFileFormat::Csv | ImageFileFormat::Obj => matches!(
            image_type,
            ImageType::PROFILE | ImageType::PROFILE_ABC32 | ImageType::POINT_CLOUD
        ),
        ImageFileFormat::PlyBinary | ImageFileFormat::PlyTexture => {
            image_type == ImageType::POINT_CLOUD
        }
    }
}

pub(crate) fn invalid(operation: Operation, violation: InputViolation) -> Error {
    Error::InvalidInput {
        field: operation.sdk_name(),
        violation,
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageFileFormat, conversion_supported, file_format_supported, prepare_multi};
    use crate::{ImageCalibration, ImageRef, ImageType, Operation};

    #[test]
    fn conversion_matrix_matches_vendor_header() {
        let types = image_types();
        for source in types {
            for target in types {
                let expected = matches!(
                    (source, target),
                    (ImageType::DEPTH, ImageType::MONO8)
                        | (ImageType::DEPTH, ImageType::RGB24_PACKED)
                        | (ImageType::PROFILE, ImageType::POINT_CLOUD)
                        | (ImageType::PROFILE, ImageType::PROFILE_ABC32)
                        | (ImageType::PROFILE_ABC32, ImageType::POINT_CLOUD)
                        | (ImageType::POINT_CLOUD, ImageType::PROFILE_ABC32)
                );
                assert_eq!(
                    conversion_supported(source, target),
                    expected,
                    "source={source:?}, target={target:?}"
                );
            }
        }
    }

    #[test]
    fn save_matrix_rejects_invalid_cross_product_entries() {
        let formats = [
            ImageFileFormat::Ply,
            ImageFileFormat::Csv,
            ImageFileFormat::Obj,
            ImageFileFormat::Bmp,
            ImageFileFormat::Jpeg,
            ImageFileFormat::Tiff,
            ImageFileFormat::TiffU16,
            ImageFileFormat::TiffF32,
            ImageFileFormat::PlyBinary,
            ImageFileFormat::PlyTexture,
            ImageFileFormat::Hibag,
        ];
        for image_type in image_types() {
            for format in formats {
                let expected = match format {
                    ImageFileFormat::Bmp => matches!(
                        image_type,
                        ImageType::MONO8 | ImageType::DEPTH | ImageType::RGB24_PACKED
                    ),
                    ImageFileFormat::Jpeg => matches!(
                        image_type,
                        ImageType::DEPTH | ImageType::JPEG | ImageType::RGB24_PACKED
                    ),
                    ImageFileFormat::Tiff => {
                        matches!(image_type, ImageType::DEPTH | ImageType::RGB24_PACKED)
                    }
                    ImageFileFormat::TiffU16
                    | ImageFileFormat::TiffF32
                    | ImageFileFormat::Hibag => image_type == ImageType::DEPTH,
                    ImageFileFormat::Ply | ImageFileFormat::Csv | ImageFileFormat::Obj => {
                        matches!(
                            image_type,
                            ImageType::PROFILE | ImageType::PROFILE_ABC32 | ImageType::POINT_CLOUD
                        )
                    }
                    ImageFileFormat::PlyBinary | ImageFileFormat::PlyTexture => {
                        image_type == ImageType::POINT_CLOUD
                    }
                };
                assert_eq!(
                    file_format_supported(image_type, format),
                    expected,
                    "image_type={image_type:?}, format={format:?}"
                );
            }
        }
    }

    #[test]
    fn multi_image_count_accepts_one_through_eight_only() {
        let depth = ImageRef {
            image_type: ImageType::DEPTH,
            width: 1,
            height: 1,
            data: &[0, 0],
            intensity_data: None,
            exposure_timestamps: None,
            frame_number: 0,
            device_timestamp: 0,
            valid: true,
            calibration: ImageCalibration::default(),
        };
        assert!(prepare_multi(Operation::DepthMosaic, &[]).is_err());
        assert!(prepare_multi(Operation::DepthMosaic, &[depth]).is_ok());
        assert!(prepare_multi(Operation::DepthMosaic, &[depth; 8]).is_ok());
        assert!(prepare_multi(Operation::DepthMosaic, &[depth; 9]).is_err());
    }

    fn image_types() -> [ImageType; 8] {
        [
            ImageType::UNDEFINED,
            ImageType::MONO8,
            ImageType::DEPTH,
            ImageType::PROFILE,
            ImageType::POINT_CLOUD,
            ImageType::RGB24_PACKED,
            ImageType::JPEG,
            ImageType::PROFILE_ABC32,
        ]
    }
}
