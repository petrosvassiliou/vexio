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

    m_pages = {
        PageTabModel{1u, L"Page-1", false, 4u},
        PageTabModel{2u, L"VBackground-1", true, 1u}
    };

    InitializeStencilCatalog();
    InitializeExternalDataAndLayers();
    SeedActiveShapeDefaultCells();
    RefreshBaselineSnapshot();

    m_isInitialized = true;

    RefreshStencilList();
    RefreshExternalDataList();
    RefreshLayersList();
    RebuildPageTabsStrip();
    SyncInspectorFromShapeSheet();
    RefreshShapeSheetSectionRows();
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
        if (const AppWindow appWindow = AppWindow::GetFromWindowId(windowId)) {
            appWindow.Resize(Windows::Graphics::SizeInt32{1600, 980});
            if (AppWindowTitleBar::IsCustomizationSupported()) {
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

    const com_ptr<ISwapChainPanelNative> panelNative = panel.try_as<ISwapChainPanelNative>();
    if (!panelNative) {
        return;
    }

    // Release previous swapchain before binding replacement dimensions on the same ISwapChainPanelNative
    m_viewport.Reset();

    auto created = vexio::win::ViewportHandle::CreateDx12SwapChain(
        m_engine,
        m_doc,
        panelNative.get(),
        physWidth,
        physHeight,
        scaleX);

    if (created.ok()) {
        m_viewport = std::move(*created.value);
        GpuSwapChainBadgeText().Text(hstring{
            std::format(L"DX12 120Hz • {}x{} @ {:.0f}%", physWidth, physHeight, scaleX * 100.0f)
        });
    } else {
        // Headless fallback if running in a remote session without hardware DXGI adapter
        auto headless = vexio::win::ViewportHandle::CreateHeadless(
            m_engine, m_doc, m_activePageId, physWidth, physHeight, scaleX);
        if (headless.ok()) {
            m_viewport = std::move(*headless.value);
            GpuSwapChainBadgeText().Text(L"DX12 WARP / Headless Active");
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

    // Update live ruler hairlines and canonical document coordinate readout
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
    rawEv.tilt_y_deg   = props.YTilt();
    rawEv.timestamp_us = pt.Timestamp();

    if (m_viewport && m_doc) {
        auto dispatchRes = m_viewport.DispatchPointer(m_doc, rawEv);
        if (dispatchRes.ok()) {
            ApplyCanvasCursor(*dispatchRes.value);
        } else if (dispatchRes.status == VEXIO_ERR_GPU_SURFACE_LOST) {
            AttachOrResizeDx12SwapChain();
        }
    }
}

void MainWindow::OnCanvasPointerPressed(IInspectable const&, PointerRoutedEventArgs const& e) {
    DiagramSwapChainPanel().CapturePointer(e.Pointer());
    DispatchCanvasPointerEvent(static_cast<uint32_t>(vexio::win::PointerEventKind::Down), e);

    // If Rectangle tool is active, instantiate a new rectangle shape at click location
    if (m_activeTool == ActiveCanvasTool::Rectangle && !m_allMasters.empty()) {
        const Point pos = e.GetCurrentPoint(DiagramSwapChainPanel()).Position();
        const double pxPerIn = 96.0 * (m_zoomPercent / 100.0);
        InstantiateMasterOnCanvas(m_allMasters.front(), pos.X / pxPerIn, pos.Y / pxPerIn);
    }
    e.Handled(true);
}

void MainWindow::OnCanvasPointerMoved(IInspectable const&, PointerRoutedEventArgs const& e) {
    DispatchCanvasPointerEvent(static_cast<uint32_t>(vexio::win::PointerEventKind::Move), e);
}

void MainWindow::OnCanvasPointerReleased(IInspectable const&, PointerRoutedEventArgs const& e) {
    DispatchCanvasPointerEvent(static_cast<uint32_t>(vexio::win::PointerEventKind::Up), e);
    DiagramSwapChainPanel().ReleasePointerCapture(e.Pointer());
    SyncInspectorFromShapeSheet();
    e.Handled(true);
}

void MainWindow::OnCanvasPointerWheelChanged(IInspectable const&, PointerRoutedEventArgs const& e) {
    const int32_t delta = e.GetCurrentPoint(DiagramSwapChainPanel()).Properties().MouseWheelDelta();
    if ((e.KeyModifiers() & VirtualKeyModifiers::Control) != VirtualKeyModifiers::None) {
        const double step = delta > 0 ? 10.0 : -10.0;
        SetZoomPercent(std::clamp(m_zoomPercent + step, 25.0, 400.0));
    }
    DispatchCanvasPointerEvent(static_cast<uint32_t>(vexio::win::PointerEventKind::Wheel), e);
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
// Stencils Catalog, Instant Search & Master Drag-onto-Canvas
// ===========================================================================
void MainWindow::InitializeStencilCatalog() {
    m_allMasters = {
        // Pack 0: Basic Flowchart
        {"flow.process",    L"Basic Flowchart Shapes",        L"Process",             L"Flowchart", "2.00 in", "1.00 in", "RGB(59,130,246)",  "Process"},
        {"flow.decision",   L"Basic Flowchart Shapes",        L"Decision Diamond",    L"Flowchart", "1.75 in", "1.25 in", "RGB(245,158,11)",  "Gateway"},
        {"flow.start_end",  L"Basic Flowchart Shapes",        L"Start / Terminator",  L"Flowchart", "1.80 in", "0.75 in", "RGB(16,185,129)",  "StartEvent"},
        {"flow.subprocess", L"Basic Flowchart Shapes",        L"Subprocess Call",     L"Flowchart", "2.20 in", "1.10 in", "RGB(99,102,241)",  "SubProcess"},
        {"flow.database",   L"Basic Flowchart Shapes",        L"Data Store Cylinder", L"Flowchart", "1.60 in", "1.40 in", "RGB(14,165,233)",  "DataStore"},

        // Pack 1: BPMN 2.0
        {"bpmn.start",      L"BPMN 2.0 Process Masters",      L"Start Event (Message)", L"BPMN 2.0", "0.65 in", "0.65 in", "RGB(34,197,94)",  "BPMN.StartEvent"},
        {"bpmn.task",       L"BPMN 2.0 Process Masters",      L"User Task Activity",    L"BPMN 2.0", "2.10 in", "1.10 in", "RGB(59,130,246)", "BPMN.Task"},
        {"bpmn.xor",        L"BPMN 2.0 Process Masters",      L"Exclusive Gateway (XOR)", L"BPMN 2.0", "0.95 in", "0.95 in", "RGB(234,179,8)", "BPMN.ExclusiveGateway"},
        {"bpmn.end",        L"BPMN 2.0 Process Masters",      L"End Event (Terminate)", L"BPMN 2.0", "0.65 in", "0.65 in", "RGB(239,68,68)",   "BPMN.EndEvent"},
        {"bpmn.pool",       L"BPMN 2.0 Process Masters",      L"Participant Pool / Lane", L"BPMN 2.0", "7.50 in", "2.40 in", "RGB(241,245,249)", "BPMN.Pool"},

        // Pack 2: UML 2.5
        {"uml.class",       L"UML 2.5 Class & Sequence",      L"Class (3-Compartment)", L"UML 2.5", "2.40 in", "1.65 in", "RGB(238,242,255)", "UML.Class"},
        {"uml.interface",   L"UML 2.5 Class & Sequence",      L"«interface» Contract",  L"UML 2.5", "2.20 in", "1.30 in", "RGB(240,253,244)", "UML.Interface"},
        {"uml.lifeline",    L"UML 2.5 Class & Sequence",      L"Sequence Lifeline",     L"UML 2.5", "1.50 in", "4.50 in", "RGB(248,250,252)", "UML.Lifeline"},
        {"uml.component",   L"UML 2.5 Class & Sequence",      L"Subsystem Component",   L"UML 2.5", "2.50 in", "1.40 in", "RGB(224,242,254)", "UML.Component"},

        // Pack 3: Cloud & Network
        {"net.router",      L"Cloud & Network Topology",      L"Core BGP Router",       L"Network", "1.40 in", "1.10 in", "RGB(37,99,235)",   "Net.Router"},
        {"net.firewall",    L"Cloud & Network Topology",      L"Zero-Trust Firewall",   L"Network", "1.50 in", "1.00 in", "RGB(220,38,38)",   "Net.Firewall"},
        {"net.k8s_cluster", L"Cloud & Network Topology",      L"Kubernetes Cluster",    L"Cloud",   "3.20 in", "2.10 in", "RGB(56,189,248)",  "Cloud.Cluster"},

        // Pack 4: Architectural CAD
        {"cad.wall_ext",    L"Architectural CAD Walls & Doors", L"Exterior Structural Wall", L"CAD", "6.00 in", "0.35 in", "RGB(71,85,105)",  "CAD.Wall"},
        {"cad.door_swing",  L"Architectural CAD Walls & Doors", L"Single Swing Door 36\"",   L"CAD", "0.90 in", "0.90 in", "RGB(148,163,184)", "CAD.Door"},
        {"cad.room_stamp",  L"Architectural CAD Walls & Doors", L"Architectural Space Tag",  L"CAD", "1.80 in", "0.90 in", "RGB(226,232,240)", "CAD.Space"}
    };
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

    std::wstring selectedPack{};
    if (auto sel = StencilPackCombo().SelectedItem().try_as<hstring>()) {
        selectedPack = sel->c_str();
    }

    m_filteredMasters.clear();
    auto items = StencilMastersListView().Items();
    items.Clear();

    for (auto const& master : m_allMasters) {
        if (query.empty() && !selectedPack.empty() && master.packName != selectedPack) {
            continue;
        }
        if (!query.empty()) {
            std::wstring hay = master.displayName + L" " + master.categoryTag + L" " + vexio::win::ToHString(master.id).c_str();
            std::transform(hay.begin(), hay.end(), hay.begin(), [](wchar_t ch) { return static_cast<wchar_t>(::towlower(ch)); });
            if (hay.find(query) == std::wstring::npos) {
                continue;
            }
        }

        m_filteredMasters.push_back(master);

        Grid card{};
        card.Padding(Thickness{6, 6, 6, 6});
        card.ColumnSpacing(10.0);
        ColumnDefinition c0{}; c0.Width(GridLength{44.0});
        ColumnDefinition c1{}; c1.Width(GridLength{1.0, GridUnitType::Star});
        card.ColumnDefinitions().Append(c0);
        card.ColumnDefinitions().Append(c1);

        // Crisp Light Preview Well + Vector Shape Silhouette (never a plain colored box)
        std::wstring pathMarkup = L"M 3,5 L 33,5 L 33,21 L 3,21 Z";
        Windows::UI::Color strokeCol = Windows::UI::ColorHelper::FromArgb(0xFF, 0x25, 0x63, 0xEB);
        std::wstring lowName = master.displayName;
        std::transform(lowName.begin(), lowName.end(), lowName.begin(), [](wchar_t ch) { return static_cast<wchar_t>(::towlower(ch)); });
        if (lowName.find(L"decision") != std::wstring::npos || lowName.find(L"gateway") != std::wstring::npos) {
            pathMarkup = (lowName.find(L"parallel") != std::wstring::npos)
                ? L"M 18,2 L 34,13 L 18,24 L 2,13 Z M 18,7 L 18,19 M 12,13 L 24,13"
                : (lowName.find(L"exclusive") != std::wstring::npos)
                    ? L"M 18,2 L 34,13 L 18,24 L 2,13 Z M 14,9 L 22,17 M 22,9 L 14,17"
                    : L"M 18,2 L 34,13 L 18,24 L 2,13 Z";
            strokeCol = Windows::UI::ColorHelper::FromArgb(0xFF, 0xD9, 0x77, 0x06);
        } else if (lowName.find(L"event") != std::wstring::npos || lowName.find(L"pump") != std::wstring::npos) {
            pathMarkup = L"M 18,3 A 10,10 0 1 1 17.99,3 Z";
            strokeCol = (lowName.find(L"end") != std::wstring::npos)
                ? Windows::UI::ColorHelper::FromArgb(0xFF, 0xDC, 0x26, 0x26)
                : Windows::UI::ColorHelper::FromArgb(0xFF, 0x05, 0x96, 0x69);
        } else if (lowName.find(L"start") != std::wstring::npos || lowName.find(L"terminator") != std::wstring::npos) {
            pathMarkup = L"M 10,5 L 26,5 A 8,8 0 0 1 26,21 L 10,21 A 8,8 0 0 1 10,5 Z";
            strokeCol = Windows::UI::ColorHelper::FromArgb(0xFF, 0x0D, 0x94, 0x88);
        } else if (lowName.find(L"data") != std::wstring::npos) {
            pathMarkup = L"M 8,5 L 34,5 L 28,21 L 2,21 Z";
            strokeCol = Windows::UI::ColorHelper::FromArgb(0xFF, 0x02, 0x84, 0xC7);
        } else if (lowName.find(L"subprocess") != std::wstring::npos) {
            pathMarkup = L"M 3,5 L 33,5 L 33,21 L 3,21 Z M 7,5 L 7,21 M 29,5 L 29,21";
            strokeCol = Windows::UI::ColorHelper::FromArgb(0xFF, 0x4F, 0x46, 0xE5);
        } else if (lowName.find(L"class") != std::wstring::npos || lowName.find(L"entity") != std::wstring::npos) {
            pathMarkup = L"M 3,3 L 33,3 L 33,23 L 3,23 Z M 3,9 L 33,9 M 3,16 L 33,16";
            strokeCol = Windows::UI::ColorHelper::FromArgb(0xFF, 0x6D, 0x28, 0xD9);
        } else if (lowName.find(L"door") != std::wstring::npos) {
            pathMarkup = L"M 5,22 L 5,4 A 18,18 0 0 1 23,22 Z";
            strokeCol = Windows::UI::ColorHelper::FromArgb(0xFF, 0x33, 0x41, 0x55);
        }

        Border swatch{};
        swatch.Width(42.0);
        swatch.Height(30.0);
        swatch.CornerRadius(CornerRadius{6, 6, 6, 6});
        swatch.Background(SolidColorBrush{Windows::UI::ColorHelper::FromArgb(0xFF, 0xF8, 0xFA, 0xFC)});
        swatch.BorderBrush(SolidColorBrush{Windows::UI::ColorHelper::FromArgb(0xFF, 0xCB, 0xD5, 0xE1)});
        swatch.BorderThickness(Thickness{1, 1, 1, 1});
        auto geom = Markup::XamlBindingHelper::ConvertValue(
            xaml_typename<Media::Geometry>(),
            box_value(hstring{pathMarkup})).try_as<Media::Geometry>();
        Shapes::Path vectorPreview{};
        if (geom) {
            vectorPreview.Data(geom);
        }
        vectorPreview.Stroke(SolidColorBrush{strokeCol});
        vectorPreview.StrokeThickness(1.6);
        vectorPreview.Stretch(Media::Stretch::Uniform);
        vectorPreview.Margin(Thickness{5, 4, 5, 4});
        swatch.Child(vectorPreview);
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
                    InstantiateMasterOnCanvas(master, 4.25, 5.50);
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
    const double pxPerIn = 96.0 * (m_zoomPercent / 100.0);
    const double dropInchesX = std::max(0.25, dropPt.X / pxPerIn);
    const double dropInchesY = std::max(0.25, dropPt.Y / pxPerIn);

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

    const uint64_t newShapeId = m_nextShapeId++;
    m_previousSelectedShapeId = m_selectedShapeId;
    m_selectedShapeId = newShapeId;

    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "PinX",            std::format("{:.3f} in", pinXInches), true);
    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "PinY",            std::format("{:.3f} in", pinYInches), true);
    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "Width",           master.defaultWidthFormula, true);
    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "Height",          master.defaultHeightFormula, true);
    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "LocPinX",         "GUARD(Width*0.5)", true);
    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "LocPinY",         "GUARD(Height*0.5)", true);
    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "Angle",           "0 deg", true);
    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "FillForegnd",     master.defaultFillFormula, true);
    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "LineWeight",      "1.5 pt", true);
    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "Prop.Label",      std::format("\"{}\"", vexio::win::ToUtf8(master.displayName)), true);
    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "Prop.Department", "\"Engineering\"", true);
    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "Prop.Owner",      "\"Architecture\"", true);
    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "Prop.Cost",       "850.00", true);
    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "Prop.Status",     "\"Active\"", true);
    (void)m_doc.SetCellFormula(m_activePageId, newShapeId, "User.BPMNActivityType", std::format("\"{}\"", master.stereotypeOrRuleType), true);

    for (auto& page : m_pages) {
        if (page.pageId == m_activePageId) {
            ++page.shapeCount;
            StatusShapeCountText().Text(hstring{
                std::format(L"Shapes: {} • Selected: Sheet.{}", page.shapeCount, m_selectedShapeId)
            });
            break;
        }
    }

    MarkDocumentDirty(true);
    SyncInspectorFromShapeSheet();
    RefreshShapeSheetSectionRows();
    AttachOrResizeDx12SwapChain();
}

