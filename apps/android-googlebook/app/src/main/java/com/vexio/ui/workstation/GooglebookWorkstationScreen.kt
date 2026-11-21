package com.vexio.ui.workstation

import android.app.Activity
import android.content.ClipData
import android.content.ClipDescription
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.view.DragEvent
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.draganddrop.dragAndDropSource
import androidx.compose.foundation.draganddrop.dragAndDropTarget
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Redo
import androidx.compose.material.icons.automirrored.filled.Undo
import androidx.compose.material.icons.filled.AccountTree
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Architecture
import androidx.compose.material.icons.filled.Brush
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.CropSquare
import androidx.compose.material.icons.filled.DeleteOutline
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.Functions
import androidx.compose.material.icons.filled.GridOn
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.NearMe
import androidx.compose.material.icons.filled.OpenInNew
import androidx.compose.material.icons.filled.PanTool
import androidx.compose.material.icons.filled.Print
import androidx.compose.material.icons.filled.RadioButtonUnchecked
import androidx.compose.material.icons.filled.Save
import androidx.compose.material.icons.filled.Schema
import androidx.compose.material.icons.filled.TableChart
import androidx.compose.material.icons.filled.TextFields
import androidx.compose.material.icons.filled.Timeline
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material.icons.filled.Verified
import androidx.compose.material.icons.filled.ViewSidebar
import androidx.compose.material.icons.filled.ZoomIn
import androidx.compose.material.icons.filled.ZoomOut
import androidx.compose.material3.Badge
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draganddrop.DragAndDropEvent
import androidx.compose.ui.draganddrop.DragAndDropTarget
import androidx.compose.ui.draganddrop.DragAndDropTransferData
import androidx.compose.ui.draganddrop.mimeTypes
import androidx.compose.ui.draganddrop.toAndroidDragEvent
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusTarget
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isAltPressed
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.PointerIcon
import androidx.compose.ui.input.pointer.pointerHoverIcon
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.input.motionprediction.MotionEventPredictor
import com.vexio.MainActivity
import com.vexio.core.BuiltInStencilCatalog
import com.vexio.core.CrossWindowShapePayload
import com.vexio.core.FormulaCommitStatus
import com.vexio.core.Plan2SamplePayloads
import com.vexio.core.ShapeSheetCellEntry
import com.vexio.core.StencilMasterItem
import com.vexio.core.VexioBooleanOp
import com.vexio.core.VexioCursorKind
import com.vexio.core.VexioSessionViewModel
import com.vexio.core.VexioThemePreset
import com.vexio.core.VexioTool
import com.vexio.core.VexioVisualPreset
import com.vexio.core.WorkstationUiState
import com.vexio.ui.theme.LocalVexioSemanticColors
import com.vexio.ui.theme.VexioExpressiveMotion
import com.vexio.ui.theme.rememberExpressiveScaleState
import com.vexio.ui.theme.rememberExpressiveToolButtonShape
import com.vexio.ui.theme.rememberMorphingToolbarShape
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

const val ACTION_NEW_DOCUMENT_WINDOW = "com.vexio.action.NEW_DOCUMENT_WINDOW"
const val EXTRA_DOCUMENT_TEMPLATE = "vexio.template"
const val VEXIO_MASTER_CLIP_MIME = "application/vnd.vexio.master+json"

private val SUPPORTED_DIAGRAM_EXTENSIONS = setOf(
    "vsdx", "vsdm", "vssx", "vstx", "vsd", "vdx", "dwg", "dxf"
)

/**
 * Navigation 3 typed workstation destination keys enabling adaptive backstack management across
 * compact convertible windows, split-screen dual panes, and ultra-wide Googlebook monitors.
 */
sealed interface WorkstationNavKey {
    data object CanvasMain : WorkstationNavKey
    data class StencilDrawer(val category: String) : WorkstationNavKey
    data object InspectorPane : WorkstationNavKey
    data object ShapeSheetStudio : WorkstationNavKey
    data object Plan2Operations : WorkstationNavKey
}

enum class AdaptiveWorkstationPosture {
    COMPACT_CONVERTIBLE,
    EXPANDED_DUAL_PANE,
    ULTRAWIDE_DESKTOP
}

/**
 * Launches a brand-new, independent freeform desktop window on Googlebook OS so multiple Visio
 * diagrams run side-by-side simultaneously with multi-resume (`allow_multiple_resumed_activities`).
 */
fun launchNewDocumentWindow(
    context: Context,
    template: String = "blank",
    initialDocumentUri: Uri? = null
) {
    val intent = Intent(context, MainActivity::class.java).apply {
        action = if (initialDocumentUri != null) Intent.ACTION_VIEW else ACTION_NEW_DOCUMENT_WINDOW
        if (initialDocumentUri != null) {
            data = initialDocumentUri
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        }
        putExtra(EXTRA_DOCUMENT_TEMPLATE, template)
        putExtra("vexio.window_nonce", System.nanoTime())
        addFlags(
            Intent.FLAG_ACTIVITY_NEW_DOCUMENT or
                Intent.FLAG_ACTIVITY_MULTIPLE_TASK or
                Intent.FLAG_ACTIVITY_NEW_TASK
        )
    }
    context.startActivity(intent)
}

/**
 * Maps `VexioCursorKind` returned by `NativeBridge.nativePushMotionBatch` to hardware OS cursors
 * via `android.view.PointerIcon.getSystemIcon`.
 */
fun resolveHardwarePointerIcon(
    context: Context,
    cursorKind: VexioCursorKind
): android.view.PointerIcon {
    val systemType = when (cursorKind) {
        VexioCursorKind.ARROW -> android.view.PointerIcon.TYPE_ARROW
        VexioCursorKind.CROSSHAIR -> android.view.PointerIcon.TYPE_CROSSHAIR
        VexioCursorKind.MOVE_ALL -> android.view.PointerIcon.TYPE_ALL_SCROLL
        VexioCursorKind.RESIZE_NS -> android.view.PointerIcon.TYPE_VERTICAL_DOUBLE_ARROW
        VexioCursorKind.RESIZE_EW -> android.view.PointerIcon.TYPE_HORIZONTAL_DOUBLE_ARROW
        VexioCursorKind.RESIZE_NWSE -> android.view.PointerIcon.TYPE_TOP_LEFT_DIAGONAL_DOUBLE_ARROW
        VexioCursorKind.RESIZE_NESW -> android.view.PointerIcon.TYPE_TOP_RIGHT_DIAGONAL_DOUBLE_ARROW
        VexioCursorKind.TEXT_IBEAM -> android.view.PointerIcon.TYPE_TEXT
        VexioCursorKind.GRAB_HAND -> android.view.PointerIcon.TYPE_GRAB
        VexioCursorKind.GRABBING_HAND -> android.view.PointerIcon.TYPE_GRABBING
        VexioCursorKind.GLUE_SNAP -> android.view.PointerIcon.TYPE_ALIAS
        VexioCursorKind.FORBIDDEN_GUARD -> android.view.PointerIcon.TYPE_NO_DROP
    }
    return android.view.PointerIcon.getSystemIcon(context, systemType)
}

/**
 * Primary Googlebook OS Workstation Composable.
 *
 * Delivers zero-flicker freeform desktop window resizing, Compose Adaptive + Navigation 3 multi-pane
 * reflow, Vulkan `SurfaceView` zero-copy motion/USI stylus batching, hardware mouse cursors,
 * cross-window Drag & Drop, hardware keyboard shortcuts, SAF file triggers, and native vector PDF printing.
 */
