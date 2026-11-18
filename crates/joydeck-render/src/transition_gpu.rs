//! Device-backed transition compositor for the native slideshow. The immutable sources,
//! render target and readback buffer live for one slide pair at one device scale.
mod triangle;

use crate::{Image, SlideParts};
use joydeck_anim::{Layer, Source, layer_mesh};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
use triangle::Triangle;
use wgpu::util::DeviceExt;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const MAX_IMAGE_BYTES: u64 = 192 * 1024 * 1024;
const MAX_TRIANGLES: usize = 128_000;

pub struct Compositor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    healthy: Arc<AtomicBool>,
    pub adapter: wgpu::AdapterInfo,
}

pub struct SlidePair {
    owner: Arc<AtomicBool>,
    images: wgpu::BindGroup,
    target: wgpu::Texture,
    view: wgpu::TextureView,
    readback: wgpu::Buffer,
    stride: u32,
    has_old: bool,
    has_parts: bool,
    has_old_back: bool,
    has_old_content: bool,
}

impl Compositor {
    /// Software adapters are usable by the pixel oracle; the audience uses its CPU
    /// renderer when the OS only offers software rasterization.
    pub fn open(allow_software: bool) -> Result<Self, String> {
        pollster::block_on(async {
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .map_err(|e| e.to_string())?;
            let info = adapter.get_info();
            if !allow_software && info.device_type == wgpu::DeviceType::Cpu {
                return Err(format!("software adapter {}", info.name));
            }
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("JoyDeck transitions"),
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits()),
                    ..Default::default()
                })
                .await
                .map_err(|e| e.to_string())?;
            let healthy = Arc::new(AtomicBool::new(true));
            let fault = healthy.clone();
            device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
                fault.store(false, Ordering::Release);
                log::warn!("JoyDeck transition device: {e}");
            }));
            let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let shader =
                device.create_shader_module(wgpu::include_wgsl!("transition_gpu/slide.wgsl"));
            let layout = wgpu::VertexBufferLayout {
                array_stride: size_of::<Triangle>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4,
                    3 => Float32x4, 4 => Float32x3, 5 => Float32x3, 6 => Uint32, 7 => Float32x3, 8 => Float32x3, 9 => Float32x3],
            };
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("JoyDeck premultiplied transition layers"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vertex"),
                    compilation_options: Default::default(),
                    buffers: &[Some(layout)],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fragment"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: FORMAT,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
            if let Some(e) = scope.pop().await {
                return Err(e.to_string());
            }
            Ok(Self {
                device,
                queue,
                pipeline,
                healthy,
                adapter: info,
            })
        })
    }

    /// Check the complete image allocation before rendering or uploading source slides.
    pub fn accepts(&self, width: u32, height: u32, parts: bool) -> bool {
        let limit = self.device.limits().max_texture_dimension_2d.min(8192);
        (1..=limit).contains(&width)
            && (1..=limit).contains(&height)
            && u64::from(width) * u64::from(height) * 4 * if parts { 8 } else { 4 }
                + 255 * u64::from(height)
                <= MAX_IMAGE_BYTES
            && self.healthy.load(Ordering::Acquire)
    }

    pub fn upload(
        &self,
        old: Option<&Image>,
        new: &Image,
        parts: Option<&SlideParts>,
    ) -> Result<SlidePair, String> {
        let (width, height) = (new.width, new.height);
        if !self.accepts(width, height, parts.is_some()) {
            return Err("transition exceeds device frame budget or device unavailable".into());
        }
        let depth = if parts.is_some() { 6 } else { 2 };
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = |label, layers, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    depth_or_array_layers: layers,
                    ..extent
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage,
                view_formats: &[],
            })
        };
        let sources = texture(
            "slide sources",
            depth,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        for (i, image) in [
            old,
            Some(new),
            parts.and_then(|p| p.old_back.as_ref()),
            parts.map(|p| &p.new_back),
            parts.and_then(|p| p.old_content.as_ref()),
            parts.map(|p| &p.new_content),
        ]
        .into_iter()
        .enumerate()
        {
            let Some(image) = image else {
                continue;
            };
            if image.width != width
                || image.height != height
                || image.pixels.len() != (width * height * 4) as usize
            {
                return Err("transition source has inconsistent dimensions".into());
            }
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &sources,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: i as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &image.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                extent,
            );
        }
        let images = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("slide pair"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &sources.create_view(&Default::default()),
                ),
            }],
        });
        let target = texture(
            "transition output",
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let view = target.create_view(&Default::default());
        let stride = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("transition readback"),
            size: u64::from(stride) * u64::from(height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(SlidePair {
            owner: self.healthy.clone(),
            images,
            target,
            view,
            readback,
            stride,
            has_old: old.is_some(),
            has_parts: parts.is_some(),
            has_old_back: parts.is_some_and(|p| p.old_back.is_some()),
            has_old_content: parts.is_some_and(|p| p.old_content.is_some()),
        })
    }

    pub fn render(&self, pair: &mut SlidePair, layers: &[Layer]) -> Result<Image, String> {
        if !Arc::ptr_eq(&pair.owner, &self.healthy) {
            return Err("transition textures belong to another device".into());
        }
        if !self.healthy.load(Ordering::Acquire) {
            return Err("transition device unavailable".into());
        }
        let dimensions = [
            f64::from(pair.target.width()),
            f64::from(pair.target.height()),
        ];
        if layers.len() > 32_768 {
            return Err("transition exceeds layer budget".into());
        }
        let mut work = 0u64;
        let max_work = u64::from(pair.target.width()) * u64::from(pair.target.height()) * 96;
        let mut instances = Vec::new();
        for layer in layers {
            let Some(source) = pair.source(layer.source) else {
                continue;
            };
            let (vertices, triangles) = layer_mesh(layer);
            if instances.len() + triangles.len() > MAX_TRIANGLES {
                return Err("transition exceeds mesh budget".into());
            }
            for indices in triangles {
                if let Some((triangle, pixels)) =
                    Triangle::new(indices.map(|i| vertices[i as usize]), dimensions, source)
                {
                    work += pixels;
                    if work > max_work {
                        return Err("transition exceeds output coverage budget".into());
                    }
                    instances.push(triangle);
                }
            }
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("compose projected slides"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &pair.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            if !instances.is_empty() {
                let geometry = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("transition triangles"),
                        usage: wgpu::BufferUsages::VERTEX,
                        contents: bytemuck::cast_slice(&instances),
                    });
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &pair.images, &[]);
                pass.set_vertex_buffer(0, geometry.slice(..));
                pass.draw(0..6, 0..instances.len() as u32);
            }
        }
        let size = pair.target.size();
        encoder.copy_texture_to_buffer(
            pair.target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &pair.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(pair.stride),
                    rows_per_image: Some(size.height),
                },
            },
            size,
        );
        let submission = self.queue.submit([encoder.finish()]);
        let (sender, receiver) = mpsc::channel();
        pair.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        let ready = self
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(5)),
            })
            .map_err(|e| e.to_string())
            .and_then(|_| {
                receiver
                    .try_recv()
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())
            });
        if let Err(error) = ready {
            pair.readback.unmap();
            return Err(error);
        }
        let data = match pair.readback.slice(..).get_mapped_range() {
            Ok(data) => data,
            Err(error) => {
                pair.readback.unmap();
                return Err(error.to_string());
            }
        };
        let mut pixels = Vec::with_capacity((size.width * size.height * 4) as usize);
        for row in data.chunks_exact(pair.stride as usize) {
            pixels.extend_from_slice(&row[..(size.width * 4) as usize]);
        }
        drop(data);
        pair.readback.unmap();
        if !self.healthy.load(Ordering::Acquire) {
            return Err("transition rendering failed".into());
        }
        Ok(Image {
            width: size.width,
            height: size.height,
            pixels,
        })
    }
}

impl SlidePair {
    fn source(&self, source: Source) -> Option<u32> {
        let source = if self.has_parts {
            source
        } else {
            source.whole()?
        };
        Some(match source {
            Source::Old if self.has_old => 0,
            Source::New => 1,
            Source::OldBack if self.has_old_back => 2,
            Source::NewBack => 3,
            Source::OldContent if self.has_old_content => 4,
            Source::NewContent => 5,
            Source::OldContent => return None,
            Source::White => 7,
            Source::Old | Source::OldBack | Source::Black => 6,
        })
    }
}
