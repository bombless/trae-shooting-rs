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
mod minimap;

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    minimap_pipeline: wgpu::RenderPipeline, // 添加小地图渲染管线
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
    map_data: Vec<Vec<u8>>, // 添加地图数据
    minimap: minimap::Minimap, // 添加小地图
    minimap_vertex_buffer: wgpu::Buffer, // 小地图顶点缓冲区
    minimap_index_buffer: wgpu::Buffer, // 小地图索引缓冲区
    minimap_indices_len: u32, // 小地图索引数量
    minimap_bind_group: wgpu::BindGroup, // 小地图绑定组
}

impl State {
    async fn new(window: &Window, wall_color: Arc<Mutex<Color>>) -> Self {

        let size = window.inner_size();
        
        // 创建默认地图数据
        let map_data = model::create_default_map();
        
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
        
        
        // Create models for the parking garage based on map data
        // 修改调用，传递地图数据并接收返回的模型和地图
        let (models, map_data) = model::create_parking_garage(&device, &dog_texture, &map_data);
        
        // 创建墙体碰撞器，基于地图数据生成
        let mut wall_colliders = Vec::new();
        
        // 定义停车场的尺寸（与model.rs中的create_parking_garage函数保持一致）
        let wall_height = 4.0;
        let cell_size = 2.0;
        
        // 计算地图尺寸
        let map_height = map_data.len();
        let map_width = if map_height > 0 { map_data[0].len() } else { 0 };
        
        // 计算地图的总尺寸
        let garage_width = map_width as f32 * cell_size;
        let garage_length = map_height as f32 * cell_size;
        
        // 计算地图原点在游戏世界中的位置（使地图居中）
        let origin_x = -garage_width / 2.0;
        let origin_z = -garage_length / 2.0;
        
        // 根据地图数据创建墙体碰撞器
        for y in 0..map_height {
            for x in 0..map_width {
                // 如果当前单元格是墙体
                if map_data[y][x] == 1 {
                    // 计算墙体在游戏世界中的位置
                    let wall_x = origin_x + x as f32 * cell_size;
                    let wall_z = origin_z + y as f32 * cell_size;
                    
                    // 检查四个方向，如果相邻单元格不是墙体，则创建墙体碰撞器
                    
                    // 上方（北）
                    if y == 0 || map_data[y-1][x] == 0 {
                        let start = [wall_x, 0.0, wall_z];
                        let end = [wall_x + cell_size, 0.0, wall_z];
                        
                        wall_colliders.push(collision::create_wall_collider(
                            start,
                            end,
                            wall_height
                        ));
                    }
                    
                    // 下方（南）
                    if y == map_height - 1 || map_data[y+1][x] == 0 {
                        let start = [wall_x, 0.0, wall_z + cell_size];
                        let end = [wall_x + cell_size, 0.0, wall_z + cell_size];
                        
                        wall_colliders.push(collision::create_wall_collider(
                            start,
                            end,
                            wall_height
                        ));
                    }
                    
                    // 左方（西）
                    if x == 0 || map_data[y][x-1] == 0 {
                        let start = [wall_x, 0.0, wall_z];
                        let end = [wall_x, 0.0, wall_z + cell_size];
                        
                        wall_colliders.push(collision::create_wall_collider(
                            start,
                            end,
                            wall_height
                        ));
                    }
                    
                    // 右方（东）
                    if x == map_width - 1 || map_data[y][x+1] == 0 {
                        let start = [wall_x + cell_size, 0.0, wall_z];
                        let end = [wall_x + cell_size, 0.0, wall_z + cell_size];
                        
                        wall_colliders.push(collision::create_wall_collider(
                            start,
                            end,
                            wall_height
                        ));
                    }
                }
            }
        }

        
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
            depth_stencil: None,
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
        
        // 创建小地图
        let minimap_size = 256; // 小地图纹理大小
        let minimap = minimap::Minimap::new(
            &device,
            &queue,
            &map_data,
            minimap_size,
            2.0, // 比例尺
            [10.0, 10.0], // 位置（左上角）
            [200.0, 200.0], // 尺寸
        );
        
        // 创建小地图顶点和索引缓冲区
        let (minimap_vertex_buffer, minimap_index_buffer, minimap_indices_len) = 
            minimap.create_vertices_and_indices(&device, size.width, size.height);
        
        // 创建小地图绑定组布局
        let minimap_bind_group_layout = device.create_bind_group_layout(
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
                label: Some("minimap_bind_group_layout"),
            }
        );
        
