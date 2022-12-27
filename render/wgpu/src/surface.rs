mod commands;
pub mod target;

use crate::buffer_pool::TexturePool;
use crate::mesh::Mesh;
use crate::surface::commands::CommandRenderer;
use crate::uniform_buffer::BufferStorage;
use crate::utils::remove_srgb;
use crate::{Descriptors, Pipelines, TextureTransforms, Transforms, UniformBuffer};
use ruffle_render::commands::CommandList;
use std::sync::Arc;
use target::CommandTarget;

#[derive(Debug)]
pub struct Surface {
    size: wgpu::Extent3d,
    sample_count: u32,
    pipelines: Arc<Pipelines>,
    format: wgpu::TextureFormat,
    actual_surface_format: wgpu::TextureFormat,
}

impl Surface {
    pub fn new(
        descriptors: &Descriptors,
        sample_count: u32,
        width: u32,
        height: u32,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let frame_buffer_format = remove_srgb(surface_format);

        let pipelines = descriptors.pipelines(sample_count, frame_buffer_format);
        Self {
            size,
            sample_count,
            pipelines,
            format: frame_buffer_format,
            actual_surface_format: surface_format,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_commands_to(
        &mut self,
        frame_view: &wgpu::TextureView,
        clear_color: Option<wgpu::Color>,
        descriptors: &Descriptors,
        uniform_buffers_storage: &mut BufferStorage<Transforms>,
        meshes: &Vec<Mesh>,
        commands: CommandList,
        texture_pool: &mut TexturePool,
    ) -> Vec<wgpu::CommandBuffer> {
        uniform_buffers_storage.recall();
        let uniform_encoder_label = create_debug_label!("Uniform upload command encoder");
        let mut uniform_buffer = UniformBuffer::new(uniform_buffers_storage);
        let mut uniform_encoder =
            descriptors
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: uniform_encoder_label.as_deref(),
                });
        let label = create_debug_label!("Draw encoder");
        let mut draw_encoder =
            descriptors
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: label.as_deref(),
                });

        let target = self.draw_commands(
            clear_color.unwrap_or(wgpu::Color::TRANSPARENT),
            descriptors,
            meshes,
            commands,
            &mut uniform_buffer,
            &mut uniform_encoder,
            &mut draw_encoder,
            None,
            texture_pool,
        );
        let mut buffers = vec![draw_encoder.finish()];

        let copy_bind_group = descriptors
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &descriptors.bind_layouts.bitmap,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &descriptors.quad.texture_transforms,
                            offset: 0,
                            size: wgpu::BufferSize::new(
                                std::mem::size_of::<TextureTransforms>() as u64
                            ),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&target.color_view()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(
                            &descriptors.bitmap_samplers.get_sampler(false, false),
                        ),
                    },
                ],
                label: create_debug_label!("Copy sRGB bind group").as_deref(),
            });

        let pipeline = if self.actual_surface_format == self.format {
            descriptors.copy_pipeline(self.format)
        } else {
            descriptors.copy_srgb_pipeline(self.actual_surface_format)
        };

        let mut copy_encoder =
            descriptors
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: create_debug_label!("Frame copy command encoder").as_deref(),
                });

        let load = match clear_color {
            Some(color) => wgpu::LoadOp::Clear(color),
            None => wgpu::LoadOp::Load,
        };

        let mut render_pass = copy_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: frame_view,
                ops: wgpu::Operations { load, store: true },
                resolve_target: None,
            })],
            depth_stencil_attachment: None,
            label: create_debug_label!("Copy back to render target").as_deref(),
        });

        render_pass.set_pipeline(&pipeline);
        render_pass.set_bind_group(0, target.globals().bind_group(), &[]);
        render_pass.set_bind_group(1, &target.whole_frame_bind_group(descriptors), &[0]);
        render_pass.set_bind_group(2, &copy_bind_group, &[]);

        render_pass.set_vertex_buffer(0, descriptors.quad.vertices.slice(..));
        render_pass.set_index_buffer(
            descriptors.quad.indices.slice(..),
            wgpu::IndexFormat::Uint32,
        );

        render_pass.draw_indexed(0..6, 0, 0..1);
        drop(render_pass);

        buffers.push(copy_encoder.finish());
        buffers.insert(0, uniform_encoder.finish());
        uniform_buffer.finish();

        buffers
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_commands<'frame, 'global: 'frame>(
        &mut self,
        clear_color: wgpu::Color,
        descriptors: &'global Descriptors,
        meshes: &'global Vec<Mesh>,
        commands: CommandList,
        uniform_buffers: &'frame mut UniformBuffer<'global, Transforms>,
        uniform_encoder: &'frame mut wgpu::CommandEncoder,
        draw_encoder: &'frame mut wgpu::CommandEncoder,
        nearest_layer: Option<&'frame CommandTarget>,
        texture_pool: &mut TexturePool,
    ) -> CommandTarget {
        let target = CommandTarget::new(
            &descriptors,
            texture_pool,
            self.size,
            self.format,
            self.sample_count,
        );

        CommandRenderer::execute(
            &self.pipelines,
            &target,
            &meshes,
            &descriptors,
            uniform_buffers,
            uniform_encoder,
            commands,
            nearest_layer.unwrap_or(&target),
            clear_color,
            draw_encoder,
            texture_pool,
        );

        target
    }
}
