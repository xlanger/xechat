# XEChat 配置说明

配置文件使用跨平台标准路径（通过 `dirs` crate 解析）：

| 平台 | 配置路径 | 数据路径 |
|------|---------|---------|
| macOS | `~/Library/Application Support/XEChat/config.toml` | `~/Library/Application Support/xechat/lancedb/` |
| Linux | `~/.config/xechat/config.toml` | `~/.local/share/xechat/lancedb/` |
| Windows | `%APPDATA%\XEChat\config.toml` | `%LOCALAPPDATA%\xechat\lancedb\` |

## 完整配置示例

```toml
# ==============================================
# XEChat 主配置
# ==============================================
model = "deepseek-v4-flash"
model_provider = "deepseek"
theme = "system"
language = "system"
timezone = "system"
max_context_tokens = 8192
auto_context_management = true

# ==============================================
# 记忆管线配置
# ==============================================
[memory]
max_memory_results = 5

# ==============================================
# 用户偏好配置
# ==============================================
[preferences]
embed_provider = "default"   # "default"（内置 E5）或 "ollama"

[preferences.ollama]
host = "http://localhost:11434"
embed_model = ""             # 留空则自动检测

# ==============================================
# DeepSeek 模型提供商
# ==============================================
[model_providers.deepseek]
name = "DeepSeek"
api_key = "${DEEPSEEK_API_KEY}"
base_url = "https://api.deepseek.com"
timeout = 120

[model_providers.deepseek.models]
"deepseek-v4-flash" = { max_tokens = 384000, temperature = 0.2, top_p = 0.95, context_window = 131072 }
"deepseek-v4-pro" = { max_tokens = 384000, temperature = 0.1, top_p = 0.9, context_window = 131072 }

# ==============================================
# OpenAI 模型提供商
# ==============================================
[model_providers.openai]
name = "OpenAI"
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com"
timeout = 120

[model_providers.openai.models]
"gpt-4o" = { max_tokens = 16384, temperature = 0.7, top_p = 1.0, context_window = 128000 }

# ==============================================
# Ollama 本地模型
# ==============================================
[model_providers.ollama]
name = "Ollama"
api_key = ""
base_url = "http://localhost:11434"
timeout = 120

# ==============================================
# OpenAI 兼容提供商（中转服务）
# ==============================================
[model_providers.siliconflow]
name = "SiliconFlow"
api_key = "${SILICONFLOW_API_KEY}"
base_url = "https://api.siliconflow.cn/v1"
timeout = 120

