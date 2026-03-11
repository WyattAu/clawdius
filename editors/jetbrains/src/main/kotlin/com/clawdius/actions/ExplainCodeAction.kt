package com.clawdius.actions

import com.clawdius.ClawdiusClient
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.actionSystem.CommonDataKeys
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.ui.Messages
import com.intellij.openapi.wm.ToolWindowManager

/**
 * Action to explain the selected code using AI
 */
class ExplainCodeAction : AnAction() {
    
    private val client = ClawdiusClient()
    
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val editor = e.getData(CommonDataKeys.EDITOR) ?: return
        
        val selectedText = editor.selectionModel.selectedText
        if (selectedText.isNullOrBlank()) {
            Messages.showWarningDialog(
                project,
                "Please select some code to explain.",
                "No Selection"
            )
            return
        }
        
        // Open the tool window
        val toolWindow = ToolWindowManager.getInstance(project).getToolWindow("Clawdius")
        toolWindow?.activate(null)
        
        // Send explanation request
        Thread {
            try {
                val prompt = "Please explain this code:\n\n```\n$selectedText\n```"
                val response = client.chat(prompt, project)
                
                // The response will appear in the chat window
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