@Composable
fun GooglebookWorkstationScreen(
    viewModel: VexioSessionViewModel,
    onOpenDocumentRequested: () -> Unit,
    onSaveDocumentRequested: () -> Unit,
    onSaveAsDocumentRequested: () -> Unit,
    onExternalUriDropped: (Uri) -> Unit,
    onPrintRequested: () -> Unit = { }
) {
    val context = LocalContext.current
    val uiState by viewModel.uiState.collectAsState()
    val semanticColors = LocalVexioSemanticColors.current
    val rootFocusRequester = remember { FocusRequester() }
    val formulaFocusRequester = remember { FocusRequester() }

    // Navigation 3 typed backstack preserving adaptive pane state across live window resizing
    val navBackStack = remember {
        mutableStateListOf<WorkstationNavKey>(WorkstationNavKey.CanvasMain)
    }

    DisposableEffect(Unit) {
        rootFocusRequester.requestFocus()
        onDispose { }
    }

    BoxWithConstraints(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .focusRequester(rootFocusRequester)
            .focusTarget()
            .onPreviewKeyEvent { keyEvent ->
                if (keyEvent.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                val ctrlOrMeta = keyEvent.isCtrlPressed || keyEvent.isMetaPressed
                val shift = keyEvent.isShiftPressed
                val alt = keyEvent.isAltPressed

                when {
                    // Shift + F5 -> Toggle Visio ShapeSheet Studio Drawer
                    shift && !ctrlOrMeta && keyEvent.key == Key.F5 -> {
                        viewModel.toggleShapeSheetDrawer()
                        if (navBackStack.lastOrNull() != WorkstationNavKey.ShapeSheetStudio) {
                            navBackStack.add(WorkstationNavKey.ShapeSheetStudio)
                        }
                        true
                    }
                    // Delete / ForwardDelete -> Delete selected shape
                    !ctrlOrMeta && (keyEvent.key == Key.Delete || keyEvent.key == Key.Backspace && alt) -> {
                        viewModel.deleteSelectedShape()
                        true
                    }
                    // Ctrl + Z / Ctrl + Shift + Z -> Undo / Redo
                    ctrlOrMeta && !shift && keyEvent.key == Key.Z -> {
                        viewModel.performUndo()
                        true
                    }
                    ctrlOrMeta && ((shift && keyEvent.key == Key.Z) || keyEvent.key == Key.Y) -> {
                        viewModel.performRedo()
                        true
                    }
                    // Ctrl + S / Ctrl + Shift + S -> SAF Save / Save As
                    ctrlOrMeta && !shift && keyEvent.key == Key.S -> {
                        onSaveDocumentRequested()
                        true
                    }
                    ctrlOrMeta && shift && keyEvent.key == Key.S -> {
                        onSaveAsDocumentRequested()
                        true
                    }
                    // Ctrl + O -> SAF Open Document
                    ctrlOrMeta && keyEvent.key == Key.O -> {
                        onOpenDocumentRequested()
                        true
                    }
                    // Ctrl + N -> Launch New Independent Side-by-Side Document Window
                    ctrlOrMeta && keyEvent.key == Key.N -> {
                        launchNewDocumentWindow(context, template = "blank")
                        true
                    }
                    // Ctrl + P -> Native OS Vector PDF PrintManager
                    ctrlOrMeta && keyEvent.key == Key.P -> {
                        onPrintRequested()
                        true
                    }
                    // Ctrl + 1 / 2 / 3 / 4 / 8 / 9 -> Visio Authoring Tool Parity
                    ctrlOrMeta && keyEvent.key == Key.One -> {
                        viewModel.selectTool(VexioTool.POINTER)
                        true
                    }
                    ctrlOrMeta && keyEvent.key == Key.Two -> {
                        viewModel.selectTool(VexioTool.TEXT_BLOCK)
                        true
                    }
                    ctrlOrMeta && keyEvent.key == Key.Three -> {
                        viewModel.selectTool(VexioTool.CONNECTOR)
                        true
                    }
                    ctrlOrMeta && keyEvent.key == Key.Four -> {
                        viewModel.selectTool(VexioTool.STYLUS_INK)
                        true
                    }
                    ctrlOrMeta && keyEvent.key == Key.Five -> {
                        viewModel.selectTool(VexioTool.FREEFORM)
                        true
                    }
                    ctrlOrMeta && keyEvent.key == Key.Eight -> {
                        viewModel.selectTool(VexioTool.RECTANGLE)
                        true
                    }
                    ctrlOrMeta && keyEvent.key == Key.Nine -> {
                        viewModel.selectTool(VexioTool.ELLIPSE)
                        true
                    }
                    // Ctrl + Plus / Minus / 0 -> Viewport Zoom
                    ctrlOrMeta && (keyEvent.key == Key.Equals || keyEvent.key == Key.NumPadAdd) -> {
                        viewModel.adjustZoom(15)
                        true
                    }
                    ctrlOrMeta && (keyEvent.key == Key.Minus || keyEvent.key == Key.NumPadSubtract) -> {
                        viewModel.adjustZoom(-15)
                        true
                    }
                    ctrlOrMeta && keyEvent.key == Key.Zero -> {
                        viewModel.setZoomPercent(100)
                        true
                    }
                    else -> false
                }
            }
    ) {
        val posture = when {
            maxWidth >= 1180.dp -> AdaptiveWorkstationPosture.ULTRAWIDE_DESKTOP
            maxWidth >= 760.dp -> AdaptiveWorkstationPosture.EXPANDED_DUAL_PANE
            else -> AdaptiveWorkstationPosture.COMPACT_CONVERTIBLE
        }

        Column(modifier = Modifier.fillMaxSize()) {
            // 1. Top Workstation Header & Window Actions Bar
            WorkstationHeaderBar(
                uiState = uiState,
                posture = posture,
                onNewWindowClick = { launchNewDocumentWindow(context, template = "blank") },
                onOpenClick = onOpenDocumentRequested,
                onSaveClick = onSaveDocumentRequested,
                onPrintClick = onPrintRequested,
                onUndoClick = viewModel::performUndo,
                onRedoClick = viewModel::performRedo,
                onTogglePlan2Ribbon = {
                    viewModel.togglePlan2Ribbon()
                    if (navBackStack.lastOrNull() != WorkstationNavKey.Plan2Operations) {
                        navBackStack.add(WorkstationNavKey.Plan2Operations)
                    }
                },
                onToggleStencilDrawer = viewModel::toggleStencilDrawer,
                onToggleShapeSheet = viewModel::toggleShapeSheetDrawer,
                onToggleInspector = viewModel::toggleInspectorPane
            )

            // 2. Live Visio ShapeSheet Formula Bar (GUARD-aware + SETF override)
            ShapeSheetFormulaBar(
                uiState = uiState,
                formulaFocusRequester = formulaFocusRequester,
                onFormulaChanged = viewModel::updateFormulaBarInput,
                onForceSetfToggled = viewModel::setForceSetfOverride,
                onCommitFormula = { viewModel.commitCellFormula() }
            )

            // 2b. Material 3 Expressive Plan 2 Enterprise Control Strip
            AnimatedVisibility(
                visible = uiState.isPlan2RibbonExpanded,
                enter = expandVertically(VexioExpressiveMotion.spatialSpring()) + fadeIn(),
                exit = shrinkVertically(VexioExpressiveMotion.spatialSpring()) + fadeOut()
            ) {
                Plan2ExpressiveControlStrip(
                    uiState = uiState,
                    onBooleanOp = { op -> viewModel.performBooleanOp(op) },
                    onSelectTheme = { slug, variant -> viewModel.applyDocumentTheme(slug, variant) },
                    onToggleVisualPreset = { preset -> viewModel.applyVisualPreset(preset.slug) },
                    onCreateSubprocess = { viewModel.createOrLinkSubprocess() },
                    onCompareDiagram = { viewModel.compareWithBaseline() },
                    onImportDataVisualizer = {
                        viewModel.importDataVisualizer(
                            Plan2SamplePayloads.dataVisualizerCsv.toByteArray(Charsets.UTF_8)
                        )
                    },
                    onImportCloudDiscovery = {
                        viewModel.importCloudDiscovery(Plan2SamplePayloads.cloudDiscoveryJson)
                    },
                    onImportEntraOrgChart = {
                        viewModel.importEntraOrLdifOrgChart(Plan2SamplePayloads.entraOrgChartLdif)
                    },
                    onSyncCrdtPeerDelta = {
                        val nextLogical = uiState.crdtLogicalClock + 1
                        val crdtDeltaJson = """
                            [
                              {
                                "SetCell": {
                                  "ts": { "wall_ms": ${1_760_000_000_000L + nextLogical}, "logical": $nextLogical, "replica_id": "${uiState.crdtReplicaId}" },
                                  "page_id": ${uiState.activePageId},
                                  "shape_id": ${uiState.selectedShapeId},
                                  "cell_name": "PinX",
                                  "formula": "5.00 in",
                                  "force_setf": false
                                }
                              }
                            ]
                        """.trimIndent()
                        viewModel.applyCrdtBatchJson(crdtDeltaJson, uiState.crdtReplicaId)
                    },
                    onExportPlan2Format = { fmt -> viewModel.exportPlan2Artifact(fmt) }
                )
            }

            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)

            // 3. Multi-Pane Compose Adaptive / Navigation 3 Workstation Body
            Row(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .animateContentSize(VexioExpressiveMotion.sizeReflowSpring)
            ) {
                // Left Navigation Rail: Stencil Categories
                StencilCategoryNavigationRail(
                    selectedCategory = uiState.selectedStencilCategory,
                    isDrawerExpanded = uiState.isStencilDrawerExpanded,
                    onCategorySelected = { category ->
                        viewModel.selectStencilCategory(category)
                        if (!uiState.isStencilDrawerExpanded) {
                            viewModel.toggleStencilDrawer()
                        }
                        navBackStack.add(WorkstationNavKey.StencilDrawer(category))
                    }
                )

                // Supporting Pane A: Stencil Master Drawer (Cross-Window Drag Source)
                val showStencilDrawer = uiState.isStencilDrawerExpanded &&
                    posture != AdaptiveWorkstationPosture.COMPACT_CONVERTIBLE
                if (showStencilDrawer) {
                    StencilMasterDrawerPane(
                        windowInstanceId = uiState.windowInstanceId,
                        selectedCategory = uiState.selectedStencilCategory,
                        onMasterClicked = { master ->
                            viewModel.instantiateMasterDrop(
                                title = master.title,
                                widthFormula = master.defaultWidthFormula,
                                heightFormula = master.defaultHeightFormula,
                                fillFormula = master.fillFormula,
                                pinXInches = 4.25f,
                                pinYInches = 5.50f
                            )
                        }
                    )
                    VerticalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                }

                // Primary Center Pane: Vulkan SurfaceView + Expressive Morphing Toolbar + ShapeSheet Grid
                Column(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxHeight()
                ) {
                    // Docked Expressive Toolbar mode when user toggles floating pill -> docked squircle bar
                    AnimatedVisibility(
                        visible = !uiState.isToolbarFloating,
                        enter = expandVertically(VexioExpressiveMotion.spatialSpring()) + fadeIn(),
                        exit = shrinkVertically(VexioExpressiveMotion.spatialSpring()) + fadeOut()
                    ) {
                        ExpressiveMorphingToolbar(
                            uiState = uiState,
                            isFloating = false,
                            onSelectTool = viewModel::selectTool,
                            onZoomDelta = viewModel::adjustZoom,
                            onToggleFloating = viewModel::toggleToolbarDockMode,
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 12.dp, vertical = 6.dp)
                        )
                    }

                    // Vulkan Canvas + Cross-Window Drag & Drop Target + Floating Expressive Pill Toolbar
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .fillMaxWidth()
                            .background(semanticColors.canvasBackdrop)
                    ) {
                        VulkanCanvasDropZone(
                            uiState = uiState,
                            viewModel = viewModel,
                            onExternalUriDropped = onExternalUriDropped,
                            onOpenNewWindowRequested = {
                                launchNewDocumentWindow(context, template = "blank")
                            },
                            modifier = Modifier.fillMaxSize()
                        )

                        // Floating Material 3 Expressive Morphing Pill Toolbar
                        if (uiState.isToolbarFloating) {
                            ExpressiveMorphingToolbar(
                                uiState = uiState,
                                isFloating = true,
                                onSelectTool = viewModel::selectTool,
                                onZoomDelta = viewModel::adjustZoom,
                                onToggleFloating = viewModel::toggleToolbarDockMode,
                                modifier = Modifier
                                    .align(Alignment.TopCenter)
                                    .padding(top = 14.dp)
                            )
                        }

                        // USI / Active Stylus Live Telemetry Pill Badge
                        AnimatedVisibility(
                            visible = uiState.isStylusActive,
                            enter = fadeIn(),
                            exit = fadeOut(),
                            modifier = Modifier
                                .align(Alignment.BottomEnd)
                                .padding(14.dp)
                        ) {
                            Surface(
                                shape = CircleShape,
                                color = semanticColors.stylusInkContainer,
                                border = BorderStroke(1.dp, semanticColors.stylusInkAccent),
                                shadowElevation = 4.dp
                            ) {
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp)
                                ) {
                                    Icon(
                                        imageVector = Icons.Default.Brush,
                                        contentDescription = null,
                                        tint = semanticColors.stylusInkAccent,
                                        modifier = Modifier.size(16.dp)
                                    )
                                    Spacer(Modifier.width(6.dp))
                                    Text(
                                        text = "USI Stylus • Pressure ${(uiState.lastStylusPressure * 100).toInt()}% • Tilt ${uiState.lastStylusTiltDeg}° • Palm Guard ON",
                                        style = MaterialTheme.typography.labelSmall,
                                        color = semanticColors.stylusInkAccent
                                    )
                                }
                            }
                        }
                    }

                    // Bottom Collapsible ShapeSheet Live Cell Table (Shift+F5)
                    AnimatedVisibility(
                        visible = uiState.isShapeSheetDrawerExpanded,
                        enter = expandVertically(VexioExpressiveMotion.spatialSpring()) + fadeIn(),
                        exit = shrinkVertically(VexioExpressiveMotion.spatialSpring()) + fadeOut()
                    ) {
                        ShapeSheetSectionTablePane(
                            uiState = uiState,
                            onCellSelected = { cellName ->
                                viewModel.selectShapeSheetCell(cellName)
                                formulaFocusRequester.requestFocus()
                            }
                        )
                    }
                }

                // Supporting Pane B: Inspector & BPMN 2.0 / UML 2.5 Validation Pane
                val showInspectorPane = uiState.isInspectorExpanded &&
                    posture == AdaptiveWorkstationPosture.ULTRAWIDE_DESKTOP
                if (showInspectorPane) {
                    VerticalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                    ShapeInspectorPane(
                        uiState = uiState,
                        onSelectCell = { cellName ->
                            viewModel.selectShapeSheetCell(cellName)
                            formulaFocusRequester.requestFocus()
                        },
                        onQuickGuardToggle = { cell ->
                            val toggledFormula = if (cell.isGuarded) {
                                cell.formula.removePrefix("GUARD(").removePrefix("guard(").removeSuffix(")")
                            } else {
                                "GUARD(${cell.formula})"
                            }
                            viewModel.commitCellFormula(
                                cellName = cell.cellName,
                                formula = toggledFormula,
                                forceSetf = true
                            )
                        }
                    )
                }
            }

            // 4. Bottom Page Switcher & Workstation Diagnostics Bar
            WorkstationBottomStatusBar(
                uiState = uiState,
                posture = posture,
                onSelectPage = viewModel::selectPage,
                onAddPage = viewModel::addNewPage,
                onZoomReset = { viewModel.setZoomPercent(100) }
            )
        }
    }
}

