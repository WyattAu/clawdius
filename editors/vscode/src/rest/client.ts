/**
 * REST API client for Clawdius server.
 * 
 * Connects to the Clawdius REST API (started via `clawdius serve`) 
 * and provides typed methods for all API endpoints.
 */

import * as vscode from 'vscode';

// ── Types ──────────────────────────────────────────────────────────────

export interface ChatRequest {
    message: string;
    session_id?: string;
    model?: string;
    stream?: boolean;
}

export interface ChatResponse {
    response: string;
    session_id: string;
    tokens_used?: number;
    model?: string;
}

export interface SprintRequest {
    task: string;
    max_iterations?: number;
    real_execution?: boolean;
    auto_approve?: boolean;
    model?: string;
    provider?: string;
    project_root?: string;
    browser_qa_url?: string;
}

export interface PhaseResult {
    phase: string;
    status: string;
    duration_ms: number;
    tokens_used: number;
    output: string;
    files_modified: string[];
    errors: string[];
}

export interface SprintResult {
    success: boolean;
    phase_results: PhaseResult[];
    total_duration_ms: number;
    summary: string;
    checkpoint_ref?: string;
    rollback_available: boolean;
    metrics: {
        total_tokens: number;
        retry_cycles: number;
        phases_succeeded: number;
        phases_failed: number;
    };
}

export interface SkillInfo {
    name: string;
    description: string;
    version: string;
    tags: string[];
}

export interface ToolDefinition {
    name: string;
    description: string;
    input_schema: Record<string, unknown>;
}

export interface ShipCheckResult {
    check_name: string;
    passed: boolean;
    message: string;
    severity: string;
}

// ── REST Client ────────────────────────────────────────────────────────

export class ClawdiusRestClient {
    private baseUrl: string;
    private outputChannel: vscode.OutputChannel;

    constructor(serverUrl: string, outputChannel: vscode.OutputChannel) {
        this.baseUrl = serverUrl.replace(/\/$/, ''); // strip trailing slash
        this.outputChannel = outputChannel;
    }

    private async request<T>(
        method: string,
        path: string,
        body?: unknown,
    ): Promise<T> {
        const url = `${this.baseUrl}${path}`;
        const headers: Record<string, string> = {
            'Content-Type': 'application/json',
        };

        // Add API key if configured
        const apiKey = vscode.workspace.getConfiguration('clawdius').get<string>('apiKey');
        if (apiKey) {
            headers['Authorization'] = `Bearer ${apiKey}`;
        }

        try {
            const response = await fetch(url, {
                method,
                headers,
                body: body ? JSON.stringify(body) : undefined,
            });

            if (!response.ok) {
                const errorText = await response.text();
                throw new Error(`HTTP ${response.status}: ${errorText}`);
            }

            return (await response.json()) as T;
        } catch (error) {
            const msg = error instanceof Error ? error.message : String(error);
            this.outputChannel.appendLine(`[Clawdius REST] ${method} ${path} failed: ${msg}`);
            throw error;
        }
    }

    // ── Health & Readiness ─────────────────────────────────────────────

    async healthCheck(): Promise<boolean> {
        try {
            const result = await this.request<{ status: string }>('GET', '/api/v1/health');
            return result.status === 'ok';
        } catch {
            return false;
        }
    }

    async readinessCheck(): Promise<boolean> {
        try {
            const result = await this.request<{ ready: boolean }>('GET', '/api/v1/ready');
            return result.ready;
        } catch {
            return false;
        }
    }

    // ── Chat ───────────────────────────────────────────────────────────

    async chat(request: ChatRequest): Promise<ChatResponse> {
        return this.request<ChatResponse>('POST', '/api/v1/chat', request);
    }

    // ── Sprint ─────────────────────────────────────────────────────────

    async runSprint(request: SprintRequest): Promise<SprintResult> {
        return this.request<SprintResult>('POST', '/api/v1/sprint', request);
    }

    async listSprintSessions(): Promise<unknown> {
        return this.request<{ sessions: unknown[]; summary: unknown }>(
            'GET',
            '/api/v1/sprint/sessions',
        );
    }

    async submitSprintSession(config: {
        task: string;
        project_root?: string;
        priority?: number;
    }): Promise<{ session_id: string }> {
        return this.request<{ session_id: string }>(
            'POST',
            '/api/v1/sprint/sessions',
            config,
        );
    }

    // ── Skills ─────────────────────────────────────────────────────────

    async listSkills(): Promise<SkillInfo[]> {
        return this.request<SkillInfo[]>('GET', '/api/v1/skills');
    }

    async executeSkill(
        name: string,
        args?: Record<string, string>,
        selection?: string,
        projectRoot?: string,
    ): Promise<{ result: string; success: boolean }> {
        return this.request<{ result: string; success: boolean }>(
            'POST',
            '/api/v1/skills/execute',
            { name, args, selection, project_root: projectRoot },
        );
    }

    // ── Tools ──────────────────────────────────────────────────────────

    async listTools(): Promise<ToolDefinition[]> {
        return this.request<ToolDefinition[]>('GET', '/api/v1/tools');
    }

    async executeTool(
        name: string,
        args: Record<string, unknown>,
    ): Promise<{ result: string; success: boolean }> {
        return this.request<{ result: string; success: boolean }>(
            'POST',
            '/api/v1/tools/execute',
            { name, args },
        );
    }

    // ── Ship ───────────────────────────────────────────────────────────

    async runShipChecks(projectRoot?: string): Promise<ShipCheckResult[]> {
        return this.request<ShipCheckResult[]>('POST', '/api/v1/ship/checks', {
            project_root: projectRoot,
        });
    }

    async generateCommitMessage(
        projectRoot?: string,
    ): Promise<{ message: string }> {
        return this.request<{ message: string }>(
            'POST',
            '/api/v1/ship/commit-message',
            { project_root: projectRoot },
        );
    }

    // ── Sessions ───────────────────────────────────────────────────────

    async listSessions(): Promise<unknown[]> {
        const result = await this.request<{ sessions: unknown[] }>(
            'GET',
            '/api/v1/sessions',
        );
        return result.sessions;
    }

    async deleteSession(id: string): Promise<void> {
        await this.request('DELETE', `/api/v1/sessions/${encodeURIComponent(id)}`);
    }

    // ── Usage ──────────────────────────────────────────────────────────

    async getUsage(): Promise<{
        total_requests: number;
        total_tokens: number;
        sessions: number;
    }> {
        return this.request('GET', '/api/v1/usage');
    }
}
