#pragma once

#include "pch.h"
#include "MainWindow.g.h"
#include "VexioInterop.h"

namespace winrt::VexioWindows::implementation {

enum class ActiveCanvasTool : uint8_t {
    Pointer,
    Text,
    Connector,
    Rectangle,
    InkPen,
    InkHighlighter,
};

struct StencilMasterDescriptor {
    std::string id;
    std::wstring packName;
    std::wstring displayName;
    std::wstring categoryTag;
    std::string defaultWidthFormula;
    std::string defaultHeightFormula;
    std::string defaultFillFormula;
    std::string stereotypeOrRuleType;
};

struct PageTabModel {
    uint32_t pageId{1};
    std::wstring name{L"Page-1"};
    bool isBackground{false};
    uint32_t shapeCount{1};
};

struct ExternalDataRecord {
    std::wstring rowId;
    std::wstring component;
    std::wstring department;
    std::wstring owner;
    std::wstring costUsd;
    std::wstring status;
};

struct DiagramLayerModel {
    std::wstring name;
    std::wstring colorHex;
    bool visible{true};
    bool locked{false};
    bool printable{true};
    bool snapGlue{true};
};

struct ShapeSheetCellRow {
    std::string cellName;
    std::wstring formula;
    std::wstring evaluatedValue;
    bool isGuarded{false};
};

struct MainWindow : MainWindowT<MainWindow> {
    MainWindow();

    // External Launch & File Association Entry Points (used by App.xaml.cpp)
    winrt::fire_and_forget OpenStorageFileAsync(winrt::Windows::Storage::StorageFile file);
    winrt::fire_and_forget OpenFileFromPathAsync(winrt::hstring filePath);

    // Keyboard Accelerators
    void OnPointerToolAccelerator(winrt::Microsoft::UI::Xaml::Input::KeyboardAccelerator const& sender,
                                  winrt::Microsoft::UI::Xaml::Input::KeyboardAcceleratorInvokedEventArgs const& args);
    void OnTextToolAccelerator(winrt::Microsoft::UI::Xaml::Input::KeyboardAccelerator const& sender,
                               winrt::Microsoft::UI::Xaml::Input::KeyboardAcceleratorInvokedEventArgs const& args);
    void OnConnectorToolAccelerator(winrt::Microsoft::UI::Xaml::Input::KeyboardAccelerator const& sender,
                                    winrt::Microsoft::UI::Xaml::Input::KeyboardAcceleratorInvokedEventArgs const& args);
    void OnUndoAccelerator(winrt::Microsoft::UI::Xaml::Input::KeyboardAccelerator const& sender,
                           winrt::Microsoft::UI::Xaml::Input::KeyboardAcceleratorInvokedEventArgs const& args);
    void OnRedoAccelerator(winrt::Microsoft::UI::Xaml::Input::KeyboardAccelerator const& sender,
                           winrt::Microsoft::UI::Xaml::Input::KeyboardAcceleratorInvokedEventArgs const& args);
    void OnSaveAccelerator(winrt::Microsoft::UI::Xaml::Input::KeyboardAccelerator const& sender,
                           winrt::Microsoft::UI::Xaml::Input::KeyboardAcceleratorInvokedEventArgs const& args);
    void OnOpenAccelerator(winrt::Microsoft::UI::Xaml::Input::KeyboardAccelerator const& sender,
                           winrt::Microsoft::UI::Xaml::Input::KeyboardAcceleratorInvokedEventArgs const& args);
    void OnTiledPrintAccelerator(winrt::Microsoft::UI::Xaml::Input::KeyboardAccelerator const& sender,
                                 winrt::Microsoft::UI::Xaml::Input::KeyboardAcceleratorInvokedEventArgs const& args);
    void OnPageSetupAccelerator(winrt::Microsoft::UI::Xaml::Input::KeyboardAccelerator const& sender,
                                winrt::Microsoft::UI::Xaml::Input::KeyboardAcceleratorInvokedEventArgs const& args);
    void OnToggleShapeSheetAccelerator(winrt::Microsoft::UI::Xaml::Input::KeyboardAccelerator const& sender,
                                       winrt::Microsoft::UI::Xaml::Input::KeyboardAcceleratorInvokedEventArgs const& args);
    void OnMacroEditorAccelerator(winrt::Microsoft::UI::Xaml::Input::KeyboardAccelerator const& sender,
                                  winrt::Microsoft::UI::Xaml::Input::KeyboardAcceleratorInvokedEventArgs const& args);

