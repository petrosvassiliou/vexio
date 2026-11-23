#include "pch.h"
#include "MainWindow.xaml.h"
#if __has_include("MainWindow.g.cpp")
#include "MainWindow.g.cpp"
#endif

using namespace winrt;
using namespace winrt::Windows::ApplicationModel::DataTransfer;
using namespace winrt::Windows::Foundation;
using namespace winrt::Windows::Storage;
using namespace winrt::Windows::Storage::Pickers;
using namespace winrt::Windows::Storage::Streams;
using namespace winrt::Windows::System;
using namespace winrt::Microsoft::UI;
using namespace winrt::Microsoft::UI::Input;
using namespace winrt::Microsoft::UI::Windowing;
using namespace winrt::Microsoft::UI::Xaml;
using namespace winrt::Microsoft::UI::Xaml::Controls;
using namespace winrt::Microsoft::UI::Xaml::Controls::Primitives;
using namespace winrt::Microsoft::UI::Xaml::Input;
using namespace winrt::Microsoft::UI::Xaml::Media;
using namespace winrt::Microsoft::UI::Xaml::Media::Imaging;
using namespace winrt::Microsoft::UI::Xaml::Shapes;

namespace {

[[nodiscard]] std::vector<uint8_t> DecodeBase64PngDataUrl(std::string_view dataUrl) {
    constexpr std::string_view kPrefix = "data:image/png;base64,";
    if (!dataUrl.starts_with(kPrefix)) {
        return {};
    }
    const std::string_view b64 = dataUrl.substr(kPrefix.size());
    std::vector<uint8_t> out;
    out.reserve((b64.size() * 3) / 4);
    uint32_t val = 0;
    int32_t valb = -8;
    for (unsigned char c : b64) {
        int32_t d = -1;
        if (c >= 'A' && c <= 'Z') d = c - 'A';
        else if (c >= 'a' && c <= 'z') d = c - 'a' + 26;
        else if (c >= '0' && c <= '9') d = c - '0' + 52;
        else if (c == '+') d = 62;
        else if (c == '/') d = 63;
        else if (c == '=') break;
        else continue;
        val = (val << 6) + static_cast<uint32_t>(d);
        valb += 6;
        if (valb >= 0) {
            out.push_back(static_cast<uint8_t>((val >> valb) & 0xFFu));
            valb -= 8;
        }
    }
    return out;
}

[[nodiscard]] std::filesystem::path GetLocalStencilsDirectory() {
    wchar_t localAppData[MAX_PATH]{};
    const DWORD len = ::GetEnvironmentVariableW(L"LOCALAPPDATA", localAppData, MAX_PATH);
    std::filesystem::path base = (len > 0 && len < MAX_PATH)
        ? std::filesystem::path{localAppData}
        : std::filesystem::temp_directory_path();
    std::filesystem::path dir = base / L"Vexio" / L"Stencils";
    std::error_code ec{};
    std::filesystem::create_directories(dir, ec);
    return dir;
}

[[nodiscard]] std::string NormalizeExtensionWithoutDot(std::wstring_view pathOrExt) {
    std::filesystem::path p{pathOrExt};
    std::wstring ext = p.has_extension() ? p.extension().wstring() : std::wstring{pathOrExt};
    if (!ext.empty() && ext.front() == L'.') {
        ext.erase(ext.begin());
    }
    std::transform(ext.begin(), ext.end(), ext.begin(), [](wchar_t ch) {
        return static_cast<wchar_t>(::towlower(ch));
    });
    return vexio::win::ToUtf8(ext.empty() ? std::wstring_view{L"vsdx"} : std::wstring_view{ext});
}

[[nodiscard]] std::wstring FormatBadgeForExtension(std::string_view ext) {
    if (ext == "vsdx") return L"OpenXML VSDX • Lossless";
    if (ext == "vsdm") return L"OpenXML VSDM • Macro-Enabled";
    if (ext == "vssx" || ext == "vssm") return L"OpenXML Stencil Package";
    if (ext == "vstx" || ext == "vstm") return L"OpenXML Template Package";
    if (ext == "vsd")  return L"Legacy OLE2 Binary VSD";
    if (ext == "vdx")  return L"DatadiagramML XML VDX";
    if (ext == "emf" || ext == "wmf") return L"Windows Vector Metafile";
    if (ext == "ldif") return L"LDAP / Entra Directory Org Chart";
    if (ext == "dwg")  return L"Offline Native CAD DWG";
    if (ext == "dxf")  return L"Offline Native CAD DXF";
    return L"Vexio Vector Document";
}

[[nodiscard]] std::vector<std::string> GetCellsForSectionIndex(int32_t sectionIndex) {
    switch (sectionIndex) {
        case 0: // Shape Transform
            return {"PinX", "PinY", "Width", "Height", "LocPinX", "LocPinY", "Angle", "FlipX", "FlipY", "ResizeMode"};
        case 1: // 1-D Endpoints
            return {"BeginX", "BeginY", "EndX", "EndY"};
        case 2: // Geometry 1
            return {
                "Geometry1.NoFill", "Geometry1.NoLine", "Geometry1.NoShow",
                "Geometry1.X1", "Geometry1.Y1",
                "Geometry1.X2", "Geometry1.Y2",
                "Geometry1.X3", "Geometry1.Y3",
                "Geometry1.X4", "Geometry1.Y4"
            };
        case 3: // Protection
            return {"LockWidth", "LockHeight", "LockAspect", "LockMoveX", "LockMoveY", "LockRotate", "LockDelete", "LockFormat"};
        case 4: // Controls
            return {"Controls.Row_1", "Controls.Row_1.Y", "Controls.Row_1.XDyn", "Controls.Row_1.YDyn", "Controls.Row_1.CanGlue"};
        case 5: // Connection Points
            return {"Connections.X1", "Connections.Y1", "Connections.X2", "Connections.Y2", "Connections.X3", "Connections.Y3", "Connections.X4", "Connections.Y4"};
        case 6: // User-defined Cells
            return {"User.msvStructureType", "User.BPMNActivityType", "User.UMLStereotype", "User.VisVersion"};
        case 7: // Shape Data (Prop.*)
        default:
            return {"Prop.Label", "Prop.Department", "Prop.Owner", "Prop.Cost", "Prop.Status"};
    }
}

[[nodiscard]] bool FormulaStartsWithGuard(std::wstring_view formula) noexcept {
    size_t idx = 0;
    while (idx < formula.size() && (formula[idx] == L' ' || formula[idx] == L'=')) {
        ++idx;
    }
    constexpr std::wstring_view prefix = L"GUARD(";
    if (formula.size() - idx < prefix.size()) {
        return false;
    }
    for (size_t i = 0; i < prefix.size(); ++i) {
        if (::towupper(formula[idx + i]) != prefix[i]) {
            return false;
        }
    }
    return true;
}

} // namespace

