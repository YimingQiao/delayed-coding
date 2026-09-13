// SPDX-License-Identifier: MIT
// Adapter around the unmodified, public-domain upstream SSE4.1 codec.
#pragma once
#include "platform.h"
#include "rans_word_sse41.h"

struct RansSimd {
    RansWordTables tables{};
    std::array<uint32_t, 256> frequencies{}, starts{};
    // Four words of padding are required by upstream's unconditional 8-byte load.
    std::vector<uint16_t> storage;
    size_t offset = 0, size = 0;

    RansSimd(const Model& model, size_t n) : storage(n + 8 + 4) {
        uint32_t start = 0;
        for (unsigned symbol = 0; symbol < 256; ++symbol) {
            require(model.frequencies[symbol] % 16 == 0);
            const uint32_t frequency = model.frequencies[symbol] / 16;
            // The upstream encoder's renormalization threshold overflows for
            // frequency 4096. Callers skip this variant for one-symbol models.
            require(frequency < RANS_WORD_M);
            frequencies[symbol] = frequency;
            starts[symbol] = start;
            RansWordTablesInitSymbol(&tables, symbol, start, frequency);
            start += frequency;
        }
        require(start == RANS_WORD_M);
    }

    void encode(const std::vector<uint32_t>& input) {
        std::array<RansWordEnc, 4> states;
        for (auto& state : states) state = RansWordEncInit();
        auto* end = storage.data() + storage.size() - 4;
        auto* cursor = end;
        for (size_t i = input.size(); i-- != 0;)
            RansWordEncPut(&states[i % 4], &cursor, starts[input[i]], frequencies[input[i]]);
        for (size_t lane = 4; lane-- != 0;) RansWordEncFlush(&states[lane], &cursor);
        offset = cursor - storage.data();
        size = (end - cursor) * 2;
    }

    void decode(std::vector<uint32_t>& output) {
        auto* cursor = storage.data() + offset;
        RansSimdDec state;
        RansSimdDecInit(&state, &cursor);
        size_t i = 0;
        for (; i + 4 <= output.size(); i += 4) {
            const uint32_t packed = RansSimdDecSym(&state, &tables);
            // Keep the same u32 output representation as every other codec.
            _mm_storeu_si128(reinterpret_cast<__m128i*>(output.data() + i),
                            _mm_cvtepu8_epi32(_mm_cvtsi32_si128(packed)));
            RansSimdDecRenorm(&state, &cursor);
        }
        for (; i < output.size(); ++i) {
            output[i] = RansWordDecSym(&state.lane[i % 4], &tables);
            RansWordDecRenorm(&state.lane[i % 4], &cursor);
        }
        require(cursor == storage.data() + storage.size() - 4);
        for (auto lane : state.lane) require(lane == RANS_WORD_L);
    }
    size_t bytes() const { return size; } // excludes the required 8 readable padding bytes
};
