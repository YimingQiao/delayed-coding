// SPDX-License-Identifier: MIT
// Benchmark-only adapter. The alias implementation lives in the pinned,
// external public-domain ryg_rans demo; do not fork its coding loops here.
#pragma once

// Upstream supplies these routines in a demo .cpp rather than a header. Rename
// only its entry point. It is never called (and assumes a local "book1" file).
// Renaming main exposes its implicit return as a warning in GCC/Clang.
#if defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wreturn-type"
#if !defined(__clang__)
#pragma GCC diagnostic ignored "-Wliteral-suffix"
#endif
#endif
#define main dc_unused_upstream_alias_demo
#include "main_alias.cpp"
#undef main
#if defined(__GNUC__)
#pragma GCC diagnostic pop
#endif

template<unsigned Lanes>
struct RansAlias {
    SymbolStats stats;
    std::vector<uint8_t> storage;
    size_t offset = 0, size = 0;

    RansAlias(const Model& model, size_t n) : storage(n * 2 + 4 * Lanes) {
        std::copy(model.frequencies.begin(), model.frequencies.end(), stats.freqs);
        stats.calc_cum_freqs();
        stats.make_alias_table();
        // Independently validate every alias code point, outside timing.
        for (uint32_t symbol = 0; symbol < 256; ++symbol) {
            for (uint32_t remainder = 0; remainder < stats.freqs[symbol]; ++remainder) {
                RansState state = stats.alias_remap[stats.cum_freqs[symbol] + remainder];
                require(RansDecGetAlias(&state, &stats, 16) == symbol);
                require(state == remainder);
            }
        }
    }
    RansAlias(const RansAlias&) = delete;
    RansAlias& operator=(const RansAlias&) = delete;

    void encode(const std::vector<uint32_t>& input) {
        std::array<RansState, Lanes> states;
        for (auto& state : states) RansEncInit(&state);
        auto* end = storage.data() + storage.size();
        auto* cursor = end;
        for (size_t i = input.size(); i-- != 0;)
            RansEncPutAlias(&states[i % Lanes], &cursor, &stats, input[i], 16);
        for (size_t lane = Lanes; lane-- != 0;) RansEncFlush(&states[lane], &cursor);
        offset = cursor - storage.data();
        size = end - cursor;
    }

    void decode(std::vector<uint32_t>& output) {
        auto* cursor = storage.data() + offset;
        std::array<RansState, Lanes> states;
        for (auto& state : states) RansDecInit(&state, &cursor);
        for (size_t i = 0; i < output.size(); ++i) {
            auto& state = states[i % Lanes];
            output[i] = RansDecGetAlias(&state, &stats, 16);
            RansDecRenorm(&state, &cursor);
        }
        require(cursor == storage.data() + storage.size());
        for (auto state : states) require(state == RANS_BYTE_L);
    }
    size_t bytes() const { return size; }
};
