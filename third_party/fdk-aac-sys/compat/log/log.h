// App-build compatibility: FDK's AOSP diagnostic uses a private liblog API.
// Keep the bounds-check diagnostic through the public NDK logging API.
#pragma once
#include <android/log.h>
static inline int android_errorWriteLog(int tag, const char *issue) {
    return __android_log_print(ANDROID_LOG_ERROR, "FDK-AAC", "bounds check %x: %s", tag, issue);
}
