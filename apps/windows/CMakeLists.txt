cmake_minimum_required(VERSION 3.26)

project(VexioWindows
    VERSION 0.1.0
    DESCRIPTION "Vexio Native Windows Workstation (WinUI 3 + DirectX 12)"
    LANGUAGES CXX
)

if(NOT WIN32)
    message(STATUS "Configuring VexioWindows target metadata on non-Windows host (cross-check mode).")
endif()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# ---------------------------------------------------------------------------
# Paths to Rust Core C-ABI Header & Import Library (vexio_core.dll.lib)
# ---------------------------------------------------------------------------
get_filename_component(VEXIO_REPO_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)
set(VEXIO_INCLUDE_DIR "${VEXIO_REPO_ROOT}/crates/vexio-core/include")

set(VEXIO_RUST_PROFILE "$<IF:$<CONFIG:Debug>,debug,release>")
set(VEXIO_RUST_TARGET_DIR "${VEXIO_REPO_ROOT}/target" CACHE PATH "Cargo workspace target directory")
set(VEXIO_CORE_IMPLIB "${VEXIO_RUST_TARGET_DIR}/${VEXIO_RUST_PROFILE}/vexio_core.dll.lib")
set(VEXIO_CORE_DLL    "${VEXIO_RUST_TARGET_DIR}/${VEXIO_RUST_PROFILE}/vexio_core.dll")

# ---------------------------------------------------------------------------
# Windows App SDK 1.6 & CppWinRT Discovery
# ---------------------------------------------------------------------------
set(WINDOWS_APP_SDK_VERSION "1.6.241114003" CACHE STRING "Windows App SDK 1.6 NuGet version")
set(CPPWINRT_VERSION "2.0.240405.15" CACHE STRING "Microsoft.Windows.CppWinRT NuGet version")
set(NUGET_PACKAGES_DIR "${CMAKE_BINARY_DIR}/packages" CACHE PATH "Restored NuGet packages directory")

set(WASDK_PACKAGE_ROOT "${NUGET_PACKAGES_DIR}/Microsoft.WindowsAppSDK.${WINDOWS_APP_SDK_VERSION}")
set(CPPWINRT_PACKAGE_ROOT "${NUGET_PACKAGES_DIR}/Microsoft.Windows.CppWinRT.${CPPWINRT_VERSION}")

if(DEFINED ENV{WINDOWS_APP_SDK_ROOT})
    set(WASDK_PACKAGE_ROOT "$ENV{WINDOWS_APP_SDK_ROOT}")
endif()

if(CMAKE_GENERATOR_PLATFORM MATCHES "ARM64|arm64" OR CMAKE_VS_PLATFORM_NAME MATCHES "ARM64|arm64" OR CMAKE_SYSTEM_PROCESSOR MATCHES "ARM64|arm64")
    set(WASDK_ARCH "arm64")
else()
    set(WASDK_ARCH "x64")
endif()

# Patch generated Visual Studio .vcxproj for C++20 WinUI 3 + C++/WinRT NuGet PackageReference
configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/Directory.Build.props.in"
    "${CMAKE_CURRENT_BINARY_DIR}/Directory.Build.props"
    @ONLY
)
configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/Directory.Build.targets.in"
    "${CMAKE_CURRENT_BINARY_DIR}/Directory.Build.targets"
    COPYONLY
)

# Imported target for Rust Core (vexio_core.dll.lib)
add_library(vexio_core SHARED IMPORTED GLOBAL)
set_target_properties(vexio_core PROPERTIES
    IMPORTED_IMPLIB "${VEXIO_CORE_IMPLIB}"
    IMPORTED_LOCATION "${VEXIO_CORE_DLL}"
    INTERFACE_INCLUDE_DIRECTORIES "${VEXIO_INCLUDE_DIR}"
)

# Optional automatic Cargo build step so CMake builds vexio-core seamlessly
option(VEXIO_AUTO_BUILD_RUST_CORE "Automatically build crates/vexio-core via Cargo before linking" ON)
if(VEXIO_AUTO_BUILD_RUST_CORE)
    find_program(CARGO_EXECUTABLE cargo)
    if(CARGO_EXECUTABLE)
        add_custom_target(vexio_core_cargo_build
            COMMAND "${CARGO_EXECUTABLE}" build -p vexio-core
                    $<$<NOT:$<CONFIG:Debug>>:--release>
            WORKING_DIRECTORY "${VEXIO_REPO_ROOT}"
            COMMENT "Building Rust vexio-core shared library (vexio_core.dll / vexio_core.dll.lib)"
            VERBATIM
        )
    endif()
endif()

