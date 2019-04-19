#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec3 a_Color;

layout(location = 0) out vec3 v_Color;

layout(set = 0, binding = 0) uniform Uniforms {
    mat4 u_Clip;
    float u_Time;
};

void main() {
    v_Color = (((cos(u_Time) + 1.0) / 3.0) + 0.25) * a_Color;
    gl_Position = u_Clip * vec4(a_Position, 1.0);
}