package com.clawdius.plugin.annotator

import com.intellij.lang.annotation.AnnotationHolder
import com.intellij.lang.annotation.ExternalAnnotator
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project
import com.intellij.psi.PsiFile
import com.clawdius.plugin.ClawdiusService
import com.clawdius.plugin.CodeSuggestion
import com.clawdius.plugin.Severity
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext

/**
 * Annotator that provides AI-powered code analysis.
 */
class ClawdiusAnnotator : ExternalAnnotator<ClawdiusAnnotator.Info, List<CodeSuggestion>>() {
    private val logger = Logger.getInstance(ClawdiusAnnotator::class.java)
    
    data class Info(val code: String, val language: String, val project: Project)
    
    override fun collectInformation(file: PsiFile): Info? {
        val settings = ClawdiusService.getInstance().settings.value
        if (!settings.enableInlineHints) {
            return null
        }
        
        val language = when (file.language.id.lowercase()) {
            "kotlin" -> "kotlin"
            "java" -> "java"
            "python" -> "python"
            "javascript" -> "javascript"
            "typescript" -> "typescript"
            "rust" -> "rust"
            "go" -> "go"
            else -> return null
        }
        
        return Info(file.text, language, file.project)
    }
    
    override fun doAnnotate(info: Info): List<CodeSuggestion>? {
        return runBlocking {
            try {
                val result = withContext(Dispatchers.IO) {
                    ClawdiusService.getInstance().analyze(
                        project = info.project,
                        code = info.code,
                        language = info.language
                    )
                }
                result.getOrNull()
            } catch (e: Exception) {
                logger.error("Analysis failed", e)
                null
            }
        }
    }
    
    override fun apply(file: PsiFile, suggestions: List<CodeSuggestion>?, holder: AnnotationHolder) {
        suggestions?.forEach { suggestion ->
            val severity = when (suggestion.severity) {
                Severity.ERROR -> com.intellij.lang.annotation.HighlightSeverity.ERROR
                Severity.WARNING -> com.intellij.lang.annotation.HighlightSeverity.WARNING
                Severity.INFO -> com.intellij.lang.annotation.HighlightSeverity.INFORMATION
                Severity.HINT -> com.intellij.lang.annotation.HighlightSeverity.INFORMATION
            }
            
            val range = suggestion.range
            val textRange = if (range != null) {
                val document = file.viewProvider.document
                if (document != null) {
                    val startOffset = document.getLineStartOffset(range.startLine) + range.startColumn
                    val endOffset = document.getLineStartOffset(range.endLine) + range.endColumn
                    com.intellij.openapi.util.TextRange(startOffset, endOffset)
                } else {
                    file.textRange
                }
            } else {
                file.textRange
            }
            
            val builder = holder.newAnnotation(severity, suggestion.message).range(textRange)
            
            suggestion.fix?.let { fix ->
                builder.withFix(ClawdiusQuickFix(suggestion.message, fix))
            }
            
            builder.create()
        }
    }
}
