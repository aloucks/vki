#version 450

layout (local_size_x = 512) in;

layout (set = 0, binding = 0, rgba32f) uniform imageBuffer velocity_buffer;
layout (set = 0, binding = 1, rgba32f) uniform imageBuffer position_buffer;
layout (set = 0, binding = 2, std140) uniform attractor_block {
    vec4 attractor[64]; // xyz = position, w = mass
    float dt;
};

void main(void)
{
    vec4 vel = imageLoad(velocity_buffer, int(gl_GlobalInvocationID.x));
    vec4 pos = imageLoad(position_buffer, int(gl_GlobalInvocationID.x));

    int i;

    pos.xyz += vel.xyz * dt;
    pos.w -= 0.0001 * dt;

    for (i = 0; i < 4; i++)
    {
        vec3 dist = (attractor[i].xyz - pos.xyz);
        vel.xyz += dt * dt * attractor[i].w * normalize(dist) / (dot(dist, dist) + 10.0);
    }

    if (pos.w <= 0.0)
    {
        pos.xyz = -pos.xyz * 0.01;
        vel.xyz *= 0.01;
        pos.w += 1.0f;
    }

    imageStore(position_buffer, int(gl_GlobalInvocationID.x), pos);
    imageStore(velocity_buffer, int(gl_GlobalInvocationID.x), vel);
}