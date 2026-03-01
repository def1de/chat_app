# Chat App

A lightweight real‑time chat application built with Rust, Axum, WebSockets, Askama templates, and SQLite. Supports user authentication with sessions, multi‑chat rooms with membership, invite links, message persistence, and a simple responsive UI.

-   Language/runtime: Rust (edition 2021)
-   Web framework: axum 0.7
-   Realtime: WebSockets (axum ws)
-   Templates: Askama
-   Database: SQLite (file `database.db`)
-   Async runtime: Tokio

## Features

-   Authentication and sessions
    -   Sign in via `/auth` (auto‑registers new username on first sign‑in)
    -   Passwords hashed with SHA‑256
    -   Session cookie `session_token` (HttpOnly), 7‑day expiry
-   Chats and membership
    -   Create chats (POST `/newchat`)
    -   Membership enforced for viewing and WebSocket access
    -   Invite links with 7‑day expiry (POST `/create_invite/:chat_id`, open `/invite/:code`)
-   Real‑time chat with persistence
    -   WebSocket endpoint per chat: `/chatsocket/:id`
    -   Messages stored in SQLite and rendered on page load
-   UI/UX
    -   Askama‑rendered pages: `index.html`, `chat.html`, `auth.html`
    -   Static assets under `/static` (CSS, favicon, JS)
    -   Chat carousel and basic keyboard UX
-   Status endpoint
    -   `/status` returns `{ "connected_clients": <number> }`

## Endpoints

-   GET `/auth` → login page
-   POST `/auth` → login/register; sets `session_token`
-   POST `/logout` → clears session
-   GET `/` → home with chat list (auth required)
-   POST `/newchat` (JSON `{ chat_name }`) → create chat (auth)
-   GET `/chat/:id` → chat view with history (auth + member)
-   GET `/chatsocket/:id` (WebSocket) → real‑time chat (auth + member)
-   POST `/create_invite/:chat_id` → returns `{ code }` (auth + member)
-   GET `/invite/:code` → join chat by code (auth)
-   GET `/status` → JSON with connected client count

## Project structure

-   `src/`
    -   `main.rs` — app setup, routes, state
    -   `handlers.rs` — HTTP handlers (pages, auth, invites, status)
    -   `websocket.rs` — WebSocket connection lifecycle and broadcast
    -   `database.rs` — SQLite access layer and schema creation
    -   `auth.rs` — extractor for authenticated user from session cookie
    -   `template.rs` — Askama view structs
-   `templates/` — Askama templates (`index.html`, `chat.html`, `auth.html`)
-   `static/` — CSS, JS, favicon (`scripts.js`, `styles.css`, …)
-   `database.db` — SQLite database (auto‑created)
-   `Cargo.toml` — dependencies

## Quick start

Prerequisites: Rust toolchain (stable). On Linux, ensure SQLite is available (libsqlite3 is commonly installed by default).

1. Build and run

-   `cargo run` (debug build)
-   The server binds by default to `0.0.0.0:1578` (see `src/main.rs`). Change this to your local IP or `127.0.0.1:1578` for local use.

2. Open the app

-   Navigate to `http://<bind-address>:1578/`
-   You will be redirected to `/auth` to sign in. A new username is created on first sign‑in.

3. Create and use chats

-   Click [+] New Chat on the home page
-   Share an invite: click “Create Invite Link” in a chat, copy the generated URL, and share

## Configuration notes

-   Bind address: edit `tokio::net::TcpListener::bind("…")` in `src/main.rs`
-   Production vs local URLs: `static/scripts.js` uses absolute URLs pointing to `chat.def1de.com` for WebSocket and status. For local use, switch to relative URLs, e.g.:
    -   WebSocket: `new WebSocket(`${location.origin.replace(/^http/, 'ws')}/chatsocket/${chatId}`)`
    -   Status: `fetch('/status')`

## Database

-   File: `database.db` (created/migrated automatically on startup)
-   Foreign keys enabled; cascading deletes on chat removal
-   Tables (simplified):
    -   `Users(userID, username, password_hash)`
    -   `Sessions(sessionID, userID, session_token, expires_at)`
    -   `Chats(chatID, chat_name)`
    -   `ChatMembers(chatID, userID)` (composite PK)
    -   `Messages(messageID, message_text, username, chatID, timestamp)`
    -   `InviteCodes(code, chatID, expires_at)`

To reset data, stop the app and delete `database.db`.

## How it works (brief)

-   App state holds a shared map of connected WebSockets, keyed by a unique socket ID
-   When a message arrives on `/chatsocket/:id`, it is saved to SQLite and broadcast to all sockets joined to that chat
-   Pages are server‑rendered via Askama; dynamic updates come from the WebSocket stream

## Security and limitations

-   No CSRF protection on POST endpoints; place behind a trusted origin/reverse proxy
-   Simple SHA‑256 password hashing without salt/argon2; for production, use a stronger KDF
-   Auto‑registration on first login by username
-   In‑memory socket registry (single process); no cross‑instance broadcast
