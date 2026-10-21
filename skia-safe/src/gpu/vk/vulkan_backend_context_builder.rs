use super::{
    Device, GetProc, Instance, PhysicalDevice, PhysicalDeviceFeatures2, Queue, Version,
    vulkan_backend_context::BackendContext,
};
use crate::gpu;
use skia_bindings as sb;

/// Builds a Vulkan [`BackendContext`] with optional extensions and protected mode.
pub struct BackendContextBuilder<'a> {
    instance: Instance,
    physical_device: PhysicalDevice,
    device: Device,
    queue: Queue,
    queue_index: usize,
    get_proc: &'a dyn GetProc,
    max_api_version: Option<Version>,
    instance_extensions: Vec<String>,
    device_extensions: Vec<String>,
    protected_context: Option<gpu::Protected>,
    device_features: Option<DeviceFeatures<'a>>,
}

#[derive(Clone, Copy)]
enum DeviceFeatures<'a> {
    Borrowed(&'a PhysicalDeviceFeatures2),
    SamplerYcbcrConversion(bool),
}

impl std::fmt::Debug for BackendContextBuilder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackendContextBuilder")
            .field("instance", &self.instance)
            .field("physical_device", &self.physical_device)
            .field("device", &self.device)
            .field("queue", &self.queue)
            .field("queue_index", &self.queue_index)
            .field("get_proc", &"<dyn GetProc>")
            .field("max_api_version", &self.max_api_version)
            .field("instance_extensions", &self.instance_extensions)
            .field("device_extensions", &self.device_extensions)
            .field("protected_context", &self.protected_context)
            .field("device_features", &self.device_features.is_some())
            .finish()
    }
}

impl<'a> BackendContextBuilder<'a> {
    /// Creates a new builder for a Vulkan backend context.
    ///
    /// The `max_api_version` is applied during native context creation, before
    /// the Vulkan memory allocator is initialized.
    ///
    /// If a version is provided, it should match the value provided in
    /// `VkApplicationInfo::apiVersion` when creating the `VkInstance`. Pass `None` to leave Skia's
    /// `VulkanBackendContext::fMaxAPIVersion` at its default `0`; Skia then queries
    /// `vkEnumerateInstanceVersion()` and uses that loader-reported version as the upper limit when
    /// validating Vulkan entry points and creating the memory allocator.
    ///
    /// Skia requires Vulkan 1.1 as the minimum supported API version.
    pub fn new(
        instance: Instance,
        physical_device: PhysicalDevice,
        device: Device,
        (queue, queue_index): (Queue, usize),
        get_proc: &'a impl GetProc,
        max_api_version: impl Into<Option<Version>>,
    ) -> Self {
        Self {
            instance,
            physical_device,
            device,
            queue,
            queue_index,
            get_proc,
            max_api_version: max_api_version.into(),
            instance_extensions: Vec::new(),
            device_extensions: Vec::new(),
            protected_context: None,
            device_features: None,
        }
    }

    /// Sets the Vulkan instance and device extension names expected by Skia.
    ///
    /// The provided names are copied into the builder.
    pub fn with_extensions(
        mut self,
        instance_extensions: &[&str],
        device_extensions: &[&str],
    ) -> Self {
        self.instance_extensions = instance_extensions
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        self.device_extensions = device_extensions.iter().map(|s| (*s).to_owned()).collect();
        self
    }

    /// Configures whether the context should operate in protected mode.
    pub fn with_protected_context(mut self, protected_context: gpu::Protected) -> Self {
        self.protected_context = Some(protected_context);
        self
    }

    /// Sets the features enabled on `device`, such as `samplerYcbcrConversion` in the `pNext`
    /// chain.
    ///
    /// Skia enables the optional paths that depend on a device feature only for the features
    /// passed here, for example sampling textures through a YCbCr conversion.
    pub fn with_device_features(mut self, device_features: &'a PhysicalDeviceFeatures2) -> Self {
        self.device_features = Some(DeviceFeatures::Borrowed(device_features));
        self
    }

    /// Tells Skia whether `samplerYcbcrConversion` is enabled on `device`.
    ///
    /// Skia samples textures through a YCbCr conversion, such as hardware buffers in YUV formats
    /// or with an external format, only if the feature is enabled. Skia treats every other device
    /// feature as not enabled. Unlike [`Self::with_device_features()`], this borrows nothing.
    ///
    /// This replaces the features set before by [`Self::with_device_features()`] and is replaced
    /// by it in turn.
    pub fn with_sampler_ycbcr_conversion(mut self, enabled: bool) -> Self {
        self.device_features = Some(DeviceFeatures::SamplerYcbcrConversion(enabled));
        self
    }

    /// Creates the Vulkan [`BackendContext`] from the configured builder state.
    ///
    /// If [`Self::with_protected_context()`] is not called, protected mode defaults to
    /// [`gpu::Protected::No`].
    /// # Safety
    /// `instance`, `physical_device`, `device`, and `queue` must outlive the `BackendContext`
    /// returned. Every structure in the `pNext` chain of the device features must outlive it as
    /// well.
    pub unsafe fn build(self) -> BackendContext<'a> {
        let instance_extensions: Vec<&str> = self
            .instance_extensions
            .iter()
            .map(String::as_str)
            .collect();
        let device_extensions: Vec<&str> =
            self.device_extensions.iter().map(String::as_str).collect();

        let context = unsafe {
            BackendContext::new_internal(
                self.instance,
                self.physical_device,
                self.device,
                (self.queue, self.queue_index),
                self.get_proc,
                self.max_api_version,
                self.protected_context.unwrap_or(gpu::Protected::No),
                &instance_extensions,
                &device_extensions,
            )
        };
        match self.device_features {
            Some(DeviceFeatures::Borrowed(device_features)) => unsafe {
                sb::C_VulkanBackendContext_setDeviceFeatures2(
                    context.native.as_ptr() as _,
                    device_features,
                )
            },
            Some(DeviceFeatures::SamplerYcbcrConversion(enabled)) => unsafe {
                sb::C_VulkanBackendContext_setSamplerYcbcrConversion(
                    context.native.as_ptr() as _,
                    enabled,
                )
            },
            None => {}
        }
        context
    }
}
