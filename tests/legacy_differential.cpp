// SPDX-License-Identifier: MIT
// Compile against the unmodified Blitzcrank utility.cpp, not a copied/reimplemented oracle.
#include "delayed_coding.h"
#include "utility.h"
#include <algorithm>
#include <iostream>
#include <random>
#include <stdexcept>

static void require(bool value) { if (!value) throw std::runtime_error("legacy differential mismatch"); }

int main() {
    static_assert(kDelayedCoding == 24, "use the original 24-bit research configuration");
    std::mt19937 random(123456);
    for (unsigned trial = 0; trial < 160; ++trial) {
        const unsigned alphabet = trial == 0 ? 65536 : 1 + random() % 512;
        std::vector<unsigned> weights(alphabet, 0);
        for (unsigned word = 0; word < 65536; ++word) ++weights[random() % alphabet];
        if (trial == 1) { weights = {65536}; }
        if (trial == 2) { weights = {65535, 1}; }
        if (trial == 3) { weights = {0, 1, 0, 65535}; }
        db_compress::DelayedCodingParams legacy;
        db_compress::InitDelayedCodingParams(weights, legacy);
        DcModel* model = nullptr;
        DcWorkspace* workspace = nullptr;
        require(dc_model_new(weights.data(), weights.size(), &model) == DC_OK);
        require(dc_workspace_new(&workspace) == DC_OK);
        for (int length : {0, 1, 2, 3, 16, 255, 4096}) {
            std::vector<uint32_t> input(length);
            std::vector<db_compress::Branch*> branches(length);
            for (int i = 0; i < length; ++i) {
                unsigned symbol;
                do { symbol = random() % weights.size(); } while (!weights[symbol]);
                input[i] = symbol;
                branches[i] = &legacy.branches_[symbol];
            }
            db_compress::BitString bits(length + 1);
            std::vector<bool> virtual_symbols(length);
            db_compress::DelayedCoding(branches, length, &bits, virtual_symbols);
            std::vector<uint8_t> payload(length * 2 + 1);
            size_t offset = 0, size = 0;
            require(dc_encode(model, 24, input.data(), input.size(), payload.data(), payload.size(),
                              workspace, &offset, &size) == DC_OK);
            require(size == bits.num_ * 2);
            for (size_t i = 0; i < bits.num_; ++i) {
                const unsigned word = bits.bits_[bits.size_ - bits.num_ + i];
                require(payload[offset + i * 2] == word >> 8);
                require(payload[offset + i * 2 + 1] == (word & 255));
            }
            std::vector<uint32_t> restored(length);
            require(dc_decode(model, 24, payload.data() + offset, size, restored.data(), restored.size()) == DC_OK);
            require(restored == input);
        }
        dc_workspace_free(workspace);
        dc_model_free(model);
    }
    std::cout << "1120 blocks match original Blitzcrank byte-for-byte and decode correctly\n";
}
