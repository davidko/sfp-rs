#include <sfp/serial_framing_protocol.h>

extern "C" {
    SFPcontext* sfpNew();
}

SFPcontext* sfpNew() {
    SFPcontext *ctx;
    ctx = (SFPcontext*)malloc(sizeof(SFPcontext));
    sfpInit(ctx);
    return ctx;
}
