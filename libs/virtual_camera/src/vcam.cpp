// Tether virtual camera — DirectShow push source filter implementation.
#include "vcam.h"

#include <string>

using namespace tether;

constexpr int kFps = 30;
constexpr REFERENCE_TIME kFrameLen = 10000000LL / kFps; // 100ns units

//======================================================================
// CVCam (filter)
//======================================================================

CVCam::CVCam(LPUNKNOWN lpunk, HRESULT* phr)
    : CSource(L"Tether Camera", lpunk, CLSID_TetherCamera) {
    // The pin registers itself with the filter via the CSourceStream ctor.
    new CVCamStream(phr, this, L"Output");
}

CUnknown* WINAPI CVCam::CreateInstance(LPUNKNOWN lpunk, HRESULT* phr) {
    return new CVCam(lpunk, phr);
}

//======================================================================
// CVCamStream (output pin)
//======================================================================

CVCamStream::CVCamStream(HRESULT* phr, CVCam* pParent, LPCWSTR pPinName)
    : CSourceStream(L"Tether Camera", phr, pParent, pPinName),
      m_pParent(pParent),
      m_rtFrameLength(kFrameLen) {
    // Start out with a black frame so consumers get valid video immediately.
    m_frame.assign(kDataLen, 0);
    GetMediaType(0, &m_mt);
}

CVCamStream::~CVCamStream() {}

STDMETHODIMP CVCamStream::QueryInterface(REFIID riid, void** ppv) {
    if (riid == _uuidof(IAMStreamConfig)) {
        *ppv = static_cast<IAMStreamConfig*>(this);
    } else if (riid == _uuidof(IKsPropertySet)) {
        *ppv = static_cast<IKsPropertySet*>(this);
    } else {
        return CSourceStream::QueryInterface(riid, ppv);
    }
    AddRef();
    return S_OK;
}

void CVCamStream::FillVideoInfo(VIDEOINFOHEADER* pvi) const {
    ZeroMemory(pvi, sizeof(VIDEOINFOHEADER));
    pvi->AvgTimePerFrame = m_rtFrameLength;
    pvi->bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    pvi->bmiHeader.biWidth = (LONG)kFrameW;
    pvi->bmiHeader.biHeight = (LONG)kFrameH; // positive => bottom-up (matches feed)
    pvi->bmiHeader.biPlanes = 1;
    pvi->bmiHeader.biBitCount = 24;
    pvi->bmiHeader.biCompression = BI_RGB;
    pvi->bmiHeader.biSizeImage = kDataLen;
}

HRESULT CVCamStream::GetMediaType(int iPosition, CMediaType* pmt) {
    if (iPosition < 0) return E_INVALIDARG;
    if (iPosition > 0) return VFW_S_NO_MORE_ITEMS;

    VIDEOINFOHEADER* pvi = (VIDEOINFOHEADER*)pmt->AllocFormatBuffer(sizeof(VIDEOINFOHEADER));
    if (!pvi) return E_OUTOFMEMORY;
    FillVideoInfo(pvi);

    pmt->SetType(&MEDIATYPE_Video);
    pmt->SetFormatType(&FORMAT_VideoInfo);
    pmt->SetTemporalCompression(FALSE);
    pmt->SetSubtype(&MEDIASUBTYPE_RGB24);
    pmt->SetSampleSize(kDataLen);
    return S_OK;
}

HRESULT CVCamStream::CheckMediaType(const CMediaType* pmt) {
    if (*pmt->Type() != MEDIATYPE_Video) return E_INVALIDARG;
    if (*pmt->Subtype() != MEDIASUBTYPE_RGB24) return E_INVALIDARG;
    if (*pmt->FormatType() != FORMAT_VideoInfo) return E_INVALIDARG;
    VIDEOINFOHEADER* pvi = (VIDEOINFOHEADER*)pmt->Format();
    if (!pvi) return E_INVALIDARG;
    if ((uint32_t)abs(pvi->bmiHeader.biWidth) != kFrameW ||
        (uint32_t)abs(pvi->bmiHeader.biHeight) != kFrameH) {
        return E_INVALIDARG;
    }
    return S_OK;
}

HRESULT CVCamStream::SetMediaType(const CMediaType* pmt) {
    return CSourceStream::SetMediaType(pmt);
}

HRESULT CVCamStream::DecideBufferSize(IMemAllocator* pAlloc,
                                      ALLOCATOR_PROPERTIES* pProperties) {
    CAutoLock cAutoLock(m_pFilter->pStateLock());
    pProperties->cBuffers = 1;
    pProperties->cbBuffer = kDataLen;

    ALLOCATOR_PROPERTIES actual;
    HRESULT hr = pAlloc->SetProperties(pProperties, &actual);
    if (FAILED(hr)) return hr;
    if (actual.cbBuffer < pProperties->cbBuffer) return E_FAIL;
    return S_OK;
}

