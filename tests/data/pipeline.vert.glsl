#version 450

layout(location = 0) in vec3 a_Position;
layout(location = 1) in vec3 a_Normal;

layout(location = 0) out vec3 v_Normal;

layout(set = 0, binding = 0) uniform UBO {
    mat4 u_data;
};

void main() {
    v_Normal = a_Normal;
    gl_Position = u_data * vec4(a_Position, 1.0);
}