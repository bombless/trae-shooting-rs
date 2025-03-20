use wgpu::util::DeviceExt;
// use glam::Vec3; - 不需要，已在main.rs中导入

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ModelVertex {
    position: [f32; 3],
    color: [f32; 3],
    tex_coords: [f32; 2],  // 添加纹理坐标
    model_type: f32,
}

// 手动实现 bytemuck traits
unsafe impl bytemuck::Pod for ModelVertex {}
unsafe impl bytemuck::Zeroable for ModelVertex {}

impl ModelVertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ModelVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // color
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // tex_coords
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // model_type
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

// 在文件开头添加
use crate::texture::Texture;

// 修改 Model 结构体
pub struct Model {
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
    pub color: [f32; 3],
    pub model_type: u32,
    pub texture: Option<Texture>,  // 添加纹理字段
}

// 修改 Model::new 方法
impl Model {
    pub fn new(
        device: &wgpu::Device,
        name: &str,
        vertices: &[ModelVertex],
        indices: &[u16],
        color: [f32; 3],
        is_wall: bool,
        texture: Option<Texture>,  // 添加纹理参数
    ) -> Self {
        let vertex_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} Vertex Buffer", name)),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }
        );
        
        let index_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} Index Buffer", name)),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            }
        );
        
        Self {
            name: name.to_string(),
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
            color,
            model_type: if is_wall { 1 } else { 0 },
            texture,  // 添加纹理
        }
    }

    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
    }
}

