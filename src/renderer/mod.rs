pub mod model;
pub mod texture;

use std::time::Duration;

use anyhow::{Context, anyhow};
use glam::{UVec2, uvec2};

use crate::{AResult, default};

pub struct RenderContext {
	pub instance: wgpu::Instance,
	pub adapter: wgpu::Adapter,
	pub device: wgpu::Device,
	pub queue: wgpu::Queue,
}

impl RenderContext {
	pub async fn new() -> AResult<Self> {
		let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
			backends: default(),
			flags: default(),
			memory_budget_thresholds: default(),
			backend_options: default(),
			display: None,
		});
		let adapter = instance
			.request_adapter(&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::default(),
				force_fallback_adapter: false,
				compatible_surface: None,
				apply_limit_buckets: false,
			})
			.await?;
		let (device, queue) = adapter
			.request_device(&wgpu::DeviceDescriptor {
				label: None,
				required_features: wgpu::Features::IMMEDIATES |
					wgpu::Features::INDIRECT_FIRST_INSTANCE,
				required_limits: adapter.limits(),
				experimental_features: wgpu::ExperimentalFeatures::disabled(),
				memory_hints: wgpu::MemoryHints::Performance,
				trace: wgpu::Trace::Off,
			})
			.await?;

		Ok(Self {
			instance,
			adapter,
			device,
			queue,
		})
	}

	pub fn submit(&self, encoder: wgpu::CommandEncoder) -> wgpu::SubmissionIndex {
		self.queue.submit([encoder.finish()])
	}

	pub fn poll_blocking(
		&self,
		submission: wgpu::SubmissionIndex,
		timeout: Option<Duration>,
	) -> AResult {
		match self.device.poll(wgpu::PollType::Wait {
			submission_index: Some(submission),
			timeout,
		}) {
			Ok(wgpu::PollStatus::Poll) => unreachable!(
				"Device::poll with PollType::Wait should never return PollStatus::Poll"
			),
			Ok(wgpu::PollStatus::QueueEmpty | wgpu::PollStatus::WaitSucceeded) => Ok(()),
			Err(err) => Err(anyhow!("RenderContext::poll_blocking failed: {err}")),
		}
	}

	pub fn submit_and_poll(&self, encoder: wgpu::CommandEncoder) -> AResult {
		let submission = self.submit(encoder);
		self.poll_blocking(submission, Some(Duration::from_secs_f32(5.0)))
	}
}

pub struct Framebuffer {
	size: wgpu::Extent3d,
	format: wgpu::TextureFormat,
	texture: wgpu::Texture,
	texture_multisample: wgpu::Texture,
	depth_format: wgpu::TextureFormat,
	depth_texture: wgpu::Texture,
	copy_buffer_size: ImgBufferSize,
	copy_buffer: wgpu::Buffer,
	surface_config: wgpu::SurfaceConfiguration,
}

