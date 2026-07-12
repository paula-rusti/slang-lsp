# System Verilog LSP Server

Powered by [`slang`](https://github.com/MikePopoloski/slang)

## Prerequisites

### Install `slang` CLI

The LSP server invokes `slang` as a subprocess for linting. Install it so it's available on your `PATH`:

```bash

git clone https://github.com/MikePopoloski/slang.git
cd slang
cmake -B build
cmake --build build -j
# The binary is at build/bin/slang - add it to your PATH or symlink it into /usr/local/bin
```

Verify it's working:

```bash
slang --version
```

## How to Build

### Build the LSP server binary

```bash
cargo build --manifest-path server/Cargo.toml --release
# Binary output: server/target/release/LspServer
```

### Install VS Code extension dependencies

```bash
cd vsc_client && npm install
```

## How to Use

1. Clone this repo and open it as a workspace in VS Code

2. Build the server binary (see above)

3. Open `vsc_client/extension.js` in the current window

4. Hit `Cmd+Shift+P` → **Start Debugging**
   This opens a new VS Code window running the LSP server and client in debug mode

5. Open a test `.sv` file from the `data/` directory in the new VS Code window — you should see squiggly lines for errors (both syntax and semantic). Open the Output panel and select **SV LSP** to see debug output from the server.


## Architecture

The LSP protocol defines communication over stdio between the language server and the LSP client (VS Code extension). Messages are sent as JSON-RPC.

```
VS Code  ←→  vsc_client (extension)  ←→  server (Rust LSP)  →  slang (subprocess)
```



