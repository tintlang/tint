"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = require("vscode");
const cp = require("child_process");
const fs = require("fs");
const path = require("path");
let outputChannel;
function activate(context) {
    outputChannel = vscode.window.createOutputChannel('Rune');
    // Ensure rune CLI is in PATH
    const runeCLI = findRuneCLI();
    if (!runeCLI) {
        vscode.window.showWarningMessage('Rune CLI not found. Install it with: cargo install --path runelang/rune-model/crates/rune-cli');
    }
    // Register commands
    const buildCmd = vscode.commands.registerCommand('rune.build', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor)
            return;
        const filePath = editor.document.uri.fsPath;
        if (!filePath.endsWith('.rn')) {
            vscode.window.showErrorMessage('Not a Rune file');
            return;
        }
        // Save file first
        if (editor.document.isDirty) {
            await editor.document.save();
        }
        const outPath = filePath.replace(/\.rn$/, '.html');
        await buildFile(filePath, outPath, runeCLI);
    });
    const checkCmd = vscode.commands.registerCommand('rune.check', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor)
            return;
        const filePath = editor.document.uri.fsPath;
        if (!filePath.endsWith('.rn')) {
            vscode.window.showErrorMessage('Not a Rune file');
            return;
        }
        if (editor.document.isDirty) {
            await editor.document.save();
        }
        await checkFile(filePath, runeCLI);
    });
    const previewCmd = vscode.commands.registerCommand('rune.showPreview', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor)
            return;
        const filePath = editor.document.uri.fsPath;
        const outPath = filePath.replace(/\.rn$/, '.html');
        if (fs.existsSync(outPath)) {
            const panel = vscode.window.createWebviewPanel('runePreview', 'Rune Preview', vscode.ViewColumn.Beside, { enableScripts: true });
            const html = fs.readFileSync(outPath, 'utf-8');
            panel.webview.html = html;
        }
        else {
            vscode.window.showInformationMessage('Build the file first with Cmd+Shift+B');
        }
    });
    context.subscriptions.push(buildCmd, checkCmd, previewCmd);
    vscode.window.showInformationMessage('Rune language support loaded');
}
async function buildFile(inPath, outPath, runeCLI) {
    if (!runeCLI) {
        vscode.window.showErrorMessage('Rune CLI not found');
        return;
    }
    return new Promise((resolve) => {
        const proc = cp.spawn(runeCLI, ['build', inPath, '-o', outPath], {
            stdio: ['pipe', 'pipe', 'pipe']
        });
        let stdout = '';
        let stderr = '';
        proc.stdout?.on('data', (data) => {
            stdout += data.toString();
        });
        proc.stderr?.on('data', (data) => {
            stderr += data.toString();
        });
        proc.on('close', (code) => {
            outputChannel.clear();
            outputChannel.appendLine(`$ rune build ${inPath} -o ${outPath}`);
            if (code === 0) {
                outputChannel.appendLine(stdout || 'Build succeeded');
                outputChannel.show();
                vscode.window.showInformationMessage(`Built: ${outPath}`);
            }
            else {
                outputChannel.appendLine(stderr || 'Build failed');
                outputChannel.show();
                vscode.window.showErrorMessage('Build failed. See output.');
            }
            resolve();
        });
    });
}
async function checkFile(filePath, runeCLI) {
    if (!runeCLI) {
        vscode.window.showErrorMessage('Rune CLI not found');
        return;
    }
    return new Promise((resolve) => {
        const proc = cp.spawn(runeCLI, ['check', filePath], {
            stdio: ['pipe', 'pipe', 'pipe']
        });
        let stdout = '';
        let stderr = '';
        proc.stdout?.on('data', (data) => {
            stdout += data.toString();
        });
        proc.stderr?.on('data', (data) => {
            stderr += data.toString();
        });
        proc.on('close', (code) => {
            outputChannel.clear();
            outputChannel.appendLine(`$ rune check ${filePath}`);
            if (code === 0) {
                outputChannel.appendLine(stdout || 'OK');
                outputChannel.show();
            }
            else {
                outputChannel.appendLine(stderr || 'Error');
                outputChannel.show();
            }
            resolve();
        });
    });
}
function findRuneCLI() {
    // Try to find rune in PATH
    const pathEnv = process.env.PATH || '';
    const paths = pathEnv.split(path.delimiter);
    for (const dir of paths) {
        const runePath = path.join(dir, 'rune');
        if (fs.existsSync(runePath)) {
            return runePath;
        }
    }
    // Try common installation locations
    const commonPaths = [
        path.join(process.env.HOME || '', '.cargo', 'bin', 'rune'),
        '/usr/local/bin/rune',
        '/usr/bin/rune'
    ];
    for (const p of commonPaths) {
        if (fs.existsSync(p)) {
            return p;
        }
    }
    return null;
}
function deactivate() { }
//# sourceMappingURL=extension.js.map