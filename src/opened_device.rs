use std::sync::Arc;

use crate::{DeviceException, FileProgress, Image, Parameter, ParameterValue, Result};

/// An opened laser-profiler device with independent session ownership.
///
/// `Device` does not borrow [`crate::Sdk`] and remains usable after that token is dropped. A live
/// device must be closed or dropped before [`crate::Sdk::shutdown`]. `Device` is `Send` but not
/// `Sync`. Its local acquisition state prevents pull and callback modes from sharing one handle.
pub struct Device {
    inner: mv3d_lp_internal::Device,
}

impl Device {
    pub(crate) fn from_internal(inner: mv3d_lp_internal::Device) -> Self {
        Self { inner }
    }

    /// Registers an exception callback. A later call replaces the previous one after native success.
    pub fn register_exception_callback<F>(&mut self, callback: F) -> Result<()>
    where
        F: Fn(DeviceException) + Send + Sync + 'static,
    {
        self.inner.register_exception_callback(Arc::new(callback))
    }

    /// Stops future Rust delivery of exception callbacks.
    pub fn disable_exception_delivery(&mut self) {
        self.inner.disable_exception_delivery();
    }

    /// Starts pull acquisition from idle, or callback acquisition after image registration.
    pub fn start(&mut self) -> Result<()> {
        self.inner.start()
    }

    /// Stops the active pull or callback acquisition.
    pub fn stop(&mut self) -> Result<()> {
        self.inner.stop()
    }

    /// Forwards one software trigger; the SDK validates its trigger mode and call order.
    pub fn soft_trigger(&mut self) -> Result<()> {
        self.inner.soft_trigger()
    }

    /// Waits up to `timeout_ms` milliseconds for one pull frame and copies every returned payload.
    ///
    /// `0` polls without blocking. `u32::MAX` is the SDK's infinite-wait sentinel and is therefore
    /// equivalent to [`Self::get_image_blocking`].
    pub fn get_image(&mut self, timeout_ms: u32) -> Result<Image> {
        self.inner.get_image(timeout_ms)
    }

    /// Waits indefinitely for one pull frame using the SDK's infinite-wait sentinel.
    pub fn get_image_blocking(&mut self) -> Result<Image> {
        self.inner.get_image(u32::MAX)
    }

    /// Registers native image delivery. The first success binds this handle to callback until Close.
    ///
    /// Call [`Self::start`] afterwards to begin measurement. A later call replaces the cookie after
    /// native success. `Image` is copied before the callback returns. A panic in `callback` retires
    /// only the current cookie; registering again restores delivery under a new cookie.
    pub fn register_image_callback<F>(&mut self, callback: F) -> Result<()>
    where
        F: Fn(Image) + Send + Sync + 'static,
    {
        self.inner.register_image_callback(Arc::new(callback))
    }

    /// Stops future Rust delivery of image callbacks. Native registration remains until Close.
    pub fn disable_image_delivery(&mut self) {
        self.inner.disable_image_delivery();
    }

    /// Discards the frames already buffered for this device.
    ///
    /// 直接转发；允许调用的状态待厂商确认。
    pub fn clear_buffer(&mut self) -> Result<()> {
        self.inner.clear_buffer()
    }

    /// Reads one parameter by its SDK Node Name, accepting `&str` or raw bytes.
    pub fn get_parameter(&mut self, key: impl AsRef<[u8]>) -> Result<Parameter> {
        self.inner.get_parameter(key.as_ref())
    }

    /// Writes one parameter by its SDK Node Name, accepting `&str` or raw bytes.
    pub fn set_parameter(&mut self, key: impl AsRef<[u8]>, value: ParameterValue) -> Result<()> {
        self.inner.set_parameter(key.as_ref(), &value)
    }

    /// Executes one command by its SDK Command Node Name, accepting `&str` or raw bytes.
    pub fn execute(&mut self, key: impl AsRef<[u8]>) -> Result<()> {
        self.inner.execute(key.as_ref())
    }

    /// Starts copying a file from the device to the host. Names accept `&str` or raw bytes.
    pub fn download_file(
        &mut self,
        device_file_name: impl AsRef<[u8]>,
        local_file_name: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.inner
            .download_file(device_file_name.as_ref(), local_file_name.as_ref())
    }

    /// Starts copying a host file into the device. Names accept `&str` or raw bytes.
    pub fn upload_file(
        &mut self,
        local_file_name: impl AsRef<[u8]>,
        device_file_name: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.inner
            .upload_file(local_file_name.as_ref(), device_file_name.as_ref())
    }

    /// Returns one progress snapshot for the active transfer.
    pub fn file_transfer_progress(&mut self) -> Result<FileProgress> {
        self.inner.file_transfer_progress()
    }

    /// Stops acquisition when needed and closes the owned handle.
    pub fn close(self) -> Result<()> {
        self.inner.close()
    }
}
