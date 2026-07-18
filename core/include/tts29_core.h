#ifndef TTS29_CORE_H
#define TTS29_CORE_H

#include <stddef.h>

typedef void (*tts29_snapshot_callback)(const char *snapshot_json, void *context);

void *tts29_start(
    const char *configuration_json,
    tts29_snapshot_callback callback,
    void *context
);

void tts29_stop(void *handle);

#endif