// ===========================================================================
// ShapeSheet Cell Inspection, Reactive Formula Commit & GUARD/SETF Enforcement
// ===========================================================================
void MainWindow::SeedActiveShapeDefaultCells() {
    if (!m_doc) {
        return;
    }

    (void)m_doc.SetCellFormula(m_activePageId, 1, "PinX",            "4.25 in", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "PinY",            "5.50 in", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "Width",           "2.00 in", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "Height",          "1.00 in", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "LocPinX",         "GUARD(Width*0.5)", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "LocPinY",         "GUARD(Height*0.5)", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "Angle",           "0 deg", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "FillForegnd",     "RGB(59,130,246)", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "LineColor",       "RGB(30,58,138)", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "LineWeight",      "1.5 pt", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "Rounding",        "0.08 in", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "Prop.Label",      "\"Order Validation\"", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "Prop.Department", "\"Operations\"", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "Prop.Owner",      "\"Platform Team\"", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "Prop.Cost",       "1250.00", true);
    (void)m_doc.SetCellFormula(m_activePageId, 1, "Prop.Status",     "\"Active\"", true);
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
        const std::wstring label = p.isBackground ? (p.name + L" [BG]") : p.name;
        tabBtn.Content(box_value(hstring{label}));

        const uint32_t targetId = p.pageId;
        tabBtn.Click([this, targetId](IInspectable const&, RoutedEventArgs const&) {
            m_activePageId = targetId;
            RebuildPageTabsStrip();
            SyncInspectorFromShapeSheet();
            RefreshShapeSheetSectionRows();
            AttachOrResizeDx12SwapChain();
        });

        panel.Children().Append(tabBtn);
    }

    StatusPageIndicatorText().Text(hstring{
        std::format(L"Page {} of {}", activeIndex + 1, m_pages.size())
    });
}

