use delayed_coding::{encode_events_into, Decoder, Event, Model, Workspace};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let models = [Model::from_counts(&[9, 1])?, Model::from_counts(&[2, 8])?];
    let input = [0, 0, 1, 1, 1, 0, 0, 1];
    let mut previous = 0;
    let events: Vec<_> = input
        .iter()
        .map(|&symbol| {
            let event = Event {
                model: &models[previous],
                symbol,
            };
            previous = symbol as usize;
            event
        })
        .collect();
    let mut storage = vec![0; input.len() * 2];
    let range = encode_events_into::<24>(&events, &mut storage, &mut Workspace::default())?;
    let mut decoder = Decoder::<24>::new(&storage[range])?;
    previous = 0;
    for expected in input {
        let symbol = decoder.read(&models[previous])?;
        assert_eq!(symbol, expected);
        previous = symbol as usize;
    }
    decoder.finish()?;
    println!("Conditional-model roundtrip OK");
    Ok(())
}
