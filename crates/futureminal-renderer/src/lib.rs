//! Futureminal Renderer - GPU-accelerated terminal text rendering.
//!
//! Uses wgpu 29.x for cross-platform GPU rendering with a glyph atlas approach.

use std::sync::Arc;
use tracing::{info, warn};

/// Renderer configuration.
#[derive(Debug, Clone)]
pub struct RendererConfig {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub background: [f32; 4],
    pub foreground: [f32; 4],
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            scale_factor: 1.0,
            // Distinctive Futureminal dark theme - NOT Warp's default
            background: [0.06, 0.08, 0.12, 1.0], // Deep slate blue
            foreground: [0.85, 0.87, 0.91, 1.0], // Soft white
        }
    }
}

/// A GPU-backed terminal renderer.
pub struct Renderer {
    config: RendererConfig,
    #[allow(dead_code)]
    device: Arc<wgpu::Device>,
    #[allow(dead_code)]
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
}

impl Renderer {
    pub async fn new(config: RendererConfig, target: impl Into<wgpu::SurfaceTarget<'static>>) -> anyhow::Result<Self> {
        info!("Initializing Futureminal renderer (wgpu 29.x)");

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        });

        let surface = instance.create_surface(target)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to request adapter: {}", e))?;

        let (device, queue): (wgpu::Device, wgpu::Queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("futureminal-renderer"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                    ..Default::default()
                }
            )
            .await?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("futureminal-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("futureminal-pipeline-layout"),
            bind_group_layouts: &[],
            ..Default::default()
        });

        let swapchain_format = surface.get_capabilities(&adapter).formats[0];

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("futureminal-render-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: swapchain_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: config.width,
            height: config.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        info!("Renderer initialized: {}x{} @ {:?}", config.width, config.height, swapchain_format);

        Ok(Self {
            config,
            device,
            queue,
            surface,
            surface_config,
            render_pipeline,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        self.config.width = width;
        self.config.height = height;
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        info!("Resized surface to {}x{}", width, height);
    }

    pub fn render(&mut self) -> anyhow::Result<()> {
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Timeout => {
                warn!("Surface timeout, reconfiguring");
                self.surface.configure(&self.device, &self.surface_config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                warn!("Surface occluded, reconfiguring");
                self.surface.configure(&self.device, &self.surface_config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                warn!("Surface outdated, reconfiguring");
                self.surface.configure(&self.device, &self.surface_config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                warn!("Surface lost, reconfiguring");
                self.surface.configure(&self.device, &self.surface_config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                warn!("Surface validation error, reconfiguring");
                self.surface.configure(&self.device, &self.surface_config);
                return Ok(());
            }
        };
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("futureminal-encoder"),
        });

        let bg = self.config.background;

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("futureminal-render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: bg[0] as f64,
                            g: bg[1] as f64,
                            b: bg[2] as f64,
                            a: bg[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            // TODO: render cells here
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

/// Initialize the renderer subsystem.
pub fn init() {
    info!("futureminal-renderer v{} initialized", env!("CARGO_PKG_VERSION"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_config_default() {
        let config = RendererConfig::default();
        assert_eq!(config.width, 800);
        assert_eq!(config.height, 600);
        // Verify distinct Futureminal theme (not Warp defaults)
        assert_eq!(config.background, [0.06, 0.08, 0.12, 1.0]);
        assert_eq!(config.foreground, [0.85, 0.87, 0.91, 1.0]);
    }

    #[test]
    fn test_renderer_config_custom() {
        let config = RendererConfig {
            width: 1920,
            height: 1080,
            scale_factor: 2.0,
            background: [0.0, 0.0, 0.0, 1.0],
            foreground: [1.0, 1.0, 1.0, 1.0],
        };
        assert_eq!(config.width, 1920);
        assert_eq!(config.scale_factor, 2.0);
    }

    #[test]
    fn test_init() {
        init();
    }
}
