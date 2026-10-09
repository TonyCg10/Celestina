#version 440

// The dark reading mode: each pixel of the pages is inverted and its hue
// turned half a circle, so white paper reads dark and the ink light while
// a colour keeps its hue. `amount` blends from the page as drawn (0) to the
// reading mode (1). The layer is premultiplied, so the colour is taken out of
// its alpha first and the gaps between pages stay clear.

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float amount;
};

layout(binding = 1) uniform sampler2D source;

void main()
{
    vec4 color = texture(source, qt_TexCoord0);
    vec3 straight = color.a > 0.0 ? color.rgb / color.a : vec3(0.0);
    vec3 inverted = vec3(1.0) - straight;
    // The luminance-preserving hue rotation by 180 degrees.
    vec3 turned = vec3(
        dot(inverted, vec3(-0.574, 1.430, 0.144)),
        dot(inverted, vec3(0.426, 0.430, 0.144)),
        dot(inverted, vec3(0.426, 1.430, -0.856)));
    vec3 shown = mix(straight, clamp(turned, 0.0, 1.0), amount);
    fragColor = vec4(shown * color.a, color.a) * qt_Opacity;
}
