package com.clawdius.plugin.inspection

import com.intellij.codeInspection.*
import com.intellij.codeInspection.LocalInspectionTool
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.project.Project
import com.intellij.openapi.util.TextRange
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiFile
import com.clawdius.plugin.ClawdiusService
import com.clawdius.plugin.CodeSuggestion
import com.clawdius.plugin.Severity
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import org.jetbrains.kotlin.utils.inheritance.isOverridable

/**
 * Local inspection that provides AI-powered code analysis.
 */
class ClawdiusInspection : LocalInspectionTool() {
    override fun buildTool(): LocalInspectionTool = ClawdiusInspection()
    
    private val logger = Logger.getInstance(ClawdiusInspection::class.java)
    
    override fun getShortName(): String = SHORTName
    
    override fun getGroupDisplayName(): String = "Clawdius"
    
    override fun getDescription(): String = "Analyzes code with AI for suggestions"
    
    override fun isEnabledByDefault(): Boolean = true
    
    override fun runInspectionOnFile(
        file: PsiFile,
        manager: InspectionManager,
        isOnTheFly: Boolean,
        problems: MutableList<ProblemDescriptor>
    ) {
        val settings = ClawdiusService.getInstance().settings.value
        if (!settings.enableInlineHints) {
            return
        }
        
        val editor = Editor.getInstance(file.project) ?: return
        
        try {
            val code = file.text
            val language = file.language.displayName.lowercase()
            
            val suggestions = runBlocking {
                withContext(Dispatchers.IO) {
                    ClawdiusService.getInstance().analyze(file.project, code, language).getOrNull()
                }
            }
            
            suggestions?.forEach { suggestion ->
                val severity = when (suggestion.severity) {
                    Severity.ERROR -> HighlightSeverity.ERROR
                    Severity.WARNING -> HighlightSeverity.WARNING
                    Severity.INFO -> HighlightSeverity.INFORMATION
                    Severity.HINT -> HighlightSeverity.INFORMATION
                }
                
                val descriptor = manager.createProblemDescriptor(
                    suggestion.message,
                    severity
                )
                
                problems.add(descriptor)
            }
        } catch (e: Exception) {
            logger.error("Inspection failed", e)
        }
    }
}
