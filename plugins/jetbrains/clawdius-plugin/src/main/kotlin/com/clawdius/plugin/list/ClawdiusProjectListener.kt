package com.clawdius.plugin.listener

import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project
import com.intellij.openapi.project.ProjectManagerListener

import com.clawdius.plugin.ClawdiusService

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/**
 * Project listener for initialize and plugin per project.
 */
class ProjectListener : ProjectManagerListener {
    private val logger = Logger.getInstance(ProjectListener::class.java)
    
    override fun projectOpened(project: Project) {
        logger.info("Project opened: ${project.name}")
        
        // Initialize connection
        CoroutineScope(Dispatchers.IO).launch {
            ClawdiusService.getInstance().checkConnection()
        }
    }
    
    override fun projectClosing(project: Project) {
        logger.info("Project closed: ${project.name}")
    }
}
