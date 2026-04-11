# Query Studio

**Query Studio** is an AI-powered desktop database management tool. Connect to any SQL or NoSQL database, ask questions in plain English, and get accurate queries generated using your actual database schema — then see the results instantly.

Built with [Tauri 2](https://tauri.app/), [Nuxt 4](https://nuxt.com/), [Shadcn Vue](https://www.shadcn-vue.com/), and Rust.

![Query Studio](https://img.shields.io/badge/version-0.1.0-blue)
![License](https://img.shields.io/badge/license-MIT-green)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey)

---

## Features

- **Natural language querying** — Type a question in plain English; the AI generates the exact SQL using your live database schema
- **Schema-aware AI** — Connects to your database, reads all tables and columns, and uses that as context so table names, column names, and relationships are always correct
- **Multi-database support** — PostgreSQL, MySQL, MariaDB, SQLite, SQL Server, MongoDB, Redis
- **Multiple AI providers** — Google Gemini, OpenAI, Anthropic Claude, Ollama (local), DeepSeek, Groq, or any OpenAI-compatible API
- **Schema explorer** — Browse tables, columns, types, primary keys, foreign keys, and preview data
- **AI data visualization** — Automatically generates the best chart type for your query results
- **AI data insights** — Plain-English explanation of what your query results mean
- **Query history** — Every query is saved with timing and result counts
- **Saved snippets** — Save and reuse your favourite queries
- **Dark/light mode** — Full theme support with system preference detection

---

## Download

Grab the latest release for your platform from the [Releases](https://github.com/iamEtornam/db-lang/releases) page.

| Platform | File |
|----------|------|
| macOS (Apple Silicon) | `Query.Studio_*_aarch64.dmg` |
| macOS (Intel) | `Query.Studio_*_x64.dmg` |
| Linux | `query-studio_*_amd64.AppImage` or `.deb` |
| Windows | `Query.Studio_*_x64-setup.exe` |

---

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) v22+
- [Rust](https://rustup.rs/) (stable)
- On Linux: `libwebkit2gtk-4.1-dev`, `libssl-dev`, `pkg-config` (see workflow for full list)

### Setup

```bash
# Clone the repo
git clone https://github.com/iamEtornam/db-lang.git
cd db-lang

# Install JS dependencies
npm install

# Start dev server (Tauri + Nuxt hot reload)
npm run tauri dev
```

### Build

```bash
# Build for production (current platform)
npm run tauri build
```

---

## Configuration

### AI Provider

Go to **Settings** in the app and configure your preferred AI provider. Query Studio supports:

| Provider | Models |
|----------|--------|
| Google Gemini | gemini-2.5-pro, gemini-2.5-flash, gemini-2.5-flash-lite |
| OpenAI | gpt-5.4, gpt-5.4-mini, gpt-4o, gpt-4o-mini |
| Anthropic | claude-opus-4-6, claude-sonnet-4-6, claude-haiku-4-5 |
| Ollama (local) | Any locally installed model |
| DeepSeek | deepseek-chat, deepseek-reasoner |
| Groq | llama-3.3-70b, openai/gpt-oss-120b, and more |
| Custom | Any OpenAI-compatible endpoint |

Your API key is stored locally and never sent anywhere except directly to the chosen provider.

---

## Creating a Release

Releases are built automatically by GitHub Actions when you push a version tag:

```bash
# Bump version in src-tauri/tauri.conf.json first, then:
git add .
git commit -m "chore: release v0.2.0"
git tag v0.2.0
git push origin main --tags
```

This triggers the [release workflow](.github/workflows/release.yml) which builds native binaries for:
- macOS (Apple Silicon + Intel)
- Linux (AppImage + .deb)
- Windows (.exe installer)

The release is created as a draft — review it on GitHub and publish when ready.

---

## Tech Stack

| Layer | Technology |
|-------|------------|
| Desktop shell | Tauri 2 (Rust) |
| Frontend | Nuxt 4, Vue 3, TypeScript |
| UI components | Shadcn Vue, Tailwind CSS 4 |
| State management | Pinia |
| Database drivers | tokio-postgres, mysql_async, rusqlite, mongodb, redis |
| AI integration | reqwest → Gemini / OpenAI / Anthropic / Ollama APIs |
| Local storage | SQLite (via rusqlite) |

---

## License

MIT
