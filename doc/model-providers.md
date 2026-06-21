# Model Providers

Peregrine is **model-agnostic** — you can run the same security analysis and
auditing workflows against any supported LLM backend. Switching providers is a
config change; your prompts, tools, and agent sessions work identically
regardless of which model is powering inference.

---

## Supported Providers

| Provider | ID | Default Model | Auth |
|---|---|---|---|
| **OpenAI** | `openai` | `gpt-5.4` | API key or browser login |
| **Anthropic** | `anthropic` | `claude-sonnet-4-6` | API key |
| **Ollama** | `ollama` | `gpt-oss:20b` | None (local) |
| **Amazon Bedrock** | `amazon-bedrock` | `openai.gpt-5.4` | AWS credentials |
| **LM Studio** | `lmstudio` | `openai/gpt-oss-20b` | None (local) |
| **Custom** | user-defined | user-defined | Varies |

You can also add any **OpenAI Responses-compatible endpoint** as a custom
provider (see [Defining Custom Providers](#defining-custom-providers)).

---

## Selecting a Provider

Set the active provider and model in your `config.toml`:

```toml
model_provider = "anthropic"
model = "claude-sonnet-4-6"
```

You can also switch providers at runtime through the TUI or desktop UI.

---

## Provider Setup

### OpenAI

```bash
export OPENAI_API_KEY="sk-..."
```

```toml
model_provider = "openai"
model = "gpt-5.4"
```

To use a proxy or custom OpenAI-compatible base URL:

```toml
openai_base_url = "https://your-proxy.example.com/v1"
```

### Anthropic

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

```toml
model_provider = "anthropic"
model = "claude-sonnet-4-6"
```

### Ollama (Local)

Start Ollama and pull a model, then configure Peregrine:

```bash
ollama serve
ollama pull gpt-oss:20b
```

```toml
model_provider = "ollama"
model = "gpt-oss:20b"
```

No API key is required. Ollama must be running locally on its default port.

### LM Studio (Local)

Start LM Studio with its local server enabled, then configure Peregrine:

```toml
model_provider = "lmstudio"
model = "openai/gpt-oss-20b"
```

No API key is required.

### Amazon Bedrock

```toml
model_provider = "amazon-bedrock"
model = "openai.gpt-5.4"
```

Authentication uses standard AWS credential resolution (`AWS_ACCESS_KEY_ID` /
`AWS_SECRET_ACCESS_KEY`, `~/.aws/credentials`, IAM roles, etc.).

---

## Defining Custom Providers

Any OpenAI Responses-compatible endpoint can be registered under
`[model_providers.<id>]`. The built-in IDs (`openai`, `ollama`, `lmstudio`,
`amazon-bedrock`) are reserved.

### Basic Example

```bash
export CORP_LLM_API_KEY="..."
```

```toml
model_provider = "my-corp-llm"

[model_providers.my-corp-llm]
name = "Corporate LLM"
base_url = "https://llm.internal.corp.com/v1"
env_key = "CORP_LLM_API_KEY"
env_key_instructions = "Set CORP_LLM_API_KEY to your corporate API key."
```

### Available Fields

| Field | Type | Description |
|---|---|---|
| `name` | string | Display name shown in the UI |
| `base_url` | string | Base URL for the API endpoint |
| `env_key` | string | Environment variable containing the API key |
| `env_key_instructions` | string | Help text shown when the key is missing |
| `http_headers` | table | Static HTTP headers (e.g. `{ "x-custom" = "value" }`) |
| `env_http_headers` | table | Headers sourced from env vars (e.g. `{ "x-api-key" = "MY_KEY_VAR" }`) |
| `query_params` | table | Query parameters appended to every request |
| `experimental_bearer_token` | string | Static bearer token for auth |
| `request_max_retries` | int | Max retries for failed requests |
| `stream_max_retries` | int | Max retries for broken streaming connections |
| `stream_idle_timeout_ms` | int | Timeout (ms) before an idle stream is dropped |

### External Command Auth

For providers that need dynamic token refresh (e.g. OAuth or short-lived
tokens), you can configure an external command that Peregrine calls to obtain
a fresh token:

```toml
[model_providers.my-provider]
name = "Token-Auth Provider"
base_url = "https://api.example.com/v1"

[model_providers.my-provider.auth]
command = "/usr/local/bin/get-token"
args = ["--scope", "llm"]
timeout_ms = 5000
refresh_interval_ms = 300000
```

### AWS Auth (Bedrock-style)

```toml
[model_providers.my-bedrock]
name = "My Bedrock"
base_url = "https://bedrock-runtime.us-east-1.amazonaws.com"

[model_providers.my-bedrock.aws]
profile = "my-aws-profile"
region = "us-east-1"
```

---

## Model Parameters

These settings can be added to `config.toml` globally or overridden per-thread
in the TUI/desktop UI.

| Setting | Config Key | Values / Description |
|---|---|---|
| **Model** | `model` | Model slug (e.g. `gpt-5.4`, `claude-sonnet-4-6`) |
| **Provider** | `model_provider` | Provider ID from the table above or a custom ID |
| **Reasoning Effort** | `model_reasoning_effort` | `low`, `medium`, `high` |
| **Reasoning Summary** | `model_reasoning_summary` | Controls reasoning summary output |
| **Verbosity** | `model_verbosity` | `low`, `medium`, `high` |
| **Service Tier** | `service_tier` | `default`, `priority`, `flex` |
| **Context Window** | `model_context_window` | Max tokens in the context window |
| **Auto-Compact Limit** | `model_auto_compact_token_limit` | Token threshold that triggers history compaction |
| **Personality** | `personality` | Model personality mode |
| **Model Catalog** | `model_catalog_json` | Path to a JSON file defining available models |
| **Model Instructions** | `model_instructions_file` | Path to a file overriding built-in model instructions |

### Example

```toml
model_provider = "openai"
model = "gpt-5.4"
model_reasoning_effort = "high"
service_tier = "priority"
model_context_window = 200000
```

---

## Environment Variables Reference

| Variable | Provider | Purpose |
|---|---|---|
| `OPENAI_API_KEY` | OpenAI | API key for OpenAI models |
| `ANTHROPIC_API_KEY` | Anthropic | API key for Claude models |
| `AWS_ACCESS_KEY_ID` | Amazon Bedrock | AWS access key |
| `AWS_SECRET_ACCESS_KEY` | Amazon Bedrock | AWS secret key |
| `AWS_PROFILE` | Amazon Bedrock | AWS profile name (alternative to key pair) |
| Custom (`env_key`) | Custom providers | API key for any custom provider |
