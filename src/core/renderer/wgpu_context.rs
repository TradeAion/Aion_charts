//! GpuContext — owns the wgpu Device, Queue, Surface and configuration.
//!
//! This struct is created once during init and shared (by reference) with
//! all renderers. It is NOT Send on wasm32, which is fine because the
//! browser main thread is single-threaded anyway.

/// Holds all GPU resources needed to render.
pub struct GpuContext {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub format: wgpu::TextureFormat,
}

impl GpuContext {
    /// Create a GpuContext from a raw window handle (native) or canvas (wasm).
    /// Returns `Err` if WebGPU is unavailable so the caller can fall back.
    pub async fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(target)
            .map_err(|e| format!("Failed to create surface: {:?}", e))?;

        let adapter: wgpu::Adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("No suitable GPU adapter found: {:?}", e))?;

        log::info!("Adapter: {:?}", adapter.get_info());

        let device_result = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("axiuscharts-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::default(),
            })
            .await
            .map_err(|e| format!("Failed to create device: {:?}", e))?;
        let (device, queue): (wgpu::Device, wgpu::Queue) = device_result;

        let caps = surface.get_capabilities(&adapter);
        // Prefer non-sRGB format so shader output matches Canvas2D colors exactly.
        // Our style colors (e.g. [0.102, 0.737, 0.612]) are sRGB values meant to
        // be used directly. An sRGB format would gamma-encode them a second time.
        let format = caps
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        log::info!(
            "Surface format: {:?}, available: {:?}",
            format,
            caps.formats
        );

        // Prefer PreMultiplied alpha so the canvas is transparent and the
        // grid canvas behind it shows through.
        let alpha_mode = if caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else {
            caps.alpha_modes[0]
        };

        log::info!(
            "Surface alpha_mode: {:?}, available: {:?}",
            alpha_mode,
            caps.alpha_modes
        );

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            format,
        })
    }

    /// Reconfigure surface on resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.config.width == width && self.config.height == height {
            return;
        }

        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }
}
