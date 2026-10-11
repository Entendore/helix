//! wgpu renderer (REN-1..3, REN-13/14/16, PBR-3/6 mini, RES-1): sRGB-
//! preferred surface; MainPass -> Rgba16Float HDR; PostPass (ACES +
//! exposure) -> target with a single hardware sRGB encode; render_scale
//! sizes the HDR target. Camera position travels in uniforms (REN-16).
use std::borrow::Cow;
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;
use winit::window::Window;

use helix_assets::{DrawCall, MeshId, Vertex};

const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute {
        offset: 0,
        shader_location: 0,
        format: wgpu::VertexFormat::Float32x3,
    },
    wgpu::VertexAttribute {
        offset: 12,
        shader_location: 1,
        format: wgpu::VertexFormat::Float32x3,
    },
];
const VERTEX_LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &VERTEX_ATTRIBUTES,
};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ObjectUniforms {
    view_proj: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    color: [f32; 4],
    view_pos: [f32; 4],
}

const SHADER: &str = r#"
struct Uniforms {
    view_proj : mat4x4<f32>,
    model     : mat4x4<f32>,
    color     : vec4<f32>,
    view_pos  : vec4<f32>,
}
@group(0) @binding(0) var<uniform> u : Uniforms;

struct VSOut {
    @builtin(position) clip_pos : vec4<f32>,
    @location(0) normal : vec3<f32>,
    @location(1) world_pos : vec3<f32>,
}

@vertex
fn vs_main(@location(0) position : vec3<f32>, @location(1) normal : vec3<f32>) -> VSOut {
    var out : VSOut;
    let world = u.model * vec4<f32>(position, 1.0);
    out.clip_pos = u.view_proj * world;
    out.normal = (u.model * vec4<f32>(normal, 0.0)).xyz;
    out.world_pos = world.xyz;
    return out;
}

@fragment
fn fs_main(in : VSOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let light_dir = normalize(vec3<f32>(0.45, 1.0, 0.3));
    let diffuse = max(dot(n, light_dir), 0.0);
    let view_dir = normalize(u.view_pos.xyz - in.world_pos);
    let half_dir = normalize(light_dir + view_dir);
    let specular = pow(max(dot(n, half_dir), 0.0), 48.0) * 0.35;
    let ambient = 0.28;
    let lit = u.color.rgb * (ambient + diffuse * 0.85) + vec3<f32>(specular);
    return vec4<f32>(lit, u.color.a);
}
"#;

const POST_SHADER: &str = r#"
struct PostUniforms { exposure : vec4<f32> }
@group(0) @binding(0) var t_hdr : texture_2d<f32>;
@group(0) @binding(1) var s_lin : sampler;
@group(0) @binding(2) var<uniform> pu : PostUniforms;

struct VSOut { @builtin(position) pos : vec4<f32>, @location(0) uv : vec2<f32> }

@vertex
fn vs_main(@builtin(vertex_index) i : u32) -> VSOut {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var out : VSOut;
    out.pos = vec4(p[i], 0.0, 1.0);
    out.uv = p[i] * vec2(0.5, -0.5) + vec2(0.5, 0.5);
    return out;
}

fn aces(x : vec3<f32>) -> vec3<f32> {
    let a = 2.51; let b = 0.03; let c = 2.43; let d = 0.59; let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3(0.0), vec3(1.0));
}

@fragment
fn fs_main(in : VSOut) -> @location(0) vec4<f32> {
    let hdr = textureSample(t_hdr, s_lin, in.uv).rgb;
    let ldr = aces(hdr * pu.exposure.x);
    return vec4(ldr, 1.0); // target is sRGB => hardware encodes exactly once
}
"#;

/// REN-2: prefer an sRGB target; fallback to the first advertised format.
pub fn pick_format(formats: &[wgpu::TextureFormat]) -> wgpu::TextureFormat {
    formats
        .iter()
        .copied()
        .find(|f| f.describe().srgb)
        .unwrap_or(formats[0])
}

