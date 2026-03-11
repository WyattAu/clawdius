use crate::components::message::{html_escape, render_markdown};

#[test]
fn test_html_escape_basic() {
    assert_eq!(html_escape("&"), "&amp;");
    assert_eq!(html_escape("<"), "&lt;");
    assert_eq!(html_escape(">"), "&gt;");
    assert_eq!(html_escape("\""), "&quot;");
    assert_eq!(html_escape("'"), "&#39;");
}

#[test]
fn test_html_escape_complex() {
    let input = "<script>alert('xss')</script>";
    let escaped = html_escape(input);
    assert_eq!(escaped, "&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;");
}

#[test]
fn test_render_markdown_paragraphs() {
    let input = "Hello world";
    let output = render_markdown(input);
    assert!(output.contains("<p>Hello world</p>"));
}

#[test]
fn test_render_markdown_headers() {
    let h1 = render_markdown("# Title");
    assert!(h1.contains("<h1>Title</h1>"));

    let h2 = render_markdown("## Subtitle");
    assert!(h2.contains("<h2>Subtitle</h2>"));

    let h3 = render_markdown("### Section");
    assert!(h3.contains("<h3>Section</h3>"));
}

#[test]
fn test_render_markdown_code_blocks() {
    let input = "```rust\nfn main() {}\n```";
    let output = render_markdown(input);
    assert!(output.contains("<pre class=\"code-block\" data-language=\"rust\">"));
    assert!(output.contains("<code>"));
    assert!(output.contains("</code></pre>"));
}

#[test]
fn test_render_markdown_lists() {
    let input = "- Item 1\n- Item 2";
    let output = render_markdown(input);
    assert!(output.contains("<ul>"));
    assert!(output.contains("<li>Item 1</li>"));
    assert!(output.contains("<li>Item 2</li>"));
    assert!(output.contains("</ul>"));
}

#[test]
fn test_render_markdown_empty() {
    let output = render_markdown("");
    assert!(output.is_empty());
}

#[test]
fn test_render_markdown_multiple_paragraphs() {
    let input = "First paragraph\n\nSecond paragraph";
    let output = render_markdown(input);
    assert!(output.contains("<p>First paragraph</p>"));
    assert!(output.contains("<br/>"));
    assert!(output.contains("<p>Second paragraph</p>"));
}