@Composable
private fun WorkstationHeaderBar(
    uiState: WorkstationUiState,
    posture: AdaptiveWorkstationPosture,
    onNewWindowClick: () -> Unit,
    onOpenClick: () -> Unit,
    onSaveClick: () -> Unit,
    onPrintClick: () -> Unit,
    onUndoClick: () -> Unit,
    onRedoClick: () -> Unit,
    onTogglePlan2Ribbon: () -> Unit,
    onToggleStencilDrawer: () -> Unit,
    onToggleShapeSheet: () -> Unit,
    onToggleInspector: () -> Unit
) {
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        tonalElevation = 2.dp,
        modifier = Modifier.fillMaxWidth()
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 6.dp)
        ) {
            // App Badge + Document Title + Dirty Indicator
            Surface(
                shape = RoundedCornerShape(10.dp),
                color = MaterialTheme.colorScheme.primaryContainer
            ) {
                Text(
                    text = "VEXIO",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                    modifier = Modifier.padding(horizontal = 10.dp, vertical = 5.dp)
                )
            }
            Spacer(Modifier.width(10.dp))
            Column(modifier = Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = buildString {
                            append(uiState.documentTitle)
                            if (uiState.isDirty) append(" • Edited")
                        },
                        style = MaterialTheme.typography.titleMedium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    Spacer(Modifier.width(8.dp))
                    Badge(
                        containerColor = MaterialTheme.colorScheme.secondaryContainer,
                        contentColor = MaterialTheme.colorScheme.onSecondaryContainer
                    ) {
                        Text(
                            text = uiState.documentExtension.uppercase(),
                            modifier = Modifier.padding(horizontal = 4.dp, vertical = 1.dp)
                        )
                    }
                }
                if (posture != AdaptiveWorkstationPosture.COMPACT_CONVERTIBLE) {
                    Text(
                        text = uiState.statusBanner,
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
            }

            // Undo / Redo
            IconButton(onClick = onUndoClick, enabled = uiState.canUndo) {
                Icon(Icons.AutoMirrored.Filled.Undo, contentDescription = "Undo (Ctrl+Z)")
            }
            IconButton(onClick = onRedoClick, enabled = uiState.canRedo) {
                Icon(Icons.AutoMirrored.Filled.Redo, contentDescription = "Redo (Ctrl+Y)")
            }

            // SAF Open / Save / Native OS Print
            IconButton(onClick = onOpenClick) {
                Icon(Icons.Default.FolderOpen, contentDescription = "Open Diagram (Ctrl+O)")
            }
            IconButton(onClick = onSaveClick) {
                Icon(Icons.Default.Save, contentDescription = "Save Diagram (Ctrl+S)")
            }
            IconButton(onClick = onPrintClick) {
                Icon(Icons.Default.Print, contentDescription = "Print Vector PDF (Ctrl+P)")
            }

            Spacer(Modifier.width(4.dp))

            // Multi-Instance Side-by-Side Window Launcher Button
            FilledTonalButton(
                onClick = onNewWindowClick,
                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 6.dp),
                shape = RoundedCornerShape(14.dp)
            ) {
                Icon(
                    imageVector = Icons.Default.OpenInNew,
                    contentDescription = null,
                    modifier = Modifier.size(16.dp)
                )
                Spacer(Modifier.width(6.dp))
                Text(
                    text = if (posture == AdaptiveWorkstationPosture.COMPACT_CONVERTIBLE) {
                        "Window"
                    } else {
                        "New Window"
                    },
                    style = MaterialTheme.typography.labelLarge
                )
            }

            Spacer(Modifier.width(6.dp))

            // Pane Toggles (Plan 2 Ribbon, Stencils, ShapeSheet Shift+F5, Inspector)
            FilledTonalIconButton(onClick = onTogglePlan2Ribbon) {
                Icon(Icons.Default.Architecture, contentDescription = "Toggle Plan 2 Operations Ribbon")
            }
            FilledTonalIconButton(onClick = onToggleStencilDrawer) {
                Icon(Icons.Default.Schema, contentDescription = "Toggle Stencil Drawer")
            }
            FilledTonalIconButton(onClick = onToggleShapeSheet) {
                Icon(Icons.Default.TableChart, contentDescription = "Toggle ShapeSheet (Shift+F5)")
            }
            if (posture == AdaptiveWorkstationPosture.ULTRAWIDE_DESKTOP) {
                FilledTonalIconButton(onClick = onToggleInspector) {
                    Icon(Icons.Default.ViewSidebar, contentDescription = "Toggle Inspector Pane")
                }
            }
        }
    }
}

