# Tether virtual camera — DirectShow source filter (tether_vcam.dll).
#
# Requires the DirectShow base classes (a.k.a. "baseclasses" / strmbase),
# which ship as source in the Windows SDK samples and must be compiled into a
# static lib. Point BASECLASSES_DIR at that source (see README.md), or set
# STRMBASE_LIB/STRMBASE_INCLUDE to a prebuilt copy (e.g. from vcpkg).
cmake_minimum_required(VERSION 3.20)
project(tether_vcam CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(NOT WIN32)
  message(FATAL_ERROR "tether_vcam is Windows-only")
endif()

# --- DirectShow base classes ------------------------------------------------
# Option A: build from SDK sample sources at BASECLASSES_DIR.
# Option B: supply a prebuilt strmbase via STRMBASE_LIB + STRMBASE_INCLUDE.
if(DEFINED STRMBASE_LIB AND DEFINED STRMBASE_INCLUDE)
  set(_strmbase_lib "${STRMBASE_LIB}")
  set(_strmbase_inc "${STRMBASE_INCLUDE}")
elseif(DEFINED BASECLASSES_DIR)
  file(GLOB _bc_src "${BASECLASSES_DIR}/*.cpp")
  # We provide our own DllRegisterServer/DllUnregisterServer, so drop the base
  # classes' default ones (dllsetup.cpp) to avoid duplicate symbols.
  list(FILTER _bc_src EXCLUDE REGEX "dllsetup\\.cpp$")
  add_library(strmbase STATIC ${_bc_src})
  target_include_directories(strmbase PUBLIC "${BASECLASSES_DIR}")
  # NOTE: do NOT define NOMINMAX — the base classes use the windows.h min/max macros.
  # UNICODE/_UNICODE required so TCHAR == wchar_t (DirectShow is a UNICODE build).
  target_compile_definitions(strmbase PUBLIC
    _WINDLL _CRT_SECURE_NO_WARNINGS _CRT_NONSTDC_NO_DEPRECATE
    UNICODE _UNICODE WIN32 _WINDOWS _WIN32_WINNT=0x0601)
  target_compile_options(strmbase PRIVATE /wd4996 /wd4238 /wd4127)
  set(_strmbase_lib strmbase)
  set(_strmbase_inc "${BASECLASSES_DIR}")
else()
  message(FATAL_ERROR
    "Set BASECLASSES_DIR to the DirectShow baseclasses source, or "
    "STRMBASE_LIB + STRMBASE_INCLUDE to a prebuilt copy. See README.md.")
endif()

add_library(tether_vcam SHARED
  src/vcam.cpp
  src/vcam.def
)
target_include_directories(tether_vcam PRIVATE "${_strmbase_inc}" src)
target_compile_definitions(tether_vcam PRIVATE
  _CRT_SECURE_NO_WARNINGS UNICODE _UNICODE WIN32 _WINDOWS _WIN32_WINNT=0x0601)
target_link_libraries(tether_vcam PRIVATE
  ${_strmbase_lib}
  strmiids winmm ole32 oleaut32 uuid advapi32 user32 gdi32
)
set_target_properties(tether_vcam PROPERTIES OUTPUT_NAME "tether_vcam")