impl Framebuffer {
	pub fn new(render: &RenderContext, img_size: UVec2) -> Self {
		let size = wgpu::Extent3d {
			width: img_size.x,
			height: img_size.y,
			depth_or_array_layers: 1,
		};
		let format = wgpu::TextureFormat::Rgba8Unorm;
		let texture = render.device.create_texture(&wgpu::TextureDescriptor {
			label: Some("frameTexture"),
			size,
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
			view_formats: &[format],
		});
		let texture_multisample = render.device.create_texture(&wgpu::TextureDescriptor {
			label: Some("frameTextureMultisample"),
			size,
			mip_level_count: 1,
			sample_count: 4,
			dimension: wgpu::TextureDimension::D2,
			format,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			view_formats: &[format],
		});
		let depth_format = wgpu::TextureFormat::Depth24Plus;
		let depth_texture = render.device.create_texture(&wgpu::TextureDescriptor {
			label: Some("frameDepthTexture"),
			size,
			mip_level_count: 1,
			sample_count: 4,
			dimension: wgpu::TextureDimension::D2,
			format: depth_format,
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			view_formats: &[depth_format],
		});
		let copy_buffer_size = ImgBufferSize::new(size);
		let copy_buffer = render.device.create_buffer(&wgpu::BufferDescriptor {
			label: None,
			mapped_at_creation: false,
			size: (copy_buffer_size.bpl_padded * copy_buffer_size.height) as wgpu::BufferAddress,
			usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
		});
		let surface_config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			format,
			color_space: wgpu::SurfaceColorSpace::Auto,
			width: size.width,
			height: size.height,
			present_mode: wgpu::PresentMode::Immediate,
			desired_maximum_frame_latency: 3,
			alpha_mode: default(),
			view_formats: vec![],
		};
		Self {
			size,
			format,
			texture,
			texture_multisample,
			depth_format,
			depth_texture,
			copy_buffer_size,
			copy_buffer,
			surface_config,
		}
	}

	pub fn size(&self) -> UVec2 {
		uvec2(self.size.width, self.size.height)
	}

	pub fn format(&self) -> wgpu::TextureFormat {
		self.format
	}

	pub fn views(&self) -> TextureViews {
		let color = self.texture.create_view(&Default::default());
		let multisample = self.texture_multisample.create_view(&Default::default());
		let depth = self
			.depth_texture
			.create_view(&wgpu::TextureViewDescriptor {
				aspect: wgpu::TextureAspect::DepthOnly,
				..Default::default()
			});
		TextureViews {
			color,
			multisample,
			depth,
		}
	}

	pub fn clear(&self, render: &RenderContext, color: wgpu::Color) -> AResult {
		let mut encoder = render.device.create_command_encoder(&Default::default());
		let views = self.views();
		encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("clear framebuffer"),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: &views.multisample,
				depth_slice: None,
				resolve_target: Some(&views.color),
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Clear(color),
					store: wgpu::StoreOp::Store,
				},
			})],
			depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
				view: &views.depth,
				depth_ops: Some(wgpu::Operations {
					load: wgpu::LoadOp::Clear(1.0),
					store: wgpu::StoreOp::Store,
				}),
				stencil_ops: None,
			}),
			timestamp_writes: None,
			occlusion_query_set: None,
			multiview_mask: None,
		});
		render.submit_and_poll(encoder)
	}

	pub fn read(&self, render: &RenderContext) -> AResult<Vec<u8>> {
		let mut encoder = render.device.create_command_encoder(&Default::default());
		{
			encoder.copy_texture_to_buffer(
				self.texture.as_image_copy(),
				wgpu::TexelCopyBufferInfo {
					buffer: &self.copy_buffer,
					layout: wgpu::TexelCopyBufferLayout {
						offset: 0,
						bytes_per_row: Some(self.copy_buffer_size.bpl_padded as u32),
						rows_per_image: None,
					},
				},
				self.size,
			)
		}
		let submission = render.submit(encoder);
		let slice = self.copy_buffer.slice(..);
		slice.map_async(wgpu::MapMode::Read, |_| {});
		render.poll_blocking(submission, Some(Duration::from_secs_f32(5.0)))?;

		let padded = slice
			.get_mapped_range()
			.context("couldn't map frame copy buffer")?;
		let mut pixels =
			vec![0u8; self.copy_buffer_size.bpl_unpadded * self.copy_buffer_size.height];
		let mut pixslice = &mut pixels[..];
		for chunk in padded.chunks(self.copy_buffer_size.bpl_padded) {
			let len = self.copy_buffer_size.bpl_unpadded;
			pixslice[0 .. len].copy_from_slice(&chunk[0 .. len]);
			pixslice = &mut pixslice[len ..];
		}
		drop(padded);
		self.copy_buffer.unmap();
		Ok(pixels)
	}
}

pub struct TextureViews {
	pub color: wgpu::TextureView,
	pub multisample: wgpu::TextureView,
	pub depth: wgpu::TextureView,
}

#[derive(Clone, Copy, Debug)]
struct ImgBufferSize {
	width: usize,
	height: usize,
	bpl_unpadded: usize,
	bpl_padded: usize,
}

impl ImgBufferSize {
	fn new(extent: wgpu::Extent3d) -> Self {
		let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
		let bpl = extent.width * std::mem::size_of::<u32>() as u32;
		let padding = (align - bpl % align) % align;
		Self {
			width: extent.width as usize,
			height: extent.height as usize,
			bpl_unpadded: bpl as usize,
			bpl_padded: (bpl + padding) as usize,
		}
	}
}
