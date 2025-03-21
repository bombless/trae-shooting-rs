use wgpu::util::DeviceExt;
use glam::{Vec3, Mat4};

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

// Create a wall edge (black outline)
fn create_wall_edge(
    device: &wgpu::Device,
    start: [f32; 3],
    end: [f32; 3],
    height: f32,
    wall_thickness: f32,
) -> Model {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    // Define edge thickness (slightly larger than the wall)
    let edge_thickness = 0.05; // 5cm thickness for the edge
    let edge_offset = 0.02; // 2cm offset to make edges visible from all angles
    
    // Calculate wall direction and length
    let dx = end[0] - start[0];
    let dz = end[2] - start[2];
    
    // Calculate normal vector to the wall (perpendicular)
    let length = (dx*dx + dz*dz).sqrt();
    let nx = -dz / length;
    let nz = dx / length;
    
    // Calculate tangent vector (along the wall)
    let tx = dx / length;
    let tz = dz / length;
    
    // Black color for all edges
    let color = [0.0, 0.0, 0.0];
    
    // Create vertices for the vertical edges (4 corners)
    
    // Front-left vertical edge - make it protrude in all directions
    let fl_base_idx = vertices.len() as u16;
    vertices.push(ModelVertex { position: [start[0] - edge_thickness - tx * edge_offset, 0.0, start[2] - edge_thickness - tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    vertices.push(ModelVertex { position: [start[0] + edge_thickness - tx * edge_offset, 0.0, start[2] + edge_thickness - tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    vertices.push(ModelVertex { position: [start[0] + edge_thickness - tx * edge_offset, height, start[2] + edge_thickness - tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    vertices.push(ModelVertex { position: [start[0] - edge_thickness - tx * edge_offset, height, start[2] - edge_thickness - tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    
    // Add indices for the front-left vertical edge - ensure correct winding order for visibility
    indices.push(fl_base_idx);
    indices.push(fl_base_idx + 1);
    indices.push(fl_base_idx + 2);
    indices.push(fl_base_idx);
    indices.push(fl_base_idx + 2);
    indices.push(fl_base_idx + 3);
    
    // Front-right vertical edge - make it protrude in all directions
    let fr_base_idx = vertices.len() as u16;
    vertices.push(ModelVertex { position: [end[0] - edge_thickness + tx * edge_offset, 0.0, end[2] - edge_thickness + tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    vertices.push(ModelVertex { position: [end[0] + edge_thickness + tx * edge_offset, 0.0, end[2] + edge_thickness + tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    vertices.push(ModelVertex { position: [end[0] + edge_thickness + tx * edge_offset, height, end[2] + edge_thickness + tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    vertices.push(ModelVertex { position: [end[0] - edge_thickness + tx * edge_offset, height, end[2] - edge_thickness + tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    
    // Add indices for the front-right vertical edge - ensure correct winding order for visibility
    indices.push(fr_base_idx);
    indices.push(fr_base_idx + 1);
    indices.push(fr_base_idx + 2);
    indices.push(fr_base_idx);
    indices.push(fr_base_idx + 2);
    indices.push(fr_base_idx + 3);
    
    // Back-left vertical edge (for walls with thickness) - make it protrude in all directions
    let bl_base_idx = vertices.len() as u16;
    vertices.push(ModelVertex { position: [start[0] + nx * wall_thickness - edge_thickness - tx * edge_offset, 0.0, start[2] + nz * wall_thickness - edge_thickness - tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    vertices.push(ModelVertex { position: [start[0] + nx * wall_thickness + edge_thickness - tx * edge_offset, 0.0, start[2] + nz * wall_thickness + edge_thickness - tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    vertices.push(ModelVertex { position: [start[0] + nx * wall_thickness + edge_thickness - tx * edge_offset, height, start[2] + nz * wall_thickness + edge_thickness - tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    vertices.push(ModelVertex { position: [start[0] + nx * wall_thickness - edge_thickness - tx * edge_offset, height, start[2] + nz * wall_thickness - edge_thickness - tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    
    // Add indices for the back-left vertical edge - ensure correct winding order for visibility
    indices.push(bl_base_idx);
    indices.push(bl_base_idx + 1);
    indices.push(bl_base_idx + 2);
    indices.push(bl_base_idx);
    indices.push(bl_base_idx + 2);
    indices.push(bl_base_idx + 3);
    
    // Back-right vertical edge (for walls with thickness) - make it protrude in all directions
    let br_base_idx = vertices.len() as u16;
    vertices.push(ModelVertex { position: [end[0] + nx * wall_thickness - edge_thickness + tx * edge_offset, 0.0, end[2] + nz * wall_thickness - edge_thickness + tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    vertices.push(ModelVertex { position: [end[0] + nx * wall_thickness + edge_thickness + tx * edge_offset, 0.0, end[2] + nz * wall_thickness + edge_thickness + tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    vertices.push(ModelVertex { position: [end[0] + nx * wall_thickness + edge_thickness + tx * edge_offset, height, end[2] + nz * wall_thickness + edge_thickness + tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    vertices.push(ModelVertex { position: [end[0] + nx * wall_thickness - edge_thickness + tx * edge_offset, height, end[2] + nz * wall_thickness - edge_thickness + tz * edge_offset], color, model_type: 0.0, tex_coords: [0.0, 0.0] });
    
    // 在 create_wall_edge 函数末尾添加缺少的索引
    // Add indices for the back-right vertical edge
    indices.push(br_base_idx);
    indices.push(br_base_idx + 1);
    indices.push(br_base_idx + 2);
    indices.push(br_base_idx);
    indices.push(br_base_idx + 2);
    indices.push(br_base_idx + 3);
    
    Model::new(device, "wall_edge", &vertices, &indices, [0.0, 0.0, 0.0], false, None)
}

// Create the entire parking garage
// 修改函数签名，使用引用而不是所有权
pub fn create_parking_garage(device: &wgpu::Device, dog_texture: &Texture) -> Vec<Model> {
    let mut models = Vec::new();
    
    // Define colors
    let floor_color1 = [0.0, 0.0, 0.0]; // Pure black
    let floor_color2 = [1.0, 1.0, 1.0]; // Pure white
    let ceiling_color1 = [0.5, 0.5, 1.0]; // Light blue
    let ceiling_color2 = [1.0, 1.0, 1.0]; // White
    let wall_color = [1.0, 1.0, 1.0]; // Pure white
    
    // Create floor (black and white checkerboard)
    let floor = create_checkerboard(
        device,
        "floor",
        50.0, // size
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
        50.0, // size
        2.0,  // tile size
        4.0,  // height (ceiling height)
        ceiling_color1,
        ceiling_color2,
        true
    );
    models.push(ceiling);
    
    // Create walls for a rectangular parking garage
    let garage_width = 30.0;
    let garage_length = 40.0;
    let wall_height = 4.0;
    
    // Define wall thickness for edge creation
    let wall_thickness = 0.3;
    
    // Front wall (with a gap for entrance)
    let front_wall1 = create_wall(
        device,
        [-garage_width/2.0, 0.0, -garage_length/2.0],
        [-5.0, 0.0, -garage_length/2.0],
        wall_height,
        wall_color,
    );
    models.push(front_wall1);
    
    // Add black edge to front wall 1
    let front_edge1 = create_wall_edge(
        device,
        [-garage_width/2.0, 0.0, -garage_length/2.0],
        [-5.0, 0.0, -garage_length/2.0],
        wall_height,
        wall_thickness,
    );
    models.push(front_edge1);
    
    let front_wall2 = create_wall(
        device,
        [5.0, 0.0, -garage_length/2.0],
        [garage_width/2.0, 0.0, -garage_length/2.0],
        wall_height,
        wall_color,
    );
    models.push(front_wall2);
    
    // Add black edge to front wall 2
    let front_edge2 = create_wall_edge(
        device,
        [5.0, 0.0, -garage_length/2.0],
        [garage_width/2.0, 0.0, -garage_length/2.0],
        wall_height,
        wall_thickness,
    );
    models.push(front_edge2);
    
    // Back wall
    let back_wall = create_wall(
        device,
        [-garage_width/2.0, 0.0, garage_length/2.0],
        [garage_width/2.0, 0.0, garage_length/2.0],
        wall_height,
        wall_color,
    );
    models.push(back_wall);
    
    // Add black edge to back wall
    let back_edge = create_wall_edge(
        device,
        [-garage_width/2.0, 0.0, garage_length/2.0],
        [garage_width/2.0, 0.0, garage_length/2.0],
        wall_height,
        wall_thickness,
    );
    models.push(back_edge);
    
    // Left wall
    let left_wall = create_wall(
        device,
        [-garage_width/2.0, 0.0, -garage_length/2.0],
        [-garage_width/2.0, 0.0, garage_length/2.0],
        wall_height,
        wall_color,
    );
    models.push(left_wall);
    
    // Add black edge to left wall
    let left_edge = create_wall_edge(
        device,
        [-garage_width/2.0, 0.0, -garage_length/2.0],
        [-garage_width/2.0, 0.0, garage_length/2.0],
        wall_height,
        wall_thickness,
    );
    models.push(left_edge);
    
    // Right wall
    let right_wall = create_wall(
        device,
        [garage_width/2.0, 0.0, -garage_length/2.0],
        [garage_width/2.0, 0.0, garage_length/2.0],
        wall_height,
        wall_color,
    );
    models.push(right_wall);
    
    // Add black edge to right wall
    let right_edge = create_wall_edge(
        device,
        [garage_width/2.0, 0.0, -garage_length/2.0],
        [garage_width/2.0, 0.0, garage_length/2.0],
        wall_height,
        wall_thickness,
    );
    models.push(right_edge);
    
    // Add some interior walls to make it more interesting
    let interior_wall1 = create_wall(
        device,
        [-10.0, 0.0, 0.0],
        [10.0, 0.0, 0.0],
        wall_height,
        wall_color,
    );
    models.push(interior_wall1);
    
    // Add black edge to interior wall 1
    let interior_edge1 = create_wall_edge(
        device,
        [-10.0, 0.0, 0.0],
        [10.0, 0.0, 0.0],
        wall_height,
        wall_thickness,
    );
    models.push(interior_edge1);
    
    let interior_wall2 = create_wall(
        device,
        [0.0, 0.0, 5.0],
        [0.0, 0.0, 15.0],
        wall_height,
        wall_color,
    );
    models.push(interior_wall2);
    
    // Add black edge to interior wall 2
    let interior_edge2 = create_wall_edge(
        device,
        [0.0, 0.0, 5.0],
        [0.0, 0.0, 15.0],
        wall_height,
        wall_thickness,
    );
    models.push(interior_edge2);
    
    models
}