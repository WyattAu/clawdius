package com.clawdius

import com.intellij.openapi.project.Project
import com.intellij.ui.components.JBScrollPane
import com.intellij.ui.components.JBTextArea
import com.intellij.util.ui.JBUI
import java.awt.BorderLayout
import java.awt.Dimension
import java.awt.FlowLayout
import java.awt.event.ActionEvent
import java.awt.event.KeyAdapter
import java.awt.event.KeyEvent
import javax.swing.*

/**
 * Main panel for the Clawdius tool window
 */
class ClawdiusToolWindowPanel(private val project: Project) : JPanel(BorderLayout()) {
    
    private val chatArea = JBTextArea().apply {
        isEditable = false
        lineWrap = true
        wrapStyleWord = true
        margin = JBUI.insets(10)
    }
    
    private val inputField = JTextField().apply {
        margin = JBUI.insets(5)
        addKeyListener(object : KeyAdapter() {
            override fun keyPressed(e: KeyEvent) {
                if (e.keyCode == KeyEvent.VK_ENTER && !e.isShiftDown) {
                    sendMessage()
                    e.consume()
                }
            }
        })
    }
    
    private val sendButton = JButton("Send").apply {
        addActionListener { sendMessage() }
    }
    
    private val client = ClawdiusClient()
    
    init {
        setupUI()
    }
    
    private fun setupUI() {
        // Chat area with scroll
        val scrollPane = JBScrollPane(chatArea).apply {
            preferredSize = Dimension(400, 500)
            verticalScrollBarPolicy = JScrollPane.VERTICAL_SCROLLBAR_AS_NEEDED
        }
        
        // Input panel
        val inputPanel = JPanel(BorderLayout()).apply {
            add(inputField, BorderLayout.CENTER)
            add(sendButton, BorderLayout.EAST)
            border = JBUI.Borders.emptyTop(5)
        }
        
        // Button panel
        val buttonPanel = JPanel(FlowLayout(FlowLayout.LEFT)).apply {
            add(JButton("Clear").apply {
                addActionListener { 
                    chatArea.text = ""
                    appendMessage("Clawdius", "Chat cleared. How can I help you?")
                }
            })
            add(JButton("Settings").apply {
                addActionListener {
                    // Open settings
                    com.intellij.openapi.options.ShowSettingsUtil.getInstance()
                        .showSettingsDialog(project, com.clawdius.settings.ClawdiusConfigurable::class.java)
                }
            })
        }
        
        // Main layout
        add(scrollPane, BorderLayout.CENTER)
        add(inputPanel, BorderLayout.SOUTH)
        add(buttonPanel, BorderLayout.NORTH)
        
        // Welcome message
        appendMessage("Clawdius", """
            Welcome to Clawdius! 🦀
            
            I'm your high-assurance AI coding assistant with:
            • Native Rust performance (<20ms boot)
            • 4-tier sandboxing for secure execution
            • Graph-RAG code understanding
            • Lean4 formal verification
            
            How can I help you today?
        """.trimIndent())
        
        // Check connection
        checkConnection()
    }
    
    private fun sendMessage() {
        val message = inputField.text.trim()
        if (message.isEmpty()) return
        
        inputField.text = ""
        appendMessage("You", message)
        
        // Send to Clawdius
        sendButton.isEnabled = false
        
        Thread {
            try {
                val response = client.chat(message, project)
                SwingUtilities.invokeLater {
                    appendMessage("Clawdius", response)
                    sendButton.isEnabled = true
                }
            } catch (e: Exception) {
                SwingUtilities.invokeLater {
                    appendMessage("Error", "Failed to get response: ${e.message}")
                    sendButton.isEnabled = true
                }
            }
        }.start()
    }
    
    private fun appendMessage(sender: String, message: String) {
        val timestamp = java.time.LocalTime.now().format(java.time.format.DateTimeFormatter.ofPattern("HH:mm"))
        chatArea.append("[$timestamp] $sender:\n$message\n\n")
        chatArea.caretPosition = chatArea.document.length
    }
    
    private fun checkConnection() {
        Thread {
            try {
                val connected = client.checkConnection()
                SwingUtilities.invokeLater {
                    if (!connected) {
                        appendMessage("Warning", """
                            Could not connect to Clawdius server.
                            
                            Please ensure the Clawdius CLI is running:
                            clawdius serve --port 9527
                            
                            Or check your settings in Tools > Clawdius.
                        """.trimIndent())
                    }
                }
            } catch (e: Exception) {
                SwingUtilities.invokeLater {
                    appendMessage("Warning", "Connection check failed: ${e.message}")
                }
            }
        }.start()
    }
}
