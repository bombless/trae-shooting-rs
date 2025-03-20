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
    @location(2) tex_coords: vec2<f32>,
    @location(3) model_type: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) model_type: f32,
};

// 添加一个新的 uniform 缓冲区用于墙体颜色
struct WallColor {
    color: vec3<f32>,
    _padding: f32,
};

@group(1) @binding(0)
var<uniform> wall_color: WallColor;

// 将纹理绑定移到条件判断外部
@group(2) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(2) @binding(1)
var s_diffuse: sampler;

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(model.position, 1.0);
    out.color = model.color;
    out.tex_coords = model.tex_coords;
    out.model_type = model.model_type;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 修改着色器逻辑，避免条件性纹理采样
    var color = in.color;
    var alpha = 1.0;
    
    // 对所有片段都进行纹理采样，但只在需要时使用结果
    let tex_color = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    
    if (in.model_type > 0.5) {
        color = wall_color.color;
        if (tex_color.a > 0.1) {
            color = color * tex_color.rgb;
            alpha = tex_color.a;
        }
    }
    
    return vec4<f32>(color, alpha);
}
