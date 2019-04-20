#version 450

layout(location = 0) in vec2 v_Texcoord;

layout(location = 0) out vec4 outColor;

layout(set = 0, binding = 0, std140) uniform Uniforms {
    mat4 u_Projection;
    mat4 u_View;
    mat4 u_Model;
    float u_Time;
};
layout(set = 0, binding = 1) uniform sampler u_Sampler;
layout(set = 0, binding = 2) uniform texture2D u_Texture;

void main() {
    outColor = texture(sampler2D(u_Texture, u_Sampler), v_Texcoord);
}