package com.clawdius.plugin.widget

import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project
import com.intellij.openapi.wm.StatusBar
import com.intellij.openapi.wm.StatusBarWidget
import com.intellij.openapi.wm.StatusBarWidgetFactory
import com.clawdius.plugin.ClawdiusService
import com.clawdius.plugin.ConnectionState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch

/**
 * Status bar widget factory for Clawdius.
 */
class ClawdiusStatusBarWidgetFactory : StatusBarWidgetFactory {
    override fun getId(): String = "com.clawdius.plugin.widget.ClawdiusStatusBarWidget"
    
    override fun getDisplayName(): String = "Clawdius Status"
    
    override fun isAvailable(project: Project): Boolean = true
    
    override fun createWidget(project: Project): StatusBarWidget {
        return ClawdiusStatusBarWidget(project)
    }
    
    override fun canBeEnabledOn(statusBar: StatusBar): Boolean = true
}

/**
 * Status bar widget showing Clawdius connection status.
 */
class ClawdiusStatusBarWidget(private val project: Project) : StatusBarWidget, StatusBarWidget.TextPresentation {
    private val logger = Logger.getInstance(ClawdiusStatusBarWidget::class.java)
    private var statusBar: StatusBar? = null
    
    init {
        val service = ClawdiusService.getInstance()
        CoroutineScope(Dispatchers.Main).launch {
            service.connectionState.collectLatest { updateWidget() }
        }
    }
    
    override fun ID(): String = "com.clawdius.plugin.widget.ClawdiusStatusBarWidget"
    
    override fun install(statusBar: StatusBar) {
        this.statusBar = statusBar
        updateWidget()
    }
    
    private fun updateWidget() {
        statusBar?.updateWidget(ID())
    }
    
    override fun getText(): String {
        val state = ClawdiusService.getInstance().connectionState.value
        return when (state) {
            is ConnectionState.Connected -> "Clawdius: Connected"
            is ConnectionState.Connecting -> "Clawdius: Connecting..."
            is ConnectionState.Disconnected -> "Clawdius: Offline"
            is ConnectionState.Error -> "Clawdius: Error"
        }
    }
    
    override fun getTooltip(): String {
        val state = ClawdiusService.getInstance().connectionState.value
        return when (state) {
            is ConnectionState.Connected -> "Clawdius AI Assistant is connected"
            is ConnectionState.Connecting -> "Connecting to Clawdius..."
            is ConnectionState.Disconnected -> "Clawdius is not connected"
            is ConnectionState.Error -> "Clawdius error: ${state.message}"
        }
    }
    
    override fun getClickConsumer(): java.util.function.Consumer<java.awt.event.MouseEvent>? = null
    
    override fun dispose() {
        statusBar = null
    }
}
