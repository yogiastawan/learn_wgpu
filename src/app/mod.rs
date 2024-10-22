use std::cell::{Cell, RefCell};

#[cfg(debug_assertions)]
use log::info;

use sdl2::{event::Event, keyboard::Keycode, video::Window, EventPump};
use wgpu::{
    include_wgsl, Adapter, Backends, BlendState, ColorWrites, CommandEncoderDescriptor, Device,
    DeviceDescriptor, Instance, PipelineCompilationOptions, Queue, RenderPipeline,
    RenderPipelineDescriptor, RequestAdapterOptions, Surface, SurfaceCapabilities,
    SurfaceConfiguration, SurfaceTargetUnsafe, TextureFormat,
};

pub struct XApp<'l> {
    #[cfg(target_os = "android")]
    wgpu_intance: RefCell<Option<Instance>>,
    surface: RefCell<Option<Surface<'l>>>,
    device: RefCell<Option<Device>>,
    config: RefCell<Option<SurfaceConfiguration>>,
    surface_format: RefCell<Option<TextureFormat>>,
    queue: RefCell<Option<Queue>>,
    pipeline: RefCell<Option<RenderPipeline>>,
    event_pump: RefCell<Option<EventPump>>,
    window: RefCell<Option<Window>>,
    window_height: Cell<u32>,
    window_width: Cell<u32>,
}

impl<'l> XApp<'l> {
    pub fn new() -> Self {
        XApp {
            #[cfg(target_os = "android")]
            wgpu_intance: RefCell::new(None),
            surface: RefCell::new(None),
            device: RefCell::new(None),
            config: RefCell::new(None),
            surface_format: RefCell::new(None),
            queue: RefCell::new(None),
            pipeline: RefCell::new(None),
            window: RefCell::new(None),
            window_height: Cell::new(0),
            window_width: Cell::new(0),
            event_pump: RefCell::new(None),
        }
    }

    pub fn init(&self, window_title: &str) -> Result<(), String> {
        // Init env_logger to show wgpu log error
        #[cfg(debug_assertions)]
        env_logger::init();

        // Init SDL2
        let sdl_ctx = sdl2::init()?;
        #[cfg(target_os = "android")]
        sdl2::hint::set("SDL_VIDEO_EXTERNAL_CONTEXT", "1");
        let sdl_video_subsystem = sdl_ctx.video()?;
        *self.event_pump.borrow_mut() = Some(sdl_ctx.event_pump()?);

        let window = sdl_video_subsystem
            .window(window_title, 0, 0)
            .fullscreen()
            .position_centered()
            .allow_highdpi()
            .build()
            .map_err(|e| e.to_string())?;
        let (w, h) = window.size();
        //set window size
        self.window_width.set(w);
        self.window_height.set(h);
        //set window
        *self.window.borrow_mut() = Some(window);

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
        self.init_surface(&instance);
        // get adapter
        let adapter = self.get_adapter(&instance)?;

        #[cfg(target_os = "android")]
        {
            *self.wgpu_intance.borrow_mut() = Some(instance);
        }

        // get surface texture format
        let surface_capabilities = self.get_surface_capability(&adapter)?;
        let surface_format = surface_capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_capabilities.formats[0]);
        *self.surface_format.borrow_mut() = Some(surface_format);
        // get device and queue
        self.get_device_and_queue(&adapter)?;

        // create config
        let config = self.create_config()?;
        *self.config.borrow_mut() = Some(config);
        // run surface configuration
        self.configure_surface()?;