HRESULT CVCamStream::OnThreadCreate() {
    m_rtLast = 0;
    return NOERROR;
}

HRESULT CVCamStream::FillBuffer(IMediaSample* pms) {
    BYTE* pData = nullptr;
    HRESULT hr = pms->GetPointer(&pData);
    if (FAILED(hr)) return hr;
    long len = pms->GetSize();
    if ((uint32_t)len < kDataLen) return E_FAIL;

    {
        CAutoLock lock(&m_cSharedState);
        // Pull the newest frame if the producer published one; otherwise the
        // persistent m_frame (last frame, or black) is reused.
        m_reader.Copy(m_frame.data(), &m_lastSeq);
        memcpy(pData, m_frame.data(), kDataLen);
    }
    pms->SetActualDataLength(kDataLen);

    // Timestamp and pace to the advertised frame rate.
    REFERENCE_TIME rtStart = m_rtLast;
    REFERENCE_TIME rtStop = rtStart + m_rtFrameLength;
    m_rtLast = rtStop;
    pms->SetTime(&rtStart, &rtStop);
    pms->SetSyncPoint(TRUE);

    // Simple throttle so we don't spin the CPU at max speed.
    Sleep((DWORD)(m_rtFrameLength / 10000));
    return S_OK;
}

//======================================================================
// IAMStreamConfig — single fixed capability
//======================================================================

HRESULT STDMETHODCALLTYPE CVCamStream::SetFormat(AM_MEDIA_TYPE* pmt) {
    // We only support one format; accept it if it matches, ignore otherwise.
    if (pmt) {
        CMediaType mt(*pmt);
        if (CheckMediaType(&mt) != S_OK) return E_FAIL;
    }
    return S_OK;
}

HRESULT STDMETHODCALLTYPE CVCamStream::GetFormat(AM_MEDIA_TYPE** ppmt) {
    *ppmt = CreateMediaType(&m_mt);
    return *ppmt ? S_OK : E_OUTOFMEMORY;
}

HRESULT STDMETHODCALLTYPE CVCamStream::GetNumberOfCapabilities(int* piCount, int* piSize) {
    *piCount = 1;
    *piSize = sizeof(VIDEO_STREAM_CONFIG_CAPS);
    return S_OK;
}

HRESULT STDMETHODCALLTYPE CVCamStream::GetStreamCaps(int iIndex, AM_MEDIA_TYPE** pmt,
                                                     BYTE* pSCC) {
    if (iIndex != 0) return S_FALSE;
    *pmt = CreateMediaType(&m_mt);
    if (!*pmt) return E_OUTOFMEMORY;

    auto* pvi = (VIDEOINFOHEADER*)(*pmt)->pbFormat;
    auto* caps = (VIDEO_STREAM_CONFIG_CAPS*)pSCC;
    ZeroMemory(caps, sizeof(*caps));
    caps->guid = FORMAT_VideoInfo;
    caps->VideoStandard = 0;
    caps->InputSize.cx = kFrameW;
    caps->InputSize.cy = kFrameH;
    caps->MinCroppingSize = caps->MaxCroppingSize = caps->InputSize;
    caps->MinOutputSize = caps->MaxOutputSize = caps->InputSize;
    caps->MinFrameInterval = caps->MaxFrameInterval = kFrameLen;
    caps->MinBitsPerSecond = caps->MaxBitsPerSecond =
        (LONG)(kDataLen * 8 * kFps);
    (void)pvi;
    return S_OK;
}

//======================================================================
// IKsPropertySet — report the pin category (capture) so apps find us
//======================================================================

HRESULT STDMETHODCALLTYPE CVCamStream::Set(REFGUID, DWORD, void*, DWORD, void*, DWORD) {
    return E_NOTIMPL;
}

HRESULT STDMETHODCALLTYPE CVCamStream::Get(REFGUID guidPropSet, DWORD dwPropID, void*,
                                           DWORD, void* pPropData, DWORD cbPropData,
                                           DWORD* pcbReturned) {
    if (guidPropSet != AMPROPSETID_Pin) return E_PROP_SET_UNSUPPORTED;
    if (dwPropID != AMPROPERTY_PIN_CATEGORY) return E_PROP_ID_UNSUPPORTED;
    if (pPropData == nullptr && pcbReturned == nullptr) return E_POINTER;
    if (pcbReturned) *pcbReturned = sizeof(GUID);
    if (pPropData == nullptr) return S_OK;
    if (cbPropData < sizeof(GUID)) return E_UNEXPECTED;
    *(GUID*)pPropData = PIN_CATEGORY_CAPTURE;
    return S_OK;
}

