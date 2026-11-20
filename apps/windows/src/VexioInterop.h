#pragma once

#include <vexio.h>

#include <algorithm>
#include <cstdint>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <winrt/base.h>
#include <winrt/Microsoft.UI.Input.h>

namespace vexio::win {

// ---------------------------------------------------------------------------
// Pointer Event Kind, Button Bitmask & Modifier Bitmask Constants (1:1 vexio.h)
// ---------------------------------------------------------------------------
enum class PointerEventKind : uint32_t {
    Move       = 0,
    Down       = 1,
    Up         = 2,
    Wheel      = 3,
    StylusMove = 4,
};

namespace PointerButtons {
    inline constexpr uint32_t None      = 0;
    inline constexpr uint32_t Primary   = 1;
    inline constexpr uint32_t Secondary = 2;
    inline constexpr uint32_t Middle    = 4;
    inline constexpr uint32_t Eraser    = 32;
} // namespace PointerButtons

namespace KeyModifiers {
    inline constexpr uint32_t None  = 0;
    inline constexpr uint32_t Shift = 1;
    inline constexpr uint32_t Ctrl  = 2;
    inline constexpr uint32_t Alt   = 4;
    inline constexpr uint32_t Meta  = 8;
} // namespace KeyModifiers

// Canonical cursor codes emitted by vexio_viewport_dispatch_pointer
enum class ViewportCursor : uint32_t {
    Arrow       = 0,
    Hand        = 1,
    IBeam       = 2,
    Crosshair   = 3,
    MoveAll     = 4,
    ResizeNS    = 5,
    ResizeWE    = 6,
    ResizeNWSE  = 7,
    ResizeNESW  = 8,
    Forbidden   = 9,
    Rotate      = 10,
};

[[nodiscard]] constexpr winrt::Microsoft::UI::Input::InputSystemCursorShape
MapCursorToWinUI(uint32_t cursorKind) noexcept {
    using Shape = winrt::Microsoft::UI::Input::InputSystemCursorShape;
    switch (static_cast<ViewportCursor>(cursorKind)) {
        case ViewportCursor::Arrow:      return Shape::Arrow;
        case ViewportCursor::Hand:       return Shape::Hand;
        case ViewportCursor::IBeam:      return Shape::IBeam;
        case ViewportCursor::Crosshair:  return Shape::Cross;
        case ViewportCursor::MoveAll:    return Shape::SizeAll;
        case ViewportCursor::ResizeNS:   return Shape::SizeNorthSouth;
        case ViewportCursor::ResizeWE:   return Shape::SizeWestEast;
        case ViewportCursor::ResizeNWSE: return Shape::SizeNorthwestSoutheast;
        case ViewportCursor::ResizeNESW: return Shape::SizeNortheastSouthwest;
        case ViewportCursor::Forbidden:  return Shape::UniversalNo;
        case ViewportCursor::Rotate:     return Shape::Cross;
        default:                         return Shape::Arrow;
    }
}

[[nodiscard]] constexpr std::wstring_view DescribeStatus(VexioStatus status) noexcept {
    switch (status) {
        case VEXIO_OK:
            return L"OK";
        case VEXIO_ERR_INVALID_ARG:
            return L"Invalid argument passed to Vexio core engine.";
        case VEXIO_ERR_CELL_GUARDED:
            return L"Cell is protected by GUARD(). Enable Force SETF or edit the GUARD expression directly.";
        case VEXIO_ERR_FORMULA_SYNTAX:
            return L"ShapeSheet formula syntax error (#NAME? / #VALUE!). Check parentheses, units, and cell references.";
        case VEXIO_ERR_IO_CORRUPT_FORMAT:
            return L"Unable to parse diagram or CAD stream: file structure is corrupt or unsupported.";
        case VEXIO_ERR_GPU_SURFACE_LOST:
            return L"DirectX 12 DXGI SwapChain surface was lost and is being recreated.";
        case VEXIO_ERR_INTERNAL_PANIC:
            return L"Vexio core isolated an internal fault at the C-ABI boundary.";
        default:
            return L"Unexpected Vexio engine status code.";
    }
}

[[nodiscard]] inline std::string ToUtf8(std::wstring_view wide) {
    return winrt::to_string(wide);
}

[[nodiscard]] inline winrt::hstring ToHString(std::string_view utf8) {
    return winrt::to_hstring(utf8);
}

template <typename T>
struct VexioResult {
    VexioStatus        status{VEXIO_OK};
    std::optional<T>   value{};

