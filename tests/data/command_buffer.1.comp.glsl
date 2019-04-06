#version 450

layout (local_size_x = 4) in;

layout(set = 0, binding = 0) buffer inBuffer {
    float in_data[];
};

layout(set = 0, binding = 1) buffer outBuffer {
    float out_data[];
};

void main() {
    out_data[uint(gl_GlobalInvocationID.x)] = in_data[uint(gl_GlobalInvocationID.x)];
}