use delayed_coding::{
    decode_into, encode_events_into, encode_into, Decoder, Event, Model, Workspace,
};
use std::{hint::black_box, time::Instant};

fn median_ns(repeats: usize, mut run: impl FnMut()) -> f64 {
    let mut samples = Vec::with_capacity(7);
    for _ in 0..7 {
        let begin = Instant::now();
        for _ in 0..repeats {
            run();
        }
        samples.push(begin.elapsed().as_nanos() as f64 / repeats as f64);
    }
    samples.sort_by(f64::total_cmp);
    samples[3]
}

fn measure<const DELAY: u32>(name: &str, model: &Model, input: &[u32], setup_ns: f64) {
    let mut buffer = vec![0; input.len() * 2];
    let mut restored = vec![0; input.len()];
    let mut workspace = Workspace::with_capacity(input.len());
    let range = encode_into::<DELAY>(model, input, &mut buffer, &mut workspace).unwrap();
    decode_into::<DELAY>(model, &buffer[range.clone()], &mut restored).unwrap();
    assert_eq!(input, restored);
    let repeats = (262144 / input.len()).max(1);
    let encode_ns = median_ns(repeats, || {
        black_box(
            encode_into::<DELAY>(
                black_box(model),
                black_box(input),
                black_box(&mut buffer),
                black_box(&mut workspace),
            )
            .unwrap(),
        );
    });
    let decode_ns = median_ns(repeats, || {
        decode_into::<DELAY>(
            black_box(model),
            black_box(&buffer[range.clone()]),
            black_box(&mut restored),
        )
        .unwrap();
        black_box(&restored);
    });
    assert_eq!(input, restored);
    println!(
        "{name},{},{DELAY},1,{},{},{:.6},{:.4},{:.4},{:.1}",
        input.len(),
        range.len(),
        model.table_bytes(),
        range.len() as f64 * 8.0 / input.len() as f64,
        encode_ns / input.len() as f64,
        decode_ns / input.len() as f64,
        setup_ns
    );
}

fn switching(count: usize, models_count: usize) {
    let models: Vec<_> = (0..models_count)
        .map(|i| {
            let mut weights = vec![128; 256];
            weights[i % 256] += 32768;
            Model::new(&weights).unwrap()
        })
        .collect();
    let mut state = 123456u64;
    let events: Vec<_> = (0..count)
        .map(|i| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let model = &models[i % models_count];
            Event {
                model,
                symbol: model.lookup(state as u16).symbol,
            }
        })
        .collect();
    let mut buffer = vec![0; count * 2];
    let mut workspace = Workspace::with_capacity(count);
    let range = encode_events_into::<24>(&events, &mut buffer, &mut workspace).unwrap();
    let mut restored = vec![0; count];
    let repeats = (262144 / count).max(1);
    let encode_ns = median_ns(repeats, || {
        black_box(
            encode_events_into::<24>(
                black_box(&events),
                black_box(&mut buffer),
                black_box(&mut workspace),
            )
            .unwrap(),
        );
    });
    let decode_ns = median_ns(repeats, || {
        let mut decoder = Decoder::<24>::new(black_box(&buffer[range.clone()])).unwrap();
        for (i, symbol) in restored.iter_mut().enumerate() {
            *symbol = decoder.read(black_box(&models[i % models_count])).unwrap();
        }
        decoder.finish().unwrap();
        black_box(&restored);
    });
    assert!(events
        .iter()
        .zip(&restored)
        .all(|(event, &symbol)| event.symbol == symbol));
    println!(
        "model_switch,{count},24,{models_count},{},{},{:.6},{:.4},{:.4},0",
        range.len(),
        models.iter().map(Model::table_bytes).sum::<usize>(),
        range.len() as f64 * 8.0 / count as f64,
        encode_ns / count as f64,
        decode_ns / count as f64
    );
}

fn main() {
    let args: Vec<_> = std::env::args()
        .skip(1)
        .filter(|s| s != "--bench")
        .collect();
    let count = args
        .first()
        .map(|s| s.parse::<usize>().expect("symbol count"))
        .unwrap_or(4096);
    assert!(
        (1..=1 << 26).contains(&count),
        "symbol count must be 1..67108864"
    );
    println!("distribution,symbols,delay,models,payload_bytes,model_bytes,bits_per_symbol,encode_ns_per_symbol,decode_ns_per_symbol,model_build_ns");
    for name in ["uniform256", "uniform16", "skewed", "near_constant"] {
        let mut weights = vec![0; 256];
        match name {
            "uniform256" => weights.fill(256),
            "uniform16" => weights[..16].fill(4096),
            "skewed" => {
                weights.fill(128);
                weights[0] += 32768;
            }
            _ => {
                weights.fill(1);
                weights[0] = 65536 - 255;
            }
        }
        let setup_ns = median_ns(100, || {
            black_box(Model::new(black_box(&weights)).unwrap());
        });
        let model = Model::new(&weights).unwrap();
        let mut state = 123456u64;
        let input: Vec<_> = (0..count)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                model.lookup(state as u16).symbol
            })
            .collect();
        measure::<16>(name, &model, &input, setup_ns);
        measure::<24>(name, &model, &input, setup_ns);
        measure::<32>(name, &model, &input, setup_ns);
    }
    for models in [1, 16, 256] {
        switching(count, models);
    }
    // Optional real byte input. The frequency model is built from the complete
    // supplied file; this row is still a kernel benchmark, not a framed compressor.
    if let Some(path) = args.get(1) {
        let bytes = std::fs::read(path).expect("read input file");
        assert!(
            !bytes.is_empty() && bytes.len() <= 1 << 26,
            "file must contain 1..67108864 bytes"
        );
        let mut counts = [0; 256];
        for &byte in &bytes {
            counts[byte as usize] += 1;
        }
        let setup_ns = median_ns(100, || {
            black_box(Model::from_counts(black_box(&counts)).unwrap());
        });
        let model = Model::from_counts(&counts).unwrap();
        let input: Vec<_> = bytes.into_iter().map(u32::from).collect();
        measure::<24>("file", &model, &input, setup_ns);
    }
}
