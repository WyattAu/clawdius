//! Multimodal input support for image-based interactions.
//!
//! Handles image files (PNG, JPEG, GIF, WebP) for vision-capable LLM models.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Supported image formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    WebP,
}

impl ImageFormat {
    /// Detect image format from file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "gif" => Some(Self::Gif),
            "webp" => Some(Self::WebP),
            _ => None,
        }
    }

    /// Detect image format from magic bytes.
    pub fn from_magic_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        // PNG: 89 50 4E 47
        if data[0] == 0x89 && data[1] == 0x50 && data[2] == 0x4E && data[3] == 0x47 {
            return Some(Self::Png);
        }
        // JPEG: FF D8 FF
        if data[0] == 0xFF && data[1] == 0xD8 && data[2] == 0xFF {
            return Some(Self::Jpeg);
        }
        // GIF: 47 49 46 38
        if data[0] == 0x47 && data[1] == 0x49 && data[2] == 0x46 && data[3] == 0x38 {
            return Some(Self::Gif);
        }
        // WebP: 52 49 46 46 ... 57 45 42 50
        if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
            return Some(Self::WebP);
        }
        None
    }

    /// Get MIME type string.
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::WebP => "image/webp",
        }
    }
}

/// An image attachment for a chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageAttachment {
    /// Base64-encoded image data.
    pub base64_data: String,
    /// Image format.
    pub format: ImageFormat,
    /// Original filename (if loaded from file).
    pub filename: Option<String>,
    /// Image width in pixels (if known).
    pub width: Option<u32>,
    /// Image height in pixels (if known).
    pub height: Option<u32>,
    /// File size in bytes.
    pub size_bytes: usize,
}

impl ImageAttachment {
    /// Load an image from a file path.
    pub fn from_file(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read image file: {}", path.display()))?;
        Self::from_bytes(&data, Some(path))
    }

    /// Create an image attachment from raw bytes.
    pub fn from_bytes(data: &[u8], path: Option<&Path>) -> Result<Self> {
        let format = ImageFormat::from_magic_bytes(data)
            .or_else(|| {
                path.and_then(|p| p.extension())
                    .and_then(|e| e.to_str())
                    .and_then(ImageFormat::from_extension)
            })
            .context("Unsupported or unrecognized image format")?;

        // Validate size (max 20MB for safety)
        const MAX_IMAGE_SIZE: usize = 20 * 1024 * 1024;
        if data.len() > MAX_IMAGE_SIZE {
            anyhow::bail!(
                "Image too large: {} bytes (max {} bytes)",
                data.len(),
                MAX_IMAGE_SIZE
            );
        }

        let base64_data = {
            use base64::engine::general_purpose::STANDARD;
            use base64::Engine;
            STANDARD.encode(data)
        };

        let filename = path
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(String::from);

        Ok(Self {
            base64_data,
            format,
            filename,
            width: None,
            height: None,
            size_bytes: data.len(),
        })
    }

    /// Get the data URL for this image (data:image/xxx;base64,...).
    pub fn data_url(&self) -> String {
        format!(
            "data:{};base64,{}",
            self.format.mime_type(),
            self.base64_data
        )
    }

    /// Get the content part for OpenAI API format.
    pub fn to_openai_content(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "image_url",
            "image_url": {
                "url": self.data_url(),
                "detail": "auto"
            }
        })
    }

    /// Get the content part for Anthropic API format.
    pub fn to_anthropic_content(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": self.format.mime_type(),
                "data": self.base64_data
            }
        })
    }
}

/// A multimodal message that can contain text and images.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultimodalMessage {
    /// Text content of the message.
    pub text: String,
    /// Image attachments.
    pub images: Vec<ImageAttachment>,
}

impl MultimodalMessage {
    /// Create a text-only message.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            images: Vec::new(),
        }
    }

    /// Create a message with a single image.
    pub fn with_image(text: impl Into<String>, image: ImageAttachment) -> Self {
        Self {
            text: text.into(),
            images: vec![image],
        }
    }

    /// Create a message with multiple images.
    pub fn with_images(text: impl Into<String>, images: Vec<ImageAttachment>) -> Self {
        Self {
            text: text.into(),
            images,
        }
    }

    /// Load from a text prompt and image file paths.
    pub fn from_files(text: impl Into<String>, paths: &[&str]) -> Result<Self> {
        let mut images = Vec::new();
        for path in paths {
            images.push(ImageAttachment::from_file(Path::new(path))?);
        }
        Ok(Self {
            text: text.into(),
            images,
        })
    }

    /// Check if this message has images.
    pub fn has_images(&self) -> bool {
        !self.images.is_empty()
    }

    /// Convert to OpenAI message format.
    pub fn to_openai_format(&self) -> serde_json::Value {
        if self.images.is_empty() {
            return serde_json::json!({
                "role": "user",
                "content": self.text
            });
        }

        let mut content = vec![serde_json::json!({
            "type": "text",
            "text": self.text
        })];

        for img in &self.images {
            content.push(img.to_openai_content());
        }

        serde_json::json!({
            "role": "user",
            "content": content
        })
    }

    /// Convert to Anthropic message format.
    pub fn to_anthropic_format(&self) -> serde_json::Value {
        if self.images.is_empty() {
            return serde_json::json!({
                "role": "user",
                "content": self.text
            });
        }

        let mut content = vec![serde_json::json!({
            "type": "text",
            "text": self.text
        })];

        for img in &self.images {
            content.push(img.to_anthropic_content());
        }

        serde_json::json!({
            "role": "user",
            "content": content
        })
    }
}

