use clawdius_core::tools::web_search::{
    format_grounded_response, format_results_for_llm, GroundedResponse, SearchProvider,
    SearchResult, WebSearchTool,
};

#[tokio::test]
async fn test_search_result_structure() {
    let result = SearchResult {
        title: "Rust Programming Language".to_string(),
        url: "https://www.rust-lang.org/".to_string(),
        snippet: "A language empowering everyone to build reliable software.".to_string(),
        source: "DuckDuckGo".to_string(),
    };

    assert_eq!(result.title, "Rust Programming Language");
    assert_eq!(result.url, "https://www.rust-lang.org/");
    assert!(!result.snippet.is_empty());
}

#[tokio::test]
async fn test_format_results_for_llm() {
    let results = vec![
        SearchResult {
            title: "Result 1".to_string(),
            url: "https://example.com/1".to_string(),
            snippet: "First result snippet".to_string(),
            source: "DuckDuckGo".to_string(),
        },
        SearchResult {
            title: "Result 2".to_string(),
            url: "https://example.com/2".to_string(),
            snippet: "Second result snippet".to_string(),
            source: "DuckDuckGo".to_string(),
        },
    ];

    let formatted = format_results_for_llm(&results);

    assert!(formatted.contains("[1] Result 1"));
    assert!(formatted.contains("[2] Result 2"));
    assert!(formatted.contains("https://example.com/1"));
    assert!(formatted.contains("https://example.com/2"));
}

#[tokio::test]
async fn test_format_grounded_response() {
    let response = GroundedResponse {
        content: "Based on my research, Rust is a systems programming language.".to_string(),
        sources: vec![SearchResult {
            title: "Rust Lang".to_string(),
            url: "https://rust-lang.org".to_string(),
            snippet: "Official site".to_string(),
            source: "DuckDuckGo".to_string(),
        }],
        confidence: 0.95,
    };

    let formatted = format_grounded_response(&response);

    assert!(formatted.contains("Based on my research"));
    assert!(formatted.contains("**Sources:**"));
    assert!(formatted.contains("[Rust Lang](https://rust-lang.org)"));
}

#[tokio::test]
async fn test_web_search_tool_creation() {
    let tool = WebSearchTool::default();
    let tool_google = WebSearchTool::new(SearchProvider::Google {
        api_key: "test-key".to_string(),
        cse_id: "test-cse".to_string(),
    });
    let tool_bing = WebSearchTool::new(SearchProvider::Bing {
        api_key: "test-key".to_string(),
    });

    assert!(matches!(tool, WebSearchTool { .. }));
    assert!(matches!(tool_google, WebSearchTool { .. }));
    assert!(matches!(tool_bing, WebSearchTool { .. }));
}

#[tokio::test]
async fn test_search_provider_variants() {
    let ddg = SearchProvider::DuckDuckGo;
    let google = SearchProvider::Google {
        api_key: "test".to_string(),
        cse_id: "test".to_string(),
    };
    let bing = SearchProvider::Bing {
        api_key: "test".to_string(),
    };

    assert!(matches!(ddg, SearchProvider::DuckDuckGo));
    assert!(matches!(google, SearchProvider::Google { .. }));
    assert!(matches!(bing, SearchProvider::Bing { .. }));
}

#[tokio::test]
async fn test_search_result_serialization() {
    let result = SearchResult {
        title: "Test".to_string(),
        url: "https://test.com".to_string(),
        snippet: "Test snippet".to_string(),
        source: "DuckDuckGo".to_string(),
    };

    let json = serde_json::to_string(&result).expect("Failed to serialize");
    let parsed: SearchResult = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(parsed.title, result.title);
    assert_eq!(parsed.url, result.url);
}

#[tokio::test]
async fn test_grounded_response_serialization() {
    let response = GroundedResponse {
        content: "Test content".to_string(),
        sources: vec![],
        confidence: 0.9,
    };

    let json = serde_json::to_string(&response).expect("Failed to serialize");
    let parsed: GroundedResponse = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(parsed.content, response.content);
    assert_eq!(parsed.confidence, response.confidence);
}

#[tokio::test]
async fn test_empty_search_results() {
    let results: Vec<SearchResult> = vec![];
    let formatted = format_results_for_llm(&results);

    assert!(formatted.is_empty());
}

#[tokio::test]
async fn test_grounded_response_no_sources() {
    let response = GroundedResponse {
        content: "No sources available".to_string(),
        sources: vec![],
        confidence: 0.5,
    };

    let formatted = format_grounded_response(&response);

    assert!(formatted.contains("No sources available"));
    assert!(!formatted.contains("**Sources:**"));
}