        *self.pipeline.borrow_mut() = Some(self.init_pipeline()?);
        Ok(())
    }

    pub fn run(&self) -> Result<(), String> {
        let mut event_pump = self.event_pump.borrow_mut();
        let event_pump = match event_pump.as_mut() {
            Some(x) => x,
            None => {
                #[cfg(debug_assertions)]
                {
                    log::error!("Error event_pump is None");
                }
                return Err("Event pum is None".to_string());
            }
        };

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
                            let inst = self.wgpu_intance.borrow();
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

    fn init_surface(&self, instance: &Instance) -> Result<(), String> {
        let window = self.window.borrow();
        let window = match window.as_ref() {
            Some(x) => x,
            None => {
                #[cfg(debug_assertions)]
                {
                    log::error!("fn: create_surface. Error: XApp's window is None");
                }
                return Err("Window is empty".to_owned());
            }
        };

        let surface = unsafe {
            let target = SurfaceTargetUnsafe::from_window(window).map_err(|e| e.to_string())?;
            match instance.create_surface_unsafe(target) {
                Ok(x) => x,
                Err(e) => return Err(e.to_string()),
            }
        };

        *self.surface.borrow_mut() = Some(surface);

        Ok(())
    }

    fn get_adapter(&self, instance: &Instance) -> Result<Adapter, String> {
        let surface = self.surface.borrow();
        let adapter_option = RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: surface.as_ref(),
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
        Ok(adapter)
    }

    fn get_device_and_queue(&self, adapter: &Adapter) -> Result<(), String> {
        let lim = adapter.limits();
        let device_desc = DeviceDescriptor {
            label: Some("Xapp Device"),
            required_limits: lim,
            ..Default::default()
        };
        let device = pollster::block_on(adapter.request_device(&device_desc, None));
        let (device, queue) = device.map_err(|e| e.to_string())?;
        *self.device.borrow_mut() = Some(device);
        *self.queue.borrow_mut() = Some(queue);

        Ok(())
    }

    fn get_surface_capability(&self, adapter: &Adapter) -> Result<SurfaceCapabilities, String> {
        let surface = self.surface.borrow();
        let surface = match surface.as_ref() {
            Some(x) => x,
            None => {
                #[cfg(debug_assertions)]
                log::error!("fn: get_surface_format. Error: XApp's surface is None");
                return Err("Surface is empty".to_string());
            }
        };
        let surface_capability = surface.get_capabilities(adapter);
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
        Ok(surface_capability)
    }

    fn create_config(&self) -> Result<SurfaceConfiguration, String> {
        let surface_format = self.surface_format.borrow();
        let surface_format = match surface_format.as_ref() {
            Some(x) => x,
            None => {
                #[cfg(debug_assertions)]
                {
                    log::error!("fn: create_config. Error XApp's surface_format is None.");
                }
                return Err("Surface format is empty".to_string());
            }
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: *surface_format,
            width: self.window_width.get(),
            height: self.window_height.get(),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: Vec::default(),
            desired_maximum_frame_latency: 2,
        };
        Ok(config)
    }

    fn configure_surface(&self) -> Result<(), String> {
        let config = self.config.borrow();
        let config = match config.as_ref() {
            Some(x) => x,
            None => {
                #[cfg(debug_assertions)]
                {
                    log::error!("fn: configure_surface. Error XApp's config is None.");
                }
                return Err("Config field is empty".to_string());
            }
        };

        let device = self.device.borrow();
        let device = match device.as_ref() {
            Some(x) => x,
            None => {
                #[cfg(debug_assertions)]
                {
                    log::error!("fn: configure_surface. Error XApp's device is None.");
                }
                return Err("Device field is empty".to_string());
            }
        };

        if let Some(x) = self.surface.borrow_mut().as_ref() {
            x.configure(device, config);
        } else {
            #[cfg(debug_assertions)]
            {
                log::error!("fn: configure_surface. Error XApp's surface is None.");
            }
            return Err("Surface field is empty".to_string());
        }

        Ok(())
    }

    fn render(&self) -> Result<(), String> {
        let output = self
            .surface
            .borrow()
            .as_ref()
            .unwrap()
            .get_current_texture()
            .map_err(|e| e.to_string())?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .borrow_mut()
            .as_ref()
            .unwrap()
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

            render_pass.set_pipeline(self.pipeline.borrow().as_ref().unwrap());
            render_pass.draw(0..3, 0..1)
        }

        self.queue
            .borrow()
            .as_ref()
            .unwrap()
            .submit([encoder.finish()]);
        output.present();

        Ok(())
    }

    fn init_pipeline(&self) -> Result<RenderPipeline, String> {
        let device = self.device.borrow();
        let device = match device.as_ref() {
            Some(x) => x,
            None => return Err("Device of XApp is empty".to_string()),
        };
        let shader = device.create_shader_module(include_wgsl!("shader.wgsl"));
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipe_line_layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        let color_target = [Some(wgpu::ColorTargetState {
            format: self.surface_format.borrow().unwrap(),
            blend: Some(BlendState::REPLACE),
            write_mask: ColorWrites::ALL,
        })];
        let pipeline_desc = RenderPipelineDescriptor {
            label: Some("render_pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
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
        Ok(render_pipeline)
    }
}
