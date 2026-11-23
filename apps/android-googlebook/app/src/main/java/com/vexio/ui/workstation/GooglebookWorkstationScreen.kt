package com.vexio.ui.workstation

import android.app.Activity
import android.content.ClipData
import android.content.ClipDescription
import android.content.Context
import android.content.Intent
import android.graphics.BitmapFactory
import android.net.Uri
import android.util.Base64
import android.view.DragEvent
import android.view.GestureDetector
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
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
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
import androidx.compose.material3.AlertDialog
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
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
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
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.PointerIcon
import androidx.compose.ui.input.pointer.pointerHoverIcon
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
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
import com.vexio.core.VexioAppLanguage
import com.vexio.core.VexioBooleanOp
import com.vexio.core.VexioCursorKind
import com.vexio.core.VexioPagePresetSpec
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
    "vsdx", "vsdm", "vssx", "vssm", "vstx", "vstm", "vsd", "vss", "vst", "vdx", "vdw", "dwg", "dxf"
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
    onImportStencilRequested: () -> Unit = {},
    onExportJpgRequested: () -> Unit = { viewModel.exportPlan2Artifact("jpg") },
    onExportFormatRequested: (String) -> Unit = { fmt -> viewModel.exportPlan2Artifact(fmt) },
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
                        if (uiState.inlineTextEditor == null) {
                            viewModel.deleteSelectedShape()
                            true
                        } else {
                            false
                        }
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
                    // Ctrl + G / Ctrl + Shift + G -> Group / Ungroup Selected Shapes
                    ctrlOrMeta && !shift && keyEvent.key == Key.G -> {
                        viewModel.groupSelectedShapes()
                        true
                    }
                    ctrlOrMeta && shift && keyEvent.key == Key.G -> {
                        viewModel.ungroupSelectedShapes()
                        true
                    }
                    // Escape -> Cancel Inline Edit / Close Backstage / Return to Select Tool
                    !ctrlOrMeta && keyEvent.key == Key.Escape -> {
                        when {
                            uiState.inlineTextEditor != null -> viewModel.cancelInlineTextEdit()
                            uiState.showStartBackstage -> viewModel.setStartBackstageVisible(false)
                            else -> viewModel.selectTool(VexioTool.POINTER)
                        }
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

        var activeColorPickerTarget by remember { mutableStateOf<VexioColorPickerTarget?>(null) }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .statusBarsPadding()
        ) {
            // 1. Clean Top Workstation Header Bar (Double-Click Title Rename, Sibling Hover Menus, Save & Save As, Language & Updates)
            WorkstationHeaderBar(
                uiState = uiState,
                posture = posture,
                onToggleBackstage = { viewModel.setStartBackstageVisible(!uiState.showStartBackstage) },
                onCreateBlankA4 = {
                    viewModel.createBlankWithPagePreset("Drawing_A4.vsdx", VexioPagePresetSpec.A4_LANDSCAPE)
                },
                onRenameDocument = viewModel::renameDocument,
                onSelectLanguage = viewModel::setLanguage,
                onCheckUpdates = { viewModel.checkForUpdatesAsync(userInitiated = true) },
                onImportStencilClick = onImportStencilRequested,
                onNewWindowClick = { launchNewDocumentWindow(context, template = "blank") },
                onOpenClick = onOpenDocumentRequested,
                onSaveClick = onSaveDocumentRequested,
                onSaveAsClick = onSaveAsDocumentRequested,
                onExportJpgClick = onExportJpgRequested,
                onExportFormat = onExportFormatRequested,
                onPrintClick = onPrintRequested,
                onUndoClick = viewModel::performUndo,
                onRedoClick = viewModel::performRedo,
                onToggleTheme = viewModel::toggleDarkTheme,
                onToggleStencilDrawer = viewModel::toggleStencilDrawer,
                onToggleShapeSheet = viewModel::toggleShapeSheetDrawer,
                onToggleInspector = viewModel::toggleInspectorPane
            )

            // 1b. Classic Collapsible Multi-Tab Visio Ribbon (Home OS Fonts + Full Color Picker + Rotate/Flip, Insert, Design ISO A4/A3, Data, Process, Review, View, Developer)
            ClassicVisioM3Ribbon(
                uiState = uiState,
                viewModel = viewModel,
                onOpenBackstage = { viewModel.setStartBackstageVisible(true) },
                onImportStencilClick = onImportStencilRequested,
                onOpenColorPicker = { target -> activeColorPickerTarget = target },
                onTogglePlan2Strip = {
                    viewModel.togglePlan2Ribbon()
                    if (navBackStack.lastOrNull() != WorkstationNavKey.Plan2Operations) {
                        navBackStack.add(WorkstationNavKey.Plan2Operations)
                    }
                }
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
                    onExportPlan2Format = onExportFormatRequested
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
                // Supporting Pane A: All 22+ Visio Stencil Categories + Custom .vssx/.vss Import Navigator
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
                        onImportStencilRequested = onImportStencilRequested,
                        onOpenOfficialCatalogRequested = {
                            viewModel.setOfficialStencilCatalogDialogVisible(true)
                        },
                        onOpenNewWindowRequested = {
                            launchNewDocumentWindow(context, template = "blank")
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
                        viewModel = viewModel,
                        onOpenColorPicker = { target -> activeColorPickerTarget = target },
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

            // 4. Bottom Page Switcher & Workstation Diagnostics Bar (Single-line non-wrapping + Page Rename)
            WorkstationBottomStatusBar(
                uiState = uiState,
                posture = posture,
                onSelectPage = viewModel::selectPage,
                onRenamePage = viewModel::renamePage,
                onAddPage = viewModel::addNewPage,
                onCreateSubprocess = { viewModel.createOrLinkSubprocess() },
                onAutoLayoutPage = { viewModel.autoLayoutActivePage() },
                onZoomReset = { viewModel.setZoomPercent(100) }
            )
        }

        // Full-Spectrum Material 3 Color Picker Dialog (Text Color, Highlight, Shape Fill, Line Color)
        val pickerTarget = activeColorPickerTarget
        if (pickerTarget != null) {
            val lang = uiState.language
            val (dialogTitle, initialHex, allowClear) = when (pickerTarget) {
                VexioColorPickerTarget.TEXT_FG -> Triple(lang.textColorLabel, uiState.textColorHex, false)
                VexioColorPickerTarget.TEXT_BG -> Triple(
                    lang.highlightColorLabel,
                    if (uiState.textBgColorHex == "none") "#FEF08A" else uiState.textBgColorHex,
                    true
                )
                VexioColorPickerTarget.SHAPE_FILL -> Triple(lang.fillColorLabel, uiState.shapeFillColorHex, false)
                VexioColorPickerTarget.SHAPE_LINE -> Triple(lang.lineColorLabel, uiState.shapeLineColorHex, false)
            }
            FullSpectrumColorPickerDialog(
                title = dialogTitle,
                initialHex = initialHex,
                allowClearNone = allowClear,
                clearLabel = lang.clearHighlightLabel,
                onDismiss = { activeColorPickerTarget = null },
                onClearNone = {
                    activeColorPickerTarget = null
                    if (pickerTarget == VexioColorPickerTarget.TEXT_BG) {
                        viewModel.applyRichTextStyle(bgColorHex = "none")
                    }
                },
                onColorSelected = { selectedHex ->
                    activeColorPickerTarget = null
                    when (pickerTarget) {
                        VexioColorPickerTarget.TEXT_FG -> viewModel.applyRichTextStyle(colorHex = selectedHex)
                        VexioColorPickerTarget.TEXT_BG -> viewModel.applyRichTextStyle(bgColorHex = selectedHex)
                        VexioColorPickerTarget.SHAPE_FILL -> viewModel.applyShapeFillColor(selectedHex)
                        VexioColorPickerTarget.SHAPE_LINE -> viewModel.applyShapeLineColor(selectedHex)
                    }
                }
            )
        }

        // In-App Welcome / Start & File Backstage Hub (No external OS launch file dialog)
        if (uiState.showStartBackstage) {
            InAppStartBackstageOverlay(
                uiState = uiState,
                onClose = { viewModel.setStartBackstageVisible(false) },
                onCreateBlankPreset = { title, preset ->
                    viewModel.createBlankWithPagePreset(title, preset)
                },
                onLoadTemplate = viewModel::loadStarterTemplate,
                onOpenFile = {
                    viewModel.setStartBackstageVisible(false)
                    onOpenDocumentRequested()
                },
                onImportStencil = onImportStencilRequested,
                onSaveFile = {
                    viewModel.setStartBackstageVisible(false)
                    onSaveDocumentRequested()
                },
                onSaveAsFile = {
                    viewModel.setStartBackstageVisible(false)
                    onSaveAsDocumentRequested()
                },
                onPrint = {
                    viewModel.setStartBackstageVisible(false)
                    onPrintRequested()
                },
                onSelectLanguage = viewModel::setLanguage,
                onToggleAutoUpdates = viewModel::setAutoCheckUpdates,
                onCheckUpdates = { viewModel.checkForUpdatesAsync(userInitiated = true) }
            )
        }

        // Automatic & Manual Update Check Dialog
        if (uiState.showUpdateDialog) {
            AlertDialog(
                onDismissRequest = viewModel::dismissUpdateDialog,
                title = {
                    Text(
                        if (uiState.availableUpdateTag != null) {
                            "Update Available (${uiState.availableUpdateTag})"
                        } else {
                            uiState.language.checkUpdatesLabel
                        }
                    )
                },
                text = { Text(uiState.updateStatusMessage) },
                confirmButton = {
                    if (uiState.availableUpdateUrl != null) {
                        FilledTonalButton(
                            onClick = {
                                val targetUrl = uiState.availableUpdateUrl
                                viewModel.dismissUpdateDialog()
                                if (!targetUrl.isNullOrBlank()) {
                                    runCatching {
                                        context.startActivity(
                                            Intent(Intent.ACTION_VIEW, Uri.parse(targetUrl))
                                                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                                        )
                                    }
                                }
                            }
                        ) {
                            Text(if (uiState.availableUpdateTag != null) "Download APK Update" else "Release Notes")
                        }
                    }
                },
                dismissButton = {
                    TextButton(onClick = viewModel::dismissUpdateDialog) {
                        Text("Close")
                    }
                }
            )
        }

        // Official & Enterprise Stencil Catalog Dialog (.vssx, .vss, .zip SVG)
        if (uiState.showOfficialStencilCatalogDialog) {
            AlertDialog(
                onDismissRequest = { viewModel.setOfficialStencilCatalogDialogVisible(false) },
                title = {
                    Text("Official & Enterprise Stencil Catalog")
                },
                text = {
                    Column(
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(max = 420.dp)
                            .verticalScroll(rememberScrollState())
                    ) {
                        Text(
                            text = "1-click download & local persistence across restarts (Microsoft 365, Azure V25, Cisco, Panduit, APC & NetEquip .vssx / .vss / .zip).",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        uiState.officialStencilCatalog.forEach { entry ->
                            val isBusy = entry.id in uiState.downloadingStencilPackIds
                            val isInstalled = entry.fileName in uiState.installedStencilFileNames
                            Surface(
                                shape = RoundedCornerShape(10.dp),
                                color = MaterialTheme.colorScheme.surfaceContainerLow,
                                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                                modifier = Modifier.fillMaxWidth()
                            ) {
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                    modifier = Modifier.padding(horizontal = 10.dp, vertical = 8.dp)
                                ) {
                                    Column(modifier = Modifier.weight(1f).padding(end = 8.dp)) {
                                        Text(
                                            text = entry.title,
                                            style = MaterialTheme.typography.labelLarge,
                                            fontWeight = FontWeight.SemiBold
                                        )
                                        Text(
                                            text = "${entry.fileName.substringAfterLast('.', "vssx").uppercase()} • ~${entry.approxSizeKb} KB • ${entry.vendor}",
                                            style = MaterialTheme.typography.labelSmall,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant
                                        )
                                    }
                                    FilledTonalButton(
                                        enabled = !isBusy,
                                        onClick = { viewModel.downloadOfficialStencilPack(entry) },
                                        contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp)
                                    ) {
                                        Text(
                                            when {
                                                isBusy -> "Installing…"
                                                isInstalled -> "Installed ✓"
                                                else -> "Install"
                                            },
                                            style = MaterialTheme.typography.labelSmall
                                        )
                                    }
                                }
                            }
                        }
                    }
                },
                confirmButton = {
                    FilledTonalButton(onClick = { viewModel.downloadAllOfficialStencilPacks() }) {
                        Text("Install All Packs")
                    }
                },
                dismissButton = {
                    TextButton(onClick = { viewModel.setOfficialStencilCatalogDialogVisible(false) }) {
                        Text("Close")
                    }
                }
            )
        }
    }
}

