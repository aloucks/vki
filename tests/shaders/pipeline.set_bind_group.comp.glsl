#version 450

layout (local_size_x = 1, local_size_y = 1) in;

layout(set = 0, binding = 0) uniform UBO {
    mat4 u_DataRead;
};

layout(set = 0, binding = 1) buffer SSB {
    mat4 u_DataWrite;
};

layout(set = 0, binding = 2, rgba32f) uniform imageBuffer u_ImageBuffer;

layout(set = 0, binding = 3) uniform sampler u_Sampler;

layout(set = 0, binding = 4) uniform texture2D u_Texture;

void main() {
    u_DataWrite = u_DataRead;
    vec2 texcoord = vec2(0.0, 0.0);
    vec4 color = texture(sampler2D(u_Texture, u_Sampler), texcoord);
    imageStore(u_ImageBuffer, int(gl_GlobalInvocationID.x), color);
}