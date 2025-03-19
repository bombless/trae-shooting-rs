use winit::{event::*, event_loop::{ControlFlow, EventLoop}, window::{WindowBuilder, Window}};
use wgpu::util::DeviceExt;
use std::time::{Duration, Instant};
use glam::{Vec3, Mat4};
use gilrs::{Gilrs, Button, Event as GilrsEvent};
use std::sync::{Arc, Mutex};
use std::thread;

mod camera;
mod texture;
mod model;
mod collision;

// 添加颜色结构体
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
struct Color {
    r: f64,
    g: f64,
    b: f64,
}

impl Default for Color {
    fn default() -> Self {
        Color {
            r: 0.5,
            g: 0.5,
            b: 0.5,
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Underground Parking Shooter")
        .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720))
        .build(&event_loop)
        .unwrap();
    
    // 创建共享的墙体颜色状态
    let wall_color = Arc::new(Mutex::new(Color::default()));
    
    // 启动HTTP服务器线程
    let http_wall_color = wall_color.clone();
    thread::spawn(move || {
        start_http_server(http_wall_color);
    });
    
    let mut state = pollster::block_on(State::new(&window, wall_color));
    let mut last_render_time = Instant::now();
    
    // Initialize controller support
    let mut gilrs = Gilrs::new().unwrap();
    
    event_loop.run(move |event, _, control_flow| {
        // Controller input handling
        while let Some(GilrsEvent { id, event, time }) = gilrs.next_event() {
            state.input_controller(&id, &event);
        }
        
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                if !state.input(event) {
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
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            state.resize(**new_inner_size);
                        }
                        WindowEvent::KeyboardInput {
                            input: KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::F),
                                ..
                            },
                            ..
                        } => {
                            println!("toggle fullscreen");
                            // Toggle fullscreen state
                            state.is_fullscreen = !state.is_fullscreen;
                            
                            // Apply fullscreen change
                            if state.is_fullscreen {
                                window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                            } else {
                                window.set_fullscreen(None);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion{ delta, .. },
                .. 
            } => {
                state.process_mouse(delta.0, delta.1);
            }
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                let now = Instant::now();
                let dt = now - last_render_time;
                last_render_time = now;
                state.update(dt);
                
                match state.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("Error: {:?}", e),
                }
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => {}
        }
    });
}

// 启动HTTP服务器的函数
fn start_http_server(wall_color: Arc<Mutex<Color>>) {
    use warp::Filter;
    // 创建一个运行时
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    rt.block_on(async {
        // 创建一个路由处理颜色更新
        let wall_color_put = wall_color.clone();
        let color_route = warp::path("color")
            .and(warp::put())
            .and(warp::body::json())
            .map(move |new_color: Color| {
                let mut color = wall_color_put.lock().unwrap();
                *color = new_color;
                warp::reply::json(&*color)
            });
        
        // 获取当前颜色的路由
        let wall_color_get = wall_color.clone();
        let get_color = warp::path("color")
            .and(warp::get())
            .map(move || {
                let color = wall_color_get.lock().unwrap();
                warp::reply::json(&*color)
            });
        
        // 合并路由
        let routes = color_route.or(get_color);
        
        println!("HTTP服务器启动在 http://localhost:3030");
        println!("使用 PUT /color 更新墙体颜色");
        println!("使用 GET /color 获取当前墙体颜色");
        
        warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
    });
}

// 在 State 结构体中添加墙体颜色的缓冲区和绑定组
struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    camera: camera::Camera,
    camera_controller: camera::CameraController,
    camera_uniform: camera::CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    depth_texture: texture::Texture,
    models: Vec<model::Model>,
    is_fullscreen: bool,
    wall_color: Arc<Mutex<Color>>, // 添加墙体颜色
    wall_color_buffer: wgpu::Buffer,
    wall_color_bind_group: wgpu::BindGroup,
    texture_bind_group: wgpu::BindGroup, // 添加纹理绑定组
    wall_colliders: Vec<collision::WallCollider>, // 添加墙体碰撞器集合
}

