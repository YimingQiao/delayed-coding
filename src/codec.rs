use crate::{model::Symbol, Error, Model};
use std::ops::Range;

/// Models are borrowed for the duration of encoding; lifetimes prevent stale references.
#[derive(Clone, Copy)]
pub struct Event<'a> {
    pub model: &'a Model,
    pub symbol: u32,
}

/// Reuse between calls to avoid reallocating the per-symbol virtual schedule.
#[derive(Default)]
pub struct Workspace {
    virtual_symbols: Vec<u8>,
}
impl Workspace {
    pub fn with_capacity(symbols: usize) -> Self {
        Self {
            virtual_symbols: Vec::with_capacity(symbols),
        }
    }
    pub fn capacity(&self) -> usize {
        self.virtual_symbols.capacity()
    }
}

pub fn max_encoded_size(symbols: usize) -> Result<usize, Error> {
    symbols.checked_mul(2).ok_or(Error::InputTooLarge)
}

#[inline]
fn validate_delay<const DELAY: u32>() -> Result<(), Error> {
    if !(16..=32).contains(&DELAY) {
        Err(Error::InvalidDelay)
    } else {
        Ok(())
    }
}

// For valid input with DELAY <= 32, n < 2^48 and f <= 2^16. For f>=2,
// ceil(2^64/f) produces floor(n/f) exactly: its error is < n/2^64 < 1/f.
// Frequency 1 is handled separately. See docs/ALGORITHM.md for invariants.
#[inline]
pub(crate) fn quotient(n: u64, symbol: &Symbol) -> u64 {
    #[cfg(feature = "reference-division")]
    {
        n / u64::from(symbol.frequency)
    }
    #[cfg(not(feature = "reference-division"))]
    {
        if symbol.frequency == 1 {
            n
        } else {
            ((u128::from(n) * u128::from(symbol.reciprocal)) >> 64) as u64
        }
    }
}

fn encode_impl<'a, const DELAY: u32, const LANES: usize>(
    count: usize,
    event_at: impl Fn(usize) -> Event<'a>,
    output: &mut [u8],
    workspace: &mut Workspace,
) -> Result<Range<usize>, Error> {
    validate_delay::<DELAY>()?;
    if !matches!(LANES, 1 | 2 | 4 | 8) {
        return Err(Error::InvalidLanes);
    }
    max_encoded_size(count)?;
    workspace.virtual_symbols.resize(count, 0);
    let mut denominators = [1u64; LANES];
    let mut virtuals = [false; LANES];
    let mut words = 0;
    for i in 0..count {
        let lane = i & (LANES - 1);
        let denominator = &mut denominators[lane];
        let next_virtual = &mut virtuals[lane];
        let event = event_at(i);
        let symbol = event
            .model
            .symbols
            .get(event.symbol as usize)
            .ok_or(Error::InvalidSymbol)?;
        if symbol.frequency == 0 {
            return Err(Error::InvalidSymbol);
        }
        workspace.virtual_symbols[i] = u8::from(*next_virtual);
        words += usize::from(!*next_virtual);
        *denominator *= u64::from(symbol.frequency);
        *next_virtual = *denominator >= (1u64 << DELAY);
        if *next_virtual {
            *denominator >>= 16;
        }
    }
    if words > output.len() / 2 {
        return Err(Error::OutputTooSmall);
    }
    let mut position = output.len();
    let mut numerators = [0u64; LANES];
    for i in (0..count).rev() {
        let numerator = &mut numerators[i & (LANES - 1)];
        let event = event_at(i);
        let symbol = &event.model.symbols[event.symbol as usize];
        debug_assert!(*numerator < (1u64 << 48));
        let q = quotient(*numerator, symbol);
        let remainder = (*numerator - q * u64::from(symbol.frequency)) as u32;
        let word = event.model.embed_validated(symbol, remainder);
        *numerator = q;
        if workspace.virtual_symbols[i] != 0 {
            *numerator = (*numerator << 16) | u64::from(word);
        } else {
            position -= 2;
            output[position..position + 2].copy_from_slice(&word.to_be_bytes());
        }
    }
    Ok(position..output.len())
}

/// Writes backwards, returning the payload's range in the caller's buffer.
/// Input validity and capacity are checked before any payload byte is written.
pub fn encode_into<const DELAY: u32>(
    model: &Model,
    symbols: &[u32],
    output: &mut [u8],
    workspace: &mut Workspace,
) -> Result<Range<usize>, Error> {
    encode_impl::<DELAY, 1>(
        symbols.len(),
        |i| Event {
            model,
            symbol: symbols[i],
        },
        output,
        workspace,
    )
}

