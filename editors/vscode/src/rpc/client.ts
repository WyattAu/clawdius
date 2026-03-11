import * as vscode from 'vscode';
import { spawn, ChildProcess } from 'child_process';
import * as path from 'path';
import * as fs from 'fs';
import { EventEmitter } from 'events';

interface JsonRpcRequest {
    jsonrpc: '2.0';
    id: number | string;
    method: string;
    params?: unknown;
}

interface JsonRpcResponse {
    jsonrpc: '2.0';
    id: number | string;
    result?: unknown;
    error?: {
        code: number;
        message: string;
        data?: unknown;
    };
}

interface PendingRequest {
    resolve: (value: unknown) => void;
    reject: (reason: Error) => void;
}

export class ClawdiusClient extends EventEmitter {
    private process: ChildProcess | null = null;
    private requestId = 0;
    private pending = new Map<number, PendingRequest>();
    private binaryPath: string;
    private isRunning = false;
    
    constructor() {
        super();
        this.binaryPath = this.findBinary();
    }
    
    private findBinary(): string {
        const config = vscode.workspace.getConfiguration('clawdius');
        const configPath = config.get<string>('binaryPath');
        
        if (configPath && fs.existsSync(configPath)) {
            return configPath;
        }
        
        const localPath = path.join(__dirname, '..', '..', 'bin', 'clawdius-code');
        if (fs.existsSync(localPath)) {
            return localPath;
        }
        
        const debugPath = path.join(__dirname, '..', '..', '..', '..', 'target', 'debug', 'clawdius-code');
        if (fs.existsSync(debugPath)) {
            return debugPath;
        }
        
        const releasePath = path.join(__dirname, '..', '..', '..', '..', 'target', 'release', 'clawdius-code');
        if (fs.existsSync(releasePath)) {
            return releasePath;
        }
        
        return 'clawdius-code';
    }
    
    async start(): Promise<void> {
        if (this.isRunning) {
            return;
        }
        
        return new Promise((resolve, reject) => {
            this.process = spawn(this.binaryPath, [], {
                stdio: ['pipe', 'pipe', 'pipe']
            });
            
            let buffer = '';
            
            this.process.stdout?.on('data', (data) => {
                buffer += data.toString();
                
                const lines = buffer.split('\n');
                buffer = lines.pop() || '';
                
                for (const line of lines) {
                    if (line.trim()) {
                        this.handleResponse(line);
                    }
                }
            });
            
            this.process.stderr?.on('data', (data) => {
                console.error('[clawdius-code]', data.toString());
            });
            
            this.process.on('error', (err) => {
                console.error('Failed to start clawdius-code:', err);
                this.isRunning = false;
                reject(err);
            });
            
            this.process.on('spawn', () => {
                console.log('clawdius-code started:', this.binaryPath);
                this.isRunning = true;
                resolve();
            });
            
            this.process.on('close', (code) => {
                console.log('clawdius-code exited with code:', code);
                this.isRunning = false;
                this.emit('close', code);
            });
        });
    }
    
    private handleResponse(line: string) {
        try {
            const response: JsonRpcResponse = JSON.parse(line);
            
            const pending = this.pending.get(response.id as number);
            if (pending) {
                this.pending.delete(response.id as number);
                
                if (response.error) {
                    pending.reject(new Error(response.error.message));
                } else {
                    pending.resolve(response.result);
                }
            } else {
                this.emit('notification', response);
            }
        } catch (e) {
            console.error('Failed to parse response:', e);
        }
    }
    
    async request<T>(method: string, params?: unknown): Promise<T> {
        if (!this.isRunning || !this.process) {
            throw new Error('Client not running. Call start() first.');
        }
        
        return new Promise((resolve, reject) => {
            const id = ++this.requestId;
            
            const request: JsonRpcRequest = {
                jsonrpc: '2.0',
                id,
                method,
                params
            };
            
            this.pending.set(id, { resolve: (v) => resolve(v as T), reject });
            
            const json = JSON.stringify(request) + '\n';
            this.process!.stdin?.write(json);
        });
    }
    
    // === Convenience Methods ===
    
    async chat(message: string, sessionId?: string): Promise<{ content: string }> {
        return this.request('chat/send', { message, sessionId });
    }
    
    async addContext(type: string, source: string): Promise<void> {
        return this.request('context/add', { type, source });
    }
    
    async createCheckpoint(description?: string): Promise<{ id: string }> {
        return this.request('state/checkpoint', { description });
    }
    
    async restoreCheckpoint(id: string): Promise<void> {
        return this.request('state/restore', { id });
    }
    
    async listSessions(): Promise<Array<{ id: string; title: string; updatedAt: string }>> {
        return this.request('session/list');
    }
    
    async loadSession(id: string): Promise<{ id: string; messages: Array<unknown> }> {
        return this.request('session/load', { id });
    }
    
    async readFile(path: string): Promise<{ content: string }> {
        return this.request('file/read', { path });
    }
    
    async writeFile(path: string, content: string): Promise<void> {
        return this.request('file/write', { path, content });
    }
    
    async executeTool(name: string, args: Record<string, unknown>): Promise<unknown> {
        return this.request('tool/execute', { name, arguments: args });
    }
    
    async inlineCompletion(params: {
        prefix: string;
        suffix?: string;
        language: string;
        file_path: string;
        line?: number;
        character?: number;
    }): Promise<{ text: string }> {
        return this.request('completion/inline', params);
    }
    
    async getConfig(): Promise<Record<string, unknown>> {
        return this.request('config/get');
    }
    
    async setConfig(key: string, value: unknown): Promise<void> {
        return this.request('config/set', { key, value });
    }
    
    async getCodeActions(context: {
        document: string;
        language: string;
        position: { line: number; column: number };
        selection?: string;
        symbolAtPosition?: {
            name: string;
            kind: string;
            range: {
                start: { line: number; column: number };
                end: { line: number; column: number };
            };
        };
    }): Promise<Array<{
        id: string;
        title: string;
        kind: string;
        edits: Array<{
            range: {
                start: { line: number; column: number };
                end: { line: number; column: number };
            };
            newText: string;
        }>;
    }>> {
        return this.request('actions/get', context);
    }
    
    async shutdown(): Promise<void> {
        return this.request('shutdown');
    }
    
    async stop(): Promise<void> {
        if (!this.process) {
            return;
        }
        
        try {
            await this.shutdown();
        } catch {
            // Ignore shutdown errors
        }
        
        this.process.kill();
        this.process = null;
        this.isRunning = false;
        this.pending.clear();
    }
    
    dispose() {
        this.stop();
    }
    
    reloadConfiguration() {
        this.binaryPath = this.findBinary();
    }
    
    get running(): boolean {
        return this.isRunning;
    }
}
