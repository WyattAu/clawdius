use leptos::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
        }
    }
}

pub fn render_markdown(content: &str) -> String {
    let mut result = String::new();
    let mut in_code_block = false;
    let mut code_lang;
    let mut in_list = false;
    let lines: Vec<&str> = content.lines().collect();

    for line in lines.iter() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            if in_code_block {
                result.push_str("</code></pre>\n");
                in_code_block = false;
            } else {
                code_lang = trimmed[3..].to_string();
                result.push_str(&format!(
                    "<pre class=\"code-block\" data-language=\"{}\"><code>",
                    code_lang
                ));
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            result.push_str(&html_escape(line));
            result.push('\n');
            continue;
        }

        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if !in_list {
                result.push_str("<ul>\n");
                in_list = true;
            }
            let item = process_inline_markdown(&trimmed[2..]);
            result.push_str(&format!("<li>{}</li>\n", item));
            continue;
        } else if in_list {
            result.push_str("</ul>\n");
            in_list = false;
        }

        if trimmed.starts_with("# ") {
            let text = process_inline_markdown(&trimmed[2..]);
            result.push_str(&format!("<h1>{}</h1>\n", text));
        } else if trimmed.starts_with("## ") {
            let text = process_inline_markdown(&trimmed[3..]);
            result.push_str(&format!("<h2>{}</h2>\n", text));
        } else if trimmed.starts_with("### ") {
            let text = process_inline_markdown(&trimmed[4..]);
            result.push_str(&format!("<h3>{}</h3>\n", text));
        } else if trimmed.is_empty() {
            result.push_str("<br/>\n");
        } else {
            let text = process_inline_markdown(trimmed);
            result.push_str(&format!("<p>{}</p>\n", text));
        }
    }

    if in_code_block {
        result.push_str("</code></pre>\n");
    }
    if in_list {
        result.push_str("</ul>\n");
    }

    result
}

fn process_inline_markdown(text: &str) -> String {
    let result = text.to_string();

    let mut new_result = String::new();
    let mut chars = result.chars().peekable();
    let mut in_code = false;

    while let Some(c) = chars.next() {
        if c == '`' {
            if in_code {
                new_result.push_str("</code>");
                in_code = false;
            } else {
                new_result.push_str("<code class=\"inline-code\">");
                in_code = true;
            }
        } else if c == '*' && chars.peek() == Some(&'*') {
            chars.next();
            let mut bold_text = String::new();
            while let Some(&next) = chars.peek() {
                if next == '*' {
                    chars.next();
                    if chars.peek() == Some(&'*') {
                        chars.next();
                        break;
                    }
                } else {
                    bold_text.push(chars.next().unwrap());
                }
            }
            new_result.push_str(&format!("<strong>{}</strong>", html_escape(&bold_text)));
        } else if c == '*' {
            let mut italic_text = String::new();
            while let Some(&next) = chars.peek() {
                if next == '*' {
                    chars.next();
                    break;
                } else {
                    italic_text.push(chars.next().unwrap());
                }
            }
            new_result.push_str(&format!("<em>{}</em>", html_escape(&italic_text)));
        } else if c == '[' {
            let mut link_text = String::new();
            let mut found_closing = false;
            while let Some(&next) = chars.peek() {
                chars.next();
                if next == ']' {
                    found_closing = true;
                    break;
                }
                link_text.push(next);
            }
            if found_closing && chars.peek() == Some(&'(') {
                chars.next();
                let mut url = String::new();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next == ')' {
                        break;
                    }
                    url.push(next);
                }
                new_result.push_str(&format!(
                    "<a href=\"{}\" target=\"_blank\">{}</a>",
                    html_escape(&url),
                    html_escape(&link_text)
                ));
            } else {
                new_result.push('[');
                new_result.push_str(&link_text);
                new_result.push(']');
            }
        } else {
            new_result.push(c);
        }
    }

    new_result
}

pub fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[component]
pub fn MessageComponent(message: Message) -> impl IntoView {
    let role_class = match message.role {
        MessageRole::User => "message-user",
        MessageRole::Assistant => "message-assistant",
        MessageRole::System => "message-system",
    };

    let content_html = render_markdown(&message.content);
    let content_copy = message.content.clone();

    view! {
        <div class=format!("message {}", role_class)>
            <div class="message-header">
                <span class="message-role">{message.role.as_str()}</span>
                <span class="message-timestamp">{message.timestamp.clone()}</span>
                <button
                    class="copy-button"
                    title="Copy message"
                    on:click=move |_| {
                        if let Some(window) = web_sys::window() {
                            let _ = window.navigator().clipboard().write_text(&content_copy);
                        }
                    }
                >
                    "📋"
                </button>
            </div>
            <div
                class="message-content markdown-content"
                inner_html=content_html
            />
        </div>
    }
}
