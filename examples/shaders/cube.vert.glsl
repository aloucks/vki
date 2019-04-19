#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec4 a_Color;

out gl_PerVertex {
    vec4 gl_Position;
};

layout(set = 0, binding = 0, std140) uniform Uniforms {
    mat4 u_Projection;
    mat4 u_View;
    mat4 u_Model;
    float u_Time;
};

layout(location = 0) out vec4 v_Color;

void main() {
    v_Color = a_Color;
    gl_Position = u_Projection * u_View * u_Model * vec4(a_Position, 1.0);
}