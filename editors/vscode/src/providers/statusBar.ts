import * as vscode from 'vscode';
import { ClawdiusClient } from '../rpc/client';

export class StatusBarProvider implements vscode.Disposable {
    private statusBarItem: vscode.StatusBarItem;
    
    constructor(private client: ClawdiusClient) {
        this.statusBarItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Right,
            100
        );
        
        this.statusBarItem.command = 'clawdius.openChat';
        this.statusBarItem.text = '$(claw) Clawdius';
        this.statusBarItem.tooltip = 'Open Clawdius Chat';
    }
    
    public activate() {
        this.statusBarItem.show();
    }
    
    public dispose() {
        this.statusBarItem.dispose();
    }
    
    public setBusy(busy: boolean) {
        if (busy) {
            this.statusBarItem.text = '$(sync~spin) Clawdius';
        } else {
            this.statusBarItem.text = '$(claw) Clawdius';
        }
    }
    
    public showMessage(message: string) {
        this.statusBarItem.text = `$(claw) ${message}`;
        setTimeout(() => {
            this.statusBarItem.text = '$(claw) Clawdius';
        }, 3000);
    }
}
