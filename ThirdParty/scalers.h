// C interface to the vendored pixel-art scalers, imported into Swift via -import-objc-header.
// Pixels are 0xAARRGGBB uint32 values (BGRA byte order on little-endian), straight alpha.
#pragma once
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// xBRZ (Zenju, GPL-3). factor 2...6. dst must hold (w*factor)*(h*factor) pixels.
void wob_xbrz_scale(int factor, const uint32_t *src, uint32_t *dst, int width, int height);

/// MMPX (McGuire & Gagiu, MIT). Exactly 2x. dst must hold (w*2)*(h*2) pixels.
void mmpx_scale2x(const uint32_t *src, uint32_t *dst, uint32_t width, uint32_t height);

#ifdef __cplusplus
}
#endif
