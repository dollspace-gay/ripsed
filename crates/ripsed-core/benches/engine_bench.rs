use criterion::{Criterion, black_box, criterion_group, criterion_main};
use ripsed_core::engine::apply;
use ripsed_core::matcher::Matcher;
use ripsed_core::operation::Op;

/// Generate a text buffer with `n` lines, each line being "line NNN: the quick brown fox
/// jumps over the lazy dog".
fn generate_text(n: usize) -> String {
    let mut buf = String::with_capacity(n * 60);
    for i in 1..=n {
        buf.push_str(&format!(
            "line {i}: the quick brown fox jumps over the lazy dog\n"
        ));
    }
    buf
}

fn bench_simple_replace(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_replace");

    for &size in &[100, 1_000, 10_000] {
        let text = generate_text(size);
        let op = Op::Replace {
            find: "fox".to_string(),
            replace: "cat".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();

        group.bench_function(format!("{size}_lines"), |b| {
            b.iter(|| {
                let result = apply(black_box(&text), &op, &matcher, None, 0).unwrap();
                black_box(result);
            });
        });
    }

    group.finish();
}

fn bench_regex_replace_with_captures(c: &mut Criterion) {
    let text = generate_text(1_000);
    let op = Op::Replace {
        find: r"(quick) (brown)".to_string(),
        replace: "$2 $1".to_string(),
        regex: true,
        case_insensitive: false,
    };
    let matcher = Matcher::new(&op).unwrap();

    c.bench_function("regex_replace_captures_1000_lines", |b| {
        b.iter(|| {
            let result = apply(black_box(&text), &op, &matcher, None, 0).unwrap();
            black_box(result);
        });
    });
}

fn bench_delete(c: &mut Criterion) {
    let text = generate_text(1_000);
    let op = Op::Delete {
        find: "fox".to_string(),
        regex: false,
        case_insensitive: false,
    };
    let matcher = Matcher::new(&op).unwrap();

    c.bench_function("delete_1000_lines", |b| {
        b.iter(|| {
            let result = apply(black_box(&text), &op, &matcher, None, 0).unwrap();
            black_box(result);
        });
    });
}

fn bench_case_insensitive_replace(c: &mut Criterion) {
    let text = generate_text(1_000);
    let op = Op::Replace {
        find: "FOX".to_string(),
        replace: "cat".to_string(),
        regex: false,
        case_insensitive: true,
    };
    let matcher = Matcher::new(&op).unwrap();

    c.bench_function("case_insensitive_replace_1000_lines", |b| {
        b.iter(|| {
            let result = apply(black_box(&text), &op, &matcher, None, 0).unwrap();
            black_box(result);
        });
    });
}

criterion_group!(
    benches,
    bench_simple_replace,
    bench_regex_replace_with_captures,
    bench_delete,
    bench_case_insensitive_replace,
);
criterion_main!(benches);
