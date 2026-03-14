package com.clawdius.plugin.action

import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.project.Project
import com.intellij.openapi.ui.Messages
import com.clawdius.plugin.ClawdiusService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext

/**
 * Action to explain selected code.
 */
class ExplainAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val editor = e.getData(Editor::class.java) ?: return
        
        val selectedText = editor.selectionModel.selectedText?.takeIf { it.isNotBlank() }
            ?: run {
                Messages.showWarningDialog(
                    "No Selection",
                    "Please select some code to explain.",
                    project
                )
                return
            }
        
        runBlocking {
            try {
                val result = withContext(Dispatchers.IO) {
                    ClawdiusService.getInstance().chat(
                        project = project,
                        message = "Explain this code:\n\n```\n$selectedText\n```",
                        context = emptyList()
                    )
                }
                
                result.getOrNull()?.let { explanation ->
                    withContext(Dispatchers.EDT) {
                        // Show explanation in a dialog or toolWindow
                        // For now, just show a message
                        Messages.showInfoMessage("Explanation", explanation, project)
                    }
                }
            } catch (e: Exception) {
                Messages.showErrorDialog(
                    "Error",
                    "Failed to get explanation: ${e.message}",
                    project
                )
            }
        }
    }
    
    override fun update(e: AnActionEvent) {
        val editor = e.getData(Editor::class.java)
        e.presentation.isEnabled = editor != null && 
            editor.selectionModel.hasSelection()
    }
}

/**
 * Action to refactor selected code.
 */
class RefactorAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val editor = e.getData(Editor::class.java) ?: return
        
        val selectedText = editor.selectionModel.selectedText?.takeIf { it.isNotBlank() }
            ?: run {
                Messages.showWarningDialog(
                    "No Selection",
                    "Please select some code to refactor.",
                    project
                )
                return
            }
        
        runBlocking {
            try {
                val result = withContext(Dispatchers.IO) {
                    ClawdiusService.getInstance().chat(
                        project = project,
                        message = "Refactor this code for better readability and performance:\n\n```\n$selectedText\n```",
                        context = emptyList()
                    )
                }
                
                result.getOrNull()?.let { refactored ->
                    withContext(Dispatchers.EDT) {
                        // Apply refactoring
                        val document = editor.document
                        val selection = editor.selectionModel
                        val start = selection.selectionStart
                        val end = selection.selectionEnd
                        
                        document.setText(refactored)
                    }
                }
            } catch (e: Exception) {
                Messages.showErrorDialog(
                    "Error",
                    "Failed to refactor: ${e.message}",
                    project
                )
            }
        }
    }
    
    override fun update(e: AnActionEvent) {
        val editor = e.getData(Editor::class.java)
        e.presentation.isEnabled = editor != null && 
            editor.selectionModel.hasSelection()
    }
}

/**
 * Action to generate tests for selected code.
 */
class GenerateTestsAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val editor = e.getData(Editor::class.java) ?: return
        
        val selectedText = editor.selectionModel.selectedText?.takeIf { it.isNotBlank() }
            ?: run {
                Messages.showWarningDialog(
                    "No Selection",
                    "Please select some code to generate tests for.",
                    project
                )
                return
            }
        
        runBlocking {
            try {
                val result = withContext(Dispatchers.IO) {
                    ClawdiusService.getInstance().chat(
                        project = project,
                        message = "Generate unit tests for this code:\n\n```\n$selectedText\n```",
                        context = emptyList()
                    )
                }
                
                result.getOrNull()?.let { tests ->
                    // Create a new file with tests
                    // For now, just show in a dialog
                    Messages.showInfoMessage("Generated Tests", tests, project)
                }
            } catch (e: Exception) {
                Messages.showErrorDialog(
                    "Error",
                    "Failed to generate tests: ${e.message}",
                    project
                )
            }
        }
    }
    
    override fun update(e: AnActionEvent) {
        val editor = e.getData(Editor::class.java)
        e.presentation.isEnabled = editor != null && 
            editor.selectionModel.hasSelection()
    }
}

/**
 * Action to fix issues in selected code.
 */
class FixAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val editor = e.getData(Editor::class.java) ?: return
        
        val selectedText = editor.selectionModel.selectedText?.takeIf { it.isNotBlank() }
            ?: run {
                Messages.showWarningDialog(
                    "No Selection",
                    "Please select some code to fix.",
                    project
                )
                return
            }
        
        runBlocking {
            try {
                val result = withContext(Dispatchers.IO) {
                    ClawdiusService.getInstance().chat(
                        project = project,
                        message = "Fix any bugs, issues, or improvements in this code:\n\n```\n$selectedText\n```",
                        context = emptyList()
                    )
                }
                
                result.getOrNull()?.let { fixed ->
                    withContext(Dispatchers.EDT) {
                        val document = editor.document
                        document.setText(fixed)
                    }
                }
            } catch (e: Exception) {
                Messages.showErrorDialog(
                    "Error",
                    "Failed to fix: ${e.message}",
                    project
                )
            }
        }
    }
    
    override fun update(e: AnActionEvent) {
        val editor = e.getData(Editor::class.java)
        e.presentation.isEnabled = editor != null && 
            editor.selectionModel.hasSelection()
    }
}

/**
 * Action to open the chat window.
 */
class ChatAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        // Activate the Clawdius tool window
        val toolWindowManager = com.intellij.openapi.wm.ToolWindowManager.getInstance(project)
        toolWindowManager.getToolWindow("Clawdius")?.activate(null)
    }
}

/**
 * Action to open settings.
 */
class OpenSettingsAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        com.intellij.openapi.options.ShowSettingsUtil.getInstance()
            .showSettingsDialog(e.project, "Clawdius")
    }
}