    [[nodiscard]] constexpr bool ok() const noexcept {
        return status == VEXIO_OK && value.has_value();
    }

    [[nodiscard]] constexpr std::wstring_view message() const noexcept {
        return DescribeStatus(status);
    }
};

// ---------------------------------------------------------------------------
// OwnedBufferGuard: RAII wrapper around VexioOwnedBuffer + vexio_buffer_free
// ---------------------------------------------------------------------------
class OwnedBufferGuard final {
public:
    OwnedBufferGuard() noexcept = default;

    explicit OwnedBufferGuard(VexioOwnedBuffer raw) noexcept
        : m_buf(raw) {}

    ~OwnedBufferGuard() noexcept {
        Reset();
    }

    OwnedBufferGuard(const OwnedBufferGuard&) = delete;
    OwnedBufferGuard& operator=(const OwnedBufferGuard&) = delete;

    OwnedBufferGuard(OwnedBufferGuard&& other) noexcept
        : m_buf(std::exchange(other.m_buf, VexioOwnedBuffer{nullptr, 0, 0})) {}

    OwnedBufferGuard& operator=(OwnedBufferGuard&& other) noexcept {
        if (this != &other) {
            Reset();
            m_buf = std::exchange(other.m_buf, VexioOwnedBuffer{nullptr, 0, 0});
        }
        return *this;
    }

    void Reset() noexcept {
        if (m_buf.ptr != nullptr) {
            vexio_buffer_free(m_buf);
            m_buf = VexioOwnedBuffer{nullptr, 0, 0};
        }
    }

    [[nodiscard]] VexioOwnedBuffer* OutPtr() noexcept {
        Reset();
        return &m_buf;
    }

    [[nodiscard]] bool Empty() const noexcept {
        return m_buf.ptr == nullptr || m_buf.len == 0;
    }

    [[nodiscard]] size_t Size() const noexcept {
        return m_buf.len;
    }

    [[nodiscard]] std::span<const uint8_t> Bytes() const noexcept {
        if (m_buf.ptr == nullptr || m_buf.len == 0) {
            return {};
        }
        return std::span<const uint8_t>{m_buf.ptr, m_buf.len};
    }

    [[nodiscard]] std::string_view AsUtf8View() const noexcept {
        if (m_buf.ptr == nullptr || m_buf.len == 0) {
            return {};
        }
        return std::string_view{reinterpret_cast<const char*>(m_buf.ptr), m_buf.len};
    }

    [[nodiscard]] std::string ToStdString() const {
        return std::string{AsUtf8View()};
    }

    [[nodiscard]] winrt::hstring ToWinRtString() const {
        return ToHString(AsUtf8View());
    }

    [[nodiscard]] std::vector<uint8_t> ToVector() const {
        const auto span = Bytes();
        return std::vector<uint8_t>(span.begin(), span.end());
    }

private:
    VexioOwnedBuffer m_buf{nullptr, 0, 0};
};

// ---------------------------------------------------------------------------
// EngineHandle: RAII wrapper around VexioEngine*
// ---------------------------------------------------------------------------
class EngineHandle final {
public:
    EngineHandle() noexcept = default;

    explicit EngineHandle(VexioEngine* raw) noexcept
        : m_ptr(raw) {}

    [[nodiscard]] static EngineHandle Create() noexcept {
        return EngineHandle{vexio_engine_create()};
    }

    ~EngineHandle() noexcept {
        Reset();
    }

    EngineHandle(const EngineHandle&) = delete;
    EngineHandle& operator=(const EngineHandle&) = delete;

    EngineHandle(EngineHandle&& other) noexcept
        : m_ptr(std::exchange(other.m_ptr, nullptr)) {}

    EngineHandle& operator=(EngineHandle&& other) noexcept {
        if (this != &other) {
            Reset();
            m_ptr = std::exchange(other.m_ptr, nullptr);
        }
        return *this;
    }

