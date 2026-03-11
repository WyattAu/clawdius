package com.clawdius.actions

import com.clawdius.ClawdiusClient
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.actionSystem.CommonDataKeys
import com.intellij.openapi.wm.ToolWindowManager

/**
 * Action to get AI-powered refactoring suggestions
 */
class RefactorCodeAction : AnAction() {
    
    private val client = ClawdiusClient()
    
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val editor = e.getData(CommonDataKeys.EDITOR) ?: return
        
        val selectedText = editor.selectionModel.selectedText
        if (selectedText.isNullOrBlank()) return
        
        // Activate tool window
        val toolWindow = ToolWindowManager.getInstance(project).getToolWindow("Clawdius")
        toolWindow?.activate(null)
        
        // Send refactor request
        Thread {
            try {
                val prompt = "Please suggest refactoring improvements for this code. Focus on readability, performance, and best practices:\n\n```\n$selectedText\n```"
                client.chat(prompt, project)
            } catch (e: Exception) {
                // Error handling
            }
        }.start()
    }
    
    override fun update(e: AnActionEvent) {
        val editor = e.getData(CommonDataKeys.EDITOR)
        val hasSelection = editor?.selectionModel?.hasSelection() == true
        e.presentation.isEnabled = e.project != null && hasSelection
    }
}
