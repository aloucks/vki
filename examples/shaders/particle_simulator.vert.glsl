#version 450

layout(location = 0) in vec4 vert;

layout(location = 0) out float intensity;

layout(set = 0, binding = 0, std140) uniform Uniforms {
    mat4 mvp;
};

void main() {
    intensity = vert.w;
    gl_PointSize = 1.0;
    gl_Position = mvp * vec4(vert.xyz, 1.0);
}