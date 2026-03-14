package com.clawdius.plugin

import com.google.gson.Gson
import com.google.gson.JsonObject
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.components.Service
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.File
import java.util.concurrent.TimeUnit

/**
 * Main application service for Clawdius plugin.
 * Manages the connection to the Clawdius backend and provides
 * access to AI capabilities.
 */
@Service(Service.Level.APP)
class ClawdiusService {
    private val logger = Logger.getInstance(ClawdiusService::class.java)
    private val gson = Gson()
    
    private val httpClient: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(120, TimeUnit.SECONDS)
        .writeTimeout(60, TimeUnit.SECONDS)
        .build()
    
    private val _connectionState = MutableStateFlow<ConnectionState>(ConnectionState.Disconnected)
    val connectionState: StateFlow<ConnectionState> = _connectionState.asStateFlow()
    
    private val _settings = MutableStateFlow(ClawdiusSettings())
    val settings: StateFlow<ClawdiusSettings> = _settings.asStateFlow()
    
    /**
     * Check if the Clawdius backend is available.
     */
    suspend fun checkConnection(): Boolean {
        return withContext(Dispatchers.IO) {
            try {
                _connectionState.value = ConnectionState.Connecting
                
                val settings = _settings.value
                val request = Request.Builder()
                    .url("${settings.serverUrl}/health")
                    .get()
                    .build()
                
                val response = httpClient.newCall(request).execute()
                val healthy = response.isSuccessful && response.body?.string()?.contains("healthy") == true
                
                _connectionState.value = if (healthy) ConnectionState.Connected else ConnectionState.Error("Server not healthy")
                healthy
            } catch (e: Exception) {
                logger.error("Failed to connect to Clawdius backend", e)
                _connectionState.value = ConnectionState.Error(e.message ?: "Unknown error")
                false
            }
        }
    }
    
    /**
     * Start the Clawdius CLI server.
     */
    suspend fun startServer(): Boolean {
        return withContext(Dispatchers.IO) {
            try {
                val settings = _settings.value
                
                // Check if already running
                if (checkConnection()) {
                    return@withContext true
                }
                
                // Start the server process
                val processBuilder = ProcessBuilder(
                    settings.clawdiusPath,
                    "serve",
                    "--port", settings.serverPort.toString(),
                    "--host", "127.0.0.1"
                )
                processBuilder.directory(File(System.getProperty("user.home")))
                processBuilder.redirectErrorStream(true)
                
                val process = processBuilder.start()
                
                // Wait for server to start
                var attempts = 0
                while (attempts < 30) {
                    Thread.sleep(1000)
                    if (checkConnection()) {
                        return@withContext true
                    }
                    attempts++
                }
                
                logger.error("Server failed to start within 30 seconds")
                false
            } catch (e: Exception) {
                logger.error("Failed to start Clawdius server", e)
                false
            }
        }
    }
    
    /**
     * Send a completion request to Clawdius.
     */
    suspend fun complete(
        project: Project,
        prefix: String,
        suffix: String = "",
        language: String = "text",
        maxTokens: Int = 500
    ): Result<String> {
        return withContext(Dispatchers.IO) {
            try {
                val settings = _settings.value
                
                val requestBody = gson.toJson(mapOf(
                    "prefix" to prefix,
                    "suffix" to suffix,
                    "language" to language,
                    "max_tokens" to maxTokens,
                    "provider" to settings.provider,
                    "model" to settings.model.ifEmpty { null },
                    "temperature" to settings.temperature
                ))
                
                val request = Request.Builder()
                    .url("${settings.serverUrl}/api/complete")
                    .header("Authorization", "Bearer ${settings.apiKey}")
                    .header("Content-Type", "application/json")
                    .post(requestBody.toRequestBody("application/json".toMediaType()))
                    .build()
                
                val response = httpClient.newCall(request).execute()
                
                if (!response.isSuccessful) {
                    return@withContext Result.failure(Exception("HTTP ${response.code}: ${response.message}"))
                }
                
                val responseBody = response.body?.string() ?: return@withContext Result.failure(Exception("Empty response"))
                val json = gson.fromJson(responseBody, JsonObject::class.java)
                val completion = json.get("completion")?.asString ?: ""
                
                Result.success(completion)
            } catch (e: Exception) {
                logger.error("Completion request failed", e)
                Result.failure(e)
            }
        }
    }
    
