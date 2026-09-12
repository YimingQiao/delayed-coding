/* SPDX-License-Identifier: MIT */
#include "delayed_coding.h"
#include <stdio.h>
#include <string.h>

int main(void) {
    const uint32_t frequencies[] = {32768, 16384, 16384};
    const uint32_t input[] = {0, 0, 1, 2, 0, 1, 0, 2};
    uint32_t restored[8];
    uint8_t output[16];
    size_t offset = 0, size = 0;
    DcModel* model = NULL;
    DcWorkspace* workspace = NULL;
    if (dc_model_new(frequencies, 3, &model) != DC_OK) return 1;
    if (dc_workspace_new(&workspace) != DC_OK) { dc_model_free(model); return 1; }
    int failed = dc_encode(model, 24, input, 8, output, sizeof(output), workspace, &offset, &size) != DC_OK;
    if (!failed) failed = dc_decode(model, 24, output + offset, size, restored, 8) != DC_OK;
    if (!failed) failed = memcmp(input, restored, sizeof(input)) != 0;
    if (!failed && dc_decode(model, 24, output + offset, size - 1, restored, 8) == DC_OK) failed = 1;
    if (!failed && dc_decode(NULL, 24, output, size, restored, 8) != DC_INVALID_ARGUMENT) failed = 1;
    if (!failed && dc_encode(model, 15, input, 8, output, 16, workspace, &offset, &size) != DC_INVALID_DELAY) failed = 1;
    if (!failed) puts("C ABI roundtrip and error checks OK");
    dc_workspace_free(workspace);
    dc_model_free(model);
    return failed;
}
