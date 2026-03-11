import * as vscode from 'vscode';
import { ClawdiusClient } from '../rpc/client';
import { DiffViewProvider, CodeChange } from './diffView';

export class ChatViewProvider implements vscode.WebviewViewProvider {
    private _view: vscode.WebviewView | undefined;
    
    constructor(
        private readonly _extensionContext: vscode.ExtensionContext,
        private readonly _client: ClawdiusClient,
        private readonly _diffView: DiffViewProvider
    ) {}
    
    public resolveWebviewView(
        webviewView: vscode.WebviewView,
        context: vscode.WebviewViewResolveContext,
        token: vscode.CancellationToken
    ): void {
        this._view = webviewView;
        
        webviewView.webview.options = {
            enableScripts: true,
            localResourceRoots: [
                this._extensionContext.extensionUri
            ]
        };
        
        webviewView.webview.html = this.getHtml();
        
        // Handle messages from webview
        webviewView.webview.onDidReceiveMessage(async (message) => {
            switch (message.command) {
                case 'send':
                    try {
                        const response = await this._client.chat(message.text);
                        webviewView.webview.postMessage({
                            command: 'response',
                            text: response.content
                        });
                    } catch (error) {
                        webviewView.webview.postMessage({
                            command: 'error',
                            message: String(error)
                        });
                    }
                    break;
                    
                case 'addContext':
                    await this._client.addContext(message.type, message.source);
                    webviewView.webview.postMessage({
                        command: 'contextAdded',
                        type: message.type,
                        source: message.source
                    });
                    break;
                    
                case 'checkpoint':
                    {
                        const checkpoint = await this._client.createCheckpoint(message.description);
                        webviewView.webview.postMessage({
                            command: 'checkpointCreated',
                            id: checkpoint.id
                        });
                    }
                    break;
                    
                case 'showDiff':
                    if (message.change) {
                        await this._diffView.showDiff(message.change);
                    }
                    break;
                    
                case 'acceptChange':
                    if (message.change) {
                        await this._diffView.acceptChange(message.change);
                        webviewView.webview.postMessage({
                            command: 'changeAccepted',
                            id: message.change.id
                        });
                    }
                    break;
                    
                case 'rejectChange':
                    if (message.change) {
                        await this._diffView.rejectChange(message.change);
                        webviewView.webview.postMessage({
                            command: 'changeRejected',
                            id: message.change.id
                        });
                    }
                    break;
                    
                case 'showAllChanges':
                    await this._diffView.showAllChanges();
                    break;
            }
        });
    }
    
    private getHtml(): string {
        return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Clawdius Chat</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        
        body {
            font-family: var(--vscode-font-family);
            background-color: var(--vscode-editor-background);
            color: var(--vscode-editor-foreground);
            height: 100vh;
            display: flex;
            flex-direction: column;
        }
        
        #messages {
            flex: 1;
            overflow-y: auto;
            padding: 10px;
        }
        
        .message {
            margin-bottom: 10px;
            padding: 10px;
            border-radius: 5px;
        }
        
        .message.user {
            background-color: var(--vscode-input-background);
            margin-left: 20%;
        }
        
        .message.assistant {
            background-color: var(--vscode-editor-inactiveSelectionBackground);
            margin-right: 20%;
        }
        
        .message.error {
            background-color: var(--vscode-inputValidation-errorBackground);
            color: var(--vscode-inputValidation-errorForeground);
        }
        
        .message.code-change {
            background-color: var(--vscode-editor-inactiveSelectionBackground);
            border-left: 3px solid var(--vscode-charts-green);
        }
        
        .message .diff-actions {
            display: flex;
            gap: 5px;
            margin-top: 8px;
        }
        
        .message .diff-actions button {
            padding: 4px 12px;
            border: none;
            border-radius: 3px;
            cursor: pointer;
            font-size: 12px;
        }
        
