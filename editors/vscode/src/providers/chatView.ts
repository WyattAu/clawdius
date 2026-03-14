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
    <!-- Highlight.js for syntax highlighting -->
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/vs2015.min.css">
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js"></script>
    <!-- Marked.js for markdown rendering -->
    <script src="https://cdnjs.cloudflare.com/ajax/libs/marked/9.1.6/marked.min.js"></script>
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
        
        .message.system {
            background-color: var(--vscode-editorInfo-foreground);
            color: var(--vscode-editor-background);
            opacity: 0.8;
            font-size: 0.9em;
            text-align: center;
            margin: 5px 30%;
        }
        
        .message.error {
            background-color: var(--vscode-inputValidation-errorBackground);
            color: var(--vscode-inputValidation-errorForeground);
        }
        
        .message.code-change {
            background-color: var(--vscode-editor-inactiveSelectionBackground);
            border-left: 3px solid var(--vscode-charts-green);
        }
        
        /* Markdown content styles */
        .message h1, .message h2, .message h3 {
            margin-top: 0.5em;
            margin-bottom: 0.5em;
        }
        
        .message p {
            margin-bottom: 0.5em;
        }
        
        .message pre {
            background-color: var(--vscode-textCodeBlock-background);
            padding: 10px;
            border-radius: 5px;
            overflow-x: auto;
            margin: 10px 0;
        }
        
        .message code {
            font-family: var(--vscode-editor-font-family);
            font-size: var(--vscode-editor-font-size);
        }
        
        .message p code {
            background-color: var(--vscode-textCodeBlock-background);
            padding: 2px 5px;
            border-radius: 3px;
        }
        
        .message ul, .message ol {
            margin-left: 1.5em;
            margin-bottom: 0.5em;
        }
        
        .message li {
            margin-bottom: 0.25em;
        }
        
        .message a {
            color: var(--vscode-textLink-foreground);
        }
        
        .message a:hover {
            color: var(--vscode-textLink-activeForeground);
        }
        
        .message blockquote {
            border-left: 3px solid var(--vscode-panel-border);
            margin-left: 0;
            padding-left: 10px;
            color: var(--vscode-descriptionForeground);
        }
        
        .message table {
            border-collapse: collapse;
            width: 100%;
            margin: 10px 0;
        }
        
        .message th, .message td {
            border: 1px solid var(--vscode-panel-border);
            padding: 5px 10px;
        }
        
        .message th {
            background-color: var(--vscode-editor-inactiveSelectionBackground);
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
        
        /* Copy button for code blocks */
        .code-block-wrapper {
            position: relative;
        }
        
        .copy-btn {
            position: absolute;
            top: 5px;
            right: 5px;
            padding: 4px 8px;
            background-color: var(--vscode-button-secondaryBackground);
            color: var(--vscode-button-secondaryForeground);
            border: none;
            border-radius: 3px;
            cursor: pointer;
            font-size: 11px;
            opacity: 0;
            transition: opacity 0.2s;
        }
        
        .code-block-wrapper:hover .copy-btn {
            opacity: 1;
        }
        
        .copy-btn:hover {
            background-color: var(--vscode-button-secondaryHoverBackground);
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
        
        /* Thinking indicator */
        .thinking {
            display: flex;
            align-items: center;
            gap: 5px;
            padding: 10px;
            color: var(--vscode-descriptionForeground);
            font-style: italic;
        }
        
        .thinking::after {
            content: '';
            width: 8px;
            height: 8px;
            border-radius: 50%;
            background-color: var(--vscode-charts-blue);
            animation: pulse 1s infinite;
        }
        
        @keyframes pulse {
            0%, 100% { opacity: 0.3; }
            50% { opacity: 1; }
        }
        
        /* Streaming text */
        .streaming {
            position: relative;
        }
        
        .streaming::after {
            content: '▊';
            animation: blink 1s infinite;
        }
        
        @keyframes blink {
            0%, 50% { opacity: 1; }
            51%, 100% { opacity: 0; }
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
        let currentStreamingElement = null;
        
        // Configure marked
        marked.setOptions({
            highlight: function(code, lang) {
                if (lang && hl.getLanguage(lang)) {
                    try {
                        return hl.highlight(code, { language: lang }).value;
                    } catch (e) {}
                }
                return hl.highlightAuto(code).value;
            },
            breaks: true,
            gfm: true
        });
        
        function renderMarkdown(text) {
            return marked.parse(text);
        }
        
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
            } else if (role === 'assistant') {
                // Render markdown for assistant messages
                const contentDiv = document.createElement('div');
                contentDiv.innerHTML = renderMarkdown(content);
                
                // Add copy buttons to code blocks
                contentDiv.querySelectorAll('pre').forEach(pre => {
                    const wrapper = document.createElement('div');
                    wrapper.className = 'code-block-wrapper';
                    pre.parentNode.insertBefore(wrapper, pre);
                    wrapper.appendChild(pre);
                    
                    const copyBtn = document.createElement('button');
                    copyBtn.className = 'copy-btn';
                    copyBtn.textContent = 'Copy';
                    copyBtn.onclick = () => {
                        const code = pre.textContent;
                        navigator.clipboard.writeText(code).then(() => {
                            copyBtn.textContent = 'Copied!';
                            setTimeout(() => copyBtn.textContent = 'Copy', 2000);
                        });
                    };
                    wrapper.appendChild(copyBtn);
                });
                
                msgDiv.appendChild(contentDiv);
            } else {
                msgDiv.innerHTML = renderMarkdown(content);
            }
            
            messagesDiv.appendChild(msgDiv);
            messagesDiv.scrollTop = messagesDiv.scrollHeight;
            return msgDiv;
        }
        
        function showThinking() {
            const thinkingDiv = document.createElement('div');
            thinkingDiv.className = 'message assistant thinking';
            thinkingDiv.id = 'thinking-indicator';
            thinkingDiv.textContent = 'Thinking';
            messagesDiv.appendChild(thinkingDiv);
            messagesDiv.scrollTop = messagesDiv.scrollHeight;
        }
        
        function hideThinking() {
            const thinkingDiv = document.getElementById('thinking-indicator');
            if (thinkingDiv) {
                thinkingDiv.remove();
            }
        }
        
        function startStreaming() {
            hideThinking();
            currentStreamingElement = addMessage('assistant', '');
            currentStreamingElement.classList.add('streaming');
        }
        
        function appendStream(text) {
            if (currentStreamingElement) {
                const contentDiv = currentStreamingElement.querySelector('div');
                if (contentDiv) {
                    contentDiv.innerHTML = renderMarkdown(text);
                }
                messagesDiv.scrollTop = messagesDiv.scrollHeight;
            }
        }
        
        function endStreaming() {
            if (currentStreamingElement) {
                currentStreamingElement.classList.remove('streaming');
                currentStreamingElement = null;
            }
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
            
            showThinking();
            
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
                    hideThinking();
                    addMessage('assistant', message.text);
                    break;
                case 'streamStart':
                    startStreaming();
                    break;
                case 'streamToken':
                    appendStream(message.text);
                    break;
                case 'streamEnd':
                    endStreaming();
                    break;
                case 'codeChange':
                    const change = message.change;
                    addPendingChange(change);
                    addMessage('assistant', '📝 Proposed change: ' + change.description + ' (' + change.filePath + ')', change);
                    break;
                case 'error':
                    hideThinking();
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