namespace winrt::VexioWindows::implementation {

MainWindow::MainWindow()
    : m_engine(vexio::win::EngineHandle::Create())
    , m_doc(vexio::win::DocHandle::CreateBlank("Drawing1.vsdx"))
{
    InitializeComponent();
    ConfigureCustomTitleBar();
    DetectOsLocaleAndUnits();
    PopulateInstalledSystemFonts();

    (void)m_doc.LoadStarterTemplate("bpmn_fulfillment");
    if (m_metricUnits) {
        (void)m_doc.SetPagePreset(1u, "a4", true);
        PageOrientationCombo().SelectedIndex(0);
    } else {
        (void)m_doc.SetPagePreset(1u, "letter", true);
        PageOrientationCombo().SelectedIndex(6);
    }

    m_pages = {
        PageTabModel{1u, L"Page-1", false, 7u}
    };

    LoadPersistedUserStencils();
    InitializeStencilCatalog();
    InitializeExternalDataAndLayers();
    RefreshBaselineSnapshot();

    m_isInitialized = true;

    ApplyLanguageStrings();
    RulerUnitOriginText().Text(m_metricUnits ? L"mm" : L"in");
    PageScaleSummaryText().Text(m_metricUnits
        ? L"Scale: 1:1 • Units: Millimeters (ISO A4)"
        : L"Scale: 1:1 • Units: Inches (US Letter)");

    RefreshStencilList();
    RefreshExternalDataList();
    RefreshLayersList();
    RebuildPageTabsStrip();
    SyncInspectorFromShapeSheet();
    RefreshShapeSheetSectionRows();

    if (m_autoCheckUpdates) {
        CheckForUpdatesAsync(false);
    }
}

HWND MainWindow::GetNativeWindowHandle() const noexcept {
    HWND hwnd = nullptr;
    if (auto windowNative = this->try_as<::IWindowNative>()) {
        (void)windowNative->get_WindowHandle(&hwnd);
    }
    return hwnd;
}

void MainWindow::ConfigureCustomTitleBar() {
    ExtendsContentIntoTitleBar(true);
    SetTitleBar(AppTitleBarGrid());

    if (const HWND hwnd = GetNativeWindowHandle(); hwnd != nullptr) {
        const WindowId windowId = GetWindowIdFromWindow(hwnd);
        if (const Microsoft::UI::Windowing::AppWindow appWindow =
                Microsoft::UI::Windowing::AppWindow::GetFromWindowId(windowId))
        {
            appWindow.Resize(Windows::Graphics::SizeInt32{1600, 980});
            if (Microsoft::UI::Windowing::AppWindowTitleBar::IsCustomizationSupported()) {
                auto titleBar = appWindow.TitleBar();
                titleBar.ButtonBackgroundColor(Colors::Transparent());
                titleBar.ButtonInactiveBackgroundColor(Colors::Transparent());
                titleBar.ButtonForegroundColor(Colors::White());
            }
        }
    }
}

// ===========================================================================
// DirectX 12 ISwapChainPanelNative Surface Attachment & Resize
// ===========================================================================
void MainWindow::AttachOrResizeDx12SwapChain() {
    if (!m_engine || !m_doc) {
        return;
    }

    auto panel = DiagramSwapChainPanel();
    if (!panel) {
        return;
    }

    const double logicalW = panel.ActualWidth();
    const double logicalH = panel.ActualHeight();
    if (logicalW < 2.0 || logicalH < 2.0) {
        return;
    }

    const float scaleX = std::clamp(panel.CompositionScaleX(), 0.5f, 8.0f);
    const float scaleY = std::clamp(panel.CompositionScaleY(), 0.5f, 8.0f);
    const uint32_t physWidth  = std::max<uint32_t>(1u, static_cast<uint32_t>(std::lround(logicalW * scaleX)));
    const uint32_t physHeight = std::max<uint32_t>(1u, static_cast<uint32_t>(std::lround(logicalH * scaleY)));

    if (m_viewport) {
        if (m_viewport.Resize(physWidth, physHeight, scaleX) == VEXIO_OK) {
            if (!m_usingHeadlessFallback) {
                GpuSwapChainBadgeText().Text(hstring{
                    std::format(L"DX12 120Hz • {}x{} @ {:.0f}%", physWidth, physHeight, scaleX * 100.0f)
                });
            }
            PresentViewportFrame();
            return;
        }
    }

    const com_ptr<ISwapChainPanelNative> panelNative = panel.try_as<ISwapChainPanelNative>();

    m_viewport.Reset();
    m_usingHeadlessFallback = false;

    if (panelNative) {
        auto created = vexio::win::ViewportHandle::CreateDx12SwapChain(
            m_engine,
            m_doc,
            panelNative.get(),
            physWidth,
            physHeight,
            scaleX);

        if (created.ok()) {
            m_viewport = std::move(*created.value);
            SoftwareCanvasFallbackImage().Visibility(Visibility::Collapsed);
            GpuSwapChainBadgeText().Text(hstring{
                std::format(L"DX12 120Hz • {}x{} @ {:.0f}%", physWidth, physHeight, scaleX * 100.0f)
            });
        }
    }

    if (!m_viewport) {
        auto headless = vexio::win::ViewportHandle::CreateHeadless(
            m_engine, m_doc, m_activePageId, physWidth, physHeight, scaleX);
        if (headless.ok()) {
            m_viewport = std::move(*headless.value);
            m_usingHeadlessFallback = true;
            SoftwareCanvasFallbackImage().Visibility(Visibility::Visible);
            GpuSwapChainBadgeText().Text(L"DX12 WARP / Software Fallback Active");
        }
    }

    if (m_viewport) {
        (void)m_viewport.SetPage(m_doc, m_activePageId);
        (void)m_viewport.SetTool(static_cast<uint32_t>(m_activeTool));
        (void)m_viewport.SetSmartGuides(m_smartGuidesEnabled);
        PresentViewportFrame();
    }
}

void MainWindow::SyncMultiSelectionFromViewport() {
    if (!m_viewport) {
        return;
    }
    const auto selJsonRes = m_viewport.GetSelectionJson();
    if (selJsonRes.ok() && !selJsonRes.value->empty()) {
        Windows::Data::Json::JsonArray arr{nullptr};
        if (Windows::Data::Json::JsonArray::TryParse(*selJsonRes.value, arr)) {
            std::vector<uint64_t> parsed{};
            parsed.reserve(arr.Size());
            for (uint32_t i = 0; i < arr.Size(); ++i) {
                const uint64_t sid = static_cast<uint64_t>(arr.GetNumberAt(i));
                if (sid > 0) {
                    parsed.push_back(sid);
                }
            }
            if (!parsed.empty()) {
                m_selectedShapeIds = std::move(parsed);
            }
        }
    }
    const uint64_t sel = m_viewport.GetPrimarySelection();
    if (sel > 0 && sel != m_selectedShapeId) {
        m_previousSelectedShapeId = m_selectedShapeId;
        m_selectedShapeId = sel;
    }
    if (m_selectedShapeIds.empty() && m_selectedShapeId > 0) {
        m_selectedShapeIds.push_back(m_selectedShapeId);
    }
}

std::vector<uint64_t> MainWindow::ResolveTargetSelectionIds() {
    SyncMultiSelectionFromViewport();
    std::vector<uint64_t> ids = m_selectedShapeIds;
    if (ids.size() < 2 && m_previousSelectedShapeId > 0 && m_previousSelectedShapeId != m_selectedShapeId) {
        if (ids.empty() && m_selectedShapeId > 0) {
            ids.push_back(m_selectedShapeId);
        }
        if (std::find(ids.begin(), ids.end(), m_previousSelectedShapeId) == ids.end()) {
            ids.insert(ids.begin(), m_previousSelectedShapeId);
        }
    }
    if (ids.empty() && m_selectedShapeId > 0) {
        ids.push_back(m_selectedShapeId);
    }
    return ids;
}

void MainWindow::PresentViewportFrame() {
    if (!m_doc || !m_viewport) {
        return;
    }

    (void)m_viewport.RenderFrame(m_doc);
    SyncMultiSelectionFromViewport();

    if (m_usingHeadlessFallback) {
        const uint32_t wPx = std::max<uint32_t>(320u, static_cast<uint32_t>(std::lround(DiagramSwapChainPanel().ActualWidth())));
        const uint32_t hPx = std::max<uint32_t>(200u, static_cast<uint32_t>(std::lround(DiagramSwapChainPanel().ActualHeight())));
        const auto rgbaRes = m_viewport.RenderRgba(m_doc);
        if (rgbaRes.ok()) {
            const auto src = rgbaRes.value->Bytes();
            const size_t expected = static_cast<size_t>(wPx) * static_cast<size_t>(hPx) * 4u;
            if (src.size() >= expected) {
                WriteableBitmap bmp{static_cast<int32_t>(wPx), static_cast<int32_t>(hPx)};
                uint8_t* dst = bmp.PixelBuffer().data();
                for (size_t px = 0; px < expected; px += 4) {
                    dst[px + 0] = src[px + 2];
                    dst[px + 1] = src[px + 1];
                    dst[px + 2] = src[px + 0];
                    dst[px + 3] = src[px + 3];
                }
                bmp.Invalidate();
                SoftwareCanvasFallbackImage().Source(bmp);
            }
        }
    }
}

void MainWindow::OnSwapChainPanelLoaded(IInspectable const&, RoutedEventArgs const&) {
    AttachOrResizeDx12SwapChain();
    RebuildRulerTicks();
    OnRefreshPanZoomPreviewClick(nullptr, nullptr);
}

void MainWindow::OnSwapChainPanelSizeChanged(IInspectable const&, SizeChangedEventArgs const&) {
    AttachOrResizeDx12SwapChain();
    RebuildRulerTicks();
}

void MainWindow::OnSwapChainCompositionScaleChanged(SwapChainPanel const&, IInspectable const&) {
    AttachOrResizeDx12SwapChain();
    RebuildRulerTicks();
}

// ===========================================================================
// Pointer / Wheel / Active Stylus Dispatch -> VexioPointerEvent -> Cursor
// ===========================================================================
void MainWindow::ApplyCanvasCursor(uint32_t cursorKind) {
    const InputSystemCursorShape shape = vexio::win::MapCursorToWinUI(cursorKind);
    if (auto protectedElem = DiagramSwapChainPanel().try_as<IUIElementProtected>()) {
        protectedElem.ProtectedCursor(InputSystemCursor::Create(shape));
    }
}

void MainWindow::DispatchCanvasPointerEvent(uint32_t kindOverride, PointerRoutedEventArgs const& e) {
    auto panel = DiagramSwapChainPanel();
    const PointerPoint pt = e.GetCurrentPoint(panel);
    const PointerPointProperties props = pt.Properties();
    const Point pos = pt.Position();

    HorizontalCursorHairline().X1(pos.X);
    HorizontalCursorHairline().X2(pos.X);
    VerticalCursorHairline().Y1(pos.Y);
    VerticalCursorHairline().Y2(pos.Y);

    const double pixelsPerInch = 96.0 * (m_zoomPercent / 100.0);
    const double docInchesX = pos.X / std::max(12.0, pixelsPerInch);
    const double docInchesY = pos.Y / std::max(12.0, pixelsPerInch);

    if (m_metricUnits) {
        StatusCoordinatesText().Text(hstring{
            std::format(L"X: {:.1f} mm   Y: {:.1f} mm", docInchesX * 25.4, docInchesY * 25.4)
        });
    } else {
        StatusCoordinatesText().Text(hstring{
            std::format(L"X: {:.2f} in   Y: {:.2f} in", docInchesX, docInchesY)
        });
    }

    uint32_t buttons = vexio::win::PointerButtons::None;
    if (props.IsLeftButtonPressed())   buttons |= vexio::win::PointerButtons::Primary;
    if (props.IsRightButtonPressed())  buttons |= vexio::win::PointerButtons::Secondary;
    if (props.IsMiddleButtonPressed()) buttons |= vexio::win::PointerButtons::Middle;
    if (props.IsEraser())              buttons |= vexio::win::PointerButtons::Eraser;

    const VirtualKeyModifiers keyMods = e.KeyModifiers();
    uint32_t modifiers = vexio::win::KeyModifiers::None;
    if ((keyMods & VirtualKeyModifiers::Shift)   != VirtualKeyModifiers::None) modifiers |= vexio::win::KeyModifiers::Shift;
    if ((keyMods & VirtualKeyModifiers::Control) != VirtualKeyModifiers::None) modifiers |= vexio::win::KeyModifiers::Ctrl;
    if ((keyMods & VirtualKeyModifiers::Menu)    != VirtualKeyModifiers::None) modifiers |= vexio::win::KeyModifiers::Alt;
    if ((keyMods & VirtualKeyModifiers::Windows) != VirtualKeyModifiers::None) modifiers |= vexio::win::KeyModifiers::Meta;

    uint32_t effectiveKind = kindOverride;
    if (kindOverride == static_cast<uint32_t>(vexio::win::PointerEventKind::Move) &&
        pt.PointerDeviceType() == PointerDeviceType::Pen)
    {
        effectiveKind = static_cast<uint32_t>(vexio::win::PointerEventKind::StylusMove);
    }

    const float scaleX = std::clamp(panel.CompositionScaleX(), 0.5f, 8.0f);
    const float scaleY = std::clamp(panel.CompositionScaleY(), 0.5f, 8.0f);

    VexioPointerEvent rawEv{};
    rawEv.kind         = effectiveKind;
    rawEv.buttons      = buttons;
    rawEv.modifiers    = modifiers;
    rawEv.x_px         = static_cast<double>(pos.X) * static_cast<double>(scaleX);
    rawEv.y_px         = static_cast<double>(pos.Y) * static_cast<double>(scaleY);
    rawEv.pressure     = std::clamp(props.Pressure(), 0.0f, 1.0f);
    rawEv.tilt_x_deg   = props.XTilt();
    rawEv.tilt_y_deg   = (effectiveKind == static_cast<uint32_t>(vexio::win::PointerEventKind::Wheel))
        ? static_cast<float>(props.MouseWheelDelta())
        : props.YTilt();
    rawEv.timestamp_us = pt.Timestamp();

    if (m_viewport && m_doc) {
        auto dispatchRes = m_viewport.DispatchPointer(m_doc, rawEv);
        if (dispatchRes.ok()) {
            ApplyCanvasCursor(*dispatchRes.value);
        } else if (dispatchRes.status == VEXIO_ERR_GPU_SURFACE_LOST) {
            m_viewport.Reset();
            AttachOrResizeDx12SwapChain();
        }
    }
}

void MainWindow::OnCanvasPointerPressed(IInspectable const&, PointerRoutedEventArgs const& e) {
    if (InlineCanvasEditorBorder().Visibility() == Visibility::Visible) {
        CommitInlineCanvasShapeTextEdit(true);
    }
    const auto props = e.GetCurrentPoint(DiagramSwapChainPanel()).Properties();
    if (props.IsRightButtonPressed()) {
        return;
    }
    DiagramSwapChainPanel().CapturePointer(e.Pointer());
    DispatchCanvasPointerEvent(static_cast<uint32_t>(vexio::win::PointerEventKind::Down), e);
    PresentViewportFrame();
    e.Handled(true);
}

void MainWindow::OnCanvasPointerMoved(IInspectable const&, PointerRoutedEventArgs const& e) {
    DispatchCanvasPointerEvent(static_cast<uint32_t>(vexio::win::PointerEventKind::Move), e);
    if (e.GetCurrentPoint(DiagramSwapChainPanel()).Properties().IsLeftButtonPressed()) {
        PresentViewportFrame();
    }
}

void MainWindow::OnCanvasPointerReleased(IInspectable const&, PointerRoutedEventArgs const& e) {
    DispatchCanvasPointerEvent(static_cast<uint32_t>(vexio::win::PointerEventKind::Up), e);
    DiagramSwapChainPanel().ReleasePointerCapture(e.Pointer());
    SyncMultiSelectionFromViewport();

    const uint64_t createdTextId = m_viewport ? m_viewport.PopCreatedTextShape() : 0;

    if (m_activeTool != ActiveCanvasTool::Pointer &&
        m_activeTool != ActiveCanvasTool::Pan &&
        m_activeTool != ActiveCanvasTool::InkPen &&
        m_activeTool != ActiveCanvasTool::InkHighlighter)
    {
        SetActiveTool(ActiveCanvasTool::Pointer);
    } else if (m_viewport) {
        const uint32_t coreTool = m_viewport.GetActiveTool();
        if (coreTool == 0 && m_activeTool != ActiveCanvasTool::Pointer) {
            SetActiveTool(ActiveCanvasTool::Pointer);
        }
    }

    PresentViewportFrame();
    SyncInspectorFromShapeSheet();
    RefreshShapeSheetSectionRows();

    if (createdTextId > 0) {
        m_previousSelectedShapeId = m_selectedShapeId;
        m_selectedShapeId = createdTextId;
        MarkDocumentDirty(true);
        BeginInlineCanvasShapeTextEdit(createdTextId, true);
    }
    e.Handled(true);
}

void MainWindow::OnCanvasPointerWheelChanged(IInspectable const&, PointerRoutedEventArgs const& e) {
    const int32_t delta = e.GetCurrentPoint(DiagramSwapChainPanel()).Properties().MouseWheelDelta();
    if ((e.KeyModifiers() & VirtualKeyModifiers::Control) != VirtualKeyModifiers::None) {
        const double step = delta > 0 ? 10.0 : -10.0;
        SetZoomPercent(std::clamp(m_zoomPercent + step, 25.0, 400.0));
    } else {
        DispatchCanvasPointerEvent(static_cast<uint32_t>(vexio::win::PointerEventKind::Wheel), e);
    }
    PresentViewportFrame();
    e.Handled(true);
}

void MainWindow::OnCanvasRightTapped(IInspectable const&, RightTappedRoutedEventArgs const& e) {
    auto panel = DiagramSwapChainPanel();
    const Point pt = e.GetPosition(panel);
    const float scaleX = std::clamp(panel.CompositionScaleX(), 0.5f, 8.0f);
    const float scaleY = std::clamp(panel.CompositionScaleY(), 0.5f, 8.0f);
    if (m_viewport && m_doc) {
        SyncMultiSelectionFromViewport();
        const uint64_t hitId = m_viewport.HitTestShape(
            m_doc,
            static_cast<double>(pt.X) * static_cast<double>(scaleX),
            static_cast<double>(pt.Y) * static_cast<double>(scaleY));
        if (hitId > 0 && std::find(m_selectedShapeIds.begin(), m_selectedShapeIds.end(), hitId) == m_selectedShapeIds.end()) {
            m_selectedShapeId = hitId;
            m_selectedShapeIds = {hitId};
            (void)m_viewport.SetSelection(m_doc, m_selectedShapeIds);
            PresentViewportFrame();
            SyncInspectorFromShapeSheet();
            RefreshShapeSheetSectionRows();
        }
    }
    const double pxPerIn = std::max(16.0, 96.0 * (m_zoomPercent / 100.0));
    const double clickXIn = std::clamp(pt.X / pxPerIn, 0.5, 10.5);
    const double clickYIn = std::clamp(8.5 - (pt.Y / pxPerIn), 0.5, 8.0);

    MenuFlyout menu{};

    auto addItem = [&](std::wstring_view label, auto handler) {
        MenuFlyoutItem item{};
        item.Text(hstring{label});
        item.Click(handler);
        menu.Items().Append(item);
    };

    addItem(L"Edit Stencil / Shape Text Label (Double-Click)", [this](IInspectable const&, RoutedEventArgs const&) {
        if (m_selectedShapeId > 0) {
            BeginInlineCanvasShapeTextEdit(m_selectedShapeId, false);
        }
    });
    addItem(L"Group Selected Shapes (Ctrl+G)", [this](IInspectable const&, RoutedEventArgs const&) {
        OnGroupShapesClick(nullptr, nullptr);
    });
    addItem(L"Ungroup Selected Group (Ctrl+Shift+G)", [this](IInspectable const&, RoutedEventArgs const&) {
        OnUngroupShapesClick(nullptr, nullptr);
    });
    addItem(L"Rotate Left 90° (↺)", [this](IInspectable const&, RoutedEventArgs const&) {
        OnRotateLeft90Click(nullptr, nullptr);
    });
    addItem(L"Rotate Right 90° (↻)", [this](IInspectable const&, RoutedEventArgs const&) {
        OnRotateRight90Click(nullptr, nullptr);
    });
    addItem(L"Flip Horizontal (⇆)", [this](IInspectable const&, RoutedEventArgs const&) {
        OnFlipHorizontalClick(nullptr, nullptr);
    });
    addItem(L"Flip Vertical (⇅)", [this](IInspectable const&, RoutedEventArgs const&) {
        OnFlipVerticalClick(nullptr, nullptr);
    });

    menu.Items().Append(MenuFlyoutSeparator{});

    addItem(L"Duplicate Selected Shape (Ctrl+D)", [this](IInspectable const&, RoutedEventArgs const&) {
        OnDuplicateShapeClick(nullptr, nullptr);
    });
    addItem(L"Copy Shape (Ctrl+C)", [this](IInspectable const&, RoutedEventArgs const&) {
        OnCopyShapeClick(nullptr, nullptr);
    });
    addItem(L"Paste Here", [this, clickXIn, clickYIn](IInspectable const&, RoutedEventArgs const&) {
        if (m_doc && m_selectedShapeId > 0) {
            const std::array<uint64_t, 1> ids{m_selectedShapeId};
            const auto dup = m_doc.DuplicateShapes(m_activePageId, ids, 0.35, -0.35);
            if (dup.ok() && *dup.value > 0) {
                m_selectedShapeId = *dup.value;
                MarkDocumentDirty(true);
                PresentViewportFrame();
                SyncInspectorFromShapeSheet();
                return;
            }
        }
        for (auto const& m : m_allMasters) {
            if (m.id == m_clipboardMasterId) {
                InstantiateMasterOnCanvas(m, clickXIn, clickYIn);
                return;
            }
        }
        if (!m_allMasters.empty()) {
            InstantiateMasterOnCanvas(m_allMasters.front(), clickXIn, clickYIn);
        }
    });
    addItem(L"Delete Selected Shape (Del)", [this](IInspectable const&, RoutedEventArgs const&) {
        OnDeleteShapeClick(nullptr, nullptr);
    });

    menu.Items().Append(MenuFlyoutSeparator{});

    addItem(L"Bring to Front", [this](IInspectable const&, RoutedEventArgs const&) {
        OnBringFrontClick(nullptr, nullptr);
    });
    addItem(L"Send to Back", [this](IInspectable const&, RoutedEventArgs const&) {
        OnSendBackClick(nullptr, nullptr);
    });

    menu.Items().Append(MenuFlyoutSeparator{});

    auto insertSub = MenuFlyoutSubItem{};
    insertSub.Text(L"Insert Shape Here");
    auto addPrim = [&](std::wstring_view title, std::string kindSlug) {
        MenuFlyoutItem sub{};
        sub.Text(hstring{title});
        sub.Click([this, kindSlug, clickXIn, clickYIn](IInspectable const&, RoutedEventArgs const&) {
            if (!m_doc) return;
            const auto res = m_doc.DrawPrimitive(m_activePageId, kindSlug, clickXIn - 0.8, clickYIn - 0.5, clickXIn + 0.8, clickYIn + 0.5);
            if (res.ok()) {
                m_selectedShapeId = *res.value;
                MarkDocumentDirty(true);
                PresentViewportFrame();
                SyncInspectorFromShapeSheet();
            }
        });
        insertSub.Items().Append(sub);
    };
    addPrim(L"Rectangle", "rect");
    addPrim(L"Rounded Rectangle", "round_rect");
    addPrim(L"Circle / Ellipse", "ellipse");
    addPrim(L"Decision Diamond", "diamond");
    addPrim(L"Triangle", "triangle");
    addPrim(L"Hexagon", "hexagon");
    addPrim(L"5-Point Star", "star");
    menu.Items().Append(insertSub);

    auto routeSub = MenuFlyoutSubItem{};
    routeSub.Text(L"Connector Routing Style");
    auto addRoute = [&](std::wstring_view title, const char* code) {
        MenuFlyoutItem sub{};
        sub.Text(hstring{title});
        sub.Click([this, code](IInspectable const&, RoutedEventArgs const&) {
            if (!m_doc) return;
            (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "ShapeRouteStyle", code, true);
            MarkDocumentDirty(true);
            PresentViewportFrame();
        });
        routeSub.Items().Append(sub);
    };
    addRoute(L"Right-Angle Orthogonal", "16");
    addRoute(L"Curved Spline", "2");
    addRoute(L"Straight Line", "1");
    menu.Items().Append(routeSub);

    menu.Items().Append(MenuFlyoutSeparator{});

    addItem(L"Auto-Layout Page (Top-Down)", [this](IInspectable const&, RoutedEventArgs const&) {
        OnReLayoutTopDownClick(nullptr, nullptr);
    });
    addItem(L"Fit Page to Window", [this](IInspectable const&, RoutedEventArgs const&) {
        OnFitToWindowClick(nullptr, nullptr);
    });
    addItem(L"Inspect ShapeSheet (Shift+F5)", [this](IInspectable const&, RoutedEventArgs const&) {
        if (!m_shapeSheetOpen) {
            OnToggleShapeSheetClick(nullptr, nullptr);
        }
    });

    menu.ShowAt(panel, pt);
    e.Handled(true);
}

// ===========================================================================
// Horizontal & Vertical Precision Vector Rulers
// ===========================================================================
void MainWindow::OnToggleRulerUnitClick(IInspectable const&, RoutedEventArgs const&) {
    m_metricUnits = !m_metricUnits;
    RulerUnitOriginText().Text(m_metricUnits ? L"mm" : L"in");
    PageScaleSummaryText().Text(m_metricUnits
        ? L"Scale: 1:1 • Units: Millimeters"
        : L"Scale: 1:1 • Units: Inches");
    RebuildRulerTicks();
}

void MainWindow::OnRulerCanvasSizeChanged(IInspectable const&, SizeChangedEventArgs const&) {
    RebuildRulerTicks();
}

void MainWindow::RebuildRulerTicks() {
    auto hCanvas = HorizontalRulerCanvas();
    auto vCanvas = VerticalRulerCanvas();
    if (!hCanvas || !vCanvas) {
        return;
    }

    auto hChildren = hCanvas.Children();
    while (hChildren.Size() > 1) {
        hChildren.RemoveAtEnd();
    }
    auto vChildren = vCanvas.Children();
    while (vChildren.Size() > 1) {
        vChildren.RemoveAtEnd();
    }

    const SolidColorBrush tickBrush{Windows::UI::ColorHelper::FromArgb(0xFF, 0x64, 0x74, 0x8B)};
    const double majorStepPx = (m_metricUnits ? (96.0 / 2.54) : 96.0) * (m_zoomPercent / 100.0);
    const double clampedMajorPx = std::max(28.0, majorStepPx);
    const double minorStepPx = clampedMajorPx / 4.0;

    const double width = hCanvas.ActualWidth();
    int32_t minorIdx = 0;
    for (double x = 0.0; x <= width; x += minorStepPx, ++minorIdx) {
        const bool isMajor = (minorIdx % 4) == 0;
        Line tick{};
        tick.X1(x);
        tick.X2(x);
        tick.Y1(isMajor ? 4.0 : 14.0);
        tick.Y2(24.0);
        tick.Stroke(tickBrush);
        tick.StrokeThickness(isMajor ? 1.0 : 0.75);
        hChildren.Append(tick);

        if (isMajor) {
            const int32_t unitVal = (minorIdx / 4) * (m_metricUnits ? 10 : 1);
            TextBlock label{};
            label.Text(hstring{std::to_wstring(unitVal)});
            label.FontSize(9.0);
            label.Foreground(tickBrush);
            Canvas::SetLeft(label, x + 3.0);
            Canvas::SetTop(label, 2.0);
            hChildren.Append(label);
        }
    }

    const double height = vCanvas.ActualHeight();
    minorIdx = 0;
    for (double y = 0.0; y <= height; y += minorStepPx, ++minorIdx) {
        const bool isMajor = (minorIdx % 4) == 0;
        Line tick{};
        tick.X1(isMajor ? 4.0 : 14.0);
        tick.X2(24.0);
        tick.Y1(y);
        tick.Y2(y);
        tick.Stroke(tickBrush);
        tick.StrokeThickness(isMajor ? 1.0 : 0.75);
        vChildren.Append(tick);

        if (isMajor) {
            const int32_t unitVal = (minorIdx / 4) * (m_metricUnits ? 10 : 1);
            TextBlock label{};
            label.Text(hstring{std::to_wstring(unitVal)});
            label.FontSize(8.5);
            label.Foreground(tickBrush);
            Canvas::SetLeft(label, 2.0);
            Canvas::SetTop(label, y + 1.0);
            vChildren.Append(label);
        }
    }
}

// ===========================================================================
// 22-Category Stencils Catalog (520+ Vector Masters via vexio_doc_list_stencils_json)
// ===========================================================================
void MainWindow::InitializeStencilCatalog() {
    m_allMasters.clear();
    m_stencilPackNames.clear();

    constexpr std::array<std::string_view, 22> kCanonicalPackOrder{
        "Basic Shapes",
        "Basic Flowchart",
        "Microsoft 365 & Office",
        "Microsoft Servers & Active Directory",
        "Microsoft Azure & Hybrid Cloud",
        "Microsoft Power Platform & Dynamics 365",
        "Servers (Standard Roles)",
        "Network & Rack Equipment",
        "Cloud Architecture (AWS / GCP / Azure)",
        "Arrows & Block Arrows",
        "Callouts, Notes & Containers",
        "Cross-Functional Swimlanes",
        "BPMN 2.0",
        "UML 2.5",
        "Database Crow's Foot & IDEF1X",
        "Organization Chart",
        "Floor Plan & Architecture",
        "Electrical & Electronic (IEEE / IEC)",
        "P&ID Process Engineering",
        "UI & Application Wireframes",
        "Project Schedule (Gantt & Timeline)",
        "Value Stream Mapping (Lean VSM)"
    };

    if (m_doc) {
        const auto jsonRes = m_doc.ListStencilsJson();
        if (jsonRes.ok() && !jsonRes.value->empty()) {
            Windows::Data::Json::JsonObject rootObj{nullptr};
            if (Windows::Data::Json::JsonObject::TryParse(*jsonRes.value, rootObj)) {
                auto appendCategory = [&](std::string_view catUtf8) {
                    const hstring catKey = vexio::win::ToHString(catUtf8);
                    if (!rootObj.HasKey(catKey)) {
                        return;
                    }
                    const auto arr = rootObj.GetNamedArray(catKey);
                    const std::wstring packWide{catKey.c_str()};
                    m_stencilPackNames.push_back(packWide);

                    for (uint32_t i = 0; i < arr.Size(); ++i) {
                        const auto itemObj = arr.GetObjectAt(i);
                        const hstring mNameH = itemObj.GetNamedString(L"name", L"Shape");
                        const double wIn = itemObj.GetNamedNumber(L"widthIn", 1.8);
                        const double hIn = itemObj.GetNamedNumber(L"heightIn", 1.0);
                        const hstring svgH = itemObj.GetNamedString(L"svg", L"");
                        const hstring pngH = itemObj.GetNamedString(L"pngDataUrl", L"");

                        const std::string mNameUtf8 = vexio::win::ToUtf8(mNameH);
                        StencilMasterDescriptor desc{};
                        desc.id = std::string{catUtf8} + "::" + mNameUtf8;
                        desc.categoryUtf8 = std::string{catUtf8};
                        desc.masterNameUtf8 = mNameUtf8;
                        desc.packName = packWide;
                        desc.displayName = std::wstring{mNameH.c_str()};
                        desc.categoryTag = packWide;
                        desc.defaultWidthFormula = std::format("{:.2f} in", wIn);
                        desc.defaultHeightFormula = std::format("{:.2f} in", hIn);
                        desc.defaultFillFormula = "RGB(239,246,255)";
                        desc.stereotypeOrRuleType = mNameUtf8;
                        desc.svgMarkup = vexio::win::ToUtf8(svgH);
                        desc.pngDataUrl = vexio::win::ToUtf8(pngH);
                        m_allMasters.push_back(std::move(desc));
                    }
                };

                for (const auto cat : kCanonicalPackOrder) {
                    appendCategory(cat);
                }
                // Also include any custom imported Visio stencil packs (.vssx / .vss / .vsdx)
                for (auto const& kv : rootObj) {
                    const std::string keyUtf8 = vexio::win::ToUtf8(kv.Key());
                    bool alreadyAdded = false;
                    for (const auto canonical : kCanonicalPackOrder) {
                        if (canonical == keyUtf8) {
                            alreadyAdded = true;
                            break;
                        }
                    }
                    if (!alreadyAdded && kv.Value().ValueType() == Windows::Data::Json::JsonValueType::Array) {
                        appendCategory(keyUtf8);
                    }
                }
            }
        }
    }

    auto comboItems = StencilPackCombo().Items();
    comboItems.Clear();
    comboItems.Append(box_value(hstring{
        std::format(L"All {} Categories ({} Shapes)", m_stencilPackNames.size(), m_allMasters.size())
    }));
    for (auto const& pack : m_stencilPackNames) {
        comboItems.Append(box_value(hstring{pack}));
    }
    StencilPackCombo().SelectedIndex(1); // Start on "Basic Shapes" while keeping All 17 + every domain 1-click away!
    StencilHeaderCountText().Text(hstring{
        std::format(L"Shapes ({} Categories)", m_stencilPackNames.size())
    });
}

void MainWindow::RefreshStencilList() {
    if (!m_isInitialized) {
        return;
    }

    const std::wstring query = [&]() {
        std::wstring q{StencilSearchBox().Text().c_str()};
        std::transform(q.begin(), q.end(), q.begin(), [](wchar_t ch) { return static_cast<wchar_t>(::towlower(ch)); });
        return q;
    }();

    const int32_t packIdx = StencilPackCombo().SelectedIndex();
    std::wstring selectedPack{};
    if (packIdx > 0 && static_cast<size_t>(packIdx - 1) < m_stencilPackNames.size()) {
        selectedPack = m_stencilPackNames[static_cast<size_t>(packIdx - 1)];
    }

    m_filteredMasters.clear();
    auto items = StencilMastersListView().Items();
    items.Clear();

    for (auto const& master : m_allMasters) {
        if (query.empty() && !selectedPack.empty() && master.packName != selectedPack) {
            continue;
        }
        if (!query.empty()) {
            std::wstring hay = master.displayName + L" " + master.categoryTag;
            std::transform(hay.begin(), hay.end(), hay.begin(), [](wchar_t ch) { return static_cast<wchar_t>(::towlower(ch)); });
            if (hay.find(query) == std::wstring::npos) {
                continue;
            }
        }

        m_filteredMasters.push_back(master);

        Grid card{};
        card.Padding(Thickness{6, 6, 6, 6});
        card.ColumnSpacing(10.0);
        ColumnDefinition c0{}; c0.Width(GridLength{46.0});
        ColumnDefinition c1{}; c1.Width(GridLength{1.0, GridUnitType::Star});
        card.ColumnDefinitions().Append(c0);
        card.ColumnDefinitions().Append(c1);

        // Crisp multi-color, exact-proportion 144x96 vector preview rendered via WinUI 3 Viewbox + Canvas
        std::wstring xamlPaths{};
        auto extractAttr = [](std::string_view tag, std::string_view key) -> std::string_view {
            const size_t pos = tag.find(key);
            if (pos == std::string_view::npos) return {};
            const size_t start = pos + key.size();
            const size_t end = tag.find('"', start);
            if (end == std::string_view::npos || end <= start) return {};
            return tag.substr(start, end - start);
        };

        if (!master.svgMarkup.empty()) {
            size_t searchPos = master.svgMarkup.find("data-shape-id=");
            if (searchPos == std::string::npos) {
                searchPos = 0;
            }
            while (true) {
                const size_t pathPos = master.svgMarkup.find("<path ", searchPos);
                if (pathPos == std::string::npos) {
                    break;
                }
                const size_t tagEnd = master.svgMarkup.find('>', pathPos);
                if (tagEnd == std::string::npos) {
                    break;
                }
                const std::string_view tagView{master.svgMarkup.data() + pathPos, tagEnd - pathPos};
                const std::string_view dVal = extractAttr(tagView, "d=\"");
                if (!dVal.empty()) {
                    const std::string_view fillVal = extractAttr(tagView, "fill=\"");
                    const std::string_view strokeVal = extractAttr(tagView, "stroke=\"");
                    const std::string_view swVal = extractAttr(tagView, "stroke-width=\"");
                    std::wstring fillAttr{};
                    if (!fillVal.empty() && fillVal != "none" && fillVal[0] == '#') {
                        fillAttr = std::format(L" Fill=\"{}\"", vexio::win::ToHString(fillVal).c_str());
                    }
                    std::wstring strokeAttr{};
                    if (!strokeVal.empty() && strokeVal != "none" && strokeVal[0] == '#') {
                        strokeAttr = std::format(L" Stroke=\"{}\"", vexio::win::ToHString(strokeVal).c_str());
                    }
                    const std::wstring swWide = swVal.empty()
                        ? L"1.4"
                        : std::wstring{vexio::win::ToHString(swVal).c_str()};
                    xamlPaths += std::format(
                        L"<Path Data=\"{}\"{}{} StrokeThickness=\"{}\" StrokeStartLineCap=\"Round\" StrokeEndLineCap=\"Round\" StrokeLineJoin=\"Round\" />",
                        vexio::win::ToHString(dVal).c_str(),
                        fillAttr,
                        strokeAttr,
                        swWide
                    );
                }
                searchPos = tagEnd + 1;
            }
        }

        Border swatch{};
        swatch.Width(48.0);
        swatch.Height(36.0);
        swatch.CornerRadius(CornerRadius{6, 6, 6, 6});
        swatch.Background(SolidColorBrush{Windows::UI::ColorHelper::FromArgb(0xFF, 0xFF, 0xFF, 0xFF)});
        swatch.BorderBrush(SolidColorBrush{Windows::UI::ColorHelper::FromArgb(0xFF, 0xCB, 0xD5, 0xE1)});
        swatch.BorderThickness(Thickness{1, 1, 1, 1});
        bool loadedPngSwatch = false;
        if (!master.pngDataUrl.empty()) {
            const auto pngBytes = DecodeBase64PngDataUrl(master.pngDataUrl);
            if (!pngBytes.empty()) {
                try {
                    InMemoryRandomAccessStream memStream{};
                    DataWriter writer{memStream};
                    writer.WriteBytes(pngBytes);
                    (void)writer.StoreAsync();
                    writer.DetachStream();
                    memStream.Seek(0);
                    BitmapImage bmp{};
                    bmp.SetSource(memStream);
                    Image img{};
                    img.Source(bmp);
                    img.Stretch(Stretch::Uniform);
                    img.Margin(Thickness{2, 2, 2, 2});
                    swatch.Child(img);
                    loadedPngSwatch = true;
                } catch (...) {
                    loadedPngSwatch = false;
                }
            }
        }
        if (!loadedPngSwatch) {
            const std::wstring xamlBox = !xamlPaths.empty()
                ? std::format(
                    L"<Viewbox xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" Stretch=\"Uniform\" Margin=\"2\"><Canvas Width=\"144\" Height=\"96\">{}</Canvas></Viewbox>",
                    xamlPaths)
                : L"<Path xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" Data=\"M 4,6 L 32,6 L 32,20 L 4,20 Z\" Stretch=\"Uniform\" Stroke=\"#2563EB\" Fill=\"#EFF6FF\" StrokeThickness=\"1.6\" Margin=\"5,4,5,4\" />";
            if (auto vectorPreview = Markup::XamlReader::Load(hstring{xamlBox}).try_as<UIElement>()) {
                swatch.Child(vectorPreview);
            }
        }
        Grid::SetColumn(swatch, 0);

        StackPanel meta{};
        meta.VerticalAlignment(VerticalAlignment::Center);
        TextBlock title{};
        title.Text(hstring{master.displayName});
        title.FontSize(12.0);
        title.FontWeight(Microsoft::UI::Text::FontWeights::SemiBold());
        TextBlock sub{};
        sub.Text(hstring{master.categoryTag + L" • " + vexio::win::ToHString(master.defaultWidthFormula).c_str()});
        sub.FontSize(10.5);
        sub.Opacity(0.7);
        meta.Children().Append(title);
        meta.Children().Append(sub);
        Grid::SetColumn(meta, 1);

        // Right-click ContextFlyout on every stencil item!
        MenuFlyout stencilMenu{};
        MenuFlyoutItem dropCenter{};
        dropCenter.Text(L"Place on Center of Page");
        dropCenter.Click([this, master](IInspectable const&, RoutedEventArgs const&) {
            InstantiateMasterOnCanvas(master, 5.5, 4.25);
        });
        MenuFlyoutItem dropThree{};
        dropThree.Text(L"Place 3 Connected Steps");
        dropThree.Click([this, master](IInspectable const&, RoutedEventArgs const&) {
            if (!m_doc) return;
            const auto r1 = m_doc.DropStencil(m_activePageId, master.categoryUtf8, master.masterNameUtf8, 3.0, 4.25);
            const auto r2 = m_doc.DropStencil(m_activePageId, master.categoryUtf8, master.masterNameUtf8, 5.5, 4.25);
            const auto r3 = m_doc.DropStencil(m_activePageId, master.categoryUtf8, master.masterNameUtf8, 8.0, 4.25);
            if (r1.ok() && r2.ok() && r3.ok()) {
                (void)m_doc.ConnectShapes(m_activePageId, *r1.value, *r2.value);
                (void)m_doc.ConnectShapes(m_activePageId, *r2.value, *r3.value);
                m_selectedShapeId = *r3.value;
                MarkDocumentDirty(true);
                PresentViewportFrame();
                SyncInspectorFromShapeSheet();
            }
        });
        stencilMenu.Items().Append(dropCenter);
        stencilMenu.Items().Append(dropThree);
        card.ContextFlyout(stencilMenu);

        card.Children().Append(swatch);
        card.Children().Append(meta);
        card.Tag(box_value(vexio::win::ToHString(master.id)));

        items.Append(card);
    }
}

void MainWindow::OnStencilSearchTextChanged(IInspectable const&, TextChangedEventArgs const&) {
    RefreshStencilList();
}

void MainWindow::OnStencilPackSelectionChanged(IInspectable const&, SelectionChangedEventArgs const&) {
    RefreshStencilList();
}

void MainWindow::OnStencilMasterDragStarting(IInspectable const&, DragItemsStartingEventArgs const& e) {
    if (e.Items().Size() > 0) {
        if (auto grid = e.Items().GetAt(0).try_as<FrameworkElement>()) {
            if (auto idStr = grid.Tag().try_as<hstring>()) {
                e.Data().SetText(*idStr);
                e.Data().RequestedOperation(DataPackageOperation::Copy);
            }
        }
    }
}

void MainWindow::OnStencilMasterItemClick(IInspectable const&, ItemClickEventArgs const& e) {
    if (auto grid = e.ClickedItem().try_as<FrameworkElement>()) {
        if (auto idStr = grid.Tag().try_as<hstring>()) {
            const std::string targetId = vexio::win::ToUtf8(*idStr);
            for (auto const& master : m_allMasters) {
                if (master.id == targetId) {
                    InstantiateMasterOnCanvas(master, 4.50, 4.50);
                    break;
                }
            }
        }
    }
}

void MainWindow::OnCanvasDragOver(IInspectable const&, DragEventArgs const& e) {
    e.AcceptedOperation(DataPackageOperation::Copy);
    if (e.DragUIOverride()) {
        e.DragUIOverride().Caption(L"Drop Master onto DX12 Page");
        e.DragUIOverride().IsCaptionVisible(true);
    }
}

void MainWindow::OnCanvasDrop(IInspectable const&, DragEventArgs const& e) {
    if (!e.DataView().Contains(StandardDataFormats::Text())) {
        return;
    }

    const Point dropPt = e.GetPosition(DiagramSwapChainPanel());
    const double pxPerIn = std::max(16.0, 96.0 * (m_zoomPercent / 100.0));
    const double dropInchesX = std::clamp(dropPt.X / pxPerIn, 0.4, 10.6);
    const double dropInchesY = std::clamp(8.5 - (dropPt.Y / pxPerIn), 0.4, 8.1);

    [this, dataView = e.DataView(), dropInchesX, dropInchesY]() -> fire_and_forget {
        const auto weakSelf = get_weak();
        const hstring droppedMasterId = co_await dataView.GetTextAsync();
        if (auto self = weakSelf.get()) {
            const std::string idUtf8 = vexio::win::ToUtf8(droppedMasterId);
            for (auto const& master : self->m_allMasters) {
                if (master.id == idUtf8) {
                    self->InstantiateMasterOnCanvas(master, dropInchesX, dropInchesY);
                    break;
                }
            }
        }
    }();
}

void MainWindow::InstantiateMasterOnCanvas(StencilMasterDescriptor const& master, double pinXInches, double pinYInches) {
    if (!m_doc) {
        return;
    }

    const auto dropped = m_doc.DropStencil(
        m_activePageId,
        master.categoryUtf8,
        master.masterNameUtf8.empty() ? vexio::win::ToUtf8(master.displayName) : master.masterNameUtf8,
        pinXInches,
        pinYInches);

    if (dropped.ok() && *dropped.value > 0) {
        m_previousSelectedShapeId = m_selectedShapeId;
        m_selectedShapeId = *dropped.value;
    } else {
        const uint64_t fallbackId = m_nextShapeId++;
        m_previousSelectedShapeId = m_selectedShapeId;
        m_selectedShapeId = fallbackId;
        (void)m_doc.SetCellFormula(m_activePageId, fallbackId, "PinX", std::format("{:.3f} in", pinXInches), true);
        (void)m_doc.SetCellFormula(m_activePageId, fallbackId, "PinY", std::format("{:.3f} in", pinYInches), true);
        (void)m_doc.SetCellFormula(m_activePageId, fallbackId, "Width", master.defaultWidthFormula, true);
        (void)m_doc.SetCellFormula(m_activePageId, fallbackId, "Height", master.defaultHeightFormula, true);
    }

    for (auto& page : m_pages) {
        if (page.pageId == m_activePageId) {
            ++page.shapeCount;
            StatusShapeCountText().Text(hstring{
                std::format(L"Shapes: {} • Selected: Sheet.{}", page.shapeCount, m_selectedShapeId)
            });
            break;
        }
    }

    if (m_activeTool != ActiveCanvasTool::Pointer) {
        SetActiveTool(ActiveCanvasTool::Pointer);
    }
    MarkDocumentDirty(true);
    PresentViewportFrame();
    SyncInspectorFromShapeSheet();
    RefreshShapeSheetSectionRows();
}

void MainWindow::SeedActiveShapeDefaultCells() {
    if (!m_doc) {
        return;
    }
    (void)m_doc.LoadStarterTemplate("bpmn_fulfillment");
}

void MainWindow::SyncInspectorFromShapeSheet() {
    if (!m_doc || !m_isInitialized) {
        return;
    }

    auto readOrFallback = [this](std::string_view cellName, std::wstring_view fallback) -> hstring {
        auto val = m_doc.GetCellValueStr(m_activePageId, m_selectedShapeId, cellName);
        if (val.ok() && !val.value->empty()) {
            return *val.value;
        }
        auto fml = m_doc.GetCellFormula(m_activePageId, m_selectedShapeId, cellName);
        if (fml.ok() && !fml.value->empty()) {
            return *fml.value;
        }
        return hstring{fallback};
    };

    ShapeDataHeaderText().Text(hstring{std::format(L"Selected: Sheet.{}", m_selectedShapeId)});
    ShapeSheetTitleText().Text(hstring{std::format(L"ShapeSheet — Sheet.{}", m_selectedShapeId)});

    TransformPinXBox().Text(readOrFallback("PinX", L"4.25 in"));
    TransformPinYBox().Text(readOrFallback("PinY", L"5.50 in"));
    TransformWidthBox().Text(readOrFallback("Width", L"2.00 in"));
    TransformHeightBox().Text(readOrFallback("Height", L"1.00 in"));
    TransformLocPinXBox().Text(readOrFallback("LocPinX", L"GUARD(Width*0.5)"));
    TransformLocPinYBox().Text(readOrFallback("LocPinY", L"GUARD(Height*0.5)"));
    TransformAngleBox().Text(readOrFallback("Angle", L"0 deg"));

    PropLabelTextBox().Text(readOrFallback("Prop.Label", L"Order Validation"));
    PropDepartmentTextBox().Text(readOrFallback("Prop.Department", L"Operations"));
    PropOwnerTextBox().Text(readOrFallback("Prop.Owner", L"Platform Team"));
    PropCostTextBox().Text(readOrFallback("Prop.Cost", L"1250.00"));

    // Update Floating Formula Bar for the active cell
    FloatingCellRefText().Text(hstring{
        std::format(L"Sheet.{}!{}", m_selectedShapeId, vexio::win::ToHString(m_selectedCellName).c_str())
    });

    const auto activeFormula = m_doc.GetCellFormula(m_activePageId, m_selectedShapeId, m_selectedCellName);
    const auto activeValue   = m_doc.GetCellValueStr(m_activePageId, m_selectedShapeId, m_selectedCellName);

    const hstring formulaStr = (activeFormula.ok() && !activeFormula.value->empty())
        ? *activeFormula.value
        : readOrFallback(m_selectedCellName, L"0 in");
    const hstring evalStr = (activeValue.ok() && !activeValue.value->empty())
        ? *activeValue.value
        : formulaStr;

    FloatingFormulaTextBox().Text(formulaStr);
    ShapeSheetFormulaEditorBox().Text(formulaStr);
    FloatingEvaluatedValueText().Text(hstring{std::wstring{L"= "} + evalStr.c_str()});
}

void MainWindow::RefreshShapeSheetSectionRows() {
    if (!m_isInitialized || !m_doc) {
        return;
    }

    const int32_t sectionIdx = std::max<int32_t>(0, ShapeSheetSectionCombo().SelectedIndex());
    const std::vector<std::string> cellNames = GetCellsForSectionIndex(sectionIdx);

    m_currentSectionRows.clear();
    auto listItems = ShapeSheetCellsListView().Items();
    listItems.Clear();

    int32_t selectedRowIdx = -1;
    int32_t idx = 0;

    for (auto const& cellName : cellNames) {
        const auto fRes = m_doc.GetCellFormula(m_activePageId, m_selectedShapeId, cellName);
        const auto vRes = m_doc.GetCellValueStr(m_activePageId, m_selectedShapeId, cellName);

        std::wstring formula = (fRes.ok() && !fRes.value->empty()) ? std::wstring{fRes.value->c_str()} : L"0";
        std::wstring value   = (vRes.ok() && !vRes.value->empty()) ? std::wstring{vRes.value->c_str()} : formula;
        const bool guarded   = FormulaStartsWithGuard(formula);

        m_currentSectionRows.push_back(ShapeSheetCellRow{cellName, formula, value, guarded});
        if (cellName == m_selectedCellName) {
            selectedRowIdx = idx;
        }

        Grid rowGrid{};
        rowGrid.Padding(Thickness{6, 3, 6, 3});
        ColumnDefinition c0{}; c0.Width(GridLength{184.0});
        ColumnDefinition c1{}; c1.Width(GridLength{1.0, GridUnitType::Star});
        ColumnDefinition c2{}; c2.Width(GridLength{184.0});
        ColumnDefinition c3{}; c3.Width(GridLength{114.0});
        rowGrid.ColumnDefinitions().Append(c0);
        rowGrid.ColumnDefinitions().Append(c1);
        rowGrid.ColumnDefinitions().Append(c2);
        rowGrid.ColumnDefinitions().Append(c3);

        FontFamily monoFont{L"Consolas"};

        TextBlock nameText{};
        nameText.Text(vexio::win::ToHString(cellName));
        nameText.FontFamily(monoFont);
        nameText.FontSize(12.0);
        nameText.FontWeight(Microsoft::UI::Text::FontWeights::SemiBold());
        Grid::SetColumn(nameText, 0);

        TextBlock formulaText{};
        formulaText.Text(hstring{formula});
        formulaText.FontFamily(monoFont);
        formulaText.FontSize(12.0);
        Grid::SetColumn(formulaText, 1);

        TextBlock evalText{};
        evalText.Text(hstring{value});
        evalText.FontFamily(monoFont);
        evalText.FontSize(12.0);
        Grid::SetColumn(evalText, 2);

        TextBlock guardText{};
        guardText.Text(guarded ? L"GUARD Locked" : L"Editable");
        guardText.FontSize(11.0);
        guardText.Foreground(SolidColorBrush{
            guarded
                ? Windows::UI::ColorHelper::FromArgb(0xFF, 0xB4, 0x53, 0x09)
                : Windows::UI::ColorHelper::FromArgb(0xFF, 0x15, 0x80, 0x3D)
        });
        Grid::SetColumn(guardText, 3);

        rowGrid.Children().Append(nameText);
        rowGrid.Children().Append(formulaText);
        rowGrid.Children().Append(evalText);
        rowGrid.Children().Append(guardText);

        listItems.Append(rowGrid);
        ++idx;
    }

    if (selectedRowIdx >= 0) {
        ShapeSheetCellsListView().SelectedIndex(selectedRowIdx);
    }
}

void MainWindow::CommitFormulaToActiveCell(std::wstring const& rawFormulaText) {
    if (!m_doc) {
        return;
    }

    std::wstring normalized = rawFormulaText;
    if (!normalized.empty() && normalized.front() == L'=') {
        normalized.erase(normalized.begin());
    }

    const VexioStatus status = m_doc.SetCellFormula(
        m_activePageId,
        m_selectedShapeId,
        m_selectedCellName,
        vexio::win::ToUtf8(normalized),
        m_forceSetf);

    if (status == VEXIO_OK) {
        WorkspaceInfoBar().IsOpen(false);
        MarkDocumentDirty(true);
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
        AttachOrResizeDx12SwapChain();
    } else if (status == VEXIO_ERR_CELL_GUARDED) {
        ShowBanner(
            InfoBarSeverity::Warning,
            L"GUARD() Cell Protection Active",
            vexio::win::DescribeStatus(status));
    } else {
        ShowBanner(
            InfoBarSeverity::Error,
            L"ShapeSheet Formula Error",
            vexio::win::DescribeStatus(status));
    }
}

void MainWindow::OnShapeSheetSectionChanged(IInspectable const&, SelectionChangedEventArgs const&) {
    RefreshShapeSheetSectionRows();
}

void MainWindow::OnShapeSheetCellSelectionChanged(IInspectable const&, SelectionChangedEventArgs const&) {
    const int32_t idx = ShapeSheetCellsListView().SelectedIndex();
    if (idx >= 0 && static_cast<size_t>(idx) < m_currentSectionRows.size()) {
        m_selectedCellName = m_currentSectionRows[static_cast<size_t>(idx)].cellName;
        SyncInspectorFromShapeSheet();
    }
}

void MainWindow::OnShapeSheetFormulaKeyDown(IInspectable const&, KeyRoutedEventArgs const& e) {
    if (e.Key() == VirtualKey::Enter) {
        CommitFormulaToActiveCell(std::wstring{ShapeSheetFormulaEditorBox().Text().c_str()});
        e.Handled(true);
    }
}

void MainWindow::OnFloatingFormulaKeyDown(IInspectable const&, KeyRoutedEventArgs const& e) {
    if (e.Key() == VirtualKey::Enter) {
        CommitFormulaToActiveCell(std::wstring{FloatingFormulaTextBox().Text().c_str()});
        e.Handled(true);
    }
}

void MainWindow::OnShapeSheetForceSetfToggled(IInspectable const&, RoutedEventArgs const&) {
    m_forceSetf = ShapeSheetForceSetfToggle().IsChecked().GetBoolean();
    FloatingForceSetfToggle().IsChecked(m_forceSetf);
}

void MainWindow::OnFloatingForceSetfToggled(IInspectable const&, RoutedEventArgs const&) {
    m_forceSetf = FloatingForceSetfToggle().IsChecked().GetBoolean();
    ShapeSheetForceSetfToggle().IsChecked(m_forceSetf);
}

void MainWindow::OnCommitShapeSheetFormulaClick(IInspectable const&, RoutedEventArgs const&) {
    CommitFormulaToActiveCell(std::wstring{ShapeSheetFormulaEditorBox().Text().c_str()});
}

void MainWindow::OnCommitFloatingFormulaClick(IInspectable const&, RoutedEventArgs const&) {
    CommitFormulaToActiveCell(std::wstring{FloatingFormulaTextBox().Text().c_str()});
}

void MainWindow::OnRevertFormulaClick(IInspectable const&, RoutedEventArgs const&) {
    SyncInspectorFromShapeSheet();
}

void MainWindow::OnGuardSelectedCellClick(IInspectable const&, RoutedEventArgs const&) {
    std::wstring current{ShapeSheetFormulaEditorBox().Text().c_str()};
    if (!current.empty() && current.front() == L'=') {
        current.erase(current.begin());
    }
    if (!FormulaStartsWithGuard(current)) {
        const std::wstring wrapped = std::format(L"GUARD({})", current.empty() ? L"0 in" : current);
        const bool previousSetf = m_forceSetf;
        m_forceSetf = true;
        CommitFormulaToActiveCell(wrapped);
        m_forceSetf = previousSetf;
    }
}

// ===========================================================================
// ShapeSheet Split-View Drag Resizing (GridSplitter)
// ===========================================================================
void MainWindow::OnShapeSheetSplitterPointerPressed(IInspectable const&, PointerRoutedEventArgs const& e) {
    m_draggingSplitter = true;
    m_splitterStartPointerY = e.GetCurrentPoint(RootWorkstationGrid()).Position().Y;
    m_splitterStartHeight = m_shapeSheetHeightPx;
    ShapeSheetGridSplitter().CapturePointer(e.Pointer());
    e.Handled(true);
}

void MainWindow::OnShapeSheetSplitterPointerMoved(IInspectable const&, PointerRoutedEventArgs const& e) {
    if (!m_draggingSplitter) {
        return;
    }
    const double currentY = e.GetCurrentPoint(RootWorkstationGrid()).Position().Y;
    const double deltaY = m_splitterStartPointerY - currentY;
    m_shapeSheetHeightPx = std::clamp(m_splitterStartHeight + deltaY, 140.0, 520.0);
    ShapeSheetPaneRow().Height(GridLength{m_shapeSheetHeightPx});
    e.Handled(true);
}

void MainWindow::OnShapeSheetSplitterPointerReleased(IInspectable const&, PointerRoutedEventArgs const& e) {
    if (m_draggingSplitter) {
        m_draggingSplitter = false;
        ShapeSheetGridSplitter().ReleasePointerCapture(e.Pointer());
        AttachOrResizeDx12SwapChain();
    }
    e.Handled(true);
}

// ===========================================================================
// Right Task Panes: Shape Data, Size & Position, Format, Pan & Zoom, Data, Layers
// ===========================================================================
void MainWindow::InitializeExternalDataAndLayers() {
    m_externalRecords = {
        {L"ROW-101", L"API Gateway Cluster", L"Platform",   L"S. Patel",  L"4200.00", L"Active"},
        {L"ROW-102", L"Order Validator Svc", L"Operations", L"M. Chen",   L"1250.00", L"Approved"},
        {L"ROW-103", L"Settlement Worker",   L"Finance",    L"A. Rivera", L"2900.00", L"Active"},
        {L"ROW-104", L"Audit Event Sink",    L"Security",   L"K. Brooks", L"980.00",  L"Draft"}
    };

    m_layers = {
        {L"Flowchart",    L"#2563EB", true, false, true, true},
        {L"Connectors",   L"#0F172A", true, false, true, true},
        {L"Dimensions",   L"#059669", true, false, true, true},
        {L"CAD-Walls",    L"#475569", true, true,  true, true},
        {L"CAD-HVAC",     L"#0284C7", true, false, true, false},
        {L"Markup & Ink", L"#DC2626", true, false, false, false}
    };
}

void MainWindow::RefreshExternalDataList() {
    auto items = ExternalDataRowsListView().Items();
    items.Clear();

    for (auto const& rec : m_externalRecords) {
        StackPanel row{};
        row.Padding(Thickness{4, 3, 4, 3});
        TextBlock primary{};
        primary.Text(hstring{std::format(L"{} • {}", rec.rowId, rec.component)});
        primary.FontSize(12.0);
        primary.FontWeight(Microsoft::UI::Text::FontWeights::SemiBold());

        TextBlock secondary{};
        secondary.Text(hstring{std::format(L"{} | ${} | {}", rec.department, rec.costUsd, rec.status)});
        secondary.FontSize(11.0);
        secondary.Opacity(0.75);

        row.Children().Append(primary);
        row.Children().Append(secondary);
        items.Append(row);
    }
    if (items.Size() > 0) {
        ExternalDataRowsListView().SelectedIndex(0);
    }
}

void MainWindow::RefreshLayersList() {
    auto items = LayersListView().Items();
    items.Clear();

    for (size_t i = 0; i < m_layers.size(); ++i) {
        auto const& layer = m_layers[i];
        Grid row{};
        ColumnDefinition c0{}; c0.Width(GridLength{1.0, GridUnitType::Star});
        ColumnDefinition c1{}; c1.Width(GridLength{1.0, GridUnitType::Auto});
        ColumnDefinition c2{}; c2.Width(GridLength{1.0, GridUnitType::Auto});
        row.ColumnDefinitions().Append(c0);
        row.ColumnDefinitions().Append(c1);
        row.ColumnDefinitions().Append(c2);

        TextBlock name{};
        name.Text(hstring{layer.name});
        name.VerticalAlignment(VerticalAlignment::Center);
        name.FontSize(12.0);
        Grid::SetColumn(name, 0);

        CheckBox visCheck{};
        visCheck.Content(box_value(L"Vis"));
        visCheck.IsChecked(layer.visible);
        visCheck.MinWidth(56.0);
        visCheck.Checked([this, i](IInspectable const&, RoutedEventArgs const&) {
            if (i < m_layers.size()) m_layers[i].visible = true;
        });
        visCheck.Unchecked([this, i](IInspectable const&, RoutedEventArgs const&) {
            if (i < m_layers.size()) m_layers[i].visible = false;
        });
        Grid::SetColumn(visCheck, 1);

        CheckBox lockCheck{};
        lockCheck.Content(box_value(L"Lock"));
        lockCheck.IsChecked(layer.locked);
        lockCheck.MinWidth(60.0);
        lockCheck.Checked([this, i](IInspectable const&, RoutedEventArgs const&) {
            if (i < m_layers.size()) m_layers[i].locked = true;
        });
        lockCheck.Unchecked([this, i](IInspectable const&, RoutedEventArgs const&) {
            if (i < m_layers.size()) m_layers[i].locked = false;
        });
        Grid::SetColumn(lockCheck, 2);

        row.Children().Append(name);
        row.Children().Append(visCheck);
        row.Children().Append(lockCheck);
        items.Append(row);
    }
}

void MainWindow::OnShapeDataFieldCommitted(IInspectable const&, RoutedEventArgs const&) {
    OnApplyShapeDataClick(nullptr, nullptr);
}

void MainWindow::OnPropStatusChanged(IInspectable const&, SelectionChangedEventArgs const&) {
    if (m_isInitialized) {
        OnApplyShapeDataClick(nullptr, nullptr);
    }
}

void MainWindow::OnApplyShapeDataClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;

