/**
 * Clawdius VSCode Extension
 *
 * Communicates with the clawdius-code binary via JSON-RPC over stdio.
 * The Rust binary handles all heavy lifting (LLM calls, sandboxing, sessions).
 */

import {
  ExtensionContext,
  commands,
  window,
  workspace,
  StatusBarItem,
  StatusBarAlignment,
  OutputChannel,
  Disposable,
} from "vscode";

import { ChildProcess, spawn } from "child_process";

// --- JSON-RPC transport over stdio ---

interface JsonRpcRequest {
  jsonrpc: "2.0";
  id: number;
  method: string;
  params?: Record<string, unknown>;
}

interface JsonRpcResponse {
  jsonrpc: "2.0";
  id: number;
  result?: unknown;
  error?: { code: number; message: string; data?: unknown };
}

class ClawdiusClient implements Disposable {
  private proc: ChildProcess | null = null;
  private requestId = 0;
  private pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void }>();
  private buffer = "";
  private outputChannel: OutputChannel;

  constructor(private binaryPath: string) {
    this.outputChannel = window.createOutputChannel("Clawdius");
  }

  async start(): Promise<void> {
    if (this.proc && !this.proc.killed) {
      return;
    }

    return new Promise((resolve, reject) => {
      this.proc = spawn(this.binaryPath, ["--stdio"], {
        stdio: ["pipe", "pipe", "pipe"],
      });

      this.proc.stdout!.on("data", (data: Buffer) => {
        this.buffer += data.toString("utf-8");
        this.processBuffer();
      });

      this.proc.stderr!.on("data", (data: Buffer) => {
        this.outputChannel.append(data.toString("utf-8"));
      });

      this.proc.on("error", (err) => {
        reject(new Error(`Failed to start clawdius-code: ${err.message}`));
      });

      this.proc.on("exit", (code) => {
        if (code !== 0 && code !== null) {
          this.outputChannel.appendLine(`clawdius-code exited with code ${code}`);
        }
      });

      // Wait for server readiness
      this.request("initialize", { client: "vscode", version: "1.0.0" })
        .then(() => resolve())
        .catch(reject);
    });
  }

  async request(method: string, params?: Record<string, unknown>): Promise<unknown> {
    if (!this.proc || this.proc.killed) {
      throw new Error("Clawdius server not running");
    }

    const id = ++this.requestId;
    const msg: JsonRpcRequest = { jsonrpc: "2.0", id, method, params };

    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      const payload = `Content-Length: ${JSON.stringify(msg).length}\r\n\r\n${JSON.stringify(msg)}`;
      this.proc!.stdin!.write(payload);
    });
  }

  private processBuffer(): void {
    while (true) {
      const headerEnd = this.buffer.indexOf("\r\n\r\n");
      if (headerEnd === -1) break;

      const header = this.buffer.substring(0, headerEnd);
      const match = header.match(/Content-Length: (\d+)/);
      if (!match) break;

      const contentLength = parseInt(match[1], 10);
      const bodyStart = headerEnd + 4;
      if (this.buffer.length < bodyStart + contentLength) break;

      const body = this.buffer.substring(bodyStart, bodyStart + contentLength);
      this.buffer = this.buffer.substring(bodyStart + contentLength);

      try {
        const response: JsonRpcResponse = JSON.parse(body);
        const pending = this.pending.get(response.id);
        if (pending) {
          this.pending.delete(response.id);
          if (response.error) {
            pending.reject(new Error(response.error.message));
          } else {
            pending.resolve(response.result);
          }
        }
      } catch {
        // Ignore malformed JSON
      }
    }
  }

  stop(): void {
    if (this.proc && !this.proc.killed) {
      this.proc.kill("SIGTERM");
      this.proc = null;
    }
  }

  dispose(): void {
    this.stop();
    this.outputChannel.dispose();
    for (const [, pending] of this.pending) {
      pending.reject(new Error("Client disposed"));
    }
    this.pending.clear();
  }
}