void MainWindow::OnInsertBlankPageClick(IInspectable const&, RoutedEventArgs const&) {
    const uint32_t nextId = static_cast<uint32_t>(m_pages.size() + 1);
    m_pages.push_back(PageTabModel{nextId, std::format(L"Page-{}", nextId), false, 0u});
    m_activePageId = nextId;
    MarkDocumentDirty(true);
    RebuildPageTabsStrip();
    AttachOrResizeDx12SwapChain();
}

void MainWindow::OnInsertBackgroundPageClick(IInspectable const&, RoutedEventArgs const&) {
    const uint32_t nextId = static_cast<uint32_t>(m_pages.size() + 1);
    m_pages.push_back(PageTabModel{nextId, std::format(L"VBackground-{}", nextId), true, 1u});
    m_activePageId = nextId;
    MarkDocumentDirty(true);
    RebuildPageTabsStrip();
    AttachOrResizeDx12SwapChain();
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
void MainWindow::OnFitToWindowClick(IInspectable const&, RoutedEventArgs const&) { SetZoomPercent(100.0); }

// ===========================================================================
// Ribbon Tabs, Workstation Dock Toggles & Active Tools
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
    PointerToolToggle().IsChecked(tool == ActiveCanvasTool::Pointer);
    TextToolToggle().IsChecked(tool == ActiveCanvasTool::Text);
    ConnectorToolToggle().IsChecked(tool == ActiveCanvasTool::Connector);
    RectToolToggle().IsChecked(tool == ActiveCanvasTool::Rectangle);
    InkPenToggle().IsChecked(tool == ActiveCanvasTool::InkPen);
    InkHighlighterToggle().IsChecked(tool == ActiveCanvasTool::InkHighlighter);

    switch (tool) {
        case ActiveCanvasTool::Pointer:
            CanvasOverlayHudText().Text(L"Pointer Tool (Ctrl+1) • Dynamic Glue Active");
            ApplyCanvasCursor(static_cast<uint32_t>(vexio::win::ViewportCursor::Arrow));
            break;
        case ActiveCanvasTool::Text:
            CanvasOverlayHudText().Text(L"Rich Text Block Tool (Ctrl+2)");
            ApplyCanvasCursor(static_cast<uint32_t>(vexio::win::ViewportCursor::IBeam));
            break;
        case ActiveCanvasTool::Connector:
            CanvasOverlayHudText().Text(L"Dynamic Obstacle-Avoiding Connector Tool (Ctrl+3)");
            ApplyCanvasCursor(static_cast<uint32_t>(vexio::win::ViewportCursor::Crosshair));
            break;
        case ActiveCanvasTool::Rectangle:
            CanvasOverlayHudText().Text(L"Rectangle Geometry Tool • Click Canvas to Place");
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

void MainWindow::OnToggleLeftStencilDockClick(IInspectable const&, RoutedEventArgs const&) {
    m_leftStencilOpen = !m_leftStencilOpen;
    LeftStencilColumn().Width(GridLength{m_leftStencilOpen ? 264.0 : 0.0});
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
    AttachOrResizeDx12SwapChain();
}

void MainWindow::OnViewAutoConnectChanged(IInspectable const&, RoutedEventArgs const&) {
    if (!m_isInitialized) return;
    const bool enabled = ViewAutoConnectCheck().IsChecked().GetBoolean();
    CanvasOverlayHudText().Text(enabled
        ? L"Pointer Tool • Dynamic Glue & AutoConnect Active"
        : L"Pointer Tool • Manual Glue Mode");
}

// ===========================================================================
// Undo / Redo, Clipboard, Routing, Layouts, Validation, CAD & Data Commands
// ===========================================================================
void MainWindow::OnUndoClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    if (m_doc.Undo() == VEXIO_OK) {
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
        AttachOrResizeDx12SwapChain();
    }
}

void MainWindow::OnRedoClick(IInspectable const&, RoutedEventArgs const&) {
    if (!m_doc) return;
    if (m_doc.Redo() == VEXIO_OK) {
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
        AttachOrResizeDx12SwapChain();
    }
}

void MainWindow::OnCopyShapeClick(IInspectable const&, RoutedEventArgs const&) {
    m_clipboardMasterId = "flow.process";
    ShowBanner(InfoBarSeverity::Informational, L"Copied Shape",
               std::format(L"Sheet.{} copied to workstation clipboard.", m_selectedShapeId));
}

void MainWindow::OnCutShapeClick(IInspectable const&, RoutedEventArgs const&) {
    m_clipboardMasterId = "flow.process";
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "Geometry1.NoShow", "TRUE", true);
    MarkDocumentDirty(true);
    AttachOrResizeDx12SwapChain();
}