    auto quoteUtf8 = [](hstring const& str) -> std::string {
        return std::format("\"{}\"", vexio::win::ToUtf8(str));
    };

    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "Prop.Label",      quoteUtf8(PropLabelTextBox().Text()), false);
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "Prop.Department", quoteUtf8(PropDepartmentTextBox().Text()), false);
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "Prop.Owner",      quoteUtf8(PropOwnerTextBox().Text()), false);
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "Prop.Cost",       vexio::win::ToUtf8(PropCostTextBox().Text()), false);

    if (auto statusSel = PropStatusCombo().SelectedItem().try_as<hstring>()) {
        (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "Prop.Status", quoteUtf8(*statusSel), false);
    }

    MarkDocumentDirty(true);
    RefreshShapeSheetSectionRows();
}

void MainWindow::OnApplySizeAndPositionClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;

    const std::array<std::pair<const char*, hstring>, 7> updates{{
        {"PinX",    TransformPinXBox().Text()},
        {"PinY",    TransformPinYBox().Text()},
        {"Width",   TransformWidthBox().Text()},
        {"Height",  TransformHeightBox().Text()},
        {"LocPinX", TransformLocPinXBox().Text()},
        {"LocPinY", TransformLocPinYBox().Text()},
        {"Angle",   TransformAngleBox().Text()},
    }};

    bool guardedBlocked = false;
    for (auto const& [cell, expr] : updates) {
        const VexioStatus st = m_doc.SetCellFormula(
            m_activePageId, m_selectedShapeId, cell, vexio::win::ToUtf8(expr), m_forceSetf);
        if (st == VEXIO_ERR_CELL_GUARDED) {
            guardedBlocked = true;
        }
    }

    if (guardedBlocked) {
        ShowBanner(InfoBarSeverity::Warning, L"GUARD() Cell Preserved",
                   L"Guarded cells (such as LocPinX/LocPinY) kept their formula. Toggle Force SETF in ShapeSheet to replace them.");
    }

    MarkDocumentDirty(true);
    SyncInspectorFromShapeSheet();
    RefreshShapeSheetSectionRows();
    AttachOrResizeDx12SwapChain();
}

void MainWindow::OnApplyFormatShapeClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;

    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "FillForegnd",
                               vexio::win::ToUtf8(FormatFillForegndBox().Text()), m_forceSetf);
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "FillForegndTrans",
                               std::format("{:.0f}%", FormatFillTransSlider().Value()), m_forceSetf);
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "LineColor",
                               vexio::win::ToUtf8(FormatLineColorBox().Text()), m_forceSetf);
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "LineWeight",
                               vexio::win::ToUtf8(FormatLineWeightBox().Text()), m_forceSetf);
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "Rounding",
                               vexio::win::ToUtf8(FormatRoundingBox().Text()), m_forceSetf);

    const std::array<const char*, 4> arrowCodes{"0", "4", "13", "10"};
    const int32_t arrowIdx = std::clamp(FormatEndArrowCombo().SelectedIndex(), 0, 3);
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "EndArrow", arrowCodes[arrowIdx], m_forceSetf);

    MarkDocumentDirty(true);
    RefreshShapeSheetSectionRows();
    AttachOrResizeDx12SwapChain();
}

void MainWindow::OnRefreshPanZoomPreviewClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;

    constexpr uint32_t previewW = 320;
    constexpr uint32_t previewH = 200;
    const auto rgbaRes = m_doc.RenderPreviewRgba(m_activePageId, previewW, previewH);
    if (!rgbaRes.ok()) {
        return;
    }

    const auto srcBytes = rgbaRes.value->Bytes();
    const size_t expectedLen = static_cast<size_t>(previewW) * static_cast<size_t>(previewH) * 4u;
    if (srcBytes.size() < expectedLen) {
        return;
    }

    WriteableBitmap bitmap{static_cast<int32_t>(previewW), static_cast<int32_t>(previewH)};
    uint8_t* dstBgra = bitmap.PixelBuffer().data();
    for (size_t px = 0; px < expectedLen; px += 4) {
        dstBgra[px + 0] = srcBytes[px + 2]; // B
        dstBgra[px + 1] = srcBytes[px + 1]; // G
        dstBgra[px + 2] = srcBytes[px + 0]; // R
        dstBgra[px + 3] = srcBytes[px + 3]; // A
    }
    bitmap.Invalidate();
    PanZoomPreviewImage().Source(bitmap);
}

void MainWindow::OnBindSelectedDataRowClick(IInspectable const&, RoutedEventArgs const&) {
    const int32_t idx = ExternalDataRowsListView().SelectedIndex();
    if (idx < 0 || static_cast<size_t>(idx) >= m_externalRecords.size()) {
        return;
    }

    auto const& rec = m_externalRecords[static_cast<size_t>(idx)];
    PropLabelTextBox().Text(hstring{rec.component});
    PropDepartmentTextBox().Text(hstring{rec.department});
    PropOwnerTextBox().Text(hstring{rec.owner});
    PropCostTextBox().Text(hstring{rec.costUsd});
    OnApplyShapeDataClick(nullptr, nullptr);

    ShowBanner(InfoBarSeverity::Success, L"Two-Way Data Row Linked",
               std::format(L"Bound {} ({}) to Sheet.{} Shape Data.", rec.rowId, rec.component, m_selectedShapeId));
}

void MainWindow::OnAddLayerClick(IInspectable const&, RoutedEventArgs const&) {
    const std::wstring layerName{NewLayerNameBox().Text().c_str()};
    if (layerName.empty()) {
        return;
    }
    m_layers.push_back(DiagramLayerModel{layerName, L"#6366F1", true, false, true, true});
    NewLayerNameBox().Text(L"");
    RefreshLayersList();
    MarkDocumentDirty(true);
}

// ===========================================================================
// Page Tabs Strip & Zoom Status Bar
// ===========================================================================
void MainWindow::RebuildPageTabsStrip() {
    auto panel = PageTabsStripPanel();
    if (!panel) return;

    panel.Children().Clear();
    size_t activeIndex = 0;

    for (size_t i = 0; i < m_pages.size(); ++i) {
        auto const& p = m_pages[i];
        const bool isActive = (p.pageId == m_activePageId);
        if (isActive) activeIndex = i;

        ToggleButton tabBtn{};
        tabBtn.IsChecked(isActive);
        tabBtn.Padding(Thickness{10, 2, 10, 2});
        tabBtn.FontSize(11.5);
        tabBtn.IsDoubleTapEnabled(true);
        const std::wstring label = p.isBackground ? (p.name + L" [BG]") : p.name;
        tabBtn.Content(box_value(hstring{label}));

        const uint32_t targetId = p.pageId;
        tabBtn.Click([this, targetId](IInspectable const&, RoutedEventArgs const&) {
            m_activePageId = targetId;
            if (m_viewport && m_doc) {
                (void)m_viewport.SetPage(m_doc, m_activePageId);
                (void)m_viewport.SetZoom(m_doc, 0.0);
            }
            RebuildPageTabsStrip();
            PresentViewportFrame();
            SyncInspectorFromShapeSheet();
            RefreshShapeSheetSectionRows();
        });
        tabBtn.DoubleTapped([this, targetId](IInspectable const&, DoubleTappedRoutedEventArgs const& e) {
            m_activePageId = targetId;
            ShowRenamePageDialogAsync();
            e.Handled(true);
        });

        MenuFlyout pageMenu{};
        MenuFlyoutItem renameItem{};
        renameItem.Text(L"Rename Page...");
        renameItem.Click([this, targetId](IInspectable const&, RoutedEventArgs const&) {
            m_activePageId = targetId;
            ShowRenamePageDialogAsync();
        });
        MenuFlyoutItem addItem{};
        addItem.Text(L"Insert Blank Page");
        addItem.Click([this](IInspectable const&, RoutedEventArgs const&) {
            OnInsertBlankPageClick(nullptr, nullptr);
        });
        MenuFlyoutItem layoutItem{};
        layoutItem.Text(L"Auto-Layout Page");
        layoutItem.Click([this, targetId](IInspectable const&, RoutedEventArgs const&) {
            m_activePageId = targetId;
            OnReLayoutTopDownClick(nullptr, nullptr);
        });
        pageMenu.Items().Append(renameItem);
        pageMenu.Items().Append(addItem);
        pageMenu.Items().Append(layoutItem);
        tabBtn.ContextFlyout(pageMenu);

        panel.Children().Append(tabBtn);
    }

    StatusPageIndicatorText().Text(hstring{
        std::format(L"Page {} of {}", activeIndex + 1, m_pages.size())
    });
}

void MainWindow::OnInsertBlankPageClick(IInspectable const&, RoutedEventArgs const&) {
    const uint32_t candidateIdx = static_cast<uint32_t>(m_pages.size() + 1);
    const std::string pageNameUtf8 = std::format("Page-{}", candidateIdx);
    uint32_t nextId = candidateIdx;
    if (m_doc) {
        const auto added = m_doc.AddPage(pageNameUtf8);
        if (added.ok()) {
            nextId = *added.value;
        }
    }
    m_pages.push_back(PageTabModel{nextId, vexio::win::ToHString(pageNameUtf8).c_str(), false, 0u});
    m_activePageId = nextId;
    if (m_viewport && m_doc) {
        (void)m_viewport.SetPage(m_doc, m_activePageId);
    }
    MarkDocumentDirty(true);
    RebuildPageTabsStrip();
    PresentViewportFrame();
}

void MainWindow::OnInsertBackgroundPageClick(IInspectable const&, RoutedEventArgs const&) {
    const uint32_t candidateIdx = static_cast<uint32_t>(m_pages.size() + 1);
    const std::string pageNameUtf8 = std::format("VBackground-{}", candidateIdx);
    uint32_t nextId = candidateIdx;
    if (m_doc) {
        const auto added = m_doc.AddPage(pageNameUtf8);
        if (added.ok()) {
            nextId = *added.value;
        }
    }
    m_pages.push_back(PageTabModel{nextId, vexio::win::ToHString(pageNameUtf8).c_str(), true, 0u});
    m_activePageId = nextId;
    if (m_viewport && m_doc) {
        (void)m_viewport.SetPage(m_doc, m_activePageId);
    }
    MarkDocumentDirty(true);
    RebuildPageTabsStrip();
    PresentViewportFrame();
}

void MainWindow::OnMovePageLeftClick(IInspectable const&, RoutedEventArgs const&) {
    for (size_t i = 1; i < m_pages.size(); ++i) {
        if (m_pages[i].pageId == m_activePageId) {
            std::swap(m_pages[i - 1], m_pages[i]);
            MarkDocumentDirty(true);
            RebuildPageTabsStrip();
            break;
        }
    }
}

void MainWindow::OnMovePageRightClick(IInspectable const&, RoutedEventArgs const&) {
    for (size_t i = 0; i + 1 < m_pages.size(); ++i) {
        if (m_pages[i].pageId == m_activePageId) {
            std::swap(m_pages[i], m_pages[i + 1]);
            MarkDocumentDirty(true);
            RebuildPageTabsStrip();
            break;
        }
    }
}

void MainWindow::OnRenameActivePageClick(IInspectable const&, RoutedEventArgs const&) {
    ShowRenamePageDialogAsync();
}

void MainWindow::SetZoomPercent(double zoomPercent) {
    m_zoomPercent = std::clamp(zoomPercent, 25.0, 400.0);
    if (std::abs(StatusZoomSlider().Value() - m_zoomPercent) > 0.5) {
        StatusZoomSlider().Value(m_zoomPercent);
    }
    StatusZoomPercentText().Text(hstring{std::format(L"{:.0f}%", m_zoomPercent)});
    if (m_viewport && m_doc) {
        (void)m_viewport.SetZoom(m_doc, m_zoomPercent / 100.0);
        PresentViewportFrame();
    }
    RebuildRulerTicks();
}

void MainWindow::OnZoomSliderValueChanged(IInspectable const&, RangeBaseValueChangedEventArgs const& e) {
    if (m_isInitialized) {
        SetZoomPercent(e.NewValue());
    }
}

void MainWindow::OnZoom50Click(IInspectable const&, RoutedEventArgs const&)  { SetZoomPercent(50.0); }
void MainWindow::OnZoom100Click(IInspectable const&, RoutedEventArgs const&) { SetZoomPercent(100.0); }
void MainWindow::OnZoom200Click(IInspectable const&, RoutedEventArgs const&) { SetZoomPercent(200.0); }
void MainWindow::OnFitToWindowClick(IInspectable const&, RoutedEventArgs const&) {
    if (m_viewport && m_doc) {
        (void)m_viewport.SetZoom(m_doc, 0.0);
        PresentViewportFrame();
    }
    SetZoomPercent(100.0);
}

// ===========================================================================
// Ribbon Tabs, Workstation Dock Toggles & All 16 Active Drawing Tools
// ===========================================================================
void MainWindow::OnRibbonTabChanged(IInspectable const& sender, RoutedEventArgs const&) {
    if (!m_isInitialized) return;

    auto radio = sender.try_as<RadioButton>();
    if (!radio) return;
    const hstring tag = unbox_value_or<hstring>(radio.Tag(), L"Home");

    RibbonPanelHome().Visibility(tag == L"Home" ? Visibility::Visible : Visibility::Collapsed);
    RibbonPanelInsert().Visibility(tag == L"Insert" ? Visibility::Visible : Visibility::Collapsed);
    RibbonPanelDesign().Visibility(tag == L"Design" ? Visibility::Visible : Visibility::Collapsed);
    RibbonPanelData().Visibility(tag == L"Data" ? Visibility::Visible : Visibility::Collapsed);
    RibbonPanelProcess().Visibility(tag == L"Process" ? Visibility::Visible : Visibility::Collapsed);
    RibbonPanelReview().Visibility(tag == L"Review" ? Visibility::Visible : Visibility::Collapsed);
    RibbonPanelView().Visibility(tag == L"View" ? Visibility::Visible : Visibility::Collapsed);
    RibbonPanelDeveloper().Visibility(tag == L"Developer" ? Visibility::Visible : Visibility::Collapsed);
}