        .message .diff-actions .show-diff-btn {
            background-color: var(--vscode-button-background);
            color: var(--vscode-button-foreground);
        }
        
        .message .diff-actions .accept-btn {
            background-color: var(--vscode-charts-green);
            color: white;
        }
        
        .message .diff-actions .reject-btn {
            background-color: var(--vscode-charts-red);
            color: white;
        }
        
        #input-area {
            display: flex;
            padding: 10px;
            border-top: 1px solid var(--vscode-panel-border);
        }
        
        #input {
            flex: 1;
            padding: 10px;
            border: 1px solid var(--vscode-input-border);
            background-color: var(--vscode-input-background);
            color: var(--vscode-input-foreground);
            border-radius: 5px;
            font-family: var(--vscode-editor-font-family);
            font-size: var(--vscode-editor-font-size);
            resize: none;
        }
        
        #input:focus {
            outline: none;
            border-color: var(--vscode-focusBorder);
        }
        
        #send-btn {
            margin-left: 10px;
            padding: 10px 20px;
            background-color: var(--vscode-button-background);
            color: var(--vscode-button-foreground);
            border: none;
            border-radius: 5px;
            cursor: pointer;
        }
        
        #send-btn:hover {
            background-color: var(--vscode-button-hoverBackground);
        }
        
        .toolbar {
            display: flex;
            gap: 5px;
            padding: 5px 10px;
            border-bottom: 1px solid var(--vscode-panel-border);
        }
        
        .toolbar button {
            padding: 5px 10px;
            background-color: var(--vscode-button-secondaryBackground);
            color: var(--vscode-button-secondaryForeground);
            border: none;
            border-radius: 3px;
            cursor: pointer;
            font-size: 12px;
        }
        
        .toolbar button:hover {
            background-color: var(--vscode-button-secondaryHoverBackground);
        }
        
        .pending-badge {
            display: inline-block;
            margin-left: 5px;
            padding: 2px 6px;
            background-color: var(--vscode-charts-orange);
            color: white;
            border-radius: 10px;
            font-size: 10px;
        }
    </style>
