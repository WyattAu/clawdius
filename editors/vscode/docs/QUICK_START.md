# Quick Start: Diff View Feature

## Testing the Implementation

### Step 1: Build
```bash
cd editors/vscode
tsc
```

### Step 2: Launch Extension
1. Open VSCode
2. Press `F5` to launch Extension Development Host
3. Wait for new VSCode window to open

### Step 3: Test Diff View

#### Test Single Change
1. Press `Ctrl+Shift+P` / `Cmd+Shift+P`
2. Type "Clawdius: Test diff view (demo)"
3. Press Enter
4. Look for status bar item: "📋 1 change"
5. Click status bar item or press `Ctrl+Shift+D`
6. Select the demo change
7. Review the diff in the editor
8. Test keybindings:
   - `Ctrl+Shift+Enter` / `Cmd+Shift+Enter` to accept
   - `Ctrl+Shift+Escape` / `Cmd+Shift+Escape` to reject

#### Test Multiple Changes
1. Press `Ctrl+Shift+P` / `Cmd+Shift+P`
2. Type "Clawdius: Test multiple diffs (demo)"
3. Press Enter
4. Look for status bar item: "📋 3 changes"
5. Click status bar item
6. Select any change from the list
7. Review, accept, or reject each change

### Step 4: Test Chat Integration
1. Open Clawdius chat view (click cat icon in activity bar)
2. Click "Show Changes" button in toolbar
3. Verify all pending changes appear
4. Note: Full integration requires backend support

## Expected Behavior

### When Adding a Change
- ✅ Status bar shows change count
- ✅ Badge appears on "Show Changes" button
- ✅ Change added to pending list

### When Viewing Diff
- ✅ Native VSCode diff editor opens
- ✅ Original content on left, modified on right
- ✅ Title shows description and filename
- ✅ Syntax highlighting applied

### When Accepting Change
- ✅ File opens in editor
- ✅ Content replaced with modified version
- ✅ Change removed from pending list
- ✅ Status bar count decrements
- ✅ Success message shown
- ✅ Next change shown (if any)

### When Rejecting Change
- ✅ Change removed from pending list
- ✅ Status bar count decrements
- ✅ Info message shown
- ✅ Diff editor closes (if no more changes)
- ✅ Next change shown (if any)

## Troubleshooting

### "No pending changes" message
- Run test command again to add changes
- Check status bar for count

### Diff not opening
- Check VSCode developer console (Help > Toggle Developer Tools)
- Look for errors in console
- Verify file path is valid

### Keybindings not working
- Check `when` clause context: `clawdius.hasPendingChanges`
- Verify extension is active
- Try command palette instead

## Files to Review

### Source Files
- `src/providers/diffView.ts` - Main implementation
- `src/providers/chatView.ts` - Chat integration
- `src/extension.ts` - Command registration
- `src/test/diffViewDemo.ts` - Demo commands

### Configuration
- `package.json` - Commands and keybindings

### Documentation
- `docs/DIFF_VIEW.md` - Full documentation
- `docs/IMPLEMENTATION_SUMMARY.md` - This summary

## Next Steps

After testing:
1. Review code quality
2. Add unit tests (if needed)
3. Integrate with actual Clawdius backend
4. Add more features (see DIFF_VIEW.md)
5. Collect user feedback
6. Iterate and improve
