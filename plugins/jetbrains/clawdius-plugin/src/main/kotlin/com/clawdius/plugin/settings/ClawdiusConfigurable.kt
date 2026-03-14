package com.clawdius.plugin.settings

import com.intellij.openapi.options.Configurable
import com.intellij.openapi.ui.DialogPanel
import com.intellij.ui.components.JBCheckBox
import com.intellij.ui.components.JBTextField
import com.intellij.util.ui.FormBuilder
import com.intellij.util.ui.JBUI
import com.clawdius.plugin.ClawdiusService
import com.clawdius.plugin.ClawdiusSettings
import javax.swing.JComponent
import javax.swing.JPanel

/**
 * Settings configurable for the Clawdius plugin.
 */
class ClawdiusConfigurable : Configurable {
    private var serverUrlField: JBTextField? = null
    private var serverPortField: JBTextField? = null
    private var clawdiusPathField: JBTextField? = null
    private var apiKeyField: JBTextField? = null
    private var providerField: JBTextField? = null
    private var modelField: JBTextField? = null
    private var enableAutoCompleteCheckBox: JBCheckBox? = null
    private var enableInlineHintsCheckBox: JBCheckBox? = null
    private var maxTokensField: JBTextField? = null
    private var temperatureField: JBTextField? = null
    private var sandboxLevelField: JBTextField? = null
    
    private var settings: ClawdiusSettings = ClawdiusService.getInstance().settings.value
    private var panel: JPanel? = null
    
    override fun getDisplayName(): String = "Clawdius"
    
    override fun createComponent(): JComponent {
        serverUrlField = JBTextField(settings.serverUrl)
        serverPortField = JBTextField(settings.serverPort.toString())
        clawdiusPathField = JBTextField(settings.clawdiusPath)
        apiKeyField = JBTextField(settings.apiKey)
        providerField = JBTextField(settings.provider)
        modelField = JBTextField(settings.model)
        enableAutoCompleteCheckBox = JBCheckBox("Enable AI-powered code completion", settings.enableAutoComplete)
        enableInlineHintsCheckBox = JBCheckBox("Enable inline hints", settings.enableInlineHints)
        maxTokensField = JBTextField(settings.maxTokens.toString())
        temperatureField = JBTextField(settings.temperature.toString())
        sandboxLevelField = JBTextField(settings.sandboxLevel)
        
        panel = FormBuilder.createFormBuilder()
            .addLabeledComponent("Server URL:", serverUrlField!!)
            .addLabeledComponent("Server Port:", serverPortField!!)
            .addLabeledComponent("Clawdius Path:", clawdiusPathField!!)
            .addSeparator()
            .addLabeledComponent("API Key:", apiKeyField!!)
            .addLabeledComponent("Provider:", providerField!!)
            .addLabeledComponent("Model (optional):", modelField!!)
            .addSeparator()
            .addComponent(enableAutoCompleteCheckBox!!)
            .addComponent(enableInlineHintsCheckBox!!)
            .addSeparator()
            .addLabeledComponent("Max Tokens:", maxTokensField!!)
            .addLabeledComponent("Temperature:", temperatureField!!)
            .addLabeledComponent("Sandbox Level:", sandboxLevelField!!)
            .addComponentFillVertically(JPanel(), 0)
            .panel
            .also { 
                it.border = JBUI.Borders.empty(10) 
            }
        
        return panel!!
    }
    
    override fun isModified(): Boolean {
        return serverUrlField?.text != settings.serverUrl ||
                serverPortField?.text?.toIntOrNull() != settings.serverPort ||
                clawdiusPathField?.text != settings.clawdiusPath ||
                apiKeyField?.text != settings.apiKey ||
                providerField?.text != settings.provider ||
                modelField?.text != settings.model ||
                enableAutoCompleteCheckBox?.isSelected != settings.enableAutoComplete ||
                enableInlineHintsCheckBox?.isSelected != settings.enableInlineHints ||
                maxTokensField?.text?.toIntOrNull() != settings.maxTokens ||
                temperatureField?.text?.toDoubleOrNull() != settings.temperature ||
                sandboxLevelField?.text != settings.sandboxLevel
    }
    
    override fun apply() {
        val newSettings = ClawdiusSettings(
            serverUrl = serverUrlField?.text ?: settings.serverUrl,
            serverPort = serverPortField?.text?.toIntOrNull() ?: settings.serverPort,
            clawdiusPath = clawdiusPathField?.text ?: settings.clawdiusPath,
            apiKey = apiKeyField?.text ?: settings.apiKey,
            provider = providerField?.text ?: settings.provider,
            model = modelField?.text ?: settings.model,
            enableAutoComplete = enableAutoCompleteCheckBox?.isSelected ?: settings.enableAutoComplete,
            enableInlineHints = enableInlineHintsCheckBox?.isSelected ?: settings.enableInlineHints,
            maxTokens = maxTokensField?.text?.toIntOrNull() ?: settings.maxTokens,
            temperature = temperatureField?.text?.toDoubleOrNull() ?: settings.temperature,
            sandboxLevel = sandboxLevelField?.text ?: settings.sandboxLevel
        )
        
        ClawdiusService.getInstance().updateSettings(newSettings)
        settings = newSettings
    }
    
    override fun reset() {
        serverUrlField?.text = settings.serverUrl
        serverPortField?.text = settings.serverPort.toString()
        clawdiusPathField?.text = settings.clawdiusPath
        apiKeyField?.text = settings.apiKey
        providerField?.text = settings.provider
        modelField?.text = settings.model
        enableAutoCompleteCheckBox?.isSelected = settings.enableAutoComplete
        enableInlineHintsCheckBox?.isSelected = settings.enableInlineHints
        maxTokensField?.text = settings.maxTokens.toString()
        temperatureField?.text = settings.temperature.toString()
        sandboxLevelField?.text = settings.sandboxLevel
    }
}