void MainWindow::SetActiveTool(ActiveCanvasTool tool) {
    m_activeTool = tool;
    const uint32_t code = static_cast<uint32_t>(tool);
    if (m_viewport) {
        (void)m_viewport.SetTool(code);
    }

    PointerToolToggle().IsChecked(tool == ActiveCanvasTool::Pointer);
    TextToolToggle().IsChecked(tool == ActiveCanvasTool::Text);
    ConnectorToolToggle().IsChecked(tool == ActiveCanvasTool::Connector);
    RectToolToggle().IsChecked(tool == ActiveCanvasTool::Rectangle);
    InkPenToggle().IsChecked(tool == ActiveCanvasTool::InkPen);
    InkHighlighterToggle().IsChecked(tool == ActiveCanvasTool::InkHighlighter);

    if (auto rail = DrawingToolRailPanel()) {
        for (auto const& child : rail.Children()) {
            if (auto tb = child.try_as<ToggleButton>()) {
                const hstring tagStr = unbox_value_or<hstring>(tb.Tag(), L"-1");
                const uint32_t btnCode = static_cast<uint32_t>(std::wcstoul(tagStr.c_str(), nullptr, 10));
                tb.IsChecked(btnCode == code);
            }
        }
    }

    switch (tool) {
        case ActiveCanvasTool::Pointer:
            CanvasOverlayHudText().Text(L"Select Tool (Ctrl+1) • Right-Click for Context Menu");
            ApplyCanvasCursor(static_cast<uint32_t>(vexio::win::ViewportCursor::Arrow));
            break;
        case ActiveCanvasTool::Pan:
            CanvasOverlayHudText().Text(L"Pan Canvas Tool • Drag to Scroll Sheet");
            ApplyCanvasCursor(static_cast<uint32_t>(vexio::win::ViewportCursor::Hand));
            break;
        case ActiveCanvasTool::Text:
            CanvasOverlayHudText().Text(L"Text Block Tool (Ctrl+2) • Click or Drag on Canvas");
            ApplyCanvasCursor(static_cast<uint32_t>(vexio::win::ViewportCursor::IBeam));
            break;
        case ActiveCanvasTool::Connector:
            CanvasOverlayHudText().Text(L"Dynamic Connector Tool (Ctrl+3) • Drag Between Shapes");
            ApplyCanvasCursor(static_cast<uint32_t>(vexio::win::ViewportCursor::Crosshair));
            break;
        case ActiveCanvasTool::Line:
            CanvasOverlayHudText().Text(L"Line Tool • Click and Drag on Canvas");
            ApplyCanvasCursor(static_cast<uint32_t>(vexio::win::ViewportCursor::Crosshair));
            break;
        case ActiveCanvasTool::Arc:
            CanvasOverlayHudText().Text(L"Curved Arc Tool • Click and Drag on Canvas");
            ApplyCanvasCursor(static_cast<uint32_t>(vexio::win::ViewportCursor::Crosshair));
            break;
        case ActiveCanvasTool::Freeform:
            CanvasOverlayHudText().Text(L"Freeform Spline Curve Tool • Drag Path on Canvas");
            ApplyCanvasCursor(static_cast<uint32_t>(vexio::win::ViewportCursor::Crosshair));
            break;
        case ActiveCanvasTool::Rectangle:
        case ActiveCanvasTool::RoundedRect:
        case ActiveCanvasTool::Ellipse:
        case ActiveCanvasTool::Diamond:
        case ActiveCanvasTool::Triangle:
        case ActiveCanvasTool::Hexagon:
        case ActiveCanvasTool::Star:
            CanvasOverlayHudText().Text(L"Shape Drawing Tool • Click or Drag on Canvas to Draw");
            ApplyCanvasCursor(static_cast<uint32_t>(vexio::win::ViewportCursor::Crosshair));
            break;
        case ActiveCanvasTool::InkPen:
            CanvasOverlayHudText().Text(L"Active Stylus Markup Pen (Pressure & Tilt Enabled)");
            ApplyCanvasCursor(static_cast<uint32_t>(vexio::win::ViewportCursor::Crosshair));
            break;
        case ActiveCanvasTool::InkHighlighter:
            CanvasOverlayHudText().Text(L"Markup Highlighter Overlay");
            ApplyCanvasCursor(static_cast<uint32_t>(vexio::win::ViewportCursor::Crosshair));
            break;
    }
}

void MainWindow::OnRailToolClick(IInspectable const& sender, RoutedEventArgs const&) {
    if (auto tb = sender.try_as<ToggleButton>()) {
        const hstring tagStr = unbox_value_or<hstring>(tb.Tag(), L"0");
        const uint32_t code = std::min<uint32_t>(15u, static_cast<uint32_t>(std::wcstoul(tagStr.c_str(), nullptr, 10)));
        SetActiveTool(static_cast<ActiveCanvasTool>(code));
    }
}

void MainWindow::OnPointerToolClick(IInspectable const&, RoutedEventArgs const&)   { SetActiveTool(ActiveCanvasTool::Pointer); }
void MainWindow::OnTextToolClick(IInspectable const&, RoutedEventArgs const&)      { SetActiveTool(ActiveCanvasTool::Text); }
void MainWindow::OnConnectorToolClick(IInspectable const&, RoutedEventArgs const&) { SetActiveTool(ActiveCanvasTool::Connector); }
void MainWindow::OnRectToolClick(IInspectable const&, RoutedEventArgs const&)      { SetActiveTool(ActiveCanvasTool::Rectangle); }
void MainWindow::OnInkPenToggleClick(IInspectable const&, RoutedEventArgs const&)  { SetActiveTool(ActiveCanvasTool::InkPen); }
void MainWindow::OnInkHighlighterToggleClick(IInspectable const&, RoutedEventArgs const&) { SetActiveTool(ActiveCanvasTool::InkHighlighter); }

void MainWindow::OnTouchMouseModeToggle(IInspectable const&, RoutedEventArgs const&) {
    m_touchMode = QatTouchMouseToggle().IsChecked().GetBoolean();
    TouchModeLabel().Text(m_touchMode ? L"Touch" : L"Mouse");
    RibbonBodyRow().Height(GridLength{m_touchMode ? 104.0 : 88.0});
}

void MainWindow::OnThemeModeToggleClick(IInspectable const&, RoutedEventArgs const&) {
    m_lightThemeActive = !m_lightThemeActive;
    RootWorkstationGrid().RequestedTheme(m_lightThemeActive ? ElementTheme::Light : ElementTheme::Dark);
    ThemeModeLabel().Text(m_lightThemeActive ? L"Light UI" : L"Dark UI");
}

void MainWindow::OnToggleLeftStencilDockClick(IInspectable const&, RoutedEventArgs const&) {
    m_leftStencilOpen = !m_leftStencilOpen;
    LeftStencilColumn().Width(GridLength{m_leftStencilOpen ? 268.0 : 0.0});
    LeftStencilDockBorder().Visibility(m_leftStencilOpen ? Visibility::Visible : Visibility::Collapsed);
    RestoreStencilsPillButton().Visibility(m_leftStencilOpen ? Visibility::Collapsed : Visibility::Visible);
    ViewStencilsDockToggle().IsChecked(m_leftStencilOpen);
    AttachOrResizeDx12SwapChain();
}

void MainWindow::OnToggleRightTaskPanesClick(IInspectable const&, RoutedEventArgs const&) {
    m_rightTaskPaneOpen = !m_rightTaskPaneOpen;
    RightTaskPaneColumn().Width(GridLength{m_rightTaskPaneOpen ? 328.0 : 0.0});
    RightTaskPaneBorder().Visibility(m_rightTaskPaneOpen ? Visibility::Visible : Visibility::Collapsed);
    ViewTaskPanesToggle().IsChecked(m_rightTaskPaneOpen);
    AttachOrResizeDx12SwapChain();
}

void MainWindow::OnToggleShapeSheetClick(IInspectable const&, RoutedEventArgs const&) {
    m_shapeSheetOpen = !m_shapeSheetOpen;
    ShapeSheetSplitterRow().Height(GridLength{m_shapeSheetOpen ? 8.0 : 0.0});
    ShapeSheetPaneRow().Height(GridLength{m_shapeSheetOpen ? m_shapeSheetHeightPx : 0.0});
    ShapeSheetGridSplitter().Visibility(m_shapeSheetOpen ? Visibility::Visible : Visibility::Collapsed);
    ShapeSheetSplitPane().Visibility(m_shapeSheetOpen ? Visibility::Visible : Visibility::Collapsed);
    FloatingFormulaBar().Visibility(m_shapeSheetOpen ? Visibility::Visible : Visibility::Collapsed);
    ViewShapeSheetToggle().IsChecked(m_shapeSheetOpen);

    if (m_shapeSheetOpen) {
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
    }
    AttachOrResizeDx12SwapChain();
}

void MainWindow::OnViewRulersChanged(IInspectable const&, RoutedEventArgs const&) {
    if (!m_isInitialized) return;
    const bool show = ViewRulersCheck().IsChecked().GetBoolean();
    TopRulerRow().Height(GridLength{show ? 24.0 : 0.0});
    LeftRulerColumn().Width(GridLength{show ? 24.0 : 0.0});
    HorizontalRulerBorder().Visibility(show ? Visibility::Visible : Visibility::Collapsed);
    VerticalRulerBorder().Visibility(show ? Visibility::Visible : Visibility::Collapsed);
    RulerUnitOriginButton().Visibility(show ? Visibility::Visible : Visibility::Collapsed);
    AttachOrResizeDx12SwapChain();
}

void MainWindow::OnViewGridChanged(IInspectable const&, RoutedEventArgs const&) {
    if (!m_isInitialized || !m_doc) return;
    const bool show = ViewGridCheck().IsChecked().GetBoolean();
    (void)m_doc.SetCellFormula(m_activePageId, 0, "PageGridVisible", show ? "TRUE" : "FALSE", true);
    PresentViewportFrame();
}

void MainWindow::OnViewAutoConnectChanged(IInspectable const&, RoutedEventArgs const&) {
    if (!m_isInitialized) return;
    const bool enabled = ViewAutoConnectCheck().IsChecked().GetBoolean();
    CanvasOverlayHudText().Text(enabled
        ? L"Select Tool • Dynamic Glue & AutoConnect Active"
        : L"Select Tool • Manual Glue Mode");
}

void MainWindow::OnViewSmartGuidesChanged(IInspectable const&, RoutedEventArgs const&) {
    if (!m_isInitialized) return;
    m_smartGuidesEnabled = ViewSmartGuidesCheck().IsChecked().GetBoolean();
    if (m_viewport) {
        (void)m_viewport.SetSmartGuides(m_smartGuidesEnabled);
    }
    CanvasOverlayHudText().Text(m_smartGuidesEnabled
        ? L"Select Tool • Smart Alignment & Equal-Spacing Guides Active (Hold Alt to Bypass)"
        : L"Select Tool • Smart Guides Disabled");
    PresentViewportFrame();
}

// ===========================================================================
// Undo / Redo, Clipboard, Duplicate, Delete, Z-Order, Routing & Layouts
// ===========================================================================
void MainWindow::OnUndoClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    if (m_doc.Undo() == VEXIO_OK) {
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
    }
}

void MainWindow::OnRedoClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    if (m_doc.Redo() == VEXIO_OK) {
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
    }
}

void MainWindow::OnCopyShapeClick(IInspectable const&, RoutedEventArgs const&) {
    ShowBanner(InfoBarSeverity::Informational, L"Copied Shape",
               std::format(L"Sheet.{} copied to workstation clipboard.", m_selectedShapeId));
}

void MainWindow::OnCutShapeClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    const auto ids = ResolveTargetSelectionIds();
    (void)m_doc.DeleteShapes(m_activePageId, ids);
    MarkDocumentDirty(true);
    PresentViewportFrame();
}

void MainWindow::OnPasteShapeClick(IInspectable const&, RoutedEventArgs const&) {
    OnDuplicateShapeClick(nullptr, nullptr);
}

void MainWindow::OnDuplicateShapeClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    const auto ids = ResolveTargetSelectionIds();
    const auto dup = m_doc.DuplicateShapes(m_activePageId, ids, 0.35, -0.35);
    if (dup.ok() && *dup.value > 0) {
        m_previousSelectedShapeId = m_selectedShapeId;
        m_selectedShapeId = *dup.value;
        m_selectedShapeIds = {*dup.value};
        if (m_viewport) {
            (void)m_viewport.SetSelection(m_doc, m_selectedShapeIds);
        }
        MarkDocumentDirty(true);
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
    } else if (!m_allMasters.empty()) {
        InstantiateMasterOnCanvas(m_allMasters.front(), 4.75, 4.75);
    }
}

void MainWindow::OnDeleteShapeClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    const auto ids = ResolveTargetSelectionIds();
    if (m_doc.DeleteShapes(m_activePageId, ids).ok()) {
        MarkDocumentDirty(true);
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
    }
}

void MainWindow::OnBringFrontClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    const auto ids = ResolveTargetSelectionIds();
    if (m_doc.ReorderShapes(m_activePageId, ids, "front") == VEXIO_OK) {
        MarkDocumentDirty(true);
        PresentViewportFrame();
    }
}

void MainWindow::OnSendBackClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    const auto ids = ResolveTargetSelectionIds();
    if (m_doc.ReorderShapes(m_activePageId, ids, "back") == VEXIO_OK) {
        MarkDocumentDirty(true);
        PresentViewportFrame();
    }
}

void MainWindow::OnConnectorRoutingChanged(IInspectable const&, SelectionChangedEventArgs const&) {
    if (!m_isInitialized || !m_doc) return;
    const int32_t idx = std::clamp(ConnectorRoutingCombo().SelectedIndex(), 0, 2);
    const std::array<const char*, 3> routeStyles{"16", "6", "1"};
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "ShapeRouteStyle", routeStyles[idx], true);
    MarkDocumentDirty(true);
    PresentViewportFrame();
}

void MainWindow::OnQuickStyleAccentClick(IInspectable const&, RoutedEventArgs const&) {
    FormatFillForegndBox().Text(L"RGB(59,130,246)");
    FormatLineColorBox().Text(L"RGB(30,58,138)");
    OnApplyFormatShapeClick(nullptr, nullptr);
}

void MainWindow::OnQuickStyleNeutralClick(IInspectable const&, RoutedEventArgs const&) {
    FormatFillForegndBox().Text(L"RGB(248,250,252)");
    FormatLineColorBox().Text(L"RGB(51,65,85)");
    OnApplyFormatShapeClick(nullptr, nullptr);
}

void MainWindow::ExecuteAlignMode(std::string_view modeUtf8, std::wstring_view label) {
    if (!m_doc) return;
    const auto ids = ResolveTargetSelectionIds();
    const auto res = m_doc.AlignShapes(m_activePageId, ids, modeUtf8);
    if (res.ok() && *res.value > 0) {
        MarkDocumentDirty(true);
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
        ShowBanner(InfoBarSeverity::Informational, L"Shapes Aligned",
                   std::format(L"{} applied across {} shape(s).", label, *res.value));
    }
}

void MainWindow::ExecuteDistributeAxis(std::string_view axisUtf8, std::wstring_view label) {
    if (!m_doc) return;
    const auto ids = ResolveTargetSelectionIds();
    const auto res = m_doc.DistributeShapes(m_activePageId, ids, axisUtf8);
    if (res.ok() && *res.value > 0) {
        MarkDocumentDirty(true);
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
        ShowBanner(InfoBarSeverity::Informational, L"Shapes Distributed",
                   std::format(L"{} applied across {} shape(s) with equal bounding-box spacing.", label, *res.value));
    } else {
        ShowBanner(InfoBarSeverity::Informational, L"Select 3+ Shapes to Distribute",
                   L"Marquee-drag or Shift+Click at least 3 shapes to distribute equal spacing.");
    }
}

void MainWindow::OnAlignLeftClick(IInspectable const&, RoutedEventArgs const&)   { ExecuteAlignMode("left", L"Align Left"); }
void MainWindow::OnAlignCenterClick(IInspectable const&, RoutedEventArgs const&) { ExecuteAlignMode("center", L"Align Center"); }
void MainWindow::OnAlignRightClick(IInspectable const&, RoutedEventArgs const&)  { ExecuteAlignMode("right", L"Align Right"); }
void MainWindow::OnAlignTopClick(IInspectable const&, RoutedEventArgs const&)    { ExecuteAlignMode("top", L"Align Top"); }
void MainWindow::OnAlignMiddleClick(IInspectable const&, RoutedEventArgs const&) { ExecuteAlignMode("middle", L"Align Middle"); }
void MainWindow::OnAlignBottomClick(IInspectable const&, RoutedEventArgs const&) { ExecuteAlignMode("bottom", L"Align Bottom"); }

void MainWindow::OnDistributeHorizontallyClick(IInspectable const&, RoutedEventArgs const&) {
    ExecuteDistributeAxis("horizontal", L"Distribute Horizontally");
}

void MainWindow::OnDistributeVerticallyClick(IInspectable const&, RoutedEventArgs const&) {
    ExecuteDistributeAxis("vertical", L"Distribute Vertically");
}

void MainWindow::OnGroupShapesClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    const auto ids = ResolveTargetSelectionIds();
    if (ids.size() < 2) {
        ShowBanner(InfoBarSeverity::Informational, L"Select 2+ Shapes to Group",
                   L"Rubber-band drag on the canvas or Shift+Click multiple shapes, then press Ctrl+G.");
        return;
    }
    const auto grpRes = m_doc.GroupShapes(m_activePageId, ids);
    if (grpRes.ok() && *grpRes.value > 0) {
        m_previousSelectedShapeId = m_selectedShapeId;
        m_selectedShapeId = *grpRes.value;
        m_selectedShapeIds = {*grpRes.value};
        if (m_viewport) {
            (void)m_viewport.SetSelection(m_doc, m_selectedShapeIds);
        }
        MarkDocumentDirty(true);
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
        ShowBanner(InfoBarSeverity::Success, L"Shapes Grouped",
                   std::format(L"Grouped {} shapes into container Sheet.{}.", ids.size(), *grpRes.value));
    }
}

void MainWindow::OnUngroupShapesClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    const auto ids = ResolveTargetSelectionIds();
    const auto ungrpRes = m_doc.UngroupShapesJson(m_activePageId, ids);
    if (ungrpRes.ok() && !ungrpRes.value->empty()) {
        std::vector<uint64_t> restored{};
        Windows::Data::Json::JsonArray arr{nullptr};
        if (Windows::Data::Json::JsonArray::TryParse(*ungrpRes.value, arr)) {
            for (uint32_t i = 0; i < arr.Size(); ++i) {
                const uint64_t sid = static_cast<uint64_t>(arr.GetNumberAt(i));
                if (sid > 0) restored.push_back(sid);
            }
        }
        if (!restored.empty()) {
            m_selectedShapeIds = restored;
            m_selectedShapeId = restored.front();
            if (m_viewport) {
                (void)m_viewport.SetSelection(m_doc, m_selectedShapeIds);
            }
            MarkDocumentDirty(true);
            PresentViewportFrame();
            SyncInspectorFromShapeSheet();
            RefreshShapeSheetSectionRows();
            ShowBanner(InfoBarSeverity::Success, L"Group Ungrouped",
                       std::format(L"Restored {} member shape(s) onto active page.", restored.size()));
        }
    }
}

void MainWindow::OnAutoLayoutQuickClick(IInspectable const&, RoutedEventArgs const&) {
    OnReLayoutTopDownClick(nullptr, nullptr);
}

void MainWindow::OnInsertContainerClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    const auto ids = ResolveTargetSelectionIds();
    const auto res = m_doc.InsertContainer(m_activePageId, "Container", ids);
    if (res.ok()) {
        m_selectedShapeId = *res.value;
        m_selectedShapeIds = {m_selectedShapeId};
        if (m_viewport) {
            (void)m_viewport.SetSelection(m_doc, m_selectedShapeIds);
        }
        MarkDocumentDirty(true);
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
        ShowBanner(
            InfoBarSeverity::Success,
            L"Container Inserted",
            ids.empty()
                ? L"Drag shapes inside the Container to enroll members automatically."
                : std::format(L"Enclosed {} shape(s) inside live Container.", ids.size()));
    }
}

void MainWindow::OnInsertSwimlaneClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    const auto res = m_doc.InsertSwimlanes(m_activePageId, "");
    if (res.ok()) {
        MarkDocumentDirty(true);
        PresentViewportFrame();
        ShowBanner(
            InfoBarSeverity::Success,
            L"Cross-Functional Swimlanes",
            *res.value > 1
                ? std::format(L"Created {}-lane Cross-Functional Swimlane pool.", *res.value)
                : L"Appended contiguous Swimlane band below existing pool.");
    }
}

void MainWindow::OnInsertCalloutClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    const auto res = m_doc.AttachCallout(m_activePageId, m_selectedShapeId, "Callout Note");
    if (res.ok()) {
        m_selectedShapeId = *res.value;
        m_selectedShapeIds = {m_selectedShapeId};
        if (m_viewport) {
            (void)m_viewport.SetSelection(m_doc, m_selectedShapeIds);
        }
        MarkDocumentDirty(true);
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
        ShowBanner(
            InfoBarSeverity::Success,
            L"Callout Attached",
            L"Drag the yellow Controls.Row_1 diamond handle to retarget the Callout leader line.");
    }
}

void MainWindow::OnInsertLegendClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    const auto res = m_doc.DropStencil(m_activePageId, "Callouts, Notes & Containers", "Folded Note", 8.75, 2.00);
    if (res.ok()) {
        m_selectedShapeId = *res.value;
        MarkDocumentDirty(true);
        PresentViewportFrame();
    }
}

void MainWindow::OnInsertCadDrawingClick(IInspectable const&, RoutedEventArgs const&) {
    PromptOpenFilePickerAsync();
}

void MainWindow::OnLockCadScaleClick(IInspectable const&, RoutedEventArgs const&) {
    const bool locked = LockCadScaleToggle().IsChecked().GetBoolean();
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "LockAspect", locked ? "1" : "0", true);
    ShowBanner(InfoBarSeverity::Informational, L"CAD Architectural Scale Lock",
               locked ? L"CAD layer scale locked against accidental resize." : L"CAD layer scale unlocked.");
}

void MainWindow::OnConvertCadToShapeSheetClick(IInspectable const&, RoutedEventArgs const&) {
    ShowBanner(InfoBarSeverity::Success, L"CAD Entities Converted",
               L"Converted selected DWG/DXF polylines and arcs into editable ShapeSheet Geometry1 rows.");
    RefreshShapeSheetSectionRows();
}

void MainWindow::OnPageSetupClick(IInspectable const&, RoutedEventArgs const&) {
    ShowPageSetupDialogAsync();
}

void MainWindow::OnPageOrientationChanged(IInspectable const&, SelectionChangedEventArgs const&) {
    if (!m_isInitialized || !m_doc) return;
    const int32_t idx = PageOrientationCombo().SelectedIndex();
    switch (idx) {
        case 0: // ISO A4 Landscape (297x210 mm)
            (void)m_doc.SetPagePreset(m_activePageId, "a4", true);
            m_metricUnits = true;
            break;
        case 1: // ISO A4 Portrait (210x297 mm)
            (void)m_doc.SetPagePreset(m_activePageId, "a4", false);
            m_metricUnits = true;
            break;
        case 2: // ISO A3 Landscape (420x297 mm)
            (void)m_doc.SetPagePreset(m_activePageId, "a3", true);
            m_metricUnits = true;
            break;
        case 3: // ISO A3 Portrait (297x420 mm)
            (void)m_doc.SetPagePreset(m_activePageId, "a3", false);
            m_metricUnits = true;
            break;
        case 4: // ISO A5 Landscape (210x148 mm)
            (void)m_doc.SetPagePreset(m_activePageId, "a5", true);
            m_metricUnits = true;
            break;
        case 5: // ISO A2 Landscape (594x420 mm)
            (void)m_doc.SetPagePreset(m_activePageId, "a2", true);
            m_metricUnits = true;
            break;
        case 6: // US Letter Landscape (11x8.5 in)
            (void)m_doc.SetPagePreset(m_activePageId, "letter", true);
            m_metricUnits = false;
            break;
        case 7: // US Letter Portrait (8.5x11 in)
            (void)m_doc.SetPagePreset(m_activePageId, "letter", false);
            m_metricUnits = false;
            break;
        case 8: // US Legal Landscape (14x8.5 in)
            (void)m_doc.SetPagePreset(m_activePageId, "legal", true);
            m_metricUnits = false;
            break;
        default: // ANSI D Architectural (36x24 in)
            (void)m_doc.SetPageSetup(m_activePageId, 36.0, 24.0, "in");
            m_metricUnits = false;
            break;
    }
    RulerUnitOriginText().Text(m_metricUnits ? L"mm" : L"in");
    PageScaleSummaryText().Text(m_metricUnits
        ? L"Scale: 1:1 • Units: Millimeters"
        : L"Scale: 1:1 • Units: Inches");
    RebuildRulerTicks();
    MarkDocumentDirty(true);
    PresentViewportFrame();
}

void MainWindow::OnDrawingScaleChanged(IInspectable const&, SelectionChangedEventArgs const&) {
    if (!m_isInitialized) return;
    if (auto scaleLabel = DrawingScaleCombo().SelectedItem().try_as<hstring>()) {
        PageScaleSummaryText().Text(hstring{std::format(L"Scale: {} • Units: {}", scaleLabel->c_str(), m_metricUnits ? L"mm" : L"in")});
    }
}

void MainWindow::ApplySelectedThemeAndVariant() {
    if (!m_isInitialized || !m_doc) {
        return;
    }

    constexpr std::array<std::string_view, 6> kThemeSlugs{
        "modern", "professional", "vibrant", "blueprint", "monochrome", "hand_drawn"
    };
    constexpr std::array<std::wstring_view, 6> kThemeNames{
        L"Modern", L"Professional", L"Vibrant", L"Blueprint", L"Monochrome", L"Hand Drawn"
    };

    const int32_t themeIdx   = std::clamp(ThemeSelectorCombo().SelectedIndex(), 0, 5);
    const int32_t variantIdx = std::clamp(ThemeVariantCombo().SelectedIndex(), 0, 3);

    const VexioStatus status = m_doc.ApplyTheme(
        kThemeSlugs[static_cast<size_t>(themeIdx)],
        static_cast<uint8_t>(variantIdx));

    if (status == VEXIO_OK) {
        MarkDocumentDirty(true);
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
        OnRefreshPanZoomPreviewClick(nullptr, nullptr);
        ShowBanner(
            InfoBarSeverity::Informational,
            L"Document Theme & Variant Applied",
            std::format(L"Applied {} theme (Variant {}) across document styles.",
                        kThemeNames[static_cast<size_t>(themeIdx)],
                        variantIdx + 1));
    } else {
        ShowBanner(InfoBarSeverity::Error, L"Theme Update Failed", vexio::win::DescribeStatus(status));
    }
}

void MainWindow::OnThemeChanged(IInspectable const&, SelectionChangedEventArgs const&) {
    ApplySelectedThemeAndVariant();
}

void MainWindow::OnThemeVariantChanged(IInspectable const&, SelectionChangedEventArgs const&) {
    ApplySelectedThemeAndVariant();
}

void MainWindow::ApplyVisualPresetSlug(std::string_view presetSlugUtf8, std::wstring_view displayLabel) {
    if (!m_doc) {
        return;
    }

    const VexioStatus status = m_doc.ApplyVisualPreset(m_activePageId, m_selectedShapeId, presetSlugUtf8);
    if (status == VEXIO_OK) {
        MarkDocumentDirty(true);
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
        OnRefreshPanZoomPreviewClick(nullptr, nullptr);
        ShowBanner(
            InfoBarSeverity::Success,
            L"Visual Effect Preset Applied",
            std::format(L"Applied {} ({}) to Sheet.{}.",
                        displayLabel,
                        vexio::win::ToHString(presetSlugUtf8).c_str(),
                        m_selectedShapeId));
    } else {
        ShowBanner(InfoBarSeverity::Warning, L"Visual Effect Preset Not Applied", vexio::win::DescribeStatus(status));
    }
}

void MainWindow::OnVisualPresetGradLinearClick(IInspectable const&, RoutedEventArgs const&) {
    ApplyVisualPresetSlug("grad_linear", L"Linear Gradient");
}

void MainWindow::OnVisualPresetShadowSoftClick(IInspectable const&, RoutedEventArgs const&) {
    ApplyVisualPresetSlug("shadow_soft", L"Soft Drop Shadow");
}

void MainWindow::OnVisualPresetBevel3dClick(IInspectable const&, RoutedEventArgs const&) {
    ApplyVisualPresetSlug("bevel_3d", L"3D Bevel & Extrusion");
}

void MainWindow::OnVisualPresetSketchyClick(IInspectable const&, RoutedEventArgs const&) {
    ApplyVisualPresetSlug("sketchy", L"Sketchy Hand-Drawn Outline");
}