/**
 * Material 3 Expressive Plan 2 Enterprise Control Strip surfacing:
 * - Constructive 2D Boolean Operations pill group (`Union`, `Combine`, `Fragment`, `Intersect`, `Subtract`, `Trim`, `Join`, `Offset`)
 * - `6×4` Themes & Variants selector + Visual Effects presets (`grad_linear`, `shadow_soft`, `bevel_3d`, `sketchy`)
 * - Subprocess Drill-Down & Diagram Compare actions
 * - Data Visualizer, Cloud Discovery, Entra/LDIF Org Chart, HLC CRDT sync, and `.pptx`/`.docx`/Power Automate/Power BI exports
 */
@Composable
private fun Plan2ExpressiveControlStrip(
    uiState: WorkstationUiState,
    onBooleanOp: (VexioBooleanOp) -> Unit,
    onSelectTheme: (String, Int) -> Unit,
    onToggleVisualPreset: (VexioVisualPreset) -> Unit,
    onCreateSubprocess: () -> Unit,
    onCompareDiagram: () -> Unit,
    onImportDataVisualizer: () -> Unit,
    onImportCloudDiscovery: () -> Unit,
    onImportEntraOrgChart: () -> Unit,
    onSyncCrdtPeerDelta: () -> Unit,
    onExportPlan2Format: (String) -> Unit
) {
    var themeMenuExpanded by remember { mutableStateOf(false) }
    var exportMenuExpanded by remember { mutableStateOf(false) }
    val activeThemeDisplay = remember(uiState.activeThemeSlug) {
        VexioThemePreset.fromSlug(uiState.activeThemeSlug)?.displayName ?: "Modern"
    }

    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        modifier = Modifier.fillMaxWidth()
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState())
                .padding(horizontal = 12.dp, vertical = 6.dp)
        ) {
            // 1. Boolean Operations Pill Group
            Surface(
                shape = RoundedCornerShape(14.dp),
                color = MaterialTheme.colorScheme.surfaceContainer,
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(3.dp),
                    modifier = Modifier.padding(horizontal = 6.dp, vertical = 3.dp)
                ) {
                    Text(
                        text = "Boolean",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(horizontal = 4.dp)
                    )
                    VexioBooleanOp.entries.forEach { op ->
                        val active = uiState.lastBooleanOp == op.slug
                        ExpressivePlan2ActionPill(
                            label = op.displayName,
                            selected = active,
                            onClick = { onBooleanOp(op) }
                        )
                    }
                }
            }

            // 2. 6×4 Theme & Effects Selector Group
            Surface(
                shape = RoundedCornerShape(14.dp),
                color = MaterialTheme.colorScheme.surfaceContainer,
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                    modifier = Modifier.padding(horizontal = 6.dp, vertical = 3.dp)
                ) {
                    Box {
                        ExpressivePlan2ActionPill(
                            label = "Theme: $activeThemeDisplay",
                            selected = true,
                            onClick = { themeMenuExpanded = true }
                        )
                        DropdownMenu(
                            expanded = themeMenuExpanded,
                            onDismissRequest = { themeMenuExpanded = false }
                        ) {
                            VexioThemePreset.entries.forEach { preset ->
                                DropdownMenuItem(
                                    text = { Text("${preset.displayName} (4 Variants)") },
                                    leadingIcon = {
                                        if (preset.slug == uiState.activeThemeSlug) {
                                            Icon(Icons.Default.Check, contentDescription = null)
                                        }
                                    },
                                    onClick = {
                                        themeMenuExpanded = false
                                        onSelectTheme(preset.slug, uiState.activeThemeVariant)
                                    }
                                )
                            }
                        }
                    }

                    (0..3).forEach { variantIdx ->
                        ExpressivePlan2ActionPill(
                            label = "V${variantIdx + 1}",
                            selected = uiState.activeThemeVariant == variantIdx,
                            onClick = { onSelectTheme(uiState.activeThemeSlug, variantIdx) }
                        )
                    }

                    VerticalDivider(
                        modifier = Modifier
                            .height(18.dp)
                            .padding(horizontal = 2.dp),
                        color = MaterialTheme.colorScheme.outlineVariant
                    )

                    VexioVisualPreset.entries.forEach { preset ->
                        ExpressivePlan2ActionPill(
                            label = preset.displayName,
                            selected = preset.slug in uiState.activeVisualPresets,
                            onClick = { onToggleVisualPreset(preset) }
                        )
                    }
                }
            }

            // 3. Subprocess Drill-Down & Diagram Compare Actions
            Surface(
                shape = RoundedCornerShape(14.dp),
                color = MaterialTheme.colorScheme.surfaceContainer,
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                    modifier = Modifier.padding(horizontal = 6.dp, vertical = 3.dp)
                ) {
                    val subLabel = if (uiState.subprocessLinks.isNotEmpty()) {
                        "Subprocess (+) [${uiState.subprocessLinks.size}]"
                    } else {
                        "Subprocess (+)"
                    }
                    ExpressivePlan2ActionPill(
                        label = subLabel,
                        selected = uiState.subprocessLinks.containsKey(uiState.selectedShapeId),
                        onClick = onCreateSubprocess
                    )

                    val compareSummary = uiState.lastCompareSummary
                    val compareLabel = if (compareSummary != null) {
                        "Compare +${compareSummary.addedCount}/~${compareSummary.modifiedCount}/-${compareSummary.removedCount}"
                    } else {
                        "Diagram Compare"
                    }
                    ExpressivePlan2ActionPill(
                        label = compareLabel,
                        selected = compareSummary != null,
                        onClick = onCompareDiagram
                    )
                }
            }

            // 4. Data Visualizer, Cloud Discovery, Entra/LDIF Org, CRDT Sync & Plan 2 Exports
            Surface(
                shape = RoundedCornerShape(14.dp),
                color = MaterialTheme.colorScheme.surfaceContainer,
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                    modifier = Modifier.padding(horizontal = 6.dp, vertical = 3.dp)
                ) {
                    ExpressivePlan2ActionPill(
                        label = "Data Visualizer",
                        selected = uiState.lastDataVisualizerShapeCount > 0,
                        onClick = onImportDataVisualizer
                    )
                    ExpressivePlan2ActionPill(
                        label = "Cloud Discovery",
                        selected = uiState.lastCloudDiscoveryShapeCount > 0,
                        onClick = onImportCloudDiscovery
                    )
                    ExpressivePlan2ActionPill(
                        label = "Entra / LDIF Org",
                        selected = uiState.lastOrgChartShapeCount > 0,
                        onClick = onImportEntraOrgChart
                    )
                    ExpressivePlan2ActionPill(
                        label = "CRDT Sync (L${uiState.crdtLogicalClock})",
                        selected = uiState.crdtAppliedOpsCount > 0,
                        onClick = onSyncCrdtPeerDelta
                    )

                    Box {
                        ExpressivePlan2ActionPill(
                            label = uiState.lastPlan2ExportFormat?.let { "Exported .$it" } ?: "Plan 2 Export ▾",
                            selected = uiState.lastPlan2ExportFormat != null,
                            onClick = { exportMenuExpanded = true }
                        )
                        DropdownMenu(
                            expanded = exportMenuExpanded,
                            onDismissRequest = { exportMenuExpanded = false }
                        ) {
                            DropdownMenuItem(
                                text = { Text("PowerPoint Slide Deck (.pptx)") },
                                onClick = {
                                    exportMenuExpanded = false
                                    onExportPlan2Format("pptx")
                                }
                            )
                            DropdownMenuItem(
                                text = { Text("Word Process Spec (.docx)") },
                                onClick = {
                                    exportMenuExpanded = false
                                    onExportPlan2Format("docx")
                                }
                            )
                            DropdownMenuItem(
                                text = { Text("Power Automate Workflow (flow.json)") },
                                onClick = {
                                    exportMenuExpanded = false
                                    onExportPlan2Format("flow.json")
                                }
                            )
                            DropdownMenuItem(
                                text = { Text("Power BI Visual Mapping (pbi.json)") },
                                onClick = {
                                    exportMenuExpanded = false
                                    onExportPlan2Format("pbi.json")
                                }
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ExpressivePlan2ActionPill(
    label: String,
    selected: Boolean,
    onClick: () -> Unit
) {
    val pillShape = rememberExpressiveToolButtonShape(isSelected = selected)
    val scale by rememberExpressiveScaleState(emphasized = selected)
    val bgColor by animateColorAsState(
        targetValue = if (selected) {
            MaterialTheme.colorScheme.secondaryContainer
        } else {
            MaterialTheme.colorScheme.surfaceContainerLowest
        },
        animationSpec = VexioExpressiveMotion.crispEffectsSpring(),
        label = "Plan2PillBg"
    )
    val fgColor by animateColorAsState(
        targetValue = if (selected) {
            MaterialTheme.colorScheme.onSecondaryContainer
        } else {
            MaterialTheme.colorScheme.onSurfaceVariant
        },
        animationSpec = VexioExpressiveMotion.crispEffectsSpring(),
        label = "Plan2PillFg"
    )

    Surface(
        shape = pillShape,
        color = bgColor,
        border = BorderStroke(
            width = 1.dp,
            color = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant
        ),
        modifier = Modifier
            .scale(scale)
            .clip(pillShape)
            .clickable(onClick = onClick)
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium,
            color = fgColor,
            modifier = Modifier.padding(horizontal = 9.dp, vertical = 5.dp)
        )
    }
}

@Composable
private fun ShapeSheetFormulaBar(
    uiState: WorkstationUiState,
    formulaFocusRequester: FocusRequester,
    onFormulaChanged: (String) -> Unit,
    onForceSetfToggled: (Boolean) -> Unit,
    onCommitFormula: () -> Unit
) {
    val semanticColors = LocalVexioSemanticColors.current
    val activeCell = uiState.shapeSheetCells.firstOrNull { it.cellName == uiState.activeCellName }
    val isGuarded = activeCell?.isGuarded == true
    val hasError = uiState.lastFormulaStatus != FormulaCommitStatus.OK

    Surface(
        color = MaterialTheme.colorScheme.surfaceContainer,
        modifier = Modifier.fillMaxWidth()
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 5.dp)
        ) {
            // Shape!Cell Reference Pill
            Surface(
                shape = RoundedCornerShape(8.dp),
                color = MaterialTheme.colorScheme.surfaceContainerHighest
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp)
                ) {
                    Icon(
                        imageVector = Icons.Default.Functions,
                        contentDescription = null,
                        modifier = Modifier.size(15.dp),
                        tint = semanticColors.formulaSyntaxKeyword
                    )
                    Spacer(Modifier.width(6.dp))
                    Text(
                        text = "Sheet.${uiState.selectedShapeId}!${uiState.activeCellName}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurface
                    )
                }
            }

            if (isGuarded) {
                Spacer(Modifier.width(8.dp))
                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = semanticColors.guardBadgeContainer
                ) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp)
                    ) {
                        Icon(
                            imageVector = Icons.Default.Lock,
                            contentDescription = null,
                            modifier = Modifier.size(13.dp),
                            tint = semanticColors.guardBadgeForeground
                        )
                        Spacer(Modifier.width(4.dp))
                        Text(
                            text = "GUARD",
                            style = MaterialTheme.typography.labelSmall,
                            color = semanticColors.guardBadgeForeground
                        )
                    }
                }
            }

            Spacer(Modifier.width(10.dp))
            Text(
                text = "=",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.primary
            )
            Spacer(Modifier.width(8.dp))

            // Monospace Formula Input Field
            Box(
                contentAlignment = Alignment.CenterStart,
                modifier = Modifier
                    .weight(1f)
                    .clip(RoundedCornerShape(8.dp))
                    .background(MaterialTheme.colorScheme.surfaceContainerLowest)
                    .border(
                        width = 1.dp,
                        color = if (hasError) {
                            MaterialTheme.colorScheme.error
                        } else {
                            MaterialTheme.colorScheme.outlineVariant
                        },
                        shape = RoundedCornerShape(8.dp)
                    )
                    .padding(horizontal = 10.dp, vertical = 6.dp)
            ) {
                BasicTextField(
                    value = uiState.formulaBarInput,
                    onValueChange = onFormulaChanged,
                    singleLine = true,
                    textStyle = MaterialTheme.typography.bodySmall.copy(
                        color = MaterialTheme.colorScheme.onSurface,
                        fontFamily = FontFamily.Monospace
                    ),
                    cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                    modifier = Modifier
                        .fillMaxWidth()
                        .focusRequester(formulaFocusRequester)
                        .onPreviewKeyEvent { ev ->
                            if (ev.type == KeyEventType.KeyDown && ev.key == Key.Enter) {
                                onCommitFormula()
                                true
                            } else {
                                false
                            }
                        }
                )
            }

            Spacer(Modifier.width(8.dp))

            // SETF(GetRef(...)) Override Checkbox for GUARD() cells
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .clip(RoundedCornerShape(8.dp))
                    .clickable { onForceSetfToggled(!uiState.forceSetfOverride) }
                    .padding(end = 6.dp)
            ) {
                Checkbox(
                    checked = uiState.forceSetfOverride,
                    onCheckedChange = onForceSetfToggled,
                    modifier = Modifier.size(28.dp)
                )
                Text(
                    text = "SETF",
                    style = MaterialTheme.typography.labelSmall,
                    color = if (uiState.forceSetfOverride) {
                        semanticColors.setfOverrideAccent
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    }
                )
            }

            Spacer(Modifier.width(6.dp))

            FilledTonalButton(
                onClick = onCommitFormula,
                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp),
                shape = RoundedCornerShape(10.dp),
                colors = ButtonDefaults.filledTonalButtonColors(
                    containerColor = MaterialTheme.colorScheme.primaryContainer
                )
            ) {
                Icon(
                    imageVector = Icons.Default.Check,
                    contentDescription = "Apply Formula",
                    modifier = Modifier.size(16.dp)
                )
                Spacer(Modifier.width(4.dp))
                Text("Apply", style = MaterialTheme.typography.labelMedium)
            }
        }
    }
}

