package com.clawdius.settings

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage
import com.intellij.util.xmlb.XmlSerializerUtil
import org.jetbrains.annotations.Nullable

/**
 * Persistent settings for Clawdius
 */
@State(
    name = "com.clawdius.settings.ClawdiusSettings",
    storages = [Storage("ClawdiusSettings.xml")]
)
class ClawdiusSettings : PersistentStateComponent<ClawdiusSettings> {
    
    var host: String = "localhost"
    var port: Int = 9527
    var autoConnect: Boolean = true
    var enableCompletions: Boolean = true
    var completionDebounceMs: Int = 300
    var maxTokens: Int = 4096
    var temperature: Double = 0.7
    var defaultModel: String = "claude-3-5-sonnet-20241022"
    
    @Nullable
    override fun getState(): ClawdiusSettings {
        return this
    }
    
    override fun loadState(state: ClawdiusSettings) {
        XmlSerializerUtil.copyBean(state, this)
    }
    
    companion object {
        fun getInstance(): ClawdiusSettings {
            return ApplicationManager.getApplication().getService(ClawdiusSettings::class.java)
        }
    }
}
