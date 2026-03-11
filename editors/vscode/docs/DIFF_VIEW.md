# Diff View Feature

Visual diff preview for code changes proposed by Clawdius in VSCode.

## Features

### 1. Diff Panel Component
- **Side-by-side diff view**: Uses VSCode's native diff editor
- **Line numbers and file names**: Clear visualization of changes
- **Accept/reject buttons**: Easy-to-use action buttons in the chat view

### 2. Integration Points
- **Automatic detection**: Shows diff when Clawdius proposes changes
- **Command palette**: `Clawdius: Show all pending changes`
- **Status bar indicator**: Shows count of unreviewed changes

### 3. Commands

| Command | Description | Keybinding |
|---------|-------------|------------|
| `clawdius.showDiff` | Show diff for current change | - |
| `clawdius.acceptChange` | Accept current change | `Ctrl+Shift+Enter` / `Cmd+Shift+Enter` |
| `clawdius.rejectChange` | Reject current change | `Ctrl+Shift+Escape` / `Cmd+Shift+Escape` |
| `clawdius.showAllChanges` | Show all pending changes | `Ctrl+Shift+D` / `Cmd+Shift+D` |

### 4. Demo Commands

For testing and demonstration purposes:

| Command | Description |
|---------|-------------|
| `clawdius.testDiff` | Add a single demo change |
| `clawdius.testMultipleDiffs` | Add multiple demo changes |

## Usage

### As a User

1. **Viewing Changes**: When Clawdius proposes a code change:
   - A message appears in the chat with a green border
   - Click "👁️ Show Diff" to open the diff view
   - Use VSCode's native diff editor to review changes

2. **Accepting Changes**:
   - Click "✓ Accept" in the chat message
   - Or use `Ctrl+Shift+Enter` while viewing the diff
   - The change is applied to the file

3. **Rejecting Changes**:
   - Click "✗ Reject" in the chat message
   - Or use `Ctrl+Shift+Escape` while viewing the diff
   - The change is discarded

4. **Reviewing Multiple Changes**:
   - Check the status bar for pending change count
   - Use `Ctrl+Shift+D` or click status bar to see all changes
   - Select a change from the quick pick menu

### As a Developer

#### Integrating with Chat Responses

The chat view automatically handles `codeChange` messages from the extension:

```typescript
// In your backend response handler
webview.postMessage({
    command: 'codeChange',
    change: {
        id: 'unique-id',
        filePath: '/path/to/file.ts',
        description: 'Description of change',
        originalContent: 'original code',
        modifiedContent: 'modified code',
        timestamp: Date.now()
    }
});
```

#### Using the DiffViewProvider API

```typescript
import { DiffViewProvider, CodeChange } from './providers/diffView';

// Create instance
const diffView = new DiffViewProvider();

// Add a change
const change: CodeChange = {
    id: 'change-1',
    filePath: '/path/to/file.ts',
    description: 'Add new feature',
    originalContent: 'original code',
    modifiedContent: 'modified code',
    timestamp: Date.now()
};
diffView.addChange(change);

// Show diff
await diffView.showDiff(change);

// Accept change
await diffView.acceptChange(change);

// Reject change
await diffView.rejectChange(change);

// Get all pending changes
const pending = diffView.getPendingChanges();

// Show all changes in quick pick
await diffView.showAllChanges();
```

## Architecture

### Files

1. **`src/providers/diffView.ts`**
   - `DiffViewProvider` class
   - Manages pending changes
   - Provides diff visualization
   - Handles accept/reject actions
   - Status bar integration

2. **`src/providers/chatView.ts`**
   - Updated to handle code changes
   - Displays diff actions in messages
   - Integrates with DiffViewProvider

3. **`src/extension.ts`**
   - Registers DiffViewProvider
   - Registers diff commands
   - Sets up context variables

4. **`package.json`**
   - Command contributions
   - Keybindings
   - Context menus

### Event Flow

```
User sends message → Backend processes → 
Backend proposes change → Extension posts codeChange message →
Chat view displays change with actions → 
User clicks action → DiffViewProvider handles action →
File updated / Change discarded
```

### VSCode Integration

The diff view uses VSCode's native diff editor:

```typescript
vscode.commands.executeCommand(
    'vscode.diff',
    originalUri,
    modifiedUri,
    title,
    options
);
```

Custom URI scheme `clawdius-diff:` is used for the diff content:

```typescript
const originalUri = vscode.Uri.parse(`clawdius-diff:${filePath}?original`);
const modifiedUri = vscode.Uri.parse(`clawdius-diff:${filePath}?modified`);
```

A `TextDocumentContentProvider` serves the content:

```typescript
class implements vscode.TextDocumentContentProvider {
    provideTextDocumentContent(uri: vscode.Uri): string {
        const isOriginal = uri.query === 'original';
        return isOriginal ? originalContent : modifiedContent;
    }
}
```

## Testing

### Manual Testing

1. Open Command Palette (`Ctrl+Shift+P`)
2. Run "Clawdius: Test diff view (demo)"
3. A demo change appears in the status bar
4. Click status bar or run "Show All Changes"
5. Review, accept, or reject the change

### Automated Testing

Run the demo commands to verify:

```bash
# Compile
cd editors/vscode
tsc

# In VSCode
# 1. F5 to launch extension development host
# 2. Run "Clawdius: Test diff view (demo)"
# 3. Verify diff opens
# 4. Test accept/reject
```

## Future Enhancements

1. **Side-by-side vs Unified Toggle**
   - Add option to switch between diff modes
   - Persist preference in settings

2. **Batch Operations**
   - Accept all changes
   - Reject all changes
   - Select multiple changes

3. **Change History**
   - View accepted/rejected changes
   - Undo previous actions
   - Restore rejected changes

4. **Enhanced Visualization**
   - Inline diff in editor
   - Syntax highlighting
   - Minimap indicators

5. **Integration with Source Control**
   - Create commit from accepted changes
   - Stage/unstage changes
   - Review before commit

## Troubleshooting

### Diff not opening
- Check that file path is absolute
- Verify original and modified content are different
- Check VSCode developer console for errors

### Status bar not updating
- Ensure DiffViewProvider is disposed properly
- Check that changes are being added correctly
- Verify event emitters are working

### Changes not persisting
- Check file permissions
- Verify workspace is not read-only
- Check for file system errors in console

## Contributing

When contributing to the diff view feature:

1. Follow existing code style
2. Add JSDoc comments to public methods
3. Update this README if adding new features
4. Test with demo commands
5. Ensure TypeScript compiles without errors