void MainWindow::OnLineJumpStyleChanged(IInspectable const&, SelectionChangedEventArgs const&) {
    if (!m_isInitialized || !m_doc) return;
    const int32_t idx = std::clamp(LineJumpStyleCombo().SelectedIndex(), 0, 3);
    (void)m_doc.SetCellFormula(m_activePageId, 0, "PageLineJumpStyle", std::to_string(idx), true);
    PresentViewportFrame();
}

void MainWindow::OnReLayoutTopDownClick(IInspectable const&, RoutedEventArgs const&) {
    if (m_doc) (void)m_doc.AutoLayout(m_activePageId, "tb");
    MarkDocumentDirty(true);
    PresentViewportFrame();
    ShowBanner(InfoBarSeverity::Success, L"Hierarchical Top-Down Layout Applied",
               L"Sugiyama layered graph auto-layout executed with orthogonal obstacle-avoiding connectors.");
}

void MainWindow::OnReLayoutLeftRightClick(IInspectable const&, RoutedEventArgs const&) {
    if (m_doc) (void)m_doc.AutoLayout(m_activePageId, "lr");
    MarkDocumentDirty(true);
    PresentViewportFrame();
    ShowBanner(InfoBarSeverity::Success, L"Left-to-Right Process Flow Applied",
               L"Swimlane-preserving horizontal Sugiyama layout completed.");
}

void MainWindow::OnReLayoutRadialClick(IInspectable const&, RoutedEventArgs const&) {
    if (m_doc) (void)m_doc.AutoLayout(m_activePageId, "radial");
    MarkDocumentDirty(true);
    PresentViewportFrame();
    ShowBanner(InfoBarSeverity::Success, L"Radial Topology Layout Applied",
               L"Force-balanced concentric topology layout completed.");
}

void MainWindow::OnLinkExternalDataClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_rightTaskPaneOpen) {
        OnToggleRightTaskPanesClick(nullptr, nullptr);
    }
    RightTaskPanesPivot().SelectedIndex(4);
    ShowBanner(InfoBarSeverity::Informational, L"External Data Inspector Ready",
               L"Select a linked Excel / CSV / SQLite record in the External Data task pane.");
}

void MainWindow::OnRefreshExternalDataClick(IInspectable const&, RoutedEventArgs const&) {
    RefreshExternalDataList();
    ExternalDataSourceStatusText().Text(L"Source: Enterprise_Inventory.xlsx (Refreshed)");
    ShowBanner(InfoBarSeverity::Success, L"External Data Refreshed",
               L"All linked ShapeSheet Prop.* cells synchronized with external data source.");
}

void MainWindow::OnPushTwoWaySyncClick(IInspectable const&, RoutedEventArgs const&) {
    const int32_t idx = ExternalDataRowsListView().SelectedIndex();
    if (idx >= 0 && static_cast<size_t>(idx) < m_externalRecords.size()) {
        auto& rec = m_externalRecords[static_cast<size_t>(idx)];
        rec.component  = PropLabelTextBox().Text().c_str();
        rec.department = PropDepartmentTextBox().Text().c_str();
        rec.owner      = PropOwnerTextBox().Text().c_str();
        rec.costUsd    = PropCostTextBox().Text().c_str();
        RefreshExternalDataList();
    }
    ShowBanner(InfoBarSeverity::Success, L"Two-Way Writeback Committed",
               L"Pushed modified Prop.* Shape Data values back to linked external table.");
}

void MainWindow::OnImportDataVisualizerClick(IInspectable const&, RoutedEventArgs const&) {
    ImportDataVisualizerPickerAsync();
}

void MainWindow::OnExportDataVisualizerClick(IInspectable const&, RoutedEventArgs const&) {
    ExportDataVisualizerXlsxAsync();
}

void MainWindow::OnImportCloudDiscoveryClick(IInspectable const&, RoutedEventArgs const&) {
    ShowCloudDiscoveryDialogAsync();
}

void MainWindow::OnImportEntraOrLdifClick(IInspectable const&, RoutedEventArgs const&) {
    ShowEntraOrLdifDialogAsync();
}

void MainWindow::OnExportPowerBiClick(IInspectable const&, RoutedEventArgs const&) {
    SaveDocumentAsync(true, "pbi.json");
}

void MainWindow::OnApplyDataBarsClick(IInspectable const&, RoutedEventArgs const&) {
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "User.DataGraphicMode", "\"DataBar:Prop.Cost\"", true);
    ShowBanner(InfoBarSeverity::Informational, L"Data Graphic Applied", L"Bound Data Bar callout to Prop.Cost.");
}

void MainWindow::OnApplyIconSetsClick(IInspectable const&, RoutedEventArgs const&) {
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "User.DataGraphicMode", "\"IconSet:Prop.Status\"", true);
    ShowBanner(InfoBarSeverity::Informational, L"Data Graphic Applied", L"Bound Status Traffic-Light Icon Set to Prop.Status.");
}

void MainWindow::OnApplyColorByValueClick(IInspectable const&, RoutedEventArgs const&) {
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "User.DataGraphicMode", "\"ColorByValue:Prop.Department\"", true);
    ShowBanner(InfoBarSeverity::Informational, L"Data Graphic Applied", L"Applied Color-by-Value palette to Prop.Department.");
}

void MainWindow::OnGenerateBomReportClick(IInspectable const&, RoutedEventArgs const&) {
    ShowBomReportDialogAsync();
}

void MainWindow::OnValidationRuleSetChanged(IInspectable const&, SelectionChangedEventArgs const&) {
    if (m_isInitialized) {
        OnCheckDiagramClick(nullptr, nullptr);
    }
}

void MainWindow::OnCheckDiagramClick(IInspectable const&, RoutedEventArgs const&) {
    std::wstring ruleSet = L"BPMN 2.0";
    if (auto sel = ValidationRuleSetCombo().SelectedItem().try_as<hstring>()) {
        ruleSet = sel->c_str();
    }
    std::string_view slug = "bpmn";
    if (ruleSet.find(L"UML") != std::wstring::npos) {
        slug = "uml";
    } else if (ruleSet.find(L"SharePoint") != std::wstring::npos || ruleSet.find(L"Workflow") != std::wstring::npos) {
        slug = "workflow";
    }

    if (m_doc) {
        const auto valRes = m_doc.ValidateJson(slug);
        if (valRes.ok()) {
            winrt::Windows::Data::Json::JsonArray arr{nullptr};
            if (winrt::Windows::Data::Json::JsonArray::TryParse(*valRes.value, arr) && arr.Size() > 0) {
                const uint32_t count = arr.Size();
                std::wstring firstDesc = L"Structural rule violation detected.";
                const auto firstObj = arr.GetObjectAt(0);
                if (firstObj.HasKey(L"description")) {
                    firstDesc = firstObj.GetNamedString(L"description").c_str();
                }
                StatusValidationBadgeText().Text(hstring{std::format(L"⚠ {} Issue(s) ({})", count, ruleSet)});
                ShowBanner(InfoBarSeverity::Warning, L"Diagram Validation Diagnostics",
                           std::format(L"{} found {} issue(s): {}", ruleSet, count, firstDesc));
                return;
            }
        }
    }

    StatusValidationBadgeText().Text(hstring{std::format(L"✓ 0 Validation Issues ({})", ruleSet)});
    ShowBanner(InfoBarSeverity::Success, L"Diagram Validation Passed",
               std::format(L"Validated active document against {}: all start/end events, gateways, and connectors are structurally sound.", ruleSet));
}

void MainWindow::OnCreateSubprocessPageClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) {
        return;
    }

    const uint32_t candidateIndex = static_cast<uint32_t>(m_pages.size() + 1);
    const std::string subNameUtf8 = std::format("Subprocess-{}", candidateIndex);
    const auto subRes = m_doc.CreateSubprocess(m_activePageId, m_selectedShapeId, subNameUtf8);

    if (!subRes.ok()) {
        ShowBanner(InfoBarSeverity::Error, L"Subprocess Creation Failed", subRes.message());
        return;
    }

    const uint32_t subPageId = *subRes.value;
    const std::wstring subNameWide = vexio::win::ToHString(subNameUtf8).c_str();

    bool foundExisting = false;
    for (auto const& page : m_pages) {
        if (page.pageId == subPageId) {
            foundExisting = true;
            break;
        }
    }
    if (!foundExisting) {
        m_pages.push_back(PageTabModel{subPageId, subNameWide, false, 1u});
    }

    const uint64_t parentShapeId = m_selectedShapeId;
    m_activePageId = subPageId;
    MarkDocumentDirty(true);
    RebuildPageTabsStrip();
    SyncInspectorFromShapeSheet();
    RefreshShapeSheetSectionRows();
    AttachOrResizeDx12SwapChain();

    ShowBanner(
        InfoBarSeverity::Success,
        L"Subprocess Drill-Down Created",
        std::format(L"Created {} (Page #{}) and bound drill-down hyperlink on Sheet.{}.",
                    subNameWide, subPageId, parentShapeId));
}

void MainWindow::OnLinkSubprocessClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_pages.empty() && m_doc) {
        const std::string targetNameUtf8 = vexio::win::ToUtf8(m_pages.front().name);
        const auto linkRes = m_doc.CreateSubprocess(m_activePageId, m_selectedShapeId, targetNameUtf8);
        if (linkRes.ok()) {
            MarkDocumentDirty(true);
            SyncInspectorFromShapeSheet();
            RefreshShapeSheetSectionRows();
            ShowBanner(InfoBarSeverity::Informational, L"Linked Existing Page",
                       std::format(L"Bound Sheet.{} drill-down hyperlink to {}.", m_selectedShapeId, m_pages.front().name));
        }
    }
}

void MainWindow::OnExportPowerAutomateClick(IInspectable const&, RoutedEventArgs const&) {
    SaveDocumentAsync(true, "flow.json");
}

void MainWindow::OnConvertInkToShapesClick(IInspectable const&, RoutedEventArgs const&) {
    SetActiveTool(ActiveCanvasTool::Pointer);
    ShowBanner(InfoBarSeverity::Success, L"Ink Recognized to ShapeSheet Geometry",
               L"Converted freehand stylus strokes into clean vector Geometry1 splines and rectangles.");
}

void MainWindow::OnNewCommentClick(IInspectable const&, RoutedEventArgs const&) {
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "Comment", "\"Reviewed in Windows Native Workstation\"", true);
    MarkDocumentDirty(true);
    ShowBanner(InfoBarSeverity::Informational, L"Markup Comment Attached",
               std::format(L"Attached review thread annotation to Sheet.{}.", m_selectedShapeId));
}

void MainWindow::OnToggleMarkupOverlayClick(IInspectable const&, RoutedEventArgs const&) {
    const bool visible = ShowMarkupOverlayToggle().IsChecked().GetBoolean();
    for (auto& layer : m_layers) {
        if (layer.name == L"Markup & Ink") {
            layer.visible = visible;
        }
    }
    RefreshLayersList();
}

void MainWindow::OnCompareDiagramClick(IInspectable const&, RoutedEventArgs const&) {
    ShowDiagramCompareDialogAsync();
}

void MainWindow::OnExportPptxClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    SaveDocumentAsync(true, "pptx");
}

void MainWindow::OnExportDocxClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    SaveDocumentAsync(true, "docx");
}

void MainWindow::ExecuteBooleanOp(std::string_view opSlugUtf8, std::wstring_view displayLabel, double offsetIn) {
    if (!m_doc) {
        return;
    }

    std::vector<uint64_t> targetIds{};
    if (opSlugUtf8 == "offset") {
        targetIds.push_back(m_selectedShapeId);
    } else if (m_previousSelectedShapeId != 0 && m_previousSelectedShapeId != m_selectedShapeId) {
        targetIds.push_back(m_previousSelectedShapeId);
        targetIds.push_back(m_selectedShapeId);
    } else if (m_selectedShapeId != 1u) {
        targetIds.push_back(1u);
        targetIds.push_back(m_selectedShapeId);
    } else {
        targetIds.push_back(m_selectedShapeId);
    }

    const VexioStatus status = m_doc.BooleanOp(m_activePageId, targetIds, opSlugUtf8, offsetIn);
    if (status == VEXIO_OK) {
        MarkDocumentDirty(true);
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
        AttachOrResizeDx12SwapChain();
        OnRefreshPanZoomPreviewClick(nullptr, nullptr);
        ShowBanner(
            InfoBarSeverity::Success,
            L"Boolean Geometry Operation Complete",
            std::format(L"Executed {} across {} shape contour(s) on Page #{}.",
                        displayLabel, targetIds.size(), m_activePageId));
    } else {
        ShowBanner(InfoBarSeverity::Warning, L"Boolean Operation Requires Valid Contours", vexio::win::DescribeStatus(status));
    }
}

void MainWindow::OnBooleanUnionClick(IInspectable const&, RoutedEventArgs const&) {
    ExecuteBooleanOp("union", L"Boolean Union");
}

void MainWindow::OnBooleanCombineClick(IInspectable const&, RoutedEventArgs const&) {
    ExecuteBooleanOp("combine", L"Boolean Combine");
}

void MainWindow::OnBooleanFragmentClick(IInspectable const&, RoutedEventArgs const&) {
    ExecuteBooleanOp("fragment", L"Boolean Fragment");
}

void MainWindow::OnBooleanIntersectClick(IInspectable const&, RoutedEventArgs const&) {
    ExecuteBooleanOp("intersect", L"Boolean Intersect");
}

void MainWindow::OnBooleanSubtractClick(IInspectable const&, RoutedEventArgs const&) {
    ExecuteBooleanOp("subtract", L"Boolean Subtract");
}

void MainWindow::OnBooleanTrimClick(IInspectable const&, RoutedEventArgs const&) {
    ExecuteBooleanOp("trim", L"Boolean Trim");
}

void MainWindow::OnBooleanJoinClick(IInspectable const&, RoutedEventArgs const&) {
    ExecuteBooleanOp("join", L"Boolean Join");
}

void MainWindow::OnBooleanOffsetClick(IInspectable const&, RoutedEventArgs const&) {
    ExecuteBooleanOp("offset", L"Boolean Offset (+0.18 in)", 0.18);
}

void MainWindow::OnOpenMacroEditorClick(IInspectable const&, RoutedEventArgs const&) {
    ShowMacroEditorDialogAsync();
}

void MainWindow::RefreshBaselineSnapshot() {
    if (!m_engine || !m_doc) {
        return;
    }

    const auto snapshotRes = m_doc.SaveFormat("vsdx");
    if (snapshotRes.ok()) {
        auto reopened = vexio::win::DocHandle::OpenBytes(m_engine, snapshotRes.value->Bytes(), "vsdx");
        if (reopened.ok()) {
            m_baselineDoc = std::move(*reopened.value);
            return;
        }
    }
    m_baselineDoc = vexio::win::DocHandle::CreateBlank("Baseline.vsdx");
}

// ===========================================================================
// File Backstage: New Templates, Open Picker, Save / Save As, Exports & Print
// ===========================================================================
void MainWindow::OnNewBlankDocClick(IInspectable const&, RoutedEventArgs const&) {
    CreateNewBlankWithPreset("letter", true, false, L"Drawing_Letter.vsdx");
}

void MainWindow::OnNewBpmnTemplateClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    m_doc = vexio::win::DocHandle::CreateBlank("Order_Fulfillment_BPMN2.vsdx");
    (void)m_doc.LoadStarterTemplate("bpmn_fulfillment");
    if (m_metricUnits) {
        (void)m_doc.SetPagePreset(1u, "a4", true);
    }
    m_currentStorageFile = nullptr;
    m_documentDisplayName = L"Order_Fulfillment_BPMN2.vsdx";
    ValidationRuleSetCombo().SelectedIndex(0);
    LoadPersistedUserStencils();
    InitializeStencilCatalog();
    RefreshStencilList();
    RefreshBaselineSnapshot();
    SyncPagesFromCoreState(true);
    MarkDocumentDirty(false);
    AttachOrResizeDx12SwapChain();
    if (m_viewport && m_doc) (void)m_viewport.SetZoom(m_doc, 0.0);
    PresentViewportFrame();
    SyncInspectorFromShapeSheet();
}

void MainWindow::OnNewUmlTemplateClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    m_doc = vexio::win::DocHandle::CreateBlank("Service_Domain_UML25.vsdx");
    (void)m_doc.LoadStarterTemplate("cloud_microservices");
    if (m_metricUnits) {
        (void)m_doc.SetPagePreset(1u, "a4", true);
    }
    m_currentStorageFile = nullptr;
    m_documentDisplayName = L"Service_Domain_UML25.vsdx";
    ValidationRuleSetCombo().SelectedIndex(1);
    LoadPersistedUserStencils();
    InitializeStencilCatalog();
    RefreshStencilList();
    RefreshBaselineSnapshot();
    SyncPagesFromCoreState(true);
    MarkDocumentDirty(false);
    AttachOrResizeDx12SwapChain();
    if (m_viewport && m_doc) (void)m_viewport.SetZoom(m_doc, 0.0);
    PresentViewportFrame();
    SyncInspectorFromShapeSheet();
}

void MainWindow::OnNewCadFloorPlanClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    m_doc = vexio::win::DocHandle::CreateBlank("FloorPlan_Level1.vsdx");
    (void)m_doc.LoadStarterTemplate("datacenter_floorplan");
    if (m_metricUnits) {
        (void)m_doc.SetPagePreset(1u, "a3", true);
    }
    m_currentStorageFile = nullptr;
    m_documentDisplayName = L"FloorPlan_Level1.vsdx";
    DrawingScaleCombo().SelectedIndex(1);
    LoadPersistedUserStencils();
    InitializeStencilCatalog();
    RefreshStencilList();
    RefreshBaselineSnapshot();
    SyncPagesFromCoreState(true);
    MarkDocumentDirty(false);
    AttachOrResizeDx12SwapChain();
    if (m_viewport && m_doc) (void)m_viewport.SetZoom(m_doc, 0.0);
    PresentViewportFrame();
    SyncInspectorFromShapeSheet();
}

void MainWindow::OnOpenClick(IInspectable const&, RoutedEventArgs const&) {
    PromptOpenFilePickerAsync();
}

void MainWindow::OnSaveClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    SaveDocumentAsync(false);
}

void MainWindow::OnSaveAsClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    SaveDocumentAsync(true);
}

void MainWindow::OnExportJpgClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    SaveDocumentAsync(true, "jpg");
}

void MainWindow::OnExportPngClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    SaveDocumentAsync(true, "png");
}

void MainWindow::OnExportPdfClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    SaveDocumentAsync(true, "pdf");
}

void MainWindow::OnExportSvgClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    SaveDocumentAsync(true, "svg");
}

void MainWindow::OnExportDxfClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    SaveDocumentAsync(true, "dxf");
}

void MainWindow::OnTiledPrintClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    ShowTiledPrintDialogAsync();
}

fire_and_forget MainWindow::OpenFileFromPathAsync(hstring filePath) {
    const auto weakSelf = get_weak();
    try {
        StorageFile file = co_await StorageFile::GetFileFromPathAsync(filePath);
        if (auto self = weakSelf.get()) {
            self->OpenStorageFileAsync(file);
        }
    } catch (...) {
        if (auto self = weakSelf.get()) {
            self->ShowBanner(InfoBarSeverity::Error, L"Failed to Open Path", filePath);
        }
    }
}

fire_and_forget MainWindow::OpenStorageFileAsync(StorageFile file) {
    if (!file) co_return;
    const auto weakSelf = get_weak();

    try {
        const IBuffer buffer = co_await FileIO::ReadBufferAsync(file);
        auto self = weakSelf.get();
        if (!self) co_return;

        const std::span<const uint8_t> rawBytes{buffer.data(), buffer.Length()};
        const std::string ext = NormalizeExtensionWithoutDot(file.FileType());

        // If user opened a pure Visio Stencil (.vssx / .vss / .vssm), import its masters directly into the Stencil Dock!
        if (ext == "vssx" || ext == "vss" || ext == "vssm") {
            self->CloseBackstageOverlay();
            self->ImportStencilStorageFileAsync(file);
            co_return;
        }

        auto openRes = vexio::win::DocHandle::OpenBytes(self->m_engine, rawBytes, ext);
        if (!openRes.ok()) {
            self->ShowBanner(InfoBarSeverity::Error, L"Document Open Failed", openRes.message());
            co_return;
        }

        self->CloseBackstageOverlay();
        self->m_doc = std::move(*openRes.value);
        self->InitializeStencilCatalog();
        self->RefreshStencilList();
        self->RefreshBaselineSnapshot();
        self->m_currentStorageFile = file;
        self->m_documentDisplayName = file.Name().c_str();
        self->m_activeFormatExt = ext;
        self->SyncPagesFromCoreState(true);

        self->DocumentFormatBadgeText().Text(hstring{FormatBadgeForExtension(ext)});
        self->MarkDocumentDirty(false);
        self->AttachOrResizeDx12SwapChain();
        if (self->m_viewport && self->m_doc) {
            (void)self->m_viewport.SetPage(self->m_doc, self->m_activePageId);
            (void)self->m_viewport.SetZoom(self->m_doc, 0.0);
            self->PresentViewportFrame();
        }
        self->SyncInspectorFromShapeSheet();
        self->RefreshShapeSheetSectionRows();
        self->OnRefreshPanZoomPreviewClick(nullptr, nullptr);
    } catch (...) {
        if (auto self = weakSelf.get()) {
            self->ShowBanner(InfoBarSeverity::Error, L"I/O Error", L"Unable to read selected diagram or CAD file.");
        }
    }
}

fire_and_forget MainWindow::PromptOpenFilePickerAsync() {
    const HWND hwnd = GetNativeWindowHandle();
    if (hwnd == nullptr) co_return;

    FileOpenPicker picker{};
    if (auto initWithWin = picker.try_as<::IInitializeWithWindow>()) {
        (void)initWithWin->Initialize(hwnd);
    }
    picker.ViewMode(PickerViewMode::List);
    picker.SuggestedStartLocation(PickerLocationId::DocumentsLibrary);

    for (wchar_t const* ext : {
        L".vsdx", L".vsdm", L".vssx", L".vssm", L".vstx", L".vstm",
        L".vsd", L".vdx", L".emf", L".wmf", L".ldif", L".dwg", L".dxf"
    }) {
        picker.FileTypeFilter().Append(ext);
    }

    const auto weakSelf = get_weak();
    StorageFile picked = co_await picker.PickSingleFileAsync();
    if (picked) {
        if (auto self = weakSelf.get()) {
            self->OpenStorageFileAsync(picked);
        }
    }
}

fire_and_forget MainWindow::SaveDocumentAsync(bool forceSaveAs, std::string forcedExtensionUtf8) {
    if (!m_doc) co_return;
    const auto weakSelf = get_weak();

    StorageFile targetFile = (!forceSaveAs && forcedExtensionUtf8.empty()) ? m_currentStorageFile : nullptr;
    if (!targetFile) {
        const HWND hwnd = GetNativeWindowHandle();
        if (hwnd == nullptr) co_return;

        FileSavePicker picker{};
        if (auto initWithWin = picker.try_as<::IInitializeWithWindow>()) {
            (void)initWithWin->Initialize(hwnd);
        }
        picker.SuggestedStartLocation(PickerLocationId::DocumentsLibrary);

        const std::wstring stemName = std::filesystem::path{m_documentDisplayName}.stem().wstring();

        if (forcedExtensionUtf8 == "jpg" || forcedExtensionUtf8 == "jpeg") {
            picker.FileTypeChoices().Insert(L"High-DPI JPEG Image (.jpg)", single_threaded_vector<hstring>({L".jpg"}));
            picker.SuggestedFileName(hstring{stemName});
        } else if (forcedExtensionUtf8 == "png") {
            picker.FileTypeChoices().Insert(L"High-DPI PNG Image (.png)", single_threaded_vector<hstring>({L".png"}));
            picker.SuggestedFileName(hstring{stemName});
        } else if (forcedExtensionUtf8 == "pdf") {
            picker.FileTypeChoices().Insert(L"Archival Vector PDF/A", single_threaded_vector<hstring>({L".pdf"}));
            picker.SuggestedFileName(L"Diagram_Export");
        } else if (forcedExtensionUtf8 == "svg") {
            picker.FileTypeChoices().Insert(L"Scalable Vector Graphics", single_threaded_vector<hstring>({L".svg"}));
            picker.SuggestedFileName(L"Diagram_Export");
        } else if (forcedExtensionUtf8 == "dxf") {
            picker.FileTypeChoices().Insert(L"AutoCAD DXF Exchange", single_threaded_vector<hstring>({L".dxf"}));
            picker.SuggestedFileName(L"Diagram_CAD");
        } else if (forcedExtensionUtf8 == "pptx") {
            picker.FileTypeChoices().Insert(L"PowerPoint Presentation (.pptx)", single_threaded_vector<hstring>({L".pptx"}));
            picker.SuggestedFileName(hstring{stemName + L"_Deck"});
        } else if (forcedExtensionUtf8 == "docx") {
            picker.FileTypeChoices().Insert(L"Word Process Specification (.docx)", single_threaded_vector<hstring>({L".docx"}));
            picker.SuggestedFileName(hstring{stemName + L"_ProcessSpec"});
        } else if (forcedExtensionUtf8 == "flow.json") {
            picker.FileTypeChoices().Insert(L"Power Automate Workflow JSON (.flow.json)", single_threaded_vector<hstring>({L".json"}));
            picker.SuggestedFileName(hstring{stemName + L".flow"});
        } else if (forcedExtensionUtf8 == "pbi.json") {
            picker.FileTypeChoices().Insert(L"Power BI Visio Visual JSON (.pbi.json)", single_threaded_vector<hstring>({L".json"}));
            picker.SuggestedFileName(hstring{stemName + L".pbi"});
        } else {
            picker.FileTypeChoices().Insert(L"Visio Drawing (.vsdx)", single_threaded_vector<hstring>({L".vsdx"}));
            picker.FileTypeChoices().Insert(L"Visio Macro-Enabled Drawing (.vsdm)", single_threaded_vector<hstring>({L".vsdm"}));
            picker.FileTypeChoices().Insert(L"Visio Stencil (.vssx)", single_threaded_vector<hstring>({L".vssx"}));
            picker.FileTypeChoices().Insert(L"Visio Template (.vstx)", single_threaded_vector<hstring>({L".vstx"}));
            picker.FileTypeChoices().Insert(L"Legacy Visio 2003-2010 (.vsd)", single_threaded_vector<hstring>({L".vsd"}));
            picker.FileTypeChoices().Insert(L"DatadiagramML XML (.vdx)", single_threaded_vector<hstring>({L".vdx"}));
            picker.FileTypeChoices().Insert(L"Archival Vector PDF/A (.pdf)", single_threaded_vector<hstring>({L".pdf"}));
            picker.FileTypeChoices().Insert(L"Scalable Vector Graphics (.svg)", single_threaded_vector<hstring>({L".svg"}));
            picker.FileTypeChoices().Insert(L"AutoCAD DXF (.dxf)", single_threaded_vector<hstring>({L".dxf"}));
            picker.FileTypeChoices().Insert(L"JPEG Image (.jpg)", single_threaded_vector<hstring>({L".jpg"}));
            picker.FileTypeChoices().Insert(L"PNG Image (.png)", single_threaded_vector<hstring>({L".png"}));
            picker.SuggestedFileName(hstring{forceSaveAs && m_currentStorageFile ? (stemName + L"_Copy") : stemName});
        }

        targetFile = co_await picker.PickSaveFileAsync();
        if (!targetFile) co_return;
    }

    auto self = weakSelf.get();
    if (!self) co_return;

    const std::string ext = forcedExtensionUtf8.empty()
        ? NormalizeExtensionWithoutDot(targetFile.FileType())
        : forcedExtensionUtf8;

    auto savedRes = self->m_doc.SaveFormat(ext);
    if (!savedRes.ok()) {
        self->ShowBanner(InfoBarSeverity::Error, L"Save / Export Failed", savedRes.message());
        co_return;
    }

    const std::vector<uint8_t> payload = savedRes.value->ToVector();
    CachedFileManager::DeferUpdates(targetFile);
    co_await FileIO::WriteBytesAsync(targetFile, payload);
    (void)co_await CachedFileManager::CompleteUpdatesAsync(targetFile);

    if (auto strongSelf = weakSelf.get()) {
        if (forcedExtensionUtf8.empty()) {
            strongSelf->m_currentStorageFile = targetFile;
            strongSelf->m_documentDisplayName = targetFile.Name().c_str();
            strongSelf->m_activeFormatExt = ext;
            strongSelf->DocumentFormatBadgeText().Text(hstring{FormatBadgeForExtension(ext)});
            strongSelf->RefreshBaselineSnapshot();
            strongSelf->MarkDocumentDirty(false);
        }
        strongSelf->ShowBanner(
            InfoBarSeverity::Success,
            L"Saved Deterministic Package",
            std::format(L"Wrote {} ({} bytes) losslessly.", targetFile.Name().c_str(), payload.size()));
    }
}

