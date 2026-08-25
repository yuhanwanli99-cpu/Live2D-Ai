/** Static settings dialog shell; behavior remains in settings-ui.ts. */
import { icon } from '../icons';

export function settingsPanelHtml(): string {
  return `
<div class="settings-box">
  <div class="settings-header">
    <div class="settings-header-title">
      设置中心
      <small>Live2D-Ai PC</small>
    </div>
    <button class="settings-close ui-lift" type="button" title="关闭（Esc）" aria-label="关闭设置中心">${icon('close')}</button>
  </div>

  <div class="settings-layout">
    <nav class="settings-nav">
      <div class="settings-nav-group">配置</div>
      <button class="settings-tab active" type="button" data-tab="llm"><span class="tab-dot"></span>对话 / LLM</button>
      <button class="settings-tab" type="button" data-tab="tts"><span class="tab-dot"></span>语音 / TTS</button>
      <button class="settings-tab" type="button" data-tab="live2d"><span class="tab-dot"></span>形象 / 背景</button>
      <button class="settings-tab" type="button" data-tab="external"><span class="tab-dot"></span>外部接入</button>
      <button class="settings-tab" type="button" data-tab="keys"><span class="tab-dot"></span>密钥</button>
      <button class="settings-tab" type="button" data-tab="persona"><span class="tab-dot"></span>人设 / 提示词</button>
      <button class="settings-tab" type="button" data-tab="system"><span class="tab-dot"></span>系统 / 高级</button>
      <button class="settings-tab" type="button" data-tab="memory"><span class="tab-dot"></span>记忆</button>
      <button class="settings-tab" type="button" data-tab="plugins"><span class="tab-dot"></span>插件</button>
      <div class="settings-nav-status">
        <div class="sv-label">状态简报</div>
        <div class="sv-line"><span class="status-dot" data-nav="llm"></span><span data-navtext="llm">LLM –</span></div>
        <div class="sv-line"><span class="status-dot" data-nav="tts"></span><span data-navtext="tts">TTS –</span></div>
        <div class="sv-line"><span class="status-dot" data-nav="ext"></span><span data-navtext="ext">外部接入 –</span></div>
        <div class="sv-line"><span class="status-dot" data-nav="sys"></span><span data-navtext="sys">系统 –</span></div>
      </div>
    </nav>

    <div class="settings-content">
      <div class="settings-error-bar" data-errorbar></div>

      <section class="settings-tab-panel active" data-panel="llm">
        <div class="settings-section-title">对话 / LLM</div>
        <label class="settings-row"><span class="lbl">LLM 提供商</span><select class="settings-llm"></select></label>
        <label class="settings-row" title="开启后思考模式输出思维链；关闭则直接回答（延迟更低）">
          <span class="lbl">思考模式</span>
          <input class="settings-thinking-toggle" type="checkbox" />
        </label>
        <label class="settings-row" data-thinking-effort-row hidden><span class="lbl">思考强度</span>
          <select class="settings-thinking-effort">
            <option value="low">low（轻度）</option>
            <option value="high">high（增强）</option>
            <option value="max">max（深度）</option>
          </select>
        </label>
        <label class="settings-row" title="仅影响本机界面：是否在聊天气泡上方展示模型的思维链过程">
          <span class="lbl">对话内展示思考</span>
          <input class="settings-show-thinking" type="checkbox" />
        </label>
        <div class="settings-editor-actions">
          <button class="btn ghost settings-check-llm" type="button">全部检查可用性</button>
          <span class="settings-tip">保存后生效；打开面板不会自动探测模型。</span>
        </div>
        <div class="settings-group-title">Provider 参数</div>
        <div class="settings-llm-cards"></div>
      </section>

      <section class="settings-tab-panel" data-panel="tts">
        <div class="settings-section-title">语音 / TTS</div>
        <label class="settings-row"><span class="lbl">TTS 提供商</span><select class="settings-tts"></select></label>
        <label class="settings-row"><span class="lbl">ASR 引擎</span><select class="settings-asr"></select></label>
        <label class="settings-row"><span class="lbl">口型模式</span>
          <select class="settings-viseme-mode">
            <option value="rms">RMS 快速（推荐）</option>
            <option value="off">关闭 viseme（最快）</option>
            <option value="pymouth">Pymouth 精细（慢）</option>
          </select>
        </label>
        <div class="settings-group-title">TTS 参数</div>
        <div class="settings-tip">「测试/试听」仅检查 Key 是否已配置，不执行真实合成。</div>
        <div class="settings-tts-cards"></div>

        <div class="settings-group-title">声音克隆库</div>
        <div class="settings-editor-actions">
          <button class="btn ghost voice-upload-btn" type="button">上传参考音频</button>
          <input class="voice-upload-input" type="file" accept="audio/*,.wav,.mp3,.m4a,.flac,.ogg" hidden />
          <span class="settings-tip">支持 wav / mp3 / m4a / flac / ogg</span>
        </div>
        <div class="settings-status-inline voice-status"></div>
        <div class="voice-list"></div>
      </section>

      <section class="settings-tab-panel" data-panel="live2d">
        <div class="settings-section-title">形象 / Live2D</div>
        <div class="settings-section-hint">选择当前使用的 Live2D 模型；扫描可导入本地模型。</div>
        <div class="settings-editor-actions">
          <button class="btn ghost settings-scan-models" type="button">扫描并导入本地模型</button>
          <button class="btn ghost settings-model-upload-btn" type="button">上传 Live2D 模型 (.zip)</button>
          <input class="settings-model-upload-input" type="file" accept=".zip" hidden />
        </div>
        <div class="settings-status-inline" data-modeluploadstatus></div>
        <div class="settings-status-inline" data-modelstatus></div>
        <div class="settings-model-list" data-modellist></div>

        <div class="settings-section-title">角色背景图</div>
        <div class="settings-section-hint">
          上传一张本地图片作为角色背景（铺在 Live2D 画布之下，立即生效）；选「无背景」回到透明桌宠形态。
        </div>
        <div data-bgsection></div>
      </section>

      <section class="settings-tab-panel" data-panel="external">
        <div class="settings-section-title">外部接入（仅电脑端）</div>
        <div class="settings-section-hint">
          开启后，外部程序可以把一段文字交给角色，走的是与聊天框输入**完全相同**的内部链路
          （同一人设、同一记忆、同一 TTS 与口型）。默认关闭，且默认只允许本机调用；随时可以「立即断开」。
        </div>
        <div data-externalsection></div>
      </section>

      <section class="settings-tab-panel" data-panel="keys">
        <div class="settings-section-title">密钥（写入 .env）</div>
        <div class="settings-section-hint">留空表示不修改该 Key；「清除」会显式删除已保存的 Key（置 null 发送）。</div>
        <div class="settings-key-cards"></div>
      </section>

      <section class="settings-tab-panel" data-panel="persona">
        <div class="settings-section-title">人设 / 提示词</div>
        <div class="settings-section-hint">
          人设只承载提示词本身；模型 / 温度 / top_p / 思考等参数在「LLM 设置」页调整。
        </div>
        <label class="settings-row"><span class="lbl">System Prompt</span>
          <textarea class="settings-persona-prompt" rows="14" spellcheck="false"></textarea>
        </label>
        <div class="settings-editor-actions">
          <button class="btn danger settings-persona-clear-all" type="button">清除 System Prompt</button>
        </div>
      </section>

      <section class="settings-tab-panel" data-panel="system">
        <div class="settings-section-title">系统 / 高级</div>
        <div class="settings-edit-stack">
          <div class="settings-row"><span class="lbl">角色名（conf.yaml）</span><input class="settings-character-name" type="text" /></div>
          <div class="settings-row" data-portrow><span class="lbl">端口</span><input class="settings-port" type="number" min="1" max="65535" data-mono /><div class="field-error-msg"></div></div>
          <div class="settings-row"><span class="lbl">Live2D FPS</span><input class="settings-fps" type="number" min="1" max="240" /></div>
          <div class="settings-row"><span class="lbl">日志级别</span>
            <select class="settings-log-level">
              <option value="INFO">INFO（推荐）</option>
              <option value="DEBUG">DEBUG（调试）</option>
              <option value="WARNING">WARNING（安静）</option>
            </select>
          </div>
          <div class="settings-row" title="仅限本机调试：开放导演分析 / 动作触发接口（localhost-only，默认关闭）。">
            <span class="lbl">开发者模式</span>
            <input class="settings-dev-mode" type="checkbox" />
          </div>
          <div class="settings-editor-actions settings-dev-console-entry" hidden>
            <button class="btn ghost settings-open-dev-console" type="button">打开开发者控制台</button>
            <span class="settings-tip">本机导演分析 / 动作触发测试台</span>
          </div>
          <div class="settings-section-title">版本</div>
          <div class="settings-version-block" data-versionblock>
            <div class="settings-row"><span class="lbl">产品版本</span><code class="settings-version-value" data-versionfield="product_version">未知</code></div>
            <div class="settings-row"><span class="lbl">后端版本</span><code class="settings-version-value" data-versionfield="backend_version">未知</code></div>
            <div class="settings-row"><span class="lbl">前端版本</span><code class="settings-version-value" data-versionfield="frontend_version">未知</code></div>
            <div class="settings-row"><span class="lbl">Commit</span><code class="settings-version-value" data-versionfield="commit">unknown</code></div>
            <div class="settings-editor-actions">
              <button class="btn ghost btn-xs settings-copy-version" type="button">复制版本信息</button>
              <span class="settings-tip">点击复制，便于反馈问题。</span>
            </div>
          </div>
          <div class="settings-row"><span class="lbl">前端版本信息</span><span class="settings-frontend-version">未知版本</span></div>
        </div>
        <div class="settings-tip">端口与日志在保存并重启后端后生效。</div>
        <div class="settings-group-title">配置导入 / 导出</div>
        <div class="settings-editor-actions">
          <button class="btn ghost settings-config-export-btn" type="button">导出配置 JSON</button>
          <button class="btn ghost settings-config-import-btn" type="button">导入配置 JSON</button>
          <input class="settings-config-import-input" type="file" accept="application/json,.json" hidden />
        </div>
        <div class="settings-status-inline" data-configtransferstatus></div>
        <div class="settings-group-title">高级：conf.yaml 设定树</div>
        <label class="settings-row"><span class="lbl">原始 YAML</span>
          <textarea class="settings-config-tree" rows="14" spellcheck="false"></textarea>
        </label>
        <div class="settings-editor-actions">
          <button class="btn settings-save-tree" type="button">保存设定树</button>
          <span class="settings-tip">保存前会做 schema 校验，非法则不会写盘。</span>
        </div>
      </section>

      <section class="settings-tab-panel" data-panel="memory">
        <div class="settings-section-title">记忆 / 知识库</div>
        <div class="settings-section-hint">「长期记忆」插件在对话中自动记录事实（称呼、偏好、约定）；知识库文档按相关性检索注入并标注来源；联网查询为真实搜索（无 Key 走 DuckDuckGo，配 TAVILY_API_KEY 走 Tavily）。改动即时生效。</div>
        <div class="settings-status-inline memory-status"></div>
        <div class="settings-group-title">长期记忆事实</div>
        <div class="settings-editor-actions memory-add-row">
          <input class="settings-input memory-fact-input" type="text" placeholder="手动记一条，例如：喜欢被叫小白" aria-label="新增记忆" />
          <button class="btn primary btn-xs memory-add" type="button">添加</button>
          <button class="btn ghost btn-xs memory-refresh" type="button">刷新</button>
          <button class="btn ghost btn-xs memory-clear" type="button">清空全部</button>
        </div>
        <div class="memory-list"></div>
        <div class="settings-group-title">知识库文档（.md / .txt）</div>
        <div class="settings-editor-actions kb-add-row">
          <input class="settings-input kb-name-input" type="text" placeholder="文档名，例如：世界观设定" aria-label="知识文档名" />
          <button class="btn primary btn-xs kb-add" type="button">添加文档</button>
        </div>
        <textarea class="settings-config-tree kb-content-input" rows="4" spellcheck="false" placeholder="文档内容（Markdown 纯文本），对话中相关内容会被检索注入" aria-label="知识文档内容"></textarea>
        <div class="settings-status-inline kb-status"></div>
        <div class="kb-list"></div>
        <div class="settings-group-title">联网查询试一试</div>
        <div class="settings-editor-actions search-row">
          <input class="settings-input search-query-input" type="text" placeholder="输入关键词，验证联网检索是否可用" aria-label="联网查询关键词" />
          <button class="btn primary btn-xs search-run" type="button">搜索</button>
        </div>
        <div class="search-results"></div>
      </section>
      <section class="settings-tab-panel" data-panel="plugins">
        <div class="settings-section-title">插件管理</div>
        <div class="settings-section-hint">原生插件随应用内置，外部插件由用户安装；启用 / 停用即时生效，部分插件需重启后端。</div>
        <div class="settings-status-inline plugin-status"></div>
        <div class="plugin-list"></div>
      </section>
    </div>
  </div>

  <div class="settings-footer">
    <div class="settings-status" data-status></div>
    <div class="settings-footer-actions">
      <button class="btn ghost settings-apply" type="button">保存并应用</button>
      <button class="btn primary settings-save" type="button">保存全部</button>
      <button class="btn settings-close2" type="button">关闭</button>
    </div>
  </div>
</div>
  `;
}