enum class VexioColorPickerTarget {
    TEXT_FG,
    TEXT_BG,
    SHAPE_FILL,
    SHAPE_LINE
}

private fun resolveComposeFontFamily(familyName: String): FontFamily {
    val norm = familyName.trim().lowercase(java.util.Locale.US)
    return when {
        "mono" in norm || "consolas" in norm || "courier" in norm || "code" in norm -> FontFamily.Monospace
        "serif" in norm && "sans" !in norm || "georgia" in norm || "times" in norm || "cambria" in norm -> FontFamily.Serif
        "cursive" in norm || "script" in norm -> FontFamily.Cursive
        else -> FontFamily.SansSerif
    }
}

private fun parseColorHexOrFallback(hex: String?, fallback: Color): Color {
    val clean = hex?.trim().orEmpty().removePrefix("#")
    if (clean.length == 6) {
        val rgb = clean.toLongOrNull(16)
        if (rgb != null) return Color(0xFF000000L or rgb)
    }
    return fallback
}

@Composable
private fun WorkstationHeaderBar(
    uiState: WorkstationUiState,
    posture: AdaptiveWorkstationPosture,
    onToggleBackstage: () -> Unit,
    onCreateBlankA4: () -> Unit,
    onRenameDocument: (String) -> Unit,
    onSelectLanguage: (VexioAppLanguage) -> Unit,
    onCheckUpdates: () -> Unit,
    onImportStencilClick: () -> Unit,
    onNewWindowClick: () -> Unit,
    onOpenClick: () -> Unit,
    onSaveClick: () -> Unit,
    onSaveAsClick: () -> Unit,
    onExportJpgClick: () -> Unit = {},
    onExportFormat: (String) -> Unit = {},
    onPrintClick: () -> Unit,
    onUndoClick: () -> Unit,
    onRedoClick: () -> Unit,
    onToggleTheme: () -> Unit,
    onToggleStencilDrawer: () -> Unit,
    onToggleShapeSheet: () -> Unit,
    onToggleInspector: () -> Unit
) {
    val lang = uiState.language
    // Shared Chrome-style hover-switching state across sibling top menus ("file", "export", "lang", "help")
    var openSiblingMenuId by remember { mutableStateOf<String?>(null) }
    var isEditingTitle by remember { mutableStateOf(false) }
    var titleFieldValue by remember(uiState.documentTitle) {
        mutableStateOf(
            TextFieldValue(
                text = uiState.documentTitle,
                selection = TextRange(0, uiState.documentTitle.length)
            )
        )
    }
    val titleFocusRequester = remember { FocusRequester() }

    LaunchedEffect(isEditingTitle) {
        if (isEditingTitle) {
            titleFieldValue = TextFieldValue(
                text = uiState.documentTitle,
                selection = TextRange(0, uiState.documentTitle.length)
            )
            runCatching { titleFocusRequester.requestFocus() }
        }
    }

    fun Modifier.hoverSwitchSiblingMenu(menuId: String): Modifier =
        this.pointerInput(menuId) {
            awaitPointerEventScope {
                while (true) {
                    val ev = awaitPointerEvent()
                    if (ev.type == PointerEventType.Enter &&
                        openSiblingMenuId != null &&
                        openSiblingMenuId != menuId
                    ) {
                        openSiblingMenuId = menuId
                    }
                }
            }
        }

    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        tonalElevation = 2.dp,
        modifier = Modifier.fillMaxWidth()
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 10.dp, vertical = 4.dp)
        ) {
            // App Badge (toggles In-App Start/File Backstage)
            Surface(
                shape = RoundedCornerShape(10.dp),
                color = MaterialTheme.colorScheme.primaryContainer,
                modifier = Modifier
                    .clip(RoundedCornerShape(10.dp))
                    .clickable(onClick = onToggleBackstage)
            ) {
                Text(
                    text = "VEXIO",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                    maxLines = 1,
                    softWrap = false,
                    modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp)
                )
            }
            Spacer(Modifier.width(6.dp))

            // File Hover-Switch Menu (Save & Save As… distinct!)
            Box(modifier = Modifier.hoverSwitchSiblingMenu("file")) {
                FilledTonalButton(
                    onClick = {
                        openSiblingMenuId = if (openSiblingMenuId == "file") null else "file"
                    },
                    contentPadding = PaddingValues(horizontal = 9.dp, vertical = 4.dp),
                    shape = RoundedCornerShape(10.dp)
                ) {
                    Text(
                        text = "${lang.fileLabel} ▾",
                        style = MaterialTheme.typography.labelMedium,
                        maxLines = 1,
                        softWrap = false
                    )
                }
                DropdownMenu(
                    expanded = openSiblingMenuId == "file",
                    onDismissRequest = { if (openSiblingMenuId == "file") openSiblingMenuId = null }
                ) {
                    DropdownMenuItem(
                        text = { Text("+ Blank ISO A4 (mm)") },
                        onClick = { openSiblingMenuId = null; onCreateBlankA4() }
                    )
                    DropdownMenuItem(
                        text = { Text(lang.openFileLabel) },
                        leadingIcon = { Icon(Icons.Default.FolderOpen, contentDescription = null) },
                        onClick = { openSiblingMenuId = null; onOpenClick() }
                    )
                    HorizontalDivider()
                    DropdownMenuItem(
                        text = { Text(lang.saveLabel) },
                        leadingIcon = { Icon(Icons.Default.Save, contentDescription = null) },
                        onClick = { openSiblingMenuId = null; onSaveClick() }
                    )
                    DropdownMenuItem(
                        text = { Text(lang.saveAsLabel) },
                        leadingIcon = { Icon(Icons.Default.Save, contentDescription = null) },
                        onClick = { openSiblingMenuId = null; onSaveAsClick() }
                    )
                    HorizontalDivider()
                    DropdownMenuItem(
                        text = { Text(lang.importStencilLabel) },
                        onClick = { openSiblingMenuId = null; onImportStencilClick() }
                    )
                    DropdownMenuItem(
                        text = { Text("Print / PDF (Ctrl+P)") },
                        leadingIcon = { Icon(Icons.Default.Print, contentDescription = null) },
                        onClick = { openSiblingMenuId = null; onPrintClick() }
                    )
                    DropdownMenuItem(
                        text = { Text("Start / Templates Hub…") },
                        onClick = { openSiblingMenuId = null; onToggleBackstage() }
                    )
                }
            }

            Spacer(Modifier.width(10.dp))
            Column(modifier = Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (isEditingTitle) {
                        Box(
                            modifier = Modifier
                                .clip(RoundedCornerShape(6.dp))
                                .background(MaterialTheme.colorScheme.surfaceContainerLowest)
                                .border(1.5.dp, MaterialTheme.colorScheme.primary, RoundedCornerShape(6.dp))
                                .padding(horizontal = 8.dp, vertical = 2.dp)
                        ) {
                            BasicTextField(
                                value = titleFieldValue,
                                onValueChange = { titleFieldValue = it },
                                singleLine = true,
                                textStyle = MaterialTheme.typography.titleSmall.copy(
                                    color = MaterialTheme.colorScheme.onSurface
                                ),
                                cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                                modifier = Modifier
                                    .width(220.dp)
                                    .focusRequester(titleFocusRequester)
                                    .onPreviewKeyEvent { ev ->
                                        if (ev.type == KeyEventType.KeyDown && ev.key == Key.Enter) {
                                            onRenameDocument(titleFieldValue.text)
                                            isEditingTitle = false
                                            true
                                        } else if (ev.type == KeyEventType.KeyDown && ev.key == Key.Escape) {
                                            isEditingTitle = false
                                            true
                                        } else {
                                            false
                                        }
                                    }
                            )
                        }
                        IconButton(
                            onClick = {
                                onRenameDocument(titleFieldValue.text)
                                isEditingTitle = false
                            },
                            modifier = Modifier.size(24.dp)
                        ) {
                            Icon(Icons.Default.Check, contentDescription = "Apply Filename", modifier = Modifier.size(15.dp))
                        }
                    } else {
                        Text(
                            text = buildString {
                                append(uiState.documentTitle)
                                if (uiState.isDirty) append(" • Edited")
                            },
                            style = MaterialTheme.typography.titleSmall,
                            maxLines = 1,
                            softWrap = false,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier
                                .clip(RoundedCornerShape(6.dp))
                                .combinedClickable(
                                    onClick = {},
                                    onDoubleClick = { isEditingTitle = true },
                                    onLongClick = { isEditingTitle = true }
                                )
                                .padding(horizontal = 4.dp, vertical = 1.dp)
                        )
                    }
                    Spacer(Modifier.width(6.dp))
                    Badge(
                        containerColor = MaterialTheme.colorScheme.secondaryContainer,
                        contentColor = MaterialTheme.colorScheme.onSecondaryContainer
                    ) {
                        Text(
                            text = "${uiState.documentExtension.uppercase()} • ${if (uiState.metricUnits) "MM" else "IN"}",
                            maxLines = 1,
                            softWrap = false,
                            modifier = Modifier.padding(horizontal = 4.dp, vertical = 1.dp)
                        )
                    }
                }
                if (posture == AdaptiveWorkstationPosture.ULTRAWIDE_DESKTOP) {
                    Text(
                        text = uiState.statusBanner,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        softWrap = false,
                        overflow = TextOverflow.Ellipsis
                    )
                }
            }

            // Undo / Redo / Open / Save / Save As / Print Quick Access
            IconButton(onClick = onUndoClick, enabled = uiState.canUndo, modifier = Modifier.size(34.dp)) {
                Icon(Icons.AutoMirrored.Filled.Undo, contentDescription = "Undo (Ctrl+Z)", modifier = Modifier.size(18.dp))
            }
            IconButton(onClick = onRedoClick, enabled = uiState.canRedo, modifier = Modifier.size(34.dp)) {
                Icon(Icons.AutoMirrored.Filled.Redo, contentDescription = "Redo (Ctrl+Y)", modifier = Modifier.size(18.dp))
            }
            IconButton(onClick = onOpenClick, modifier = Modifier.size(34.dp)) {
                Icon(Icons.Default.FolderOpen, contentDescription = "Open Diagram (Ctrl+O)", modifier = Modifier.size(18.dp))
            }
            IconButton(onClick = onSaveClick, modifier = Modifier.size(34.dp)) {
                Icon(Icons.Default.Save, contentDescription = lang.saveLabel, modifier = Modifier.size(18.dp))
            }
            FilledTonalButton(
                onClick = onSaveAsClick,
                contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                shape = RoundedCornerShape(10.dp)
            ) {
                Text(
                    text = lang.saveAsLabel.substringBefore(" ("),
                    style = MaterialTheme.typography.labelSmall,
                    maxLines = 1,
                    softWrap = false
                )
            }
            IconButton(onClick = onPrintClick, modifier = Modifier.size(34.dp)) {
                Icon(Icons.Default.Print, contentDescription = "Print Vector PDF (Ctrl+P)", modifier = Modifier.size(18.dp))
            }

            // Sibling Hover-to-Switch Menus (Chrome-style: hovering switches open menu seamlessly)
            Box(modifier = Modifier.hoverSwitchSiblingMenu("export")) {
                FilledTonalButton(
                    onClick = {
                        openSiblingMenuId = if (openSiblingMenuId == "export") null else "export"
                    },
                    contentPadding = PaddingValues(horizontal = 9.dp, vertical = 4.dp),
                    shape = RoundedCornerShape(10.dp)
                ) {
                    Text("Export ▾", style = MaterialTheme.typography.labelMedium, maxLines = 1, softWrap = false)
                }
                DropdownMenu(
                    expanded = openSiblingMenuId == "export",
                    onDismissRequest = { if (openSiblingMenuId == "export") openSiblingMenuId = null }
                ) {
                    DropdownMenuItem(
                        text = { Text("Export High-DPI JPEG (.jpg)") },
                        onClick = { openSiblingMenuId = null; onExportJpgClick() }
                    )
                    DropdownMenuItem(
                        text = { Text("Export Archival PDF/A (.pdf)") },
                        onClick = { openSiblingMenuId = null; onExportFormat("pdf") }
                    )
                    DropdownMenuItem(
                        text = { Text("Export Vector SVG (.svg)") },
                        onClick = { openSiblingMenuId = null; onExportFormat("svg") }
                    )
                    DropdownMenuItem(
                        text = { Text("Export AutoCAD DXF (.dxf)") },
                        onClick = { openSiblingMenuId = null; onExportFormat("dxf") }
                    )
                }
            }

            Spacer(Modifier.width(4.dp))

            Box(modifier = Modifier.hoverSwitchSiblingMenu("lang")) {
                FilledTonalButton(
                    onClick = {
                        openSiblingMenuId = if (openSiblingMenuId == "lang") null else "lang"
                    },
                    contentPadding = PaddingValues(horizontal = 9.dp, vertical = 4.dp),
                    shape = RoundedCornerShape(10.dp)
                ) {
                    Text("${uiState.language.badge} ▾", style = MaterialTheme.typography.labelMedium, maxLines = 1, softWrap = false)
                }
                DropdownMenu(
                    expanded = openSiblingMenuId == "lang",
                    onDismissRequest = { if (openSiblingMenuId == "lang") openSiblingMenuId = null }
                ) {
                    VexioAppLanguage.entries.forEach { l ->
                        DropdownMenuItem(
                            text = { Text("${l.badge} — ${l.nativeName}") },
                            leadingIcon = {
                                if (uiState.language == l) {
                                    Icon(Icons.Default.Check, contentDescription = null)
                                }
                            },
                            onClick = {
                                openSiblingMenuId = null
                                onSelectLanguage(l)
                            }
                        )
                    }
                }
            }

            Spacer(Modifier.width(4.dp))

            Box(modifier = Modifier.hoverSwitchSiblingMenu("help")) {
                FilledTonalButton(
                    onClick = {
                        openSiblingMenuId = if (openSiblingMenuId == "help") null else "help"
                    },
                    contentPadding = PaddingValues(horizontal = 9.dp, vertical = 4.dp),
                    shape = RoundedCornerShape(10.dp)
                ) {
                    Text("Updates ▾", style = MaterialTheme.typography.labelMedium, maxLines = 1, softWrap = false)
                }
                DropdownMenu(
                    expanded = openSiblingMenuId == "help",
                    onDismissRequest = { if (openSiblingMenuId == "help") openSiblingMenuId = null }
                ) {
                    DropdownMenuItem(
                        text = { Text(uiState.language.checkUpdatesLabel) },
                        onClick = { openSiblingMenuId = null; onCheckUpdates() }
                    )
                    DropdownMenuItem(
                        text = { Text(uiState.language.importStencilLabel) },
                        onClick = { openSiblingMenuId = null; onImportStencilClick() }
                    )
                    DropdownMenuItem(
                        text = { Text("Welcome / Start Hub") },
                        onClick = { openSiblingMenuId = null; onToggleBackstage() }
                    )
                }
            }

            IconButton(onClick = onToggleTheme, modifier = Modifier.size(34.dp)) {
                Icon(
                    imageVector = if (uiState.isDarkTheme) Icons.Default.LightMode else Icons.Default.DarkMode,
                    contentDescription = if (uiState.isDarkTheme) "Switch to Light Mode" else "Switch to Dark Mode",
                    modifier = Modifier.size(18.dp)
                )
            }

            if (posture != AdaptiveWorkstationPosture.COMPACT_CONVERTIBLE) {
                FilledTonalButton(
                    onClick = onNewWindowClick,
                    contentPadding = PaddingValues(horizontal = 10.dp, vertical = 5.dp),
                    shape = RoundedCornerShape(14.dp)
                ) {
                    Icon(Icons.Default.OpenInNew, contentDescription = null, modifier = Modifier.size(14.dp))
                    Spacer(Modifier.width(4.dp))
                    Text("New Window", style = MaterialTheme.typography.labelMedium, maxLines = 1, softWrap = false)
                }
            }

            Spacer(Modifier.width(4.dp))

            FilledTonalIconButton(onClick = onToggleStencilDrawer, modifier = Modifier.size(34.dp)) {
                Icon(Icons.Default.Schema, contentDescription = "Toggle Stencil Navigator", modifier = Modifier.size(18.dp))
            }
            FilledTonalIconButton(onClick = onToggleShapeSheet, modifier = Modifier.size(34.dp)) {
                Icon(Icons.Default.TableChart, contentDescription = "Toggle ShapeSheet Studio (Shift+F5)", modifier = Modifier.size(18.dp))
            }
            if (posture != AdaptiveWorkstationPosture.COMPACT_CONVERTIBLE) {
                FilledTonalIconButton(onClick = onToggleInspector, modifier = Modifier.size(34.dp)) {
                    Icon(Icons.Default.ViewSidebar, contentDescription = "Toggle Inspector Pane", modifier = Modifier.size(18.dp))
                }
            }
        }
    }
}

