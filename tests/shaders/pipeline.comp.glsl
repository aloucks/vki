#version 450

layout (local_size_x = 1, local_size_y = 1) in;

layout(set = 0, binding = 0) uniform UBO {
    mat4 u_data;
};

layout(set = 0, binding = 1) buffer SSB {
    mat4 s_data;
};

void main() {
    s_data = u_data;
}