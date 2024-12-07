#version 460

layout (location = 0) in vec4 out_color;
layout (location = 0) out vec4 frag_color;

void main() {
    frag_color = out_color;
}