        // 创建小地图绑定组
        let minimap_bind_group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                layout: &minimap_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&minimap.texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&minimap.texture.sampler),
                    },
                ],
                label: Some("minimap_bind_group"),
            }
        );
        
        // 创建小地图渲染管线布局 - 使用专门的UI布局
        let minimap_pipeline_layout = device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
                label: Some("Minimap Pipeline Layout"),
                bind_group_layouts: &[&minimap_bind_group_layout], // 只需要小地图纹理绑定组
                push_constant_ranges: &[],
            }
        );
        
        // 创建小地图渲染管线 - 使用专门的UI着色器
        let minimap_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui_shader.wgsl").into()),
        });
        
        let minimap_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Minimap Pipeline"),
            layout: Some(&minimap_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &minimap_shader,
                entry_point: "vs_main",
                buffers: &[minimap::Minimap::vertex_buffer_layout()], // 使用小地图提供的顶点布局
            },
            fragment: Some(wgpu::FragmentState {
                module: &minimap_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            minimap_pipeline,
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
            map_data, // 添加地图数据
            minimap, // 添加小地图
            minimap_vertex_buffer, // 小地图顶点缓冲区
            minimap_index_buffer, // 小地图索引缓冲区
            minimap_indices_len, // 小地图索引数量
            minimap_bind_group, // 小地图绑定组
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
            
            // 更新小地图的顶点和索引缓冲区
            let (vertex_buffer, index_buffer, indices_len) = 
                self.minimap.create_vertices_and_indices(&self.device, new_size.width, new_size.height);
            self.minimap_vertex_buffer = vertex_buffer;
            self.minimap_index_buffer = index_buffer;
            self.minimap_indices_len = indices_len;
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
        
        // 更新小地图上的玩家位置
        let garage_width = self.map_data[0].len() as f32 * 2.0; // 每个单元格2.0单位
        let garage_length = self.map_data.len() as f32 * 2.0;
        let origin_x = -garage_width / 2.0;
        let origin_z = -garage_length / 2.0;
        
        self.minimap.update_player_position(
            &self.queue,
            self.camera.position,
            &self.map_data,
            2.0, // 地图比例尺
            [origin_x, origin_z], // 地图原点偏移
        );
        
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
        
        // 渲染3D场景
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
            
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.wall_color_bind_group, &[]); 
            render_pass.set_bind_group(2, &self.texture_bind_group, &[]); // 设置纹理绑定组
            
            // Render all models
            for model in &self.models {
                model.draw(&mut render_pass);
            }
        }
        
        // 渲染小地图（2D UI）
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Minimap Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // 使用Load操作，保留之前的渲染结果
                        store: true,
                    },
                })],
                depth_stencil_attachment: None, // 2D UI不需要深度测试
            });
            
            // 设置小地图渲染管线和绑定组 - 使用新的UI渲染管线
            render_pass.set_pipeline(&self.minimap_pipeline);
            // 只需要设置小地图纹理绑定组
            render_pass.set_bind_group(0, &self.minimap_bind_group, &[]);
            
            // 设置小地图顶点和索引缓冲区
            render_pass.set_vertex_buffer(0, self.minimap_vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.minimap_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            
            // 绘制小地图
            render_pass.draw_indexed(0..self.minimap_indices_len, 0, 0..1);
        }
        
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        
        Ok(())
    }
}