    void Reset() noexcept {
        if (m_ptr != nullptr) {
            vexio_engine_destroy(m_ptr);
            m_ptr = nullptr;
        }
    }

    [[nodiscard]] VexioEngine* Get() const noexcept {
        return m_ptr;
    }

    [[nodiscard]] explicit operator bool() const noexcept {
        return m_ptr != nullptr;
    }

private:
    VexioEngine* m_ptr{nullptr};
};

// ---------------------------------------------------------------------------
// DocHandle: RAII wrapper around VexioDoc* + Save / Undo / Redo / ShapeSheet
// ---------------------------------------------------------------------------
class DocHandle final {
public:
    DocHandle() noexcept = default;

    explicit DocHandle(VexioDoc* raw) noexcept
        : m_ptr(raw) {}

    [[nodiscard]] static DocHandle CreateBlank(std::string_view titleUtf8) {
        const std::string nullTerminated{titleUtf8};
        return DocHandle{vexio_doc_create_blank(nullTerminated.c_str())};
    }

    [[nodiscard]] static VexioResult<DocHandle> OpenBytes(
        const EngineHandle& engine,
        std::span<const uint8_t> fileBytes,
        std::string_view extensionWithoutDotUtf8)
    {
        if (!engine) {
            return VexioResult<DocHandle>{VEXIO_ERR_INVALID_ARG, std::nullopt};
        }

        const std::string extNullTerminated{extensionWithoutDotUtf8};
        const VexioByteSlice slice{
            fileBytes.empty() ? nullptr : fileBytes.data(),
            fileBytes.size()
        };

        VexioDoc* rawDoc = nullptr;
        const VexioStatus status = vexio_doc_open_bytes(
            engine.Get(),
            slice,
            extNullTerminated.c_str(),
            &rawDoc);

        if (status != VEXIO_OK || rawDoc == nullptr) {
            if (rawDoc != nullptr) {
                vexio_doc_destroy(rawDoc);
            }
            return VexioResult<DocHandle>{
                status == VEXIO_OK ? VEXIO_ERR_IO_CORRUPT_FORMAT : status,
                std::nullopt
            };
        }

        return VexioResult<DocHandle>{VEXIO_OK, DocHandle{rawDoc}};
    }

    ~DocHandle() noexcept {
        Reset();
    }

    DocHandle(const DocHandle&) = delete;
    DocHandle& operator=(const DocHandle&) = delete;

    DocHandle(DocHandle&& other) noexcept
        : m_ptr(std::exchange(other.m_ptr, nullptr)) {}

    DocHandle& operator=(DocHandle&& other) noexcept {
        if (this != &other) {
            Reset();
            m_ptr = std::exchange(other.m_ptr, nullptr);
        }
        return *this;
    }

    void Reset() noexcept {
        if (m_ptr != nullptr) {
            vexio_doc_destroy(m_ptr);
            m_ptr = nullptr;
        }
    }

    [[nodiscard]] VexioDoc* Get() const noexcept {
        return m_ptr;
    }

    [[nodiscard]] explicit operator bool() const noexcept {
        return m_ptr != nullptr;
    }

    [[nodiscard]] VexioResult<OwnedBufferGuard> SaveFormat(std::string_view extWithoutDotUtf8) const {
        if (m_ptr == nullptr) {
            return VexioResult<OwnedBufferGuard>{VEXIO_ERR_INVALID_ARG, std::nullopt};
        }

        const std::string extNullTerminated{extWithoutDotUtf8};
        OwnedBufferGuard buffer;
        const VexioStatus status = vexio_doc_save_format(
            m_ptr,
            extNullTerminated.c_str(),
            buffer.OutPtr());

        if (status != VEXIO_OK) {
            return VexioResult<OwnedBufferGuard>{status, std::nullopt};
        }
        return VexioResult<OwnedBufferGuard>{VEXIO_OK, std::move(buffer)};
    }

