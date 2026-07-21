#ifndef TTS29_CORE_H
#define TTS29_CORE_H

#include <stddef.h>
#include <stdbool.h>
#include <stdint.h>

typedef void (*tts29_snapshot_callback)(const char *snapshot_json, void *context);

void *tts29_start(
    const char *configuration_json,
    tts29_snapshot_callback callback,
    void *context
);

void tts29_login(void *handle, const char *nsec);
void tts29_restore_login(void *handle, const char *nsec);
void tts29_credential_load_failed(void *handle, const char *error);
void tts29_dispatch(void *handle, const char *action_json);
void tts29_credential_result(
    void *handle,
    uint64_t request_id,
    bool succeeded,
    const char *error
);
void tts29_stop(void *handle);

#endif