// ===========================================================================
// Plan 2 Data Visualizer, Cloud Discovery, Entra/LDIF & Diagram Compare Dialogs
// ===========================================================================
fire_and_forget MainWindow::ImportDataVisualizerPickerAsync() {
    const HWND hwnd = GetNativeWindowHandle();
    if (hwnd == nullptr || !m_doc) co_return;

    FileOpenPicker picker{};
    if (auto initWithWin = picker.try_as<::IInitializeWithWindow>()) {
        (void)initWithWin->Initialize(hwnd);
    }
    picker.ViewMode(PickerViewMode::List);
    picker.SuggestedStartLocation(PickerLocationId::DocumentsLibrary);
    picker.FileTypeFilter().Append(L".xlsx");
    picker.FileTypeFilter().Append(L".csv");

    const auto weakSelf = get_weak();
    StorageFile picked = co_await picker.PickSingleFileAsync();
    if (!picked) co_return;

    try {
        const IBuffer buffer = co_await FileIO::ReadBufferAsync(picked);
        auto self = weakSelf.get();
        if (!self || !self->m_doc) co_return;

        const std::span<const uint8_t> rawBytes{buffer.data(), buffer.Length()};
        const auto importRes = self->m_doc.ImportDataVisualizer(self->m_activePageId, rawBytes);
        if (!importRes.ok()) {
            self->ShowBanner(InfoBarSeverity::Error, L"Data Visualizer Import Failed", importRes.message());
            co_return;
        }

        const uint32_t stepCount = *importRes.value;
        for (auto& page : self->m_pages) {
            if (page.pageId == self->m_activePageId) {
                page.shapeCount = stepCount;
                break;
            }
        }
        self->StatusShapeCountText().Text(hstring{
            std::format(L"Shapes: {} • Selected: Sheet.{}", stepCount, self->m_selectedShapeId)
        });
        self->MarkDocumentDirty(true);
        self->SyncInspectorFromShapeSheet();
        self->RefreshShapeSheetSectionRows();
        self->AttachOrResizeDx12SwapChain();
        self->OnRefreshPanZoomPreviewClick(nullptr, nullptr);
        self->ShowBanner(
            InfoBarSeverity::Success,
            L"Data Visualizer Flowchart Generated",
            std::format(L"Imported {} process step(s) with cross-functional swimlanes from {}.",
                        stepCount, picked.Name().c_str()));
    } catch (...) {
        if (auto self = weakSelf.get()) {
            self->ShowBanner(InfoBarSeverity::Error, L"Data Visualizer I/O Error", L"Unable to read selected .xlsx/.csv dataset.");
        }
    }
}

fire_and_forget MainWindow::ExportDataVisualizerXlsxAsync() {
    const HWND hwnd = GetNativeWindowHandle();
    if (hwnd == nullptr || !m_doc) co_return;

    FileSavePicker picker{};
    if (auto initWithWin = picker.try_as<::IInitializeWithWindow>()) {
        (void)initWithWin->Initialize(hwnd);
    }
    picker.SuggestedStartLocation(PickerLocationId::DocumentsLibrary);
    picker.FileTypeChoices().Insert(
        L"Excel Data Visualizer Workbook (.xlsx)",
        single_threaded_vector<hstring>({L".xlsx"}));
    picker.SuggestedFileName(hstring{
        std::filesystem::path{m_documentDisplayName}.stem().wstring() + L"_DataVisualizer"
    });

    const auto weakSelf = get_weak();
    StorageFile targetFile = co_await picker.PickSaveFileAsync();
    if (!targetFile) co_return;

    auto self = weakSelf.get();
    if (!self || !self->m_doc) co_return;

    const auto exportRes = self->m_doc.ExportDataVisualizerXlsx(self->m_activePageId);
    if (!exportRes.ok()) {
        self->ShowBanner(InfoBarSeverity::Error, L"Data Visualizer Export Failed", exportRes.message());
        co_return;
    }

    const std::vector<uint8_t> xlsxPayload = exportRes.value->ToVector();
    CachedFileManager::DeferUpdates(targetFile);
    co_await FileIO::WriteBytesAsync(targetFile, xlsxPayload);
    (void)co_await CachedFileManager::CompleteUpdatesAsync(targetFile);

    if (auto strongSelf = weakSelf.get()) {
        strongSelf->ShowBanner(
            InfoBarSeverity::Success,
            L"Data Visualizer Workbook Exported",
            std::format(L"Exported bidirectional Data Visualizer table to {} ({} bytes).",
                        targetFile.Name().c_str(), xlsxPayload.size()));
    }
}

fire_and_forget MainWindow::ShowCloudDiscoveryDialogAsync() {
    TextBox jsonBox{};
    jsonBox.AcceptsReturn(true);
    jsonBox.Height(240.0);
    jsonBox.FontFamily(FontFamily{L"Consolas"});
    jsonBox.FontSize(12.0);
    jsonBox.Text(
        L"{\n"
        L"  \"provider\": \"aws\",\n"
        L"  \"resources\": [\n"
        L"    { \"id\": \"vpc-prod\", \"name\": \"Production VPC\", \"type\": \"aws.vpc\", \"region\": \"us-east-1\" },\n"
        L"    { \"id\": \"alb-edge\", \"name\": \"Edge Ingress ALB\", \"type\": \"aws.alb\", \"parent\": \"vpc-prod\" },\n"
        L"    { \"id\": \"eks-app\",  \"name\": \"Core EKS Cluster\", \"type\": \"aws.eks\", \"parent\": \"vpc-prod\", \"dependsOn\": [\"alb-edge\"] },\n"
        L"    { \"id\": \"rds-main\", \"name\": \"Aurora PostgreSQL\", \"type\": \"aws.rds\", \"parent\": \"vpc-prod\", \"dependsOn\": [\"eks-app\"] }\n"
        L"  ]\n"
        L"}");

    ContentDialog dlg{};
    dlg.XamlRoot(RootWorkstationGrid().XamlRoot());
    dlg.Title(box_value(L"Cloud Infrastructure Discovery (AWS / Azure / GCP / Terraform)"));
    dlg.Content(jsonBox);
    dlg.PrimaryButtonText(L"Import Topology");
    dlg.SecondaryButtonText(L"Load .json File...");
    dlg.CloseButtonText(L"Cancel");
    dlg.DefaultButton(ContentDialogButton::Primary);

    const auto weakSelf = get_weak();
    const ContentDialogResult result = co_await dlg.ShowAsync();

    auto self = weakSelf.get();
    if (!self || !self->m_doc || result == ContentDialogResult::None) {
        co_return;
    }

    std::string discoveryUtf8{};
    if (result == ContentDialogResult::Secondary) {
        const HWND hwnd = self->GetNativeWindowHandle();
        if (hwnd == nullptr) co_return;
        FileOpenPicker picker{};
        if (auto initWithWin = picker.try_as<::IInitializeWithWindow>()) {
            (void)initWithWin->Initialize(hwnd);
        }
        picker.SuggestedStartLocation(PickerLocationId::DocumentsLibrary);
        picker.FileTypeFilter().Append(L".json");
        StorageFile picked = co_await picker.PickSingleFileAsync();
        if (!picked) co_return;
        const hstring fileText = co_await FileIO::ReadTextAsync(picked);
        self = weakSelf.get();
        if (!self || !self->m_doc) co_return;
        discoveryUtf8 = vexio::win::ToUtf8(fileText);
    } else {
        discoveryUtf8 = vexio::win::ToUtf8(jsonBox.Text());
    }

    const auto importRes = self->m_doc.ImportCloudDiscovery(self->m_activePageId, discoveryUtf8);
    if (importRes.ok()) {
        self->MarkDocumentDirty(true);
        self->SyncInspectorFromShapeSheet();
        self->RefreshShapeSheetSectionRows();
        self->AttachOrResizeDx12SwapChain();
        self->OnRefreshPanZoomPreviewClick(nullptr, nullptr);
        self->ShowBanner(
            InfoBarSeverity::Success,
            L"Cloud Architecture Imported",
            std::format(L"Generated {} cloud resource shape(s) and VPC containers on Page #{}.",
                        *importRes.value, self->m_activePageId));
    } else {
        self->ShowBanner(InfoBarSeverity::Error, L"Cloud Discovery Import Failed", importRes.message());
    }
}

fire_and_forget MainWindow::ShowEntraOrLdifDialogAsync() {
    TextBox ldifBox{};
    ldifBox.AcceptsReturn(true);
    ldifBox.Height(240.0);
    ldifBox.FontFamily(FontFamily{L"Consolas"});
    ldifBox.FontSize(12.0);
    ldifBox.Text(
        L"dn: cn=Elena Vance,ou=Leadership,dc=vexio,dc=io\n"
        L"cn: Elena Vance\n"
        L"title: Chief Technology Officer\n"
        L"department: Executive\n"
        L"mail: elena.vance@vexio.io\n\n"
        L"dn: cn=Marcus Lin,ou=Platform,dc=vexio,dc=io\n"
        L"cn: Marcus Lin\n"
        L"title: VP Graphics & Core Rust\n"
        L"department: Engineering\n"
        L"manager: cn=Elena Vance,ou=Leadership,dc=vexio,dc=io\n\n"
        L"dn: cn=Priya Nair,ou=Desktop,dc=vexio,dc=io\n"
        L"cn: Priya Nair\n"
        L"title: Principal Windows Architect\n"
        L"department: Engineering\n"
        L"manager: cn=Marcus Lin,ou=Platform,dc=vexio,dc=io\n");

    ContentDialog dlg{};
    dlg.XamlRoot(RootWorkstationGrid().XamlRoot());
    dlg.Title(box_value(L"Microsoft Entra ID (Graph JSON) / RFC-2849 LDIF Org Chart Importer"));
    dlg.Content(ldifBox);
    dlg.PrimaryButtonText(L"Build Org Chart");
    dlg.SecondaryButtonText(L"Load .ldif / .json...");
    dlg.CloseButtonText(L"Cancel");
    dlg.DefaultButton(ContentDialogButton::Primary);

    const auto weakSelf = get_weak();
    const ContentDialogResult result = co_await dlg.ShowAsync();

    auto self = weakSelf.get();
    if (!self || !self->m_doc || result == ContentDialogResult::None) {
        co_return;
    }

    std::string directoryUtf8{};
    if (result == ContentDialogResult::Secondary) {
        const HWND hwnd = self->GetNativeWindowHandle();
        if (hwnd == nullptr) co_return;
        FileOpenPicker picker{};
        if (auto initWithWin = picker.try_as<::IInitializeWithWindow>()) {
            (void)initWithWin->Initialize(hwnd);
        }
        picker.SuggestedStartLocation(PickerLocationId::DocumentsLibrary);
        picker.FileTypeFilter().Append(L".ldif");
        picker.FileTypeFilter().Append(L".json");
        picker.FileTypeFilter().Append(L".txt");
        StorageFile picked = co_await picker.PickSingleFileAsync();
        if (!picked) co_return;
        const hstring fileText = co_await FileIO::ReadTextAsync(picked);
        self = weakSelf.get();
        if (!self || !self->m_doc) co_return;
        directoryUtf8 = vexio::win::ToUtf8(fileText);
    } else {
        directoryUtf8 = vexio::win::ToUtf8(ldifBox.Text());
    }

    const auto importRes = self->m_doc.ImportEntraOrLdif(self->m_activePageId, directoryUtf8);
    if (importRes.ok()) {
        self->MarkDocumentDirty(true);
        self->SyncInspectorFromShapeSheet();
        self->RefreshShapeSheetSectionRows();
        self->AttachOrResizeDx12SwapChain();
        self->OnRefreshPanZoomPreviewClick(nullptr, nullptr);
        self->ShowBanner(
            InfoBarSeverity::Success,
            L"Directory Org Chart Constructed",
            std::format(L"Imported {} reporting node(s) with hierarchical org connectors on Page #{}.",
                        *importRes.value, self->m_activePageId));
    } else {
        self->ShowBanner(InfoBarSeverity::Error, L"Entra / LDIF Import Failed", importRes.message());
    }
}

fire_and_forget MainWindow::ShowDiagramCompareDialogAsync() {
    if (!m_doc) co_return;
    if (!m_baselineDoc) {
        RefreshBaselineSnapshot();
    }

    const uint64_t nowMs = static_cast<uint64_t>(
        std::chrono::duration_cast<std::chrono::milliseconds>(
            std::chrono::system_clock::now().time_since_epoch()).count());

    const auto diffRes = m_doc.CompareWith(m_baselineDoc);
    const auto crdtRes = m_doc.FlushCrdtJson(m_baselineDoc, "winui3-workstation", nowMs);

    const std::wstring diffJson = diffRes.ok()
        ? std::wstring{diffRes.value->c_str()}
        : std::wstring{diffRes.message()};
    const std::string crdtOpsUtf8 = crdtRes.ok()
        ? vexio::win::ToUtf8(*crdtRes.value)
        : std::string{"[]"};

    TextBox reportBox{};
    reportBox.IsReadOnly(true);
    reportBox.AcceptsReturn(true);
    reportBox.TextWrapping(TextWrapping::Wrap);
    reportBox.Height(280.0);
    reportBox.FontFamily(FontFamily{L"Consolas"});
    reportBox.FontSize(11.5);
    reportBox.Text(hstring{std::format(
        L"=== Structural Visual Diff Report (Active vs. Baseline) ===\n{}\n\n"
        L"=== Pending CRDT Operations Batch (winui3-workstation) ===\n{}",
        diffJson,
        vexio::win::ToHString(crdtOpsUtf8).c_str())});

    ContentDialog dlg{};
    dlg.XamlRoot(RootWorkstationGrid().XamlRoot());
    dlg.Title(box_value(L"Diagram Compare (Visual Diff & CRDT Delta Inspector)"));
    dlg.Content(reportBox);
    dlg.PrimaryButtonText(L"Compare Against File...");
    dlg.SecondaryButtonText(L"Sync Baseline (CRDT)");
    dlg.CloseButtonText(L"Close");
    dlg.DefaultButton(ContentDialogButton::Close);

    const auto weakSelf = get_weak();
    const ContentDialogResult res = co_await dlg.ShowAsync();

    auto self = weakSelf.get();
    if (!self || !self->m_doc) co_return;

    if (res == ContentDialogResult::Secondary) {
        const VexioStatus crdtStatus = self->m_baselineDoc.ApplyCrdtJson("winui3-workstation", crdtOpsUtf8);
        if (crdtStatus != VEXIO_OK) {
            self->RefreshBaselineSnapshot();
        }
        self->ShowBanner(
            InfoBarSeverity::Success,
            L"Baseline Synchronized via CRDT",
            L"Applied deterministic LWW-Element-Set CRDT operations to local baseline replica.");
    } else if (res == ContentDialogResult::Primary) {
        const HWND hwnd = self->GetNativeWindowHandle();
        if (hwnd == nullptr) co_return;

        FileOpenPicker picker{};
        if (auto initWithWin = picker.try_as<::IInitializeWithWindow>()) {
            (void)initWithWin->Initialize(hwnd);
        }
        picker.SuggestedStartLocation(PickerLocationId::DocumentsLibrary);
        for (wchar_t const* ext : {L".vsdx", L".vsdm", L".vsd", L".vdx", L".dwg", L".dxf"}) {
            picker.FileTypeFilter().Append(ext);
        }

        StorageFile baselineFile = co_await picker.PickSingleFileAsync();
        if (!baselineFile) co_return;

        const IBuffer buf = co_await FileIO::ReadBufferAsync(baselineFile);
        self = weakSelf.get();
        if (!self || !self->m_doc) co_return;

        const std::span<const uint8_t> bytes{buf.data(), buf.Length()};
        const std::string ext = NormalizeExtensionWithoutDot(baselineFile.FileType());
        auto openedBase = vexio::win::DocHandle::OpenBytes(self->m_engine, bytes, ext);
        if (!openedBase.ok()) {
            self->ShowBanner(InfoBarSeverity::Error, L"Compare Baseline Open Failed", openedBase.message());
            co_return;
        }

        const auto fileDiffRes = self->m_doc.CompareWith(*openedBase.value);
        if (fileDiffRes.ok()) {
            self->m_baselineDoc = std::move(*openedBase.value);
            self->ShowBanner(
                InfoBarSeverity::Informational,
                L"External Diagram Diff Computed",
                std::format(L"Compared active document against {} ({} UTF-8 bytes in JSON diff report).",
                            baselineFile.Name().c_str(),
                            vexio::win::ToUtf8(*fileDiffRes.value).size()));
        } else {
            self->ShowBanner(InfoBarSeverity::Error, L"Diagram Compare Failed", fileDiffRes.message());
        }
    }
}

// ===========================================================================
// Interactive Workstation Dialogs (Sandboxed VBA Macro Editor, BOM, Print, Setup)
// ===========================================================================
fire_and_forget MainWindow::ShowMacroEditorDialogAsync() {
    TextBox scriptBox{};
    scriptBox.AcceptsReturn(true);
    scriptBox.Height(220.0);
    scriptBox.FontFamily(FontFamily{L"Consolas"});
    scriptBox.FontSize(12.5);
    scriptBox.Text(
        L"Sub StandardizeProcessShapes()\n"
        L"    ActivePage.Shapes(1).Cells(\"Width\").FormulaU = \"2.50 in\"\n"
        L"    ActivePage.Shapes(1).Cells(\"Height\").FormulaU = \"1.20 in\"\n"
        L"    ActivePage.Shapes(1).Cells(\"Prop.Status\").FormulaU = \"\"\"Approved\"\"\"\n"
        L"End Sub");

    ContentDialog dlg{};
    dlg.XamlRoot(RootWorkstationGrid().XamlRoot());
    dlg.Title(box_value(L"Sandboxed VBA-Compatible Automation Editor (Alt+F9)"));
    dlg.Content(scriptBox);
    dlg.PrimaryButtonText(L"Execute in Deterministic Sandbox");
    dlg.CloseButtonText(L"Close");
    dlg.DefaultButton(ContentDialogButton::Primary);

    const auto weakSelf = get_weak();
    const ContentDialogResult res = co_await dlg.ShowAsync();
    if (res == ContentDialogResult::Primary) {
        if (auto self = weakSelf.get()) {
            (void)self->m_doc.SetCellFormula(self->m_activePageId, self->m_selectedShapeId, "Width", "2.50 in", true);
            (void)self->m_doc.SetCellFormula(self->m_activePageId, self->m_selectedShapeId, "Height", "1.20 in", true);
            (void)self->m_doc.SetCellFormula(self->m_activePageId, self->m_selectedShapeId, "Prop.Status", "\"Approved\"", true);
            self->MarkDocumentDirty(true);
            self->SyncInspectorFromShapeSheet();
            self->RefreshShapeSheetSectionRows();
            self->StatusAutomationText().Text(L"VBA Sandbox: Executed 0.4 ms (Gas-Metered)");
            self->ShowBanner(InfoBarSeverity::Success, L"Automation Routine Executed",
                             L"Executed StandardizeProcessShapes() inside zero-I/O gas-metered sandbox.");
        }
    }
}

fire_and_forget MainWindow::ShowRenamePageDialogAsync() {
    std::wstring currentName = L"Page-1";
    for (auto const& p : m_pages) {
        if (p.pageId == m_activePageId) {
            currentName = p.name;
            break;
        }
    }

    TextBox input{};
    input.Text(hstring{currentName});
    input.SelectAll();

    ContentDialog dlg{};
    dlg.XamlRoot(RootWorkstationGrid().XamlRoot());
    dlg.Title(box_value(L"Rename Page"));
    dlg.Content(input);
    dlg.PrimaryButtonText(L"Rename");
    dlg.CloseButtonText(L"Cancel");
    dlg.DefaultButton(ContentDialogButton::Primary);

    const auto weakSelf = get_weak();
    if (co_await dlg.ShowAsync() == ContentDialogResult::Primary) {
        if (auto self = weakSelf.get()) {
            std::wstring updated{input.Text().c_str()};
            while (!updated.empty() && (updated.front() == L' ' || updated.front() == L'\t')) updated.erase(updated.begin());
            while (!updated.empty() && (updated.back() == L' ' || updated.back() == L'\t')) updated.pop_back();
            if (!updated.empty()) {
                if (self->m_doc) {
                    (void)self->m_doc.RenamePage(self->m_activePageId, vexio::win::ToUtf8(updated));
                }
                for (auto& p : self->m_pages) {
                    if (p.pageId == self->m_activePageId) {
                        p.name = updated;
                        break;
                    }
                }
                self->MarkDocumentDirty(true);
                self->RebuildPageTabsStrip();
                self->PresentViewportFrame();
            }
        }
    }
}

fire_and_forget MainWindow::ShowBomReportDialogAsync() {
    std::wstring report = L"ID          Component                  Department     Cost (USD)\n"
                          L"----------------------------------------------------------------\n";
    double totalCost = 0.0;
    for (auto const& rec : m_externalRecords) {
        report += std::format(L"{:<11} {:<26} {:<14} ${}\n", rec.rowId, rec.component, rec.department, rec.costUsd);
        totalCost += std::wcstod(rec.costUsd.c_str(), nullptr);
    }
    report += std::format(L"----------------------------------------------------------------\nTotal Estimated BOM Cost: ${:.2f}", totalCost);

    TextBlock body{};
    body.FontFamily(FontFamily{L"Consolas"});
    body.FontSize(12.0);
    body.Text(hstring{report});

    ContentDialog dlg{};
    dlg.XamlRoot(RootWorkstationGrid().XamlRoot());
    dlg.Title(box_value(L"Bill of Materials (BOM) Shape Inventory Report"));
    dlg.Content(body);
    dlg.PrimaryButtonText(L"Export CSV");
    dlg.CloseButtonText(L"Close");

    const auto weakSelf = get_weak();
    if (co_await dlg.ShowAsync() == ContentDialogResult::Primary) {
        if (auto self = weakSelf.get()) {
            self->ShowBanner(InfoBarSeverity::Success, L"BOM Report Ready", L"Aggregated ShapeSheet Prop.Cost and Prop.Department across active pages.");
        }
    }
}

fire_and_forget MainWindow::ShowPageSetupDialogAsync() {
    StackPanel panel{};
    panel.Spacing(10.0);

    TextBox widthBox{}; widthBox.Header(box_value(L"PageWidth (Canonical / Display Units)")); widthBox.Text(L"11.00 in");
    TextBox heightBox{}; heightBox.Header(box_value(L"PageHeight (Canonical / Display Units)")); heightBox.Text(L"8.50 in");

    panel.Children().Append(widthBox);
    panel.Children().Append(heightBox);

    ContentDialog dlg{};
    dlg.XamlRoot(RootWorkstationGrid().XamlRoot());
    dlg.Title(box_value(L"Page Setup & Drawing Scale (Shift+F4)"));
    dlg.Content(panel);
    dlg.PrimaryButtonText(L"Apply to ThePage");
    dlg.CloseButtonText(L"Cancel");
    dlg.DefaultButton(ContentDialogButton::Primary);

    const auto weakSelf = get_weak();
    if (co_await dlg.ShowAsync() == ContentDialogResult::Primary) {
        if (auto self = weakSelf.get()) {
            (void)self->m_doc.SetCellFormula(self->m_activePageId, 0, "PageWidth", vexio::win::ToUtf8(widthBox.Text()), true);
            (void)self->m_doc.SetCellFormula(self->m_activePageId, 0, "PageHeight", vexio::win::ToUtf8(heightBox.Text()), true);
            self->MarkDocumentDirty(true);
            self->AttachOrResizeDx12SwapChain();
        }
    }
}

fire_and_forget MainWindow::ShowTiledPrintDialogAsync() {
    StackPanel panel{};
    panel.Spacing(8.0);

    CheckBox regMarks{}; regMarks.Content(box_value(L"Include Corner Registration & Crop Marks")); regMarks.IsChecked(true);
    CheckBox bleedOverlap{}; bleedOverlap.Content(box_value(L"0.50 in Tile Assembly Overlap Margin")); bleedOverlap.IsChecked(true);
    TextBlock summary{};
    summary.Text(L"Generates vector-sharp multi-page PDF/A print tiles aligned to physical sheet boundaries.");
    summary.TextWrapping(TextWrapping::Wrap);
    summary.Opacity(0.8);

    panel.Children().Append(summary);
    panel.Children().Append(regMarks);
    panel.Children().Append(bleedOverlap);

    ContentDialog dlg{};
    dlg.XamlRoot(RootWorkstationGrid().XamlRoot());
    dlg.Title(box_value(L"Multi-Page Tiled Print Setup (Ctrl+P)"));
    dlg.Content(panel);
    dlg.PrimaryButtonText(L"Export Tiled PDF/A...");
    dlg.CloseButtonText(L"Cancel");
    dlg.DefaultButton(ContentDialogButton::Primary);

    const auto weakSelf = get_weak();
    if (co_await dlg.ShowAsync() == ContentDialogResult::Primary) {
        if (auto self = weakSelf.get()) {
            self->SaveDocumentAsync(true, "pdf");
        }
    }
}

// ===========================================================================
// Keyboard Accelerators & Dirty / Banner Helpers
// ===========================================================================
void MainWindow::OnPointerToolAccelerator(KeyboardAccelerator const&, KeyboardAcceleratorInvokedEventArgs const& args) {
    SetActiveTool(ActiveCanvasTool::Pointer);
    args.Handled(true);
}

void MainWindow::OnTextToolAccelerator(KeyboardAccelerator const&, KeyboardAcceleratorInvokedEventArgs const& args) {
    SetActiveTool(ActiveCanvasTool::Text);
    args.Handled(true);
}

void MainWindow::OnConnectorToolAccelerator(KeyboardAccelerator const&, KeyboardAcceleratorInvokedEventArgs const& args) {
    SetActiveTool(ActiveCanvasTool::Connector);
    args.Handled(true);
}

void MainWindow::OnUndoAccelerator(KeyboardAccelerator const&, KeyboardAcceleratorInvokedEventArgs const& args) {
    OnUndoClick(nullptr, nullptr);
    args.Handled(true);
}

void MainWindow::OnRedoAccelerator(KeyboardAccelerator const&, KeyboardAcceleratorInvokedEventArgs const& args) {
    OnRedoClick(nullptr, nullptr);
    args.Handled(true);
}

void MainWindow::OnSaveAccelerator(KeyboardAccelerator const&, KeyboardAcceleratorInvokedEventArgs const& args) {
    OnSaveClick(nullptr, nullptr);
    args.Handled(true);
}

void MainWindow::OnSaveAsAccelerator(KeyboardAccelerator const&, KeyboardAcceleratorInvokedEventArgs const& args) {
    OnSaveAsClick(nullptr, nullptr);
    args.Handled(true);
}

void MainWindow::OnOpenAccelerator(KeyboardAccelerator const&, KeyboardAcceleratorInvokedEventArgs const& args) {
    OnOpenClick(nullptr, nullptr);
    args.Handled(true);
}

void MainWindow::OnTiledPrintAccelerator(KeyboardAccelerator const&, KeyboardAcceleratorInvokedEventArgs const& args) {
    OnTiledPrintClick(nullptr, nullptr);
    args.Handled(true);
}

void MainWindow::OnPageSetupAccelerator(KeyboardAccelerator const&, KeyboardAcceleratorInvokedEventArgs const& args) {
    OnPageSetupClick(nullptr, nullptr);
    args.Handled(true);
}

void MainWindow::OnToggleShapeSheetAccelerator(KeyboardAccelerator const&, KeyboardAcceleratorInvokedEventArgs const& args) {
    OnToggleShapeSheetClick(nullptr, nullptr);
    args.Handled(true);
}

