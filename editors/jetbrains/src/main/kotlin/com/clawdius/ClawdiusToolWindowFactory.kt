package com.clawdius

import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project
import com.intellij.openapi.wm.ToolWindow
import com.intellij.openapi.wm.ToolWindowFactory
import com.intellij.ui.content.ContentFactory

/**
 * Factory for creating the Clawdius tool window
 */
class ClawdiusToolWindowFactory : ToolWindowFactory {
    
    override fun createToolWindowContent(project: Project, toolWindow: ToolWindow) {
        val clawdiusPanel = ClawdiusToolWindowPanel(project)
        val contentFactory = ContentFactory.getInstance()
        val content = contentFactory.createContent(clawdiusPanel, "", false)
        toolWindow.contentManager.addContent(content)
    }
    
    override fun init(toolWindow: ToolWindow) {
        LOG.info("Clawdius tool window initialized")
    }
    
    companion object {
        private val LOG = Logger.getInstance(ClawdiusToolWindowFactory::class.java)
    }
}
