#version 450

layout(location = 0) in float intensity;

layout(location = 0) out vec4 color;

void main() {
    color = mix(vec4(0.0, 0.2, 0.75, 1.0), vec4(0.2, 0.05, 0.0, 1.0), intensity);
}