@Composable
private fun StencilCategoryNavigationRail(
    selectedCategory: String,
    isDrawerExpanded: Boolean,
    onCategorySelected: (String) -> Unit
) {
    NavigationRail(
        containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
        modifier = Modifier.fillMaxHeight()
    ) {
        Spacer(Modifier.height(8.dp))
        BuiltInStencilCatalog.categories.forEach { category ->
            val icon = when (category) {
                "BPMN 2.0" -> Icons.Default.AccountTree
                "Flowchart" -> Icons.Default.Schema
                "UML 2.5" -> Icons.Default.Timeline
                else -> Icons.Default.Architecture
            }
            val selected = isDrawerExpanded && selectedCategory == category
            NavigationRailItem(
                selected = selected,
                onClick = { onCategorySelected(category) },
                icon = { Icon(icon, contentDescription = category) },
                label = {
                    Text(
                        text = category.substringBefore(' '),
                        style = MaterialTheme.typography.labelSmall
                    )
                }
            )
        }
    }
}

@Composable
private fun StencilMasterDrawerPane(
    windowInstanceId: String,
    selectedCategory: String,
    onMasterClicked: (StencilMasterItem) -> Unit
) {
    val categoryMasters = remember(selectedCategory) {
        BuiltInStencilCatalog.masters.filter { it.category == selectedCategory }
    }

    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        modifier = Modifier
            .width(236.dp)
            .fillMaxHeight()
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 14.dp, vertical = 10.dp)
            ) {
                Text(
                    text = "$selectedCategory Stencils",
                    style = MaterialTheme.typography.titleSmall,
                    modifier = Modifier.weight(1f)
                )
                Text(
                    text = "Drag ⇄",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.primary
                )
            }
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)

            LazyColumn(
                contentPadding = PaddingValues(10.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.fillMaxSize()
            ) {
                items(categoryMasters, key = { it.masterId }) { master ->
                    DraggableStencilMasterCard(
                        windowInstanceId = windowInstanceId,
                        master = master,
                        onClick = { onMasterClicked(master) }
                    )
                }
            }
        }
    }
}

