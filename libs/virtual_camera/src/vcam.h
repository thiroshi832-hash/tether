// Tether virtual camera — DirectShow push source filter.
//
// A minimal capture-source filter that advertises a single fixed format
// (1280x720 RGB24, 30fps) and, on each FillBuffer, serves the latest frame the
// Rust side published into shared memory (or the previous/black frame when
// nothing new is available). Registered under CLSID_VideoInputDeviceCategory so
// conferencing apps enumerate it as "Tether Camera".
//
// Requires the DirectShow base classes (streams.h / strmbase). See README.md.
#pragma once

#include <streams.h>
#include <vector>
#include "shared.h"
#include "frame_reader.h"

class CVCamStream;

class CVCam : public CSource {
public:
    static CUnknown* WINAPI CreateInstance(LPUNKNOWN lpunk, HRESULT* phr);

private:
    CVCam(LPUNKNOWN lpunk, HRESULT* phr);
};

class CVCamStream : public CSourceStream,
                    public IAMStreamConfig,
                    public IKsPropertySet {
public:
    CVCamStream(HRESULT* phr, CVCam* pParent, LPCWSTR pPinName);
    ~CVCamStream();

    // IUnknown — delegate lifetime to the owning filter.
    STDMETHODIMP QueryInterface(REFIID riid, void** ppv) override;
    STDMETHODIMP_(ULONG) AddRef() override { return GetOwner()->AddRef(); }
    STDMETHODIMP_(ULONG) Release() override { return GetOwner()->Release(); }

    // CSourceStream
    HRESULT FillBuffer(IMediaSample* pms) override;
    HRESULT DecideBufferSize(IMemAllocator* pAlloc,
                             ALLOCATOR_PROPERTIES* pProperties) override;
    HRESULT CheckMediaType(const CMediaType* pmt) override;
    HRESULT GetMediaType(int iPosition, CMediaType* pmt) override;
    HRESULT SetMediaType(const CMediaType* pmt) override;
    HRESULT OnThreadCreate() override;

    // IQualityControl
    STDMETHODIMP Notify(IBaseFilter* pSender, Quality q) override { return E_NOTIMPL; }

    // IAMStreamConfig
    HRESULT STDMETHODCALLTYPE SetFormat(AM_MEDIA_TYPE* pmt) override;
    HRESULT STDMETHODCALLTYPE GetFormat(AM_MEDIA_TYPE** ppmt) override;
    HRESULT STDMETHODCALLTYPE GetNumberOfCapabilities(int* piCount, int* piSize) override;
    HRESULT STDMETHODCALLTYPE GetStreamCaps(int iIndex, AM_MEDIA_TYPE** pmt,
                                            BYTE* pSCC) override;

    // IKsPropertySet
    HRESULT STDMETHODCALLTYPE Set(REFGUID guidPropSet, DWORD dwID, void* pInstanceData,
                                  DWORD cbInstanceData, void* pPropData,
                                  DWORD cbPropData) override;
    HRESULT STDMETHODCALLTYPE Get(REFGUID guidPropSet, DWORD dwPropID, void* pInstanceData,
                                  DWORD cbInstanceData, void* pPropData, DWORD cbPropData,
                                  DWORD* pcbReturned) override;
    HRESULT STDMETHODCALLTYPE QuerySupported(REFGUID guidPropSet, DWORD dwPropID,
                                             DWORD* pTypeSupport) override;

private:
    void FillVideoInfo(VIDEOINFOHEADER* pvi) const;

    CVCam* m_pParent = nullptr;
    CCritSec m_cSharedState;         // guards timing/format state
    REFERENCE_TIME m_rtLast = 0;     // end time of the previous sample
    REFERENCE_TIME m_rtFrameLength;  // 100ns units, = 1/30s
    tether::FrameReader m_reader;
    uint32_t m_lastSeq = 0;
    std::vector<BYTE> m_frame;       // last known frame (persists across repeats)
};
