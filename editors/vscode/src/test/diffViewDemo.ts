import * as vscode from 'vscode';
import { DiffViewProvider, CodeChange } from '../providers/diffView';

export function registerDiffDemo(context: vscode.ExtensionContext, diffView: DiffViewProvider) {
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.testDiff', async () => {
            const demoChange: CodeChange = {
                id: `change-${Date.now()}`,
                filePath: '/tmp/demo.ts',
                description: 'Add hello world function',
                originalContent: `// Original file
function main() {
    console.log('Hello');
}
`,
                modifiedContent: `// Modified file
function main() {
    console.log('Hello, World!');
}

function greet(name: string) {
    return \`Hello, \${name}!\`;
}
`,
                timestamp: Date.now()
            };
            
            diffView.addChange(demoChange);
            
            vscode.window.showInformationMessage(
                'Demo change added! Check the status bar or use "Show All Changes" command.',
                'Show Diff'
            ).then(selection => {
                if (selection === 'Show Diff') {
                    diffView.showDiff(demoChange);
                }
            });
        })
    );
    
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.testMultipleDiffs', async () => {
            const changes: CodeChange[] = [
                {
                    id: `change-${Date.now()}-1`,
                    filePath: '/tmp/file1.ts',
                    description: 'Update imports',
                    originalContent: `import { foo } from './bar';\n`,
                    modifiedContent: `import { foo, bar } from './bar';\nimport { baz } from './baz';\n`,
                    timestamp: Date.now()
                },
                {
                    id: `change-${Date.now()}-2`,
                    filePath: '/tmp/file2.ts',
                    description: 'Add error handling',
                    originalContent: `function process(data) {\n    return data.value;\n}\n`,
                    modifiedContent: `function process(data) {\n    if (!data) {\n        throw new Error('No data provided');\n    }\n    return data.value;\n}\n`,
                    timestamp: Date.now()
                },
                {
                    id: `change-${Date.now()}-3`,
                    filePath: '/tmp/file3.ts',
                    description: 'Refactor function signature',
                    originalContent: `function calculate(a, b, c) {\n    return a + b + c;\n}\n`,
                    modifiedContent: `function calculate(options: { a: number; b: number; c: number }) {\n    return options.a + options.b + options.c;\n}\n`,
                    timestamp: Date.now()
                }
            ];
            
            changes.forEach(change => diffView.addChange(change));
            
            vscode.window.showInformationMessage(
                `${changes.length} demo changes added! Use "Show All Changes" to review them.`
            );
        })
    );
}