// Create a checkerboard pattern for floor or ceiling
// 修改创建棋盘的函数
fn create_checkerboard(
    device: &wgpu::Device,
    name: &str,
    size: f32,
    tile_size: f32,
    height: f32,
    color1: [f32; 3],
    color2: [f32; 3],
    is_ceiling: bool, // 添加参数控制朝向
) -> Model {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let tiles = (size / tile_size) as i32;
    
    for x in -tiles..=tiles {
        for z in -tiles..=tiles {
            let x0 = x as f32 * tile_size;
            let z0 = z as f32 * tile_size;
            let x1 = x0 + tile_size;
            let z1 = z0 + tile_size;
            
            let color = if (x + z) % 2 == 0 { color1 } else { color2 };
            let base_idx = vertices.len() as u16;

            // 根据是否为天花板调整顶点顺序
            if is_ceiling {
                vertices.push(ModelVertex { position: [x0, height, z0], color, model_type: 0.0, tex_coords: [0.0, 0.0]  });
                vertices.push(ModelVertex { position: [x1, height, z0], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
                vertices.push(ModelVertex { position: [x1, height, z1], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
                vertices.push(ModelVertex { position: [x0, height, z1], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
            } else {
                vertices.push(ModelVertex { position: [x0, height, z0], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
                vertices.push(ModelVertex { position: [x0, height, z1], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
                vertices.push(ModelVertex { position: [x1, height, z1], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
                vertices.push(ModelVertex { position: [x1, height, z0], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
            }
            
            indices.extend_from_slice(&[
                base_idx, base_idx + 1, base_idx + 2,
                base_idx, base_idx + 2, base_idx + 3,
            ]);
        }
    }
        
    Model::new(device, name, &vertices, &indices, [0.0, 0.0, 0.0], false, None)
}

// Create a wall with thickness
// 修改创建墙体的函数
fn create_wall(
    device: &wgpu::Device,
    start: [f32; 3],
    end: [f32; 3],
    height: f32,
    color: [f32; 3],
) -> Model {

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    // Calculate wall direction and length
    let dx = end[0] - start[0];
    let dz = end[2] - start[2];
    
    // Define wall thickness
    let thickness = 0.3; // 30cm thickness
    
    // Calculate normal vector to the wall (perpendicular)
    let length = (dx*dx + dz*dz).sqrt();
    let nx = -dz / length;
    let nz = dx / length;
    
    // Calculate the four corners of the front face
    let front_bl = [start[0], 0.0, start[2]];
    let front_br = [end[0], 0.0, end[2]];
    let front_tr = [end[0], height, end[2]];
    let front_tl = [start[0], height, start[2]];
    
    // Calculate the four corners of the back face (offset by thickness in normal direction)
    let back_bl = [start[0] + nx * thickness, 0.0, start[2] + nz * thickness];
    let back_br = [end[0] + nx * thickness, 0.0, end[2] + nz * thickness];
    let back_tr = [end[0] + nx * thickness, height, end[2] + nz * thickness];
    let back_tl = [start[0] + nx * thickness, height, start[2] + nz * thickness];
    
    // Add all 8 vertices
    // 在 create_wall 函数中修改顶点创建部分
    // Front face vertices
    vertices.push(ModelVertex { position: front_bl, color, tex_coords: [0.0, 1.0], model_type: 1.0 });
    vertices.push(ModelVertex { position: front_br, color, tex_coords: [1.0, 1.0], model_type: 1.0 });
    vertices.push(ModelVertex { position: front_tr, color, tex_coords: [1.0, 0.0], model_type: 1.0 });
    vertices.push(ModelVertex { position: front_tl, color, tex_coords: [0.0, 0.0], model_type: 1.0 });
    
    // Back face vertices
    vertices.push(ModelVertex { position: back_bl, color, tex_coords: [0.0, 1.0], model_type: 1.0 });
    vertices.push(ModelVertex { position: back_br, color, tex_coords: [1.0, 1.0], model_type: 1.0 });
    vertices.push(ModelVertex { position: back_tr, color, tex_coords: [1.0, 0.0], model_type: 1.0 });
    vertices.push(ModelVertex { position: back_tl, color, tex_coords: [0.0, 0.0], model_type: 1.0 });
    
    // Add indices for all six faces (each face has two triangles)
    let base_idx = 0;
    
    // Front face (0,1,2,3)
    indices.push(base_idx);
    indices.push(base_idx + 2);
    indices.push(base_idx + 1);
    indices.push(base_idx);
    indices.push(base_idx + 3);
    indices.push(base_idx + 2);
    
    // Back face (4,5,6,7)
    indices.push(base_idx + 4);
    indices.push(base_idx + 5);
    indices.push(base_idx + 6);
    indices.push(base_idx + 4);
    indices.push(base_idx + 6);
    indices.push(base_idx + 7);
    
    // Top face (3,2,6,7)
    indices.push(base_idx + 3);
    indices.push(base_idx + 6);
    indices.push(base_idx + 2);
    indices.push(base_idx + 3);
    indices.push(base_idx + 7);
    indices.push(base_idx + 6);
    
    // Bottom face (0,1,5,4)
    indices.push(base_idx);
    indices.push(base_idx + 1);
    indices.push(base_idx + 5);
    indices.push(base_idx);
    indices.push(base_idx + 5);
    indices.push(base_idx + 4);
    
    // Left face (0,3,7,4)
    indices.push(base_idx);
    indices.push(base_idx + 7);
    indices.push(base_idx + 3);
    indices.push(base_idx);
    indices.push(base_idx + 4);
    indices.push(base_idx + 7);
    
    // Right face (1,2,6,5)
    indices.push(base_idx + 1);
    indices.push(base_idx + 6);
    indices.push(base_idx + 5);
    indices.push(base_idx + 1);
    indices.push(base_idx + 2);
    indices.push(base_idx + 6);

    Model::new(device, "wall", &vertices, &indices, [0.5, 0.5, 0.5], true, None)
}

// Create a wall edge (black outline) as a cylinder
fn create_wall_edge(
    device: &wgpu::Device,
    start: [f32; 3],
    end: [f32; 3],
    height: f32,
    wall_thickness: f32,
) -> Model {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    // Define cylinder parameters
    let radius = 0.025; // 2.5cm radius for the cylinder (thinner than before)
    let segments = 8; // Number of segments in the cylinder (can adjust for smoother/rougher)
    
    // Black color for all edges
    let color = [0.0, 0.0, 0.0];
    
    // Helper function to create a cylinder
    let mut create_cylinder = |base_x: f32, base_z: f32| {
        let base_idx = vertices.len() as u16;
        
        // Create vertices for top and bottom circles
        for y in [0.0, height] {
            for i in 0..=segments {
                let angle = std::f32::consts::PI * 2.0 * (i as f32) / (segments as f32);
                let x = base_x + radius * angle.cos();
                let z = base_z + radius * angle.sin();
                vertices.push(ModelVertex {
                    position: [x, y, z],
                    color,
                    model_type: 0.0,
                    tex_coords: [i as f32 / segments as f32, if y == 0.0 { 1.0 } else { 0.0 }],
                });
            }
        }
        
        // Create indices for the cylinder wall
        for i in 0..segments {
            let top_start = base_idx + (segments + 1) + i;
            let top_end = base_idx + (segments + 1) + i + 1;
            let bottom_start = base_idx + i;
            let bottom_end = base_idx + i + 1;
            
            // First triangle
            indices.push(bottom_start);
            indices.push(top_start);
            indices.push(bottom_end);
            
            // Second triangle
            indices.push(bottom_end);
            indices.push(top_start);
            indices.push(top_end);
        }
    };
    
    // Calculate wall direction and length
    let dx = end[0] - start[0];
    let dz = end[2] - start[2];
    
    // Calculate normal vector to the wall (perpendicular)
    let length = (dx*dx + dz*dz).sqrt();
    let nx = -dz / length;
    let nz = dx / length;
    
    // Create cylinders at each corner
    // Front-left corner
    create_cylinder(start[0], start[2]);
    
    // Front-right corner
    create_cylinder(end[0], end[2]);
    
    // Back-left corner
    create_cylinder(
        start[0] + nx * wall_thickness,
        start[2] + nz * wall_thickness
    );
    
    // Back-right corner
    create_cylinder(
        end[0] + nx * wall_thickness,
        end[2] + nz * wall_thickness
    );
    
    Model::new(device, "wall_edge", &vertices, &indices, [0.0, 0.0, 0.0], false, None)
}

// Create the entire parking garage based on a 2D map
pub fn create_parking_garage(device: &wgpu::Device, _dog_texture: &Texture, map_data: &Vec<Vec<u8>>) -> (Vec<Model>, Vec<Vec<u8>>) {
    let mut models = Vec::new();
    
    // Define colors
    let floor_color1 = [0.0, 0.0, 0.0]; // Pure black
    let floor_color2 = [1.0, 1.0, 1.0]; // Pure white
    let ceiling_color1 = [0.5, 0.5, 1.0]; // Light blue
    let ceiling_color2 = [1.0, 1.0, 1.0]; // White
    let wall_color = [1.0, 1.0, 1.0]; // Pure white
    
    // 如果没有提供地图数据，创建默认地图
    let map = if map_data.is_empty() {
        create_default_map()
    } else {
        map_data.clone()
    };
    
    // 计算地图尺寸
    let map_height = map.len();
    let map_width = if map_height > 0 { map[0].len() } else { 0 };
    
    // 设置地图比例尺（每个单元格对应的游戏世界单位）
    let cell_size = 2.0;
    let wall_height = 4.0;
    let wall_thickness = 0.3;
    
    // 计算地图的总尺寸
    let garage_width = map_width as f32 * cell_size;
    let garage_length = map_height as f32 * cell_size;
    
    // 计算地图原点在游戏世界中的位置（使地图居中）
    let origin_x = -garage_width / 2.0;
    let origin_z = -garage_length / 2.0;
    
    // Create floor (black and white checkerboard)
    let floor = create_checkerboard(
        device,
        "floor",
        garage_width.max(50.0), // 使用地图尺寸或最小尺寸50.0
        2.0,  // tile size
        0.0,  // height (at ground level)
        floor_color1,
        floor_color2,
        false
    );
    models.push(floor);
    
    // Create ceiling (blue and white checkerboard)
    let ceiling = create_checkerboard(
        device,
        "ceiling",
        garage_width.max(50.0), // 使用地图尺寸或最小尺寸50.0
        2.0,  // tile size
        wall_height,  // height (ceiling height)
        ceiling_color1,
        ceiling_color2,
        true
    );
    models.push(ceiling);
    
    // 根据地图数据创建墙体
    for y in 0..map_height {
        for x in 0..map_width {
            // 如果当前单元格是墙体
            if map[y][x] == 1 {
                // 计算墙体在游戏世界中的位置
                let wall_x = origin_x + x as f32 * cell_size;
                let wall_z = origin_z + y as f32 * cell_size;
                
                // 检查四个方向，如果相邻单元格不是墙体，则创建墙体
                
                // 上方（北）
                if y == 0 || map[y-1][x] == 0 {
                    let start = [wall_x, 0.0, wall_z];
                    let end = [wall_x + cell_size, 0.0, wall_z];
                    
                    let wall = create_wall(
                        device,
                        start,
                        end,
                        wall_height,
                        wall_color,
                    );
                    models.push(wall);
                    
                    let edge = create_wall_edge(
                        device,
                        start,
                        end,
                        wall_height,
                        wall_thickness,
                    );
                    models.push(edge);
                }
                
                // 下方（南）
                if y == map_height - 1 || map[y+1][x] == 0 {
                    let start = [wall_x, 0.0, wall_z + cell_size];
                    let end = [wall_x + cell_size, 0.0, wall_z + cell_size];
                    
                    let wall = create_wall(
                        device,
                        start,
                        end,
                        wall_height,
                        wall_color,
                    );
                    models.push(wall);
                    
                    let edge = create_wall_edge(
                        device,
                        start,
                        end,
                        wall_height,
                        wall_thickness,
                    );
                    models.push(edge);
                }
                
                // 左方（西）
                if x == 0 || map[y][x-1] == 0 {
                    let start = [wall_x, 0.0, wall_z];
                    let end = [wall_x, 0.0, wall_z + cell_size];
                    
                    let wall = create_wall(
                        device,
                        start,
                        end,
                        wall_height,
                        wall_color,
                    );
                    models.push(wall);
                    
                    let edge = create_wall_edge(
                        device,
                        start,
                        end,
                        wall_height,
                        wall_thickness,
                    );
                    models.push(edge);
                }
                
                // 右方（东）
                if x == map_width - 1 || map[y][x+1] == 0 {
                    let start = [wall_x + cell_size, 0.0, wall_z];
                    let end = [wall_x + cell_size, 0.0, wall_z + cell_size];
                    
                    let wall = create_wall(
                        device,
                        start,
                        end,
                        wall_height,
                        wall_color,
                    );
                    models.push(wall);
                    
                    let edge = create_wall_edge(
                        device,
                        start,
                        end,
                        wall_height,
                        wall_thickness,
                    );
                    models.push(edge);
                }
            }
        }
    }
    
    (models, map)
}

// 创建默认地图数据
pub fn create_default_map() -> Vec<Vec<u8>> {
    // 创建一个20x15的地图
    let mut map = vec![vec![0; 20]; 15];
    
    // 设置外墙
    for x in 0..20 {
        map[0][x] = 1;  // 上边界
        map[14][x] = 1; // 下边界
    }
    
    for y in 0..15 {
        map[y][0] = 1;  // 左边界
        map[y][19] = 1; // 右边界
    }
    
    // 设置入口（在上边界中间留出空隙）
    map[0][9] = 0;
    map[0][10] = 0;
    
    // 添加一些内部墙体
    for x in 5..15 {
        map[7][x] = 1; // 水平墙
    }
    
    for y in 3..12 {
        map[y][5] = 1; // 垂直墙
    }
    
    map
}