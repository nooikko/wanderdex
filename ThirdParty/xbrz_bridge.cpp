#include "scalers.h"
#include "xbrz/xbrz.h"

extern "C" void wob_xbrz_scale(int factor, const uint32_t *src, uint32_t *dst, int width, int height) {
    xbrz::scale(factor, src, dst, width, height, xbrz::ColorFormat::ARGB);
}
