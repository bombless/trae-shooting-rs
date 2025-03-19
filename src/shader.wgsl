// Vertex shader

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view_position: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) model_type: f32, // 添加模型类型字段
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) model_type: f32, // 添加模型类型字段
};

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.color = model.color;
    out.clip_position = camera.view_proj * vec4<f32>(model.position, 1.0);
    out.model_type = model.model_type; // 传递模型类型
    return out;
}

// Fragment shader

// 添加一个新的 uniform 缓冲区用于墙体颜色
struct WallColor {
    color: vec3<f32>,
    _padding: f32,
};

@group(1) @binding(0)
var<uniform> wall_color: WallColor;

// 在片段着色器中使用墙体颜色
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 如果是墙体模型，使用 uniform 中的颜色
    if (in.model_type == 1.0) {
        return vec4<f32>(wall_color.color, 1.0);
    }
    // 否则使用顶点颜色
    return vec4<f32>(in.color, 1.0);
}