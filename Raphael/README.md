# Raphael

**Universal AI-to-Desktop Bridge** — Rust + Tauri v2 desktop application.

Raphael is an open-source starter project with a dark orange/black interface inspired by the supplied logo. The application stores settings in the operating system's app-config directory and exposes a local MCP-style JSON-RPC endpoint.

## Included in this source snapshot

- Tauri v2 desktop shell, tray menu, hide-on-close behavior.
- Axum/Tokio HTTP listener with Bearer token authentication.
- JSON-RPC methods: `initialize`, `ping`, `tools/list`, `tools/call`.
- `/mcp` HTTP POST endpoint, `/sse` event-stream endpoint, `/messages` POST alias, and `/health`.
- A separate `--stdio` process mode with newline-delimited JSON-RPC and three built-in tools.
- YAML configuration, theme picker, editable tools, activity feed, approval dialog.
- Safe launcher allowlist; arbitrary shell execution is intentionally disabled.

## Important transport note

The HTTP implementation is a lightweight JSON-RPC bridge with an SSE endpoint-discovery/keepalive stream. It is **not yet a complete implementation of the legacy MCP HTTP+SSE session transport** (session IDs and response delivery over the event stream are not implemented). The `/mcp` endpoint is also not a full Streamable HTTP implementation with MCP session negotiation. The stdio mode implements the basic initialize/list/call flow for its built-in tools. Do not advertise this snapshot as fully MCP transport-certified. For production interoperability, replace the hand-written transport with the official Rust MCP SDK (`rmcp`) and its Streamable HTTP and stdio transports.

## Security model

- HTTP requests must include `Authorization: Bearer <token>`.
- The server binds to `127.0.0.1` only; it is not exposed to the LAN or internet by default.
- The generated token is stored in `config.yaml`. Keep it private.
- Approval can be enabled globally and per tool.
- `shell_command` is intentionally restricted to the `status` built-in. Arbitrary shell execution is not enabled.
- `open_url` accepts only `http` and `https` URLs.
- The app launcher supports a small platform-specific allowlist.
- Webhooks make outbound HTTP POST requests to the configured URL. Only configure trusted endpoints.

## Requirements

- Rust toolchain (stable; current Tauri v2 prerequisites)
- Tauri CLI v2 (`cargo install tauri-cli --version '^2'`)
- Platform build prerequisites from the Tauri v2 documentation
- Internet access on first UI launch for Tailwind CDN (the app has local CSS for its core layout)

## Setup

1. Extract this repository and open a terminal in the project root.
2. Generate platform icons from the supplied `app-icon.png`:

   ```bash
   cargo tauri icon app-icon.png --output src-tauri/icons
   ```

3. Start the desktop application:

   ```bash
   cargo tauri dev
   ```

4. Build installers:

   ```bash
   cargo tauri build
   ```

   Windows outputs are placed under `src-tauri/target/release/bundle/` (NSIS setup `.exe` and MSI). macOS DMG output is generated when building on macOS. Build each platform on its native supported build environment.

## MCP connection

- HTTP JSON-RPC endpoint: `http://127.0.0.1:<port>/mcp`
- Health check: `http://127.0.0.1:<port>/health`
- SSE discovery stream: `http://127.0.0.1:<port>/sse`
- Authorization: `Bearer <token>`

Example request body:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"example","version":"1.0"}}}
```

Example `tools/list` body:

```json
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
```

## stdio

The app executable can be started in stdio mode:

```bash
raphael --stdio
```

The stdio process reads one JSON-RPC message per line and writes responses to stdout. This mode exposes the three built-in tools and does not currently load the GUI's dynamic YAML tool list.

## Project structure

```text
Raphael/
├── app-icon.png
├── Cargo.toml
├── README.md
├── LICENSE
├── src/
│   ├── index.html
│   └── app-icon.png
└── src-tauri/
    ├── Cargo.toml
    ├── build.rs
    ├── tauri.conf.json
    ├── capabilities/
    │   └── default.json
    └── src/
        ├── main.rs
        ├── lib.rs
        ├── config.rs
        ├── mcp_server.rs
        └── stdio.rs
```

## Configuration

The first desktop launch creates `config.yaml` inside the platform-specific Raphael application config directory. Use **Customize & Security → Open Folder** to open that directory.

## License

MIT. See [LICENSE](LICENSE).

## Publishing Windows installers on GitHub

This repository includes `.github/workflows/release-windows.yml`. It builds **Windows x64 NSIS `.exe` and MSI installers** on a GitHub-hosted Windows runner. The build is deliberately performed on Windows rather than attempting to cross-build the WebView2-based desktop app from Linux.

### Create a public installer release

1. Push the Raphael repository to GitHub.
2. In GitHub, open **Actions → Build Raphael Windows installers → Run workflow** to test a build. The workflow uploads both installer files as an Actions artifact.
3. When ready to publish version `0.1.0`, create and push the tag:

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

4. The workflow builds the installers and attaches the `.exe` and `.msi` files to the GitHub Release for that tag. Share the Release page with users; they download and run the installer without installing Rust or Cargo.

Expected output folders on the Windows runner:

- `src-tauri/target/release/bundle/nsis/*.exe`
- `src-tauri/target/release/bundle/msi/*.msi`

The NSIS installer provides the familiar Windows setup wizard. The MSI is suitable for Windows Installer-based deployment. Builds are currently unsigned; Windows SmartScreen may show an unknown-publisher warning until the publisher signs the installer with a trusted code-signing certificate. Do not claim the installer is signed unless a signing certificate and signing workflow have been configured.
