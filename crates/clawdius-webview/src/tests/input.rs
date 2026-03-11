use crate::components::input::FileAttachment;

#[test]
fn test_file_attachment_creation() {
    let attachment = FileAttachment {
        name: "test.txt".to_string(),
        content: "SGVsbG8gV29ybGQ=".to_string(),
        mime_type: "text/plain".to_string(),
    };

    assert_eq!(attachment.name, "test.txt");
    assert_eq!(attachment.mime_type, "text/plain");
}

#[test]
fn test_file_attachment_clone() {
    let attachment = FileAttachment {
        name: "image.png".to_string(),
        content: "base64data".to_string(),
        mime_type: "image/png".to_string(),
    };

    let cloned = attachment.clone();
    assert_eq!(attachment.name, cloned.name);
    assert_eq!(attachment.mime_type, cloned.mime_type);
}

#[test]
fn test_file_attachment_debug() {
    let attachment = FileAttachment {
        name: "doc.pdf".to_string(),
        content: "data".to_string(),
        mime_type: "application/pdf".to_string(),
    };

    let debug_str = format!("{:?}", attachment);
    assert!(debug_str.contains("doc.pdf"));
    assert!(debug_str.contains("application/pdf"));
}