/**
 * Classic Collapsible Multi-Tab Visio Ribbon adapted to Material 3 Expressive:
 * File | Home | Insert | Design | Data | Process | Review | View | Developer
 */
@Composable
private fun ClassicVisioM3Ribbon(
    uiState: WorkstationUiState,
    viewModel: VexioSessionViewModel,
    onOpenBackstage: () -> Unit,
    onImportStencilClick: () -> Unit,
    onOpenColorPicker: (VexioColorPickerTarget) -> Unit,
    onTogglePlan2Strip: () -> Unit
) {
    val lang = uiState.language
    var fontMenuOpen by remember { mutableStateOf(false) }
    var fontSizeMenuOpen by remember { mutableStateOf(false) }
    var fontSearchQuery by remember { mutableStateOf("") }
    var pageSizeMenuOpen by remember { mutableStateOf(false) }

    val tabs = listOf(
        "Home" to lang.homeLabel,
        "Insert" to lang.insertLabel,
        "Design" to lang.designLabel,
        "Data" to lang.dataLabel,
        "Process" to lang.processLabel,
        "Review" to lang.reviewLabel,
        "View" to lang.viewLabel,
        "Developer" to lang.developerLabel
    )

    Surface(
        color = MaterialTheme.colorScheme.surfaceContainer,
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            // Ribbon Tab Header Row (Zero Truncation across all 6 languages)
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState())
                    .padding(horizontal = 10.dp, vertical = 3.dp)
            ) {
                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier
                        .clip(RoundedCornerShape(8.dp))
                        .clickable(onClick = onOpenBackstage)
                ) {
                    Text(
                        text = lang.fileLabel,
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.onPrimary,
                        maxLines = 1,
                        softWrap = false,
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp)
                    )
                }

                tabs.forEach { (tabId, label) ->
                    val selected = uiState.activeRibbonTab == tabId && !uiState.isRibbonCollapsed
                    Surface(
                        shape = RoundedCornerShape(8.dp),
                        color = if (selected) MaterialTheme.colorScheme.secondaryContainer else Color.Transparent,
                        modifier = Modifier
                            .clip(RoundedCornerShape(8.dp))
                            .clickable { viewModel.selectRibbonTab(tabId) }
                    ) {
                        Text(
                            text = label,
                            style = MaterialTheme.typography.labelLarge,
                            maxLines = 1,
                            softWrap = false,
                            color = if (selected) {
                                MaterialTheme.colorScheme.onSecondaryContainer
                            } else {
                                MaterialTheme.colorScheme.onSurfaceVariant
                            },
                            modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp)
                        )
                    }
                }

                Spacer(Modifier.width(8.dp))
                IconButton(
                    onClick = viewModel::toggleRibbonCollapsed,
                    modifier = Modifier.size(26.dp)
                ) {
                    Icon(
                        imageVector = if (uiState.isRibbonCollapsed) Icons.Default.ExpandMore else Icons.Default.ExpandLess,
                        contentDescription = "Collapse / Expand Ribbon"
                    )
                }
            }

            // Collapsible Active Ribbon Tab Body
            androidx.compose.animation.AnimatedVisibility(
                visible = !uiState.isRibbonCollapsed,
                enter = expandVertically(VexioExpressiveMotion.spatialSpring()) + fadeIn(),
                exit = shrinkVertically(VexioExpressiveMotion.spatialSpring()) + fadeOut()
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState())
                        .padding(horizontal = 12.dp, vertical = 5.dp)
                ) {
                    when (uiState.activeRibbonTab) {
                        "Home" -> {
                            // Installed OS Fonts Selector (Searchable + Rendered in Native FontFamily)
                            Box {
                                ExpressivePlan2ActionPill(
                                    label = "${uiState.textFontFamily} ▾",
                                    selected = false,
                                    onClick = {
                                        fontSearchQuery = ""
                                        fontMenuOpen = true
                                    }
                                )
                                DropdownMenu(
                                    expanded = fontMenuOpen,
                                    onDismissRequest = { fontMenuOpen = false },
                                    modifier = Modifier
                                        .width(248.dp)
                                        .heightIn(max = 360.dp)
                                ) {
                                    Box(modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp)) {
                                        OutlinedTextField(
                                            value = fontSearchQuery,
                                            onValueChange = { fontSearchQuery = it },
                                            singleLine = true,
                                            placeholder = {
                                                Text(
                                                    "Filter ${uiState.availableFontFamilies.size} OS fonts…",
                                                    style = MaterialTheme.typography.labelSmall
                                                )
                                            },
                                            textStyle = MaterialTheme.typography.bodySmall,
                                            modifier = Modifier.fillMaxWidth()
                                        )
                                    }
                                    val filteredFonts = uiState.availableFontFamilies.filter {
                                        fontSearchQuery.isBlank() || it.contains(fontSearchQuery.trim(), ignoreCase = true)
                                    }
                                    filteredFonts.forEach { f ->
                                        DropdownMenuItem(
                                            text = {
                                                Text(
                                                    text = f,
                                                    fontFamily = resolveComposeFontFamily(f),
                                                    maxLines = 1,
                                                    softWrap = false
                                                )
                                            },
                                            leadingIcon = {
                                                if (f.equals(uiState.textFontFamily, ignoreCase = true)) {
                                                    Icon(Icons.Default.Check, contentDescription = null)
                                                }
                                            },
                                            onClick = {
                                                fontMenuOpen = false
                                                viewModel.applyRichTextStyle(fontFamily = f)
                                            }
                                        )
                                    }
                                }
                            }
                            Box {
                                ExpressivePlan2ActionPill(
                                    label = "${uiState.textFontSizePt} pt ▾",
                                    selected = false,
                                    onClick = { fontSizeMenuOpen = true }
                                )
                                DropdownMenu(
                                    expanded = fontSizeMenuOpen,
                                    onDismissRequest = { fontSizeMenuOpen = false }
                                ) {
                                    listOf(8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72).forEach { pt ->
                                        DropdownMenuItem(
                                            text = { Text("$pt pt") },
                                            leadingIcon = {
                                                if (uiState.textFontSizePt == pt) {
                                                    Icon(Icons.Default.Check, contentDescription = null)
                                                }
                                            },
                                            onClick = {
                                                fontSizeMenuOpen = false
                                                viewModel.applyRichTextStyle(fontSizePt = pt)
                                            }
                                        )
                                    }
                                }
                            }
                            ExpressivePlan2ActionPill(
                                label = "A▲",
                                selected = false,
                                onClick = { viewModel.applyRichTextStyle(fontSizePt = uiState.textFontSizePt + 2) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "A▼",
                                selected = false,
                                onClick = { viewModel.applyRichTextStyle(fontSizePt = (uiState.textFontSizePt - 2).coerceAtLeast(6)) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "B",
                                selected = uiState.textBold,
                                onClick = { viewModel.applyRichTextStyle(bold = !uiState.textBold) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "I",
                                selected = uiState.textItalic,
                                onClick = { viewModel.applyRichTextStyle(italic = !uiState.textItalic) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "U̲",
                                selected = uiState.textUnderline,
                                onClick = { viewModel.applyRichTextStyle(underline = !uiState.textUnderline) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "S̶",
                                selected = uiState.textStrikethrough,
                                onClick = { viewModel.applyRichTextStyle(strikethrough = !uiState.textStrikethrough) }
                            )

                            // Full Color Picker Chips (Text Color, Highlight + Clear, Fill Color, Line Color)
                            ColorSwatchActionPill(
                                label = lang.textColorLabel,
                                swatchColor = parseColorHexOrFallback(uiState.textColorHex, Color(0xFF0F172A)),
                                isTransparent = false,
                                onClick = { onOpenColorPicker(VexioColorPickerTarget.TEXT_FG) }
                            )
                            ColorSwatchActionPill(
                                label = lang.highlightColorLabel,
                                swatchColor = parseColorHexOrFallback(uiState.textBgColorHex, Color(0xFFFEF08A)),
                                isTransparent = uiState.textBgColorHex == "none",
                                onClick = { onOpenColorPicker(VexioColorPickerTarget.TEXT_BG) }
                            )
                            if (uiState.textBgColorHex != "none") {
                                ExpressivePlan2ActionPill(
                                    label = "✕",
                                    selected = false,
                                    onClick = { viewModel.applyRichTextStyle(bgColorHex = "none") }
                                )
                            }
                            ColorSwatchActionPill(
                                label = lang.fillColorLabel,
                                swatchColor = parseColorHexOrFallback(uiState.shapeFillColorHex, Color(0xFFE8F0FE)),
                                isTransparent = false,
                                onClick = { onOpenColorPicker(VexioColorPickerTarget.SHAPE_FILL) }
                            )
                            ColorSwatchActionPill(
                                label = lang.lineColorLabel,
                                swatchColor = parseColorHexOrFallback(uiState.shapeLineColorHex, Color(0xFF1A73E8)),
                                isTransparent = false,
                                onClick = { onOpenColorPicker(VexioColorPickerTarget.SHAPE_LINE) }
                            )

                            ExpressivePlan2ActionPill(
                                label = "⇤ L",
                                selected = uiState.textHorizAlign == 0,
                                onClick = { viewModel.applyRichTextStyle(horizAlign = 0) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "⇔ C",
                                selected = uiState.textHorizAlign == 1,
                                onClick = { viewModel.applyRichTextStyle(horizAlign = 1) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "⇥ R",
                                selected = uiState.textHorizAlign == 2,
                                onClick = { viewModel.applyRichTextStyle(horizAlign = 2) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "• List",
                                selected = uiState.textBullets,
                                onClick = { viewModel.applyRichTextStyle(bullets = !uiState.textBullets) }
                            )

                            VerticalDivider(modifier = Modifier.height(20.dp), color = MaterialTheme.colorScheme.outlineVariant)

                            // Arrange, Align, Distribute, Group, Rotate 90° & Flip for All Stencils
                            var alignMenuOpen by remember { mutableStateOf(false) }
                            var distributeMenuOpen by remember { mutableStateOf(false) }
                            Box {
                                ExpressivePlan2ActionPill(
                                    label = "Align ▾",
                                    selected = false,
                                    onClick = { alignMenuOpen = true }
                                )
                                DropdownMenu(
                                    expanded = alignMenuOpen,
                                    onDismissRequest = { alignMenuOpen = false }
                                ) {
                                    listOf(
                                        "left" to "Align Left",
                                        "center" to "Align Center",
                                        "right" to "Align Right",
                                        "top" to "Align Top",
                                        "middle" to "Align Middle",
                                        "bottom" to "Align Bottom"
                                    ).forEach { (modeSlug, title) ->
                                        DropdownMenuItem(
                                            text = { Text(title) },
                                            onClick = {
                                                alignMenuOpen = false
                                                viewModel.alignSelectedShapes(modeSlug)
                                            }
                                        )
                                    }
                                }
                            }
                            Box {
                                ExpressivePlan2ActionPill(
                                    label = "Distribute ▾",
                                    selected = false,
                                    onClick = { distributeMenuOpen = true }
                                )
                                DropdownMenu(
                                    expanded = distributeMenuOpen,
                                    onDismissRequest = { distributeMenuOpen = false }
                                ) {
                                    DropdownMenuItem(
                                        text = { Text("Distribute Horizontally") },
                                        onClick = {
                                            distributeMenuOpen = false
                                            viewModel.distributeSelectedShapes("horizontal")
                                        }
                                    )
                                    DropdownMenuItem(
                                        text = { Text("Distribute Vertically") },
                                        onClick = {
                                            distributeMenuOpen = false
                                            viewModel.distributeSelectedShapes("vertical")
                                        }
                                    )
                                }
                            }
                            ExpressivePlan2ActionPill(
                                label = "Group",
                                selected = false,
                                onClick = { viewModel.groupSelectedShapes() }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Ungroup",
                                selected = false,
                                onClick = { viewModel.ungroupSelectedShapes() }
                            )
                            ExpressivePlan2ActionPill(
                                label = "↺ 90°",
                                selected = false,
                                onClick = { viewModel.rotateOrFlipSelected(deltaAngleDeg = -90.0) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "↻ 90°",
                                selected = false,
                                onClick = { viewModel.rotateOrFlipSelected(deltaAngleDeg = 90.0) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "⇆ Flip H",
                                selected = false,
                                onClick = { viewModel.rotateOrFlipSelected(flipX = true) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "⇅ Flip V",
                                selected = false,
                                onClick = { viewModel.rotateOrFlipSelected(flipY = true) }
                            )
                        }

                        "Insert" -> {
                            ExpressivePlan2ActionPill(
                                label = "+ New Page",
                                selected = false,
                                onClick = viewModel::addNewPage
                            )
                            ExpressivePlan2ActionPill(
                                label = lang.importStencilLabel,
                                selected = true,
                                onClick = onImportStencilClick
                            )
                            ExpressivePlan2ActionPill(
                                label = "Text Box (Ctrl+2)",
                                selected = uiState.activeTool == VexioTool.TEXT_BLOCK,
                                onClick = { viewModel.selectTool(VexioTool.TEXT_BLOCK) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "+ Container",
                                selected = false,
                                onClick = { viewModel.insertContainer() }
                            )
                            ExpressivePlan2ActionPill(
                                label = "+ Swimlanes",
                                selected = false,
                                onClick = { viewModel.insertSwimlanes() }
                            )
                            ExpressivePlan2ActionPill(
                                label = "+ Callout",
                                selected = false,
                                onClick = { viewModel.attachCallout() }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Subprocess (+)",
                                selected = false,
                                onClick = { viewModel.createOrLinkSubprocess() }
                            )
                        }

                        "Design" -> {
                            Box {
                                ExpressivePlan2ActionPill(
                                    label = "Page Size: ${uiState.activePagePreset.label} ▾",
                                    selected = true,
                                    onClick = { pageSizeMenuOpen = true }
                                )
                                DropdownMenu(expanded = pageSizeMenuOpen, onDismissRequest = { pageSizeMenuOpen = false }) {
                                    VexioPagePresetSpec.entries.forEach { preset ->
                                        DropdownMenuItem(
                                            text = { Text(preset.label) },
                                            leadingIcon = {
                                                if (uiState.activePagePreset == preset) {
                                                    Icon(Icons.Default.Check, contentDescription = null)
                                                }
                                            },
                                            onClick = {
                                                pageSizeMenuOpen = false
                                                viewModel.applyPagePreset(preset)
                                            }
                                        )
                                    }
                                }
                            }
                            VexioThemePreset.entries.forEach { preset ->
                                ExpressivePlan2ActionPill(
                                    label = preset.displayName,
                                    selected = uiState.activeThemeSlug == preset.slug,
                                    onClick = { viewModel.applyDocumentTheme(preset.slug, uiState.activeThemeVariant) }
                                )
                            }
                            ExpressivePlan2ActionPill(
                                label = "Auto-Layout",
                                selected = false,
                                onClick = { viewModel.autoLayoutActivePage() }
                            )
                        }

                        "Data" -> {
                            ExpressivePlan2ActionPill(
                                label = "Data Visualizer (.csv/.xlsx)",
                                selected = uiState.lastDataVisualizerShapeCount > 0,
                                onClick = {
                                    viewModel.importDataVisualizer(
                                        Plan2SamplePayloads.dataVisualizerCsv.toByteArray(Charsets.UTF_8)
                                    )
                                }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Cloud Discovery",
                                selected = uiState.lastCloudDiscoveryShapeCount > 0,
                                onClick = { viewModel.importCloudDiscovery(Plan2SamplePayloads.cloudDiscoveryJson) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Entra / LDIF Org Chart",
                                selected = uiState.lastOrgChartShapeCount > 0,
                                onClick = { viewModel.importEntraOrLdifOrgChart(Plan2SamplePayloads.entraOrgChartLdif) }
                            )
                        }

                        "Process" -> {
                            ExpressivePlan2ActionPill(
                                label = "Validate BPMN 2.0 ✓",
                                selected = uiState.validationSummary.standard == "BPMN 2.0",
                                onClick = { viewModel.validateActiveDiagram("bpmn") }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Validate UML 2.5 ✓",
                                selected = uiState.validationSummary.standard == "UML 2.5",
                                onClick = { viewModel.validateActiveDiagram("uml") }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Create Subprocess Drill-Down",
                                selected = false,
                                onClick = { viewModel.createOrLinkSubprocess() }
                            )
                        }

                        "Review" -> {
                            ExpressivePlan2ActionPill(
                                label = "Markup Pen",
                                selected = uiState.activeTool == VexioTool.STYLUS_INK,
                                onClick = { viewModel.selectTool(VexioTool.STYLUS_INK) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Highlighter",
                                selected = uiState.activeTool == VexioTool.HIGHLIGHTER,
                                onClick = { viewModel.selectTool(VexioTool.HIGHLIGHTER) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Diagram Compare (Visual Diff)",
                                selected = uiState.lastCompareSummary != null,
                                onClick = { viewModel.compareWithBaseline() }
                            )
                        }

                        "View" -> {
                            ExpressivePlan2ActionPill(
                                label = "Smart Guides",
                                selected = uiState.smartGuidesEnabled,
                                onClick = { viewModel.toggleSmartGuides() }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Stencils Navigator",
                                selected = uiState.isStencilDrawerExpanded,
                                onClick = viewModel::toggleStencilDrawer
                            )
                            ExpressivePlan2ActionPill(
                                label = "Inspector Pane",
                                selected = uiState.isInspectorExpanded,
                                onClick = viewModel::toggleInspectorPane
                            )
                            ExpressivePlan2ActionPill(
                                label = "ShapeSheet (Shift+F5)",
                                selected = uiState.isShapeSheetDrawerExpanded,
                                onClick = viewModel::toggleShapeSheetDrawer
                            )
                            ExpressivePlan2ActionPill(
                                label = "100% Zoom",
                                selected = uiState.zoomPercent == 100,
                                onClick = { viewModel.setZoomPercent(100) }
                            )
                        }

                        "Developer" -> {
                            ExpressivePlan2ActionPill(
                                label = "ShapeSheet Studio",
                                selected = uiState.isShapeSheetDrawerExpanded,
                                onClick = viewModel::toggleShapeSheetDrawer
                            )
                            ExpressivePlan2ActionPill(
                                label = "Plan 2 Strip",
                                selected = uiState.isPlan2RibbonExpanded,
                                onClick = onTogglePlan2Strip
                            )
                            VexioBooleanOp.entries.forEach { op ->
                                ExpressivePlan2ActionPill(
                                    label = op.displayName,
                                    selected = uiState.lastBooleanOp == op.slug,
                                    onClick = { viewModel.performBooleanOp(op) }
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ColorSwatchActionPill(
    label: String,
    swatchColor: Color,
    isTransparent: Boolean,
    onClick: () -> Unit
) {
    Surface(
        shape = RoundedCornerShape(10.dp),
        color = MaterialTheme.colorScheme.surfaceContainerLowest,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
        modifier = Modifier
            .clip(RoundedCornerShape(10.dp))
            .clickable(onClick = onClick)
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(5.dp),
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 5.dp)
        ) {
            Box(
                modifier = Modifier
                    .size(14.dp)
                    .clip(CircleShape)
                    .background(if (isTransparent) Color.Transparent else swatchColor)
                    .border(1.dp, MaterialTheme.colorScheme.outline, CircleShape)
            )
            Text(
                text = label,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                softWrap = false
            )
        }
    }
}

@Composable
private fun InAppStartBackstageOverlay(
    uiState: WorkstationUiState,
    onClose: () -> Unit,
    onCreateBlankPreset: (String, VexioPagePresetSpec) -> Unit,
    onLoadTemplate: (String) -> Unit,
    onOpenFile: () -> Unit,
    onImportStencil: () -> Unit,
    onSaveFile: () -> Unit,
    onSaveAsFile: () -> Unit,
    onPrint: () -> Unit,
    onSelectLanguage: (VexioAppLanguage) -> Unit,
    onToggleAutoUpdates: (Boolean) -> Unit,
    onCheckUpdates: () -> Unit
) {
    val lang = uiState.language

    Surface(
        color = MaterialTheme.colorScheme.background,
        modifier = Modifier.fillMaxSize()
    ) {
        Row(modifier = Modifier.fillMaxSize()) {
            // Widened Left Backstage Rail (316.dp — zero truncation across EN, DE, IT, FR, ES, GR)
            Surface(
                color = MaterialTheme.colorScheme.surfaceContainerHigh,
                modifier = Modifier
                    .width(316.dp)
                    .fillMaxHeight()
            ) {
                Column(
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier
                        .fillMaxSize()
                        .verticalScroll(rememberScrollState())
                        .padding(16.dp)
                ) {
                    FilledTonalButton(
                        onClick = onClose,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Text("← ${lang.continueCanvasLabel}", maxLines = 1, softWrap = false)
                    }
                    Spacer(Modifier.height(6.dp))
                    Text("VEXIO PROFESSIONAL", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                    Text("1:1 Visio & Offline CAD Suite", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    HorizontalDivider()
                    FilledTonalButton(
                        onClick = { onCreateBlankPreset("Drawing_A4.vsdx", VexioPagePresetSpec.A4_LANDSCAPE) },
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Text("+ Blank ISO A4 (mm)", maxLines = 1, softWrap = false)
                    }
                    FilledTonalButton(onClick = onOpenFile, modifier = Modifier.fillMaxWidth()) {
                        Text(lang.openFileLabel, maxLines = 1, softWrap = false)
                    }
                    FilledTonalButton(onClick = onImportStencil, modifier = Modifier.fillMaxWidth()) {
                        Text(lang.importStencilLabel, maxLines = 1, softWrap = false)
                    }
                    HorizontalDivider()
                    FilledTonalButton(onClick = onSaveFile, modifier = Modifier.fillMaxWidth()) {
                        Text(lang.saveLabel, maxLines = 1, softWrap = false)
                    }
                    FilledTonalButton(onClick = onSaveAsFile, modifier = Modifier.fillMaxWidth()) {
                        Text(lang.saveAsLabel, maxLines = 1, softWrap = false)
                    }
                    TextButton(onClick = onPrint, modifier = Modifier.fillMaxWidth()) {
                        Text("Print / PDF (Ctrl+P)", maxLines = 1, softWrap = false)
                    }
                    HorizontalDivider()
                    FilledTonalButton(onClick = onCheckUpdates, modifier = Modifier.fillMaxWidth()) {
                        Text(lang.checkUpdatesLabel, maxLines = 1, softWrap = false)
                    }
                }
            }

            // Main Start / Templates / Preferences Content
            Column(
                verticalArrangement = Arrangement.spacedBy(18.dp),
                modifier = Modifier
                    .weight(1f)
                    .fillMaxHeight()
                    .verticalScroll(rememberScrollState())
                    .padding(24.dp)
            ) {
                Text(lang.startHeroTitle, style = MaterialTheme.typography.headlineSmall)
                Text(
                    "Create a blank ISO Metric (A4 / A3) or US Letter page, start from a 1:1 Visio template, or open/import Visio (.vsdx, .vsd, .vssx, .vss) and CAD (.dwg, .dxf) files directly inside the app.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )

                Text("Blank Page Presets (Metric Default / OS Standard)", style = MaterialTheme.typography.titleMedium)
                Row(
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    modifier = Modifier.horizontalScroll(rememberScrollState())
                ) {
                    listOf(
                        Triple("Blank ISO A4 Landscape", "297 × 210 mm • Metric", VexioPagePresetSpec.A4_LANDSCAPE),
                        Triple("Blank ISO A3 Landscape", "420 × 297 mm • Metric", VexioPagePresetSpec.A3_LANDSCAPE),
                        Triple("Blank ISO A4 Portrait", "210 × 297 mm • Metric", VexioPagePresetSpec.A4_PORTRAIT),
                        Triple("Blank US Letter", "11 × 8.5 in • Imperial", VexioPagePresetSpec.LETTER_LANDSCAPE)
                    ).forEach { (cardTitle, subtitle, preset) ->
                        Surface(
                            shape = RoundedCornerShape(16.dp),
                            color = MaterialTheme.colorScheme.surfaceContainerLow,
                            border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                            modifier = Modifier
                                .width(210.dp)
                                .clip(RoundedCornerShape(16.dp))
                                .clickable { onCreateBlankPreset("${preset.slug.uppercase()}_Diagram.vsdx", preset) }
                        ) {
                            Column(modifier = Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                                Text(cardTitle, style = MaterialTheme.typography.titleSmall)
                                Text(subtitle, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }
                }

                Text("Starter Diagram Templates", style = MaterialTheme.typography.titleMedium)
                Row(
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    modifier = Modifier.horizontalScroll(rememberScrollState())
                ) {
                    listOf(
                        "Flowchart Starter" to "flowchart",
                        "BPMN 2.0 Process" to "bpmn",
                        "Network & Rack Topology" to "network",
                        "CAD Floor Plan" to "floorplan",
                        "Organization Chart" to "orgchart",
                        "UML 2.5 Architecture" to "uml"
                    ).forEach { (title, slug) ->
                        Surface(
                            shape = RoundedCornerShape(14.dp),
                            color = MaterialTheme.colorScheme.surfaceContainer,
                            border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                            modifier = Modifier
                                .clip(RoundedCornerShape(14.dp))
                                .clickable { onLoadTemplate(slug) }
                        ) {
                            Text(
                                text = title,
                                style = MaterialTheme.typography.labelLarge,
                                modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)
                            )
                        }
                    }
                }

                Surface(
                    shape = RoundedCornerShape(16.dp),
                    color = MaterialTheme.colorScheme.surfaceContainerLow,
                    border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Column(
                        verticalArrangement = Arrangement.spacedBy(10.dp),
                        modifier = Modifier.padding(16.dp)
                    ) {
                        Text("Language (EN, DE, IT, FR, ES, GR) & Updates", style = MaterialTheme.typography.titleSmall)
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                            modifier = Modifier.horizontalScroll(rememberScrollState())
                        ) {
                            VexioAppLanguage.entries.forEach { l ->
                                ExpressivePlan2ActionPill(
                                    label = "${l.badge} • ${l.nativeName}",
                                    selected = uiState.language == l,
                                    onClick = { onSelectLanguage(l) }
                                )
                            }
                        }
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Checkbox(
                                checked = uiState.autoCheckUpdates,
                                onCheckedChange = onToggleAutoUpdates
                            )
                            Text("Automatically check for updates on startup", style = MaterialTheme.typography.bodySmall)
                            Spacer(Modifier.width(12.dp))
                            Text(
                                text = uiState.updateStatusMessage,
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    }
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
 * Collapsible 22-Category Visio Professional Stencil Navigator with live search,
 * category filter chips, crisp multi-geometry SVG previews, cross-window Drag & Drop,
 * and stencil context menus.
 */
@Composable
private fun StencilMasterDrawerPane(
    uiState: WorkstationUiState,
    posture: AdaptiveWorkstationPosture,
    onSelectCategory: (String) -> Unit,
    onMasterClicked: (StencilMasterItem) -> Unit,
    onImportStencilRequested: () -> Unit = {},
    onOpenOfficialCatalogRequested: () -> Unit = {},
    onOpenNewWindowRequested: () -> Unit
) {
    var searchQuery by remember { mutableStateOf("") }
    var expandedCategories by remember {
        mutableStateOf(
            setOf(
                "Basic Shapes",
                "Basic Flowchart",
                "Arrows & Connectors",
                "BPMN 2.0",
                "UML 2.5"
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
                    text = "Stencils (${uiState.stencilMasters.size} • ${uiState.stencilCategories.size} Packs)",
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

            // + Import Visio Stencil (.vssx / .vss / .zip) + Official Stencil Packs Buttons
            Row(
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 10.dp, vertical = 2.dp)
            ) {
                FilledTonalButton(
                    onClick = onImportStencilRequested,
                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 6.dp),
                    shape = RoundedCornerShape(10.dp),
                    colors = ButtonDefaults.filledTonalButtonColors(
                        containerColor = MaterialTheme.colorScheme.secondaryContainer
                    ),
                    modifier = Modifier.weight(1f)
                ) {
                    Icon(
                        imageVector = Icons.Default.FolderOpen,
                        contentDescription = null,
                        modifier = Modifier.size(14.dp)
                    )
                    Spacer(Modifier.width(4.dp))
                    Text(
                        text = "+ Stencil",
                        style = MaterialTheme.typography.labelSmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
                FilledTonalButton(
                    onClick = onOpenOfficialCatalogRequested,
                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 6.dp),
                    shape = RoundedCornerShape(10.dp),
                    colors = ButtonDefaults.filledTonalButtonColors(
                        containerColor = MaterialTheme.colorScheme.primaryContainer
                    ),
                    modifier = Modifier.weight(1f)
                ) {
                    Icon(
                        imageVector = Icons.Default.Add,
                        contentDescription = null,
                        modifier = Modifier.size(14.dp)
                    )
                    Spacer(Modifier.width(4.dp))
                    Text(
                        text = "Official Packs",
                        style = MaterialTheme.typography.labelSmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
            }

            Spacer(Modifier.height(6.dp))

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

            // Horizontal Category Filter Pills (All + 22 Packs)
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

            // 22-Category Accordion List
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

private data class ParsedComposeSvgDoc(
    val viewBoxW: Float,
    val viewBoxH: Float,
    val shapes: List<ParsedComposeSvgPath>
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

private fun parseSvgPointsToPath(pointsStr: String, closePoly: Boolean): Path? {
    val nums = pointsStr.trim().split(Regex("[\\s,]+")).mapNotNull { it.toFloatOrNull() }
    if (nums.size < 4) return null
    return Path().apply {
        moveTo(nums[0], nums[1])
        var idx = 2
        while (idx + 1 < nums.size) {
            lineTo(nums[idx], nums[idx + 1])
            idx += 2
        }
        if (closePoly) close()
    }
}

private fun parseStencilSvgDoc(svg: String, defaultAccent: Color): ParsedComposeSvgDoc {
    if (svg.isBlank()) return ParsedComposeSvgDoc(144f, 96f, emptyList())

    var viewW = 144f
    var viewH = 96f
    val svgTagEnd = svg.indexOf('>')
    if (svgTagEnd > 0) {
        val header = svg.substring(0, svgTagEnd)
        val vb = extractSvgAttr("viewBox", header)
        if (!vb.isNullOrBlank()) {
            val parts = vb.trim().split(Regex("[\\s,]+")).mapNotNull { it.toFloatOrNull() }
            if (parts.size == 4 && parts[2] > 1f && parts[3] > 1f) {
                viewW = parts[2]
                viewH = parts[3]
            }
        }
    }

    val groupIdx = svg.indexOf("data-shape-id=")
    val defsEndIdx = svg.indexOf("</defs>")
    val startOffset = when {
        groupIdx >= 0 -> groupIdx
        defsEndIdx >= 0 -> defsEndIdx + 7
        svgTagEnd >= 0 -> svgTagEnd + 1
        else -> 0
    }
    val body = svg.substring(startOffset)
    val list = mutableListOf<ParsedComposeSvgPath>()
    val tagRegex = Regex("""<(path|polygon|polyline|rect|circle|ellipse|line)\b([^>]*)/?>""")

    for (m in tagRegex.findAll(body)) {
        val elemName = m.groupValues[1]
        val attrs = m.groupValues[2]
        val fillRaw = extractSvgAttr("fill", attrs)
        val strokeRaw = extractSvgAttr("stroke", attrs)

        if (elemName == "rect" && groupIdx < 0 &&
            fillRaw?.equals("#ffffff", ignoreCase = true) == true &&
            strokeRaw == null
        ) {
            continue
        }

        val parsedPath: Path? = when (elemName) {
            "path" -> {
                val d = extractSvgAttr("d", attrs)
                if (!d.isNullOrBlank()) {
                    runCatching { PathParser().parsePathString(d).toPath() }.getOrNull()
                } else {
                    null
                }
            }
            "polygon" -> extractSvgAttr("points", attrs)?.let { parseSvgPointsToPath(it, closePoly = true) }
            "polyline" -> extractSvgAttr("points", attrs)?.let { parseSvgPointsToPath(it, closePoly = false) }
            "rect" -> {
                val rx = extractSvgAttr("x", attrs)?.toFloatOrNull() ?: 0f
                val ry = extractSvgAttr("y", attrs)?.toFloatOrNull() ?: 0f
                val rw = extractSvgAttr("width", attrs)?.toFloatOrNull() ?: 0f
                val rh = extractSvgAttr("height", attrs)?.toFloatOrNull() ?: 0f
                if (rw > 0.1f && rh > 0.1f) {
                    Path().apply {
                        addRect(androidx.compose.ui.geometry.Rect(rx, ry, rx + rw, ry + rh))
                    }
                } else {
                    null
                }
            }
            "circle" -> {
                val cx = extractSvgAttr("cx", attrs)?.toFloatOrNull() ?: 0f
                val cy = extractSvgAttr("cy", attrs)?.toFloatOrNull() ?: 0f
                val r = extractSvgAttr("r", attrs)?.toFloatOrNull() ?: 0f
                if (r > 0.1f) {
                    Path().apply {
                        addOval(androidx.compose.ui.geometry.Rect(cx - r, cy - r, cx + r, cy + r))
                    }
                } else {
                    null
                }
            }
            "ellipse" -> {
                val cx = extractSvgAttr("cx", attrs)?.toFloatOrNull() ?: 0f
                val cy = extractSvgAttr("cy", attrs)?.toFloatOrNull() ?: 0f
                val rx = extractSvgAttr("rx", attrs)?.toFloatOrNull() ?: 0f
                val ry = extractSvgAttr("ry", attrs)?.toFloatOrNull() ?: 0f
                if (rx > 0.1f && ry > 0.1f) {
                    Path().apply {
                        addOval(androidx.compose.ui.geometry.Rect(cx - rx, cy - ry, cx + rx, cy + ry))
                    }
                } else {
                    null
                }
            }
            "line" -> {
                val x1 = extractSvgAttr("x1", attrs)?.toFloatOrNull() ?: 0f
                val y1 = extractSvgAttr("y1", attrs)?.toFloatOrNull() ?: 0f
                val x2 = extractSvgAttr("x2", attrs)?.toFloatOrNull() ?: 0f
                val y2 = extractSvgAttr("y2", attrs)?.toFloatOrNull() ?: 0f
                Path().apply {
                    moveTo(x1, y1)
                    lineTo(x2, y2)
                }
            }
            else -> null
        }

        if (parsedPath != null) {
            val fill = if (elemName == "line" || elemName == "polyline") {
                parseSvgHexColor(fillRaw ?: "none", defaultAccent)
            } else {
                parseSvgHexColor(fillRaw, defaultAccent)
            }
            val stroke = parseSvgHexColor(strokeRaw, defaultAccent)
            val sw = extractSvgAttr("stroke-width", attrs)?.toFloatOrNull() ?: 1.3f
            list.add(ParsedComposeSvgPath(parsedPath, fill, stroke, sw))
        }
    }
    return ParsedComposeSvgDoc(viewW, viewH, list)
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
    val decodedPngPreview = remember(master.pngDataUrl) {
        val prefix = "data:image/png;base64,"
        if (master.pngDataUrl.startsWith(prefix)) {
            runCatching {
                val raw = Base64.decode(master.pngDataUrl.removePrefix(prefix), Base64.DEFAULT)
                BitmapFactory.decodeByteArray(raw, 0, raw.size)?.asImageBitmap()
            }.getOrNull()
        } else {
            null
        }
    }
    val parsedSvgDoc = remember(master.svgPreview, primaryStroke) {
        parseStencilSvgDoc(master.svgPreview, primaryStroke)
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
                    if (decodedPngPreview != null) {
                        Image(
                            bitmap = decodedPngPreview,
                            contentDescription = null,
                            contentScale = ContentScale.Fit,
                            modifier = Modifier
                                .fillMaxSize()
                                .padding(3.dp)
                        )
                    } else {
                    val fillTint = primaryStroke.copy(alpha = 0.14f)
                    androidx.compose.foundation.Canvas(
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(3.dp)
                    ) {
                        val w = size.width
                        val h = size.height
                        if (parsedSvgDoc.shapes.isNotEmpty()) {
                            val srcW = parsedSvgDoc.viewBoxW
                            val srcH = parsedSvgDoc.viewBoxH
                            val fitScale = minOf(w / srcW, h / srcH)
                            val offX = (w - srcW * fitScale) * 0.5f
                            val offY = (h - srcH * fitScale) * 0.5f
                            withTransform({
                                translate(left = offX, top = offY)
                                scale(scaleX = fitScale, scaleY = fitScale, pivot = Offset.Zero)
                            }) {
                                parsedSvgDoc.shapes.forEach { seg ->
                                    if (seg.fillColor != null) {
                                        drawPath(seg.path, color = seg.fillColor)
                                    }
                                    if (seg.strokeColor != null) {
                                        drawPath(
                                            seg.path,
                                            color = seg.strokeColor,
                                            style = Stroke(
                                                width = seg.strokeWidthPx.coerceAtLeast(1.1f),
                                                cap = androidx.compose.ui.graphics.StrokeCap.Round,
                                                join = androidx.compose.ui.graphics.StrokeJoin.Round
                                            )
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
                    val doubleTapDetector = GestureDetector(
                        ctx,
                        object : GestureDetector.SimpleOnGestureListener() {
                            override fun onDoubleTap(e: MotionEvent): Boolean {
                                viewModel.onCanvasDoubleTap(e.x, e.y)
                                return true
                            }
                        }
                    )

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
                            viewModel.uiState.value.inlineTextEditor?.let { activeEd ->
                                viewModel.commitInlineTextEdit(activeEd.text)
                            }
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
                        if (doubleTapDetector.onTouchEvent(motionEvent)) {
                            return@setOnTouchListener true
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

        // Pure In-Place Stencil & Text-Box Label Editor (directly over shape or below-icon caption)
        val inlineEd = uiState.inlineTextEditor
        if (inlineEd != null) {
            val safeDensity = density.coerceAtLeast(1f)
            val offsetXDp = (inlineEd.xPx / safeDensity).coerceAtLeast(4f).dp
            val offsetYDp = (inlineEd.yPx / safeDensity).coerceAtLeast(4f).dp
            val boxWidthDp = (inlineEd.widthPx / safeDensity).coerceIn(132f, 480f).dp
            val boxHeightDp = (inlineEd.heightPx / safeDensity).coerceIn(26f, 96f).dp
            val inlineFocusRequester = remember(inlineEd.shapeId) { FocusRequester() }
            var inlineFieldValue by remember(inlineEd.shapeId) {
                mutableStateOf(
                    TextFieldValue(
                        text = inlineEd.text,
                        selection = TextRange(0, inlineEd.text.length)
                    )
                )
            }
            LaunchedEffect(inlineEd.shapeId) {
                runCatching { inlineFocusRequester.requestFocus() }
            }

            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .offset(x = offsetXDp, y = offsetYDp)
                    .width(boxWidthDp)
                    .height(boxHeightDp)
                    .background(
                        if (uiState.textBgColorHex != "none") {
                            parseColorHexOrFallback(uiState.textBgColorHex, Color.White)
                        } else {
                            Color.White
                        },
                        RoundedCornerShape(2.dp)
                    )
                    .border(1.5.dp, MaterialTheme.colorScheme.primary, RoundedCornerShape(2.dp))
                    .padding(horizontal = 4.dp, vertical = 2.dp)
            ) {
                val combinedDecoration = when {
                    uiState.textUnderline && uiState.textStrikethrough ->
                        androidx.compose.ui.text.style.TextDecoration.combine(
                            listOf(
                                androidx.compose.ui.text.style.TextDecoration.Underline,
                                androidx.compose.ui.text.style.TextDecoration.LineThrough
                            )
                        )
                    uiState.textUnderline -> androidx.compose.ui.text.style.TextDecoration.Underline
                    uiState.textStrikethrough -> androidx.compose.ui.text.style.TextDecoration.LineThrough
                    else -> androidx.compose.ui.text.style.TextDecoration.None
                }
                BasicTextField(
                    value = inlineFieldValue,
                    onValueChange = { next ->
                        inlineFieldValue = next
                        viewModel.updateInlineTextDraft(next.text)
                    },
                    textStyle = MaterialTheme.typography.bodyMedium.copy(
                        color = parseColorHexOrFallback(uiState.textColorHex, Color(0xFF0F172A)),
                        fontFamily = resolveComposeFontFamily(uiState.textFontFamily),
                        fontWeight = if (uiState.textBold) FontWeight.Bold else FontWeight.Normal,
                        fontStyle = if (uiState.textItalic) FontStyle.Italic else FontStyle.Normal,
                        textDecoration = combinedDecoration,
                        textAlign = when (uiState.textHorizAlign) {
                            0 -> androidx.compose.ui.text.style.TextAlign.Start
                            2 -> androidx.compose.ui.text.style.TextAlign.End
                            else -> androidx.compose.ui.text.style.TextAlign.Center
                        }
                    ),
                    cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                    modifier = Modifier
                        .fillMaxWidth()
                        .focusRequester(inlineFocusRequester)
                        .onPreviewKeyEvent { keyEv ->
                            if (keyEv.type == KeyEventType.KeyDown &&
                                keyEv.key == Key.Enter &&
                                !keyEv.isShiftPressed
                            ) {
                                viewModel.commitInlineTextEdit(inlineFieldValue.text)
                                true
                            } else if (keyEv.type == KeyEventType.KeyDown &&
                                keyEv.key == Key.Escape
                            ) {
                                viewModel.cancelInlineTextEdit()
                                true
                            } else {
                                false
                            }
                        }
                )
            }
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
                        text = "Drop Stencil Master or .vsdx / .vssx / .vsd / .dwg / .dxf Here",
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
                text = { Text("Edit Shape Label (Double-Click)") },
                leadingIcon = { Icon(Icons.Default.Edit, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.beginInlineTextEdit(uiState.selectedShapeId)
                }
            )
            DropdownMenuItem(
                text = { Text("Group Selected Shapes (Ctrl+G)") },
                onClick = {
                    contextMenuVisible = false
                    viewModel.groupSelectedShapes()
                }
            )
            DropdownMenuItem(
                text = { Text("Ungroup Selected Group (Ctrl+Shift+G)") },
                onClick = {
                    contextMenuVisible = false
                    viewModel.ungroupSelectedShapes()
                }
            )
            DropdownMenuItem(
                text = { Text("Rotate Left 90° (↺)") },
                onClick = {
                    contextMenuVisible = false
                    viewModel.rotateOrFlipSelected(deltaAngleDeg = -90.0)
                }
            )
            DropdownMenuItem(
                text = { Text("Rotate Right 90° (↻)") },
                onClick = {
                    contextMenuVisible = false
                    viewModel.rotateOrFlipSelected(deltaAngleDeg = 90.0)
                }
            )
            DropdownMenuItem(
                text = { Text("Flip Horizontal (⇆)") },
                onClick = {
                    contextMenuVisible = false
                    viewModel.rotateOrFlipSelected(flipX = true)
                }
            )
            DropdownMenuItem(
                text = { Text("Flip Vertical (⇅)") },
                onClick = {
                    contextMenuVisible = false
                    viewModel.rotateOrFlipSelected(flipY = true)
                }
            )
            HorizontalDivider()
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
    viewModel: VexioSessionViewModel,
    onOpenColorPicker: (VexioColorPickerTarget) -> Unit,
    onSelectCell: (String) -> Unit,
    onQuickGuardToggle: (ShapeSheetCellEntry) -> Unit
) {
    val semanticColors = LocalVexioSemanticColors.current
    val lang = uiState.language
    var selectedTab by remember { mutableStateOf(0) }
    var inspectorFontMenuOpen by remember { mutableStateOf(false) }

    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        modifier = Modifier
            .width(296.dp)
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
                style = MaterialTheme.typography.titleSmall,
                maxLines = 1,
                softWrap = false
            )

            // Single-line non-wrapping Inspector Segmented Tabs ("Format" | "Geometry" | "Validate" — NEVER wraps!)
            Row(
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(10.dp))
                    .background(MaterialTheme.colorScheme.surfaceContainer)
                    .padding(3.dp)
            ) {
                listOf(
                    0 to lang.inspectorFormatTab,
                    1 to lang.inspectorGeometryTab,
                    2 to lang.inspectorValidateTab
                ).forEach { (idx, label) ->
                    val active = selectedTab == idx
                    Surface(
                        shape = RoundedCornerShape(8.dp),
                        color = if (active) MaterialTheme.colorScheme.primaryContainer else Color.Transparent,
                        modifier = Modifier
                            .weight(1f)
                            .clip(RoundedCornerShape(8.dp))
                            .clickable { selectedTab = idx }
                    ) {
                        Box(contentAlignment = Alignment.Center, modifier = Modifier.padding(vertical = 5.dp, horizontal = 4.dp)) {
                            Text(
                                text = label,
                                style = MaterialTheme.typography.labelSmall,
                                maxLines = 1,
                                softWrap = false,
                                overflow = TextOverflow.Ellipsis,
                                color = if (active) {
                                    MaterialTheme.colorScheme.onPrimaryContainer
                                } else {
                                    MaterialTheme.colorScheme.onSurfaceVariant
                                }
                            )
                        }
                    }
                }
            }

            when (selectedTab) {
                0 -> {
                    // Format Tab: OS Font Family + Full Spectrum Color Pickers (Text, Highlight, Fill, Line)
                    Surface(
                        shape = RoundedCornerShape(14.dp),
                        color = MaterialTheme.colorScheme.surfaceContainerLowest,
                        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Column(
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                            modifier = Modifier.padding(12.dp)
                        ) {
                            Text(
                                text = uiState.selectedShapeName,
                                style = MaterialTheme.typography.labelLarge,
                                maxLines = 1,
                                softWrap = false,
                                overflow = TextOverflow.Ellipsis
                            )

                            // Installed OS Font Picker
                            Box(modifier = Modifier.fillMaxWidth()) {
                                FilledTonalButton(
                                    onClick = { inspectorFontMenuOpen = true },
                                    modifier = Modifier.fillMaxWidth(),
                                    shape = RoundedCornerShape(10.dp),
                                    contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp)
                                ) {
                                    Text(
                                        text = "Font: ${uiState.textFontFamily} (${uiState.textFontSizePt} pt) ▾",
                                        style = MaterialTheme.typography.labelMedium,
                                        fontFamily = resolveComposeFontFamily(uiState.textFontFamily),
                                        maxLines = 1,
                                        softWrap = false,
                                        overflow = TextOverflow.Ellipsis
                                    )
                                }
                                DropdownMenu(
                                    expanded = inspectorFontMenuOpen,
                                    onDismissRequest = { inspectorFontMenuOpen = false },
                                    modifier = Modifier
                                        .width(248.dp)
                                        .heightIn(max = 320.dp)
                                ) {
                                    uiState.availableFontFamilies.forEach { f ->
                                        DropdownMenuItem(
                                            text = {
                                                Text(
                                                    text = f,
                                                    fontFamily = resolveComposeFontFamily(f),
                                                    maxLines = 1,
                                                    softWrap = false
                                                )
                                            },
                                            leadingIcon = {
                                                if (f.equals(uiState.textFontFamily, ignoreCase = true)) {
                                                    Icon(Icons.Default.Check, contentDescription = null)
                                                }
                                            },
                                            onClick = {
                                                inspectorFontMenuOpen = false
                                                viewModel.applyRichTextStyle(fontFamily = f)
                                            }
                                        )
                                    }
                                }
                            }

                            // 2x2 Grid of Full Color Picker Triggers
                            Row(
                                horizontalArrangement = Arrangement.spacedBy(6.dp),
                                modifier = Modifier.fillMaxWidth()
                            ) {
                                Box(modifier = Modifier.weight(1f)) {
                                    ColorSwatchActionPill(
                                        label = lang.fillColorLabel,
                                        swatchColor = parseColorHexOrFallback(uiState.shapeFillColorHex, Color(0xFFE8F0FE)),
                                        isTransparent = false,
                                        onClick = { onOpenColorPicker(VexioColorPickerTarget.SHAPE_FILL) }
                                    )
                                }
                                Box(modifier = Modifier.weight(1f)) {
                                    ColorSwatchActionPill(
                                        label = lang.lineColorLabel,
                                        swatchColor = parseColorHexOrFallback(uiState.shapeLineColorHex, Color(0xFF1A73E8)),
                                        isTransparent = false,
                                        onClick = { onOpenColorPicker(VexioColorPickerTarget.SHAPE_LINE) }
                                    )
                                }
                            }
                            Row(
                                horizontalArrangement = Arrangement.spacedBy(6.dp),
                                modifier = Modifier.fillMaxWidth()
                            ) {
                                Box(modifier = Modifier.weight(1f)) {
                                    ColorSwatchActionPill(
                                        label = lang.textColorLabel,
                                        swatchColor = parseColorHexOrFallback(uiState.textColorHex, Color(0xFF0F172A)),
                                        isTransparent = false,
                                        onClick = { onOpenColorPicker(VexioColorPickerTarget.TEXT_FG) }
                                    )
                                }
                                Box(modifier = Modifier.weight(1f)) {
                                    ColorSwatchActionPill(
                                        label = lang.highlightColorLabel,
                                        swatchColor = parseColorHexOrFallback(uiState.textBgColorHex, Color(0xFFFEF08A)),
                                        isTransparent = uiState.textBgColorHex == "none",
                                        onClick = { onOpenColorPicker(VexioColorPickerTarget.TEXT_BG) }
                                    )
                                }
                            }
                            if (uiState.textBgColorHex != "none") {
                                TextButton(
                                    onClick = { viewModel.applyRichTextStyle(bgColorHex = "none") },
                                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp)
                                ) {
                                    Text(lang.clearHighlightLabel, style = MaterialTheme.typography.labelSmall)
                                }
                            }
                        }
                    }

                    // Plan 2 Enterprise Telemetry Card
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
                        }
                    }
                }

                1 -> {
                    // Geometry Tab: ShapeSheet Cells + GUARD() Toggle
                    Surface(
                        shape = RoundedCornerShape(14.dp),
                        color = MaterialTheme.colorScheme.surfaceContainerLowest,
                        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Column(modifier = Modifier.padding(12.dp)) {
                            Text(
                                text = uiState.selectedShapeName,
                                style = MaterialTheme.typography.labelLarge,
                                maxLines = 1,
                                softWrap = false,
                                overflow = TextOverflow.Ellipsis
                            )
                            Spacer(Modifier.height(6.dp))
                            uiState.shapeSheetCells.take(10).forEach { cell ->
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
                                        maxLines = 1,
                                        softWrap = false,
                                        modifier = Modifier.width(76.dp)
                                    )
                                    Text(
                                        text = cell.formula,
                                        style = MaterialTheme.typography.bodySmall,
                                        maxLines = 1,
                                        softWrap = false,
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
                }

                else -> {
                    // Validate Tab: BPMN 2.0 / UML 2.5 Validator + Subprocess/CRDT/Shortcuts
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

                    Row(
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        ExpressivePlan2ActionPill(
                            label = "Run BPMN 2.0",
                            selected = uiState.validationSummary.standard == "BPMN 2.0",
                            onClick = { viewModel.validateActiveDiagram("bpmn") }
                        )
                        ExpressivePlan2ActionPill(
                            label = "Run UML 2.5",
                            selected = uiState.validationSummary.standard == "UML 2.5",
                            onClick = { viewModel.validateActiveDiagram("uml") }
                        )
                        ExpressivePlan2ActionPill(
                            label = "Workflow",
                            selected = uiState.validationSummary.standard == "Workflow",
                            onClick = { viewModel.validateActiveDiagram("workflow") }
                        )
                    }

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
                            Text("Ctrl+S / Ctrl+Shift+S  Save / Save As", style = MaterialTheme.typography.labelSmall)
                            Text("Ctrl+N  New Side-by-Side Window", style = MaterialTheme.typography.labelSmall)
                            Text("Ctrl+1..5  Pointer / Text / Conn / Ink / Freeform", style = MaterialTheme.typography.labelSmall)
                            Text("Shift+F5  ShapeSheet Formula Grid", style = MaterialTheme.typography.labelSmall)
                            Text("Ctrl+P  Vector PDF OS Print", style = MaterialTheme.typography.labelSmall)
                        }
                    }
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
    onRenamePage: (Int, String) -> Boolean,
    onAddPage: () -> Unit,
    onCreateSubprocess: () -> Unit,
    onAutoLayoutPage: () -> Unit,
    onZoomReset: () -> Unit
) {
    var editingPageId by remember { mutableStateOf<Int?>(null) }
    var editingPageDraft by remember { mutableStateOf(TextFieldValue("")) }
    val pageRenameFocusRequester = remember { FocusRequester() }

    LaunchedEffect(editingPageId) {
        if (editingPageId != null) {
            runCatching { pageRenameFocusRequester.requestFocus() }
        }
    }

    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = Modifier
            .fillMaxWidth()
            .height(36.dp)
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 10.dp)
        ) {
            // Single-line Horizontally Scrollable Page Tabs (Zero Vertical Wrapping on 7–15+ Pages!)
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                modifier = Modifier
                    .weight(1f)
                    .horizontalScroll(rememberScrollState())
            ) {
                uiState.pages.forEach { tab ->
                    val active = tab.pageId == uiState.activePageId
                    var pageMenuExpanded by remember(tab.pageId) { mutableStateOf(false) }

                    Box {
                        if (editingPageId == tab.pageId) {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                modifier = Modifier
                                    .clip(RoundedCornerShape(8.dp))
                                    .background(MaterialTheme.colorScheme.surfaceContainerLowest)
                                    .border(1.5.dp, MaterialTheme.colorScheme.primary, RoundedCornerShape(8.dp))
                                    .padding(horizontal = 8.dp, vertical = 3.dp)
                            ) {
                                BasicTextField(
                                    value = editingPageDraft,
                                    onValueChange = { editingPageDraft = it },
                                    singleLine = true,
                                    textStyle = MaterialTheme.typography.labelMedium.copy(
                                        color = MaterialTheme.colorScheme.onSurface
                                    ),
                                    cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                                    modifier = Modifier
                                        .width(148.dp)
                                        .focusRequester(pageRenameFocusRequester)
                                        .onPreviewKeyEvent { ev ->
                                            if (ev.type == KeyEventType.KeyDown && ev.key == Key.Enter) {
                                                onRenamePage(tab.pageId, editingPageDraft.text)
                                                editingPageId = null
                                                true
                                            } else if (ev.type == KeyEventType.KeyDown && ev.key == Key.Escape) {
                                                editingPageId = null
                                                true
                                            } else {
                                                false
                                            }
                                        }
                                )
                                Icon(
                                    imageVector = Icons.Default.Check,
                                    contentDescription = "Commit Page Name",
                                    tint = MaterialTheme.colorScheme.primary,
                                    modifier = Modifier
                                        .size(16.dp)
                                        .clickable {
                                            onRenamePage(tab.pageId, editingPageDraft.text)
                                            editingPageId = null
                                        }
                                )
                            }
                        } else {
                            Surface(
                                shape = RoundedCornerShape(8.dp),
                                color = if (active) {
                                    MaterialTheme.colorScheme.primaryContainer
                                } else {
                                    MaterialTheme.colorScheme.surfaceContainerLow
                                },
                                modifier = Modifier
                                    .clip(RoundedCornerShape(8.dp))
                                    .combinedClickable(
                                        onClick = { onSelectPage(tab.pageId) },
                                        onDoubleClick = {
                                            editingPageDraft = TextFieldValue(
                                                text = tab.name,
                                                selection = TextRange(0, tab.name.length)
                                            )
                                            editingPageId = tab.pageId
                                        },
                                        onLongClick = { pageMenuExpanded = true }
                                    )
                            ) {
                                Text(
                                    text = tab.name,
                                    style = MaterialTheme.typography.labelMedium,
                                    maxLines = 1,
                                    softWrap = false,
                                    color = if (active) {
                                        MaterialTheme.colorScheme.onPrimaryContainer
                                    } else {
                                        MaterialTheme.colorScheme.onSurfaceVariant
                                    },
                                    modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp)
                                )
                            }
                        }

                        DropdownMenu(
                            expanded = pageMenuExpanded,
                            onDismissRequest = { pageMenuExpanded = false }
                        ) {
                            DropdownMenuItem(
                                text = { Text(uiState.language.renamePageLabel) },
                                leadingIcon = { Icon(Icons.Default.Edit, contentDescription = null) },
                                onClick = {
                                    pageMenuExpanded = false
                                    editingPageDraft = TextFieldValue(
                                        text = tab.name,
                                        selection = TextRange(0, tab.name.length)
                                    )
                                    editingPageId = tab.pageId
                                }
                            )
                            DropdownMenuItem(
                                text = { Text("Switch to ${tab.name} (${tab.scaleLabel})") },
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
                            .padding(4.dp)
                            .size(16.dp)
                    )
                }
            }

            if (posture != AdaptiveWorkstationPosture.COMPACT_CONVERTIBLE) {
                Text(
                    text = "Cursor: ${uiState.activeCursor.name} • Tool: ${uiState.activeTool.displayName}",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    softWrap = false,
                    modifier = Modifier.padding(horizontal = 10.dp)
                )
            }

            Text(
                text = "Zoom ${uiState.zoomPercent}%",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.primary,
                maxLines = 1,
                softWrap = false,
                modifier = Modifier
                    .clip(RoundedCornerShape(6.dp))
                    .clickable { onZoomReset() }
                    .padding(horizontal = 8.dp, vertical = 4.dp)
            )
        }
    }
}

@Composable
private fun FullSpectrumColorPickerDialog(
    title: String,
    initialHex: String,
    allowClearNone: Boolean,
    clearLabel: String,
    onDismiss: () -> Unit,
    onClearNone: () -> Unit,
    onColorSelected: (String) -> Unit
) {
    val startColor = remember(initialHex) {
        parseColorHexOrFallback(initialHex, Color(0xFF1D4ED8))
    }
    var red by remember(initialHex) { mutableStateOf((startColor.red * 255f).toInt().coerceIn(0, 255)) }
    var green by remember(initialHex) { mutableStateOf((startColor.green * 255f).toInt().coerceIn(0, 255)) }
    var blue by remember(initialHex) { mutableStateOf((startColor.blue * 255f).toInt().coerceIn(0, 255)) }
    var hexInput by remember(initialHex) {
        mutableStateOf("#%02X%02X%02X".format(java.util.Locale.US, red, green, blue))
    }

    fun syncHexFromRgb(r: Int, g: Int, b: Int) {
        red = r.coerceIn(0, 255)
        green = g.coerceIn(0, 255)
        blue = b.coerceIn(0, 255)
        hexInput = "#%02X%02X%02X".format(java.util.Locale.US, red, green, blue)
    }

    val liveColor = Color(red / 255f, green / 255f, blue / 255f, 1f)
    val standardSwatches = listOf(
        "#0F172A", "#334155", "#64748B", "#FFFFFF",
        "#DBEAFE", "#3B82F6", "#1D4ED8", "#1E3A8A",
        "#DCFCE7", "#22C55E", "#15803D", "#065F46",
        "#FEF9C3", "#FACC15", "#D97706", "#B45309",
        "#FEE2E2", "#EF4444", "#DC2626", "#991B1B",
        "#F3E8FF", "#A855F7", "#7C3AED", "#5B21B6"
    )

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title, maxLines = 1, softWrap = false) },
        text = {
            Column(
                verticalArrangement = Arrangement.spacedBy(10.dp),
                modifier = Modifier.verticalScroll(rememberScrollState())
            ) {
                // Live Color Preview + Hex Input
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Box(
                        modifier = Modifier
                            .size(44.dp)
                            .clip(RoundedCornerShape(10.dp))
                            .background(liveColor)
                            .border(1.dp, MaterialTheme.colorScheme.outline, RoundedCornerShape(10.dp))
                    )
                    OutlinedTextField(
                        value = hexInput,
                        onValueChange = { raw ->
                            hexInput = raw
                            val clean = raw.trim().removePrefix("#")
                            if (clean.length == 6) {
                                val parsed = clean.toIntOrNull(16)
                                if (parsed != null) {
                                    red = (parsed shr 16) and 0xFF
                                    green = (parsed shr 8) and 0xFF
                                    blue = parsed and 0xFF
                                }
                            }
                        },
                        singleLine = true,
                        label = { Text("HEX (#RRGGBB)") },
                        modifier = Modifier.weight(1f)
                    )
                }

                // Interactive Hue Spectrum Strip (Tap anywhere along 360° HSV spectrum)
                androidx.compose.foundation.Canvas(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(26.dp)
                        .clip(RoundedCornerShape(8.dp))
                        .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(8.dp))
                        .pointerInput(Unit) {
                            awaitPointerEventScope {
                                while (true) {
                                    val ev = awaitPointerEvent()
                                    val change = ev.changes.firstOrNull()
                                    if (change != null && change.pressed && size.width > 0) {
                                        val hue = ((change.position.x / size.width.toFloat()).coerceIn(0f, 1f)) * 360f
                                        val c = Color.hsv(hue, 0.85f, 0.92f)
                                        syncHexFromRgb(
                                            (c.red * 255f).toInt(),
                                            (c.green * 255f).toInt(),
                                            (c.blue * 255f).toInt()
                                        )
                                    }
                                }
                            }
                        }
                ) {
                    val steps = 36
                    val segW = size.width / steps
                    for (i in 0 until steps) {
                        val hue = i * (360f / steps)
                        drawRect(
                            color = Color.hsv(hue, 0.85f, 0.92f),
                            topLeft = Offset(i * segW, 0f),
                            size = androidx.compose.ui.geometry.Size(segW + 1f, size.height)
                        )
                    }
                }

                // Fine RGB Channel Sliders (0..255)
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text("Red: $red", style = MaterialTheme.typography.labelSmall)
                    Slider(
                        value = red.toFloat(),
                        onValueChange = { syncHexFromRgb(it.toInt(), green, blue) },
                        valueRange = 0f..255f
                    )
                    Text("Green: $green", style = MaterialTheme.typography.labelSmall)
                    Slider(
                        value = green.toFloat(),
                        onValueChange = { syncHexFromRgb(red, it.toInt(), blue) },
                        valueRange = 0f..255f
                    )
                    Text("Blue: $blue", style = MaterialTheme.typography.labelSmall)
                    Slider(
                        value = blue.toFloat(),
                        onValueChange = { syncHexFromRgb(red, green, it.toInt()) },
                        valueRange = 0f..255f
                    )
                }

                // 24 Visio Theme & Standard Swatches
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    standardSwatches.chunked(8).forEach { rowHexes ->
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            rowHexes.forEach { swHex ->
                                val swColor = parseColorHexOrFallback(swHex, Color.Black)
                                Box(
                                    modifier = Modifier
                                        .size(26.dp)
                                        .clip(CircleShape)
                                        .background(swColor)
                                        .border(
                                            width = if (swHex.equals(hexInput, ignoreCase = true)) 2.dp else 1.dp,
                                            color = if (swHex.equals(hexInput, ignoreCase = true)) {
                                                MaterialTheme.colorScheme.primary
                                            } else {
                                                MaterialTheme.colorScheme.outlineVariant
                                            },
                                            shape = CircleShape
                                        )
                                        .clickable {
                                            syncHexFromRgb(
                                                (swColor.red * 255f).toInt(),
                                                (swColor.green * 255f).toInt(),
                                                (swColor.blue * 255f).toInt()
                                            )
                                        }
                                )
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {
            FilledTonalButton(
                onClick = {
                    val normalized = "#%02X%02X%02X".format(java.util.Locale.US, red, green, blue)
                    onColorSelected(normalized)
                }
            ) {
                Text("Apply Color")
            }
        },
        dismissButton = {
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                if (allowClearNone) {
                    TextButton(onClick = onClearNone) {
                        Text(clearLabel)
                    }
                }
                TextButton(onClick = onDismiss) {
                    Text("Cancel")
                }
            }
        }
    )
}

