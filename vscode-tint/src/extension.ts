import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

let outputChannel: vscode.OutputChannel;
let languageServer: TintLanguageServer | undefined;

export function activate(context: vscode.ExtensionContext) {
    outputChannel = vscode.window.createOutputChannel('Tint');

    const analyzer = findTintAnalyzer();
    if (analyzer) {
        outputChannel.appendLine(`[analyzer] starting: ${analyzer}`);
        languageServer = new TintLanguageServer(analyzer, outputChannel);
        context.subscriptions.push(
            languageServer,
            vscode.languages.registerHoverProvider('tint', {
                provideHover: (document, position) => languageServer?.hover(document, position) ?? null
            }),
            vscode.languages.registerDefinitionProvider('tint', {
                provideDefinition: (document, position) => languageServer?.definition(document, position) ?? null
            }),
            vscode.workspace.onDidOpenTextDocument(document => languageServer?.open(document)),
            vscode.workspace.onDidChangeTextDocument(event => languageServer?.change(event.document)),
            vscode.workspace.onDidCloseTextDocument(document => languageServer?.close(document))
        );
        for (const document of vscode.workspace.textDocuments) {
            languageServer.open(document);
        }
    } else {
        vscode.window.showWarningMessage(
            'Tint analyzer not found. Build it with: cargo build -p tint-analyzer'
        );
    }
    
    // Ensure tint CLI is in PATH
    const tintCLI = findTintCLI();
    if (!tintCLI) {
        vscode.window.showWarningMessage(
            'Tint CLI not found. Install a release from: https://github.com/tintlang/tint/releases'
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

function findTintAnalyzer(): string | null {
    const workspaceCandidates = (vscode.workspace.workspaceFolders || []).flatMap(folder => [
        path.join(folder.uri.fsPath, 'tint-model', 'target', 'debug', 'tint-analyzer'),
        path.join(folder.uri.fsPath, 'tint-model', 'target', 'release', 'tint-analyzer'),
        path.join(folder.uri.fsPath, 'target', 'debug', 'tint-analyzer'),
        path.join(folder.uri.fsPath, 'target', 'release', 'tint-analyzer')
    ]);
    const candidates = [
        process.env.TINT_ANALYZER,
        ...workspaceCandidates,
        ...((process.env.PATH || '').split(path.delimiter).map(dir => path.join(dir, 'tint-analyzer'))),
        path.join(process.env.HOME || '', '.cargo', 'bin', 'tint-analyzer'),
        path.resolve(__dirname, '../../tint-model/target/debug/tint-analyzer'),
        path.resolve(__dirname, '../../tint-model/target/release/tint-analyzer')
    ];
    return candidates.find((candidate): candidate is string => !!candidate && fs.existsSync(candidate)) || null;
}

class TintLanguageServer implements vscode.Disposable {
    private readonly process: cp.ChildProcessWithoutNullStreams;
    private readonly diagnostics: vscode.DiagnosticCollection;
    private readonly pending = new Map<number, (result: any) => void>();
    private buffer = Buffer.alloc(0);
    private nextId = 1;

    constructor(executable: string, private readonly output: vscode.OutputChannel) {
        this.process = cp.spawn(executable, [], { stdio: ['pipe', 'pipe', 'pipe'] });
        this.diagnostics = vscode.languages.createDiagnosticCollection('tint');
        this.process.on('error', error => {
            this.output.appendLine(`[analyzer] failed to start: ${error.message}`);
            this.output.show(true);
        });
        this.process.on('close', (code, signal) => {
            this.output.appendLine(`[analyzer] exited: code=${code ?? 'none'} signal=${signal ?? 'none'}`);
        });
        this.process.stdout.on('data', data => this.receive(data));
        this.process.stderr.on('data', data => this.output.appendLine(`[analyzer] ${data.toString().trimEnd()}`));
        this.request('initialize', {
            processId: process.pid,
            rootUri: vscode.workspace.workspaceFolders?.[0]?.uri.toString() ?? null,
            capabilities: {}
        }).then(() => this.notify('initialized', {}));
    }

    open(document: vscode.TextDocument): void {
        if (document.languageId === 'tint') {
            this.notify('textDocument/didOpen', {
                textDocument: { uri: document.uri.toString(), languageId: 'tint', version: document.version, text: document.getText() }
            });
        }
    }

    change(document: vscode.TextDocument): void {
        if (document.languageId === 'tint') {
            this.notify('textDocument/didChange', {
                textDocument: { uri: document.uri.toString(), version: document.version },
                contentChanges: [{ text: document.getText() }]
            });
        }
    }

    close(document: vscode.TextDocument): void {
        if (document.languageId === 'tint') {
            this.notify('textDocument/didClose', { textDocument: { uri: document.uri.toString() } });
            this.diagnostics.delete(document.uri);
        }
    }

    async hover(document: vscode.TextDocument, position: vscode.Position): Promise<vscode.Hover | null> {
        const result = await this.request('textDocument/hover', {
            textDocument: { uri: document.uri.toString() },
            position: { line: position.line, character: position.character }
        });
        if (!result?.contents?.value) return null;
        return new vscode.Hover(new vscode.MarkdownString(result.contents.value));
    }

    async definition(document: vscode.TextDocument, position: vscode.Position): Promise<vscode.Location | null> {
        const result = await this.request('textDocument/definition', {
            textDocument: { uri: document.uri.toString() },
            position: { line: position.line, character: position.character }
        });
        if (!result?.uri || !result.range) return null;
        return new vscode.Location(vscode.Uri.parse(result.uri), toRange(result.range));
    }

    dispose(): void {
        this.notify('shutdown', {});
        this.process.kill();
        this.diagnostics.dispose();
        languageServer = undefined;
    }

    private notify(method: string, params: unknown): void {
        this.output.appendLine(`[analyzer] -> ${method}`);
        this.write({ jsonrpc: '2.0', method, params });
    }

    private request(method: string, params: unknown): Promise<any> {
        const id = this.nextId++;
        return new Promise(resolve => {
            this.pending.set(id, resolve);
            this.write({ jsonrpc: '2.0', id, method, params });
        });
    }

    private write(message: unknown): void {
        const body = Buffer.from(JSON.stringify(message));
        this.process.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
        this.process.stdin.write(body);
    }

    private receive(data: Buffer): void {
        this.buffer = Buffer.concat([this.buffer, data]);
        while (true) {
            const separator = this.buffer.indexOf('\r\n\r\n');
            if (separator < 0) return;
            const header = this.buffer.subarray(0, separator).toString();
            const match = /Content-Length:\s*(\d+)/i.exec(header);
            if (!match) return;
            const length = Number(match[1]);
            const start = separator + 4;
            if (this.buffer.length < start + length) return;
            const message = JSON.parse(this.buffer.subarray(start, start + length).toString());
            this.buffer = this.buffer.subarray(start + length);
            this.handle(message);
        }
    }

    private handle(message: any): void {
        if (message.method === 'textDocument/publishDiagnostics') {
            this.output.appendLine(`[analyzer] diagnostics: ${(message.params?.diagnostics || []).length}`);
        }
        if (message.id !== undefined && this.pending.has(message.id)) {
            this.pending.get(message.id)?.(message.result);
            this.pending.delete(message.id);
        }
        if (message.method === 'textDocument/publishDiagnostics') {
            const uri = vscode.Uri.parse(message.params.uri);
            const diagnostics = (message.params.diagnostics || []).map((item: any) => new vscode.Diagnostic(
                toRange(item.range), item.message, item.severity === 1 ? vscode.DiagnosticSeverity.Error : vscode.DiagnosticSeverity.Warning
            ));
            this.diagnostics.set(uri, diagnostics);
        }
    }
}

function toRange(range: any): vscode.Range {
    return new vscode.Range(
        new vscode.Position(range.start.line, range.start.character),
        new vscode.Position(range.end.line, range.end.character)
    );
}

export function deactivate() {}
