#version 460

layout(location = 0) in vec3 color_in;

layout(location = 0) out vec4 color;

void main() {
	color = vec4(color_in, 1.0);
}
