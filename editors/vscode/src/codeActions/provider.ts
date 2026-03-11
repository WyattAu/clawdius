import * as vscode from 'vscode';
import { ClawdiusClient } from '../rpc/client';

export interface CodeActionContext {
    document: string;
    language: string;
    position: { line: number; column: number };
    selection?: string;
    symbolAtPosition?: CodeSymbol;
}

export interface CodeSymbol {
    name: string;
    kind: SymbolKind;
    range: {
        start: { line: number; column: number };
        end: { line: number; column: number };
    };
}

export enum SymbolKind {
    Function = 'Function',
    Variable = 'Variable',
    Class = 'Class',
    Module = 'Module',
    Trait = 'Trait',
    Struct = 'Struct',
    Enum = 'Enum',
    Method = 'Method',
    Property = 'Property',
}

export interface CodeActionResult {
    id: string;
    title: string;
    kind: 'QuickFix' | 'Refactor' | 'Source';
    edits: TextEdit[];
}

export interface TextEdit {
    range: {
        start: { line: number; column: number };
        end: { line: number; column: number };
    };
    newText: string;
}

export class ClawdiusCodeActionProvider implements vscode.CodeActionProvider {
    constructor(private client: ClawdiusClient) {}

    async provideCodeActions(
        document: vscode.TextDocument,
        range: vscode.Range,
        _context: vscode.CodeActionContext
    ): Promise<(vscode.CodeAction | vscode.Command)[] | undefined> {
        try {
            const actionContext = await this.buildActionContext(document, range);
            const actions = await this.client.getCodeActions(actionContext);
            
            return actions.map(action => this.toVSCodeAction(action as CodeActionResult, document));
        } catch (error) {
            console.error('Failed to get code actions:', error);
            return [];
        }
    }

    private async buildActionContext(
        document: vscode.TextDocument,
        range: vscode.Range
    ): Promise<CodeActionContext> {
        const selection = range.isEmpty ? undefined : document.getText(range);
        
        const symbolAtPosition = await this.getSymbolAtPosition(document, range.start);
        
        return {
            document: document.getText(),
            language: this.normalizeLanguage(document.languageId),
            position: {
                line: range.start.line,
                column: range.start.character,
            },
            selection,
            symbolAtPosition,
        };
    }

    private async getSymbolAtPosition(
        document: vscode.TextDocument,
        position: vscode.Position
    ): Promise<CodeSymbol | undefined> {
        try {
            const symbols = await vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
                'vscode.executeDocumentSymbolProvider',
                document.uri
            );

            if (!symbols) {
                return undefined;
            }

            const findSymbol = (syms: vscode.DocumentSymbol[]): CodeSymbol | undefined => {
                for (const sym of syms) {
                    if (sym.range.contains(position)) {
                        const symbol: CodeSymbol = {
                            name: sym.name,
                            kind: this.toSymbolKind(sym.kind),
                            range: {
                                start: {
                                    line: sym.range.start.line,
                                    column: sym.range.start.character,
                                },
                                end: {
                                    line: sym.range.end.line,
                                    column: sym.range.end.character,
                                },
                            },
                        };
                        
                        if (sym.children.length > 0) {
                            const childSymbol = findSymbol(sym.children);
                            if (childSymbol) {
                                return childSymbol;
                            }
                        }
                        
                        return symbol;
                    }
                }
                return undefined;
            };

            return findSymbol(symbols);
        } catch {
            return undefined;
        }
    }

    private toSymbolKind(kind: vscode.SymbolKind): SymbolKind {
        switch (kind) {
            case vscode.SymbolKind.Function:
                return SymbolKind.Function;
            case vscode.SymbolKind.Variable:
                return SymbolKind.Variable;
            case vscode.SymbolKind.Class:
                return SymbolKind.Class;
            case vscode.SymbolKind.Module:
                return SymbolKind.Module;
            case vscode.SymbolKind.Interface:
                return SymbolKind.Trait;
            case vscode.SymbolKind.Struct:
                return SymbolKind.Struct;
            case vscode.SymbolKind.Enum:
                return SymbolKind.Enum;
            case vscode.SymbolKind.Method:
                return SymbolKind.Method;
            case vscode.SymbolKind.Property:
                return SymbolKind.Property;
            default:
                return SymbolKind.Variable;
        }
    }

    private normalizeLanguage(languageId: string): string {
        const languageMap: Record<string, string> = {
            'typescriptreact': 'typescript',
            'javascriptreact': 'javascript',
            'rust': 'rust',
            'python': 'python',
        };
        
        return languageMap[languageId] || languageId;
    }

    private toVSCodeAction(action: CodeActionResult, document: vscode.TextDocument): vscode.CodeAction {
        const vscodeAction = new vscode.CodeAction(
            action.title,
            this.toVSCodeActionKind(action.kind)
        );

        const edits = action.edits.map(edit => {
            const range = new vscode.Range(
                new vscode.Position(edit.range.start.line, edit.range.start.column),
                new vscode.Position(edit.range.end.line, edit.range.end.column)
            );
            return vscode.TextEdit.replace(range, edit.newText);
        });

        const workspaceEdit = new vscode.WorkspaceEdit();
        workspaceEdit.set(document.uri, edits);
        vscodeAction.edit = workspaceEdit;

        return vscodeAction;
    }

    private toVSCodeActionKind(kind: string): vscode.CodeActionKind {
        switch (kind) {
            case 'QuickFix':
                return vscode.CodeActionKind.QuickFix;
            case 'Refactor':
                return vscode.CodeActionKind.Refactor;
            case 'Source':
                return vscode.CodeActionKind.Source;
            default:
                return vscode.CodeActionKind.Refactor;
        }
    }
}