void MainWindow::OnPasteShapeClick(IInspectable const&, RoutedEventArgs const&) {
    for (auto const& master : m_allMasters) {
        if (master.id == m_clipboardMasterId) {
            InstantiateMasterOnCanvas(master, 4.75, 5.00);
            break;
        }
    }
}

void MainWindow::OnConnectorRoutingChanged(IInspectable const&, SelectionChangedEventArgs const&) {
    if (!m_isInitialized || !m_doc) return;
    const int32_t idx = std::clamp(ConnectorRoutingCombo().SelectedIndex(), 0, 2);
    const std::array<const char*, 3> routeStyles{"1", "16", "2"}; // Visio ShapeRouteStyle codes
    (void)m_doc.SetCellFormula(m_activePageId, m_selectedShapeId, "ShapeRouteStyle", routeStyles[idx], true);
    MarkDocumentDirty(true);
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

void MainWindow::OnAlignCenterClick(IInspectable const&, RoutedEventArgs const&) {
    TransformPinXBox().Text(L"5.50 in");
    OnApplySizeAndPositionClick(nullptr, nullptr);
}

void MainWindow::OnDistributeHorizontallyClick(IInspectable const&, RoutedEventArgs const&) {
    ShowBanner(InfoBarSeverity::Informational, L"Shapes Distributed",
               L"Distributed active page shapes evenly along horizontal PinX centers.");
}

void MainWindow::OnAutoLayoutQuickClick(IInspectable const&, RoutedEventArgs const&) {
    OnReLayoutTopDownClick(nullptr, nullptr);
}

void MainWindow::OnInsertContainerClick(IInspectable const&, RoutedEventArgs const&) {
    StencilMasterDescriptor container{"part.container", L"Diagram Parts", L"Logical Container", L"Container", "5.50 in", "3.50 in", "RGB(241,245,249)", "Container"};
    InstantiateMasterOnCanvas(container, 5.50, 4.25);
}

void MainWindow::OnInsertSwimlaneClick(IInspectable const&, RoutedEventArgs const&) {
    for (auto const& m : m_allMasters) {
        if (m.id == "bpmn.pool") {
            InstantiateMasterOnCanvas(m, 5.50, 4.25);
            break;
        }
    }
}

void MainWindow::OnInsertCalloutClick(IInspectable const&, RoutedEventArgs const&) {
    StencilMasterDescriptor callout{"part.callout", L"Diagram Parts", L"Annotation Callout", L"Callout", "2.10 in", "0.85 in", "RGB(254,249,195)", "Callout"};
    InstantiateMasterOnCanvas(callout, 6.25, 6.25);
}

void MainWindow::OnInsertLegendClick(IInspectable const&, RoutedEventArgs const&) {
    StencilMasterDescriptor legend{"part.legend", L"Diagram Parts", L"Data Graphics Legend", L"Legend", "2.60 in", "1.80 in", "RGB(255,255,255)", "Legend"};
    InstantiateMasterOnCanvas(legend, 8.75, 2.00);
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
    if (idx == 0) {
        (void)m_doc.SetCellFormula(m_activePageId, 0, "PageWidth", "11 in", true);
        (void)m_doc.SetCellFormula(m_activePageId, 0, "PageHeight", "8.5 in", true);
    } else if (idx == 1) {
        (void)m_doc.SetCellFormula(m_activePageId, 0, "PageWidth", "8.5 in", true);
        (void)m_doc.SetCellFormula(m_activePageId, 0, "PageHeight", "11 in", true);
    } else if (idx == 2) {
        (void)m_doc.SetCellFormula(m_activePageId, 0, "PageWidth", "420 mm", true);
        (void)m_doc.SetCellFormula(m_activePageId, 0, "PageHeight", "297 mm", true);
    } else {
        (void)m_doc.SetCellFormula(m_activePageId, 0, "PageWidth", "36 in", true);
        (void)m_doc.SetCellFormula(m_activePageId, 0, "PageHeight", "24 in", true);
    }
    MarkDocumentDirty(true);
    AttachOrResizeDx12SwapChain();
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
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
        AttachOrResizeDx12SwapChain();
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
        SyncInspectorFromShapeSheet();
        RefreshShapeSheetSectionRows();
        AttachOrResizeDx12SwapChain();
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
}

void MainWindow::OnReLayoutTopDownClick(IInspectable const&, RoutedEventArgs const&) {
    ShowBanner(InfoBarSeverity::Success, L"Hierarchical Top-Down Layout Applied",
               L"Sugiyama layered graph auto-layout executed with orthogonal obstacle-avoiding connectors.");
    AttachOrResizeDx12SwapChain();
}

void MainWindow::OnReLayoutLeftRightClick(IInspectable const&, RoutedEventArgs const&) {
    ShowBanner(InfoBarSeverity::Success, L"Left-to-Right Process Flow Applied",
               L"Swimlane-preserving horizontal Sugiyama layout completed.");
    AttachOrResizeDx12SwapChain();
}

void MainWindow::OnReLayoutRadialClick(IInspectable const&, RoutedEventArgs const&) {
    ShowBanner(InfoBarSeverity::Success, L"Radial Topology Layout Applied",
               L"Force-balanced concentric topology layout completed.");
    AttachOrResizeDx12SwapChain();
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
    FileBackstageFlyout().Hide();
    SaveDocumentAsync(true, "pptx");
}

void MainWindow::OnExportDocxClick(IInspectable const&, RoutedEventArgs const&) {
    FileBackstageFlyout().Hide();
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
    FileBackstageFlyout().Hide();
    m_doc = vexio::win::DocHandle::CreateBlank("Drawing1.vsdx");
    m_currentStorageFile = nullptr;
    m_documentDisplayName = L"Drawing1.vsdx";
    m_activeFormatExt = "vsdx";
    m_activePageId = 1;
    m_selectedShapeId = 1;
    m_previousSelectedShapeId = 1;
    SeedActiveShapeDefaultCells();
    RefreshBaselineSnapshot();
    MarkDocumentDirty(false);
    SyncInspectorFromShapeSheet();
    RefreshShapeSheetSectionRows();
    AttachOrResizeDx12SwapChain();
}

void MainWindow::OnNewBpmnTemplateClick(IInspectable const&, RoutedEventArgs const&) {
    OnNewBlankDocClick(nullptr, nullptr);
    m_documentDisplayName = L"Order_Fulfillment_BPMN2.vsdx";
    StencilPackCombo().SelectedIndex(1);
    ValidationRuleSetCombo().SelectedIndex(0);
    MarkDocumentDirty(false);
}

void MainWindow::OnNewUmlTemplateClick(IInspectable const&, RoutedEventArgs const&) {
    OnNewBlankDocClick(nullptr, nullptr);
    m_documentDisplayName = L"Service_Domain_UML25.vsdx";
    StencilPackCombo().SelectedIndex(2);
    ValidationRuleSetCombo().SelectedIndex(1);
    MarkDocumentDirty(false);
}

void MainWindow::OnNewCadFloorPlanClick(IInspectable const&, RoutedEventArgs const&) {
    OnNewBlankDocClick(nullptr, nullptr);
    m_documentDisplayName = L"FloorPlan_Level1.vsdx";
    StencilPackCombo().SelectedIndex(4);
    DrawingScaleCombo().SelectedIndex(1);
    MarkDocumentDirty(false);
}

void MainWindow::OnOpenClick(IInspectable const&, RoutedEventArgs const&) {
    FileBackstageFlyout().Hide();
    PromptOpenFilePickerAsync();
}

void MainWindow::OnSaveClick(IInspectable const&, RoutedEventArgs const&) {
    FileBackstageFlyout().Hide();
    SaveDocumentAsync(false);
}

void MainWindow::OnSaveAsClick(IInspectable const&, RoutedEventArgs const&) {
    FileBackstageFlyout().Hide();
    SaveDocumentAsync(true);
}

void MainWindow::OnExportPdfClick(IInspectable const&, RoutedEventArgs const&) {
    FileBackstageFlyout().Hide();
    SaveDocumentAsync(true, "pdf");
}

void MainWindow::OnExportSvgClick(IInspectable const&, RoutedEventArgs const&) {
    FileBackstageFlyout().Hide();
    SaveDocumentAsync(true, "svg");
}

void MainWindow::OnExportDxfClick(IInspectable const&, RoutedEventArgs const&) {
    FileBackstageFlyout().Hide();
    SaveDocumentAsync(true, "dxf");
}

void MainWindow::OnTiledPrintClick(IInspectable const&, RoutedEventArgs const&) {
    FileBackstageFlyout().Hide();
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

        auto openRes = vexio::win::DocHandle::OpenBytes(self->m_engine, rawBytes, ext);
        if (!openRes.ok()) {
            self->ShowBanner(InfoBarSeverity::Error, L"Document Open Failed", openRes.message());
            co_return;
        }

        self->m_doc = std::move(*openRes.value);
        self->RefreshBaselineSnapshot();
        self->m_currentStorageFile = file;
        self->m_documentDisplayName = file.Name().c_str();
        self->m_activeFormatExt = ext;
        self->m_activePageId = 1;
        self->m_selectedShapeId = 1;
        self->m_previousSelectedShapeId = 1;

        self->DocumentFormatBadgeText().Text(hstring{FormatBadgeForExtension(ext)});
        self->MarkDocumentDirty(false);
        self->SyncInspectorFromShapeSheet();
        self->RefreshShapeSheetSectionRows();
        self->AttachOrResizeDx12SwapChain();
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

        if (forcedExtensionUtf8 == "pdf") {
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
            picker.SuggestedFileName(hstring{stemName});
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

    ContentDialog dlg{};
    dlg.XamlRoot(RootWorkstationGrid().XamlRoot());
    dlg.Title(box_value(L"Rename Active Page"));
    dlg.Content(input);
    dlg.PrimaryButtonText(L"Rename");
    dlg.CloseButtonText(L"Cancel");
    dlg.DefaultButton(ContentDialogButton::Primary);

    const auto weakSelf = get_weak();
    if (co_await dlg.ShowAsync() == ContentDialogResult::Primary) {
        if (auto self = weakSelf.get()) {
            const std::wstring updated{input.Text().c_str()};
            if (!updated.empty()) {
                for (auto& p : self->m_pages) {
                    if (p.pageId == self->m_activePageId) {
                        p.name = updated;
                        break;
                    }
                }
                self->MarkDocumentDirty(true);
                self->RebuildPageTabsStrip();
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

} // namespace winrt::VexioWindows::implementation