    // Quick Access Toolbar & File Backstage Commands
    void OnSaveClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnSaveAsClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnOpenClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnUndoClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnRedoClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnTiledPrintClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnTouchMouseModeToggle(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);

    void OnNewBlankDocClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnNewBpmnTemplateClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnNewUmlTemplateClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnNewCadFloorPlanClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnExportPdfClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnExportSvgClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnExportDxfClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);

    // Ribbon Tab Switching & Commands
    void OnRibbonTabChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);

    void OnPasteShapeClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnCutShapeClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnCopyShapeClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnPointerToolClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnTextToolClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnConnectorToolClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnRectToolClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnConnectorRoutingChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::SelectionChangedEventArgs const& e);
    void OnQuickStyleAccentClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnQuickStyleNeutralClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnAlignCenterClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnDistributeHorizontallyClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnAutoLayoutQuickClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);

    void OnInsertBlankPageClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnInsertBackgroundPageClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnInsertContainerClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnInsertSwimlaneClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnInsertCalloutClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnInsertLegendClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnInsertCadDrawingClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnLockCadScaleClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnConvertCadToShapeSheetClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);

    void OnPageSetupClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnPageOrientationChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::SelectionChangedEventArgs const& e);
    void OnDrawingScaleChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::SelectionChangedEventArgs const& e);
    void OnThemeChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::SelectionChangedEventArgs const& e);
    void OnThemeVariantChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::SelectionChangedEventArgs const& e);
    void OnVisualPresetGradLinearClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnVisualPresetShadowSoftClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnVisualPresetBevel3dClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnVisualPresetSketchyClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnLineJumpStyleChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::SelectionChangedEventArgs const& e);
    void OnReLayoutTopDownClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnReLayoutLeftRightClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnReLayoutRadialClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);

    void OnLinkExternalDataClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnRefreshExternalDataClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnPushTwoWaySyncClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnImportDataVisualizerClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnExportDataVisualizerClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnImportCloudDiscoveryClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnImportEntraOrLdifClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnExportPowerBiClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnApplyDataBarsClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnApplyIconSetsClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnApplyColorByValueClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnGenerateBomReportClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);

    void OnValidationRuleSetChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::SelectionChangedEventArgs const& e);
    void OnCheckDiagramClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnCreateSubprocessPageClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnLinkSubprocessClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnExportPowerAutomateClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);

    void OnInkPenToggleClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnInkHighlighterToggleClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnConvertInkToShapesClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnNewCommentClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnToggleMarkupOverlayClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnCompareDiagramClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnExportPptxClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnExportDocxClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);

    void OnViewRulersChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnViewGridChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnViewAutoConnectChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnToggleLeftStencilDockClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnToggleRightTaskPanesClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnToggleShapeSheetClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnZoom50Click(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnZoom100Click(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnZoom200Click(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnFitToWindowClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);

    void OnGuardSelectedCellClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnBooleanUnionClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnBooleanCombineClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnBooleanFragmentClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnBooleanIntersectClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnBooleanSubtractClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnBooleanTrimClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnBooleanJoinClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnBooleanOffsetClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnOpenMacroEditorClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);

    // Stencils Dock & Drag-and-Drop
    void OnStencilSearchTextChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::TextChangedEventArgs const& e);
    void OnStencilPackSelectionChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::SelectionChangedEventArgs const& e);
    void OnStencilMasterDragStarting(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::DragItemsStartingEventArgs const& e);
    void OnStencilMasterItemClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::ItemClickEventArgs const& e);

    // DirectX 12 SwapChainPanel, Rulers & Pointer/Pen/Wheel Input
    void OnToggleRulerUnitClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnRulerCanvasSizeChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::SizeChangedEventArgs const& e);
    void OnSwapChainPanelLoaded(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnSwapChainPanelSizeChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::SizeChangedEventArgs const& e);
    void OnSwapChainCompositionScaleChanged(winrt::Microsoft::UI::Xaml::Controls::SwapChainPanel const& sender, winrt::Windows::Foundation::IInspectable const& args);
    void OnCanvasPointerPressed(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Input::PointerRoutedEventArgs const& e);
    void OnCanvasPointerMoved(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Input::PointerRoutedEventArgs const& e);
    void OnCanvasPointerReleased(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Input::PointerRoutedEventArgs const& e);
    void OnCanvasPointerWheelChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Input::PointerRoutedEventArgs const& e);
    void OnCanvasDragOver(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::DragEventArgs const& e);
    void OnCanvasDrop(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::DragEventArgs const& e);

    // Right Task Panes
    void OnShapeDataFieldCommitted(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnPropStatusChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::SelectionChangedEventArgs const& e);
    void OnApplyShapeDataClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnApplySizeAndPositionClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnApplyFormatShapeClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnRefreshPanZoomPreviewClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnBindSelectedDataRowClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnAddLayerClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);

    // ShapeSheet Split-View & Floating Formula Bar
    void OnShapeSheetSplitterPointerPressed(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Input::PointerRoutedEventArgs const& e);
    void OnShapeSheetSplitterPointerMoved(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Input::PointerRoutedEventArgs const& e);
    void OnShapeSheetSplitterPointerReleased(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Input::PointerRoutedEventArgs const& e);
    void OnShapeSheetSectionChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::SelectionChangedEventArgs const& e);
    void OnShapeSheetCellSelectionChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::SelectionChangedEventArgs const& e);
    void OnShapeSheetFormulaKeyDown(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Input::KeyRoutedEventArgs const& e);
    void OnFloatingFormulaKeyDown(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Input::KeyRoutedEventArgs const& e);
    void OnShapeSheetForceSetfToggled(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnFloatingForceSetfToggled(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnCommitShapeSheetFormulaClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnCommitFloatingFormulaClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnRevertFormulaClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);

    // Page Tabs & Status Bar
    void OnMovePageLeftClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnMovePageRightClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnRenameActivePageClick(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::RoutedEventArgs const& e);
    void OnZoomSliderValueChanged(winrt::Windows::Foundation::IInspectable const& sender, winrt::Microsoft::UI::Xaml::Controls::Primitives::RangeBaseValueChangedEventArgs const& e);

private:
    // Windowing & HWND Helpers
    [[nodiscard]] HWND GetNativeWindowHandle() const noexcept;
    void ConfigureCustomTitleBar();

    // DirectX 12 SwapChain Binding & Input Translation
    void AttachOrResizeDx12SwapChain();
    void DispatchCanvasPointerEvent(uint32_t kindOverride,
                                    winrt::Microsoft::UI::Xaml::Input::PointerRoutedEventArgs const& e);
    void ApplyCanvasCursor(uint32_t cursorKind);

    // Stencils, Rulers, Page Tabs, Task Panes & ShapeSheet Synchronization
    void InitializeStencilCatalog();
    void RefreshStencilList();
    void InstantiateMasterOnCanvas(StencilMasterDescriptor const& master, double pinXInches, double pinYInches);

    void InitializeExternalDataAndLayers();
    void RefreshExternalDataList();
    void RefreshLayersList();
    void RebuildPageTabsStrip();
    void RebuildRulerTicks();

    void SeedActiveShapeDefaultCells();
    void SyncInspectorFromShapeSheet();
    void RefreshShapeSheetSectionRows();
    void CommitFormulaToActiveCell(std::wstring const& rawFormulaText);
    void SetActiveTool(ActiveCanvasTool tool);
    void SetZoomPercent(double zoomPercent);
    void ShowBanner(winrt::Microsoft::UI::Xaml::Controls::InfoBarSeverity severity,
                    std::wstring_view title,
                    std::wstring_view message);
    void MarkDocumentDirty(bool dirty);

    // Phase 7/8 Plan 2 Operations & Dialog Helpers
    void ApplySelectedThemeAndVariant();
    void ApplyVisualPresetSlug(std::string_view presetSlugUtf8, std::wstring_view displayLabel);
    void ExecuteBooleanOp(std::string_view opSlugUtf8, std::wstring_view displayLabel, double offsetIn = 0.18);
    void RefreshBaselineSnapshot();

    // File Open / Save / Export Pickers & Plan 2 Connector Dialogs
    winrt::fire_and_forget PromptOpenFilePickerAsync();
    winrt::fire_and_forget SaveDocumentAsync(bool forceSaveAs, std::string forcedExtensionUtf8 = {});
    winrt::fire_and_forget ImportDataVisualizerPickerAsync();
    winrt::fire_and_forget ExportDataVisualizerXlsxAsync();
    winrt::fire_and_forget ShowCloudDiscoveryDialogAsync();
    winrt::fire_and_forget ShowEntraOrLdifDialogAsync();
    winrt::fire_and_forget ShowDiagramCompareDialogAsync();
    winrt::fire_and_forget ShowMacroEditorDialogAsync();
    winrt::fire_and_forget ShowRenamePageDialogAsync();
    winrt::fire_and_forget ShowBomReportDialogAsync();
    winrt::fire_and_forget ShowPageSetupDialogAsync();
    winrt::fire_and_forget ShowTiledPrintDialogAsync();

private:
    // RAII Rust Core Handles (vexio.h)
    vexio::win::EngineHandle   m_engine{};
    vexio::win::DocHandle      m_doc{};
    vexio::win::DocHandle      m_baselineDoc{};
    vexio::win::ViewportHandle m_viewport{};

    // Active Document & Selection State
    std::wstring m_documentDisplayName{L"Drawing1.vsdx"};
    std::string  m_activeFormatExt{"vsdx"};
    winrt::Windows::Storage::StorageFile m_currentStorageFile{nullptr};
    bool         m_isDirty{false};
    bool         m_isInitialized{false};

    uint32_t     m_activePageId{1};
    uint64_t     m_selectedShapeId{1};
    uint64_t     m_previousSelectedShapeId{1};
    uint64_t     m_nextShapeId{2};
    std::string  m_selectedCellName{"Width"};

    // Workstation View & Tool State
    ActiveCanvasTool m_activeTool{ActiveCanvasTool::Pointer};
    bool   m_leftStencilOpen{true};
    bool   m_rightTaskPaneOpen{true};
    bool   m_shapeSheetOpen{false};
    bool   m_touchMode{false};
    bool   m_metricUnits{false};
    bool   m_forceSetf{false};
    double m_zoomPercent{100.0};
    double m_shapeSheetHeightPx{248.0};

    // GridSplitter Drag State
    bool   m_draggingSplitter{false};
    double m_splitterStartPointerY{0.0};
    double m_splitterStartHeight{248.0};

    // Catalogs & Models
    std::vector<StencilMasterDescriptor> m_allMasters{};
    std::vector<StencilMasterDescriptor> m_filteredMasters{};
    std::vector<PageTabModel>            m_pages{};
    std::vector<ExternalDataRecord>      m_externalRecords{};
    std::vector<DiagramLayerModel>       m_layers{};
    std::vector<ShapeSheetCellRow>       m_currentSectionRows{};
    std::string                          m_clipboardMasterId{"flow.process"};
};

} // namespace winrt::VexioWindows::implementation

namespace winrt::VexioWindows::factory_implementation {

struct MainWindow : MainWindowT<MainWindow, implementation::MainWindow> {};

} // namespace winrt::VexioWindows::factory_implementation
