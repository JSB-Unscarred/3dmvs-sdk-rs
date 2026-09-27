//! 图像处理接口：深度图转点云、格式转换、拼接、保存与显示。
//!
//! SDK 的处理输出只在下一次处理调用前有效，因此调用与复制都在会话的处理锁内完成。

use std::ffi::{CStr, c_void};

use crate::error::sdk_call;
use crate::{Error, Image, ImageFileFormat, ImageType, Result, Sdk, sys};

/// 头文件规定多图接口最多接受 8 张图像。
const MAX_IMAGES: usize = 8;

impl Sdk {
    /// 把深度图转换为点云。
    pub fn depth_to_point_cloud(&self, depth: &Image) -> Result<Image> {
        let mut input = depth.to_raw()?;
        self.process(|output| {
            // SAFETY: input 借用已校验的buffer，output 是可写输出。
            unsafe { sdk_call!(MV3D_LP_MapDepthToPointCloud(&raw mut input, output)) }
        })
    }

    /// 把多张深度图合成为一个环形点云。
    pub fn depth_to_round_point_cloud(&self, depths: &[Image]) -> Result<Image> {
        let (mut inputs, count) = raw_images(depths)?;
        self.process(|output| {
            // SAFETY: inputs 含 count 个借用已校验 buffer 的输入，output 是可写输出。
            unsafe {
                sdk_call!(MV3D_LP_MapDepthToPointCloudRound(
                    inputs.as_mut_ptr(),
                    count,
                    output
                ))
            }
        })
    }

    /// 转换为 SDK 支持的目标格式。
    pub fn convert_image(&self, image: &Image, target: ImageType) -> Result<Image> {
        let mut input = image.to_raw()?;
        self.process(|output| {
            output.enImageType = target.raw().cast_signed();
            // SAFETY: input 借用已校验的buffer，output 只预置了目标格式。
            unsafe { sdk_call!(MV3D_LP_ImageConvert(&raw mut input, output)) }
        })
    }

    /// 拼接多张深度图。
    pub fn mosaic_depth(&self, depths: &[Image]) -> Result<Image> {
        let (mut inputs, count) = raw_images(depths)?;
        self.process(|output| {
            // SAFETY: inputs 含 count 个借用已校验 buffer 的输入，output 是可写输出。
            unsafe { sdk_call!(MV3D_LP_DepthMosaic(inputs.as_mut_ptr(), count, output)) }
        })
    }

    /// 用厂商编码器保存图像；文件名按 SDK 的本地编码解释。
    pub fn save_image(
        &self,
        image: &Image,
        format: ImageFileFormat,
        file_name: &CStr,
    ) -> Result<()> {
        let mut input = image.to_raw()?;
        let _processing = self.lock_processing();
        // SAFETY: input 借用已校验的buffer，file_name 以 NUL 结尾。
        unsafe {
            sdk_call!(MV3D_LP_SaveImage(
                &raw mut input,
                format as i32,
                file_name.as_ptr()
            ))
        }
    }

    /// 把图像绘制到 Win32 窗口。
    pub fn display_image<W>(&self, image: &Image, window: &W, range: DisplayRange) -> Result<()>
    where
        W: raw_window_handle::HasWindowHandle + ?Sized,
    {
        use raw_window_handle::RawWindowHandle;

        let hwnd = match window.window_handle().map(|handle| handle.as_raw()) {
            Ok(RawWindowHandle::Win32(handle)) => handle.hwnd,
            _ => return Err(Error::InvalidInput("window has no Win32 HWND")),
        };
        let (display_type, min, max) = match range {
            DisplayRange::Auto => (sys::DisplayType_Auto, 0, 0),
            DisplayRange::Manual { min, max } => (sys::DisplayType_Manual, min, max),
        };
        let mut input = image.to_raw()?;
        let _processing = self.lock_processing();
        // SAFETY: input 借用已校验的buffer；hwnd 来自调用期间借用的窗口。
        unsafe {
            sdk_call!(MV3D_LP_DisplayImage(
                &raw mut input,
                hwnd.get() as *mut c_void,
                display_type,
                min,
                max
            ))
        }
    }

    /// 在处理锁内调用 SDK，并在释放锁前复制输出。
    fn process(
        &self,
        call: impl FnOnce(&mut sys::MV3D_LP_IMAGE_DATA) -> Result<()>,
    ) -> Result<Image> {
        let _processing = self.lock_processing();
        let mut output = sys::MV3D_LP_IMAGE_DATA::default();
        call(&mut output)?;
        // SAFETY: SDK 调用成功，输出buffer在下一次处理调用（即锁释放）之前有效。
        Ok(unsafe { Image::from_raw(&output) })
    }
}

/// [`Sdk::display_image`] 使用的深度显示范围。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DisplayRange {
    /// 由 SDK 自动确定。
    Auto,
    /// 指定范围。
    Manual {
        /// 下界。
        min: i32,
        /// 上界。
        max: i32,
    },
}

/// 校验多图接口的输入并构造借用它们的 SDK 输入数组；头文件规定最多 8 张。
fn raw_images(images: &[Image]) -> Result<(Vec<sys::MV3D_LP_IMAGE_DATA>, u32)> {
    let count = match u32::try_from(images.len()) {
        Ok(count) if images.len() <= MAX_IMAGES => count,
        _ => return Err(Error::InvalidInput("at most 8 images are accepted")),
    };
    Ok((
        images.iter().map(Image::to_raw).collect::<Result<_>>()?,
        count,
    ))
}
