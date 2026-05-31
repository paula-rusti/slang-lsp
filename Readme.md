# System Verilog LSP Server

Powered by `slang`

## How to use

1. Clone this repo and open it as a workspace in VsCode

2. Open `sv-lsp-client/extension.js` file in current window

3. Hit Cmd+Shift+P -> Start Debugging 
This will open a new vscode workspace where it runs the lsp server and client in debug mode

4. Open a test .sv file from the data directory in the new VsCode window and you should see squiggly lines showcasing errors (both syntax and semantic errors). Open the Output window and select SV LSP and should see debug lines from the lsp server.


## Architecture
LSP protocol define communication over stdio between the language server and the LSP client (VSC extension). Messages are sent as JSON RPC.