    /**
     * Send a chat message to Clawdius.
     */
    suspend fun chat(
        project: Project,
        message: String,
        context: List<ContextItem> = emptyList()
    ): Result<String> {
        return withContext(Dispatchers.IO) {
            try {
                val settings = _settings.value
                
                val requestBody = gson.toJson(mapOf(
                    "message" to message,
                    "context" to context.map { mapOf(
                        "type" to it.type,
                        "path" to it.path,
                        "content" to it.content,
                        "language" to it.language
                    )},
                    "provider" to settings.provider,
                    "model" to settings.model.ifEmpty { null },
                    "max_tokens" to settings.maxTokens,
                    "temperature" to settings.temperature,
                    "sandbox_level" to settings.sandboxLevel
                ))
                
                val request = Request.Builder()
                    .url("${settings.serverUrl}/api/chat")
                    .header("Authorization", "Bearer ${settings.apiKey}")
                    .header("Content-Type", "application/json")
                    .post(requestBody.toRequestBody("application/json".toMediaType()))
                    .build()
                
                val response = httpClient.newCall(request).execute()
                
                if (!response.isSuccessful) {
                    return@withContext Result.failure(Exception("HTTP ${response.code}: ${response.message}"))
                }
                
                val responseBody = response.body?.string() ?: return@withContext Result.failure(Exception("Empty response"))
                val json = gson.fromJson(responseBody, JsonObject::class.java)
                val reply = json.get("response")?.asString ?: json.get("message")?.asString ?: ""
                
                Result.success(reply)
            } catch (e: Exception) {
                logger.error("Chat request failed", e)
                Result.failure(e)
            }
        }
    }
    
    /**
     * Analyze code and provide suggestions.
     */
    suspend fun analyze(
        project: Project,
        code: String,
        language: String
    ): Result<List<CodeSuggestion>> {
        return withContext(Dispatchers.IO) {
            try {
                val settings = _settings.value
                
                val requestBody = gson.toJson(mapOf(
                    "code" to code,
                    "language" to language,
                    "provider" to settings.provider,
                    "model" to settings.model.ifEmpty { null }
                ))
                
                val request = Request.Builder()
                    .url("${settings.serverUrl}/api/analyze")
                    .header("Authorization", "Bearer ${settings.apiKey}")
                    .header("Content-Type", "application/json")
                    .post(requestBody.toRequestBody("application/json".toMediaType()))
                    .build()
                
                val response = httpClient.newCall(request).execute()
                
                if (!response.isSuccessful) {
                    return@withContext Result.failure(Exception("HTTP ${response.code}: ${response.message}"))
                }
                
                val responseBody = response.body?.string() ?: return@withContext Result.failure(Exception("Empty response"))
                val json = gson.fromJson(responseBody, JsonObject::class.java)
                val suggestionsArray = json.getAsJsonArray("suggestions") ?: return@withContext Result.success(emptyList())
                
                val suggestions = suggestionsArray.map { element ->
                    val obj = element.asJsonObject
                    CodeSuggestion(
                        message = obj.get("message")?.asString ?: "",
                        severity = Severity.valueOf(obj.get("severity")?.asString ?: "INFO"),
                        range = obj.get("range")?.asJsonObject?.let { range ->
                            TextRange(
                                startLine = range.get("startLine")?.asInt ?: 0,
                                startColumn = range.get("startColumn")?.asInt ?: 0,
                                endLine = range.get("endLine")?.asInt ?: 0,
                                endColumn = range.get("endColumn")?.asInt ?: 0
                            )
                        },
                        fix = obj.get("fix")?.asString
                    )
                }
                
                Result.success(suggestions)
            } catch (e: Exception) {
                logger.error("Analysis request failed", e)
                Result.failure(e)
            }
        }
    }
    
    /**
     * Update settings.
     */
    fun updateSettings(newSettings: ClawdiusSettings) {
        _settings.value = newSettings
    }
    
    companion object {
        fun getInstance(): ClawdiusService {
            return ApplicationManager.getApplication().getService(ClawdiusService::class.java)
        }
    }
}

/**
 * Connection state to the Clawdius backend.
 */
sealed class ConnectionState {
    data object Disconnected : ConnectionState()
    data object Connecting : ConnectionState()
    data object Connected : ConnectionState()
    data class Error(val message: String) : ConnectionState()
}

/**
 * Settings for the Clawdius plugin.
 */
data class ClawdiusSettings(
    val serverUrl: String = "http://localhost:3000",
    val serverPort: Int = 3000,
    val clawdiusPath: String = "clawdius",
    val apiKey: String = "",
    val provider: String = "anthropic",
    val model: String = "",
    val enableAutoComplete: Boolean = true,
    val enableInlineHints: Boolean = true,
    val maxTokens: Int = 2048,
    val temperature: Double = 0.7,
    val sandboxLevel: String = "filtered"
)

/**
 * Context item for requests.
 */
data class ContextItem(
    val type: String,
    val path: String? = null,
    val content: String? = null,
    val language: String? = null
)

/**
 * Code suggestion from analysis.
 */
data class CodeSuggestion(
    val message: String,
    val severity: Severity,
    val range: TextRange? = null,
    val fix: String? = null
)

enum class Severity {
    ERROR, WARNING, INFO, HINT
}

data class TextRange(
    val startLine: Int,
    val startColumn: Int,
    val endLine: Int,
    val endColumn: Int
)