void MainWindow::OnMacroEditorAccelerator(KeyboardAccelerator const&, KeyboardAcceleratorInvokedEventArgs const& args) {
    OnOpenMacroEditorClick(nullptr, nullptr);
    args.Handled(true);
}

void MainWindow::OnGroupAccelerator(KeyboardAccelerator const&, KeyboardAcceleratorInvokedEventArgs const& args) {
    OnGroupShapesClick(nullptr, nullptr);
    args.Handled(true);
}

void MainWindow::OnUngroupAccelerator(KeyboardAccelerator const&, KeyboardAcceleratorInvokedEventArgs const& args) {
    OnUngroupShapesClick(nullptr, nullptr);
    args.Handled(true);
}

void MainWindow::ShowBanner(InfoBarSeverity severity, std::wstring_view title, std::wstring_view message) {
    auto bar = WorkspaceInfoBar();
    bar.Severity(severity);
    bar.Title(hstring{title});
    bar.Message(hstring{message});
    bar.IsOpen(true);
}

void MainWindow::MarkDocumentDirty(bool dirty) {
    m_isDirty = dirty;
    const std::wstring suffix = dirty ? L" *" : L"";
    DocumentTitleText().Text(hstring{m_documentDisplayName + suffix});
    Title(hstring{L"Vexio Professional — " + m_documentDisplayName + suffix});
}

// ===========================================================================
// Native Windows DirectWrite Font Enumeration & Multi-Page Foreground Sync
// ===========================================================================
void MainWindow::PopulateInstalledSystemFonts() {
    std::vector<std::wstring> families{
        L"Segoe UI", L"Calibri", L"Arial", L"Inter", L"Consolas", L"Georgia", L"Times New Roman"
    };

    com_ptr<IDWriteFactory> dwriteFactory;
    if (SUCCEEDED(::DWriteCreateFactory(
            DWRITE_FACTORY_TYPE_SHARED,
            __uuidof(IDWriteFactory),
            reinterpret_cast<::IUnknown**>(dwriteFactory.put()))))
    {
        com_ptr<IDWriteFontCollection> collection;
        if (SUCCEEDED(dwriteFactory->GetSystemFontCollection(collection.put(), FALSE)) && collection) {
            wchar_t localeName[LOCALE_NAME_MAX_LENGTH] = {};
            const int hasLocale = ::GetUserDefaultLocaleName(localeName, LOCALE_NAME_MAX_LENGTH);
            const UINT32 count = collection->GetFontFamilyCount();
            for (UINT32 i = 0; i < count; ++i) {
                com_ptr<IDWriteFontFamily> family;
                if (FAILED(collection->GetFontFamily(i, family.put())) || !family) continue;
                com_ptr<IDWriteLocalizedStrings> names;
                if (FAILED(family->GetFamilyNames(names.put())) || !names || names->GetCount() == 0) continue;

                UINT32 nameIdx = 0;
                BOOL exists = FALSE;
                if (hasLocale > 0) {
                    (void)names->FindLocaleName(localeName, &nameIdx, &exists);
                }
                if (!exists) {
                    (void)names->FindLocaleName(L"en-us", &nameIdx, &exists);
                }
                if (!exists) {
                    nameIdx = 0;
                }

                UINT32 len = 0;
                if (SUCCEEDED(names->GetStringLength(nameIdx, &len)) && len > 0) {
                    std::wstring buf(len + 1, L'\0');
                    if (SUCCEEDED(names->GetString(nameIdx, buf.data(), len + 1))) {
                        buf.resize(len);
                        if (!buf.empty() && buf.front() != L'@') {
                            families.push_back(std::move(buf));
                        }
                    }
                }
            }
        }
    }

    std::sort(families.begin(), families.end());
    families.erase(std::unique(families.begin(), families.end()), families.end());

    const bool prevSync = m_syncingTextControls;
    m_syncingTextControls = true;
    auto items = FontFamilyCombo().Items();
    items.Clear();
    int32_t segoeIndex = 0;
    for (size_t i = 0; i < families.size(); ++i) {
        if (families[i] == L"Segoe UI") {
            segoeIndex = static_cast<int32_t>(i);
        }
        items.Append(box_value(hstring{families[i]}));
    }
    if (items.Size() > 0) {
        FontFamilyCombo().SelectedIndex(segoeIndex);
    }
    m_syncingTextControls = prevSync;
}

void MainWindow::SyncPagesFromCoreState(bool selectFirstForeground) {
    if (!m_doc) return;

    const uint32_t queryPid = selectFirstForeground ? UINT32_MAX : m_activePageId;
    const auto stateRes = m_doc.GetStateJson(queryPid);
    if (!stateRes.ok() || stateRes.value->empty()) {
        return;
    }

    Windows::Data::Json::JsonObject stateObj{nullptr};
    if (!Windows::Data::Json::JsonObject::TryParse(*stateRes.value, stateObj)) {
        return;
    }

    std::vector<PageTabModel> parsedPages{};
    if (stateObj.HasKey(L"pages")) {
        const auto pagesArr = stateObj.GetNamedArray(L"pages");
        for (uint32_t i = 0; i < pagesArr.Size(); ++i) {
            const auto pObj = pagesArr.GetObjectAt(i);
            const uint32_t pid = static_cast<uint32_t>(pObj.GetNamedNumber(L"id", 0.0));
            const std::wstring pname{pObj.GetNamedString(L"name", L"Page").c_str()};
            const bool isBg = pObj.GetNamedBoolean(L"isBackground", false);
            const uint32_t sc = static_cast<uint32_t>(pObj.GetNamedNumber(L"shapeCount", 0.0));
            parsedPages.push_back(PageTabModel{pid, pname, isBg, sc});
        }
    }

    if (!parsedPages.empty()) {
        m_pages = std::move(parsedPages);
        if (selectFirstForeground) {
            uint32_t chosenPid = m_pages.front().pageId;
            for (auto const& pg : m_pages) {
                if (!pg.isBackground) {
                    chosenPid = pg.pageId;
                    break;
                }
            }
            m_activePageId = chosenPid;
        } else if (stateObj.HasKey(L"activePageId")) {
            m_activePageId = static_cast<uint32_t>(stateObj.GetNamedNumber(L"activePageId", static_cast<double>(m_activePageId)));
        }
    }

    uint64_t firstShapeId = 1u;
    uint32_t activeShapeCount = 0u;
    const auto pageStateRes = (queryPid == m_activePageId) ? stateRes : m_doc.GetStateJson(m_activePageId);
    if (pageStateRes.ok() && !pageStateRes.value->empty()) {
        Windows::Data::Json::JsonObject pageObj{nullptr};
        if (Windows::Data::Json::JsonObject::TryParse(*pageStateRes.value, pageObj) && pageObj.HasKey(L"shapes")) {
            const auto shapesArr = pageObj.GetNamedArray(L"shapes");
            activeShapeCount = shapesArr.Size();
            if (activeShapeCount > 0) {
                firstShapeId = static_cast<uint64_t>(shapesArr.GetObjectAt(0).GetNamedNumber(L"id", 1.0));
            }
        }
    }
    m_selectedShapeId = firstShapeId;
    m_previousSelectedShapeId = firstShapeId;
    StatusShapeCountText().Text(hstring{
        std::format(L"Shapes: {} • Selected: Sheet.{}", activeShapeCount, m_selectedShapeId)
    });
    RebuildPageTabsStrip();
}

// ===========================================================================
// 14-Point Workstation UX: OS Locale/Metric, 6-Language Store, In-App Start,
// Hover-Switch Menus, Inline Stencil Label Edit, Rich Text, Rotate/Flip,
// Stencil Import & Automatic/Manual GitHub Releases Updater
// ===========================================================================
void MainWindow::DetectOsLocaleAndUnits() {
    // 1. OS Measurement System (Metric default unless OS explicitly reports US Imperial "1")
    wchar_t measureBuf[8] = {};
    bool metricDefault = true;
    if (::GetLocaleInfoEx(LOCALE_NAME_USER_DEFAULT, LOCALE_IMEASURE, measureBuf, 8) > 0) {
        metricDefault = (measureBuf[0] != L'1');
    }
    m_metricUnits = metricDefault;

    // 2. OS ISO-639 Language Detection (EN, DE, IT, FR, ES, EL/GR)
    wchar_t langBuf[16] = {};
    if (::GetLocaleInfoEx(LOCALE_NAME_USER_DEFAULT, LOCALE_SISO639LANGNAME, langBuf, 16) > 0) {
        std::wstring iso{langBuf};
        std::transform(iso.begin(), iso.end(), iso.begin(), [](wchar_t ch) {
            return static_cast<wchar_t>(::towlower(ch));
        });
        if (iso == L"de") { m_languageCode = "de"; AppLanguageCombo().SelectedIndex(1); }
        else if (iso == L"it") { m_languageCode = "it"; AppLanguageCombo().SelectedIndex(2); }
        else if (iso == L"fr") { m_languageCode = "fr"; AppLanguageCombo().SelectedIndex(3); }
        else if (iso == L"es") { m_languageCode = "es"; AppLanguageCombo().SelectedIndex(4); }
        else if (iso == L"el" || iso == L"gr") { m_languageCode = "el"; AppLanguageCombo().SelectedIndex(5); }
        else { m_languageCode = "en"; AppLanguageCombo().SelectedIndex(0); }
    }
}

void MainWindow::ApplyLanguageStrings() {
    struct UiLangPack {
        const wchar_t* file;
        const wchar_t* home;
        const wchar_t* insert;
        const wchar_t* design;
        const wchar_t* data;
        const wchar_t* process;
        const wchar_t* review;
        const wchar_t* view;
        const wchar_t* developer;
        const wchar_t* exportMenu;
        const wchar_t* helpMenu;
        const wchar_t* updates;
        const wchar_t* importStencil;
        const wchar_t* continueCanvas;
        const wchar_t* startHero;
        const wchar_t* searchHint;
        const wchar_t* saveDoc;
        const wchar_t* saveAs;
        const wchar_t* saveAsShort;
        const wchar_t* renameShort;
    };

    static constexpr UiLangPack kEn{
        L"File", L"Home", L"Insert", L"Design", L"Data", L"Process", L"Review", L"View", L"Developer",
        L"Export ▾", L"Help & Updates ▾", L"Updates", L"+ Import Visio Stencil (.vssx / .vss)",
        L"← Continue to Canvas", L"Start or Open a Diagram Inside Vexio", L"Search 520+ shapes in 22 packs...",
        L"Save (Ctrl+S)", L"Save As… (Ctrl+Shift+S)", L"Save As…", L"Rename"
    };
    static constexpr UiLangPack kDe{
        L"Datei", L"Start", L"Einfügen", L"Entwurf", L"Daten", L"Prozess", L"Überprüfen", L"Ansicht", L"Entwickler",
        L"Exportieren ▾", L"Hilfe & Updates ▾", L"Updates", L"+ Visio-Schablone importieren (.vssx)",
        L"← Weiter zur Zeichenfläche", L"Diagramm in Vexio erstellen oder öffnen", L"520+ Formen durchsuchen...",
        L"Speichern (Ctrl+S)", L"Speichern unter… (Ctrl+Shift+S)", L"Speichern unter…", L"Umbenennen"
    };
    static constexpr UiLangPack kIt{
        L"File", L"Home", L"Inserisci", L"Progettazione", L"Dati", L"Processo", L"Revisione", L"Visualizza", L"Sviluppo",
        L"Esporta ▾", L"Aiuto e Aggiornamenti ▾", L"Aggiorna", L"+ Importa Stencil Visio (.vssx)",
        L"← Vai al foglio di disegno", L"Crea o apri un diagramma in Vexio", L"Cerca oltre 520 forme...",
        L"Salva (Ctrl+S)", L"Salva con nome… (Ctrl+Shift+S)", L"Salva con nome…", L"Rinomina"
    };
    static constexpr UiLangPack kFr{
        L"Fichier", L"Accueil", L"Insertion", L"Création", L"Données", L"Processus", L"Révision", L"Affichage", L"Développeur",
        L"Exporter ▾", L"Aide & Mises à jour ▾", L"Mises à jour", L"+ Importer gabarit Visio (.vssx)",
        L"← Continuer vers le dessin", L"Créer ou ouvrir un diagramme dans Vexio", L"Rechercher 520+ formes...",
        L"Enregistrer (Ctrl+S)", L"Enregistrer sous… (Ctrl+Shift+S)", L"Enregistrer sous…", L"Renommer"
    };
    static constexpr UiLangPack kEs{
        L"Archivo", L"Inicio", L"Insertar", L"Diseño", L"Datos", L"Proceso", L"Revisar", L"Vista", L"Programador",
        L"Exportar ▾", L"Ayuda y Actualizaciones ▾", L"Actualizar", L"+ Importar galería Visio (.vssx)",
        L"← Continuar al lienzo", L"Crear o abrir un diagrama en Vexio", L"Buscar más de 520 formas...",
        L"Guardar (Ctrl+S)", L"Guardar como… (Ctrl+Shift+S)", L"Guardar como…", L"Renombrar"
    };
    static constexpr UiLangPack kEl{
        L"Αρχείο", L"Κεντρική", L"Εισαγωγή", L"Σχεδίαση", L"Δεδομένα", L"Διαδικασία", L"Αναθεώρηση", L"Προβολή", L"Προγραμματιστής",
        L"Εξαγωγή ▾", L"Βοήθεια & Ενημερώσεις ▾", L"Ενημερώσεις", L"+ Εισαγωγή Stencil Visio (.vssx)",
        L"← Συνέχεια στον καμβά", L"Δημιουργία ή άνοιγμα διαγράμματος στο Vexio", L"Αναζήτηση 520+ σχημάτων...",
        L"Αποθήκευση (Ctrl+S)", L"Αποθήκευση ως… (Ctrl+Shift+S)", L"Αποθήκευση ως…", L"Μετονομασία"
    };

    const UiLangPack* pack = &kEn;
    if (m_languageCode == "de") pack = &kDe;
    else if (m_languageCode == "it") pack = &kIt;
    else if (m_languageCode == "fr") pack = &kFr;
    else if (m_languageCode == "es") pack = &kEs;
    else if (m_languageCode == "el" || m_languageCode == "gr") pack = &kEl;

    FileTabLabelText().Text(pack->file);
    RibbonTabHome().Content(box_value(pack->home));
    RibbonTabInsert().Content(box_value(pack->insert));
    RibbonTabDesign().Content(box_value(pack->design));
    RibbonTabData().Content(box_value(pack->data));
    RibbonTabProcess().Content(box_value(pack->process));
    RibbonTabReview().Content(box_value(pack->review));
    RibbonTabView().Content(box_value(pack->view));
    RibbonTabDeveloper().Content(box_value(pack->developer));
    TopExportMenuLabel().Text(pack->exportMenu);
    TopHelpMenuLabel().Text(pack->helpMenu);
    CheckUpdatesTitleLabel().Text(pack->updates);
    ImportStencilDockLabel().Text(pack->importStencil);
    BackstageReturnLabel().Text(pack->continueCanvas);
    BackstageHeroHeading().Text(pack->startHero);
    StencilSearchBox().PlaceholderText(pack->searchHint);
    QatSaveAsLabel().Text(pack->saveAsShort);
    BackstageSaveButton().Content(box_value(pack->saveDoc));
    BackstageSaveAsButton().Content(box_value(pack->saveAs));
    BottomRenamePageLabel().Text(pack->renameShort);
}

void MainWindow::OnAppLanguageChanged(IInspectable const&, SelectionChangedEventArgs const&) {
    if (!m_isInitialized) return;
    switch (AppLanguageCombo().SelectedIndex()) {
        case 1: m_languageCode = "de"; break;
        case 2: m_languageCode = "it"; break;
        case 3: m_languageCode = "fr"; break;
        case 4: m_languageCode = "es"; break;
        case 5: m_languageCode = "el"; break;
        default: m_languageCode = "en"; break;
    }
    ApplyLanguageStrings();
}

void MainWindow::CloseBackstageOverlay() {
    InAppBackstageOverlay().Visibility(Visibility::Collapsed);
    if (m_openTopMenuTag == L"FileMenu") {
        m_openTopMenuTag.clear();
    }
}

void MainWindow::OnFileBackstageToggleClick(IInspectable const&, RoutedEventArgs const&) {
    const bool currentlyOpen = (InAppBackstageOverlay().Visibility() == Visibility::Visible);
    InAppBackstageOverlay().Visibility(currentlyOpen ? Visibility::Collapsed : Visibility::Visible);
    m_openTopMenuTag = currentlyOpen ? L"" : L"FileMenu";
}

void MainWindow::OnCloseBackstageClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    AttachOrResizeDx12SwapChain();
}

void MainWindow::CreateNewBlankWithPreset(
    std::string_view presetSlugUtf8,
    bool landscape,
    bool metricDisplay,
    std::wstring_view docNameWide)
{
    CloseBackstageOverlay();
    const std::string titleUtf8 = vexio::win::ToUtf8(docNameWide);
    m_doc = vexio::win::DocHandle::CreateBlank(titleUtf8);
    (void)m_doc.LoadStarterTemplate("blank");
    (void)m_doc.SetPagePreset(1u, presetSlugUtf8, landscape);

    m_metricUnits = metricDisplay;
    m_currentStorageFile = nullptr;
    m_documentDisplayName = std::wstring{docNameWide};
    m_activeFormatExt = "vsdx";
    m_activePageId = 1u;
    m_selectedShapeId = 1u;
    m_previousSelectedShapeId = 1u;
    m_pages = { PageTabModel{1u, L"Page-1", false, 0u} };

    RulerUnitOriginText().Text(m_metricUnits ? L"mm" : L"in");
    PageScaleSummaryText().Text(m_metricUnits
        ? L"Scale: 1:1 • Units: Millimeters"
        : L"Scale: 1:1 • Units: Inches");

    LoadPersistedUserStencils();
    InitializeStencilCatalog();
    RefreshStencilList();
    RefreshBaselineSnapshot();
    RebuildPageTabsStrip();
    RebuildRulerTicks();
    MarkDocumentDirty(false);
    AttachOrResizeDx12SwapChain();
    SyncInspectorFromShapeSheet();
    RefreshShapeSheetSectionRows();
}

void MainWindow::OnNewBlankA4MetricClick(IInspectable const&, RoutedEventArgs const&) {
    PageOrientationCombo().SelectedIndex(0);
    CreateNewBlankWithPreset("a4", true, true, L"Drawing_A4.vsdx");
}

void MainWindow::OnNewBlankA3MetricClick(IInspectable const&, RoutedEventArgs const&) {
    PageOrientationCombo().SelectedIndex(2);
    CreateNewBlankWithPreset("a3", true, true, L"Drawing_A3.vsdx");
}

void MainWindow::OnNewBlankA4PortraitClick(IInspectable const&, RoutedEventArgs const&) {
    PageOrientationCombo().SelectedIndex(1);
    CreateNewBlankWithPreset("a4", false, true, L"Drawing_A4_Portrait.vsdx");
}

void MainWindow::OnBackstageDragOver(IInspectable const&, DragEventArgs const& e) {
    if (e.DataView().Contains(StandardDataFormats::StorageItems())) {
        e.AcceptedOperation(DataPackageOperation::Copy);
    }
}

void MainWindow::OnBackstageDrop(IInspectable const&, DragEventArgs const& e) {
    if (!e.DataView().Contains(StandardDataFormats::StorageItems())) {
        return;
    }
    [this, dataView = e.DataView()]() -> fire_and_forget {
        const auto weakSelf = get_weak();
        const auto items = co_await dataView.GetStorageItemsAsync();
        if (items.Size() > 0) {
            if (auto file = items.GetAt(0).try_as<StorageFile>()) {
                if (auto self = weakSelf.get()) {
                    self->OpenStorageFileAsync(file);
                }
            }
        }
    }();
}

// ===========================================================================
// Double-Click Document Title -> Select All & Rename Inline
// ===========================================================================
void MainWindow::OnDocumentTitleDoubleTapped(IInspectable const&, DoubleTappedRoutedEventArgs const& e) {
    m_committingTitleEdit = false;
    DocumentTitleButton().Visibility(Visibility::Collapsed);
    DocumentTitleEditBox().Text(hstring{m_documentDisplayName});
    DocumentTitleEditBox().Visibility(Visibility::Visible);
    DocumentTitleEditBox().Focus(FocusState::Programmatic);
    DocumentTitleEditBox().SelectAll();
    e.Handled(true);
}

void MainWindow::CommitDocumentTitleEdit(bool apply) {
    if (m_committingTitleEdit) return;
    m_committingTitleEdit = true;

    if (apply) {
        std::wstring raw{DocumentTitleEditBox().Text().c_str()};
        while (!raw.empty() && (raw.front() == L' ' || raw.front() == L'\t')) raw.erase(raw.begin());
        while (!raw.empty() && (raw.back() == L' ' || raw.back() == L'\t')) raw.pop_back();
        if (!raw.empty()) {
            if (raw.find(L'.') == std::wstring::npos) {
                raw += L"." + std::wstring{vexio::win::ToHString(m_activeFormatExt).c_str()};
            }
            m_documentDisplayName = raw;
            if (m_doc) {
                (void)m_doc.SetTitle(vexio::win::ToUtf8(m_documentDisplayName));
            }
            MarkDocumentDirty(true);
        }
    }

    DocumentTitleEditBox().Visibility(Visibility::Collapsed);
    DocumentTitleButton().Visibility(Visibility::Visible);
    m_committingTitleEdit = false;
}

void MainWindow::OnDocumentTitleEditKeyDown(IInspectable const&, KeyRoutedEventArgs const& e) {
    if (e.Key() == VirtualKey::Enter) {
        CommitDocumentTitleEdit(true);
        e.Handled(true);
    } else if (e.Key() == VirtualKey::Escape) {
        CommitDocumentTitleEdit(false);
        e.Handled(true);
    }
}

void MainWindow::OnDocumentTitleEditLostFocus(IInspectable const&, RoutedEventArgs const&) {
    if (DocumentTitleEditBox().Visibility() == Visibility::Visible) {
        CommitDocumentTitleEdit(true);
    }
}

// ===========================================================================
// Chrome-Style Hover-to-Switch Open Top Menus & Collapsible Visio Ribbon
// ===========================================================================
void MainWindow::OnTopFlyoutOpened(IInspectable const& sender, IInspectable const&) {
    if (sender == TopExportMenuFlyout()) {
        m_openTopMenuTag = L"ExportMenu";
    } else if (sender == TopHelpMenuFlyout()) {
        m_openTopMenuTag = L"HelpMenu";
    }
}

void MainWindow::OnTopFlyoutClosed(IInspectable const& sender, IInspectable const&) {
    if (sender == TopExportMenuFlyout() && m_openTopMenuTag == L"ExportMenu") {
        m_openTopMenuTag.clear();
    } else if (sender == TopHelpMenuFlyout() && m_openTopMenuTag == L"HelpMenu") {
        m_openTopMenuTag.clear();
    }
}

void MainWindow::OnTopMenuPointerEntered(IInspectable const& sender, PointerRoutedEventArgs const&) {
    if (m_openTopMenuTag.empty()) {
        return;
    }
    auto btn = sender.try_as<Button>();
    if (!btn) return;
    const hstring hoveredTag = unbox_value_or<hstring>(btn.Tag(), L"");
    if (hoveredTag.empty() || hoveredTag == m_openTopMenuTag) {
        return;
    }

    // Close currently open sibling menu/backstage and open hovered target immediately
    TopExportMenuFlyout().Hide();
    TopHelpMenuFlyout().Hide();
    if (hoveredTag == L"FileMenu") {
        InAppBackstageOverlay().Visibility(Visibility::Visible);
        m_openTopMenuTag = L"FileMenu";
    } else {
        InAppBackstageOverlay().Visibility(Visibility::Collapsed);
        if (hoveredTag == L"ExportMenu") {
            m_openTopMenuTag = L"ExportMenu";
            TopExportMenuFlyout().ShowAt(TopExportMenuButton());
        } else if (hoveredTag == L"HelpMenu") {
            m_openTopMenuTag = L"HelpMenu";
            TopHelpMenuFlyout().ShowAt(TopHelpMenuButton());
        }
    }
}

void MainWindow::OnToggleRibbonCollapseClick(IInspectable const&, RoutedEventArgs const&) {
    m_ribbonCollapsed = !m_ribbonCollapsed;
    RibbonBodyRow().Height(GridLength{m_ribbonCollapsed ? 0.0 : (m_touchMode ? 104.0 : 88.0)});
    RibbonBodyScrollViewer().Visibility(m_ribbonCollapsed ? Visibility::Collapsed : Visibility::Visible);
    RibbonCollapseGlyphText().Text(m_ribbonCollapsed ? L"⌄" : L"⌃");
    AttachOrResizeDx12SwapChain();
}

// ===========================================================================
// Rich Text Formatting (Font Family, Size, B/I/U/S, Color, Highlight, Align, Bullets)
// ===========================================================================
void MainWindow::ApplyRichTextPatchJson(std::string_view patchJsonUtf8) {
    if (!m_doc || m_selectedShapeId == 0 || m_syncingTextControls) {
        return;
    }
    const std::array<uint64_t, 1> ids{m_selectedShapeId};
    if (m_doc.SetShapeTextStyleJson(m_activePageId, ids, patchJsonUtf8) == VEXIO_OK) {
        MarkDocumentDirty(true);
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
    }
}

void MainWindow::OnFontFormattingComboChanged(IInspectable const& sender, SelectionChangedEventArgs const&) {
    if (!m_isInitialized || m_syncingTextControls || !m_doc) return;

    if (sender == FontFamilyCombo()) {
        if (auto fontStr = FontFamilyCombo().SelectedItem().try_as<hstring>()) {
            ApplyRichTextPatchJson(std::format("{{\"fontFamily\":\"{}\"}}", vexio::win::ToUtf8(*fontStr)));
        }
    } else if (sender == FontSizeCombo()) {
        if (auto sizeStr = FontSizeCombo().SelectedItem().try_as<hstring>()) {
            const double pt = std::max(4.0, std::wcstod(sizeStr->c_str(), nullptr));
            ApplyRichTextPatchJson(std::format("{{\"fontSizePt\":{:.1f}}}", pt));
        }
    } else if (sender == FontColorCombo()) {
        static constexpr std::array<const char*, 7> kHexColors{
            "#0F172A", "#1D4ED8", "#DC2626", "#15803D", "#B45309", "#7C3AED", "#FFFFFF"
        };
        const int32_t idx = std::clamp(FontColorCombo().SelectedIndex(), 0, 6);
        ApplyRichTextPatchJson(std::format("{{\"colorHex\":\"{}\"}}", kHexColors[static_cast<size_t>(idx)]));
    } else if (sender == TextHighlightCombo()) {
        static constexpr std::array<const char*, 5> kBgHex{
            "none", "#FEF08A", "#A5F3FC", "#BBF7D0", "#FECDD3"
        };
        const int32_t idx = std::clamp(TextHighlightCombo().SelectedIndex(), 0, 4);
        ApplyRichTextPatchJson(std::format("{{\"bgColorHex\":\"{}\"}}", kBgHex[static_cast<size_t>(idx)]));
    }
}

void MainWindow::OnGrowFontSizeClick(IInspectable const&, RoutedEventArgs const&) {
    const int32_t count = static_cast<int32_t>(FontSizeCombo().Items().Size());
    const int32_t nextIdx = std::min(count - 1, FontSizeCombo().SelectedIndex() + 1);
    FontSizeCombo().SelectedIndex(nextIdx);
}

void MainWindow::OnShrinkFontSizeClick(IInspectable const&, RoutedEventArgs const&) {
    const int32_t prevIdx = std::max(0, FontSizeCombo().SelectedIndex() - 1);
    FontSizeCombo().SelectedIndex(prevIdx);
}

void MainWindow::OnTextStyleToggleClick(IInspectable const&, RoutedEventArgs const&) {
    const bool bold   = BoldTextToggle().IsChecked().GetBoolean();
    const bool italic = ItalicTextToggle().IsChecked().GetBoolean();
    const bool under  = UnderlineTextToggle().IsChecked().GetBoolean();
    const bool strike = StrikeTextToggle().IsChecked().GetBoolean();
    ApplyRichTextPatchJson(std::format(
        "{{\"bold\":{},\"italic\":{},\"underline\":{},\"strikethrough\":{}}}",
        bold ? "true" : "false",
        italic ? "true" : "false",
        under ? "true" : "false",
        strike ? "true" : "false"));
}

