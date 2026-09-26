import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

let outputChannel: vscode.OutputChannel;

export function activate(context: vscode.ExtensionContext) {
    outputChannel = vscode.window.createOutputChannel('Tint');
    
    // Ensure tint CLI is in PATH
    const tintCLI = findTintCLI();
    if (!tintCLI) {
        vscode.window.showWarningMessage(
            'Tint CLI not found. Install it with: cargo install --path tint-model/crates/tint-cli (run from the repo root)'
        );
    }

    // Register commands
    const buildCmd = vscode.commands.registerCommand('tint.build', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) return;
        
        const filePath = editor.document.uri.fsPath;
        if (!filePath.endsWith('.tn')) {
            vscode.window.showErrorMessage('Not a Tint file');
            return;
        }

        // Save file first
        if (editor.document.isDirty) {
            await editor.document.save();
        }

        const outPath = filePath.replace(/\.tn$/, '.html');
        await buildFile(filePath, outPath, tintCLI);
    });

    const checkCmd = vscode.commands.registerCommand('tint.check', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) return;
        
        const filePath = editor.document.uri.fsPath;
        if (!filePath.endsWith('.tn')) {
            vscode.window.showErrorMessage('Not a Tint file');
            return;
        }

        if (editor.document.isDirty) {
            await editor.document.save();
        }

        await checkFile(filePath, tintCLI);
    });

    const previewCmd = vscode.commands.registerCommand('tint.showPreview', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) return;
        
        const filePath = editor.document.uri.fsPath;
        const outPath = filePath.replace(/\.tn$/, '.html');
        
        if (fs.existsSync(outPath)) {
            const panel = vscode.window.createWebviewPanel(
                'tintPreview',
                'Tint Preview',
                vscode.ViewColumn.Beside,
                { enableScripts: true }
            );
            
            const html = fs.readFileSync(outPath, 'utf-8');
            panel.webview.html = html;
        } else {
            vscode.window.showInformationMessage('Build the file first with Cmd+Shift+B');
        }
    });

    context.subscriptions.push(buildCmd, checkCmd, previewCmd);
    
    vscode.window.showInformationMessage('Tint language support loaded');
}

async function buildFile(inPath: string, outPath: string, tintCLI: string | null): Promise<void> {
    if (!tintCLI) {
        vscode.window.showErrorMessage('Tint CLI not found');
        return;
    }

    return new Promise((resolve) => {
        const proc = cp.spawn(tintCLI, ['build', inPath, '-o', outPath], {
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
            outputChannel.appendLine(`$ tint build ${inPath} -o ${outPath}`);
            
            if (code === 0) {
                outputChannel.appendLine(stdout || 'Build succeeded');
                outputChannel.show();
                vscode.window.showInformationMessage(`Built: ${outPath}`);
            } else {
                outputChannel.appendLine(stderr || 'Build failed');
                outputChannel.show();
                vscode.window.showErrorMessage('Build failed. See output.');
            }
            resolve();
        });
    });
}

async function checkFile(filePath: string, tintCLI: string | null): Promise<void> {
    if (!tintCLI) {
        vscode.window.showErrorMessage('Tint CLI not found');
        return;
    }

    return new Promise((resolve) => {
        const proc = cp.spawn(tintCLI, ['check', filePath], {
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
            outputChannel.appendLine(`$ tint check ${filePath}`);
            
            if (code === 0) {
                outputChannel.appendLine(stdout || 'OK');
                outputChannel.show();
            } else {
                outputChannel.appendLine(stderr || 'Error');
                outputChannel.show();
            }
            resolve();
        });
    });
}

function findTintCLI(): string | null {
    // Try to find tint in PATH
    const pathEnv = process.env.PATH || '';
    const paths = pathEnv.split(path.delimiter);
    
    for (const dir of paths) {
        const tintPath = path.join(dir, 'tint');
        if (fs.existsSync(tintPath)) {
            return tintPath;
        }
    }

    // Try common installation locations
    const commonPaths = [
        path.join(process.env.HOME || '', '.cargo', 'bin', 'tint'),
        '/usr/local/bin/tint',
        '/usr/bin/tint'
    ];

    for (const p of commonPaths) {
        if (fs.existsSync(p)) {
            return p;
        }
    }

    return null;
}

export function deactivate() {}
