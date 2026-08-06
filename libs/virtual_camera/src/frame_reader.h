// Tether virtual camera — reads the latest frame published by the Rust side.
#pragma once

#include "shared.h"
#include <windows.h>

namespace tether {

// Opens (does not create) the shared section written by src/virtual_camera.rs
// and copies out the most recent frame. Safe to construct even before the Rust
// side exists — Valid() reports whether the section is currently available and
// each Copy() retries the open, so a consumer app can be started first.
class FrameReader {
public:
    FrameReader() = default;
    ~FrameReader() { Close(); }

    // Copies the current frame into dst (must be >= kDataLen). Returns false if
    // the producer isn't running yet or no frame has been published. last_seq
    // is updated so callers can tell fresh frames from repeats.
    bool Copy(BYTE* dst, uint32_t* last_seq) {
        if (!map_ && !Open()) {
            return false;
        }
        auto* hdr = reinterpret_cast<volatile SharedHeader*>(view_);
        if (hdr->magic != kMagic || hdr->data_len != kDataLen) {
            return false;
        }
        if (hdr->sequence == 0) {
            return false; // no frame yet
        }
        if (mutex_) {
            WaitForSingleObject(mutex_, 50);
        }
        uint32_t seq = hdr->sequence;
        memcpy(dst, const_cast<BYTE*>(view_ + sizeof(SharedHeader)), kDataLen);
        if (mutex_) {
            ReleaseMutex(mutex_);
        }
        bool fresh = (last_seq == nullptr) || (seq != *last_seq);
        if (last_seq) {
            *last_seq = seq;
        }
        return fresh;
    }

    bool Valid() const { return map_ != nullptr; }

private:
    bool Open() {
        map_ = OpenFileMappingW(FILE_MAP_READ, FALSE, kMapName);
        if (!map_) {
            return false;
        }
        view_ = reinterpret_cast<BYTE*>(
            MapViewOfFile(map_, FILE_MAP_READ, 0, 0, sizeof(SharedHeader) + kDataLen));
        if (!view_) {
            CloseHandle(map_);
            map_ = nullptr;
            return false;
        }
        mutex_ = OpenMutexW(SYNCHRONIZE, FALSE, kMutexName);
        return true;
    }

    void Close() {
        if (view_) {
            UnmapViewOfFile(view_);
            view_ = nullptr;
        }
        if (map_) {
            CloseHandle(map_);
            map_ = nullptr;
        }
        if (mutex_) {
            CloseHandle(mutex_);
            mutex_ = nullptr;
        }
    }

    HANDLE map_ = nullptr;
    HANDLE mutex_ = nullptr;
    BYTE* view_ = nullptr;
};

} // namespace tether
