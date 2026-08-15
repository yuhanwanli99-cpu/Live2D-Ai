// CubismJavaFramework Masked Vertex Shader (GLES 2.0)
uniform mat4 u_matrix;
uniform mat4 u_clipMatrix;
attribute vec2 a_position;
attribute vec2 a_texCoord;
varying vec2 v_texCoord;
varying vec4 v_clipPos;
void main() {
    gl_Position = u_matrix * vec4(a_position, 0.0, 1.0);
    v_clipPos = u_clipMatrix * vec4(a_position, 0.0, 1.0);
    v_texCoord = a_texCoord;
    v_texCoord.y = 1.0 - v_texCoord.y;
}