impl State {
    async fn new(window: &Window, wall_color: Arc<Mutex<Color>>) -> Self {

        let size = window.inner_size();
        
        // Instance is a handle to the GPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });
        
        // Surface is the part of the window we draw to
        let surface = unsafe { instance.create_surface(&window) }.unwrap();
        
        // Adapter is a handle to the actual graphics card
        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ).await.unwrap();
        
        // Device is used for creating resources and Queue is used for submitting commands
        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ).await.unwrap();
        
        // Configure the surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
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

        

        // 加载狗狗纹理
        let dog_bytes = include_bytes!("../dog.png"); // 确保这个路径正确
        let dog_texture = texture::Texture::from_bytes(
            &device,
            &queue,
            dog_bytes,
            "dog_texture"
        ).expect("无法加载狗狗纹理");
        
        // Create depth texture
        let depth_texture = texture::Texture::create_depth_texture(&device, &config, "depth_texture");
        
        // Camera setup
        let camera = camera::Camera::new((0.0, 1.8, -2.0), 0.0, 0.0); // 将 z 坐标从 0.0 改为 2.0，让相机往前移动一些
        let camera_controller = camera::CameraController::new(4.0, 1.0);
        
        let mut camera_uniform = camera::CameraUniform::new();
        camera_uniform.update_view_proj(&camera, config.width as f32 / config.height as f32);
        
        let camera_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[camera_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        
        let camera_bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }
                ],
                label: Some("camera_bind_group_layout"),
            }
        );
        
        let camera_bind_group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                layout: &camera_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    }
                ],
                label: Some("camera_bind_group"),
            }
        );
        
        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        
        
        // Create models for the parking garage
        // 修改调用，传递引用
        let models = model::create_parking_garage(&device, &dog_texture);
        
        // 创建墙体碰撞器
        let mut wall_colliders = Vec::new();
        
        // 定义停车场的尺寸（与model.rs中的create_parking_garage函数保持一致）
        let garage_width = 30.0;
        let garage_length = 40.0;
        let wall_height = 4.0;
        
        // 前墙（入口处有缺口）
        wall_colliders.push(collision::create_wall_collider(
            [-garage_width/2.0, 0.0, -garage_length/2.0],
            [-5.0, 0.0, -garage_length/2.0],
            wall_height
        ));
        
        wall_colliders.push(collision::create_wall_collider(
            [5.0, 0.0, -garage_length/2.0],
            [garage_width/2.0, 0.0, -garage_length/2.0],
            wall_height
        ));
        
        // 后墙
        wall_colliders.push(collision::create_wall_collider(
            [-garage_width/2.0, 0.0, garage_length/2.0],
            [garage_width/2.0, 0.0, garage_length/2.0],
            wall_height
        ));
        
        // 左墙
        wall_colliders.push(collision::create_wall_collider(
            [-garage_width/2.0, 0.0, -garage_length/2.0],
            [-garage_width/2.0, 0.0, garage_length/2.0],
            wall_height
        ));
        
        // 右墙
        wall_colliders.push(collision::create_wall_collider(
            [garage_width/2.0, 0.0, -garage_length/2.0],
            [garage_width/2.0, 0.0, garage_length/2.0],
            wall_height
        ));
        
        // 内部墙体1
        wall_colliders.push(collision::create_wall_collider(
            [-10.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            wall_height
        ));
        
        // 内部墙体2
        wall_colliders.push(collision::create_wall_collider(
            [0.0, 0.0, 5.0],
            [0.0, 0.0, 15.0],
            wall_height
        ));

        
        // 创建墙体颜色 uniform 缓冲区
        let wall_color_data = [0.5f32, 0.5f32, 0.5f32, 0.0f32]; // 初始颜色 + padding

        
        let wall_color_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Wall Color Buffer"),
                contents: bytemuck::cast_slice(&wall_color_data),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        
        // 创建墙体颜色绑定组布局
        let wall_color_bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }
                ],
                label: Some("wall_color_bind_group_layout"),
            }
        );

        // 在创建墙体颜色绑定组布局后添加
        let texture_bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
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
                ],
                label: Some("texture_bind_group_layout"),
            }
        );
        
        // 创建纹理绑定组
        let texture_bind_group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                layout: &texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&dog_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&dog_texture.sampler),
                    },
                ],
                label: Some("texture_bind_group"),
            }
        );

        // 修改渲染管线布局，添加纹理绑定组布局
        let render_pipeline_layout = device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &camera_bind_group_layout,
                    &wall_color_bind_group_layout,
                    &texture_bind_group_layout, // 添加纹理绑定组布局
                ],
                push_constant_ranges: &[],
            }
        );

        // 创建渲染管线（使用上面创建的布局）
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout), // 使用包含墙体颜色绑定组的布局
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[model::ModelVertex::desc()],
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
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
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

        // 创建墙体颜色绑定组
        let wall_color_bind_group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                layout: &wall_color_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wall_color_buffer.as_entire_binding(),
                    }
                ],
                label: Some("wall_color_bind_group"),
            }
        );

        // 删除第二次创建的 render_pipeline_layout

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            camera,
            camera_controller,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            depth_texture,
            models,
            is_fullscreen: false,
            wall_color, // 添加墙体颜色
            wall_color_bind_group,
            wall_color_buffer,
            texture_bind_group, // 添加纹理绑定组
            wall_colliders, // 添加墙体碰撞器集合
        }
    }
    
    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = texture::Texture::create_depth_texture(
                &self.device,
                &self.config,
                "depth_texture"
            );
        }
    }
    
    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                input: KeyboardInput {
                    state: ElementState::Pressed,
                    virtual_keycode: Some(VirtualKeyCode::F),
                    ..                    
                },
                ..                
            } => {
                // Return true to indicate we've handled the F key press
                // The actual fullscreen toggle is handled in the main event loop
                false
            }
            _ => self.camera_controller.process_keyboard(event)
        }
    }
    
    fn process_mouse(&mut self, dx: f64, dy: f64) {
        self.camera_controller.process_mouse(dx, dy);
    }
    
    fn input_controller(&mut self, id: &gilrs::GamepadId, event: &gilrs::EventType) {
        self.camera_controller.process_controller(id, event);
    }
    
    fn update(&mut self, dt: std::time::Duration) {
        // 更新相机位置
        self.camera_controller.update_camera(&mut self.camera, dt);
        
        // 碰撞检测和响应
        let player_radius = 0.5; // 玩家碰撞半径
        let mut position = self.camera.position;
        
        // 对每个墙体进行碰撞检测
        for collider in &self.wall_colliders {
            position = collider.resolve_collision(position, player_radius);
        }
        
        // 更新相机位置
        self.camera.position = position;
        
        // 更新相机uniform
        self.camera_uniform.update_view_proj(&self.camera, self.config.width as f32 / self.config.height as f32);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));
        
        // 更新墙体颜色（如果有变化）
        self.update_wall_color();
    }
    
    fn update_wall_color(&mut self) {
        if let Ok(color) = self.wall_color.lock() {
            // 更新墙体颜色 uniform 缓冲区
            let wall_color_data = [
                color.r as f32,
                color.g as f32,
                color.b as f32,
                0.0f32, // padding
            ];
            self.queue.write_buffer(
                &self.wall_color_buffer,
                0,
                bytemuck::cast_slice(&wall_color_data)
            );
        }
    }
    
    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
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
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });
            
            // 在 render 方法中
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.wall_color_bind_group, &[]); 
            render_pass.set_bind_group(2, &self.texture_bind_group, &[]); // 设置纹理绑定组
            
            // Render all models
            for model in &self.models {
                model.draw(&mut render_pass);
            }
        }
        
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        
        Ok(())
    }
}