    [[nodiscard]] VexioResult<OwnedBufferGuard> RenderPreviewRgba(
        uint32_t pageId,
        uint32_t widthPx,
        uint32_t heightPx) const
    {
        if (m_ptr == nullptr || widthPx == 0 || heightPx == 0) {
            return VexioResult<OwnedBufferGuard>{VEXIO_ERR_INVALID_ARG, std::nullopt};
        }

        OwnedBufferGuard rgba;
        const VexioStatus status = vexio_render_preview_rgba(
            m_ptr,
            pageId,
            widthPx,
            heightPx,
            rgba.OutPtr());

        if (status != VEXIO_OK) {
            return VexioResult<OwnedBufferGuard>{status, std::nullopt};
        }
        return VexioResult<OwnedBufferGuard>{VEXIO_OK, std::move(rgba)};
    }

    [[nodiscard]] VexioResult<winrt::hstring> GetCellFormula(
        uint32_t pageId,
        uint64_t shapeId,
        std::string_view cellNameUtf8) const
    {
        if (m_ptr == nullptr) {
            return VexioResult<winrt::hstring>{VEXIO_ERR_INVALID_ARG, std::nullopt};
        }

        const std::string cellNullTerminated{cellNameUtf8};
        OwnedBufferGuard utf8;
        const VexioStatus status = vexio_cell_get_formula(
            m_ptr,
            pageId,
            shapeId,
            cellNullTerminated.c_str(),
            utf8.OutPtr());

        if (status != VEXIO_OK) {
            return VexioResult<winrt::hstring>{status, std::nullopt};
        }
        return VexioResult<winrt::hstring>{VEXIO_OK, utf8.ToWinRtString()};
    }

    [[nodiscard]] VexioResult<winrt::hstring> GetCellValueStr(
        uint32_t pageId,
        uint64_t shapeId,
        std::string_view cellNameUtf8) const
    {
        if (m_ptr == nullptr) {
            return VexioResult<winrt::hstring>{VEXIO_ERR_INVALID_ARG, std::nullopt};
        }

        const std::string cellNullTerminated{cellNameUtf8};
        OwnedBufferGuard utf8;
        const VexioStatus status = vexio_cell_get_value_str(
            m_ptr,
            pageId,
            shapeId,
            cellNullTerminated.c_str(),
            utf8.OutPtr());

        if (status != VEXIO_OK) {
            return VexioResult<winrt::hstring>{status, std::nullopt};
        }
        return VexioResult<winrt::hstring>{VEXIO_OK, utf8.ToWinRtString()};
    }

    [[nodiscard]] VexioStatus SetCellFormula(
        uint32_t pageId,
        uint64_t shapeId,
        std::string_view cellNameUtf8,
        std::string_view formulaUtf8,
        bool forceSetf) noexcept
    {
        if (m_ptr == nullptr) {
            return VEXIO_ERR_INVALID_ARG;
        }

        const std::string cellNullTerminated{cellNameUtf8};
        const std::string formulaNullTerminated{formulaUtf8};
        return vexio_cell_set_formula(
            m_ptr,
            pageId,
            shapeId,
            cellNullTerminated.c_str(),
            formulaNullTerminated.c_str(),
            forceSetf);
    }

    [[nodiscard]] VexioStatus Undo() noexcept {
        if (m_ptr == nullptr) {
            return VEXIO_ERR_INVALID_ARG;
        }
        return vexio_doc_undo(m_ptr);
    }

    [[nodiscard]] VexioStatus Redo() noexcept {
        if (m_ptr == nullptr) {
            return VEXIO_ERR_INVALID_ARG;
        }
        return vexio_doc_redo(m_ptr);
    }

private:
    VexioDoc* m_ptr{nullptr};
};

// ---------------------------------------------------------------------------
// ViewportHandle: RAII wrapper around VexioViewport* + DX12 SwapChainPanel
// ---------------------------------------------------------------------------
class ViewportHandle final {
public:
    ViewportHandle() noexcept = default;

    explicit ViewportHandle(VexioViewport* raw) noexcept
        : m_ptr(raw) {}

