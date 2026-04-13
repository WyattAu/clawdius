import * as vscode from 'vscode';
import { ClawdiusClient } from './rpc/client';
import { ChatViewProvider } from './providers/chatView';
import { StatusBarProvider } from './providers/statusBar';
import { DiffViewProvider } from './providers/diffView';
import { ClawdiusCompletionProvider } from './completion/provider';
import { ClawdiusCodeActionProvider } from './codeActions/provider';
let client: ClawdiusClient;
let diffViewProvider: DiffViewProvider;

export async function activate(context: vscode.ExtensionContext) {
    console.log('Clawdius Code is activating...');
    
    client = new ClawdiusClient();
    
    try {
        await client.start();
    } catch (err) {
        vscode.window.showErrorMessage(`Failed to start Clawdius: ${err}`);
        return;
    }
    
    diffViewProvider = new DiffViewProvider();
    context.subscriptions.push(diffViewProvider);
    
    registerCommands(context, client, diffViewProvider);
    
    const chatViewProvider = new ChatViewProvider(context, client, diffViewProvider);
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(
            'clawdius.chatView',
            chatViewProvider,
            { webviewOptions: { retainContextWhenHidden: true } }
        )
    );
    
    const statusBar = new StatusBarProvider(client);
    statusBar.activate();
    context.subscriptions.push(statusBar);
    
    const completionProvider = new ClawdiusCompletionProvider(client);
    context.subscriptions.push(
        vscode.languages.registerInlineCompletionItemProvider(
            { pattern: '**/*' },
            completionProvider
        )
    );
    
    const codeActionProvider = new ClawdiusCodeActionProvider(client);
    context.subscriptions.push(
        vscode.languages.registerCodeActionsProvider(
            [
                { scheme: 'file', language: 'rust' },
                { scheme: 'file', language: 'typescript' },
                { scheme: 'file', language: 'javascript' },
                { scheme: 'file', language: 'python' },
            ],
            codeActionProvider,
            {
                providedCodeActionKinds: [
                    vscode.CodeActionKind.QuickFix,
                    vscode.CodeActionKind.Refactor,
                    vscode.CodeActionKind.Source,
                ],
            }
        )
    );
    
    context.subscriptions.push(
        vscode.workspace.onDidChangeConfiguration(e => {
            if (e.affectsConfiguration('clawdius')) {
                client.reloadConfiguration();
            }
        })
    );
}

function registerCommands(context: vscode.ExtensionContext, client: ClawdiusClient, diffView: DiffViewProvider) {
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.chat', async () => {
            const input = await vscode.window.showInputBox({
                prompt: 'Ask Clawdius',
                placeHolder: 'Type your question...'
            });
            
            if (input) {
                const response = await client.chat(input);
                vscode.window.showInformationMessage(response.content);
            }
        })
    );
    
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.chatSelection', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }
            
            const selection = editor.document.getText(editor.selection);
            if (selection) {
                await client.chat(`Explain this code:\n\`\`\`\n${selection}\n\`\`\``);
            }
        })
    );
    
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.addContext', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                return;
            }
            
            await client.addContext('file', editor.document.uri.fsPath);
            vscode.window.showInformationMessage('Added to context');
        })
    );
    
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.addFileContext', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                return;
            }
            
            await client.addContext('file', editor.document.uri.fsPath);
            vscode.window.showInformationMessage('Added to context');
        })
    );
    
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.checkpoint', async () => {
            const description = await vscode.window.showInputBox({
                prompt: 'Checkpoint description'
            });
            
            if (description !== undefined) {
                const checkpoint = await client.createCheckpoint(description);
                vscode.window.showInformationMessage(`Checkpoint created: ${checkpoint.id}`);
            }
        })
    );
    
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.openChat', () => {
            vscode.commands.executeCommand('workbench.view.extension.clawdius-chatView');
        })
    );
    
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.showDiff', async () => {
            const current = diffView.getCurrentChange();
            if (current) {
                await diffView.showDiff(current);
            } else {
                vscode.window.showWarningMessage('No change currently selected');
            }
        })
    );
    
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.acceptChange', async () => {
            await diffView.acceptCurrentChange();
            updateChangeContext(diffView);
        })
    );
    
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.rejectChange', async () => {
            await diffView.rejectCurrentChange();
            updateChangeContext(diffView);
        })
    );
    
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.showAllChanges', async () => {
            await diffView.showAllChanges();
        })
    );
}

function updateChangeContext(diffView: DiffViewProvider) {
    const hasChanges = diffView.getPendingChanges().length > 0;
    vscode.commands.executeCommand('setContext', 'clawdius.hasPendingChanges', hasChanges);
}

export async function deactivate() {
    console.log('Clawdius Code is deactivating...');
    if (client) {
        await client.stop();
    }
}