/// Parse --image arguments from CLI.
/// Returns (text_message, image_paths).
pub fn parse_image_args(args: &[String]) -> (String, Vec<String>) {
    let mut text_parts = Vec::new();
    let mut image_paths = Vec::new();

    for arg in args {
        if let Some(path) = arg.strip_prefix("--image=") {
            image_paths.push(path.to_string());
        } else if arg == "--image" {
            // Next arg should be the path (handled by caller)
        } else {
            text_parts.push(arg.clone());
        }
    }

    (text_parts.join(" "), image_paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_png_magic_bytes() {
        let data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A];
        assert_eq!(ImageFormat::from_magic_bytes(&data), Some(ImageFormat::Png));
    }

    #[test]
    fn test_jpeg_magic_bytes() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(
            ImageFormat::from_magic_bytes(&data),
            Some(ImageFormat::Jpeg)
        );
    }

    #[test]
    fn test_gif_magic_bytes() {
        let data = b"GIF89a";
        assert_eq!(ImageFormat::from_magic_bytes(data), Some(ImageFormat::Gif));
    }

    #[test]
    fn test_webp_magic_bytes() {
        let data = b"RIFF\x00\x00\x00\x00WEBP".to_vec();
        assert_eq!(
            ImageFormat::from_magic_bytes(&data),
            Some(ImageFormat::WebP)
        );
    }

    #[test]
    fn test_unknown_format() {
        let data = [0x00, 0x01, 0x02, 0x03];
        assert_eq!(ImageFormat::from_magic_bytes(&data), None);
    }

    #[test]
    fn test_extension_detection() {
        assert_eq!(ImageFormat::from_extension("png"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("jpg"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_extension("jpeg"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_extension("gif"), Some(ImageFormat::Gif));
        assert_eq!(ImageFormat::from_extension("webp"), Some(ImageFormat::WebP));
        assert_eq!(ImageFormat::from_extension("txt"), None);
    }

    #[test]
    fn test_mime_types() {
        assert_eq!(ImageFormat::Png.mime_type(), "image/png");
        assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(ImageFormat::Gif.mime_type(), "image/gif");
        assert_eq!(ImageFormat::WebP.mime_type(), "image/webp");
    }

    #[test]
    fn test_multimodal_message_text_only() {
        let msg = MultimodalMessage::text("Hello");
        assert!(!msg.has_images());
        let openai = msg.to_openai_format();
        assert_eq!(openai["content"], "Hello");
    }

    #[test]
    fn test_image_too_large_rejected() {
        let large_data = vec![0xFFu8; 21 * 1024 * 1024]; // 21MB
        let result = ImageAttachment::from_bytes(&large_data, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_image_args() {
        let args = vec![
            "Hello".to_string(),
            "--image=diagram.png".to_string(),
            "describe this".to_string(),
        ];
        let (text, paths) = parse_image_args(&args);
        assert_eq!(text, "Hello describe this");
        assert_eq!(paths, vec!["diagram.png"]);
    }

    #[test]
    fn test_google_content_format() {
        let img = ImageAttachment {
            base64_data: "abc123".to_string(),
            format: ImageFormat::Png,
            filename: None,
            width: None,
            height: None,
            size_bytes: 6,
        };
        let content = img.to_google_content();
        assert_eq!(content["inline_data"]["mime_type"], "image/png");
        assert_eq!(content["inline_data"]["data"], "abc123");
    }

    #[test]
    fn test_multimodal_message_google_format() {
        let img = ImageAttachment {
            base64_data: "data".to_string(),
            format: ImageFormat::Jpeg,
            filename: None,
            width: None,
            height: None,
            size_bytes: 4,
        };
        let msg = MultimodalMessage::with_image("Describe this", img);
        let google = msg.to_google_format();
        assert_eq!(google["role"], "user");
        assert!(google["parts"].is_array());
        assert_eq!(google["parts"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_chat_message_with_images() {
        use crate::llm::messages::{ChatMessage, ChatRole};
        use crate::multimodal::{ImageAttachment, ImageFormat};

        let img = ImageAttachment {
            base64_data: "test".to_string(),
            format: ImageFormat::Png,
            filename: None,
            width: None,
            height: None,
            size_bytes: 4,
        };

        let msg = ChatMessage::with_images(ChatRole::User, "What's in this image?", vec![img]);

        assert!(msg.has_images());
        assert_eq!(msg.content, "What's in this image?");
        assert_eq!(msg.images.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_chat_message_text_only() {
        use crate::llm::messages::{ChatMessage, ChatRole};

        let msg = ChatMessage::text(ChatRole::User, "Hello");
        assert!(!msg.has_images());
        assert!(msg.images.is_none());
    }
}