HRESULT STDMETHODCALLTYPE CVCamStream::QuerySupported(REFGUID guidPropSet, DWORD dwPropID,
                                                      DWORD* pTypeSupport) {
    if (guidPropSet != AMPROPSETID_Pin) return E_PROP_SET_UNSUPPORTED;
    if (dwPropID != AMPROPERTY_PIN_CATEGORY) return E_PROP_ID_UNSUPPORTED;
    if (pTypeSupport) *pTypeSupport = KSPROPERTY_SUPPORT_GET;
    return S_OK;
}

//======================================================================
// COM registration
//======================================================================

const AMOVIESETUP_MEDIATYPE s_MediaType = { &MEDIATYPE_Video, &MEDIASUBTYPE_RGB24 };
const AMOVIESETUP_PIN s_Pin = {
    const_cast<LPWSTR>(L"Output"), FALSE, TRUE, FALSE, FALSE,
    &CLSID_NULL, nullptr, 1, &s_MediaType,
};
const AMOVIESETUP_FILTER s_Filter = {
    &CLSID_TetherCamera, kFilterName, MERIT_DO_NOT_USE, 1, &s_Pin,
};

CFactoryTemplate g_Templates[] = {
    { kFilterName, &CLSID_TetherCamera, CVCam::CreateInstance, nullptr, &s_Filter },
};
int g_cTemplates = sizeof(g_Templates) / sizeof(g_Templates[0]);

// The DirectShow base classes (dllentry.cpp) provide DllMain, DllGetClassObject,
// DllCanUnloadNow and the module handle g_hInst. We provide only the (custom)
// self-registration, so dllsetup.cpp is excluded from the build to avoid a
// duplicate DllRegisterServer.
extern HINSTANCE g_hInst;

static const wchar_t* kClsidStr = L"{6B1E2C9A-7F43-4E8B-9C2D-1A5E4F0D8B37}";

static LONG regSetSz(HKEY root, const std::wstring& subkey, const wchar_t* name,
                     const std::wstring& data) {
    HKEY h;
    LONG r = RegCreateKeyExW(root, subkey.c_str(), 0, nullptr, 0, KEY_WRITE, nullptr, &h,
                             nullptr);
    if (r != ERROR_SUCCESS) return r;
    r = RegSetValueExW(h, name, 0, REG_SZ, (const BYTE*)data.c_str(),
                       (DWORD)((data.size() + 1) * sizeof(wchar_t)));
    RegCloseKey(h);
    return r;
}

STDAPI DllRegisterServer() {
    // 1) COM CLSID -> InprocServer32 (this DLL, apartment-agnostic).
    wchar_t module[MAX_PATH] = {0};
    if (GetModuleFileNameW(g_hInst, module, MAX_PATH) == 0) {
        return HRESULT_FROM_WIN32(GetLastError());
    }
    const std::wstring clsidKey = std::wstring(L"CLSID\\") + kClsidStr;
    regSetSz(HKEY_CLASSES_ROOT, clsidKey, nullptr, kFilterName);
    const std::wstring inproc = clsidKey + L"\\InprocServer32";
    regSetSz(HKEY_CLASSES_ROOT, inproc, nullptr, module);
    regSetSz(HKEY_CLASSES_ROOT, inproc, L"ThreadingModel", L"Both");

    // 2) Register under the video capture category so apps enumerate it.
    const bool inited = SUCCEEDED(CoInitialize(nullptr));
    IFilterMapper2* pFM2 = nullptr;
    if (SUCCEEDED(CoCreateInstance(CLSID_FilterMapper2, nullptr, CLSCTX_INPROC_SERVER,
                                   IID_IFilterMapper2, (void**)&pFM2))) {
        REGFILTER2 rf2;
        rf2.dwVersion = 1;
        rf2.dwMerit = MERIT_DO_NOT_USE;
        rf2.cPins = 1;
        rf2.rgPins = &s_Pin;
        pFM2->RegisterFilter(CLSID_TetherCamera, kFilterName, nullptr,
                             &CLSID_VideoInputDeviceCategory, kFilterName, &rf2);
        pFM2->Release();
    }
    if (inited) CoUninitialize();
    return S_OK;
}

STDAPI DllUnregisterServer() {
    const bool inited = SUCCEEDED(CoInitialize(nullptr));
    IFilterMapper2* pFM2 = nullptr;
    if (SUCCEEDED(CoCreateInstance(CLSID_FilterMapper2, nullptr, CLSCTX_INPROC_SERVER,
                                   IID_IFilterMapper2, (void**)&pFM2))) {
        pFM2->UnregisterFilter(&CLSID_VideoInputDeviceCategory, kFilterName,
                               CLSID_TetherCamera);
        pFM2->Release();
    }
    if (inited) CoUninitialize();
    const std::wstring clsidKey = std::wstring(L"CLSID\\") + kClsidStr;
    RegDeleteTreeW(HKEY_CLASSES_ROOT, clsidKey.c_str());
    return S_OK;
}
