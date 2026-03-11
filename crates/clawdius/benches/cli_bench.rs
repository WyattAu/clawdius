use clap::Parser;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::path::PathBuf;

#[derive(Parser)]
struct BenchCli {
    #[arg(short, long)]
    no_tui: bool,

    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,

    #[arg(short = 'f', long, default_value = "text")]
    output_format: String,

    #[arg(short, long)]
    quiet: bool,
}

fn bench_cli_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("cli_parsing");

    let test_cases = vec![
        ("minimal", vec!["clawdius"]),
        ("with_flags", vec!["clawdius", "--no-tui", "--quiet"]),
        ("with_cwd", vec!["clawdius", "-c", "/home/user/project"]),
        (
            "full_options",
            vec!["clawdius", "--no-tui", "-c", ".", "-f", "json", "--quiet"],
        ),
    ];

    for (name, args) in test_cases {
        group.bench_with_input(BenchmarkId::new("parse", name), &args, |b, args| {
            b.iter(|| black_box(BenchCli::try_parse_from(args)));
        });
    }

    group.finish();
}

fn bench_output_formatting(c: &mut Criterion) {
    let mut group = c.benchmark_group("output_formatting");

    let data = serde_json::json!({
        "session_id": "123e4567-e89b-12d3-a456-426614174000",
        "messages": [
            {
                "role": "user",
                "content": "Hello, world!"
            },
            {
                "role": "assistant",
                "content": "Hi there! How can I help you today?"
            }
        ],
        "metadata": {
            "provider": "anthropic",
            "model": "claude-3-5-sonnet",
            "tokens": {
                "input": 150,
                "output": 250
            }
        }
    });

    group.bench_function("json_serialize_compact", |b| {
        b.iter(|| black_box(serde_json::to_string(&data)));
    });

    group.bench_function("json_serialize_pretty", |b| {
        b.iter(|| black_box(serde_json::to_string_pretty(&data)));
    });

    let json_str = serde_json::to_string(&data).unwrap();
    group.throughput(Throughput::Bytes(json_str.len() as u64));
    group.bench_function("json_deserialize", |b| {
        b.iter(|| black_box(serde_json::from_str::<serde_json::Value>(&json_str)));
    });

    group.finish();
}

fn bench_tui_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("tui_components");

    group.bench_function("text_styling", |b| {
        use ratatui::style::{Color, Modifier, Style};

        b.iter(|| {
            black_box(
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD | Modifier::ITALIC),
            )
        });
    });

    group.bench_function("paragraph_creation", |b| {
        use ratatui::style::{Color, Style};
        use ratatui::widgets::Paragraph;

        let text = "This is a sample text for benchmarking TUI rendering components.";

        b.iter(|| black_box(Paragraph::new(text).style(Style::default().fg(Color::Green))));
    });

    group.bench_function("layout_calculation", |b| {
        use ratatui::layout::{Constraint, Direction, Layout, Rect};

        let area = Rect::new(0, 0, 80, 24);

        b.iter(|| {
            black_box(
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(1),
                        Constraint::Length(3),
                    ])
                    .split(area),
            )
        });
    });

    group.bench_function("buffer_rendering", |b| {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use ratatui::style::{Color, Style};

        let area = Rect::new(0, 0, 80, 24);

        b.iter(|| {
            let mut buffer = Buffer::empty(area);
            for y in 0..area.height {
                for x in 0..area.width {
                    let cell = buffer.get_mut(x, y);
                    cell.set_char(' ');
                    cell.set_style(Style::default().bg(Color::Black));
                }
            }
            black_box(buffer)
        });
    });

    group.finish();
}

fn bench_string_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_operations");

    let text = "This is a sample text that would be displayed in the TUI interface.";

    group.throughput(Throughput::Bytes(text.len() as u64));
    group.bench_function("text_truncation", |b| {
        b.iter(|| {
            let max_width = 40;
            let truncated = if text.len() > max_width {
                format!("{}...", &text[..max_width - 3])
            } else {
                text.to_string()
            };
            black_box(truncated)
        });
    });

    group.bench_function("text_wrapping", |b| {
        let text = "This is a longer text that needs to be wrapped across multiple lines in the terminal interface for proper display.";

        b.iter(|| {
            let width = 40;
            let wrapped: Vec<String> = text
                .chars()
                .collect::<Vec<_>>()
                .chunks(width)
                .map(|chunk| chunk.iter().collect())
                .collect();
            black_box(wrapped)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_cli_parsing,
    bench_output_formatting,
    bench_tui_components,
    bench_string_operations,
);
criterion_main!(benches);
