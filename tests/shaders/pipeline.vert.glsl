#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec3 a_Color;

layout(location = 0) out vec3 v_Color;

layout(set = 0, binding = 0) uniform MVP {
    mat4 u_MVP;
};

void main() {
    v_Color = a_Color;
    gl_Position = u_MVP * vec4(a_Position, 1.0);
}