/// REN-13: Gribb-Hartmann planes for column-major matrices, 0..1 depth,
/// depth-direction aware (ZB-3).
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    pub fn from_view_proj(m: &Mat4, depth_reversed: bool) -> Frustum {
        let row = |i: usize| Vec4::new(m.col(0)[i], m.col(1)[i], m.col(2)[i], m.col(3)[i]);
        let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
        let (near, far) = if depth_reversed {
            (r3 - r2, r2)
        } else {
            (r2, r3 - r2)
        };
        Frustum { planes: [r3 + r0, r3 - r0, r3 + r1, r3 - r1, near, far] }
    }

    pub fn sphere_visible(&self, center: Vec3, radius: f32) -> bool {
        self.planes.iter().all(|p| p.truncate().dot(center) + p.w > -radius)
    }
}

struct GpuMesh {
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    count: u32,
}

struct GpuObject {
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

pub struct CoreRenderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    meshes: HashMap<u32, GpuMesh>,
    next_mesh: u32,
    objects: Vec<GpuObject>,
    depth: Option<(wgpu::Texture, wgpu::TextureView)>,
    depth_size: (u32, u32),
    post_pipeline: wgpu::RenderPipeline,
    post_bgl: wgpu::BindGroupLayout,
    post_uniform: wgpu::Buffer,
    post_bg: Option<wgpu::BindGroup>,
    hdr: Option<(wgpu::Texture, wgpu::TextureView)>,
    hdr_size: (u32, u32),
    linear_sampler: wgpu::Sampler,
}

impl CoreRenderer {
    pub async fn new(adapter: &wgpu::Adapter, color_format: wgpu::TextureFormat) -> Self {
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("helix.device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .expect("device");

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("helix.shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
        });
        let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("helix.post-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(POST_SHADER)),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("helix.object-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(std::mem::size_of::<ObjectUniforms>() as u64),
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("helix.layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        // MainPass renders into the HDR target (constant Rgba16Float, PBR-3).
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("helix.pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[VERTEX_LAYOUT],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: crate::DEPTH_COMPARE,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let post_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("helix.post-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(16),
                    },
                    count: None,
                },
            ],
        });
        let post_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("helix.post-layout"),
            bind_group_layouts: &[&post_bgl],
            push_constant_ranges: &[],
        });
        let post_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("helix.post-pipeline"),
            layout: Some(&post_layout),
            vertex: wgpu::VertexState {
                module: &post_shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &post_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sampler.linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let post_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("helix.post-uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
            meshes: HashMap::new(),
            next_mesh: 0,
            objects: Vec::new(),
            depth: None,
            depth_size: (0, 0),
            post_pipeline,
            post_bgl,
            post_uniform,
            post_bg: None,
            hdr: None,
            hdr_size: (0, 0),
            linear_sampler,
        }
    }

    pub fn register_mesh(&mut self, vertices: &[Vertex], indices: &[u16]) -> MeshId {
        let vb = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("helix.vbuf"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ib = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("helix.ibuf"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let id = MeshId(self.next_mesh);
        self.next_mesh += 1;
        self.meshes.insert(id.0, GpuMesh { vb, ib, count: indices.len() as u32 });
        id
    }

    fn ensure_depth(&mut self, size: (u32, u32)) -> wgpu::TextureView {
        if self.depth_size != size {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("target:depth"),
                size: wgpu::Extent3d {
                    width: size.0.max(1),
                    height: size.1.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: crate::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            self.depth = Some((tex, view));
            self.depth_size = size;
        }
        self.depth.as_ref().unwrap().1.clone()
    }

    fn ensure_hdr(&mut self, size: (u32, u32)) {
        if self.hdr_size != size {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("target:hdr_color"),
                size: wgpu::Extent3d {
                    width: size.0.max(1),
                    height: size.1.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            self.post_bg = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("helix.post-bg"),
                layout: &self.post_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.post_uniform.as_entire_buffer_binding(),
                    },
                ],
            }));
            self.hdr = Some((tex, view));
            self.hdr_size = size;
        }
    }

    /// One encoder, two declared-order passes: MainPass -> HDR, PostPass ->
    /// target (linear upsample + tonemap + single sRGB encode).
    pub fn render_to_view(
        &mut self,
        target: &wgpu::TextureView,
        size: (u32, u32),
        view_proj: Mat4,
        camera_pos: Vec3,
        draws: &[DrawCall],
        render_scale: f32,
        exposure_ev: f32,
    ) {
        let scale = render_scale.clamp(0.5, 1.0); // RES-1 bounds
        let hdr_size = (
            ((size.0 as f32 * scale) as u32).max(1),
            ((size.1 as f32 * scale) as u32).max(1),
        );
        self.ensure_hdr(hdr_size);
        let hdr_view = self.hdr.as_ref().unwrap().1.clone();
        let depth_view = self.ensure_depth(hdr_size);

        let exposure = 2.0f32.powf(exposure_ev);
        self.queue
            .write_buffer(&self.post_uniform, 0, bytemuck::bytes_of(&[exposure, 0.0, 0.0, 0.0]));

        while self.objects.len() < draws.len() {
            let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("helix.object-uniform"),
                size: std::mem::size_of::<ObjectUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("helix.object-bg"),
                layout: &self.bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_buffer_binding(),
                }],
            });
            self.objects.push(GpuObject { uniform, bind_group });
        }
        let vp_arr = view_proj.to_cols_array_2d();
        let eye = [camera_pos.x, camera_pos.y, camera_pos.z, 1.0];
        for (obj, dc) in self.objects.iter_mut().zip(draws) {
            let u = ObjectUniforms {
                view_proj: vp_arr,
                model: dc.model.to_cols_array_2d(),
                color: dc.color,
                view_pos: eye,
            };
            self.queue.write_buffer(&obj.uniform, 0, bytemuck::bytes_of(&u));
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("helix.frame") });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pass:MainPass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &hdr_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.07,
                            b: 0.10,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(crate::DEPTH_CLEAR),
                        store: wgpu::StoreOp::Store,
                    },
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            for (slot, dc) in draws.iter().enumerate() {
                let mesh = self.meshes.get(&dc.mesh.0).expect("unregistered mesh");
                pass.set_bind_group(0, &self.objects[slot].bind_group, &[]);
                pass.set_vertex_buffer(0, mesh.vb.slice(..));
                pass.set_index_buffer(mesh.ib.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..mesh.count, 0, 0..1);
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pass:PostPass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.post_pipeline);
            pass.set_bind_group(0, self.post_bg.as_ref().expect("post bg"), &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
    }
}

