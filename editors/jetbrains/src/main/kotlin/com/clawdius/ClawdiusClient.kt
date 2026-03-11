package com.clawdius

import com.clawdius.settings.ClawdiusSettings
import com.google.gson.Gson
import com.google.gson.JsonObject
import com.intellij.openapi.project.Project
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.PrintWriter
import java.net.Socket

/**
 * Client for communicating with the Clawdius server
 */
class ClawdiusClient {
    
    private val gson = Gson()
    
    /**
     * Send a chat message to Clawdius
     */
    fun chat(message: String, project: Project): String {
        val settings = ClawdiusSettings.getInstance()
        val host = settings.host
        val port = settings.port
        
        return try {
            Socket(host, port).use { socket ->
                val out = PrintWriter(socket.getOutputStream(), true)
                val reader = BufferedReader(InputStreamReader(socket.getInputStream()))
                
                // Send request
                val request = JsonObject().apply {
                    addProperty("jsonrpc", "2.0")
                    addProperty("id", System.currentTimeMillis())
                    addProperty("method", "chat")
                    add("params", JsonObject().apply {
                        addProperty("message", message)
                        addProperty("project_path", project.basePath ?: "")
                    })
                }
                
                out.println(gson.toJson(request))
                
                // Read response
                val responseLine = reader.readLine() ?: return "No response from server"
                val response = gson.fromJson(responseLine, JsonObject::class.java)
                
                if (response.has("error")) {
                    "Error: ${response.get("error").asJsonObject.get("message").asString}"
                } else {
                    response.get("result").asJsonObject.get("text").asString
                }
            }
        } catch (e: Exception) {
            "Connection error: ${e.message}. Is Clawdius running on $host:$port?"
        }
    }
    
    /**
     * Request code completion
     */
    fun complete(prefix: String, suffix: String, language: String, filePath: String): String? {
        val settings = ClawdiusSettings.getInstance()
        val host = settings.host
        val port = settings.port
        
        return try {
            Socket(host, port).use { socket ->
                val out = PrintWriter(socket.getOutputStream(), true)
                val reader = BufferedReader(InputStreamReader(socket.getInputStream()))
                
                val request = JsonObject().apply {
                    addProperty("jsonrpc", "2.0")
                    addProperty("id", System.currentTimeMillis())
                    addProperty("method", "completion/inline")
                    add("params", JsonObject().apply {
                        addProperty("prefix", prefix)
                        addProperty("suffix", suffix)
                        addProperty("language", language)
                        addProperty("file_path", filePath)
                    })
                }
                
                out.println(gson.toJson(request))
                
                val responseLine = reader.readLine() ?: return null
                val response = gson.fromJson(responseLine, JsonObject::class.java)
                
                if (response.has("result")) {
                    response.get("result").asJsonObject.get("text").asString
                } else null
            }
        } catch (e: Exception) {
            null
        }
    }
    
    /**
     * Check if the Clawdius server is running
     */
    fun checkConnection(): Boolean {
        val settings = ClawdiusSettings.getInstance()
        
        return try {
            Socket(settings.host, settings.port).use { socket ->
                val out = PrintWriter(socket.getOutputStream(), true)
                val reader = BufferedReader(InputStreamReader(socket.getInputStream()))
                
                val request = JsonObject().apply {
                    addProperty("jsonrpc", "2.0")
                    addProperty("id", 1)
                    addProperty("method", "ping")
                }
                
                out.println(gson.toJson(request))
                
                val responseLine = reader.readLine()
                responseLine != null && responseLine.contains("pong")
            }
        } catch (e: Exception) {
            false
        }
    }
}
