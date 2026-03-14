package com.clawdius.plugin.toolwindow

import com.intellij.openapi.project.Project
import com.intellij.ui.components.JBScrollPane
import com.intellij.ui.components.JBTextArea
import com.intellij.util.ui.JBUI
import com.clawdius.plugin.ClawdiusService
import com.clawdius.plugin.ContextItem
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import java.awt.BorderLayout
import java.awt.Dimension
import java.awt.event.KeyAdapter
import java.awt.event.KeyEvent
import javax.swing.*

/**
 * Main panel for the Clawdius tool window.
 * Provides a chat interface for interacting with the AI.
 */
class ClawdiusToolWindowPanel(private val project: Project) : JPanel(BorderLayout()) {
    
    private val chatHistory = JBTextArea()
    private val inputField = JBTextArea()
    private val sendButton = JButton("Send")
    private val clearButton = JButton("Clear")
    private val contextList = JList<ContextItem>()
    
    init {
        createUI()
    }
    
    private fun createUI() {
        // Chat history scroll pane
        val historyScrollPane = JBScrollPane(chatHistory).apply {
            preferredSize = Dimension(400, 300)
        }
        
        // Input panel
        val inputScrollPane = JBScrollPane(inputField).apply {
            preferredSize = Dimension(400, 60)
        }
        
        val buttonPanel = JPanel().apply {
            layout = BoxLayout(this, BoxLayout.X_AXIS)
            add(sendButton)
            add(Box.createHorizontalStrut(5))
            add(clearButton)
        }
        
        val inputPanel = JPanel(BorderLayout()).apply {
            border = JBUI.Borders.empty(5)
            add(inputScrollPane, BorderLayout.CENTER)
            add(buttonPanel, BorderLayout.SOUTH)
        }
        
        // Main layout
        add(historyScrollPane, BorderLayout.CENTER)
        add(inputPanel, BorderLayout.SOUTH)
        
        // Welcome message
        appendToChat("Clawdius", "Welcome! I'm ready to help you with your code.")
    }
    
    private fun sendMessage() {
        val message = inputField.text.trim()
        if (message.isEmpty()) return
        
        // Clear input
        inputField.text = ""
        
        // Add user message to history
        appendToChat("You", message)
        
        // Send to Clawdius
        runBlocking {
            sendToClawdius(message)
        }
    }
    
    private suspend fun sendToClawdius(message: String) {
        val service = ClawdiusService.getInstance()
        
        try {
            val result = service.chat(project, message, contextList)
            
            result.fold(
                onSuccess = { response ->
                    appendToChat("Clawdius", response)
                },
                onFailure = { error ->
                    appendToChat("Clawdius", "Error: ${error.message}")
                }
            )
        } catch (e: Exception) {
            appendToChat("Clawdius", "Error: ${e.message}")
        }
    }
    
    private fun appendToChat(sender: String, message: String) {
        SwingUtilities.invokeLater {
            chatHistory.append("[$sender]\n$message\n\n")
            chatHistory.caretPosition = chatHistory.document.length
        }
    }
    
    private fun clearChat() {
        chatHistory.text = ""
        appendToChat("Clawdius", "Chat cleared. How can I help you?")
    }
}