pub struct Renderer {
    core: CoreRenderer,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

impl Renderer {
    /// Returns probed capabilities alongside the renderer (CAP-1).
    pub async fn new(window: Arc<Window>, vsync: bool) -> (Self, crate::caps::Capabilities) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let surface: wgpu::Surface<'static> =
            instance.create_surface(window.clone()).expect("surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("no suitable GPU adapter");

        let caps = crate::caps::probe(&adapter);
        let surface_caps = surface.get_capabilities(&adapter);
        let format = pick_format(&surface_caps.formats);

        // FG-1/FG-3 + REN-11: validate the declared graph at boot.
        let graph = crate::framegraph::FrameGraph::default_graph();
        for warning in graph.validate().expect("engine frame graph is invalid") {
            log::warn!("frame graph: {warning}");
        }
        log::debug!("{}", graph.dump_dot());

        let core = CoreRenderer::new(&adapter, format).await;
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: if vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            },
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&core.device, &config);
        (Self { core, surface, config }, caps)
    }

    pub fn register_mesh(&mut self, v: &[Vertex], i: &[u16]) -> MeshId {
        self.core.register_mesh(v, i)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return; // WIN-1
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.core.device, &self.config);
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn render(&mut self, frame: &crate::render_thread::FrameData) -> Result<(), wgpu::SurfaceError> {
        let ftex = self.surface.get_current_texture()?;
        let view = ftex.texture.create_view(&Default::default());
        let v0 = &frame.views[0]; // CAM-2: swapchain view first
        self.core.render_to_view(
            &view,
            frame.size,
            v0.view_proj,
            v0.position,
            &v0.draws,
            frame.render_scale,
            frame.exposure_ev,
        );
        ftex.present();
        Ok(())
    }
}
