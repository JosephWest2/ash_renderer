#version 460

layout (location = 0) in vec3 position;
layout (location = 1) in vec4 color;
layout (set = 0, binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
} ubo;

layout (location = 0) out vec4 out_color;
void main() {
    out_color = color;
    gl_Position =  ubo.proj * ubo.view * ubo.model * vec4(position, 1);
}
