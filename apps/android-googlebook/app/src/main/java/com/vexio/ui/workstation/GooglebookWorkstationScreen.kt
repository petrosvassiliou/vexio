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
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.draganddrop.dragAndDropSource
import androidx.compose.foundation.draganddrop.dragAndDropTarget
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
import androidx.compose.material.icons.filled.ChangeHistory
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.CropSquare
import androidx.compose.material.icons.filled.DarkMode
import androidx.compose.material.icons.filled.DeleteOutline
import androidx.compose.material.icons.filled.Diamond
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.FlipToBack
import androidx.compose.material.icons.filled.FlipToFront
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.Functions
import androidx.compose.material.icons.filled.Gesture
import androidx.compose.material.icons.filled.GridOn
import androidx.compose.material.icons.filled.Hexagon
import androidx.compose.material.icons.filled.HorizontalRule
import androidx.compose.material.icons.filled.LightMode
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.NearMe
import androidx.compose.material.icons.filled.OpenInNew
import androidx.compose.material.icons.filled.PanTool
import androidx.compose.material.icons.filled.Print
import androidx.compose.material.icons.filled.RadioButtonUnchecked
import androidx.compose.material.icons.filled.RoundedCorner
import androidx.compose.material.icons.filled.Save
import androidx.compose.material.icons.filled.Schema
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.StarBorder
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
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
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
import androidx.compose.ui.layout.ContentScale
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
    onExportJpgRequested: () -> Unit = { viewModel.exportPlan2Artifact("jpg") },
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
                    // Ctrl + D -> Duplicate Selected Shape
                    ctrlOrMeta && !shift && keyEvent.key == Key.D -> {
                        viewModel.duplicateSelectedShapes()
                        true
                    }
                    // Escape -> Return to Select Tool
                    !ctrlOrMeta && keyEvent.key == Key.Escape -> {
                        viewModel.selectTool(VexioTool.POINTER)
                        true
                    }
                    // Ctrl + 1..9 -> Visio Authoring Tool Parity
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
                    ctrlOrMeta && keyEvent.key == Key.Seven -> {
                        viewModel.selectTool(VexioTool.HIGHLIGHTER)
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
                onLoadTemplate = viewModel::loadStarterTemplate,
                onOpenClick = onOpenDocumentRequested,
                onSaveClick = onSaveDocumentRequested,
                onExportJpgClick = onExportJpgRequested,
                onPrintClick = onPrintRequested,
                onUndoClick = viewModel::performUndo,
                onRedoClick = viewModel::performRedo,
                onToggleTheme = viewModel::toggleDarkTheme,
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

            // 2. Developer ShapeSheet Formula Bar (Hidden by default until user toggles ShapeSheet / Shift+F5)
            androidx.compose.animation.AnimatedVisibility(
                visible = uiState.isShapeSheetDrawerExpanded,
                enter = expandVertically(VexioExpressiveMotion.spatialSpring()) + fadeIn(),
                exit = shrinkVertically(VexioExpressiveMotion.spatialSpring()) + fadeOut()
            ) {
                ShapeSheetFormulaBar(
                    uiState = uiState,
                    formulaFocusRequester = formulaFocusRequester,
                    onFormulaChanged = viewModel::updateFormulaBarInput,
                    onForceSetfToggled = viewModel::setForceSetfOverride,
                    onCommitFormula = { viewModel.commitCellFormula() }
                )
            }

            // 2b. Material 3 Expressive Plan 2 Enterprise Control Strip (Collapsible)
            androidx.compose.animation.AnimatedVisibility(
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
                // Supporting Pane A: All 17 Visio Stencil Categories Navigator (Never gated off when toggled open)
                if (uiState.isStencilDrawerExpanded) {
                    StencilMasterDrawerPane(
                        uiState = uiState,
                        posture = posture,
                        onSelectCategory = { cat ->
                            viewModel.selectStencilCategory(cat)
                            navBackStack.add(WorkstationNavKey.StencilDrawer(cat))
                        },
                        onMasterClicked = { master ->
                            viewModel.instantiateMasterDrop(
                                title = master.title,
                                widthFormula = master.defaultWidthFormula,
                                heightFormula = master.defaultHeightFormula,
                                fillFormula = master.fillFormula,
                                pinXInches = 4.50f,
                                pinYInches = 4.50f,
                                category = master.category
                            )
                        },
                        onOpenNewWindowRequested = {
                            launchNewDocumentWindow(context, template = "blank")
                        }
                    )
                    VerticalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                }

                // Always-Visible Basic Drawing & Diagramming Tool Rail (All 16 Tools)
                PersistentDrawingToolRail(
                    activeTool = uiState.activeTool,
                    onSelectTool = viewModel::selectTool
                )
                VerticalDivider(color = MaterialTheme.colorScheme.outlineVariant)

                // Primary Center Pane: Vulkan SurfaceView + Expressive Morphing Toolbar + ShapeSheet Grid
                Column(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxHeight()
                ) {
                    // Docked Expressive Quick Toolbar
                    androidx.compose.animation.AnimatedVisibility(
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
                                .padding(horizontal = 10.dp, vertical = 5.dp)
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
                                    .padding(top = 12.dp)
                            )
                        }

                        // USI / Active Stylus Live Telemetry Pill Badge
                        androidx.compose.animation.AnimatedVisibility(
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
                    androidx.compose.animation.AnimatedVisibility(
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

                // Supporting Pane B: Inspector & Validation Pane
                val showInspectorPane = uiState.isInspectorExpanded &&
                    posture != AdaptiveWorkstationPosture.COMPACT_CONVERTIBLE
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
                onCreateSubprocess = { viewModel.createOrLinkSubprocess() },
                onAutoLayoutPage = { viewModel.autoLayoutActivePage() },
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
    onLoadTemplate: (String) -> Unit,
    onOpenClick: () -> Unit,
    onSaveClick: () -> Unit,
    onExportJpgClick: () -> Unit = {},
    onPrintClick: () -> Unit,
    onUndoClick: () -> Unit,
    onRedoClick: () -> Unit,
    onToggleTheme: () -> Unit,
    onTogglePlan2Ribbon: () -> Unit,
    onToggleStencilDrawer: () -> Unit,
    onToggleShapeSheet: () -> Unit,
    onToggleInspector: () -> Unit
) {
    var templatesMenuExpanded by remember { mutableStateOf(false) }

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

            // Starter Templates Menu
            Box {
                FilledTonalButton(
                    onClick = { templatesMenuExpanded = true },
                    contentPadding = PaddingValues(horizontal = 10.dp, vertical = 5.dp),
                    shape = RoundedCornerShape(12.dp)
                ) {
                    Text("Templates ▾", style = MaterialTheme.typography.labelMedium)
                }
                DropdownMenu(
                    expanded = templatesMenuExpanded,
                    onDismissRequest = { templatesMenuExpanded = false }
                ) {
                    listOf(
                        "Flowchart Starter" to "flowchart",
                        "BPMN 2.0 Process" to "bpmn",
                        "Network & Cloud" to "network",
                        "Floor Plan & CAD" to "floorplan",
                        "Organization Chart" to "orgchart",
                        "UML 2.5 Class Diagram" to "uml"
                    ).forEach { (label, slug) ->
                        DropdownMenuItem(
                            text = { Text(label) },
                            onClick = {
                                templatesMenuExpanded = false
                                onLoadTemplate(slug)
                            }
                        )
                    }
                }
            }

            Spacer(Modifier.width(4.dp))

            // Undo / Redo
            IconButton(onClick = onUndoClick, enabled = uiState.canUndo) {
                Icon(Icons.AutoMirrored.Filled.Undo, contentDescription = "Undo (Ctrl+Z)")
            }
            IconButton(onClick = onRedoClick, enabled = uiState.canRedo) {
                Icon(Icons.AutoMirrored.Filled.Redo, contentDescription = "Redo (Ctrl+Y)")
            }

            // SAF Open / Save / JPG Export / Native OS Print
            IconButton(onClick = onOpenClick) {
                Icon(Icons.Default.FolderOpen, contentDescription = "Open Diagram (Ctrl+O)")
            }
            IconButton(onClick = onSaveClick) {
                Icon(Icons.Default.Save, contentDescription = "Save Diagram (Ctrl+S)")
            }
            FilledTonalButton(
                onClick = onExportJpgClick,
                contentPadding = PaddingValues(horizontal = 9.dp, vertical = 5.dp),
                shape = RoundedCornerShape(10.dp)
            ) {
                Text("JPG", style = MaterialTheme.typography.labelMedium)
            }
            IconButton(onClick = onPrintClick) {
                Icon(Icons.Default.Print, contentDescription = "Print Vector PDF (Ctrl+P)")
            }

            // Light Mode (Default) / Dark Mode Switch
            IconButton(onClick = onToggleTheme) {
                Icon(
                    imageVector = if (uiState.isDarkTheme) Icons.Default.LightMode else Icons.Default.DarkMode,
                    contentDescription = if (uiState.isDarkTheme) "Switch to Light Mode (Default)" else "Switch to Dark Mode"
                )
            }

            Spacer(Modifier.width(4.dp))

            // Multi-Instance Side-by-Side Window Launcher Button
            FilledTonalButton(
                onClick = onNewWindowClick,
                contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp),
                shape = RoundedCornerShape(14.dp)
            ) {
                Icon(
                    imageVector = Icons.Default.OpenInNew,
                    contentDescription = null,
                    modifier = Modifier.size(15.dp)
                )
                Spacer(Modifier.width(5.dp))
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

            // Pane Toggles (Stencils, Plan 2 Ribbon, ShapeSheet Shift+F5, Inspector)
            FilledTonalIconButton(onClick = onToggleStencilDrawer) {
                Icon(Icons.Default.Schema, contentDescription = "Toggle 22-Pack Stencil Navigator")
            }
            FilledTonalIconButton(onClick = onTogglePlan2Ribbon) {
                Icon(Icons.Default.Architecture, contentDescription = "Toggle Advanced Operations Ribbon")
            }
            FilledTonalIconButton(onClick = onToggleShapeSheet) {
                Icon(Icons.Default.TableChart, contentDescription = "Toggle ShapeSheet Formula Studio (Shift+F5)")
            }
            if (posture != AdaptiveWorkstationPosture.COMPACT_CONVERTIBLE) {
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

private fun resolveToolIcon(tool: VexioTool): ImageVector = when (tool) {
    VexioTool.POINTER -> Icons.Default.NearMe
    VexioTool.PAN_HAND -> Icons.Default.PanTool
    VexioTool.CONNECTOR -> Icons.Default.Timeline
    VexioTool.LINE -> Icons.Default.HorizontalRule
    VexioTool.ARC -> Icons.Default.Gesture
    VexioTool.FREEFORM -> Icons.Default.Architecture
    VexioTool.RECTANGLE -> Icons.Default.CropSquare
    VexioTool.ROUNDED_RECT -> Icons.Default.RoundedCorner
    VexioTool.ELLIPSE -> Icons.Default.RadioButtonUnchecked
    VexioTool.DIAMOND -> Icons.Default.Diamond
    VexioTool.TRIANGLE -> Icons.Default.ChangeHistory
    VexioTool.HEXAGON -> Icons.Default.Hexagon
    VexioTool.STAR -> Icons.Default.StarBorder
    VexioTool.TEXT_BLOCK -> Icons.Default.TextFields
    VexioTool.STYLUS_INK -> Icons.Default.Brush
    VexioTool.HIGHLIGHTER -> Icons.Default.Edit
}

/**
 * Always-visible vertical Basic Drawing & Diagramming Tool Rail exposing all 16 Visio tools.
 */
@Composable
private fun PersistentDrawingToolRail(
    activeTool: VexioTool,
    onSelectTool: (VexioTool) -> Unit
) {
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        modifier = Modifier
            .width(50.dp)
            .fillMaxHeight()
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(3.dp),
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(vertical = 6.dp, horizontal = 4.dp)
        ) {
            VexioTool.entries.forEach { tool ->
                val selected = activeTool == tool
                val btnShape = rememberExpressiveToolButtonShape(isSelected = selected)
                Surface(
                    shape = btnShape,
                    color = if (selected) {
                        MaterialTheme.colorScheme.primary
                    } else {
                        Color.Transparent
                    },
                    modifier = Modifier
                        .size(38.dp)
                        .clip(btnShape)
                        .clickable { onSelectTool(tool) }
                ) {
                    Box(contentAlignment = Alignment.Center) {
                        Icon(
                            imageVector = resolveToolIcon(tool),
                            contentDescription = "${tool.displayName} (${tool.shortcutHint})",
                            tint = if (selected) {
                                MaterialTheme.colorScheme.onPrimary
                            } else {
                                MaterialTheme.colorScheme.onSurfaceVariant
                            },
                            modifier = Modifier.size(19.dp)
                        )
                    }
                }
            }
        }
    }
}

/**
 * Collapsible 17-Category Visio Professional Stencil Navigator with live search,
 * category filter chips, crisp multi-geometry SVG previews, cross-window Drag & Drop,
 * and stencil context menus.
 */
@Composable
private fun StencilMasterDrawerPane(
    uiState: WorkstationUiState,
    posture: AdaptiveWorkstationPosture,
    onSelectCategory: (String) -> Unit,
    onMasterClicked: (StencilMasterItem) -> Unit,
    onOpenNewWindowRequested: () -> Unit
) {
    var searchQuery by remember { mutableStateOf("") }
    var expandedCategories by remember {
        mutableStateOf(
            setOf(
                "Basic Shapes",
                "Basic Flowchart",
                "Arrows & Callouts",
                "BPMN 2.0"
            )
        )
    }

    val drawerWidth = if (posture == AdaptiveWorkstationPosture.COMPACT_CONVERTIBLE) 224.dp else 268.dp
    val trimmedQuery = searchQuery.trim()
    val visibleCategories = remember(uiState.selectedStencilCategory, uiState.stencilCategories) {
        if (uiState.selectedStencilCategory != "All" &&
            uiState.selectedStencilCategory in uiState.stencilCategories
        ) {
            listOf(uiState.selectedStencilCategory)
        } else {
            uiState.stencilCategories
        }
    }

    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        modifier = Modifier
            .width(drawerWidth)
            .fillMaxHeight()
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Header + Expand All / Collapse Toggle
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 12.dp, vertical = 8.dp)
            ) {
                Text(
                    text = "Stencils (${uiState.stencilCategories.size} Packs)",
                    style = MaterialTheme.typography.titleSmall,
                    modifier = Modifier.weight(1f)
                )
                val allExpanded = expandedCategories.size >= uiState.stencilCategories.size
                Text(
                    text = if (allExpanded) "Collapse" else "Expand All",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier
                        .clip(RoundedCornerShape(6.dp))
                        .clickable {
                            expandedCategories = if (allExpanded) {
                                setOf("Basic Shapes", "Basic Flowchart")
                            } else {
                                uiState.stencilCategories.toSet()
                            }
                        }
                        .padding(horizontal = 6.dp, vertical = 3.dp)
                )
            }

            // Search Field
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 10.dp)
                    .clip(RoundedCornerShape(10.dp))
                    .background(MaterialTheme.colorScheme.surfaceContainerLowest)
                    .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(10.dp))
                    .padding(horizontal = 8.dp, vertical = 6.dp)
            ) {
                Icon(
                    imageVector = Icons.Default.Search,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(15.dp)
                )
                Spacer(Modifier.width(6.dp))
                Box(modifier = Modifier.weight(1f)) {
                    if (searchQuery.isEmpty()) {
                        Text(
                            text = "Search ${uiState.stencilMasters.size} shapes in ${uiState.stencilCategories.size} packs…",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis
                        )
                    }
                    BasicTextField(
                        value = searchQuery,
                        onValueChange = { searchQuery = it },
                        singleLine = true,
                        textStyle = MaterialTheme.typography.labelMedium.copy(
                            color = MaterialTheme.colorScheme.onSurface
                        ),
                        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                        modifier = Modifier.fillMaxWidth()
                    )
                }
                if (searchQuery.isNotEmpty()) {
                    Icon(
                        imageVector = Icons.Default.Close,
                        contentDescription = "Clear Search",
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier
                            .size(15.dp)
                            .clickable { searchQuery = "" }
                    )
                }
            }

            Spacer(Modifier.height(6.dp))

            // Horizontal Category Filter Pills (All + 17 Packs)
            Row(
                horizontalArrangement = Arrangement.spacedBy(5.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState())
                    .padding(horizontal = 10.dp, vertical = 2.dp)
            ) {
                (listOf("All") + uiState.stencilCategories).forEach { cat ->
                    val active = uiState.selectedStencilCategory == cat
                    Surface(
                        shape = CircleShape,
                        color = if (active) {
                            MaterialTheme.colorScheme.primaryContainer
                        } else {
                            MaterialTheme.colorScheme.surfaceContainer
                        },
                        border = BorderStroke(
                            width = 1.dp,
                            color = if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant
                        ),
                        modifier = Modifier
                            .clip(CircleShape)
                            .clickable {
                                onSelectCategory(cat)
                                if (cat != "All") {
                                    expandedCategories = expandedCategories + cat
                                }
                            }
                    ) {
                        Text(
                            text = cat,
                            style = MaterialTheme.typography.labelSmall,
                            color = if (active) {
                                MaterialTheme.colorScheme.onPrimaryContainer
                            } else {
                                MaterialTheme.colorScheme.onSurfaceVariant
                            },
                            modifier = Modifier.padding(horizontal = 9.dp, vertical = 4.dp)
                        )
                    }
                }
            }

            Spacer(Modifier.height(4.dp))
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)

            // 17-Category Accordion List
            LazyColumn(
                contentPadding = PaddingValues(10.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.fillMaxSize()
            ) {
                visibleCategories.forEach { category ->
                    val mastersInCat = uiState.stencilMasters.filter { m ->
                        m.category == category && (
                            trimmedQuery.isEmpty() ||
                                m.title.contains(trimmedQuery, ignoreCase = true) ||
                                m.category.contains(trimmedQuery, ignoreCase = true) ||
                                m.prompt.contains(trimmedQuery, ignoreCase = true)
                            )
                    }
                    if (mastersInCat.isNotEmpty()) {
                        val isExpanded = trimmedQuery.isNotEmpty() ||
                            uiState.selectedStencilCategory == category ||
                            category in expandedCategories

                        item(key = "header::$category") {
                            Surface(
                                shape = RoundedCornerShape(10.dp),
                                color = MaterialTheme.colorScheme.surfaceContainer,
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .clip(RoundedCornerShape(10.dp))
                                    .clickable {
                                        expandedCategories = if (category in expandedCategories) {
                                            expandedCategories - category
                                        } else {
                                            expandedCategories + category
                                        }
                                    }
                            ) {
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    modifier = Modifier.padding(horizontal = 10.dp, vertical = 7.dp)
                                ) {
                                    Icon(
                                        imageVector = if (isExpanded) Icons.Default.ExpandLess else Icons.Default.ExpandMore,
                                        contentDescription = null,
                                        modifier = Modifier.size(16.dp),
                                        tint = MaterialTheme.colorScheme.primary
                                    )
                                    Spacer(Modifier.width(6.dp))
                                    Text(
                                        text = category,
                                        style = MaterialTheme.typography.labelLarge,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                        modifier = Modifier.weight(1f)
                                    )
                                    Badge(
                                        containerColor = MaterialTheme.colorScheme.secondaryContainer,
                                        contentColor = MaterialTheme.colorScheme.onSecondaryContainer
                                    ) {
                                        Text(
                                            text = "${mastersInCat.size}",
                                            modifier = Modifier.padding(horizontal = 4.dp, vertical = 1.dp)
                                        )
                                    }
                                }
                            }
                        }

                        if (isExpanded) {
                            items(mastersInCat, key = { it.masterId }) { master ->
                                DraggableStencilMasterCard(
                                    windowInstanceId = uiState.windowInstanceId,
                                    master = master,
                                    onClick = { onMasterClicked(master) },
                                    onOpenNewWindowRequested = onOpenNewWindowRequested
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

private data class ParsedComposeSvgPath(
    val path: Path,
    val fillColor: Color?,
    val strokeColor: Color?,
    val strokeWidthPx: Float
)

private fun extractSvgAttr(name: String, tagSlice: String): String? {
    val needle = "$name=\""
    val start = tagSlice.indexOf(needle)
    if (start < 0) return null
    val valStart = start + needle.length
    val valEnd = tagSlice.indexOf('"', valStart)
    if (valEnd < 0) return null
    return tagSlice.substring(valStart, valEnd)
}

private fun parseSvgHexColor(raw: String?, fallback: Color): Color? {
    val trimmed = raw?.trim().orEmpty()
    if (trimmed.isEmpty() || trimmed == "none") return null
    if (trimmed.startsWith("url(")) return fallback.copy(alpha = 0.16f)
    val hex = trimmed.removePrefix("#")
    if (hex.length == 6) {
        val rgb = hex.toLongOrNull(16)
        if (rgb != null) {
            return Color(0xFF000000L or rgb)
        }
    }
    return fallback
}

private fun parseStencilSvgPaths(svg: String, defaultAccent: Color): List<ParsedComposeSvgPath> {
    if (svg.isBlank()) return emptyList()
    val groupIdx = svg.indexOf("data-shape-id=")
    if (groupIdx < 0) return emptyList()
    val body = svg.substring(groupIdx)
    val list = mutableListOf<ParsedComposeSvgPath>()
    var cursor = 0
    while (cursor < body.length) {
        val pStart = body.indexOf("<path ", cursor)
        if (pStart < 0) break
        val pEnd = body.indexOf('>', pStart)
        if (pEnd < 0) break
        val tag = body.substring(pStart + 6, pEnd)
        val d = extractSvgAttr("d", tag)
        if (!d.isNullOrBlank()) {
            val parsedPath = runCatching { PathParser().parsePathString(d).toPath() }.getOrNull()
            if (parsedPath != null) {
                val fill = parseSvgHexColor(extractSvgAttr("fill", tag), defaultAccent)
                val stroke = parseSvgHexColor(extractSvgAttr("stroke", tag), defaultAccent)
                val sw = extractSvgAttr("stroke-width", tag)?.toFloatOrNull() ?: 1.3f
                list.add(ParsedComposeSvgPath(parsedPath, fill, stroke, sw))
            }
        }
        cursor = pEnd + 1
    }
    return list
}

/**
 * Stencil Master card acting as a cross-window Drag & Drop source (`DRAG_FLAG_GLOBAL`),
 * rendering crisp multi-geometry SVG vector previews via Compose `PathParser`, and exposing
 * right-click / long-press context menus.
 */
@Composable
private fun DraggableStencilMasterCard(
    windowInstanceId: String,
    master: StencilMasterItem,
    onClick: () -> Unit,
    onOpenNewWindowRequested: () -> Unit
) {
    var cardMenuExpanded by remember { mutableStateOf(false) }
    val primaryStroke = MaterialTheme.colorScheme.primary
    val parsedSvgPaths = remember(master.svgPreview, primaryStroke) {
        parseStencilSvgPaths(master.svgPreview, primaryStroke)
    }

    val jsonPayload = remember(windowInstanceId, master) {
        Json.encodeToString(
            CrossWindowShapePayload(
                sourceWindowId = windowInstanceId,
                masterId = master.masterId,
                title = master.title,
                widthFormula = master.defaultWidthFormula,
                heightFormula = master.defaultHeightFormula,
                fillFormula = master.fillFormula,
                category = master.category
            )
        )
    }

    Box {
        Surface(
            shape = RoundedCornerShape(12.dp),
            color = MaterialTheme.colorScheme.surfaceContainerLowest,
            border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(12.dp))
                .combinedClickable(
                    onClick = onClick,
                    onLongClick = { cardMenuExpanded = true }
                )
                .dragAndDropSource(
                    transferData = { _: Offset ->
                        val item = ClipData.Item(jsonPayload)
                        val clipData = ClipData(
                            master.title,
                            arrayOf(VEXIO_MASTER_CLIP_MIME, ClipDescription.MIMETYPE_TEXT_PLAIN),
                            item
                        )
                        DragAndDropTransferData(
                            clipData = clipData,
                            flags = View.DRAG_FLAG_GLOBAL or View.DRAG_FLAG_GLOBAL_URI_READ
                        )
                    }
                )
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.padding(horizontal = 10.dp, vertical = 8.dp)
            ) {
                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = Color(0xFFFFFFFF),
                    border = BorderStroke(1.dp, Color(0xFFCBD5E1)),
                    modifier = Modifier.size(width = 56.dp, height = 40.dp)
                ) {
                    val fillTint = primaryStroke.copy(alpha = 0.14f)
                    androidx.compose.foundation.Canvas(
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(3.dp)
                    ) {
                        val w = size.width
                        val h = size.height
                        if (parsedSvgPaths.isNotEmpty()) {
                            val srcW = 144f
                            val srcH = 96f
                            val fitScale = minOf(w / srcW, h / srcH)
                            val offX = (w - srcW * fitScale) * 0.5f
                            val offY = (h - srcH * fitScale) * 0.5f
                            withTransform({
                                translate(left = offX, top = offY)
                                scale(scaleX = fitScale, scaleY = fitScale, pivot = Offset.Zero)
                            }) {
                                parsedSvgPaths.forEach { seg ->
                                    if (seg.fillColor != null) {
                                        drawPath(seg.path, color = seg.fillColor)
                                    }
                                    if (seg.strokeColor != null) {
                                        drawPath(
                                            seg.path,
                                            color = seg.strokeColor,
                                            style = Stroke(width = (seg.strokeWidthPx).coerceAtLeast(1.6f))
                                        )
                                    }
                                }
                            }
                        } else {
                            val strokeStyle = Stroke(width = 1.7.dp.toPx())
                            val low = master.title.lowercase()
                            when {
                                master.shapeKind == "Circle" || "event" in low || "pump" in low -> {
                                    val r = minOf(w, h) * 0.42f
                                    drawCircle(color = fillTint, radius = r)
                                    drawCircle(color = primaryStroke, radius = r, style = strokeStyle)
                                }
                                master.shapeKind == "Diamond" || "gateway" in low || "decision" in low -> {
                                    val p = Path().apply {
                                        moveTo(w * 0.5f, 0f)
                                        lineTo(w, h * 0.5f)
                                        lineTo(w * 0.5f, h)
                                        lineTo(0f, h * 0.5f)
                                        close()
                                    }
                                    drawPath(p, color = fillTint)
                                    drawPath(p, color = primaryStroke, style = strokeStyle)
                                }
                                master.shapeKind == "Hexagon" || "cluster" in low -> {
                                    val p = Path().apply {
                                        moveTo(w * 0.22f, 0f)
                                        lineTo(w * 0.78f, 0f)
                                        lineTo(w, h * 0.5f)
                                        lineTo(w * 0.78f, h)
                                        lineTo(w * 0.22f, h)
                                        lineTo(0f, h * 0.5f)
                                        close()
                                    }
                                    drawPath(p, color = fillTint)
                                    drawPath(p, color = primaryStroke, style = strokeStyle)
                                }
                                master.shapeKind == "Star" || "star" in low -> {
                                    val p = Path().apply {
                                        moveTo(w * 0.5f, 0f)
                                        lineTo(w * 0.63f, h * 0.36f)
                                        lineTo(w, h * 0.38f)
                                        lineTo(w * 0.70f, h * 0.62f)
                                        lineTo(w * 0.81f, h)
                                        lineTo(w * 0.5f, h * 0.77f)
                                        lineTo(w * 0.19f, h)
                                        lineTo(w * 0.30f, h * 0.62f)
                                        lineTo(0f, h * 0.38f)
                                        lineTo(w * 0.37f, h * 0.36f)
                                        close()
                                    }
                                    drawPath(p, color = fillTint)
                                    drawPath(p, color = primaryStroke, style = strokeStyle)
                                }
                                master.shapeKind == "Arrow" || "arrow" in low -> {
                                    val p = Path().apply {
                                        moveTo(0f, h * 0.28f)
                                        lineTo(w * 0.62f, h * 0.28f)
                                        lineTo(w * 0.62f, 0f)
                                        lineTo(w, h * 0.5f)
                                        lineTo(w * 0.62f, h)
                                        lineTo(w * 0.62f, h * 0.72f)
                                        lineTo(0f, h * 0.72f)
                                        close()
                                    }
                                    drawPath(p, color = fillTint)
                                    drawPath(p, color = primaryStroke, style = strokeStyle)
                                }
                                master.shapeKind == "ClassBox" || "class" in low || "entity" in low || "window" in low -> {
                                    drawRect(color = fillTint)
                                    drawRect(color = primaryStroke, style = strokeStyle)
                                    drawLine(primaryStroke, Offset(0f, h * 0.32f), Offset(w, h * 0.32f), strokeWidth = strokeStyle.width * 0.8f)
                                }
                                master.shapeKind == "Cylinder" || "database" in low -> {
                                    drawRoundRect(color = fillTint, cornerRadius = CornerRadius(6.dp.toPx()))
                                    drawRoundRect(color = primaryStroke, cornerRadius = CornerRadius(6.dp.toPx()), style = strokeStyle)
                                    drawLine(primaryStroke, Offset(0f, h * 0.28f), Offset(w, h * 0.28f), strokeWidth = strokeStyle.width * 0.8f)
                                }
                                else -> {
                                    val cr = CornerRadius(if (master.shapeKind == "Pill" || "start" in low) h * 0.5f else 4.dp.toPx())
                                    drawRoundRect(color = fillTint, cornerRadius = cr)
                                    drawRoundRect(color = primaryStroke, cornerRadius = cr, style = strokeStyle)
                                }
                            }
                        }
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
                        text = master.category,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
            }
        }

        DropdownMenu(
            expanded = cardMenuExpanded,
            onDismissRequest = { cardMenuExpanded = false }
        ) {
            DropdownMenuItem(
                text = { Text("Add '${master.title}' to Page") },
                leadingIcon = { Icon(Icons.Default.Add, contentDescription = null) },
                onClick = {
                    cardMenuExpanded = false
                    onClick()
                }
            )
            DropdownMenuItem(
                text = { Text("Open New Side-by-Side Window") },
                leadingIcon = { Icon(Icons.Default.OpenInNew, contentDescription = null) },
                onClick = {
                    cardMenuExpanded = false
                    onOpenNewWindowRequested()
                }
            )
        }
    }
}

/**
 * Material 3 Expressive Toolbar that morphs smoothly between a floating capsule pill (`28.dp`)
 * and a docked desktop squircle bar (`16.dp`), exposing all 16 Visio drawing & authoring tools.
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
            horizontalArrangement = Arrangement.spacedBy(3.dp),
            modifier = Modifier
                .horizontalScroll(rememberScrollState())
                .padding(horizontal = 10.dp, vertical = 5.dp)
        ) {
            VexioTool.entries.forEach { tool ->
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
                            horizontal = if (isSelected) 11.dp else 8.dp,
                            vertical = 6.dp
                        )
                    ) {
                        Icon(
                            imageVector = resolveToolIcon(tool),
                            contentDescription = "${tool.displayName} (${tool.shortcutHint})",
                            tint = contentColor,
                            modifier = Modifier.size(17.dp)
                        )
                        if (isSelected) {
                            Spacer(Modifier.width(5.dp))
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
                    .height(22.dp)
                    .padding(horizontal = 4.dp),
                color = MaterialTheme.colorScheme.outlineVariant
            )

            IconButton(
                onClick = { onZoomDelta(-15) },
                modifier = Modifier.size(30.dp)
            ) {
                Icon(
                    imageVector = Icons.Default.ZoomOut,
                    contentDescription = "Zoom Out",
                    modifier = Modifier.size(17.dp)
                )
            }
            Text(
                text = "${uiState.zoomPercent}%",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurface
            )
            IconButton(
                onClick = { onZoomDelta(15) },
                modifier = Modifier.size(30.dp)
            ) {
                Icon(
                    imageVector = Icons.Default.ZoomIn,
                    contentDescription = "Zoom In",
                    modifier = Modifier.size(17.dp)
                )
            }

            IconButton(
                onClick = onToggleFloating,
                modifier = Modifier.size(30.dp)
            ) {
                Icon(
                    imageVector = Icons.Default.Tune,
                    contentDescription = if (isFloating) "Dock Toolbar" else "Float Pill Toolbar",
                    modifier = Modifier.size(16.dp)
                )
            }
        }
    }
}

/**
 * Vulkan `SurfaceView` Canvas container equipped with:
 * - Zero-flicker `SurfaceHolder.Callback` (`nativeAttachVulkanSurface` / `nativeResizeViewport`)
 * - Software RGBA viewport fallback `Image` when Vulkan swapchain is absent
 * - Hardware mouse cursor binding (`PointerIcon.getSystemIcon`)
 * - Unbuffered low-latency USI active stylus dispatch + `MotionEventPredictor` + Palm Rejection
 * - Cross-Window Drag & Drop target accepting Vexio Master JSON payloads & `.vsdx`/`.vsd`/`.dwg`/`.dxf` URIs
 * - Full desktop hardware mouse secondary-button (right-click) context menu
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
    var lastRightClickPageIn by remember { mutableStateOf(5.5 to 4.25) }

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
                            val dpi = 96f * density * (viewModel.uiState.value.zoomPercent / 100f)
                            val pinXIn = (dragEvent.x / dpi.coerceAtLeast(1f)).coerceIn(0.5f, 10.5f)
                            val pinYIn = (8.5f - (dragEvent.y / dpi.coerceAtLeast(1f))).coerceIn(0.5f, 8.0f)
                            viewModel.instantiateMasterDrop(
                                title = payload.title,
                                widthFormula = payload.widthFormula,
                                heightFormula = payload.heightFormula,
                                fillFormula = payload.fillFormula,
                                pinXInches = pinXIn,
                                pinYInches = pinYIn,
                                category = payload.category
                            )
                            return true
                        }
                    }
                } finally {
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

                    fun showCanvasContextMenuAt(xPx: Float, yPx: Float) {
                        val pxToDp = ctx.resources.displayMetrics.density.coerceAtLeast(1f)
                        contextMenuOffsetDp = DpOffset(
                            x = (xPx / pxToDp).dp,
                            y = (yPx / pxToDp).dp
                        )
                        val dpi = (96f * pxToDp * (viewModel.uiState.value.zoomPercent / 100f)).coerceAtLeast(1f)
                        val xIn = (xPx / dpi).toDouble().coerceIn(1.0, 10.0)
                        val yIn = (8.5 - (yPx / dpi).toDouble()).coerceIn(1.0, 7.5)
                        lastRightClickPageIn = xIn to yIn
                        contextMenuVisible = true
                    }

                    setOnTouchListener { view, motionEvent ->
                        if (motionEvent.actionMasked == MotionEvent.ACTION_DOWN) {
                            val tool = motionEvent.getToolType(0)
                            if (tool == MotionEvent.TOOL_TYPE_STYLUS ||
                                tool == MotionEvent.TOOL_TYPE_ERASER
                            ) {
                                view.requestUnbufferedDispatch(motionEvent)
                            }
                            if ((motionEvent.buttonState and MotionEvent.BUTTON_SECONDARY) != 0) {
                                showCanvasContextMenuAt(motionEvent.x, motionEvent.y)
                                return@setOnTouchListener true
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
                        if (motionEvent.actionMasked == MotionEvent.ACTION_BUTTON_PRESS &&
                            motionEvent.actionButton == MotionEvent.BUTTON_SECONDARY
                        ) {
                            showCanvasContextMenuAt(motionEvent.x, motionEvent.y)
                            return@setOnGenericMotionListener true
                        }
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

        // Software RGBA Viewport Fallback Overlay (active when device lacks Vulkan swapchain)
        val fallbackBitmap = uiState.canvasFallbackBitmap
        if (fallbackBitmap != null) {
            Image(
                bitmap = fallbackBitmap.asImageBitmap(),
                contentDescription = "Diagram Canvas",
                contentScale = ContentScale.FillBounds,
                modifier = Modifier.fillMaxSize()
            )
        }

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

        // Full Desktop Right-Click Context Menu
        DropdownMenu(
            expanded = contextMenuVisible,
            onDismissRequest = { contextMenuVisible = false },
            offset = contextMenuOffsetDp
        ) {
            DropdownMenuItem(
                text = { Text("Duplicate Selection (Ctrl+D)") },
                leadingIcon = { Icon(Icons.Default.ContentCopy, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.duplicateSelectedShapes()
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
            HorizontalDivider()
            DropdownMenuItem(
                text = { Text("Bring to Front") },
                leadingIcon = { Icon(Icons.Default.FlipToFront, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.reorderSelectedShapes("front")
                }
            )
            DropdownMenuItem(
                text = { Text("Send to Back") },
                leadingIcon = { Icon(Icons.Default.FlipToBack, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.reorderSelectedShapes("back")
                }
            )
            HorizontalDivider()
            DropdownMenuItem(
                text = { Text("Insert Rectangle Here") },
                leadingIcon = { Icon(Icons.Default.CropSquare, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.drawQuickPrimitive("rect", lastRightClickPageIn.first, lastRightClickPageIn.second)
                }
            )
            DropdownMenuItem(
                text = { Text("Insert Circle / Ellipse Here") },
                leadingIcon = { Icon(Icons.Default.RadioButtonUnchecked, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.drawQuickPrimitive("ellipse", lastRightClickPageIn.first, lastRightClickPageIn.second)
                }
            )
            DropdownMenuItem(
                text = { Text("Insert Diamond Here") },
                leadingIcon = { Icon(Icons.Default.Diamond, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.drawQuickPrimitive("diamond", lastRightClickPageIn.first, lastRightClickPageIn.second)
                }
            )
            DropdownMenuItem(
                text = { Text("Insert Hexagon Here") },
                leadingIcon = { Icon(Icons.Default.Hexagon, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.drawQuickPrimitive("hexagon", lastRightClickPageIn.first, lastRightClickPageIn.second)
                }
            )
            DropdownMenuItem(
                text = { Text("Insert 5-Point Star Here") },
                leadingIcon = { Icon(Icons.Default.StarBorder, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.drawQuickPrimitive("star", lastRightClickPageIn.first, lastRightClickPageIn.second)
                }
            )
            DropdownMenuItem(
                text = { Text("Insert Straight Line Here") },
                leadingIcon = { Icon(Icons.Default.HorizontalRule, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.drawQuickPrimitive("line", lastRightClickPageIn.first, lastRightClickPageIn.second)
                }
            )
            HorizontalDivider()
            DropdownMenuItem(
                text = { Text("Auto-Layout Active Page") },
                leadingIcon = { Icon(Icons.Default.Schema, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.autoLayoutActivePage()
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
                text = { Text("Inspect ShapeSheet (Shift+F5)") },
                leadingIcon = { Icon(Icons.Default.TableChart, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    if (!uiState.isShapeSheetDrawerExpanded) {
                        viewModel.toggleShapeSheetDrawer()
                    }
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
    onCreateSubprocess: () -> Unit,
    onAutoLayoutPage: () -> Unit,
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
            // Page Tabs with Context Menus
            Row(
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                modifier = Modifier
                    .weight(1f)
                    .horizontalScroll(rememberScrollState())
            ) {
                uiState.pages.forEach { tab ->
                    val active = tab.pageId == uiState.activePageId
                    var pageMenuExpanded by remember(tab.pageId) { mutableStateOf(false) }

                    Box {
                        Surface(
                            shape = RoundedCornerShape(10.dp),
                            color = if (active) {
                                MaterialTheme.colorScheme.primaryContainer
                            } else {
                                MaterialTheme.colorScheme.surfaceContainerLow
                            },
                            modifier = Modifier
                                .clip(RoundedCornerShape(10.dp))
                                .combinedClickable(
                                    onClick = { onSelectPage(tab.pageId) },
                                    onLongClick = { pageMenuExpanded = true }
                                )
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

                        DropdownMenu(
                            expanded = pageMenuExpanded,
                            onDismissRequest = { pageMenuExpanded = false }
                        ) {
                            DropdownMenuItem(
                                text = { Text("Switch to ${tab.name}") },
                                onClick = {
                                    pageMenuExpanded = false
                                    onSelectPage(tab.pageId)
                                }
                            )
                            DropdownMenuItem(
                                text = { Text("Insert New Page") },
                                leadingIcon = { Icon(Icons.Default.Add, contentDescription = null) },
                                onClick = {
                                    pageMenuExpanded = false
                                    onAddPage()
                                }
                            )
                            DropdownMenuItem(
                                text = { Text("Create Subprocess Page") },
                                leadingIcon = { Icon(Icons.Default.AccountTree, contentDescription = null) },
                                onClick = {
                                    pageMenuExpanded = false
                                    onCreateSubprocess()
                                }
                            )
                            DropdownMenuItem(
                                text = { Text("Auto-Layout Active Page") },
                                leadingIcon = { Icon(Icons.Default.Schema, contentDescription = null) },
                                onClick = {
                                    pageMenuExpanded = false
                                    onAutoLayoutPage()
                                }
                            )
                        }
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
