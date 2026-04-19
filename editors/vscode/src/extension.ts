import * as vscode from 'vscode';
import { ClawdiusClient } from './rpc/client';
import { ClawdiusRestClient } from './rest/client';
import { ChatViewProvider } from './providers/chatView';
import { StatusBarProvider } from './providers/statusBar';
import { DiffViewProvider } from './providers/diffView';
import { ClawdiusCompletionProvider } from './completion/provider';
import { ClawdiusCodeActionProvider } from './codeActions/provider';
let client: ClawdiusClient;
let diffViewProvider: DiffViewProvider;
let restClient: ClawdiusRestClient | undefined;

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
                // Reinitialize REST client with new URL
                const serverUrl = vscode.workspace.getConfiguration('clawdius').get<string>('serverUrl');
                const outputChannel = vscode.window.createOutputChannel('Clawdius');
                restClient = new ClawdiusRestClient(serverUrl || 'http://localhost:8080', outputChannel);
            }
        })
    );

    // Initialize REST client for sprint/skills/ship commands
    const serverUrl = vscode.workspace.getConfiguration('clawdius').get<string>('serverUrl');
    const outputChannel = vscode.window.createOutputChannel('Clawdius');
    restClient = new ClawdiusRestClient(serverUrl || 'http://localhost:8080', outputChannel);
    context.subscriptions.push(outputChannel);

    // Check REST server connectivity
    if (restClient) {
        const healthy = await restClient.healthCheck();
        if (healthy) {
            outputChannel.appendLine('[Clawdius] REST API server connected');
        } else {
            outputChannel.appendLine('[Clawdius] REST API server not reachable (start with `clawdius serve`)');
        }
    }

    registerSprintCommands(context, restClient);
    registerSkillCommands(context, restClient);
    registerShipCommands(context, restClient);
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

function registerSprintCommands(
    context: vscode.ExtensionContext,
    restClient: ClawdiusRestClient | undefined,
) {
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.runSprint', async () => {
            if (!restClient) {
                vscode.window.showErrorMessage('REST API not configured');
                return;
            }
            const task = await vscode.window.showInputBox({
                prompt: 'Sprint task description',
                placeHolder: 'Describe what the sprint should accomplish...',
                title: 'Run Sprint',
            });
            if (!task) return;

            const realExecution = await vscode.window.showQuickPick(['Yes', 'No'], {
                placeHolder: 'Enable real command execution (build, test)?',
            });
            if (!realExecution) return;

            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: `Sprint: ${task}`,
                    cancellable: false,
                },
                async (progress) => {
                    try {
                        progress.report({ message: 'Starting sprint...' });
                        const result = await restClient.runSprint({
                            task,
                            real_execution: realExecution === 'Yes',
                            auto_approve: true,
                            max_iterations: 3,
                        });

                        const phases = result.phase_results
                            .map((p) => `  ${p.phase}: ${p.status} (${p.duration_ms}ms)`)
                            .join('\n');
                        const message = result.success
                            ? `Sprint completed successfully!\n\n${phases}\n\n${result.summary}`
                            : `Sprint failed.\n\n${phases}\n\n${result.summary}`;

                        if (result.success) {
                            vscode.window.showInformationMessage(message);
                        } else {
                            vscode.window.showWarningMessage(message);
                        }
                    } catch (error) {
                        const msg = error instanceof Error ? error.message : String(error);
                        vscode.window.showErrorMessage(`Sprint failed: ${msg}`);
                    }
                },
            );
        }),
    );
}

