use std::cell::{Cell, RefCell};

#[cfg(debug_assertions)]
use log::info;

use sdl2::{event::Event, keyboard::Keycode, video::Window, EventPump, Sdl};
use wgpu::{
    include_wgsl, util::DeviceExt, Backends, BlendState, ColorWrites, CommandEncoderDescriptor,
    Device, DeviceDescriptor, Instance, PipelineCompilationOptions, Queue, RenderPipeline,
    RenderPipelineDescriptor, RequestAdapterOptions, Surface, SurfaceConfiguration,
    SurfaceTargetUnsafe, TextureFormat,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-0.5, 0.5, 0.0],
        color: [1.0, 0.0, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5, 0.0],
        color: [0.0, 1.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.0],
        color: [0.0, 0.0, 1.0],
    },
];

pub struct XApp<'l> {
    sdl_ctx: Sdl,
    #[cfg(target_os = "android")]
    wgpu_intance: Instance,
    surface: Surface<'l>,
    device: Device,
    config: SurfaceConfiguration,
    surface_format: TextureFormat,
    queue: Queue,
    pipeline: RenderPipeline,
    // event_pump: EventPump,
    window: Window,
    window_height: u32,
    window_width: u32,

    vertex_buffer: wgpu::Buffer,
}

impl<'l> XApp<'l> {
    pub fn new(window_title: &str) -> Result<Self, String> {
        // Init env_logger to show wgpu log error
        #[cfg(debug_assertions)]
        env_logger::init();

        // Init SDL2
        let sdl_ctx = sdl2::init()?;
        #[cfg(target_os = "android")]
        sdl2::hint::set("SDL_VIDEO_EXTERNAL_CONTEXT", "1");
        let sdl_video_subsystem = sdl_ctx.video()?;
        // let event_pump = sdl_ctx.event_pump()?;

        let window = sdl_video_subsystem
            .window(window_title, 0, 0)
            .fullscreen()
            .position_centered()
            .allow_highdpi()
            .build()
            .map_err(|e| e.to_string())?;
        let (w, h) = window.size();

        //create instance
        let backend = {
            #[cfg(debug_assertions)]
            {
                info!("Targeting debug use backend: Secondary");
                Backends::SECONDARY
            }

            #[cfg(not(debug_assertions))]
            {
                Backends::PRIMARY
            }
        };

        let instance = Instance::new(wgpu::InstanceDescriptor {
            backends: backend,
            gles_minor_version: wgpu::Gles3MinorVersion::Version0,
            ..Default::default()
        });

        // create surface
        let surface = unsafe {
            let target = SurfaceTargetUnsafe::from_window(&window).map_err(|e| e.to_string())?;
            match instance.create_surface_unsafe(target) {
                Ok(x) => x,
                Err(e) => return Err(e.to_string()),
            }
        };

        // get adapter
        let adapter = {
            let adapter_option = RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            };

            let adapter = pollster::block_on(instance.request_adapter(&adapter_option));
            let adapter = match adapter {
                Some(x) => x,
                None => return Err("Cannot get adapter".to_string()),
            };

            #[cfg(debug_assertions)]
            {
                let down_level_capa = adapter.get_downlevel_capabilities();
                let features = adapter.features();
                let limits = adapter.limits();
                let info = adapter.get_info();

                info!("Adapter: ");
                info!(" - Info: {:?}", info);
                info!(" - Adapter limits: {:?}", limits);
                info!(" - Features {:?}", features);
                info!(" - Down level capacity: {:?}", down_level_capa);
            }
            adapter
        };

