package com.clawdius.plugin.completion

import com.intellij.codeInsight.completion.*
import com.intellij.codeInsight.lookup.LookupElementBuilder
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project
import com.intellij.patterns.PlatformPatterns
import com.intellij.util.ProcessingContext
import com.clawdius.plugin.ClawdiusService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import com.intellij.lang.Language

/**
 * Completion contributor that provides AI-powered code completions.
 */
class ClawdiusCompletionContributor : CompletionContributor() {
    private val logger = Logger.getInstance(ClawdiusCompletionContributor::class.java)
    
    init {
        extend(
            CompletionType.BASIC,
            PlatformPatterns.psiElement(),
            ClawdiusCompletionProvider()
        )
    }
}

/**
 * Completion provider that fetches suggestions from Clawdius.
 */
class ClawdiusCompletionProvider : CompletionProvider<CompletionParameters>() {
    private val logger = Logger.getInstance(ClawdiusCompletionProvider::class.java)
    
    override fun addCompletions(
        parameters: CompletionParameters,
        context: ProcessingContext,
        result: CompletionResultSet
    ) {
        val project = parameters.position.project
        val service = ClawdiusService.getInstance()
        val settings = service.settings.value
        
        if (!settings.enableAutoComplete) {
            return
        }
        
        // Don't block the UI - run asynchronously
        val editor = parameters.editor
        val document = editor.document
        val offset = parameters.offset
        
        // Get prefix (text before cursor) - limit to last 2000 chars for efficiency
        val prefixStart = maxOf(0, offset - 2000)
        val prefix = document.text.substring(prefixStart, offset)
        
        // Get suffix (text after cursor) - limit to next 500 chars
        val suffixEnd = minOf(document.textLength, offset + 500)
        val suffix = document.text.substring(offset, suffixEnd)
        
        // Determine language
        val language = getLanguage(parameters)
        
        // Get completions from Clawdius (with timeout)
        try {
            runBlocking {
                val completionResult = withContext(Dispatchers.IO) {
                    service.complete(
                        project = project,
                        prefix = prefix,
                        suffix = suffix,
                        language = language,
                        maxTokens = 200
                    )
                }
                
                completionResult.getOrNull()?.let { completion ->
                    if (completion.isNotBlank()) {
                        // Add the completion to results
                        result.addElement(
                            LookupElementBuilder.create(completion)
                                .withPresentableText(formatCompletion(completion))
                                .withTypeText("Clawdius AI")
                                .withInsertHandler { context, _ ->
                                    // Handle insert if needed
                                }
                        )
                    }
                }
            }
        } catch (e: Exception) {
            logger.debug("Completion request failed or timed out", e)
        }
    }
    
    private fun formatCompletion(completion: String): String {
        // Show first line or first 50 chars
        val firstLine = completion.lines().firstOrNull() ?: completion
        return if (firstLine.length > 50) {
            firstLine.take(47) + "..."
        } else {
            firstLine
        }
    }
    
    private fun getLanguage(parameters: CompletionParameters): String {
        val file = parameters.originalFile
        val language = file.language
        
        return mapLanguage(language)
    }
    
    private fun mapLanguage(language: Language): String {
        return when (language.id) {
            "kotlin", "kotlin-script" -> "kotlin"
            "JAVA" -> "java"
            "Python" -> "python"
            "JavaScript" -> "javascript"
            "TypeScript" -> "typescript"
            "Rust" -> "rust"
            "Go" -> "go"
            "ruby" -> "ruby"
            "PHP" -> "php"
            "C#" -> "csharp"
            "CPP" -> "cpp"
            "C" -> "c"
            "Swift" -> "swift"
            "ObjectiveC" -> "objective-c"
            "Scala" -> "scala"
            "Groovy" -> "groovy"
            "SQL" -> "sql"
            "HTML" -> "html"
            "CSS" -> "css"
            "JSON" -> "json"
            "YAML" -> "yaml"
            "Markdown" -> "markdown"
            "XML" -> "xml"
            else -> "text"
        }
    }
}
