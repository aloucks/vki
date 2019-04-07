#version 450

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inColor;

layout(location = 0) out vec3 outColor;

layout(set = 0, binding = 0) uniform MVP {
    mat4 clip;
};

void main() {
    outColor = inColor;
    gl_Position = clip * vec4(inPosition, 1.0);
}