# Query Studio

**Query Studio** is a powerful, AI-driven database tool that bridges the gap between natural language and SQL. Built with [Tauri](https://tauri.app/), [Vue 3](https://vuejs.org/), and [TypeScript](https://www.typescriptlang.org/), it offers a seamless native experience for managing and querying your databases.

## Features

-   **✨ AI-Powered Query Generation**: leveraging Google's **Gemini Pro**, simply describe what you need in plain English (e.g., *"Find all users who signed up last week and spent over $500"*), and Query Studio generates the precise SQL for you.
-   **🔌 Multi-Database Support**: Connects to **PostgreSQL**, **MySQL**, **SQLite**, and **MSSQL**.
-   **🧠 Logic Breakdown**: Don't just get the code—understand it. The app provides a step-by-step breakdown of the logic behind the generated SQL.
-   **⚡️ Real-Time Execution**: Execute queries instantly and view results in a responsive, sortable data table.
-   **🔒 Secure Connection Management**: Store and manage your database connections locally and securely.
-   **🌙 Beautiful UI**: A modern, dark-mode-first interface designed for developer productivity.

## Getting Started

### Prerequisites

-   **Node.js** (v18+)
-   **Rust & Cargo** (Required for Tauri)

### Installation

1.  Clone the repository:
    ```bash
    git clone https://github.com/iamEtornam/db-lang.git
    cd db-lang
    ```

2.  Install dependencies:
    ```bash
    npm install
    ```

### Running the App

To run the application in development mode (with hot-reloading):

```bash
npm run tauri dev
```

This will launch the Tauri window alongside the Vite development server.

## Tech Stack

-   **Frontend**: Vue 3, TypeScript, Tailwind CSS
-   **Backend/Core**: Rust (Tauri)
-   **AI**: Google Gemini Pro via Genkit (or direct API integration)
