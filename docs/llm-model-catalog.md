# LLM model catalog

The picker and Rust defaults share `src-tauri/resources/llm-models.json`, checked on 2026-10-05. The first suggestion is the default for a new provider selection. Existing saved models remain pinned; no saved configuration is migrated automatically. Fetching Gemini/Ollama models updates suggestions without changing the chosen ID. The Model ID field accepts other supported snapshots or deployment IDs.

| Provider | New-selection default | Official catalog |
| --- | --- | --- |
| Gemini | `gemini-3.8-flash` | [Gemini models](https://ai.google.dev/gemini-api/docs/models) |
| OpenAI | `gpt-6.1-sol` | [OpenAI models](https://developers.openai.com/api/docs/models) |
| Anthropic | `claude-sonnet-5-5` | [Claude models](https://platform.claude.com/docs/en/models/overview) |
| Ollama | `qwen3.5:4b` | [Ollama library](https://ollama.com/library), [Qwen3.5 tags](https://ollama.com/library/qwen3.5), [Gemma4 tags](https://ollama.com/library/gemma4) |
| DeepSeek | `deepseek-flash` | [Current model IDs](https://api-docs.deepseek.com/api/list-models/), [Chat Completions](https://api-docs.deepseek.com/api/create-chat-completion/) |
| Groq | `openai/gpt-oss-120b` | [Supported models](https://console.groq.com/docs/models) |
| Custom | User supplied | The configured endpoint determines available IDs. |

Suggestions focus on text generation; audio, image, embedding, moderation and special-purpose models are omitted. Gemini Pro suggestions retain their preview ID; Groq Qwen3.8 is a preview and its Llama suggestions can require enterprise access. Ollama tags require local installation and suitable memory; fetching installed tags is more authoritative than the fallback suggestions. No models are downloaded automatically. Availability and billing still depend on the provider account.

GPT-6 text-only requests keep the existing Chat Completions protocol. Temperature is omitted for OpenAI GPT-5/GPT-6 and o-series reasoning families to avoid unsupported sampling parameters under their default reasoning settings. Legacy non-reasoning OpenAI sampling stays unchanged. Anthropic responses extract text blocks while skipping thinking blocks. See [Using GPT-6 compatibility guidance](https://developers.openai.com/api/docs/guides/latest-model). This is a catalog/default update, without a tools or Responses API migration.

The Gemini refresh key now travels in `x-goog-api-key`, not in the URL. Provider switching clears the previous provider's unsaved key and endpoint; loading saved configuration preserves both. It does not automatically send the previous provider's key to Gemini. Tests cover shared defaults, preserved model choices, Gemini text filtering and OpenAI request/response shapes. Live provider generation is unverified without account-specific credentials; the browser checks use synthetic IPC/network responses.
