#version 460

layout(location = 0) in vec3 color;
layout(location = 1) in vec3 position;

layout(location = 0) out vec3 color_out;

void main() {
	gl_Position = vec4(position, 1.0);
	color_out = color;
}
