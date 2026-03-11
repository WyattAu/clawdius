package com.clawdius.actions

import com.clawdius.ClawdiusClient
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.actionSystem.CommonDataKeys
import com.intellij.openapi.wm.ToolWindowManager

/**
 * Action to generate unit tests for the selected code
 */
class GenerateTestsAction : AnAction() {
    
    private val client = ClawdiusClient()
    
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val editor = e.getData(CommonDataKeys.EDITOR) ?: return
        
        val selectedText = editor.selectionModel.selectedText
        if (selectedText.isNullOrBlank()) return
        
        // Activate tool window
        val toolWindow = ToolWindowManager.getInstance(project).getToolWindow("Clawdius")
        toolWindow?.activate(null)
        
        // Send test generation request
        Thread {
            try {
                val file = e.getData(CommonDataKeys.VIRTUAL_FILE)
                val language = file?.fileType?.name ?: "unknown"
                
                val prompt = """
                    Generate comprehensive unit tests for this $language code.
                    Include edge cases, error cases, and normal cases.
                    Use the appropriate testing framework for the language.
                    
                    ```$language
                    $selectedText
                    ```
                """.trimIndent()
                
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
