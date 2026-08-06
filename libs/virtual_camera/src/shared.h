// Tether virtual camera — shared-memory contract.
//
// This MUST stay byte-for-byte in sync with the Rust writer in
// src/virtual_camera.rs. All header fields are u32 (no padding). Frame data is
// a fixed-size BGR24, bottom-up DIB, i.e. exactly a MEDIASUBTYPE_RGB24 sample,
// so FillBuffer is a single memcpy.
#pragma once

#include <windows.h>
#include <cstdint>

namespace tether {

// {6B1E2C9A-7F43-4E8B-9C2D-1A5E4F0D8B37}
// clang-format off
static const GUID CLSID_TetherCamera =
    { 0x6b1e2c9a, 0x7f43, 0x4e8b, { 0x9c, 0x2d, 0x1a, 0x5e, 0x4f, 0x0d, 0x8b, 0x37 } };
// clang-format on

constexpr uint32_t kFrameW = 1280;
constexpr uint32_t kFrameH = 720;
constexpr uint32_t kBpp = 3; // BGR24
constexpr uint32_t kDataLen = kFrameW * kFrameH * kBpp;
constexpr uint32_t kMagic = 0x31435654; // "TVC1"
constexpr uint32_t kVersion = 1;

// 8 x u32, tightly packed, matches the Rust header.
#pragma pack(push, 1)
struct SharedHeader {
    uint32_t magic;
    uint32_t version;
    uint32_t width;
    uint32_t height;
    uint32_t format; // 0 = BGR24 bottom-up
    uint32_t sequence;
    uint32_t data_len;
    uint32_t reserved;
};
#pragma pack(pop)

static_assert(sizeof(SharedHeader) == 32, "header must be 32 bytes");

constexpr wchar_t kMapName[] = L"Local\\TetherVirtualCameraFrame";
constexpr wchar_t kMutexName[] = L"Local\\TetherVirtualCameraMutex";

// Friendly device name shown to consumer apps.
constexpr wchar_t kFilterName[] = L"Tether Camera";

} // namespace tether
