const vscode = require('vscode');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');
const path = require('path');

let client;

function activate(context) {
    const outputChannel = vscode.window.createOutputChannel('SV LSP');
    outputChannel.appendLine('SV LSP: activating...');

    const serverPath = path.join(__dirname, '..', 'target', 'release', 'LspServer');
    outputChannel.appendLine('server path: ' + serverPath);

    const serverOptions = {
        command: serverPath,
        transport: TransportKind.stdio,
    };

    const clientOptions = {
        documentSelector: [{ scheme: 'file', language: 'systemverilog' }],
        outputChannel: outputChannel,
    };

    client = new LanguageClient('sv-lsp', 'SV LSP', serverOptions, clientOptions);

    client.start().then(() => {
        outputChannel.appendLine('server ready');
    }).catch(err => {
        outputChannel.appendLine('failed: ' + err.message);
    });
}

function deactivate() {
    return client?.stop();
}

module.exports = { activate, deactivate };