// --- Extension activation ---

let client: ClawdiusClient | null = null;
let statusBarItem: StatusBarItem;

export async function activate(context: ExtensionContext): Promise<void> {
  statusBarItem = window.createStatusBarItem(StatusBarAlignment.Right, 100);
  statusBarItem.text = "$(robot) Clawdius";
  statusBarItem.tooltip = "Clawdius: Not Connected";
  statusBarItem.command = "clawdius.startServer";
  statusBarItem.show();

  const config = workspace.getConfiguration("clawdius");
  const binaryPath = config.get<string>("binaryPath", "clawdius-code");

  // Start server
  context.subscriptions.push(
    commands.registerCommand("clawdius.startServer", async () => {
      try {
        client = new ClawdiusClient(binaryPath);
        await client.start();
        statusBarItem.tooltip = "Clawdius: Connected";
        window.showInformationMessage("Clawdius server started.");
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        window.showErrorMessage(`Failed to start Clawdius: ${message}`);
      }
    })
  );

  // Stop server
  context.subscriptions.push(
    commands.registerCommand("clawdius.stopServer", () => {
      client?.stop();
      client = null;
      statusBarItem.tooltip = "Clawdius: Not Connected";
      window.showInformationMessage("Clawdius server stopped.");
    })
  );

  // Chat
  context.subscriptions.push(
    commands.registerCommand("clawdius.chat", async () => {
      const input = await window.showInputBox({
        prompt: "Clawdius Chat",
        placeHolder: "Enter your message...",
      });
      if (!input || !client) return;

      try {
        const result = await client.request("chat/send", { message: input });
        const text = typeof result === "object" && result !== null && "content" in result
          ? String((result as { content: string }).content)
          : JSON.stringify(result);
        window.showInformationMessage(text, { modal: false });
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        window.showErrorMessage(`Clawdius error: ${message}`);
      }
    })
  );

  // Sprint
  context.subscriptions.push(
    commands.registerCommand("clawdius.sprint", async () => {
      const task = await window.showInputBox({
        prompt: "Clawdius Sprint",
        placeHolder: "Describe the task...",
      });
      if (!task || !client) return;

      try {
        await client.request("sprint/start", { task });
        window.showInformationMessage(`Sprint started: ${task}`);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        window.showErrorMessage(`Sprint error: ${message}`);
      }
    })
  );

  // Analyze
  context.subscriptions.push(
    commands.registerCommand("clawdius.analyze", async () => {
      if (!client) {
        window.showErrorMessage("Clawdius server not running.");
        return;
      }
      try {
        const result = await client.request("analyze/run");
        const text = JSON.stringify(result, null, 2);
        const doc = await workspace.openTextDocument({ content: text, language: "json" });
        await window.showTextDocument(doc);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        window.showErrorMessage(`Analyze error: ${message}`);
      }
    })
  );

  // Verify
  context.subscriptions.push(
    commands.registerCommand("clawdius.verify", async () => {
      if (!client) {
        window.showErrorMessage("Clawdius server not running.");
        return;
      }
      try {
        const result = await client.request("verify/lean");
        const text = JSON.stringify(result, null, 2);
        window.showInformationMessage(`Verification complete: ${text}`);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        window.showErrorMessage(`Verify error: ${message}`);
      }
    })
  );

  // Checkpoint
  context.subscriptions.push(
    commands.registerCommand("clawdius.checkpoint", async () => {
      if (!client) {
        window.showErrorMessage("Clawdius server not running.");
        return;
      }
      try {
        await client.request("checkpoint/create");
        window.showInformationMessage("Checkpoint created.");
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        window.showErrorMessage(`Checkpoint error: ${message}`);
      }
    })
  );

  // Auto-start if configured
  if (config.get<boolean>("autoStart", false)) {
    commands.executeCommand("clawdius.startServer");
  }
}

export function deactivate(): void {
  client?.dispose();
  client = null;
  statusBarItem?.dispose();
}
