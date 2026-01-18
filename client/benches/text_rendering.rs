use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use common::text_frame::TextFrame;
use pinhole::text_renderer::TextRenderer;


fn bench_render_various_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_frame_sizes");

    let sizes = vec![
        (40, 20),   // Small
        (80, 30),   // Medium
        (120, 40),  // Default
        (160, 50),  // Large
    ];

    for (width, height) in sizes {
        let total_chars = width * height;

        group.bench_with_input(
            BenchmarkId::new("full_frame", format!("{}x{}", width, height)),
            &(width, height),
            |b, &(w, h)| {
                let mut renderer = TextRenderer::new().unwrap();
                let mut frame = TextFrame::new(w, h, ' ').unwrap();

                // Fill entire frame with characters (worst case)
                for i in 0..(w * h) {
                    let ch = match i % 4 {
                        0 => 'A',
                        1 => '■',
                        2 => '━',
                        _ => '│',
                    };
                    frame.chars_mut()[i] = ch;
                }

                b.iter(|| {
                    black_box(renderer.prepare_buffer(black_box(&frame)).unwrap());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("50%_changes", format!("{}x{}", width, height)),
            &(width, height),
            |b, &(w, h)| {
                let mut renderer = TextRenderer::new().unwrap();
                let mut frame1 = TextFrame::new(w, h, ' ').unwrap();
                let mut frame2 = TextFrame::new(w, h, ' ').unwrap();

                // Fill 50% of chars
                for i in (0..(w * h)).step_by(2) {
                    frame2.chars_mut()[i] = 'X';
                }

                // Initialize with first frame
                renderer.prepare_buffer(&frame1).unwrap();

                b.iter(|| {
                    black_box(renderer.prepare_buffer(black_box(&frame2)).unwrap());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("10%_changes", format!("{}x{}", width, height)),
            &(width, height),
            |b, &(w, h)| {
                let mut renderer = TextRenderer::new().unwrap();
                let mut frame1 = TextFrame::new(w, h, ' ').unwrap();
                let mut frame2 = TextFrame::new(w, h, ' ').unwrap();

                // Fill 10% of chars
                for i in (0..(w * h)).step_by(10) {
                    frame2.chars_mut()[i] = 'Y';
                }

                // Initialize with first frame
                renderer.prepare_buffer(&frame1).unwrap();

                b.iter(|| {
                    black_box(renderer.prepare_buffer(black_box(&frame2)).unwrap());
                });
            },
        );
    }

    group.finish();
}

fn bench_utf8_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("utf8_encoding");

    group.bench_function("ascii_only", |b| {
        let mut renderer = TextRenderer::new().unwrap();
        let mut frame = TextFrame::new(120, 40, ' ').unwrap();

        // Fill with ASCII characters (1 byte each)
        for ch in frame.chars_mut() {
            *ch = 'A';
        }

        b.iter(|| {
            black_box(renderer.prepare_buffer(black_box(&frame)).unwrap());
        });
    });

    group.bench_function("box_drawing_chars", |b| {
        let mut renderer = TextRenderer::new().unwrap();
        let mut frame = TextFrame::new(120, 40, ' ').unwrap();

        // Fill with box drawing characters (3 bytes each in UTF-8)
        for ch in frame.chars_mut() {
            *ch = '━';
        }

        b.iter(|| {
            black_box(renderer.prepare_buffer(black_box(&frame)).unwrap());
        });
    });

    group.bench_function("mixed_chars", |b| {
        let mut renderer = TextRenderer::new().unwrap();
        let mut frame = TextFrame::new(120, 40, ' ').unwrap();

        // Mix of ASCII and multi-byte UTF-8
        for (i, ch) in frame.chars_mut().iter_mut().enumerate() {
            *ch = match i % 5 {
                0 => ' ',
                1 => '.',
                2 => '@',
                3 => '■',
                _ => '━',
            };
        }

        b.iter(|| {
            black_box(renderer.prepare_buffer(black_box(&frame)).unwrap());
        });
    });

    group.finish();
}

fn bench_buffer_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_operations");

    group.bench_function("buffer_clear_and_reuse", |b| {
        let mut renderer = TextRenderer::new().unwrap();
        let mut frame1 = TextFrame::new(120, 40, ' ').unwrap();
        let mut frame2 = TextFrame::new(120, 40, 'X').unwrap();

        b.iter(|| {
            black_box(renderer.prepare_buffer(black_box(&frame1)).unwrap());
            black_box(renderer.prepare_buffer(black_box(&frame2)).unwrap());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_render_various_sizes,
    bench_utf8_encoding,
    bench_buffer_operations
);
criterion_main!(benches);