function registerSkillCommands(
    context: vscode.ExtensionContext,
    restClient: ClawdiusRestClient | undefined,
) {
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.listSkills', async () => {
            if (!restClient) {
                vscode.window.showErrorMessage('REST API not configured');
                return;
            }
            try {
                const skills = await restClient.listSkills();
                if (skills.length === 0) {
                    vscode.window.showInformationMessage('No skills available');
                    return;
                }
                const items = skills.map((s) => ({
                    label: `/${s.name}`,
                    description: s.description,
                    detail: `v${s.version} [${s.tags.join(', ')}]`,
                }));
                const selected = await vscode.window.showQuickPick(items, {
                    placeHolder: 'Select a skill to run',
                });
                if (!selected) return;

                const editor = vscode.window.activeTextEditor;
                const selection = editor?.document.getText(editor.selection) || undefined;
                const projectRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

                const result = await restClient.executeSkill(
                    selected.label.replace('/', ''),
                    undefined,
                    selection,
                    projectRoot,
                );
                if (result.success) {
                    // Show result in a new untitled document
                    const doc = await vscode.workspace.openTextDocument({
                        content: result.result,
                        language: 'markdown',
                    });
                    await vscode.window.showTextDocument(doc, { preview: true });
                } else {
                    vscode.window.showErrorMessage(`Skill failed: ${result.result}`);
                }
            } catch (error) {
                const msg = error instanceof Error ? error.message : String(error);
                vscode.window.showErrorMessage(`Failed to list skills: ${msg}`);
            }
        }),
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.runSkill', async () => {
            if (!restClient) {
                vscode.window.showErrorMessage('REST API not configured');
                return;
            }
            const editor = vscode.window.activeTextEditor;
            if (!editor || editor.selection.isEmpty) {
                vscode.window.showWarningMessage('Select code first to run a skill on it');
                return;
            }

            const skills = await restClient.listSkills();
            const items = skills.map((s) => ({
                label: `/${s.name}`,
                description: s.description,
            }));
            const selected = await vscode.window.showQuickPick(items, {
                placeHolder: 'Select a skill to run on selection',
            });
            if (!selected) return;

            const selection = editor.document.getText(editor.selection);
            const projectRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: `Running ${selected.label}`,
                },
                async () => {
                    try {
                        const result = await restClient.executeSkill(
                            selected.label.replace('/', ''),
                            undefined,
                            selection,
                            projectRoot,
                        );
                        const doc = await vscode.workspace.openTextDocument({
                            content: result.result,
                            language: 'markdown',
                        });
                        await vscode.window.showTextDocument(doc, { preview: true });
                    } catch (error) {
                        const msg = error instanceof Error ? error.message : String(error);
                        vscode.window.showErrorMessage(`Skill failed: ${msg}`);
                    }
                },
            );
        }),
    );
}

function registerShipCommands(
    context: vscode.ExtensionContext,
    restClient: ClawdiusRestClient | undefined,
) {
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.preShipChecks', async () => {
            if (!restClient) {
                vscode.window.showErrorMessage('REST API not configured');
                return;
            }
            const projectRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: 'Running pre-ship checks...',
                },
                async () => {
                    try {
                        const results = await restClient.runShipChecks(projectRoot);
                        const passed = results.filter((r) => r.passed).length;
                        const failed = results.filter((r) => !r.passed).length;
                        const message = `Pre-ship checks: ${passed} passed, ${failed} failed`;
                        if (failed === 0) {
                            vscode.window.showInformationMessage(message);
                        } else {
                            const details = results
                                .filter((r) => !r.passed)
                                .map((r) => `  ✗ ${r.check_name}: ${r.message}`)
                                .join('\n');
                            vscode.window.showWarningMessage(`${message}\n\n${details}`);
                        }
                    } catch (error) {
                        const msg = error instanceof Error ? error.message : String(error);
                        vscode.window.showErrorMessage(`Ship checks failed: ${msg}`);
                    }
                },
            );
        }),
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.generateCommitMessage', async () => {
            if (!restClient) {
                vscode.window.showErrorMessage('REST API not configured');
                return;
            }
            const projectRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: 'Generating commit message...',
                },
                async () => {
                    try {
                        const result = await restClient.generateCommitMessage(projectRoot);
                        // Copy to clipboard
                        await vscode.env.clipboard.writeText(result.message);
                        vscode.window.showInformationMessage(
                            `Commit message generated and copied to clipboard:\n\n${result.message}`,
                        );
                    } catch (error) {
                        const msg = error instanceof Error ? error.message : String(error);
                        vscode.window.showErrorMessage(`Failed to generate commit message: ${msg}`);
                    }
                },
            );
        }),
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