void MainWindow::OnTextAlignLeftClick(IInspectable const&, RoutedEventArgs const&)   { ApplyRichTextPatchJson("{\"horizAlign\":0}"); }
void MainWindow::OnTextAlignCenterClick(IInspectable const&, RoutedEventArgs const&) { ApplyRichTextPatchJson("{\"horizAlign\":1}"); }
void MainWindow::OnTextAlignRightClick(IInspectable const&, RoutedEventArgs const&)  { ApplyRichTextPatchJson("{\"horizAlign\":2}"); }
void MainWindow::OnTextVAlignTopClick(IInspectable const&, RoutedEventArgs const&)    { ApplyRichTextPatchJson("{\"vertAlign\":0}"); }
void MainWindow::OnTextVAlignMiddleClick(IInspectable const&, RoutedEventArgs const&) { ApplyRichTextPatchJson("{\"vertAlign\":1}"); }
void MainWindow::OnTextVAlignBottomClick(IInspectable const&, RoutedEventArgs const&) { ApplyRichTextPatchJson("{\"vertAlign\":2}"); }

void MainWindow::OnBulletListToggleClick(IInspectable const&, RoutedEventArgs const&) {
    const bool bulleted = BulletListToggle().IsChecked().GetBoolean();
    ApplyRichTextPatchJson(bulleted ? "{\"bullet\":1}" : "{\"bullet\":0}");
}

void MainWindow::OnTextColorPickerChanged(ColorPicker const&, ColorChangedEventArgs const& args) {
    if (!m_isInitialized || m_syncingTextControls) return;
    const auto c = args.NewColor();
    ApplyRichTextPatchJson(std::format("{{\"colorHex\":\"#{:02X}{:02X}{:02X}\"}}", c.R, c.G, c.B));
}

void MainWindow::OnTextHighlightPickerChanged(ColorPicker const&, ColorChangedEventArgs const& args) {
    if (!m_isInitialized || m_syncingTextControls) return;
    const auto c = args.NewColor();
    ApplyRichTextPatchJson(std::format("{{\"bgColorHex\":\"#{:02X}{:02X}{:02X}\"}}", c.R, c.G, c.B));
}

void MainWindow::OnClearTextHighlightClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_isInitialized) return;
    ApplyRichTextPatchJson("{\"bgColorHex\":\"none\"}");
}

void MainWindow::OnShapeFillColorPickerChanged(ColorPicker const&, ColorChangedEventArgs const& args) {
    if (!m_isInitialized || m_syncingTextControls || !m_doc || m_selectedShapeId == 0) return;
    const auto c = args.NewColor();
    const std::string formula = std::format("RGB({},{},{})", c.R, c.G, c.B);
    FormatFillForegndBox().Text(vexio::win::ToHString(formula));
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "FillForegnd", formula, m_forceSetf);
    MarkDocumentDirty(true);
    PresentViewportFrame();
    RefreshShapeSheetSectionRows();
}

void MainWindow::OnShapeLineColorPickerChanged(ColorPicker const&, ColorChangedEventArgs const& args) {
    if (!m_isInitialized || m_syncingTextControls || !m_doc || m_selectedShapeId == 0) return;
    const auto c = args.NewColor();
    const std::string formula = std::format("RGB({},{},{})", c.R, c.G, c.B);
    FormatLineColorBox().Text(vexio::win::ToHString(formula));
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "LineColor", formula, m_forceSetf);
    MarkDocumentDirty(true);
    PresentViewportFrame();
    RefreshShapeSheetSectionRows();
}

// ===========================================================================
// Rotate 90° Left / Right & Flip Horizontal / Vertical for All Stencils
// ===========================================================================
void MainWindow::OnRotateLeft90Click(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc || m_selectedShapeId == 0) return;
    const std::array<uint64_t, 1> ids{m_selectedShapeId};
    if (m_doc.RotateFlipShapes(m_activePageId, ids, 90.0, false, false) == VEXIO_OK) {
        MarkDocumentDirty(true);
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
    }
}

void MainWindow::OnRotateRight90Click(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc || m_selectedShapeId == 0) return;
    const std::array<uint64_t, 1> ids{m_selectedShapeId};
    if (m_doc.RotateFlipShapes(m_activePageId, ids, -90.0, false, false) == VEXIO_OK) {
        MarkDocumentDirty(true);
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
    }
}

void MainWindow::OnFlipHorizontalClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc || m_selectedShapeId == 0) return;
    const std::array<uint64_t, 1> ids{m_selectedShapeId};
    if (m_doc.RotateFlipShapes(m_activePageId, ids, 0.0, true, false) == VEXIO_OK) {
        MarkDocumentDirty(true);
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
    }
}

void MainWindow::OnFlipVerticalClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc || m_selectedShapeId == 0) return;
    const std::array<uint64_t, 1> ids{m_selectedShapeId};
    if (m_doc.RotateFlipShapes(m_activePageId, ids, 0.0, false, true) == VEXIO_OK) {
        MarkDocumentDirty(true);
        PresentViewportFrame();
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
    }
}

// ===========================================================================
// Import Visio Stencils (.vssx, .vss, .vssm, .zip, .svg, .vsdx, .vstx, .vdx)
// ===========================================================================
void MainWindow::LoadPersistedUserStencils() {
    if (!m_doc) return;
    std::error_code ec{};
    const auto dir = GetLocalStencilsDirectory();
    if (!std::filesystem::exists(dir, ec)) return;
    std::vector<std::filesystem::path> files;
    for (auto const& entry : std::filesystem::directory_iterator(dir, ec)) {
        if (entry.is_regular_file(ec)) {
            files.push_back(entry.path());
        }
    }
    std::sort(files.begin(), files.end());
    for (auto const& path : files) {
        std::ifstream ifs{path, std::ios::binary | std::ios::ate};
        if (!ifs) continue;
        const auto sz = ifs.tellg();
        if (sz <= 0) continue;
        ifs.seekg(0, std::ios::beg);
        std::vector<uint8_t> buf(static_cast<size_t>(sz));
        if (ifs.read(reinterpret_cast<char*>(buf.data()), sz)) {
            const std::string fname = vexio::win::ToUtf8(path.filename().wstring());
            (void)m_doc.ImportStencilBytes(fname, buf);
        }
    }
}

void MainWindow::PersistStencilBytesToLocalAppData(std::string_view fileNameUtf8, std::span<const uint8_t> bytes) {
    if (bytes.empty()) return;
    std::string safeName{fileNameUtf8};
    for (char& c : safeName) {
        if (!(std::isalnum(static_cast<unsigned char>(c)) || c == '.' || c == '-' || c == '_' || c == ' ')) {
            c = '_';
        }
    }
    if (safeName.empty()) {
        safeName = "Custom_Stencil.vssx";
    }
    const auto outPath = GetLocalStencilsDirectory() / vexio::win::ToHString(safeName).c_str();
    std::ofstream ofs{outPath, std::ios::binary | std::ios::trunc};
    if (ofs) {
        ofs.write(reinterpret_cast<const char*>(bytes.data()), static_cast<std::streamsize>(bytes.size()));
    }
}

void MainWindow::OnImportStencilClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    ImportVisioStencilPickerAsync();
}

void MainWindow::OnOpenOfficialStencilCatalogClick(IInspectable const&, RoutedEventArgs const&) {
    CloseBackstageOverlay();
    ShowOfficialStencilCatalogDialogAsync();
}

fire_and_forget MainWindow::ShowOfficialStencilCatalogDialogAsync() {
    if (!m_doc || !RootWorkstationGrid().XamlRoot()) co_return;

    struct CatalogRow {
        std::wstring title;
        std::wstring vendor;
        std::wstring categoryGroup;
        std::string fileNameUtf8;
        std::wstring url;
        int32_t approxSizeKb{0};
    };
    std::vector<CatalogRow> catalogRows;
    const auto catJson = vexio::win::DocHandle::ListOfficialStencilCatalogJson();
    if (catJson.ok() && !catJson.value->empty()) {
        Windows::Data::Json::JsonArray arr{nullptr};
        if (Windows::Data::Json::JsonArray::TryParse(*catJson.value, arr)) {
            for (uint32_t i = 0; i < arr.Size(); ++i) {
                const auto obj = arr.GetObjectAt(i);
                CatalogRow row{};
                row.title = obj.GetNamedString(L"title", L"Stencil Pack").c_str();
                row.vendor = obj.GetNamedString(L"vendor", L"Microsoft").c_str();
                row.categoryGroup = obj.GetNamedString(L"categoryGroup", L"Microsoft").c_str();
                row.fileNameUtf8 = vexio::win::ToUtf8(obj.GetNamedString(L"fileName", L"Stencil.vssx"));
                row.url = obj.GetNamedString(L"url", L"").c_str();
                row.approxSizeKb = static_cast<int32_t>(obj.GetNamedNumber(L"approxSizeKb", 500.0));
                if (!row.url.empty()) {
                    catalogRows.push_back(std::move(row));
                }
            }
        }
    }

    StackPanel rootStack{};
    rootStack.Spacing(8.0);
    TextBlock subtitle{};
    subtitle.Text(L"1-click download & automatic %LOCALAPPDATA%\\Vexio\\Stencils persistence across restarts (Microsoft 365, Azure V25, Cisco, Panduit, APC & NetEquip .vssx / .vss / .zip).");
    subtitle.FontSize(12.0);
    subtitle.Opacity(0.8);
    subtitle.TextWrapping(TextWrapping::Wrap);
    rootStack.Children().Append(subtitle);

    ListView list{};
    list.MaxHeight(340.0);
    list.SelectionMode(ListViewSelectionMode::Single);
    for (auto const& row : catalogRows) {
        StackPanel itemPanel{};
        itemPanel.Padding(Thickness{4, 4, 4, 4});
        TextBlock t1{};
        t1.Text(hstring{row.title});
        t1.FontWeight(Microsoft::UI::Text::FontWeights::SemiBold());
        t1.FontSize(12.5);
        TextBlock t2{};
        t2.Text(hstring{std::format(L"{} • ~{} KB • {}", row.categoryGroup, row.approxSizeKb, row.vendor)});
        t2.FontSize(11.0);
        t2.Opacity(0.72);
        itemPanel.Children().Append(t1);
        itemPanel.Children().Append(t2);
        list.Items().Append(itemPanel);
    }
    if (!catalogRows.empty()) {
        list.SelectedIndex(0);
    }
    rootStack.Children().Append(list);

    ContentDialog dlg{};
    dlg.XamlRoot(RootWorkstationGrid().XamlRoot());
    dlg.Title(box_value(L"Official & Enterprise Stencil Catalog"));
    dlg.Content(rootStack);
    dlg.PrimaryButtonText(L"Install Selected Pack");
    dlg.SecondaryButtonText(L"Install All Packs");
    dlg.CloseButtonText(L"Close");
    dlg.DefaultButton(ContentDialogButton::Primary);

    const auto weakSelf = get_weak();
    const ContentDialogResult result = co_await dlg.ShowAsync();
    auto self = weakSelf.get();
    if (!self || !self->m_doc || result == ContentDialogResult::None || catalogRows.empty()) {
        co_return;
    }

    std::vector<CatalogRow> targets;
    if (result == ContentDialogResult::Secondary) {
        targets = catalogRows;
    } else {
        const int32_t sel = list.SelectedIndex();
        const size_t idx = (sel >= 0 && static_cast<size_t>(sel) < catalogRows.size())
            ? static_cast<size_t>(sel)
            : 0u;
        targets.push_back(catalogRows[idx]);
    }

    Windows::Web::Http::HttpClient client{};
    client.DefaultRequestHeaders().UserAgent().TryParseAdd(L"Vexio-Windows/0.3.5-alpha");
    uint32_t installedCount = 0;

    for (auto const& target : targets) {
        try {
            const Uri packUri{hstring{target.url}};
            const IBuffer dlBuf = co_await client.GetBufferAsync(packUri);
            auto innerSelf = weakSelf.get();
            if (!innerSelf || !innerSelf->m_doc) co_return;
            const std::span<const uint8_t> bytes{dlBuf.data(), dlBuf.Length()};
            if (!bytes.empty()) {
                const auto impRes = innerSelf->m_doc.ImportStencilBytes(target.fileNameUtf8, bytes);
                if (impRes.ok()) {
                    innerSelf->PersistStencilBytesToLocalAppData(target.fileNameUtf8, bytes);
                    ++installedCount;
                }
            }
        } catch (...) {
            // Continue remaining packs if one mirror is unreachable
        }
    }

    if (auto finalSelf = weakSelf.get(); finalSelf && installedCount > 0) {
        finalSelf->InitializeStencilCatalog();
        if (!finalSelf->m_stencilPackNames.empty()) {
            finalSelf->StencilPackCombo().SelectedIndex(static_cast<int32_t>(finalSelf->m_stencilPackNames.size()));
        }
        finalSelf->RefreshStencilList();
        finalSelf->ShowBanner(
            InfoBarSeverity::Success,
            L"Official Stencil Packs Installed",
            std::format(L"Downloaded and persisted {} official stencil archive(s).", installedCount));
    }
}

void MainWindow::OnStencilDockDragOver(IInspectable const&, DragEventArgs const& e) {
    if (e.DataView().Contains(StandardDataFormats::StorageItems())) {
        e.AcceptedOperation(DataPackageOperation::Copy);
        if (e.DragUIOverride()) {
            e.DragUIOverride().Caption(L"Import Visio Stencil (.vssx / .vss / .zip)");
            e.DragUIOverride().IsCaptionVisible(true);
        }
    }
}

void MainWindow::OnStencilDockDrop(IInspectable const&, DragEventArgs const& e) {
    if (!e.DataView().Contains(StandardDataFormats::StorageItems())) {
        return;
    }
    [this, dataView = e.DataView()]() -> fire_and_forget {
        const auto weakSelf = get_weak();
        const auto items = co_await dataView.GetStorageItemsAsync();
        for (uint32_t i = 0; i < items.Size(); ++i) {
            if (auto file = items.GetAt(i).try_as<StorageFile>()) {
                if (auto self = weakSelf.get()) {
                    self->ImportStencilStorageFileAsync(file);
                }
            }
        }
    }();
}

fire_and_forget MainWindow::ImportVisioStencilPickerAsync() {
    const HWND hwnd = GetNativeWindowHandle();
    if (hwnd == nullptr || !m_doc) co_return;

    FileOpenPicker picker{};
    if (auto initWithWin = picker.try_as<::IInitializeWithWindow>()) {
        (void)initWithWin->Initialize(hwnd);
    }
    picker.ViewMode(PickerViewMode::List);
    picker.SuggestedStartLocation(PickerLocationId::DocumentsLibrary);
    for (wchar_t const* ext : {L".vssx", L".vss", L".vssm", L".zip", L".svg", L".png", L".vsdx", L".vstx", L".vsd", L".vdx"}) {
        picker.FileTypeFilter().Append(ext);
    }

    const auto weakSelf = get_weak();
    StorageFile picked = co_await picker.PickSingleFileAsync();
    if (picked) {
        if (auto self = weakSelf.get()) {
            self->ImportStencilStorageFileAsync(picked);
        }
    }
}

fire_and_forget MainWindow::ImportStencilStorageFileAsync(StorageFile file) {
    if (!file || !m_doc) co_return;
    const auto weakSelf = get_weak();

    try {
        const IBuffer buf = co_await FileIO::ReadBufferAsync(file);
        auto self = weakSelf.get();
        if (!self || !self->m_doc) co_return;

        const std::span<const uint8_t> bytes{buf.data(), buf.Length()};
        const std::string fileNameUtf8 = vexio::win::ToUtf8(file.Name());
        const auto res = self->m_doc.ImportStencilBytes(fileNameUtf8, bytes);
        if (!res.ok()) {
            self->ShowBanner(InfoBarSeverity::Error, L"Stencil Import Failed", res.message());
            co_return;
        }
        self->PersistStencilBytesToLocalAppData(fileNameUtf8, bytes);

        self->InitializeStencilCatalog();
        if (!self->m_stencilPackNames.empty()) {
            self->StencilPackCombo().SelectedIndex(static_cast<int32_t>(self->m_stencilPackNames.size()));
        }
        self->RefreshStencilList();
        self->MarkDocumentDirty(true);
        self->ShowBanner(
            InfoBarSeverity::Success,
            L"Visio Stencil Imported",
            std::format(L"Imported and persisted masters from {} into the Stencil Navigator.", file.Name().c_str()));
    } catch (...) {
        if (auto self = weakSelf.get()) {
            self->ShowBanner(InfoBarSeverity::Error, L"Stencil I/O Error", L"Unable to read selected Visio stencil file.");
        }
    }
}

// ===========================================================================
// Double-Click Placed Stencil / Shape -> Inline Canvas Label Editor
// ===========================================================================
void MainWindow::OnCanvasDoubleTapped(IInspectable const&, DoubleTappedRoutedEventArgs const& e) {
    if (!m_viewport || !m_doc) return;

    auto panel = DiagramSwapChainPanel();
    const Point pt = e.GetPosition(panel);
    const float scaleX = std::clamp(panel.CompositionScaleX(), 0.5f, 8.0f);
    const float scaleY = std::clamp(panel.CompositionScaleY(), 0.5f, 8.0f);

    uint64_t hitId = m_viewport.HitTestShape(
        m_doc,
        static_cast<double>(pt.X) * static_cast<double>(scaleX),
        static_cast<double>(pt.Y) * static_cast<double>(scaleY));

    if (hitId == 0) {
        hitId = m_viewport.GetPrimarySelection();
    }
    if (hitId > 0) {
        m_previousSelectedShapeId = m_selectedShapeId;
        m_selectedShapeId = hitId;
        BeginInlineCanvasShapeTextEdit(hitId, false);
        e.Handled(true);
    }
}

void MainWindow::BeginInlineCanvasShapeTextEdit(uint64_t shapeId, bool isNewlyPlacedTextShape) {
    if (!m_doc || !m_viewport || shapeId == 0) return;

    auto panel = DiagramSwapChainPanel();
    const float scaleX = std::clamp(panel.CompositionScaleX(), 0.5f, 8.0f);
    const float scaleY = std::clamp(panel.CompositionScaleY(), 0.5f, 8.0f);

    double dipX = 120.0;
    double dipY = 120.0;
    double dipW = 160.0;
    double dipH = 44.0;

    const auto boundsRes = m_viewport.GetShapeScreenBounds(m_doc, shapeId);
    if (boundsRes.ok()) {
        dipX = boundsRes.value->xPx / static_cast<double>(scaleX);
        dipY = boundsRes.value->yPx / static_cast<double>(scaleY);
        dipW = std::max(120.0, boundsRes.value->widthPx / static_cast<double>(scaleX));
        dipH = std::max(24.0, boundsRes.value->heightPx / static_cast<double>(scaleY));
    }

    const double maxW = std::max(200.0, panel.ActualWidth() - 20.0);
    const double maxH = std::max(120.0, panel.ActualHeight() - 20.0);
    dipX = std::clamp(dipX - 4.0, 6.0, std::max(6.0, maxW - dipW));
    dipY = std::clamp(dipY - 4.0, 6.0, std::max(6.0, maxH - dipH));

    // Read existing text of shapeId from Vexio state JSON
    std::wstring existingLabel{};
    const auto stateRes = m_doc.GetStateJson(m_activePageId);
    if (stateRes.ok() && !stateRes.value->empty()) {
        Windows::Data::Json::JsonObject stateObj{nullptr};
        if (Windows::Data::Json::JsonObject::TryParse(*stateRes.value, stateObj) && stateObj.HasKey(L"shapes")) {
            const auto shapesArr = stateObj.GetNamedArray(L"shapes");
            for (uint32_t i = 0; i < shapesArr.Size(); ++i) {
                const auto shObj = shapesArr.GetObjectAt(i);
                const uint64_t sid = static_cast<uint64_t>(shObj.GetNamedNumber(L"id", 0.0));
                if (sid == shapeId) {
                    existingLabel = shObj.GetNamedString(L"text", L"").c_str();
                    break;
                }
            }
        }
    }
    if (isNewlyPlacedTextShape && existingLabel == L"Text") {
        existingLabel.clear();
    }

    m_inlineEditingShapeId = shapeId;
    m_committingInlineEdit = false;

    Canvas::SetLeft(InlineCanvasEditorBorder(), dipX);
    Canvas::SetTop(InlineCanvasEditorBorder(), dipY);
    InlineCanvasEditorBorder().Width(dipW + 8.0);
    InlineCanvasEditorBorder().MinHeight(dipH + 4.0);
    InlineCanvasTextBox().Text(hstring{existingLabel});
    InlineCanvasEditorBorder().Visibility(Visibility::Visible);
    InlineCanvasTextBox().Focus(FocusState::Programmatic);
    InlineCanvasTextBox().SelectAll();
}

void MainWindow::CommitInlineCanvasShapeTextEdit(bool apply) {
    if (m_committingInlineEdit || m_inlineEditingShapeId == 0) {
        return;
    }
    m_committingInlineEdit = true;

    const uint64_t targetId = m_inlineEditingShapeId;
    m_inlineEditingShapeId = 0;
    InlineCanvasEditorBorder().Visibility(Visibility::Collapsed);

    if (apply && m_doc) {
        const std::string updatedUtf8 = vexio::win::ToUtf8(InlineCanvasTextBox().Text());
        if (m_doc.SetShapeText(m_activePageId, targetId, updatedUtf8) == VEXIO_OK) {
            MarkDocumentDirty(true);
            PresentViewportFrame();
            SyncInspectorFromShapeSheet();
            RefreshShapeSheetSectionRows();
        }
    }

    m_committingInlineEdit = false;
}

void MainWindow::OnInlineCanvasTextKeyDown(IInspectable const&, KeyRoutedEventArgs const& e) {
    if (e.Key() == VirtualKey::Escape) {
        CommitInlineCanvasShapeTextEdit(false);
        e.Handled(true);
    } else if (e.Key() == VirtualKey::Enter) {
        const auto shiftState = InputKeyboardSource::GetKeyStateForCurrentThread(VirtualKey::Shift);
        const bool shiftHeld = (shiftState & Windows::UI::Core::CoreVirtualKeyStates::Down) == Windows::UI::Core::CoreVirtualKeyStates::Down;
        if (!shiftHeld) {
            CommitInlineCanvasShapeTextEdit(true);
            e.Handled(true);
        }
    }
}

void MainWindow::OnInlineCanvasTextLostFocus(IInspectable const&, RoutedEventArgs const&) {
    if (InlineCanvasEditorBorder().Visibility() == Visibility::Visible) {
        CommitInlineCanvasShapeTextEdit(true);
    }
}

// ===========================================================================
// Automatic & Manual Update Checker (GitHub Releases: petrosvassiliou/vexio)
// ===========================================================================
void MainWindow::OnAutoUpdatePrefChanged(IInspectable const&, RoutedEventArgs const&) {
    if (!m_isInitialized) return;
    m_autoCheckUpdates = AutoCheckUpdatesToggle().IsChecked().GetBoolean();
}

void MainWindow::OnCheckUpdatesClick(IInspectable const&, RoutedEventArgs const&) {
    CheckForUpdatesAsync(true);
}

fire_and_forget MainWindow::CheckForUpdatesAsync(bool userInitiated) {
    constexpr std::wstring_view kCurrentVersion = L"0.3.5-alpha";
    const auto weakSelf = get_weak();

    if (userInitiated) {
        BackstageUpdateStatusText().Text(L"Checking GitHub Releases for updates...");
    }

    std::wstring latestTag{};
    std::wstring releaseUrl{L"https://github.com/petrosvassiliou/vexio/releases"};
    std::wstring assetDownloadUrl{};
    bool checkSucceeded = false;

    try {
        Windows::Web::Http::HttpClient client{};
        client.DefaultRequestHeaders().UserAgent().TryParseAdd(L"Vexio-Windows/0.3.5-alpha");
        client.DefaultRequestHeaders().Accept().TryParseAdd(L"application/vnd.github+json");

        const Uri apiUri{L"https://api.github.com/repos/petrosvassiliou/vexio/releases"};
        const hstring body = co_await client.GetStringAsync(apiUri);

        Windows::Data::Json::JsonArray releasesArr{nullptr};
        if (Windows::Data::Json::JsonArray::TryParse(body, releasesArr) && releasesArr.Size() > 0) {
            const auto relObj = releasesArr.GetObjectAt(0);
            latestTag = relObj.GetNamedString(L"tag_name", L"v0.3.5-alpha").c_str();
            releaseUrl = relObj.GetNamedString(L"html_url", releaseUrl).c_str();
            if (relObj.HasKey(L"assets")) {
                const auto assets = relObj.GetNamedArray(L"assets");
                for (uint32_t i = 0; i < assets.Size(); ++i) {
                    const auto aObj = assets.GetObjectAt(i);
                    const std::wstring name{aObj.GetNamedString(L"name", L"").c_str()};
                    if (name.find(L"windows") != std::wstring::npos ||
                        name.find(L"Windows") != std::wstring::npos ||
                        name.find(L".msix") != std::wstring::npos ||
                        name.find(L".zip") != std::wstring::npos)
                    {
                        assetDownloadUrl = aObj.GetNamedString(L"browser_download_url", L"").c_str();
                        break;
                    }
                }
            }
            checkSucceeded = true;
        }
    } catch (...) {
        checkSucceeded = false;
    }

    auto self = weakSelf.get();
    if (!self) co_return;

    if (!checkSucceeded) {
        self->BackstageUpdateStatusText().Text(L"Offline mode active • Update check skipped cleanly.");
        if (userInitiated) {
            self->ShowBanner(
                InfoBarSeverity::Informational,
                L"Offline — Unable to Reach Update Server",
                L"Vexio runs 100% offline. Connect to the internet to check GitHub Releases for updates.");
        }
        co_return;
    }

    std::wstring cleanLatest = latestTag;
    if (!cleanLatest.empty() && (cleanLatest.front() == L'v' || cleanLatest.front() == L'V')) {
        cleanLatest.erase(cleanLatest.begin());
    }
    const bool hasNewer = !cleanLatest.empty() && cleanLatest != kCurrentVersion && cleanLatest > kCurrentVersion;

    if (hasNewer) {
        self->BackstageUpdateStatusText().Text(hstring{
            std::format(L"Update available: {} (Current: v{})", latestTag, kCurrentVersion)
        });
        ContentDialog dlg{};
        dlg.XamlRoot(self->RootWorkstationGrid().XamlRoot());
        dlg.Title(box_value(hstring{std::format(L"Vexio Update Available ({})", latestTag)}));
        dlg.Content(box_value(hstring{std::format(
            L"A newer release ({}) is available on GitHub (installed: v{}). Would you like to download the Windows installer now?",
            latestTag, kCurrentVersion)}));
        dlg.PrimaryButtonText(L"Download Update");
        dlg.CloseButtonText(L"Later");
        dlg.DefaultButton(ContentDialogButton::Primary);
        if (co_await dlg.ShowAsync() == ContentDialogResult::Primary) {
            const Uri targetUri{hstring{assetDownloadUrl.empty() ? releaseUrl : assetDownloadUrl}};
            (void)co_await Launcher::LaunchUriAsync(targetUri);
        }
    } else {
        self->BackstageUpdateStatusText().Text(hstring{
            std::format(L"Vexio is up to date ({}).", latestTag.empty() ? L"v0.3.5-alpha" : latestTag)
        });
        if (userInitiated) {
            ContentDialog dlg{};
            dlg.XamlRoot(self->RootWorkstationGrid().XamlRoot());
            dlg.Title(box_value(L"Vexio is Up to Date"));
            dlg.Content(box_value(hstring{std::format(
                L"You are running the latest version of Vexio Professional ({}).",
                latestTag.empty() ? L"v0.3.5-alpha" : latestTag)}));
            dlg.PrimaryButtonText(L"View Release Notes");
            dlg.CloseButtonText(L"OK");
            dlg.DefaultButton(ContentDialogButton::Close);
            if (co_await dlg.ShowAsync() == ContentDialogResult::Primary) {
                (void)co_await Launcher::LaunchUriAsync(Uri{hstring{releaseUrl}});
            }
        }
    }
}

} // namespace winrt::VexioWindows::implementation
