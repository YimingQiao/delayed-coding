use delayed_coding::{decode_into, encode, Model};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = Model::from_counts(&[10, 5, 1])?;
    let input = [0, 0, 1, 0, 2, 0, 1, 0];
    let payload = encode::<24>(&model, &input)?;
    let mut restored = [0; 8];
    decode_into::<24>(&model, &payload, &mut restored)?;
    assert_eq!(restored, input);
    println!(
        "{} symbols -> {} payload bytes; roundtrip OK",
        input.len(),
        payload.len()
    );
    Ok(())
}
