#version 450

layout(location = 0) in vec2 position;
layout(push_constant) uniform pushConstants {
    vec2 offset;
} push_const;

layout(location = 0) out vec2 tex_coords;

void main() {
    gl_Position = vec4(position + push_const.offset, 0.0, 1.0);
    tex_coords = position;
}