[model_providers.siliconflow.models]
"Qwen/Qwen3-235B-A22B" = { max_tokens = 32768, temperature = 0.7, top_p = 0.9, context_window = 131072 }
```

## 配置项说明

### 顶层配置

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `model` | String | `"deepseek-v4-flash"` | 当前使用的模型 |
| `model_provider` | String | `"deepseek"` | 当前提供商（对应 `model_providers` 的 key） |
| `theme` | String | `"system"` | 主题：`"system"` / `"dark"` / `"light"` |
| `language` | String | `"system"` | 语言：`"system"` / `"zh"` / `"en"` |
| `timezone` | String | `"system"` | 时区：IANA 标识符或 `"system"` |
| `max_context_tokens` | Option\<u32\> | `8192` | 最大上下文 token 数 |
| `auto_context_management` | Option\<bool\> | `true` | 自动上下文压缩 |

### 记忆配置

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `memory.max_memory_results` | u32 | `5` | 记忆检索返回的最大结果数 |

### 偏好配置

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `preferences.embed_provider` | String | `"default"` | 嵌入提供商：`"default"`（E5）或 `"ollama"` |
| `preferences.ollama.host` | String | `"http://localhost:11434"` | Ollama 服务地址 |
| `preferences.ollama.embed_model` | String | `""` | Ollama 嵌入模型（留空自动检测） |

### 提供商配置

| 配置项 | 类型 | 说明 |
|--------|------|------|
| `name` | String | 显示名称 |
| `api_key` | String | API 密钥（支持环境变量引用） |
| `base_url` | String | API 基础 URL |
| `timeout` | Option\<u64\> | 请求超时（秒） |
| `models` | Map\<String, ModelConfig\> | 模型列表 |

### 模型配置

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `max_tokens` | u32 | — | 最大输出 token 数 |
| `temperature` | f32 | — | 采样温度（0.0–2.0） |
| `top_p` | f32 | — | 核采样阈值（0.0–1.0） |
| `frequency_penalty` | f32 | `0.0` | 频率惩罚（0.0–2.0） |
| `presence_penalty` | f32 | `0.0` | 存在惩罚（0.0–2.0） |
| `context_window` | u32 | `8192` | 上下文窗口大小 |
| `stop_sequences` | Vec\<String\> | `[]` | 自定义停止序列 |

## 环境变量安全配置

XEChat 支持在配置文件中使用环境变量引用，避免 API Key 直接暴露：

**格式 1：简单引用**
```toml
api_key = "$DEEPSEEK_API_KEY"
```

**格式 2：带花括号引用**
```toml
api_key = "${DEEPSEEK_API_KEY}"
```

**解析规则**：
- 自动解析并替换为实际环境变量值
- 环境变量不存在时保留原样
- 支持 `api_key` 和 `base_url` 字段

**环境变量回退**：

如果配置文件中 `api_key` 为空，XEChat 会尝试读取环境变量 `{PROVIDER_KEY_UPPER}_API_KEY`：

```bash
# DeepSeek
export DEEPSEEK_API_KEY="sk-xxx"

# OpenAI
export OPENAI_API_KEY="sk-xxx"

# 自定义提供商（如 siliconflow）
export SILICONFLOW_API_KEY="sk-xxx"
```

## Provider 路由规则

XEChat 根据提供商标识符自动路由到对应的 API 协议：

| 标识符 | 协议 | 说明 |
|--------|------|------|
| `deepseek` | DeepSeek Chat Completions | SSE 流式 + reasoning_content |
| `openai` | OpenAI Responses API | SSE 具名事件流式 |
| `ollama` | Ollama Chat API | NDJSON 流式 |
| 其他 | OpenAI Compatible | 通用 OpenAI 兼容协议 |

**注意**：非 `deepseek` / `openai` / `ollama` 的提供商自动走 OpenAI Compatible 协议，在模型选择器中会显示"OpenAI Compatible"标识。

## 智能上下文窗口压缩

当对话历史累积到接近模型上下文上限时，XEChat 会自动裁剪最早的消息。

**工作原理**：
- 基于字符数估算 Token 使用量（`chars / 3.5`）
- 当 `auto_context_management = true` 且总 Token 超过 `max_context_tokens` 时自动触发
- 从最早的消息开始移除，优先保留最近的对话上下文
- 至少保留最后 4 条消息，防止上下文断裂

```toml
max_context_tokens = 8192      # 根据模型实际上下文窗口调整
auto_context_management = true # 设为 false 关闭压缩
```

## 数据存储路径

所有路径遵循跨平台官方标准（`dirs` crate 解析）：

| 数据 | macOS | Linux | Windows |
|------|-------|-------|---------|
| 配置文件 | `~/Library/Application Support/XEChat/config.toml` | `~/.config/xechat/config.toml` | `%APPDATA%\XEChat\config.toml` |
| 对话数据 | `~/Library/Application Support/xechat/lancedb/` | `~/.local/share/xechat/lancedb/` | `%LOCALAPPDATA%\xechat\lancedb\` |
| 嵌入模型 | `~/Library/Application Support/xechat/models/` | `~/.local/share/xechat/models/` | `%LOCALAPPDATA%\xechat\models\` |

> 旧版（v1）使用 `~/.xechat/` 路径，XEChat 会自动检测并迁移。
