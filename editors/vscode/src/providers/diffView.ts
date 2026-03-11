import * as vscode from 'vscode';
import { EventEmitter } from 'events';

export interface CodeChange {
    id: string;
    filePath: string;
    description: string;
    originalContent: string;
    modifiedContent: string;
    timestamp: number;
}

export class DiffViewProvider extends EventEmitter {
    private pendingChanges: CodeChange[] = [];
    private currentChangeIndex: number = -1;
    private statusBarItem: vscode.StatusBarItem;
    
    constructor() {
        super();
        
        this.statusBarItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Right,
            99
        );
        this.updateStatusBar();
    }
    
    async showDiff(change: CodeChange): Promise<void> {
        const originalUri = vscode.Uri.parse(`clawdius-diff:${change.filePath}?original`);
        const modifiedUri = vscode.Uri.parse(`clawdius-diff:${change.filePath}?modified`);
        
        const textDocumentContentProvider = new (class implements vscode.TextDocumentContentProvider {
            private change: CodeChange;
            
            constructor(change: CodeChange) {
                this.change = change;
            }
            
            provideTextDocumentContent(uri: vscode.Uri): string {
                const isOriginal = uri.query === 'original';
                return isOriginal ? this.change.originalContent : this.change.modifiedContent;
            }
        })(change);
        
        const disposable = vscode.workspace.registerTextDocumentContentProvider(
            'clawdius-diff',
            textDocumentContentProvider
        );
        
        try {
            await vscode.commands.executeCommand(
                'vscode.diff',
                originalUri,
                modifiedUri,
                `Diff: ${change.description} (${change.filePath})`,
                {
                    preview: true,
                    viewColumn: vscode.ViewColumn.Active
                }
            );
            
            this.currentChangeIndex = this.pendingChanges.findIndex(c => c.id === change.id);
            this.emit('diffShown', change);
        } finally {
            setTimeout(() => disposable.dispose(), 1000);
        }
    }
    
    async acceptChange(change: CodeChange): Promise<void> {
        try {
            const uri = vscode.Uri.file(change.filePath);
            const document = await vscode.workspace.openTextDocument(uri);
            const editor = await vscode.window.showTextDocument(document);
            
            await editor.edit(editBuilder => {
                const fullRange = new vscode.Range(
                    document.positionAt(0),
                    document.positionAt(document.getText().length)
                );
                editBuilder.replace(fullRange, change.modifiedContent);
            });
            
            this.removeChange(change.id);
            this.emit('changeAccepted', change);
            
            vscode.window.showInformationMessage(`Applied change: ${change.description}`);
            
            if (this.pendingChanges.length > 0) {
                const nextChange = this.pendingChanges[0];
                await this.showDiff(nextChange);
            }
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to apply change: ${error}`);
        }
    }
    
    async rejectChange(change: CodeChange): Promise<void> {
        this.removeChange(change.id);
        this.emit('changeRejected', change);
        
        vscode.window.showInformationMessage(`Rejected change: ${change.description}`);
        
        if (this.pendingChanges.length > 0) {
            const nextChange = this.pendingChanges[0];
            await this.showDiff(nextChange);
        } else {
            await vscode.commands.executeCommand('workbench.action.closeActiveEditor');
        }
    }
    
    addChange(change: CodeChange): void {
        const existing = this.pendingChanges.find(c => c.id === change.id);
        if (!existing) {
            this.pendingChanges.push(change);
            this.updateStatusBar();
            this.emit('changeAdded', change);
        }
    }
    
    removeChange(id: string): void {
        const index = this.pendingChanges.findIndex(c => c.id === id);
        if (index !== -1) {
            this.pendingChanges.splice(index, 1);
            if (this.currentChangeIndex >= this.pendingChanges.length) {
                this.currentChangeIndex = this.pendingChanges.length - 1;
            }
            this.updateStatusBar();
            this.emit('changeRemoved', id);
        }
    }
    
    getPendingChanges(): CodeChange[] {
        return [...this.pendingChanges];
    }
    
    getCurrentChange(): CodeChange | undefined {
        if (this.currentChangeIndex >= 0 && this.currentChangeIndex < this.pendingChanges.length) {
            return this.pendingChanges[this.currentChangeIndex];
        }
        return undefined;
    }
    
    async showAllChanges(): Promise<void> {
        if (this.pendingChanges.length === 0) {
            vscode.window.showInformationMessage('No pending changes to review');
            return;
        }
        
        const items = this.pendingChanges.map(change => ({
            label: change.filePath,
            description: change.description,
            detail: `${change.id} - ${new Date(change.timestamp).toLocaleString()}`,
            change
        }));
        
        const selected = await vscode.window.showQuickPick(items, {
            placeHolder: 'Select a change to review',
            matchOnDescription: true,
            matchOnDetail: true
        });
        
        if (selected) {
            await this.showDiff(selected.change);
        }
    }
    
    async acceptCurrentChange(): Promise<void> {
        const current = this.getCurrentChange();
        if (current) {
            await this.acceptChange(current);
        } else {
            vscode.window.showWarningMessage('No change currently selected');
        }
    }
    
    async rejectCurrentChange(): Promise<void> {
        const current = this.getCurrentChange();
        if (current) {
            await this.rejectChange(current);
        } else {
            vscode.window.showWarningMessage('No change currently selected');
        }
    }
    
    private updateStatusBar(): void {
        const count = this.pendingChanges.length;
        if (count > 0) {
            this.statusBarItem.text = `$(git-compare) ${count} change${count > 1 ? 's' : ''}`;
            this.statusBarItem.tooltip = `${count} pending change${count > 1 ? 's' : ''} to review`;
            this.statusBarItem.command = 'clawdius.showAllChanges';
            this.statusBarItem.show();
        } else {
            this.statusBarItem.hide();
        }
    }
    
    dispose(): void {
        this.statusBarItem.dispose();
        this.removeAllListeners();
    }
}