/// Encode a sequence whose probability model may change at every symbol.
pub fn encode_events_into<const DELAY: u32>(
    events: &[Event<'_>],
    output: &mut [u8],
    workspace: &mut Workspace,
) -> Result<Range<usize>, Error> {
    encode_impl::<DELAY, 1>(events.len(), |i| events[i], output, workspace)
}

/// Round-robin interleaving of 1, 2, 4 or 8 independent coding states into one
/// payload. The lane count is external format metadata. No per-lane length header
/// is needed: encoder and decoder use the same deterministic physical-word order.
pub fn encode_interleaved_into<const DELAY: u32, const LANES: usize>(
    model: &Model,
    symbols: &[u32],
    output: &mut [u8],
    workspace: &mut Workspace,
) -> Result<Range<usize>, Error> {
    encode_impl::<DELAY, LANES>(
        symbols.len(),
        |i| Event {
            model,
            symbol: symbols[i],
        },
        output,
        workspace,
    )
}

/// Interleaved encoding with explicit per-symbol models. The caller remains
/// responsible for selecting contexts in forward order before encoding.
pub fn encode_events_interleaved_into<const DELAY: u32, const LANES: usize>(
    events: &[Event<'_>],
    output: &mut [u8],
    workspace: &mut Workspace,
) -> Result<Range<usize>, Error> {
    encode_impl::<DELAY, LANES>(events.len(), |i| events[i], output, workspace)
}

/// Allocating convenience API. Use encode_into and a reusable workspace in hot paths.
pub fn encode<const DELAY: u32>(model: &Model, symbols: &[u32]) -> Result<Vec<u8>, Error> {
    let mut output = vec![0; max_encoded_size(symbols.len())?];
    let range = encode_into::<DELAY>(model, symbols, &mut output, &mut Workspace::default())?;
    output.copy_within(range.clone(), 0);
    output.truncate(range.len());
    Ok(output)
}

/// Forward decoder with explicit model selection. All input reads are bounded.
/// Errors are sticky; discard the decoder after a failed read. Call finish after
/// decoding the externally supplied symbol count. This is not a checksum.
pub struct Decoder<'a, const DELAY: u32 = 24, const LANES: usize = 1> {
    input: &'a [u8],
    position: usize,
    states: [CodingState; LANES],
    lane: usize,
    error: Option<Error>,
}

#[derive(Clone, Copy)]
struct CodingState {
    numerator: u64,
    denominator: u64,
    virtual_word: Option<u16>,
}

impl<'a, const DELAY: u32, const LANES: usize> Decoder<'a, DELAY, LANES> {
    pub fn new(input: &'a [u8]) -> Result<Self, Error> {
        validate_delay::<DELAY>()?;
        if !matches!(LANES, 1 | 2 | 4 | 8) {
            return Err(Error::InvalidLanes);
        }
        Ok(Self {
            input,
            position: 0,
            states: [CodingState {
                numerator: 0,
                denominator: 1,
                virtual_word: None,
            }; LANES],
            lane: 0,
            error: None,
        })
    }

    #[inline]
    pub fn read(&mut self, model: &Model) -> Result<u32, Error> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let state = &mut self.states[self.lane];
        let word = if let Some(word) = state.virtual_word.take() {
            word
        } else {
            let Some(bytes) = self
                .input
                .get(self.position..self.position.saturating_add(2))
            else {
                self.error = Some(Error::TruncatedInput);
                return Err(Error::TruncatedInput);
            };
            self.position += 2;
            u16::from_be_bytes([bytes[0], bytes[1]])
        };
        let decoded = model.lookup(word);
        // Malformed payloads must not panic on integer overflow in Debug builds.
        // The valid-stream invariant is stronger; wrapping arithmetic here does
        // not make malformed streams trusted or guarantee corruption detection.
        state.numerator = state
            .numerator
            .wrapping_mul(u64::from(decoded.frequency))
            .wrapping_add(u64::from(decoded.remainder));
        state.denominator *= u64::from(decoded.frequency);
        if state.denominator >= (1u64 << DELAY) {
            state.virtual_word = Some(state.numerator as u16);
            state.numerator >>= 16;
            state.denominator >>= 16;
        }
        self.lane = (self.lane + 1) & (LANES - 1);
        Ok(decoded.symbol)
    }

    pub fn bytes_read(&self) -> usize {
        self.position
    }
    pub fn finish(&self) -> Result<(), Error> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if self.position != self.input.len() {
            return Err(Error::TrailingInput);
        }
        if self
            .states
            .iter()
            .any(|s| s.numerator != 0 || s.virtual_word.is_some_and(|w| w != 0))
        {
            return Err(Error::InvalidState);
        }
        Ok(())
    }
}

/// Decode exactly output.len() symbols, then check consumption and final state.
/// On error, output may contain a decoded prefix.
pub fn decode_into<const DELAY: u32>(
    model: &Model,
    input: &[u8],
    output: &mut [u32],
) -> Result<(), Error> {
    decode_interleaved_into::<DELAY, 1>(model, input, output)
}

pub fn decode_interleaved_into<const DELAY: u32, const LANES: usize>(
    model: &Model,
    input: &[u8],
    output: &mut [u32],
) -> Result<(), Error> {
    let mut decoder = Decoder::<DELAY, LANES>::new(input)?;
    for symbol in output {
        *symbol = decoder.read(model)?;
    }
    decoder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reciprocals_match_division_at_state_boundaries() {
        for frequency in 1..=65536u32 {
            let model = Model::new(&[frequency, 65536 - frequency]).unwrap();
            let symbol = &model.symbols[0];
            let maximum = (1u64 << 48) - 1;
            let near_multiple = (maximum / u64::from(frequency) - 1) * u64::from(frequency);
            for n in [
                0,
                1,
                u64::from(frequency) - 1,
                u64::from(frequency),
                near_multiple - 1,
                near_multiple,
                near_multiple + 1,
                maximum,
            ] {
                assert_eq!(
                    quotient(n, symbol),
                    n / u64::from(frequency),
                    "f={frequency}, n={n}"
                );
            }
        }
    }
}