    [[nodiscard]] static VexioResult<ViewportHandle> CreateDx12SwapChain(
        const EngineHandle& engine,
        const DocHandle& doc,
        ISwapChainPanelNative* swapChainPanelNative,
        uint32_t widthPx,
        uint32_t heightPx,
        float dpiScale) noexcept
    {
        if (!engine || !doc || swapChainPanelNative == nullptr) {
            return VexioResult<ViewportHandle>{VEXIO_ERR_INVALID_ARG, std::nullopt};
        }

        const uint32_t safeWidth  = std::max<uint32_t>(1u, widthPx);
        const uint32_t safeHeight = std::max<uint32_t>(1u, heightPx);
        const float    safeDpi    = std::clamp(dpiScale, 0.5f, 8.0f);

        VexioViewport* rawViewport = nullptr;
        const VexioStatus status = vexio_viewport_create_dx12_swapchain(
            engine.Get(),
            doc.Get(),
            static_cast<void*>(swapChainPanelNative),
            safeWidth,
            safeHeight,
            safeDpi,
            &rawViewport);

        if (status != VEXIO_OK || rawViewport == nullptr) {
            if (rawViewport != nullptr) {
                vexio_viewport_destroy(rawViewport);
            }
            return VexioResult<ViewportHandle>{
                status == VEXIO_OK ? VEXIO_ERR_GPU_SURFACE_LOST : status,
                std::nullopt
            };
        }

        return VexioResult<ViewportHandle>{VEXIO_OK, ViewportHandle{rawViewport}};
    }

    [[nodiscard]] static VexioResult<ViewportHandle> CreateHeadless(
        const EngineHandle& engine,
        const DocHandle& doc,
        uint32_t pageId,
        uint32_t widthPx,
        uint32_t heightPx,
        float dpiScale) noexcept
    {
        if (!engine || !doc) {
            return VexioResult<ViewportHandle>{VEXIO_ERR_INVALID_ARG, std::nullopt};
        }

        VexioViewport* rawViewport = nullptr;
        const VexioStatus status = vexio_viewport_create_headless(
            engine.Get(),
            doc.Get(),
            pageId,
            std::max<uint32_t>(1u, widthPx),
            std::max<uint32_t>(1u, heightPx),
            std::clamp(dpiScale, 0.5f, 8.0f),
            &rawViewport);

        if (status != VEXIO_OK || rawViewport == nullptr) {
            if (rawViewport != nullptr) {
                vexio_viewport_destroy(rawViewport);
            }
            return VexioResult<ViewportHandle>{
                status == VEXIO_OK ? VEXIO_ERR_INVALID_ARG : status,
                std::nullopt
            };
        }

        return VexioResult<ViewportHandle>{VEXIO_OK, ViewportHandle{rawViewport}};
    }

    ~ViewportHandle() noexcept {
        Reset();
    }

    ViewportHandle(const ViewportHandle&) = delete;
    ViewportHandle& operator=(const ViewportHandle&) = delete;

    ViewportHandle(ViewportHandle&& other) noexcept
        : m_ptr(std::exchange(other.m_ptr, nullptr)) {}

    ViewportHandle& operator=(ViewportHandle&& other) noexcept {
        if (this != &other) {
            Reset();
            m_ptr = std::exchange(other.m_ptr, nullptr);
        }
        return *this;
    }

    void Reset() noexcept {
        if (m_ptr != nullptr) {
            vexio_viewport_destroy(m_ptr);
            m_ptr = nullptr;
        }
    }

    [[nodiscard]] VexioViewport* Get() const noexcept {
        return m_ptr;
    }

    [[nodiscard]] explicit operator bool() const noexcept {
        return m_ptr != nullptr;
    }

    [[nodiscard]] VexioResult<uint32_t> DispatchPointer(
        const DocHandle& doc,
        const VexioPointerEvent& event) const noexcept
    {
        if (m_ptr == nullptr || !doc) {
            return VexioResult<uint32_t>{VEXIO_ERR_INVALID_ARG, std::nullopt};
        }

        uint32_t cursorKind = 0;
        const VexioStatus status = vexio_viewport_dispatch_pointer(
            m_ptr,
            doc.Get(),
            &event,
            &cursorKind);

        if (status != VEXIO_OK) {
            return VexioResult<uint32_t>{status, std::nullopt};
        }
        return VexioResult<uint32_t>{VEXIO_OK, cursorKind};
    }

private:
    VexioViewport* m_ptr{nullptr};
};

} // namespace vexio::win