        // get surface texture format
        let surface_capabilities = {
            let surface_capability = surface.get_capabilities(&adapter);
            #[cfg(debug_assertions)]
            {
                let texture_formats = &surface_capability.formats;
                let alpha_modes = &surface_capability.alpha_modes;
                let present_modes = &surface_capability.present_modes;
                let usage = &surface_capability.usages;

                info!("Surface Capabilities:");
                info!(" - Texture format:");
                for (i, tf) in texture_formats.iter().enumerate() {
                    info!("  {}. {:?}", i, tf);
                }

                info!(" - Alpha Mode:");
                for (i, am) in alpha_modes.iter().enumerate() {
                    info!("  {}. {:?}", i, am);
                }

                info!(" - Present Mode:");
                for (i, pm) in present_modes.iter().enumerate() {
                    info!("  {}. {:?}", i, pm);
                }

                info!(" - Usage: {:?}", usage);
            }
            surface_capability
        };
        let surface_format = surface_capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_capabilities.formats[0]);

        // get device and queue
        let (device, queue) = {
            let lim = adapter.limits();
            let device_desc = DeviceDescriptor {
                label: Some("Xapp Device"),
                required_limits: lim,
                ..Default::default()
            };
            let device = pollster::block_on(adapter.request_device(&device_desc, None));
            match device.map_err(|e| e.to_string()) {
                Ok((x, y)) => (x, y),
                Err(e) => return Err(e),
            }
        };

        // create config
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format.clone(),
            width: w,
            height: h,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: Vec::default(),
            desired_maximum_frame_latency: 2,
        };
        // run surface configuration
        surface.configure(&device, &config);

        //create pipe line
        let shader = device.create_shader_module(include_wgsl!("shader.wgsl"));
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipe_line_layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        let color_target = [Some(wgpu::ColorTargetState {
            format: surface_format.clone(),
            blend: Some(BlendState::REPLACE),
            write_mask: ColorWrites::ALL,
        })];
        let pipeline_desc = RenderPipelineDescriptor {
            label: Some("render_pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &color_target,
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },

            multiview: None,
            cache: None,
        };

        let render_pipeline = device.create_render_pipeline(&pipeline_desc);

        //create vertext buffer
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertice triangle"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Ok(XApp {
            sdl_ctx: sdl_ctx,
            #[cfg(target_os = "android")]
            wgpu_intance: instance,
            surface: surface,
            device: device,
            config: config,
            surface_format: surface_format,
            queue: queue,
            pipeline: render_pipeline,
            window: window,
            window_height: w,
            window_width: h,
            // event_pump: event_pump,
            vertex_buffer: vertex_buffer,
        })
    }

    pub fn run(&self) -> Result<(), String> {
        let mut event_pump = self.sdl_ctx.event_pump()?;

        'run: loop {
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { timestamp } => {
                        #[cfg(debug_assertions)]
                        {
                            info!("Exiting XApp. Running for about {}", timestamp);
                        }
                        break 'run;
                    }
                    Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        timestamp,
                        ..
                    } => {
                        #[cfg(all(not(target_os = "android"), debug_assertions))]
                        {
                            info!(
                                "Exiting XApp from escape key. Running for about {}",
                                timestamp
                            );
                            break 'run;
                        }
                    }

                    Event::AppWillEnterForeground { timestamp } => {
                        #[cfg(debug_assertions)]
                        {
                            info!(
                                "Will enter foreground (onResume) XApp. Running for about {}",
                                timestamp
                            );
                        }

                        #[cfg(target_os = "android")]
                        {
                            let inst = self.wgpu_intance;
                            let inst = match inst.as_ref() {
                                Some(x) => x,
                                None => {
                                    return Err("WGPU intance is empty".to_string());
                                }
                            };
                            let _ = self.init_surface(inst)?;
                        }
                    }
                    e => {
                        #[cfg(debug_assertions)]
                        info!("{:?}", e);
                    }
                }
            }

            self.render()?;
        }
        Ok(())
    }

    fn render(&self) -> Result<(), String> {
        let output = self
            .surface
            .get_current_texture()
            .map_err(|e| e.to_string())?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Render encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..3, 0..1);
            render_pass.draw(0..VERTICES.len() as u32, 0..1)
        }

        self.queue.submit([encoder.finish()]);
        output.present();

        Ok(())
    }
}
