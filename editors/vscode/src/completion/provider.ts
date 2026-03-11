import * as vscode from 'vscode';
import { ClawdiusClient } from '../rpc/client';

interface CompletionResponse {
    text: string;
}

export class ClawdiusCompletionProvider implements vscode.InlineCompletionItemProvider {
    private client: ClawdiusClient;
    private debounceTimer: NodeJS.Timeout | undefined;
    private lastCompletionTime: number = 0;
    
    constructor(client: ClawdiusClient) {
        this.client = client;
    }
    
    async provideInlineCompletionItems(
        document: vscode.TextDocument,
        position: vscode.Position,
        context: vscode.InlineCompletionContext,
        token: vscode.CancellationToken
    ): Promise<vscode.InlineCompletionItem[] | vscode.InlineCompletionList | undefined> {
        const config = vscode.workspace.getConfiguration('clawdius.completion');
        const enabled = config.get<boolean>('enabled', true);
        
        if (!enabled) {
            return undefined;
        }
        
        const debounceDelay = config.get<number>('debounceDelay', 300);
        
        await this.debounce(debounceDelay);
        
        if (token.isCancellationRequested) {
            return undefined;
        }
        
        const prefix = document.getText(
            new vscode.Range(new vscode.Position(0, 0), position)
        );
        
        const suffix = document.getText(
            new vscode.Range(position, document.lineAt(document.lineCount - 1).range.end)
        );
        
        const lineText = document.lineAt(position).text;
        const cursorPosition = position.character;
        const textBeforeCursor = lineText.substring(0, cursorPosition);
        
        if (this.shouldSkipCompletion(textBeforeCursor)) {
            return undefined;
        }
        
        try {
            const completion = await this.client.request<CompletionResponse>('completion/inline', {
                prefix,
                suffix,
                language: document.languageId,
                file_path: document.uri.fsPath,
                line: position.line,
                character: position.character,
            });
            
            if (!completion || !completion.text || token.isCancellationRequested) {
                return undefined;
            }
            
            return [
                new vscode.InlineCompletionItem(
                    completion.text,
                    new vscode.Range(position, position)
                )
            ];
        } catch (error) {
            console.error('Completion error:', error);
            return undefined;
        }
    }
    
    private shouldSkipCompletion(textBeforeCursor: string): boolean {
        if (textBeforeCursor.trim().length === 0) {
            return true;
        }
        
        const lastChar = textBeforeCursor.slice(-1);
        const config = vscode.workspace.getConfiguration('clawdius.completion');
        const triggerCharacters = config.get<string[]>('triggerCharacters', ['.', '(', ' ']);
        
        if (triggerCharacters.length > 0 && !triggerCharacters.includes(lastChar)) {
            return true;
        }
        
        return false;
    }
    
    private debounce(ms: number): Promise<void> {
        return new Promise(resolve => {
            if (this.debounceTimer) {
                clearTimeout(this.debounceTimer);
            }
            
            this.debounceTimer = setTimeout(() => {
                resolve();
            }, ms);
        });
    }
}
