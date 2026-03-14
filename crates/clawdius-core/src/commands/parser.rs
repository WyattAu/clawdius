//! Command parser

use crate::commands::{CommandArgument, CommandTemplate, CustomCommand, TemplateStep};
use crate::error::Result;
use regex::Regex;

/// Parse a custom command from markdown
pub struct CommandParser;

impl CommandParser {
    /// Parse a command from markdown content
    pub fn parse(content: &str) -> Result<CustomCommand> {
        // Simple parser for command markdown files
        let lines: Vec<&str> = content.lines().collect();

        let name = lines
            .first()
            .map_or("Untitled", |l| l.trim_start_matches('#').trim())
            .to_string();

        // Parse frontmatter section for arguments
        let mut arguments = Vec::new();
        let mut steps = Vec::new();
        let mut current_section = None;

        for line in lines.iter().skip(1) {
            let trimmed = line.trim();

            // Skip empty lines at the beginning
            if trimmed.is_empty() && current_section.is_none() {
                continue;
            }

            // Check for section headers
            if trimmed.starts_with("## Arguments") || trimmed.starts_with("## Params") {
                current_section = Some("arguments");
                continue;
            }

            if trimmed.starts_with("## Steps") {
                current_section = Some("steps");
                continue;
            }

            // Parse arguments section
            if current_section == Some("arguments") {
                if let Some(arg) = Self::parse_argument_line(trimmed) {
                    arguments.push(arg);
                }
            }

            // Parse steps section
            if current_section == Some("steps") {
                if let Some(step) = Self::parse_step_line(trimmed) {
                    steps.push(step);
                }
            }

            // Check for property lines (name: value)
            if trimmed.starts_with("name:") {
                // Already handled above
            } else if trimmed.starts_with("description:") {
                // Description is parsed separately
            } else if !trimmed.starts_with('#') && !trimmed.is_empty() {
                // This is content that might be a step
                if current_section == Some("steps") {
                    if let Some(step) = steps.last_mut() {
                        step.template.push('\n');
                        step.template.push_str(trimmed);
                    }
                }
            }
        }

        Ok(CustomCommand {
            id: name.to_lowercase().replace(' ', "-"),
            name,
            description: String::new(),
            template: CommandTemplate {
                steps,
                arguments: Vec::new(), // Arguments are stored separately in CustomCommand
                allow_extra_args: false,
            },
            arguments,
        })
    }

    /// Parse an argument line in the format:
    /// `name: description [required] [default:value]`
    fn parse_argument_line(line: &str) -> Option<CommandArgument> {
        let line = line.trim();

        // Skip empty lines or lines that don't look like arguments
        if line.is_empty() || !line.contains(':') {
            return None;
        }

        // Format: `name: description` or `name: description required` or `name: description default:value`
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() < 2 {
            return None;
        }

        let name = parts[0].trim().to_string();
        let rest = parts[1].trim();

        let mut description = String::new();
        let mut required = false;
        let mut default_val = None;

        // Parse the rest for modifiers
        let rest_parts: Vec<&str> = rest.split_whitespace().collect();
        for (i, part) in rest_parts.iter().enumerate() {
            if *part == "required" {
                required = true;
            } else if part.starts_with("default:") {
                default_val = Some(part.trim_start_matches("default:").to_string());
            } else if i == 0 || !description.is_empty() {
                // Build description from remaining words
                if !description.is_empty() {
                    description.push(' ');
                }
                description.push_str(part);
            }
        }

        if name.is_empty() || description.is_empty() {
            return None;
        }

        Some(CommandArgument {
            name,
            description,
            required,
            default: default_val,
        })
    }

    /// Parse a step line in the format:
    /// `- tool: description`
    fn parse_step_line(line: &str) -> Option<TemplateStep> {
        let line = line.trim();

        if line.starts_with("- ") {
            let rest = line.trim_start_matches("- ");
            let parts: Vec<&str> = rest.splitn(2, ':').collect();
            if parts.len() < 2 {
                return None;
            }

            let tool = parts[0].trim().to_string();
            let description = parts[1].trim().to_string();

            Some(TemplateStep {
                tool,
                template: String::new(), // Will be filled by subsequent lines
                description,
            })
        } else {
            None
        }
    }

    /// Extract variables from template content
    /// Variables are in the format {{variable_name}}
    #[must_use]
    pub fn extract_variables(template: &str) -> Vec<String> {
        let re = Regex::new(r"\{\{(\w+)\}\}").unwrap();
        re.captures_iter(template)
            .filter_map(|cap| Some(cap[1].to_string()))
            .collect()
    }
}
