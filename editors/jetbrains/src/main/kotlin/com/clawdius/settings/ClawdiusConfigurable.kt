package com.clawdius.settings

import com.intellij.openapi.options.Configurable
import com.intellij.ui.components.JBCheckBox
import com.intellij.ui.components.JBTextField
import com.intellij.util.ui.FormBuilder
import com.intellij.util.ui.JBUI
import java.awt.Dimension
import javax.swing.*

/**
 * Settings configurable for Clawdius
 */
class ClawdiusConfigurable : Configurable {
    
    private val hostField = JBTextField().apply {
        preferredSize = Dimension(200, preferredSize.height)
    }
    
    private val portField = JBTextField().apply {
        preferredSize = Dimension(100, preferredSize.height)
    }
    
    private val autoConnectCheckbox = JBCheckBox("Auto-connect on startup")
    
    private val enableCompletionsCheckbox = JBCheckBox("Enable inline completions")
    
    private val debounceField = JBTextField().apply {
        preferredSize = Dimension(100, preferredSize.height)
    }
    
    private val maxTokensField = JBTextField().apply {
        preferredSize = Dimension(100, preferredSize.height)
    }
    
    private val temperatureField = JBTextField().apply {
        preferredSize = Dimension(100, preferredSize.height)
    }
    
    private val modelField = JBTextField().apply {
        preferredSize = Dimension(250, preferredSize.height)
    }
    
    private var settings: ClawdiusSettings? = null
    
    override fun getDisplayName(): String = "Clawdius"
    
    override fun createComponent(): JComponent {
        settings = ClawdiusSettings.getInstance()
        reset()
        
        return FormBuilder.createFormBuilder()
            .addSeparator()
            .addComponent(createLabel("Connection Settings", true))
            .addLabeledComponent("Host:", hostField)
            .addLabeledComponent("Port:", portField)
            .addComponent(autoConnectCheckbox)
            .addSeparator()
            .addComponent(createLabel("Completion Settings", true))
            .addComponent(enableCompletionsCheckbox)
            .addLabeledComponent("Debounce (ms):", debounceField)
            .addSeparator()
            .addComponent(createLabel("Model Settings", true))
            .addLabeledComponent("Default Model:", modelField)
            .addLabeledComponent("Max Tokens:", maxTokensField)
            .addLabeledComponent("Temperature:", temperatureField)
            .addComponentFillVertically(JPanel(), 0)
            .panel
            .apply {
                border = JBUI.Borders.empty(10)
            }
    }
    
    private fun createLabel(text: String, bold: Boolean = false): JLabel {
        return JLabel(text).apply {
            if (bold) {
                font = font.deriveFont(font.style or java.awt.Font.BOLD)
            }
            border = JBUI.Borders.emptyTop(10)
        }
    }
    
    override fun isModified(): Boolean {
        val s = settings ?: return false
        return hostField.text != s.host ||
                portField.text != s.port.toString() ||
                autoConnectCheckbox.isSelected != s.autoConnect ||
                enableCompletionsCheckbox.isSelected != s.enableCompletions ||
                debounceField.text != s.completionDebounceMs.toString() ||
                maxTokensField.text != s.maxTokens.toString() ||
                temperatureField.text != s.temperature.toString() ||
                modelField.text != s.defaultModel
    }
    
    override fun apply() {
        val s = settings ?: return
        s.host = hostField.text
        s.port = portField.text.toIntOrNull() ?: 9527
        s.autoConnect = autoConnectCheckbox.isSelected
        s.enableCompletions = enableCompletionsCheckbox.isSelected
        s.completionDebounceMs = debounceField.text.toIntOrNull() ?: 300
        s.maxTokens = maxTokensField.text.toIntOrNull() ?: 4096
        s.temperature = temperatureField.text.toDoubleOrNull() ?: 0.7
        s.defaultModel = modelField.text
    }
    
    override fun reset() {
        val s = settings ?: return
        hostField.text = s.host
        portField.text = s.port.toString()
        autoConnectCheckbox.isSelected = s.autoConnect
        enableCompletionsCheckbox.isSelected = s.enableCompletions
        debounceField.text = s.completionDebounceMs.toString()
        maxTokensField.text = s.maxTokens.toString()
        temperatureField.text = s.temperature.toString()
        modelField.text = s.defaultModel
    }
}