</head>
<body>
    <div class="toolbar">
        <button id="add-file-btn">+ Add File</button>
        <button id="checkpoint-btn">📍 Checkpoint</button>
        <button id="clear-btn">🗑️ Clear</button>
        <button id="show-changes-btn">📋 Show Changes</button>
    </div>
    
    <div id="messages"></div>
    
    <div id="input-area">
        <textarea id="input" placeholder="Type your message... (Shift+Enter to send)" rows="3"></textarea>
        <button id="send-btn">Send</button>
    </div>
    
    <script>
        const vscode = acquireVsCodeApi();
        const messagesDiv = document.getElementById('messages');
        const input = document.getElementById('input');
        const sendBtn = document.getElementById('send-btn');
        const addFileBtn = document.getElementById('add-file-btn');
        const checkpointBtn = document.getElementById('checkpoint-btn');
        const clearBtn = document.getElementById('clear-btn');
        const showChangesBtn = document.getElementById('show-changes-btn');
        
        let pendingChanges = [];
        
        function addMessage(role, content, change = null) {
            const msgDiv = document.createElement('div');
            msgDiv.className = 'message ' + role;
            
            if (change) {
                msgDiv.classList.add('code-change');
                msgDiv.innerHTML = '<div>' + escapeHtml(content) + '</div>';
                
                const actionsDiv = document.createElement('div');
                actionsDiv.className = 'diff-actions';
                
                const showDiffBtn = document.createElement('button');
                showDiffBtn.className = 'show-diff-btn';
                showDiffBtn.textContent = '👁️ Show Diff';
                showDiffBtn.onclick = () => {
                    vscode.postMessage({
                        command: 'showDiff',
                        change: change
                    });
                };
                actionsDiv.appendChild(showDiffBtn);
                
                const acceptBtn = document.createElement('button');
                acceptBtn.className = 'accept-btn';
                acceptBtn.textContent = '✓ Accept';
                acceptBtn.onclick = () => {
                    vscode.postMessage({
                        command: 'acceptChange',
                        change: change
                    });
                    msgDiv.remove();
                    removePendingChange(change.id);
                };
                actionsDiv.appendChild(acceptBtn);
                
                const rejectBtn = document.createElement('button');
                rejectBtn.className = 'reject-btn';
                rejectBtn.textContent = '✗ Reject';
                rejectBtn.onclick = () => {
                    vscode.postMessage({
                        command: 'rejectChange',
                        change: change
                    });
                    msgDiv.remove();
                    removePendingChange(change.id);
                };
                actionsDiv.appendChild(rejectBtn);
                
                msgDiv.appendChild(actionsDiv);
            } else {
                msgDiv.textContent = content;
            }
            
            messagesDiv.appendChild(msgDiv);
            messagesDiv.scrollTop = messagesDiv.scrollHeight;
        }
        
        function escapeHtml(text) {
            const div = document.createElement('div');
            div.textContent = text;
            return div.innerHTML;
        }
        
        function addPendingChange(change) {
            pendingChanges.push(change);
            updateChangesButton();
        }
        
        function removePendingChange(id) {
            pendingChanges = pendingChanges.filter(c => c.id !== id);
            updateChangesButton();
        }
        
        function updateChangesButton() {
            if (pendingChanges.length > 0) {
                showChangesBtn.innerHTML = '📋 Show Changes <span class="pending-badge">' + pendingChanges.length + '</span>';
            } else {
                showChangesBtn.textContent = '📋 Show Changes';
            }
        }
        
        function sendMessage() {
            const text = input.value.trim();
            if (!text) return;
            
            addMessage('user', text);
            input.value = '';
            
            vscode.postMessage({
                command: 'send',
                text: text
            });
        }
        
        sendBtn.addEventListener('click', sendMessage);
        
        input.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && e.shiftKey) {
                e.preventDefault();
                sendMessage();
            }
        });
        
        addFileBtn.addEventListener('click', async () => {
            const file = await vscode.window.showOpenDialog({
                canSelectMany: false,
                filters: { 'All files': ['*'] }
            });
            if (file && file[0]) {
                vscode.postMessage({
                    command: 'addContext',
                    type: 'file',
                    source: file[0].fsPath
                });
                addMessage('system', '📄 Added: ' + file[0].fsPath);
            }
        });
        
        checkpointBtn.addEventListener('click', async () => {
            const description = await vscode.window.showInputBox({
                prompt: 'Checkpoint description'
            });
            if (description !== undefined) {
                vscode.postMessage({
                    command: 'checkpoint',
                    description: description
                });
                addMessage('system', '📍 Checkpoint created');
            }
        });
        
        clearBtn.addEventListener('click', () => {
            messagesDiv.innerHTML = '';
        });
        
        showChangesBtn.addEventListener('click', () => {
            vscode.postMessage({
                command: 'showAllChanges'
            });
        });
        
        // Handle messages from extension
        window.addEventListener('message', event => {
            const message = event.data;
            
            switch (message.command) {
                case 'response':
                    addMessage('assistant', message.text);
                    break;
                case 'codeChange':
                    const change = message.change;
                    addPendingChange(change);
                    addMessage('assistant', '📝 Proposed change: ' + change.description + ' (' + change.filePath + ')', change);
                    break;
                case 'error':
                    addMessage('error', message.message);
                    break;
                case 'contextAdded':
                    addMessage('system', '✓ Context added: ' + message.source);
                    break;
                case 'checkpointCreated':
                    addMessage('system', '✓ Checkpoint: ' + message.id);
                    break;
                case 'changeAccepted':
                    removePendingChange(message.id);
                    break;
                case 'changeRejected':
                    removePendingChange(message.id);
                    break;
            }
        });
    </script>
</body>
</html>`;
    }
}
