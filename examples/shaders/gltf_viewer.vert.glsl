#version 450

// Adapted from: https://github.com/KhronosGroup/glTF-WebGL-PBR/blob/a94fcda490fa81f1a5d6f30b139fc8ea94676f9d/shaders/pbr-vert.glsl

layout(location = 0) in vec4 a_Position;
layout(location = 1) in vec4 a_Normal;
layout(location = 2) in vec4 a_Tangent;
layout(location = 3) in vec2 a_TexCoord0;
layout(location = 4) in vec2 a_TexCoord1;
layout(location = 5) in vec4 a_Color;
layout(location = 6) in uvec4 a_Joint;
layout(location = 7) in vec4 a_Weight;
layout(location = 8) in vec3 a_MorphPosition0;
layout(location = 9) in vec3 a_MorphPosition1;
layout(location = 10) in vec3 a_MorphNormal0;
layout(location = 11) in vec3 a_MorphNormal1;
layout(location = 12) in vec3 a_MorphTangent0;
layout(location = 13) in vec3 a_MorphTangent1;

layout(location = 0) out vec3 v_Position;
layout(location = 1) out vec2 v_UV[2];
layout(location = 3) out mat3 v_TBN;
layout(location = 6) out vec3 v_Normal;
layout(location = 7) out vec4 v_Color;

layout(push_constant) uniform PrimitiveConstants {
    bool HAS_POSITIONS;
    bool HAS_NORMALS;
    bool HAS_TANGENTS;
    bool HAS_COLORS;
    bool HAS_TEXCOORDS[2];
    bool HAS_WEIGHTS;
    bool HAS_JOINTS;
    bool HAS_MORPH_POSITIONS[2];
    bool HAS_MORPH_NORMALS[2];
    bool HAS_MORPH_TANGENTS[2];
};

layout(set = 0, binding = 0, std140) uniform CameraAndLightSettings {
    vec4 u_ScaleDiffBaseMR;
    vec4 u_ScaleFGDSpec;
    vec4 u_ScaleIBLAmbient;

    vec3 u_CameraPosition;
    vec3 u_LightDirection;
    vec3 u_LightColor;

    float u_SpecularEnvMipCount;
};

// NOTE: set 1 is only used in the fragment shader

layout(set = 2, binding = 0, std140) uniform MeshSettings {
    mat4 u_MVPMatrix;
    mat4 u_ModelMatrix;
    mat4 u_NormalMatrix;

    // We use vec4 instead of float[4] so that we don't have to pad
    // each element of the array.
    vec4 u_MorphWeights;
};

layout(set = 2, binding = 1, std140) uniform SkinSettings {
    mat4 u_JointMatrix[128];
};

void outTexCoords() {
    if (HAS_TEXCOORDS[0]) {
        v_UV[0] = a_TexCoord0;
    } else {
        v_UV[0] = vec2(0.0, 0.0);
    }
    if (HAS_TEXCOORDS[1]) {
        v_UV[1] = a_TexCoord1;
    } else {
        v_UV[1] = vec2(0.0, 0.0);
    }
}

#define APPLY_MORPH_WEIGHT(a_MorphSemanticN, N) \
 (u_MorphWeights[N] * a_MorphSemanticN ##N);

void outNormalsAndTangents(mat4 skinMatrix) {
    if (HAS_NORMALS) {
        vec3 normal = a_Normal.xyz;
        if (HAS_MORPH_NORMALS[0]) {
            normal += APPLY_MORPH_WEIGHT(a_MorphNormal, 0);
        }
        if (HAS_MORPH_NORMALS[1]) {
            normal += APPLY_MORPH_WEIGHT(a_MorphNormal, 1);
        }
        if (HAS_TANGENTS) {
            vec3 tangent = a_Tangent.xyz;
            if (HAS_MORPH_TANGENTS[0]) {
                tangent += APPLY_MORPH_WEIGHT(a_MorphTangent, 0);
            }
            if (HAS_MORPH_TANGENTS[1]) {
                tangent += APPLY_MORPH_WEIGHT(a_MorphTangent, 1);
            }
            vec3 normalW = normalize(vec3(u_NormalMatrix * skinMatrix * vec4(normal, 0.0)));
            vec3 tangentW = normalize(vec3(u_ModelMatrix * skinMatrix * vec4(tangent, 0.0)));
            vec3 bitangentW = cross(normalW, tangentW) * a_Tangent.w;
            v_TBN = mat3(tangentW, bitangentW, normalW);
        } else {
            v_Normal = normalize(vec3(u_ModelMatrix * skinMatrix * vec4(normal, 0.0)));
        }
    }
}

void outColors() {
    if (HAS_COLORS) {
        v_Color = a_Color;
    }
}

mat4 getSkinMatrix() {
    if (HAS_JOINTS) {
        mat4 skinMatrix =
            a_Weight.x * u_JointMatrix[a_Joint.x] +
            a_Weight.y * u_JointMatrix[a_Joint.y] +
            a_Weight.z * u_JointMatrix[a_Joint.z] +
            a_Weight.w * u_JointMatrix[a_Joint.w];
        return skinMatrix;
    } else {
        return mat4(1.0);
    }
}

void outPositions(mat4 skinMatrix) {
    vec4 pos = u_ModelMatrix * a_Position;
    //v_Position = vec3(pos.xyz) / pos.w;

    vec3 position = a_Position.xyz;

    if (HAS_MORPH_POSITIONS[0]) {
        position += APPLY_MORPH_WEIGHT(a_MorphPosition, 0);
    }
    if (HAS_MORPH_POSITIONS[1]) {
        position += APPLY_MORPH_WEIGHT(a_MorphPosition, 1);
    }
    // needs w for proper perspective correction
    vec4 skinnedPosition = skinMatrix * vec4(position, a_Position.w);
    v_Position = skinnedPosition.xyz / pos.w;
    gl_Position = u_MVPMatrix * skinnedPosition;
}

void main() {
    mat4 skinMatrix = getSkinMatrix();
    outColors();
    outTexCoords();
    outNormalsAndTangents(skinMatrix);
    outPositions(skinMatrix);
}