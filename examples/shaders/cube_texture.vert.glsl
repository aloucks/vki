#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec2 a_Texcoord;

layout(set = 0, binding = 0, std140) uniform Uniforms {
    mat4 u_Projection;
    mat4 u_View;
    mat4 u_Model;
    float u_Time;
};

layout(location = 0) out vec2 v_Texcoord;

void main() {
    v_Texcoord = a_Texcoord;
    gl_Position = u_Projection * u_View * u_Model * vec4(a_Position, 1.0);
}