use clawdius_core::tools::file::{FileListParams, FileReadParams, FileTool, FileWriteParams};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::fs;
use tempfile::TempDir;

fn bench_file_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_read");
    let temp_dir = TempDir::new().unwrap();
    let tool = FileTool::new();

    let small_file = temp_dir.path().join("small.txt");
    let small_content: String = (0..20).map(|i| format!("Line {i}\n")).collect();
    fs::write(&small_file, &small_content).unwrap();

    group.throughput(Throughput::Bytes(small_content.len() as u64));
    group.bench_with_input(BenchmarkId::new("read", "small"), &small_file, |b, path| {
        b.iter(|| {
            tool.read(black_box(FileReadParams {
                path: path.to_string_lossy().to_string(),
                offset: None,
                limit: None,
            }))
        });
    });

    let medium_file = temp_dir.path().join("medium.txt");
    let medium_content: String = (0..2000).map(|i| format!("Line {i}\n")).collect();
    fs::write(&medium_file, &medium_content).unwrap();

    group.throughput(Throughput::Bytes(medium_content.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("read", "medium"),
        &medium_file,
        |b, path| {
            b.iter(|| {
                tool.read(black_box(FileReadParams {
                    path: path.to_string_lossy().to_string(),
                    offset: None,
                    limit: None,
                }))
            });
        },
    );

    let large_file = temp_dir.path().join("large.txt");
    let large_content: String = (0..20000).map(|i| format!("Line {i}\n")).collect();
    fs::write(&large_file, &large_content).unwrap();

    group.throughput(Throughput::Bytes(large_content.len() as u64));
    group.bench_with_input(BenchmarkId::new("read", "large"), &large_file, |b, path| {
        b.iter(|| {
            tool.read(black_box(FileReadParams {
                path: path.to_string_lossy().to_string(),
                offset: None,
                limit: None,
            }))
        });
    });

    group.finish();
}

fn bench_file_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_write");
    let temp_dir = TempDir::new().unwrap();
    let tool = FileTool::new();

    let small_content: String = (0..20).map(|i| format!("Line {i}\n")).collect();
    group.throughput(Throughput::Bytes(small_content.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("write", "small"),
        &small_content,
        |b, content| {
            b.iter(|| {
                let path = temp_dir
                    .path()
                    .join(format!("write_small_{}.txt", std::process::id()));
                tool.write(black_box(FileWriteParams {
                    path: path.to_string_lossy().to_string(),
                    content: content.clone(),
                }))
            });
        },
    );

    let medium_content: String = (0..2000).map(|i| format!("Line {i}\n")).collect();
    group.throughput(Throughput::Bytes(medium_content.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("write", "medium"),
        &medium_content,
        |b, content| {
            b.iter(|| {
                let path = temp_dir
                    .path()
                    .join(format!("write_medium_{}.txt", std::process::id()));
                tool.write(black_box(FileWriteParams {
                    path: path.to_string_lossy().to_string(),
                    content: content.clone(),
                }))
            });
        },
    );

    group.finish();
}

fn bench_file_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_list");
    let temp_dir = TempDir::new().unwrap();
    let tool = FileTool::new();

    for i in 0..100 {
        fs::write(temp_dir.path().join(format!("file_{i:03}.txt")), "content").unwrap();
    }

    group.bench_function("list_100_files", |b| {
        b.iter(|| {
            tool.list(black_box(FileListParams {
                path: temp_dir.path().to_string_lossy().to_string(),
            }))
        });
    });

    group.finish();
}

fn bench_file_read_with_offset_limit(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_read_offset_limit");
    let temp_dir = TempDir::new().unwrap();
    let tool = FileTool::new();

    let file = temp_dir.path().join("large.txt");
    let content: String = (0..10000).map(|i| format!("Line {i}\n")).collect();
    fs::write(&file, &content).unwrap();

    group.bench_function("read_first_100_lines", |b| {
        b.iter(|| {
            tool.read(black_box(FileReadParams {
                path: file.to_string_lossy().to_string(),
                offset: Some(0),
                limit: Some(100),
            }))
        });
    });

    group.bench_function("read_middle_100_lines", |b| {
        b.iter(|| {
            tool.read(black_box(FileReadParams {
                path: file.to_string_lossy().to_string(),
                offset: Some(5000),
                limit: Some(100),
            }))
        });
    });

    group.bench_function("read_last_100_lines", |b| {
        b.iter(|| {
            tool.read(black_box(FileReadParams {
                path: file.to_string_lossy().to_string(),
                offset: Some(9900),
                limit: Some(100),
            }))
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_file_read,
    bench_file_write,
    bench_file_list,
    bench_file_read_with_offset_limit,
);
criterion_main!(benches);
