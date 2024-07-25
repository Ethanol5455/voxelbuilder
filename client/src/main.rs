use std::{
    fs::{self, File},
    io::Read,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::SystemTime,
};

use camera::{Camera, CameraUniform};
use cgmath::{Array, Vector2, Vector3};
use connection::{run_networking, NetworkingMessage};
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{CursorGrabMode, Window, WindowBuilder},
};

use anyhow::Result;
use common::{
    items::{ItemInfo, ItemManager, ItemType, TextureCoordinates},
    player_data::Player,
    Chunk, QuadVertex,
};

mod camera;
mod connection;
mod packets;
mod texture;

fn read_file_as_bytes(path: &str) -> Result<Vec<u8>> {
    let mut in_file = File::open(path)?;

    let metadata = fs::metadata(path)?;
    let mut buffer = vec![0; metadata.len() as usize];
    in_file.read(&mut buffer)?;

    Ok(buffer)
}

struct State {
    player: Player,
    to_networking: Sender<NetworkingMessage>,
    from_networking: Receiver<NetworkingMessage>,

    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,

    delta_time: f32,
    send_player_info_timer: SystemTime,

    camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,

    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,

    diffuse_bind_group: wgpu::BindGroup,
    depth_texture: texture::Texture,
}

impl State {
    // Creating some of the wgpu types requires async code
    async fn new(
        window: Window,
        to_networking: Sender<NetworkingMessage>,
        from_networking: Receiver<NetworkingMessage>,
        player: Player,
    ) -> Self {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let diffuse_bytes = read_file_as_bytes("res/texture_atlas.png").unwrap();
        let diffuse_texture = texture::Texture::from_bytes(
            &device,
            &queue,
            &diffuse_bytes,
            "./res/texture_atlas.png",
        )
        .unwrap();

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        let depth_texture =
            texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        let camera = Camera::new(player.position, player.rotation, &config);
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&texture_bind_group_layout, &camera_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[QuadVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let mut item_manager = ItemManager::new();
        item_manager.put_new_item(ItemInfo {
            item_type: ItemType::Air,
            is_transparent: true,
            show_in_inventory: false,
            name: "Air".to_string(),
            top_tex_coords: TextureCoordinates::extract_coordinates(
                Vector2 { x: 10, y: 10 },
                Vector2 { x: 0, y: 0 },
            ),
            side_tex_coords: TextureCoordinates::extract_coordinates(
                Vector2 { x: 10, y: 10 },
                Vector2 { x: 0, y: 0 },
            ),
            bottom_tex_coords: TextureCoordinates::extract_coordinates(
                Vector2 { x: 10, y: 10 },
                Vector2 { x: 0, y: 0 },
            ),
        });
        item_manager.put_new_item(ItemInfo {
            item_type: ItemType::Air,
            is_transparent: false,
            show_in_inventory: true,
            name: "Grass".to_string(),
            top_tex_coords: TextureCoordinates::extract_coordinates(
                Vector2 { x: 10, y: 10 },
                Vector2 { x: 3, y: 0 },
            ),
            side_tex_coords: TextureCoordinates::extract_coordinates(
                Vector2 { x: 10, y: 10 },
                Vector2 { x: 2, y: 0 },
            ),
            bottom_tex_coords: TextureCoordinates::extract_coordinates(
                Vector2 { x: 10, y: 10 },
                Vector2 { x: 1, y: 0 },
            ),
        });

        let mut test_chunk = Chunk::new(
            Vector3::from_value(0),
            item_manager.get_id_by_name("Grass".to_owned()).unwrap(),
        );
        test_chunk.build_mesh(&item_manager);
        println!("Building mesh!");

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&test_chunk.vertices.as_slice()),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&test_chunk.indices.as_slice()),
            usage: wgpu::BufferUsages::INDEX,
        });

        let num_indices = test_chunk.indices.len() as u32;

        Self {
            player,
            to_networking,
            from_networking,
            window,
            surface,
            device,
            queue,
            config,
            size,

            delta_time: 0.0,
            send_player_info_timer: SystemTime::now(),

            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,

            render_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,
            diffuse_bind_group,
            depth_texture,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn resize_window(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            self.depth_texture =
                texture::Texture::create_depth_texture(&self.device, &self.config, "depth_texture");

            self.camera.on_resize(&self.config);
        }
    }

    fn device_input(&mut self, event: &DeviceEvent) -> bool {
        self.camera.process_device_events(event)
    }

    fn window_input(&mut self, event: &WindowEvent) -> bool {
        self.camera.process_window_events(event)
    }

    fn update(&mut self) {
        if SystemTime::now()
            .duration_since(self.send_player_info_timer)
            .expect("Is time moving backwards?")
            .as_millis()
            > 1000
        {
            self.to_networking
                .send(NetworkingMessage::SendPlayerInfo(self.player.clone()))
                .expect("Could not send internal message. Is the networking thread down?");
            self.send_player_info_timer = SystemTime::now();
        }

        self.camera.update(self.delta_time);
        self.camera.update_player_position(&mut self.player);
        self.camera_uniform.update_view_proj(&self.camera);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[
                    // This is what @location(0) in the fragment shader targets
                    Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.89,
                                b: 1.0,
                                a: 1.0,
                            }),
                            store: true,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }

        // submit will accept anything that implements IntoIter
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub async fn run() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    window.set_cursor_grab(CursorGrabMode::Locked).unwrap();
    window.set_cursor_visible(false);

    let (networking_tx, from_networking) = mpsc::channel();
    let (to_networking, networking_rs) = mpsc::channel();
    thread::spawn(move || {
        let _ = run_networking(networking_tx, networking_rs);
    });

    for msg in &from_networking {
        match msg {
            NetworkingMessage::ConnectionEstablished => break,
            _ => eprintln!("Shouldn't have received networking message type {:?} before networking is connected", msg),
        }
    }
    println!("Connected to server");

    let mut player = None;
    for msg in &from_networking {
        match msg {
            NetworkingMessage::PlayerInfoReceived(info) => {player = Some(info); break;},
            _ => eprintln!("Shouldn't have received networking message type {:?} before networking is connected", msg),
        }
    }
    println!("Got player info");

    let mut state = State::new(window, to_networking, from_networking, player.unwrap()).await;

    let mut delta_timer = SystemTime::now();

    event_loop.run(move |event, _, control_flow| match event {
        Event::DeviceEvent { ref event, .. } => {
            state.device_input(event);
        }
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == state.window().id() => {
            if !state.window_input(event) {
                match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    WindowEvent::KeyboardInput { input, .. } => {
                        match (input.virtual_keycode, input.state) {
                            (Some(VirtualKeyCode::RControl), ElementState::Pressed) => {
                                state.window.set_cursor_visible(true);
                                state.window.set_cursor_grab(CursorGrabMode::None).unwrap();
                            }
                            _ => (),
                        }
                    }
                    WindowEvent::MouseInput {
                        state: button_state,
                        button,
                        ..
                    } => match (button, button_state) {
                        (_, ElementState::Pressed) => {
                            if state.window.has_focus() {
                                state.window.set_cursor_visible(false);
                                state
                                    .window
                                    .set_cursor_grab(CursorGrabMode::Locked)
                                    .unwrap();
                            }
                        }
                        _ => (),
                    },
                    WindowEvent::Resized(physical_size) => {
                        state.resize_window(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        state.resize_window(**new_inner_size);
                    }
                    _ => {}
                }
            }
        }
        Event::RedrawRequested(window_id) if window_id == state.window().id() => {
            state.delta_time = delta_timer.elapsed().unwrap().as_secs_f32();
            delta_timer = SystemTime::now();
            state.update();
            match state.render() {
                Ok(_) => {}
                // Reconfigure the surface if lost
                Err(wgpu::SurfaceError::Lost) => state.resize_window(state.size),
                // The system is out of memory, we should probably quit
                Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                // All other errors (Outdated, Timeout) should be resolved by the next frame
                Err(e) => eprintln!("{:?}", e),
            }
        }
        Event::MainEventsCleared => {
            // RedrawRequested will only trigger once, unless we manually
            // request it.
            state.window().request_redraw();
        }
        _ => {}
    });
}

fn main() {
    pollster::block_on(run());
}