/**
 * Stencil Master card acting as a cross-window Drag & Drop source (`DRAG_FLAG_GLOBAL`) so users
 * can drag masters directly across side-by-side Vexio windows on Googlebook OS.
 */
@Composable
private fun DraggableStencilMasterCard(
    windowInstanceId: String,
    master: StencilMasterItem,
    onClick: () -> Unit
) {
    val jsonPayload = remember(windowInstanceId, master) {
        Json.encodeToString(
            CrossWindowShapePayload(
                sourceWindowId = windowInstanceId,
                masterId = master.masterId,
                title = master.title,
                widthFormula = master.defaultWidthFormula,
                heightFormula = master.defaultHeightFormula,
                fillFormula = master.fillFormula
            )
        )
    }

    Surface(
        shape = RoundedCornerShape(14.dp),
        color = MaterialTheme.colorScheme.surfaceContainerLowest,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
        modifier = Modifier
            .fillMaxWidth()
            .dragAndDropSource {
                detectTapGestures(
                    onTap = { onClick() },
                    onLongPress = {
                        val item = ClipData.Item(jsonPayload)
                        val clipData = ClipData(
                            master.title,
                            arrayOf(VEXIO_MASTER_CLIP_MIME, ClipDescription.MIMETYPE_TEXT_PLAIN),
                            item
                        )
                        startTransfer(
                            DragAndDropTransferData(
                                clipData = clipData,
                                flags = View.DRAG_FLAG_GLOBAL or View.DRAG_FLAG_GLOBAL_URI_READ
                            )
                        )
                    }
                )
            }
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 10.dp)
        ) {
            Surface(
                shape = RoundedCornerShape(10.dp),
                color = MaterialTheme.colorScheme.primaryContainer,
                modifier = Modifier.size(36.dp)
            ) {
                Box(contentAlignment = Alignment.Center) {
                    Icon(
                        imageVector = when (master.shapeKind) {
                            "Circle" -> Icons.Default.RadioButtonUnchecked
                            "Diamond" -> Icons.Default.AccountTree
                            else -> Icons.Default.CropSquare
                        },
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onPrimaryContainer,
                        modifier = Modifier.size(18.dp)
                    )
                }
            }
            Spacer(Modifier.width(10.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = master.title,
                    style = MaterialTheme.typography.labelLarge,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
                Text(
                    text = "${master.defaultWidthFormula} × ${master.defaultHeightFormula}",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

/**
 * Material 3 Expressive Toolbar that morphs smoothly between a floating capsule pill (`28.dp`)
 * and a docked desktop squircle bar (`16.dp`), with individual tool buttons morphing between
 * circular pills and active expressive squircles via `spring(dampingRatio = 0.68f, stiffness = 380f)`.
 */
@Composable
private fun ExpressiveMorphingToolbar(
    uiState: WorkstationUiState,
    isFloating: Boolean,
    onSelectTool: (VexioTool) -> Unit,
    onZoomDelta: (Int) -> Unit,
    onToggleFloating: () -> Unit,
    modifier: Modifier = Modifier
) {
    val toolbarShape = rememberMorphingToolbarShape(isFloatingPill = isFloating)
    val tools = remember {
        listOf(
            VexioTool.POINTER to Icons.Default.NearMe,
            VexioTool.RECTANGLE to Icons.Default.CropSquare,
            VexioTool.ELLIPSE to Icons.Default.RadioButtonUnchecked,
            VexioTool.FREEFORM to Icons.Default.Architecture,
            VexioTool.CONNECTOR to Icons.Default.Timeline,
            VexioTool.TEXT_BLOCK to Icons.Default.TextFields,
            VexioTool.STYLUS_INK to Icons.Default.Brush,
            VexioTool.PAN_HAND to Icons.Default.PanTool
        )
    }

    Surface(
        shape = toolbarShape,
        color = MaterialTheme.colorScheme.surfaceContainerHighest.copy(
            alpha = if (isFloating) 0.96f else 1.0f
        ),
        shadowElevation = if (isFloating) 8.dp else 1.dp,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
        modifier = modifier
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
            modifier = Modifier
                .horizontalScroll(rememberScrollState())
                .padding(horizontal = 10.dp, vertical = 6.dp)
        ) {
            tools.forEach { (tool, icon) ->
                val isSelected = uiState.activeTool == tool
                val itemShape = rememberExpressiveToolButtonShape(isSelected = isSelected)
                val scale by rememberExpressiveScaleState(emphasized = isSelected)
                val bgColor by animateColorAsState(
                    targetValue = if (isSelected) {
                        MaterialTheme.colorScheme.primary
                    } else {
                        Color.Transparent
                    },
                    animationSpec = VexioExpressiveMotion.crispEffectsSpring(),
                    label = "ToolBgColor"
                )
                val contentColor by animateColorAsState(
                    targetValue = if (isSelected) {
                        MaterialTheme.colorScheme.onPrimary
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
                    animationSpec = VexioExpressiveMotion.crispEffectsSpring(),
                    label = "ToolFgColor"
                )

                Surface(
                    shape = itemShape,
                    color = bgColor,
                    modifier = Modifier
                        .scale(scale)
                        .clip(itemShape)
                        .clickable { onSelectTool(tool) }
                ) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.padding(
                            horizontal = if (isSelected) 12.dp else 9.dp,
                            vertical = 7.dp
                        )
                    ) {
                        Icon(
                            imageVector = icon,
                            contentDescription = "${tool.displayName} (${tool.shortcutHint})",
                            tint = contentColor,
                            modifier = Modifier.size(18.dp)
                        )
                        if (isSelected) {
                            Spacer(Modifier.width(6.dp))
                            Text(
                                text = tool.displayName,
                                style = MaterialTheme.typography.labelMedium,
                                color = contentColor
                            )
                        }
                    }
                }
            }

            VerticalDivider(
                modifier = Modifier
                    .height(24.dp)
                    .padding(horizontal = 4.dp),
                color = MaterialTheme.colorScheme.outlineVariant
            )

            IconButton(
                onClick = { onZoomDelta(-15) },
                modifier = Modifier.size(32.dp)
            ) {
                Icon(
                    imageVector = Icons.Default.ZoomOut,
                    contentDescription = "Zoom Out",
                    modifier = Modifier.size(18.dp)
                )
            }
            Text(
                text = "${uiState.zoomPercent}%",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurface
            )
            IconButton(
                onClick = { onZoomDelta(15) },
                modifier = Modifier.size(32.dp)
            ) {
                Icon(
                    imageVector = Icons.Default.ZoomIn,
                    contentDescription = "Zoom In",
                    modifier = Modifier.size(18.dp)
                )
            }

            IconButton(
                onClick = onToggleFloating,
                modifier = Modifier.size(32.dp)
            ) {
                Icon(
                    imageVector = Icons.Default.Tune,
                    contentDescription = if (isFloating) "Dock Toolbar" else "Float Pill Toolbar",
                    modifier = Modifier.size(17.dp)
                )
            }
        }
    }
}

/**
 * Vulkan `SurfaceView` Canvas container equipped with:
 * - Zero-flicker `SurfaceHolder.Callback` (`nativeAttachVulkanSurface` / `nativeResizeViewport`)
 * - Hardware mouse cursor binding (`PointerIcon.getSystemIcon`)
 * - Unbuffered low-latency USI active stylus dispatch + `MotionEventPredictor` + Palm Rejection
 * - Cross-Window Drag & Drop target accepting Vexio Master JSON payloads & `.vsdx`/`.vsd`/`.dwg`/`.dxf` URIs
 * - Hardware mouse secondary-button (right-click) context menu
 */
@Composable
private fun VulkanCanvasDropZone(
    uiState: WorkstationUiState,
    viewModel: VexioSessionViewModel,
    onExternalUriDropped: (Uri) -> Unit,
    onOpenNewWindowRequested: () -> Unit,
    modifier: Modifier = Modifier
) {
    val context = LocalContext.current
    val density = LocalDensity.current.density
    val semanticColors = LocalVexioSemanticColors.current

    var isDragHovering by remember { mutableStateOf(false) }
    var contextMenuVisible by remember { mutableStateOf(false) }
    var contextMenuOffsetDp by remember { mutableStateOf(DpOffset.Zero) }

    val hardwarePointerIcon = remember(uiState.activeCursor) {
        resolveHardwarePointerIcon(context, uiState.activeCursor)
    }

    val dragAndDropTarget = remember(context, density) {
        object : DragAndDropTarget {
            override fun onEntered(event: DragAndDropEvent) {
                isDragHovering = true
            }

            override fun onExited(event: DragAndDropEvent) {
                isDragHovering = false
            }

            override fun onEnded(event: DragAndDropEvent) {
                isDragHovering = false
            }

            override fun onDrop(event: DragAndDropEvent): Boolean {
                isDragHovering = false
                val dragEvent: DragEvent = event.toAndroidDragEvent()
                val clipData = dragEvent.clipData ?: return false
                if (clipData.itemCount == 0) return false

                // Request cross-window URI read permission if dropped from Files app or another Vexio window
                val activity = context as? Activity
                val dragPermissions = activity?.requestDragAndDropPermissions(dragEvent)

                try {
                    val firstItem = clipData.getItemAt(0)

                    // Case 1: Dropped .vsdx / .vsdm / .vssx / .vstx / .vsd / .dwg / .dxf file URI
                    val droppedUri = firstItem.uri
                    if (droppedUri != null) {
                        val lastSeg = droppedUri.lastPathSegment?.lowercase().orEmpty()
                        val ext = lastSeg.substringAfterLast('.', "")
                        if (ext in SUPPORTED_DIAGRAM_EXTENSIONS ||
                            clipData.description?.hasMimeType("application/*") == true
                        ) {
                            onExternalUriDropped(droppedUri)
                            return true
                        }
                    }

                    // Case 2: Cross-window Vexio Stencil Master / Shape JSON payload
                    val rawText = firstItem.text?.toString()
                    if (!rawText.isNullOrBlank() && rawText.trimStart().startsWith("{")) {
                        val payload = runCatching {
                            Json.decodeFromString<CrossWindowShapePayload>(rawText)
                        }.getOrNull()
                        if (payload != null) {
                            // Convert physical drop coordinates into canonical Visio page inches (96 logical dpi)
                            val dpi = 96f * density * (viewModel.uiState.value.zoomPercent / 100f)
                            val pinXIn = (dragEvent.x / dpi.coerceAtLeast(1f)).coerceIn(0.5f, 10.5f)
                            val pinYIn = (8.5f - (dragEvent.y / dpi.coerceAtLeast(1f))).coerceIn(0.5f, 8.0f)
                            viewModel.instantiateMasterDrop(
                                title = payload.title,
                                widthFormula = payload.widthFormula,
                                heightFormula = payload.heightFormula,
                                fillFormula = payload.fillFormula,
                                pinXInches = pinXIn,
                                pinYInches = pinYIn
                            )
                            return true
                        }
                    }
                } finally {
                    // Note: if URI opening is asynchronous, MainActivity copies bytes immediately on drop
                    if (clipData.getItemAt(0).uri == null) {
                        dragPermissions?.release()
                    }
                }
                return false
            }
        }
    }

    Box(
        modifier = modifier
            .pointerHoverIcon(PointerIcon(hardwarePointerIcon))
            .dragAndDropTarget(
                shouldStartDragAndDrop = { startEvent ->
                    val mimes = startEvent.mimeTypes()
                    mimes.contains(VEXIO_MASTER_CLIP_MIME) ||
                        mimes.contains(ClipDescription.MIMETYPE_TEXT_PLAIN) ||
                        mimes.any {
                            it.startsWith("application/") ||
                                it.startsWith("image/") ||
                                it == "*/*"
                        }
                },
                target = dragAndDropTarget
            )
    ) {
        AndroidView(
            factory = { ctx ->
                SurfaceView(ctx).apply {
                    isFocusable = false
                    isFocusableInTouchMode = false
                    val predictor = MotionEventPredictor.newInstance(this)

                    holder.addCallback(object : SurfaceHolder.Callback {
                        override fun surfaceCreated(surfaceHolder: SurfaceHolder) {
                            val frame = surfaceHolder.surfaceFrame
                            viewModel.attachVulkanSurface(
                                surface = surfaceHolder.surface,
                                width = frame.width(),
                                height = frame.height(),
                                density = ctx.resources.displayMetrics.density
                            )
                        }

                        override fun surfaceChanged(
                            surfaceHolder: SurfaceHolder,
                            format: Int,
                            width: Int,
                            height: Int
                        ) {
                            // Live zero-flicker freeform Googlebook OS window resize
                            viewModel.resizeVulkanViewport(
                                width = width,
                                height = height,
                                density = ctx.resources.displayMetrics.density
                            )
                        }

                        override fun surfaceDestroyed(surfaceHolder: SurfaceHolder) {
                            viewModel.detachVulkanSurface()
                        }
                    })

                    setOnTouchListener { view, motionEvent ->
                        // Request unbuffered sub-frame dispatch for USI / active stylus strokes
                        if (motionEvent.actionMasked == MotionEvent.ACTION_DOWN) {
                            val tool = motionEvent.getToolType(0)
                            if (tool == MotionEvent.TOOL_TYPE_STYLUS ||
                                tool == MotionEvent.TOOL_TYPE_ERASER
                            ) {
                                view.requestUnbufferedDispatch(motionEvent)
                            }
                            if ((motionEvent.buttonState and MotionEvent.BUTTON_SECONDARY) != 0) {
                                val pxToDp = ctx.resources.displayMetrics.density
                                contextMenuOffsetDp = DpOffset(
                                    x = (motionEvent.x / pxToDp).dp,
                                    y = (motionEvent.y / pxToDp).dp
                                )
                                contextMenuVisible = true
                            }
                        }
                        val handled = viewModel.dispatchMotionEvent(motionEvent, predictor)
                        view.pointerIcon = resolveHardwarePointerIcon(
                            ctx,
                            viewModel.uiState.value.activeCursor
                        )
                        handled
                    }

                    setOnGenericMotionListener { view, motionEvent ->
                        val handled = viewModel.dispatchMotionEvent(motionEvent, predictor)
                        view.pointerIcon = resolveHardwarePointerIcon(
                            ctx,
                            viewModel.uiState.value.activeCursor
                        )
                        handled
                    }
                }
            },
            update = { surfaceView ->
                surfaceView.pointerIcon = hardwarePointerIcon
            },
            modifier = Modifier.fillMaxSize()
        )

        // Cross-window Drag & Drop active highlight overlay
        if (isDragHovering) {
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .fillMaxSize()
                    .padding(12.dp)
                    .border(
                        width = 2.5.dp,
                        color = semanticColors.dropZoneHighlightBorder,
                        shape = RoundedCornerShape(20.dp)
                    )
                    .background(
                        color = semanticColors.dropZoneHighlightFill,
                        shape = RoundedCornerShape(20.dp)
                    )
            ) {
                Surface(
                    shape = RoundedCornerShape(16.dp),
                    color = MaterialTheme.colorScheme.surfaceContainerHighest,
                    shadowElevation = 6.dp
                ) {
                    Text(
                        text = "Drop Stencil Master or .vsdx / .vsd / .dwg / .dxf Diagram Here",
                        style = MaterialTheme.typography.titleSmall,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(horizontal = 18.dp, vertical = 10.dp)
                    )
                }
            }
        }

        // Hardware Mouse Right-Click Context Menu
        DropdownMenu(
            expanded = contextMenuVisible,
            onDismissRequest = { contextMenuVisible = false },
            offset = contextMenuOffsetDp
        ) {
            DropdownMenuItem(
                text = { Text("Edit ShapeSheet (Shift+F5)") },
                leadingIcon = { Icon(Icons.Default.TableChart, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    if (!uiState.isShapeSheetDrawerExpanded) {
                        viewModel.toggleShapeSheetDrawer()
                    }
                }
            )
            DropdownMenuItem(
                text = { Text("Link Subprocess Page (+)") },
                leadingIcon = { Icon(Icons.Default.AccountTree, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.createOrLinkSubprocess()
                }
            )
            DropdownMenuItem(
                text = { Text("Boolean Union (Selected Shapes)") },
                leadingIcon = { Icon(Icons.Default.Architecture, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.performBooleanOp(VexioBooleanOp.UNION)
                }
            )
            DropdownMenuItem(
                text = { Text("Open New Diagram Window (Ctrl+N)") },
                leadingIcon = { Icon(Icons.Default.OpenInNew, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    onOpenNewWindowRequested()
                }
            )
            DropdownMenuItem(
                text = { Text("Delete Selected Shape (Del)") },
                leadingIcon = { Icon(Icons.Default.DeleteOutline, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.deleteSelectedShape()
                }
            )
        }
    }
}

@Composable
private fun ShapeSheetSectionTablePane(
    uiState: WorkstationUiState,
    onCellSelected: (String) -> Unit
) {
    val semanticColors = LocalVexioSemanticColors.current

    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        tonalElevation = 2.dp,
        modifier = Modifier
            .fillMaxWidth()
            .height(190.dp)
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .fillMaxWidth()
                    .background(MaterialTheme.colorScheme.surfaceContainer)
                    .padding(horizontal = 14.dp, vertical = 6.dp)
            ) {
                Icon(
                    imageVector = Icons.Default.GridOn,
                    contentDescription = null,
                    modifier = Modifier.size(16.dp),
                    tint = MaterialTheme.colorScheme.primary
                )
                Spacer(Modifier.width(8.dp))
                Text(
                    text = "ShapeSheet — ${uiState.selectedShapeName}",
                    style = MaterialTheme.typography.titleSmall,
                    modifier = Modifier.weight(1f)
                )
                Text(
                    text = "Canonical Units: in / deg • Reactive Topological DAG",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)

            LazyColumn(
                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 6.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
                modifier = Modifier.fillMaxSize()
            ) {
                items(uiState.shapeSheetCells, key = { it.cellName }) { entry ->
                    val isSelected = entry.cellName == uiState.activeCellName
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(8.dp))
                            .background(
                                if (isSelected) {
                                    MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.55f)
                                } else {
                                    Color.Transparent
                                }
                            )
                            .clickable { onCellSelected(entry.cellName) }
                            .padding(horizontal = 10.dp, vertical = 5.dp)
                    ) {
                        Text(
                            text = entry.section,
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.width(140.dp)
                        )
                        Text(
                            text = entry.cellName,
                            style = MaterialTheme.typography.bodySmall.copy(fontWeight = FontWeight.SemiBold),
                            color = semanticColors.formulaSyntaxKeyword,
                            modifier = Modifier.width(120.dp)
                        )
                        Text(
                            text = "= ${entry.formula}",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurface,
                            modifier = Modifier.weight(1f)
                        )
                        if (entry.isGuarded) {
                            Text(
                                text = "GUARDED",
                                style = MaterialTheme.typography.labelSmall,
                                color = semanticColors.guardBadgeForeground
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ShapeInspectorPane(
    uiState: WorkstationUiState,
    onSelectCell: (String) -> Unit,
    onQuickGuardToggle: (ShapeSheetCellEntry) -> Unit
) {
    val semanticColors = LocalVexioSemanticColors.current

    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        modifier = Modifier
            .width(276.dp)
            .fillMaxHeight()
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Text(
                text = "Shape & Layer Inspector",
                style = MaterialTheme.typography.titleSmall
            )
            Surface(
                shape = RoundedCornerShape(14.dp),
                color = MaterialTheme.colorScheme.surfaceContainerLowest,
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Text(
                        text = uiState.selectedShapeName,
                        style = MaterialTheme.typography.labelLarge
                    )
                    Spacer(Modifier.height(6.dp))
                    uiState.shapeSheetCells.take(6).forEach { cell ->
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(6.dp))
                                .clickable { onSelectCell(cell.cellName) }
                                .padding(vertical = 4.dp)
                        ) {
                            Text(
                                text = cell.cellName,
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.width(68.dp)
                            )
                            Text(
                                text = cell.formula,
                                style = MaterialTheme.typography.bodySmall,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                                modifier = Modifier.weight(1f)
                            )
                            IconButton(
                                onClick = { onQuickGuardToggle(cell) },
                                modifier = Modifier.size(24.dp)
                            ) {
                                Icon(
                                    imageVector = Icons.Default.Lock,
                                    contentDescription = "Toggle GUARD()",
                                    tint = if (cell.isGuarded) {
                                        semanticColors.guardBadgeForeground
                                    } else {
                                        MaterialTheme.colorScheme.outlineVariant
                                    },
                                    modifier = Modifier.size(14.dp)
                                )
                            }
                        }
                    }
                }
            }

            // Plan 2 Enterprise Telemetry Card (6×4 Theme, Visual Presets, Subprocesses, Compare & CRDT)
            Surface(
                shape = RoundedCornerShape(14.dp),
                color = MaterialTheme.colorScheme.surfaceContainer,
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                    modifier = Modifier.padding(12.dp)
                ) {
                    Text(
                        text = "Visio Plan 2 Telemetry",
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary
                    )
                    Text(
                        text = "Theme: ${uiState.activeThemeSlug} (Variant ${uiState.activeThemeVariant + 1}/4)",
                        style = MaterialTheme.typography.labelSmall
                    )
                    Text(
                        text = "Effects: ${uiState.activeVisualPresets.joinToString(", ").ifEmpty { "Standard" }}",
                        style = MaterialTheme.typography.labelSmall
                    )
                    Text(
                        text = "Subprocess Links: ${uiState.subprocessLinks.size} • CRDT Ops: ${uiState.crdtAppliedOpsCount} (L${uiState.crdtLogicalClock})",
                        style = MaterialTheme.typography.labelSmall
                    )
                    val compare = uiState.lastCompareSummary
                    if (compare != null) {
                        Text(
                            text = "Diff: +${compare.addedCount} added / ~${compare.modifiedCount} mod / -${compare.removedCount} del",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.secondary
                        )
                    }
                }
            }

            // BPMN 2.0 / UML 2.5 Structural Validation Diagnostic Card
            Surface(
                shape = RoundedCornerShape(14.dp),
                color = semanticColors.validationPassContainer,
                modifier = Modifier.fillMaxWidth()
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.padding(12.dp)
                ) {
                    Icon(
                        imageVector = Icons.Default.Verified,
                        contentDescription = null,
                        tint = semanticColors.validationPassForeground,
                        modifier = Modifier.size(20.dp)
                    )
                    Spacer(Modifier.width(8.dp))
                    Column {
                        Text(
                            text = "${uiState.validationSummary.standard} Validator",
                            style = MaterialTheme.typography.labelLarge,
                            color = semanticColors.validationPassForeground
                        )
                        Text(
                            text = uiState.validationSummary.statusMessage,
                            style = MaterialTheme.typography.labelMedium,
                            color = semanticColors.validationPassForeground
                        )
                    }
                }
            }

            // Keyboard Shortcut Reference Footer
            Surface(
                shape = RoundedCornerShape(14.dp),
                color = MaterialTheme.colorScheme.surfaceContainer,
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(
                    verticalArrangement = Arrangement.spacedBy(3.dp),
                    modifier = Modifier.padding(12.dp)
                ) {
                    Text(
                        text = "Googlebook Hardware Shortcuts",
                        style = MaterialTheme.typography.labelLarge
                    )
                    Text("Ctrl+N  New Side-by-Side Window", style = MaterialTheme.typography.labelSmall)
                    Text("Ctrl+1..5  Pointer / Text / Conn / Ink / Freeform", style = MaterialTheme.typography.labelSmall)
                    Text("Shift+F5  ShapeSheet Formula Grid", style = MaterialTheme.typography.labelSmall)
                    Text("Ctrl+P  Vector PDF OS Print", style = MaterialTheme.typography.labelSmall)
                }
            }
        }
    }
}

@Composable
private fun WorkstationBottomStatusBar(
    uiState: WorkstationUiState,
    posture: AdaptiveWorkstationPosture,
    onSelectPage: (Int) -> Unit,
    onAddPage: () -> Unit,
    onZoomReset: () -> Unit
) {
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = Modifier.fillMaxWidth()
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 10.dp, vertical = 4.dp)
        ) {
            // Page Tabs
            Row(
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                modifier = Modifier
                    .weight(1f)
                    .horizontalScroll(rememberScrollState())
            ) {
                uiState.pages.forEach { tab ->
                    val active = tab.pageId == uiState.activePageId
                    Surface(
                        shape = RoundedCornerShape(10.dp),
                        color = if (active) {
                            MaterialTheme.colorScheme.primaryContainer
                        } else {
                            MaterialTheme.colorScheme.surfaceContainerLow
                        },
                        modifier = Modifier
                            .clip(RoundedCornerShape(10.dp))
                            .clickable { onSelectPage(tab.pageId) }
                    ) {
                        Text(
                            text = "${tab.name} (${tab.scaleLabel})",
                            style = MaterialTheme.typography.labelMedium,
                            color = if (active) {
                                MaterialTheme.colorScheme.onPrimaryContainer
                            } else {
                                MaterialTheme.colorScheme.onSurfaceVariant
                            },
                            modifier = Modifier.padding(horizontal = 10.dp, vertical = 5.dp)
                        )
                    }
                }
                Surface(
                    shape = CircleShape,
                    color = MaterialTheme.colorScheme.surfaceContainerLow,
                    modifier = Modifier
                        .clip(CircleShape)
                        .clickable { onAddPage() }
                ) {
                    Icon(
                        imageVector = Icons.Default.Add,
                        contentDescription = "Add Page",
                        modifier = Modifier
                            .padding(5.dp)
                            .size(16.dp)
                    )
                }
            }

            if (posture != AdaptiveWorkstationPosture.COMPACT_CONVERTIBLE) {
                Text(
                    text = "Cursor: ${uiState.activeCursor.name} • Tool: ${uiState.activeTool.displayName}",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 10.dp)
                )
            }

            Text(
                text = "Zoom ${uiState.zoomPercent}%",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier
                    .clip(RoundedCornerShape(6.dp))
                    .clickable { onZoomReset() }
                    .padding(horizontal = 8.dp, vertical = 4.dp)
            )
        }
    }
}
