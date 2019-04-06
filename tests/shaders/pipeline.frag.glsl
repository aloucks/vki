#version 450

layout(location = 0) in vec3 v_Normal;

layout(location = 0) out vec4 fragColor;

void main() {
    // use v_Normal so that it isn't optimized out
    fragColor = vec4(v_Normal, 1.0) * vec4(1.0, 1.0, 1.0, 1.0);
}