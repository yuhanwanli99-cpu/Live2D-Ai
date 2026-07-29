// CubismJavaFramework Normal Vertex Shader (GLES 2.0)
uniform mat4 u_matrix;
attribute vec2 a_position;
attribute vec2 a_texCoord;
varying vec2 v_texCoord;
void main() {
    gl_Position = u_matrix * vec4(a_position, 0.0, 1.0);
    v_texCoord = a_texCoord;
    v_texCoord.y = 1.0 - v_texCoord.y;
}
