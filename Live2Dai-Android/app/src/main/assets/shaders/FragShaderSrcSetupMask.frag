// CubismJavaFramework SetupMask Fragment Shader (GLES 2.0)
precision mediump float;
varying vec4 v_myPos;
varying vec2 v_texCoord;
uniform sampler2D s_texture0;
uniform vec4 u_channelFlag;
uniform vec4 u_baseColor;
uniform vec4 u_color;
uniform vec4 u_multiplyColor;
uniform vec4 u_screenColor;
void main() {
    // 检查当前像素是否在 layout bounds 内
    float isInside = step(u_baseColor.x, v_myPos.x/v_myPos.w)
                   * step(u_baseColor.y, v_myPos.y/v_myPos.w)
                   * step(v_myPos.x/v_myPos.w, u_baseColor.z)
                   * step(v_myPos.y/v_myPos.w, u_baseColor.w);
    // 采样纹理并应用 multiply/screen color
    vec4 texColor = texture2D(s_texture0, v_texCoord);
    texColor.rgb = texColor.rgb * u_multiplyColor.rgb;
    texColor.rgb = texColor.rgb + u_screenColor.rgb - (texColor.rgb * u_screenColor.rgb);
    // 输出通道颜色 * 纹理alpha * 边界测试
    gl_FragColor = u_channelFlag * texColor.a * isInside;
}