# ---------------------------------------------------------------------------
# WinUI 3 Native Workstation Executable
# ---------------------------------------------------------------------------
set(VEXIO_WINDOWS_SOURCES
    src/pch.h
    src/VexioInterop.h
    src/App.idl
    src/App.xaml
    src/App.xaml.h
    src/App.xaml.cpp
    src/MainWindow.idl
    src/MainWindow.xaml
    src/MainWindow.xaml.h
    src/MainWindow.xaml.cpp
    app.manifest
)

add_executable(Vexio WIN32 ${VEXIO_WINDOWS_SOURCES})

if(TARGET vexio_core_cargo_build)
    add_dependencies(Vexio vexio_core_cargo_build)
endif()

target_include_directories(Vexio PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/src"
    "${CMAKE_CURRENT_BINARY_DIR}/Generated Files"
    "${VEXIO_INCLUDE_DIR}"
    "${WASDK_PACKAGE_ROOT}/include"
    "${CPPWINRT_PACKAGE_ROOT}/build/native/include"
)

target_compile_definitions(Vexio PRIVATE
    UNICODE
    _UNICODE
    WIN32_LEAN_AND_MEAN
    NOMINMAX
    WINRT_LEAN_AND_MEAN
    MICROSOFT_WINDOWSAPPSDK_VERSION_MAJOR=1
    MICROSOFT_WINDOWSAPPSDK_VERSION_MINOR=6
)

if(MSVC)
    target_compile_options(Vexio PRIVATE
        /W4
        /permissive-
        /utf-8
        /EHsc
        /Zc:__cplusplus
        /Zc:preprocessor
        /bigobj
        /await:strict
    )
endif()

target_link_directories(Vexio PRIVATE
    "${WASDK_PACKAGE_ROOT}/lib/win10-${WASDK_ARCH}"
    "${VEXIO_RUST_TARGET_DIR}/${VEXIO_RUST_PROFILE}"
)

# Link Rust Core + DirectX 12 + DXGI + Windows App SDK 1.6
target_link_libraries(Vexio PRIVATE
    vexio_core
    d3d12.lib
    dxgi.lib
    dxguid.lib
    WindowsApp.lib
    Microsoft.WindowsAppRuntime.lib
    Microsoft.WindowsAppRuntime.Bootstrap.lib
    shcore.lib
    dwmapi.lib
)

# Mark XAML & MIDL files with Visual Studio / MSBuild WinUI 3 item metadata
set_property(SOURCE src/App.xaml PROPERTY VS_XAML_TYPE "ApplicationDefinition")
set_property(SOURCE src/MainWindow.xaml PROPERTY VS_XAML_TYPE "Page")
set_property(SOURCE src/App.idl PROPERTY VS_SETTINGS "SubType=Code" "DependentUpon=src/App.xaml")
set_property(SOURCE src/MainWindow.idl PROPERTY VS_SETTINGS "SubType=Code" "DependentUpon=src/MainWindow.xaml")

set_property(TARGET Vexio PROPERTY VS_PACKAGE_REFERENCES
    "Microsoft.Windows.CppWinRT_${CPPWINRT_VERSION};Microsoft.WindowsAppSDK_${WINDOWS_APP_SDK_VERSION};Microsoft.Windows.SDK.BuildTools_10.0.22621.756"
)

set_target_properties(Vexio PROPERTIES
    VS_GLOBAL_RootNamespace "VexioWindows"
    VS_GLOBAL_AppContainerApplication "false"
    VS_GLOBAL_AppxPackage "false"
    VS_GLOBAL_CppWinRTOptimized "true"
    VS_GLOBAL_CppWinRTRootNamespaceAutoMerge "true"
    VS_GLOBAL_UseWinUI "true"
    VS_GLOBAL_ApplicationType "Windows Store"
    VS_GLOBAL_ApplicationTypeRevision "10.0"
    VS_GLOBAL_WindowsPackageType "None"
    VS_GLOBAL_EnablePreviewMsixTooling "true"
    VS_GLOBAL_WindowsAppSDKSelfContained "false"
)

# Stage vexio_core.dll and Package.appxmanifest next to Vexio.exe for immediate launch & MSIX packaging
add_custom_command(TARGET Vexio POST_BUILD
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "${VEXIO_CORE_DLL}"
            "$<TARGET_FILE_DIR:Vexio>/vexio_core.dll"
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different
            "${CMAKE_CURRENT_SOURCE_DIR}/Package.appxmanifest"
            "$<TARGET_FILE_DIR:Vexio>/AppxManifest.xml"
    COMMENT "Staging vexio_core.dll and MSIX manifest alongside Vexio.exe"
    VERBATIM
)
