#version 450

layout(location = 0) out vec4 outColor;

layout(location = 0) in vec4 v_Color;

layout(set = 0, binding = 0, std140) uniform Uniforms {
    mat4 u_Projection;
    mat4 u_View;
    mat4 u_Model;
    float u_Time;
};

void main() {
    outColor = (0.6 + cos(u_Time * 2.0) / 2.5) * v_Color;
}