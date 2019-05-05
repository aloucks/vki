#version 450

layout (local_size_x = 1) in;

layout(push_constant) uniform PushConstants {
    uint one;
    uint two;
};

layout(set = 0, binding = 0) buffer outBuffer {
    uint out_data[];
};

void main() {
    out_data[0] = one;
    out_data[1] = two;
}