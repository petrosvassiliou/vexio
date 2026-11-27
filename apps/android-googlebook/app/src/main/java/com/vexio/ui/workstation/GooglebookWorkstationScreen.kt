package com.vexio.ui.workstation

import android.app.Activity
import android.content.ClipData
import android.content.ClipDescription
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Paint
import android.graphics.Point
import android.graphics.RectF
import android.net.Uri
import android.util.Base64
import android.util.LruCache
import android.view.DragEvent
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import androidx.activity.compose.BackHandler
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
import androidx.compose.foundation.draganddrop.dragAndDropTarget
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
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
import androidx.compose.material.icons.filled.MoreVert
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
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.VerticalDivider
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draganddrop.DragAndDropEvent
import androidx.compose.ui.draganddrop.DragAndDropTarget
import androidx.compose.ui.draganddrop.mimeTypes
import androidx.compose.ui.draganddrop.toAndroidDragEvent
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusTarget
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.asAndroidBitmap
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
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.PointerIcon
import androidx.compose.ui.input.pointer.isSecondaryPressed
import androidx.compose.ui.input.pointer.pointerHoverIcon
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties
import androidx.input.motionprediction.MotionEventPredictor
import com.vexio.MainActivity
import com.vexio.core.CrossWindowShapePayload
import com.vexio.core.DataGraphicUiState
import com.vexio.core.DiagramLayerUiItem
import com.vexio.core.DiagramPageTab
import com.vexio.core.ExternalRecordsetUiItem
import com.vexio.core.FormulaCommitStatus
import com.vexio.core.InspectedObjectUiState
import com.vexio.core.Plan2SamplePayloads
import com.vexio.core.ReviewCommentUiItem
import com.vexio.core.ShapeDataUiItem
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
import com.vexio.ui.i18n.VexioI18n
import com.vexio.ui.theme.LocalVexioSemanticColors
import com.vexio.ui.theme.VexioExpressiveMotion
import com.vexio.ui.theme.rememberExpressiveScaleState
import com.vexio.ui.theme.rememberExpressiveToolButtonShape
import com.vexio.ui.theme.rememberMorphingToolbarShape
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

const val ACTION_NEW_DOCUMENT_WINDOW = "com.vexio.action.NEW_DOCUMENT_WINDOW"
const val EXTRA_DOCUMENT_TEMPLATE = "vexio.template"
const val VEXIO_MASTER_CLIP_MIME = "application/vnd.vexio.master+json"

private val SUPPORTED_DIAGRAM_EXTENSIONS = setOf(
    "vsdx", "vsdm", "vssx", "vssm", "vstx", "vstm", "vsd", "vss", "vst", "vdx", "vdw", "dwg", "dxf",
    "xlsx", "csv", "tsv", "sqlite", "db", "sqlite3", "json"
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
    onLinkExternalDataRequested: () -> Unit = {},
    onRefreshExternalDataRequested: () -> Unit = { viewModel.refreshExternalRecordsets() },
    onTwoWayWritebackRequested: (Int) -> Unit = {},
    onExportBomReportRequested: () -> Unit = {},
    onExportJpgRequested: () -> Unit = { viewModel.exportPlan2Artifact("jpg") },
    onExportFormatRequested: (String) -> Unit = { fmt -> viewModel.exportPlan2Artifact(fmt) },
    onSaveAndExitRequested: () -> Unit = { onSaveDocumentRequested() },
    onDiscardAndExitRequested: () -> Unit = { },
    onPrintRequested: () -> Unit = { }
) {
    val context = LocalContext.current
    val uiState by viewModel.uiState.collectAsState()
    val semanticColors = LocalVexioSemanticColors.current
    val rootFocusRequester = remember { FocusRequester() }
    val formulaFocusRequester = remember { FocusRequester() }
    var showUnsavedExitDialog by remember { mutableStateOf(false) }

    BackHandler(enabled = !uiState.showStartBackstage && uiState.isDirty) {
        showUnsavedExitDialog = true
    }

    if (showUnsavedExitDialog) {
        val lang = uiState.language
        AlertDialog(
            onDismissRequest = { showUnsavedExitDialog = false },
            title = {
                Text(
                    text = VexioI18n.unsavedPromptTitle(uiState.documentTitle, lang),
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold
                )
            },
            text = {
                Text(
                    text = VexioI18n.unsavedPromptMessage(lang),
                    style = MaterialTheme.typography.bodyMedium
                )
            },
            confirmButton = {
                Button(
                    onClick = {
                        showUnsavedExitDialog = false
                        onSaveAndExitRequested()
                    }
                ) {
                    Text(lang.saveLabel)
                }
            },
            dismissButton = {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TextButton(onClick = { showUnsavedExitDialog = false }) {
                        Text(VexioI18n.cancelButton(lang))
                    }
                    OutlinedButton(
                        onClick = {
                            showUnsavedExitDialog = false
                            onDiscardAndExitRequested()
                        }
                    ) {
                        Text(
                            text = VexioI18n.dontSaveButton(lang),
                            color = MaterialTheme.colorScheme.error
                        )
                    }
                }
            }
        )
    }

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
                    // Alt + F9 -> Sandboxed VBA Macro Studio
                    alt && !ctrlOrMeta && keyEvent.key == Key.F9 -> {
                        viewModel.setVbaMacroDialogVisible(true)
                        true
                    }
                    // Ctrl + K -> Shape Hyperlink & ScreenTip
                    ctrlOrMeta && !shift && keyEvent.key == Key.K -> {
                        viewModel.setHyperlinkDialogVisible(true)
                        true
                    }
                    // Ctrl + 6 -> Edit Sub-Shape Vertices
                    ctrlOrMeta && keyEvent.key == Key.Six -> {
                        viewModel.selectTool(VexioTool.EDIT_VERTICES)
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
                    // Ctrl + W / Ctrl + Q -> Close Window (prompts Save / Don't Save / Cancel if dirty!)
                    ctrlOrMeta && (keyEvent.key == Key.W || keyEvent.key == Key.Q) -> {
                        if (uiState.isDirty) {
                            showUnsavedExitDialog = true
                        } else {
                            (context as? Activity)?.finish()
                        }
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
                    // Ctrl + Shift + 1 -> Connection Point Tool (Visio Shift+Ctrl+1 Parity)
                    ctrlOrMeta && shift && keyEvent.key == Key.One -> {
                        viewModel.selectTool(
                            if (uiState.activeTool == VexioTool.CONNECTION_POINT) VexioTool.POINTER else VexioTool.CONNECTION_POINT
                        )
                        true
                    }
                    // Ctrl + 1..9 -> Visio Authoring Tool Parity
                    ctrlOrMeta && !shift && keyEvent.key == Key.One -> {
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
        var pageSetupTargetPageId by remember { mutableStateOf<Int?>(null) }

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
                onLinkExternalDataClick = onLinkExternalDataRequested,
                onRefreshExternalDataClick = onRefreshExternalDataRequested,
                onTwoWayWritebackClick = { onTwoWayWritebackRequested(0) },
                onExportBomReportClick = onExportBomReportRequested,
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
                        onMasterSelected = { master ->
                            viewModel.highlightStencilMaster(master.masterId)
                        },
                        onMasterDoubleClicked = { master ->
                            viewModel.dropStencilAtViewportCenter(master)
                        },
                        onImportStencilRequested = onImportStencilRequested,
                        onOpenOfficialCatalogRequested = {
                            viewModel.setOfficialStencilCatalogDialogVisible(true)
                        },
                        onAddToMyShapesRequested = {
                            viewModel.addSelectionToMyShapes()
                        },
                        onExportMyShapesVssxRequested = {
                            onExportFormatRequested("vssx")
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
                            onZoomReset = { viewModel.setZoomPercent(100) },
                            onToggleFloating = viewModel::toggleToolbarDockMode,
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 10.dp, vertical = 4.dp)
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
                                onZoomReset = { viewModel.setZoomPercent(100) },
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
                        onLinkExternalDataClick = onLinkExternalDataRequested,
                        onRefreshExternalDataClick = onRefreshExternalDataRequested,
                        onTwoWayWritebackClick = onTwoWayWritebackRequested,
                        onExportBomReportClick = onExportBomReportRequested,
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

            // 4. Bottom Page Switcher & Workstation Diagnostics Bar (Right-Click Page Menu + Page Setup)
            WorkstationBottomStatusBar(
                uiState = uiState,
                posture = posture,
                onSelectPage = viewModel::selectPage,
                onRenamePage = viewModel::renamePage,
                onAddPage = viewModel::addNewPage,
                onDuplicatePage = { pid -> viewModel.duplicatePage(pid) },
                onDeletePage = { pid -> viewModel.deletePage(pid) },
                onSetPageBackground = { pid, isBg, backId, colorHex ->
                    viewModel.setPageBackground(pid, isBg, backId, colorHex)
                },
                onAddBackgroundPage = { pid -> viewModel.addNewBackgroundPage(pid) },
                onOpenPageSetup = { pid ->
                    viewModel.selectPage(pid)
                    pageSetupTargetPageId = pid
                },
                onCreateSubprocess = { viewModel.createOrLinkSubprocess() },
                onAutoLayoutPage = { viewModel.autoLayoutActivePage() },
                onZoomReset = { viewModel.setZoomPercent(100) }
            )
        }

        val setupPid = pageSetupTargetPageId
        if (setupPid != null) {
            val targetTab = uiState.pages.firstOrNull { it.pageId == setupPid }
                ?: uiState.pages.firstOrNull()
            if (targetTab != null) {
                PageSetupDialog(
                    pageTab = targetTab,
                    allPages = uiState.pages,
                    metricUnits = uiState.metricUnits,
                    lang = uiState.language,
                    onDismiss = { pageSetupTargetPageId = null },
                    onApply = { wIn, hIn, metric, isBg, backId, colorHex ->
                        viewModel.applyCustomPageSetup(
                            pageId = targetTab.pageId,
                            widthInches = wIn,
                            heightInches = hIn,
                            metricUnits = metric,
                            isBackground = isBg,
                            backPageId = backId,
                            pageColorHex = colorHex
                        )
                        pageSetupTargetPageId = null
                    }
                )
            } else {
                pageSetupTargetPageId = null
            }
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
                lang = lang,
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

        // In-App Welcome / Startup Hub (No external OS launch file dialog)
        if (uiState.showStartBackstage) {
            InAppStartBackstageOverlay(
                uiState = uiState,
                onClose = { viewModel.setStartBackstageVisible(false) },
                onCreateNewBlank = {
                    viewModel.createBlankWithPagePreset("Untitled_Diagram.vsdx", VexioPagePresetSpec.A4_LANDSCAPE)
                },
                onLoadTemplate = viewModel::loadStarterTemplate,
                onOpenFile = {
                    viewModel.setStartBackstageVisible(false)
                    onOpenDocumentRequested()
                },
                onSelectLanguage = viewModel::setLanguage,
                onToggleAutoUpdates = viewModel::setAutoCheckUpdates,
                onCheckUpdates = { viewModel.checkForUpdatesAsync(userInitiated = true) }
            )
        }

        // Automatic & Manual Update Check Dialog
        if (uiState.showUpdateDialog) {
            val lang = uiState.language
            AlertDialog(
                onDismissRequest = viewModel::dismissUpdateDialog,
                title = {
                    Text(
                        if (uiState.availableUpdateTag != null) {
                            VexioI18n.localizeUpdateStatus("", uiState.availableUpdateTag, lang)
                        } else {
                            lang.checkUpdatesLabel
                        }
                    )
                },
                text = {
                    Text(
                        VexioI18n.localizeUpdateStatus(
                            uiState.updateStatusMessage,
                            uiState.availableUpdateTag,
                            lang
                        )
                    )
                },
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
                            Text(
                                if (uiState.availableUpdateTag != null) {
                                    VexioI18n.downloadApkUpdate(lang)
                                } else {
                                    VexioI18n.releaseNotesButton(lang)
                                }
                            )
                        }
                    }
                },
                dismissButton = {
                    TextButton(onClick = viewModel::dismissUpdateDialog) {
                        Text(VexioI18n.closeButton(lang))
                    }
                }
            )
        }

        // Diagram-as-Code Studio Dialog (Mermaid / PlantUML / Graphviz DOT)
        if (uiState.showMarkupStudioDialog) {
            AlertDialog(
                onDismissRequest = { viewModel.setMarkupStudioDialogVisible(false) },
                title = { Text("Diagram-as-Code — Mermaid · PlantUML · DOT") },
                text = {
                    Column(
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            listOf("mermaid" to "Mermaid (.mmd)", "plantuml" to "PlantUML (.puml)", "dot" to "DOT (.dot)").forEach { (slug, label) ->
                                FilterChip(
                                    selected = uiState.markupStudioFormat == slug,
                                    onClick = { viewModel.setMarkupStudioFormat(slug) },
                                    label = { Text(label) }
                                )
                            }
                        }
                        OutlinedTextField(
                            value = uiState.markupStudioDraft,
                            onValueChange = viewModel::setMarkupStudioDraft,
                            minLines = 7,
                            maxLines = 12,
                            modifier = Modifier.fillMaxWidth()
                        )
                    }
                },
                confirmButton = {
                    FilledTonalButton(onClick = { viewModel.importDiagramMarkup() }) {
                        Text("Build Visio Shapes")
                    }
                },
                dismissButton = {
                    TextButton(onClick = { viewModel.setMarkupStudioDialogVisible(false) }) {
                        Text("Close")
                    }
                }
            )
        }

        // Sandboxed VBA Macro Studio Dialog (Alt+F9)
        if (uiState.showVbaMacroDialog) {
            AlertDialog(
                onDismissRequest = { viewModel.setVbaMacroDialogVisible(false) },
                title = { Text("Sandboxed VBA Macro Studio (Alt+F9)") },
                text = {
                    Column(
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Text(
                            text = "Deterministic zero-I/O gas-metered VBA execution with full Undo/Redo support.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        OutlinedTextField(
                            value = uiState.vbaMacroScript,
                            onValueChange = viewModel::setVbaMacroScript,
                            textStyle = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                            minLines = 7,
                            maxLines = 12,
                            modifier = Modifier.fillMaxWidth()
                        )
                        uiState.lastVbaReportSummary?.let { msg ->
                            Text(msg, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary)
                        }
                    }
                },
                confirmButton = {
                    FilledTonalButton(onClick = { viewModel.runVbaMacro() }) {
                        Text("Execute in Sandbox")
                    }
                },
                dismissButton = {
                    TextButton(onClick = { viewModel.setVbaMacroDialogVisible(false) }) {
                        Text("Close")
                    }
                }
            )
        }

        // Review — Threaded Diagram Comments Dialog
        if (uiState.showCommentsDialog) {
            var commentAuthor by remember { mutableStateOf("Reviewer") }
            var commentDraft by remember { mutableStateOf("") }
            AlertDialog(
                onDismissRequest = { viewModel.setCommentsDialogVisible(false) },
                title = { Text("Review — Threaded Comments") },
                text = {
                    Column(
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(max = 380.dp)
                            .verticalScroll(rememberScrollState())
                    ) {
                        if (uiState.reviewComments.isEmpty()) {
                            Text(
                                text = "No comments on this page yet. Add a review note attached to Sheet.${uiState.selectedShapeId}.",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        } else {
                            uiState.reviewComments.forEach { c ->
                                Surface(
                                    shape = RoundedCornerShape(10.dp),
                                    color = MaterialTheme.colorScheme.surfaceContainerLow,
                                    border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                                    modifier = Modifier.fillMaxWidth()
                                ) {
                                    Row(
                                        verticalAlignment = Alignment.CenterVertically,
                                        horizontalArrangement = Arrangement.SpaceBetween,
                                        modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp)
                                    ) {
                                        Column(modifier = Modifier.weight(1f)) {
                                            Text(
                                                text = "${if (c.resolved) "✓ " else "● "}#${c.id} · ${c.author} (Sheet.${c.shapeId})",
                                                style = MaterialTheme.typography.labelLarge,
                                                fontWeight = FontWeight.SemiBold
                                            )
                                            Text(c.text, style = MaterialTheme.typography.bodySmall)
                                        }
                                        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                                            TextButton(onClick = { viewModel.toggleResolveComment(c.id, !c.resolved) }) {
                                                Text(if (c.resolved) "Reopen" else "Resolve")
                                            }
                                            TextButton(onClick = { viewModel.deleteReviewComment(c.id) }) {
                                                Text("Delete", color = MaterialTheme.colorScheme.error)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        OutlinedTextField(
                            value = commentAuthor,
                            onValueChange = { commentAuthor = it },
                            singleLine = true,
                            label = { Text("Author") },
                            modifier = Modifier.fillMaxWidth()
                        )
                        OutlinedTextField(
                            value = commentDraft,
                            onValueChange = { commentDraft = it },
                            minLines = 2,
                            maxLines = 4,
                            label = { Text("Comment on Sheet.${uiState.selectedShapeId}") },
                            modifier = Modifier.fillMaxWidth()
                        )
                    }
                },
                confirmButton = {
                    FilledTonalButton(
                        onClick = {
                            if (commentDraft.isNotBlank()) {
                                viewModel.addReviewComment(commentAuthor, commentDraft)
                                commentDraft = ""
                            }
                        }
                    ) {
                        Text("Post Comment")
                    }
                },
                dismissButton = {
                    TextButton(onClick = { viewModel.setCommentsDialogVisible(false) }) {
                        Text("Done")
                    }
                }
            )
        }

        // Insert — Shape Hyperlink & ScreenTip Dialog (Ctrl+K)
        if (uiState.showHyperlinkDialog) {
            var addrDraft by remember(uiState.hyperlinkAddress) { mutableStateOf(uiState.hyperlinkAddress) }
            var subDraft by remember(uiState.hyperlinkSubAddress) { mutableStateOf(uiState.hyperlinkSubAddress) }
            var descDraft by remember(uiState.hyperlinkDescription) { mutableStateOf(uiState.hyperlinkDescription) }
            var tipDraft by remember(uiState.shapeScreentip) { mutableStateOf(uiState.shapeScreentip) }
            AlertDialog(
                onDismissRequest = { viewModel.setHyperlinkDialogVisible(false) },
                title = { Text("Hyperlink & ScreenTip — Sheet.${uiState.selectedShapeId}") },
                text = {
                    Column(
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        OutlinedTextField(
                            value = addrDraft,
                            onValueChange = { addrDraft = it },
                            singleLine = true,
                            label = { Text("Address (URL or File)") },
                            modifier = Modifier.fillMaxWidth()
                        )
                        OutlinedTextField(
                            value = subDraft,
                            onValueChange = { subDraft = it },
                            singleLine = true,
                            label = { Text("Sub-Address (Target Page Name)") },
                            modifier = Modifier.fillMaxWidth()
                        )
                        OutlinedTextField(
                            value = descDraft,
                            onValueChange = { descDraft = it },
                            singleLine = true,
                            label = { Text("Description") },
                            modifier = Modifier.fillMaxWidth()
                        )
                        OutlinedTextField(
                            value = tipDraft,
                            onValueChange = { tipDraft = it },
                            singleLine = true,
                            label = { Text("ScreenTip (Hover Tooltip)") },
                            modifier = Modifier.fillMaxWidth()
                        )
                    }
                },
                confirmButton = {
                    FilledTonalButton(
                        onClick = {
                            viewModel.saveHyperlinkAndScreentip(descDraft, addrDraft, subDraft, tipDraft)
                        }
                    ) {
                        Text("Save")
                    }
                },
                dismissButton = {
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        TextButton(onClick = { viewModel.saveHyperlinkAndScreentip("", "", "", "") }) {
                            Text("Clear")
                        }
                        TextButton(onClick = { viewModel.setHyperlinkDialogVisible(false) }) {
                            Text("Cancel")
                        }
                    }
                }
            )
        }

        // Smart-Shape Context Actions (Actions.*) Authoring Dialog
        if (uiState.showSmartActionsDialog) {
            var rowNameDraft by remember { mutableStateOf("ToggleState") }
            var menuDraft by remember { mutableStateOf("Toggle Active Status") }
            var actionFormulaDraft by remember { mutableStateOf("SETF(GetRef(FillForegnd),\"RGB(220,252,231)\")") }
            var checkedFormulaDraft by remember { mutableStateOf("0") }
            var beginGroupDraft by remember { mutableStateOf(false) }

            AlertDialog(
                onDismissRequest = { viewModel.setSmartActionsDialogVisible(false) },
                title = { Text("Smart-Shape Context Actions (Actions.*) — Sheet.${uiState.selectedShapeId}") },
                text = {
                    Column(
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(max = 420.dp)
                            .verticalScroll(rememberScrollState())
                    ) {
                        if (uiState.selectedShapeActions.isNotEmpty()) {
                            uiState.selectedShapeActions.forEach { act ->
                                Surface(
                                    shape = RoundedCornerShape(8.dp),
                                    color = MaterialTheme.colorScheme.surfaceContainer,
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .clickable {
                                            rowNameDraft = act.rowName
                                            menuDraft = act.menu
                                            actionFormulaDraft = act.actionFormula
                                            checkedFormulaDraft = act.checkedFormula
                                            beginGroupDraft = act.beginGroup
                                        }
                                ) {
                                    Row(
                                        verticalAlignment = Alignment.CenterVertically,
                                        modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp)
                                    ) {
                                        Text(
                                            text = "${if (act.checked) "✓ " else "• "}${act.menu} (${act.rowName})",
                                            style = MaterialTheme.typography.bodySmall,
                                            modifier = Modifier.weight(1f)
                                        )
                                        TextButton(onClick = { viewModel.executeSmartAction(act.rowName) }) {
                                            Text("Run")
                                        }
                                        TextButton(onClick = { viewModel.deleteSmartAction(act.rowName) }) {
                                            Text("Delete")
                                        }
                                    }
                                }
                            }
                        }
                        OutlinedTextField(
                            value = rowNameDraft,
                            onValueChange = { rowNameDraft = it },
                            singleLine = true,
                            label = { Text("Row Name (Actions.<RowName>)") },
                            modifier = Modifier.fillMaxWidth()
                        )
                        OutlinedTextField(
                            value = menuDraft,
                            onValueChange = { menuDraft = it },
                            singleLine = true,
                            label = { Text("Right-Click Menu Label") },
                            modifier = Modifier.fillMaxWidth()
                        )
                        OutlinedTextField(
                            value = actionFormulaDraft,
                            onValueChange = { actionFormulaDraft = it },
                            textStyle = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                            label = { Text("Action Formula (SETF / GOTOPAGE / OPENTEXTWIN / RUNMACRO)") },
                            modifier = Modifier.fillMaxWidth()
                        )
                        OutlinedTextField(
                            value = checkedFormulaDraft,
                            onValueChange = { checkedFormulaDraft = it },
                            singleLine = true,
                            textStyle = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                            label = { Text("Checked Formula (0 / 1 / cell ref)") },
                            modifier = Modifier.fillMaxWidth()
                        )
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier.clickable { beginGroupDraft = !beginGroupDraft }
                        ) {
                            Checkbox(checked = beginGroupDraft, onCheckedChange = { beginGroupDraft = it })
                            Text("Begin separator group above item", style = MaterialTheme.typography.bodySmall)
                        }
                    }
                },
                confirmButton = {
                    FilledTonalButton(
                        onClick = {
                            viewModel.upsertSmartAction(
                                rowName = rowNameDraft,
                                menu = menuDraft,
                                actionFormula = actionFormulaDraft,
                                checkedFormula = checkedFormulaDraft,
                                beginGroup = beginGroupDraft
                            )
                        }
                    ) {
                        Text("Add / Update Action")
                    }
                },
                dismissButton = {
                    TextButton(onClick = { viewModel.setSmartActionsDialogVisible(false) }) {
                        Text("Done")
                    }
                }
            )
        }

        // Enterprise Domain Diagram Wizards Dialog
        if (uiState.showDomainWizardsDialog) {
            val wizardChoices = listOf(
                "gantt" to "Gantt",
                "timeline" to "Timeline",
                "pert" to "PERT",
                "sql_reverse" to "SQL → ERD",
                "sql_forward" to "ERD → SQL",
                "org_chart" to "Org Chart",
                "rack_42u" to "42U Rack",
                "arch_room" to "Room SqFt"
            )
            AlertDialog(
                onDismissRequest = { viewModel.setDomainWizardsDialogVisible(false) },
                title = { Text("Enterprise Domain Diagram Wizards") },
                text = {
                    Column(
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier
                            .fillMaxWidth()
                            .verticalScroll(rememberScrollState())
                    ) {
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                            modifier = Modifier
                                .fillMaxWidth()
                                .horizontalScroll(rememberScrollState())
                        ) {
                            wizardChoices.forEach { (slug, label) ->
                                FilterChip(
                                    selected = uiState.domainWizardSlug == slug,
                                    onClick = { viewModel.selectDomainWizardPreset(slug) },
                                    label = { Text(label) }
                                )
                            }
                        }
                        OutlinedTextField(
                            value = uiState.domainWizardSpec,
                            onValueChange = viewModel::setDomainWizardSpec,
                            textStyle = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                            minLines = 6,
                            maxLines = 10,
                            label = { Text("Wizard Specification (CSV / SQL DDL / Dialect)") },
                            modifier = Modifier.fillMaxWidth()
                        )
                        if (uiState.domainWizardSlug == "sql_forward" && !uiState.lastDomainWizardOutput.isNullOrBlank()) {
                            OutlinedTextField(
                                value = uiState.lastDomainWizardOutput.orEmpty(),
                                onValueChange = {},
                                readOnly = true,
                                textStyle = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                                minLines = 4,
                                maxLines = 8,
                                label = { Text("Generated SQL DDL") },
                                modifier = Modifier.fillMaxWidth()
                            )
                        }
                    }
                },
                confirmButton = {
                    FilledTonalButton(onClick = { viewModel.runDomainWizard() }) {
                        Text("Run Wizard")
                    }
                },
                dismissButton = {
                    TextButton(onClick = { viewModel.setDomainWizardsDialogVisible(false) }) {
                        Text("Close")
                    }
                }
            )
        }

        // Presentation Mode Slide Snippets Dialog
        if (uiState.showSlideSnippetsDialog) {
            AlertDialog(
                onDismissRequest = { viewModel.setSlideSnippetsDialogVisible(false) },
                title = { Text("Presentation Mode — Slide Snippets") },
                text = {
                    Column(
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(max = 360.dp)
                            .verticalScroll(rememberScrollState())
                    ) {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            FilledTonalButton(onClick = { viewModel.autoGenerateSlideSnippets() }) {
                                Text("Auto-Generate Slides")
                            }
                            if (uiState.slideSnippets.isNotEmpty()) {
                                OutlinedButton(onClick = { viewModel.deleteSlideSnippet(0) }) {
                                    Text("Clear All")
                                }
                            }
                        }
                        if (uiState.slideSnippets.isEmpty()) {
                            Text(
                                text = "Tap Auto-Generate Slides to create presenter frames from Containers or diagram stages.",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        } else {
                            uiState.slideSnippets.forEachIndexed { idx, s ->
                                Surface(
                                    shape = RoundedCornerShape(10.dp),
                                    color = MaterialTheme.colorScheme.surfaceContainerLow,
                                    border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                                    modifier = Modifier.fillMaxWidth()
                                ) {
                                    Row(
                                        verticalAlignment = Alignment.CenterVertically,
                                        horizontalArrangement = Arrangement.SpaceBetween,
                                        modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp)
                                    ) {
                                        Column(modifier = Modifier.weight(1f)) {
                                            Text("#${idx + 1} · ${s.title}", style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.SemiBold)
                                            if (s.notes.isNotBlank()) {
                                                Text(s.notes, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                            }
                                        }
                                        TextButton(onClick = { viewModel.focusSlideSnippet(idx, enterPresenter = true) }) {
                                            Text("Present")
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                confirmButton = {
                    if (uiState.slideSnippets.isNotEmpty()) {
                        FilledTonalButton(onClick = { viewModel.focusSlideSnippet(0, enterPresenter = true) }) {
                            Text("Start Presentation ▶")
                        }
                    }
                },
                dismissButton = {
                    TextButton(onClick = { viewModel.setSlideSnippetsDialogVisible(false) }) {
                        Text("Close")
                    }
                }
            )
        }

        // Floating Presentation Mode Stepper Bar
        if (uiState.isPresentationModeActive && uiState.slideSnippets.isNotEmpty()) {
            val safeIdx = uiState.activeSlideIndex.coerceIn(0, uiState.slideSnippets.lastIndex)
            val curSlide = uiState.slideSnippets[safeIdx]
            Box(
                contentAlignment = Alignment.BottomCenter,
                modifier = Modifier
                    .fillMaxSize()
                    .padding(bottom = 52.dp)
            ) {
                Surface(
                    shape = RoundedCornerShape(999.dp),
                    color = MaterialTheme.colorScheme.inverseSurface,
                    contentColor = MaterialTheme.colorScheme.inverseOnSurface,
                    tonalElevation = 8.dp
                ) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier.padding(horizontal = 14.dp, vertical = 6.dp)
                    ) {
                        TextButton(onClick = { viewModel.focusSlideSnippet(safeIdx - 1, enterPresenter = true) }) {
                            Text("◀ Prev", color = MaterialTheme.colorScheme.inverseOnSurface)
                        }
                        Text(
                            text = "Slide ${safeIdx + 1}/${uiState.slideSnippets.size}: ${curSlide.title}",
                            style = MaterialTheme.typography.labelLarge,
                            fontWeight = FontWeight.SemiBold
                        )
                        TextButton(onClick = { viewModel.focusSlideSnippet(safeIdx + 1, enterPresenter = true) }) {
                            Text("Next ▶", color = MaterialTheme.colorScheme.inverseOnSurface)
                        }
                        TextButton(onClick = { viewModel.setPresentationModeActive(false) }) {
                            Text("Exit ✕", color = MaterialTheme.colorScheme.inversePrimary)
                        }
                    }
                }
            }
        }

        // Official & Enterprise Stencil Catalog Dialog (.vssx, .vss, .zip SVG)
        if (uiState.showOfficialStencilCatalogDialog) {
            val lang = uiState.language
            AlertDialog(
                onDismissRequest = { viewModel.setOfficialStencilCatalogDialogVisible(false) },
                title = {
                    Text(VexioI18n.officialCatalogTitle(lang))
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
                            text = VexioI18n.officialCatalogSubtitle(lang),
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
                                                isBusy -> VexioI18n.installingLabel(lang)
                                                isInstalled -> VexioI18n.installedBadge(lang)
                                                else -> VexioI18n.installButton(lang)
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
                        Text(VexioI18n.installAllPacksButton(lang))
                    }
                },
                dismissButton = {
                    TextButton(onClick = { viewModel.setOfficialStencilCatalogDialogVisible(false) }) {
                        Text(VexioI18n.closeButton(lang))
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
private fun CompactHeaderIconButton(
    icon: ImageVector,
    tooltip: String,
    enabled: Boolean = true,
    active: Boolean = false,
    onClick: () -> Unit
) {
    val bgColor = when {
        !enabled -> Color.Transparent
        active -> MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.72f)
        else -> Color.Transparent
    }
    val tintColor = when {
        !enabled -> MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f)
        active -> MaterialTheme.colorScheme.primary
        else -> MaterialTheme.colorScheme.onSurfaceVariant
    }
    VexioTooltip(text = tooltip) {
        Box(
            contentAlignment = Alignment.Center,
            modifier = Modifier
                .size(28.dp)
                .clip(RoundedCornerShape(7.dp))
                .background(bgColor)
                .then(
                    if (enabled) Modifier.clickable(onClick = onClick) else Modifier
                )
        ) {
            Icon(
                imageVector = icon,
                contentDescription = tooltip,
                tint = tintColor,
                modifier = Modifier.size(16.dp)
            )
        }
    }
}

@Composable
private fun CompactHeaderChipButton(
    label: String,
    tooltip: String = "",
    leadingIcon: ImageVector? = null,
    onClick: () -> Unit
) {
    val chipContent: @Composable () -> Unit = {
        Surface(
            shape = RoundedCornerShape(7.dp),
            color = MaterialTheme.colorScheme.surfaceContainerHighest.copy(alpha = 0.6f),
            border = BorderStroke(0.75.dp, MaterialTheme.colorScheme.outlineVariant),
            modifier = Modifier
                .height(26.dp)
                .clip(RoundedCornerShape(7.dp))
                .clickable(onClick = onClick)
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                modifier = Modifier.padding(horizontal = 8.dp)
            ) {
                if (leadingIcon != null) {
                    Icon(
                        imageVector = leadingIcon,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(13.dp)
                    )
                }
                Text(
                    text = label,
                    fontSize = 11.5.sp,
                    fontWeight = FontWeight.Medium,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    softWrap = false
                )
            }
        }
    }
    if (tooltip.isNotBlank()) {
        VexioTooltip(text = tooltip) { chipContent() }
    } else {
        chipContent()
    }
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
                    val ev = awaitPointerEvent(PointerEventPass.Initial)
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
        tonalElevation = 1.dp,
        modifier = Modifier
            .fillMaxWidth()
            .height(36.dp)
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(3.dp),
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 10.dp)
        ) {
            // Compact Brand Pill (toggles Start / File Backstage)
            VexioTooltip(text = VexioI18n.startHubMenu(lang)) {
                Surface(
                    shape = RoundedCornerShape(6.dp),
                    color = MaterialTheme.colorScheme.primaryContainer,
                    modifier = Modifier
                        .height(23.dp)
                        .clip(RoundedCornerShape(6.dp))
                        .clickable(onClick = onToggleBackstage)
                ) {
                    Box(contentAlignment = Alignment.Center, modifier = Modifier.padding(horizontal = 8.dp)) {
                        Text(
                            text = "VEXIO",
                            fontSize = 10.5.sp,
                            fontWeight = FontWeight.Bold,
                            letterSpacing = 0.6.sp,
                            color = MaterialTheme.colorScheme.onPrimaryContainer,
                            maxLines = 1,
                            softWrap = false
                        )
                    }
                }
            }

            Spacer(Modifier.width(4.dp))

            // Document Title + Micro Format/Unit Badge (Single clean desktop row!)
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.weight(1f)
            ) {
                if (isEditingTitle) {
                    Box(
                        contentAlignment = Alignment.CenterStart,
                        modifier = Modifier
                            .height(25.dp)
                            .clip(RoundedCornerShape(6.dp))
                            .background(MaterialTheme.colorScheme.surfaceContainerLowest)
                            .border(1.5.dp, MaterialTheme.colorScheme.primary, RoundedCornerShape(6.dp))
                            .padding(horizontal = 7.dp)
                    ) {
                        BasicTextField(
                            value = titleFieldValue,
                            onValueChange = { titleFieldValue = it },
                            singleLine = true,
                            textStyle = MaterialTheme.typography.labelLarge.copy(
                                fontSize = 12.5.sp,
                                color = MaterialTheme.colorScheme.onSurface
                            ),
                            cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                            modifier = Modifier
                                .width(200.dp)
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
                    CompactHeaderIconButton(
                        icon = Icons.Default.Check,
                        tooltip = VexioI18n.applyButton(lang),
                        onClick = {
                            onRenameDocument(titleFieldValue.text)
                            isEditingTitle = false
                        }
                    )
                } else {
                    Text(
                        text = buildString {
                            append(uiState.documentTitle)
                            if (uiState.isDirty) append(" •")
                        },
                        fontSize = 12.5.sp,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.onSurface,
                        maxLines = 1,
                        softWrap = false,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier
                            .clip(RoundedCornerShape(5.dp))
                            .combinedClickable(
                                onClick = {},
                                onDoubleClick = { isEditingTitle = true },
                                onLongClick = { isEditingTitle = true }
                            )
                            .padding(horizontal = 5.dp, vertical = 2.dp)
                    )
                }
                Spacer(Modifier.width(5.dp))
                Surface(
                    shape = RoundedCornerShape(5.dp),
                    color = MaterialTheme.colorScheme.secondaryContainer.copy(alpha = 0.65f)
                ) {
                    Text(
                        text = "${uiState.documentExtension.uppercase()} • ${if (uiState.metricUnits) "MM" else "IN"}",
                        fontSize = 9.5.sp,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.onSecondaryContainer,
                        maxLines = 1,
                        softWrap = false,
                        modifier = Modifier.padding(horizontal = 5.dp, vertical = 2.dp)
                    )
                }
            }

            // Compact Quick-Access Icon Strip (28dp uniform desktop buttons + discreet tooltips)
            CompactHeaderIconButton(
                icon = Icons.AutoMirrored.Filled.Undo,
                tooltip = VexioI18n.undoTooltip(lang),
                enabled = uiState.canUndo,
                onClick = onUndoClick
            )
            CompactHeaderIconButton(
                icon = Icons.AutoMirrored.Filled.Redo,
                tooltip = VexioI18n.redoTooltip(lang),
                enabled = uiState.canRedo,
                onClick = onRedoClick
            )
            CompactHeaderIconButton(
                icon = Icons.Default.FolderOpen,
                tooltip = lang.openFileLabel,
                onClick = onOpenClick
            )
            CompactHeaderIconButton(
                icon = Icons.Default.Save,
                tooltip = lang.saveLabel,
                onClick = onSaveClick
            )
            CompactHeaderIconButton(
                icon = Icons.Default.Print,
                tooltip = VexioI18n.printPdfLabel(lang),
                onClick = onPrintClick
            )

            VerticalDivider(
                modifier = Modifier
                    .height(18.dp)
                    .padding(horizontal = 3.dp),
                color = MaterialTheme.colorScheme.outlineVariant
            )

            // Compact Export & Save As Menu (26dp height — zero bloat!)
            Box(modifier = Modifier.hoverSwitchSiblingMenu("export")) {
                CompactHeaderChipButton(
                    label = VexioI18n.exportMenuButton(lang),
                    onClick = {
                        openSiblingMenuId = if (openSiblingMenuId == "export") null else "export"
                    }
                )
                DropdownMenu(
                    expanded = openSiblingMenuId == "export",
                    onDismissRequest = { if (openSiblingMenuId == "export") openSiblingMenuId = null }
                ) {
                    DropdownMenuItem(
                        text = { Text(lang.saveAsLabel) },
                        leadingIcon = { Icon(Icons.Default.Save, contentDescription = null, modifier = Modifier.size(16.dp)) },
                        onClick = { openSiblingMenuId = null; onSaveAsClick() }
                    )
                    HorizontalDivider()
                    DropdownMenuItem(
                        text = { Text(VexioI18n.exportJpgItem(lang)) },
                        onClick = { openSiblingMenuId = null; onExportJpgClick() }
                    )
                    DropdownMenuItem(
                        text = { Text(VexioI18n.exportPdfItem(lang)) },
                        onClick = { openSiblingMenuId = null; onExportFormat("pdf") }
                    )
                    DropdownMenuItem(
                        text = { Text(VexioI18n.exportSvgItem(lang)) },
                        onClick = { openSiblingMenuId = null; onExportFormat("svg") }
                    )
                    DropdownMenuItem(
                        text = { Text(VexioI18n.exportDxfItem(lang)) },
                        onClick = { openSiblingMenuId = null; onExportFormat("dxf") }
                    )
                    DropdownMenuItem(
                        text = { Text("Export AutoCAD DWG (.dwg)") },
                        onClick = { openSiblingMenuId = null; onExportFormat("dwg") }
                    )
                }
            }

            // Compact Language Selector (26dp height)
            Box(modifier = Modifier.hoverSwitchSiblingMenu("lang")) {
                CompactHeaderChipButton(
                    label = "${uiState.language.badge} ▾",
                    tooltip = uiState.language.nativeName,
                    onClick = {
                        openSiblingMenuId = if (openSiblingMenuId == "lang") null else "lang"
                    }
                )
                DropdownMenu(
                    expanded = openSiblingMenuId == "lang",
                    onDismissRequest = { if (openSiblingMenuId == "lang") openSiblingMenuId = null }
                ) {
                    VexioAppLanguage.entries.forEach { l ->
                        DropdownMenuItem(
                            text = { Text("${l.badge} — ${l.nativeName}") },
                            leadingIcon = {
                                if (uiState.language == l) {
                                    Icon(Icons.Default.Check, contentDescription = null, modifier = Modifier.size(16.dp))
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

            if (posture != AdaptiveWorkstationPosture.COMPACT_CONVERTIBLE) {
                CompactHeaderIconButton(
                    icon = Icons.Default.OpenInNew,
                    tooltip = VexioI18n.newWindowButton(lang),
                    onClick = onNewWindowClick
                )
            }

            CompactHeaderIconButton(
                icon = if (uiState.isDarkTheme) Icons.Default.LightMode else Icons.Default.DarkMode,
                tooltip = if (uiState.isDarkTheme) "Light Mode" else "Dark Mode",
                onClick = onToggleTheme
            )

            // Compact More / Updates / Stencil Import Menu (28dp icon button)
            Box(modifier = Modifier.hoverSwitchSiblingMenu("help")) {
                CompactHeaderIconButton(
                    icon = Icons.Default.MoreVert,
                    tooltip = lang.checkUpdatesLabel,
                    onClick = {
                        openSiblingMenuId = if (openSiblingMenuId == "help") null else "help"
                    }
                )
                DropdownMenu(
                    expanded = openSiblingMenuId == "help",
                    onDismissRequest = { if (openSiblingMenuId == "help") openSiblingMenuId = null }
                ) {
                    DropdownMenuItem(
                        text = { Text(VexioI18n.blankIsoA4Short(lang)) },
                        leadingIcon = { Icon(Icons.Default.Add, contentDescription = null, modifier = Modifier.size(16.dp)) },
                        onClick = { openSiblingMenuId = null; onCreateBlankA4() }
                    )
                    DropdownMenuItem(
                        text = { Text(uiState.language.importStencilLabel) },
                        leadingIcon = { Icon(Icons.Default.FolderOpen, contentDescription = null, modifier = Modifier.size(16.dp)) },
                        onClick = { openSiblingMenuId = null; onImportStencilClick() }
                    )
                    DropdownMenuItem(
                        text = { Text(uiState.language.checkUpdatesLabel) },
                        onClick = { openSiblingMenuId = null; onCheckUpdates() }
                    )
                    HorizontalDivider()
                    DropdownMenuItem(
                        text = { Text(VexioI18n.startHubMenu(lang)) },
                        onClick = { openSiblingMenuId = null; onToggleBackstage() }
                    )
                }
            }

            VerticalDivider(
                modifier = Modifier
                    .height(18.dp)
                    .padding(horizontal = 3.dp),
                color = MaterialTheme.colorScheme.outlineVariant
            )

            // Workspace Pane Toggles (28dp compact icons highlighted when active!)
            CompactHeaderIconButton(
                icon = Icons.Default.Schema,
                tooltip = VexioI18n.toggleStencilsTooltip(lang),
                active = uiState.isStencilDrawerExpanded,
                onClick = onToggleStencilDrawer
            )
            CompactHeaderIconButton(
                icon = Icons.Default.TableChart,
                tooltip = VexioI18n.toggleShapeSheetTooltip(lang),
                active = uiState.isShapeSheetDrawerExpanded,
                onClick = onToggleShapeSheet
            )
            if (posture != AdaptiveWorkstationPosture.COMPACT_CONVERTIBLE) {
                CompactHeaderIconButton(
                    icon = Icons.Default.ViewSidebar,
                    tooltip = VexioI18n.toggleInspectorTooltip(lang),
                    active = uiState.isInspectorExpanded,
                    onClick = onToggleInspector
                )
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
    onLinkExternalDataClick: () -> Unit = {},
    onRefreshExternalDataClick: () -> Unit = {},
    onTwoWayWritebackClick: () -> Unit = {},
    onExportBomReportClick: () -> Unit = {},
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
                            var headingMenuOpen by remember { mutableStateOf(false) }
                            Box {
                                ExpressivePlan2ActionPill(
                                    label = "Headings ▾",
                                    selected = false,
                                    onClick = { headingMenuOpen = true }
                                )
                                DropdownMenu(
                                    expanded = headingMenuOpen,
                                    onDismissRequest = { headingMenuOpen = false }
                                ) {
                                    listOf(
                                        "title" to "Diagram Title (24 pt Bold)",
                                        "subtitle" to "Subtitle (14 pt)",
                                        "h1" to "Heading 1 — Underlined (18 pt)",
                                        "h2" to "Heading 2 (14 pt Bold)",
                                        "h3" to "Heading 3 (12 pt Accent)",
                                        "section_banner" to "Section Header Banner",
                                        "callout_title" to "Accent Callout Header",
                                        "body" to "Body Text (11 pt)",
                                        "caption" to "Caption / Note (9.5 pt)"
                                    ).forEach { (slug, label) ->
                                        DropdownMenuItem(
                                            text = { Text(label) },
                                            onClick = {
                                                headingMenuOpen = false
                                                viewModel.applyOrInsertHeading(slug, forceInsert = false)
                                            }
                                        )
                                    }
                                }
                            }

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
                                                    VexioI18n.filterFontsPlaceholder(uiState.availableFontFamilies.size, lang),
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
                                label = VexioI18n.bulletListPill(lang),
                                selected = uiState.textBullets,
                                onClick = { viewModel.applyRichTextStyle(bullets = !uiState.textBullets) }
                            )

                            VerticalDivider(modifier = Modifier.height(20.dp), color = MaterialTheme.colorScheme.outlineVariant)

                            // Arrange, Align, Distribute, Group, Rotate 90° & Flip for All Stencils
                            var alignMenuOpen by remember { mutableStateOf(false) }
                            var distributeMenuOpen by remember { mutableStateOf(false) }
                            Box {
                                ExpressivePlan2ActionPill(
                                    label = VexioI18n.alignMenuPill(lang),
                                    selected = false,
                                    onClick = { alignMenuOpen = true }
                                )
                                DropdownMenu(
                                    expanded = alignMenuOpen,
                                    onDismissRequest = { alignMenuOpen = false }
                                ) {
                                    listOf("left", "center", "right", "top", "middle", "bottom").forEach { modeSlug ->
                                        DropdownMenuItem(
                                            text = { Text(VexioI18n.alignOptionLabel(modeSlug, lang)) },
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
                                    label = VexioI18n.distributeMenuPill(lang),
                                    selected = false,
                                    onClick = { distributeMenuOpen = true }
                                )
                                DropdownMenu(
                                    expanded = distributeMenuOpen,
                                    onDismissRequest = { distributeMenuOpen = false }
                                ) {
                                    DropdownMenuItem(
                                        text = { Text(VexioI18n.distributeHorizontally(lang)) },
                                        onClick = {
                                            distributeMenuOpen = false
                                            viewModel.distributeSelectedShapes("horizontal")
                                        }
                                    )
                                    DropdownMenuItem(
                                        text = { Text(VexioI18n.distributeVertically(lang)) },
                                        onClick = {
                                            distributeMenuOpen = false
                                            viewModel.distributeSelectedShapes("vertical")
                                        }
                                    )
                                }
                            }
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.groupPill(lang),
                                selected = false,
                                onClick = { viewModel.groupSelectedShapes() }
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.ungroupPill(lang),
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
                                label = VexioI18n.flipHPill(lang),
                                selected = false,
                                onClick = { viewModel.rotateOrFlipSelected(flipX = true) }
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.flipVPill(lang),
                                selected = false,
                                onClick = { viewModel.rotateOrFlipSelected(flipY = true) }
                            )
                        }

                        "Insert" -> {
                            var insertHeadingMenuOpen by remember { mutableStateOf(false) }
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.newPagePill(lang),
                                selected = false,
                                onClick = viewModel::addNewPage
                            )
                            ExpressivePlan2ActionPill(
                                label = lang.importStencilLabel,
                                selected = true,
                                onClick = onImportStencilClick
                            )
                            Box {
                                ExpressivePlan2ActionPill(
                                    label = "+ Title / Heading ▾",
                                    selected = false,
                                    onClick = { insertHeadingMenuOpen = true }
                                )
                                DropdownMenu(
                                    expanded = insertHeadingMenuOpen,
                                    onDismissRequest = { insertHeadingMenuOpen = false }
                                ) {
                                    listOf(
                                        "title" to "Insert Diagram Title (24 pt)",
                                        "subtitle" to "Insert Subtitle (14 pt)",
                                        "h1" to "Insert Heading 1 — Underlined",
                                        "h2" to "Insert Heading 2",
                                        "h3" to "Insert Heading 3",
                                        "section_banner" to "Insert Section Header Banner",
                                        "callout_title" to "Insert Accent Callout Header"
                                    ).forEach { (slug, label) ->
                                        DropdownMenuItem(
                                            text = { Text(label) },
                                            onClick = {
                                                insertHeadingMenuOpen = false
                                                viewModel.applyOrInsertHeading(slug, forceInsert = true)
                                            }
                                        )
                                    }
                                }
                            }
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.textBoxPill(lang),
                                selected = uiState.activeTool == VexioTool.TEXT_BLOCK,
                                onClick = { viewModel.selectTool(VexioTool.TEXT_BLOCK) }
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.containerPill(lang),
                                selected = false,
                                onClick = { viewModel.insertContainer() }
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.swimlanesPill(lang),
                                selected = false,
                                onClick = { viewModel.insertSwimlanes() }
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.calloutPill(lang),
                                selected = false,
                                onClick = { viewModel.attachCallout() }
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.subprocessPill(lang),
                                selected = false,
                                onClick = { viewModel.createOrLinkSubprocess() }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Hyperlink & Tip…",
                                selected = uiState.showHyperlinkDialog || uiState.hyperlinkAddress.isNotBlank(),
                                onClick = { viewModel.setHyperlinkDialogVisible(true) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Diagram Wizards…",
                                selected = uiState.showDomainWizardsDialog,
                                onClick = { viewModel.setDomainWizardsDialogVisible(true) }
                            )
                            ExpressivePlan2ActionPill(
                                label = if (uiState.cadScaleLocked) "CAD Scale Locked (${uiState.cadScaleSlug})" else "Lock CAD Scale",
                                selected = uiState.cadScaleLocked,
                                onClick = {
                                    val slug = if (uiState.metricUnits) "1:100" else "1/4_in_ft"
                                    viewModel.applyCadArchitecturalScale(slug, !uiState.cadScaleLocked)
                                }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Convert CAD Layers",
                                selected = false,
                                onClick = { viewModel.convertCadLayersToShapeSheet("*") }
                            )
                        }

                        "Design" -> {
                            var bordersMenuOpen by remember { mutableStateOf(false) }
                            Box {
                                ExpressivePlan2ActionPill(
                                    label = VexioI18n.pageSizePrefix(VexioI18n.pagePresetLabel(uiState.activePagePreset, lang), lang),
                                    selected = true,
                                    onClick = { pageSizeMenuOpen = true }
                                )
                                DropdownMenu(expanded = pageSizeMenuOpen, onDismissRequest = { pageSizeMenuOpen = false }) {
                                    VexioPagePresetSpec.entries.forEach { preset ->
                                        DropdownMenuItem(
                                            text = { Text(VexioI18n.pagePresetLabel(preset, lang)) },
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
                            Box {
                                ExpressivePlan2ActionPill(
                                    label = "Borders & Titles ▾",
                                    selected = false,
                                    onClick = { bordersMenuOpen = true }
                                )
                                DropdownMenu(
                                    expanded = bordersMenuOpen,
                                    onDismissRequest = { bordersMenuOpen = false }
                                ) {
                                    listOf(
                                        "modern_banner" to "Modern Header Banner",
                                        "executive_underline" to "Executive Title & Accent Rule",
                                        "classic_frame" to "Classic Frame & Title",
                                        "pill_banner" to "Rounded Pill Banner",
                                        "iso_engineering" to "ISO 7200 / ANSI Engineering Block",
                                        "none" to "None (Remove Border & Title)"
                                    ).forEach { (slug, label) ->
                                        DropdownMenuItem(
                                            text = { Text(label) },
                                            onClick = {
                                                bordersMenuOpen = false
                                                viewModel.applyBorderAndTitle(slug)
                                            }
                                        )
                                    }
                                }
                            }
                            VexioThemePreset.entries.forEach { preset ->
                                ExpressivePlan2ActionPill(
                                    label = VexioI18n.themePresetName(preset, lang),
                                    selected = uiState.activeThemeSlug == preset.slug,
                                    onClick = { viewModel.applyDocumentTheme(preset.slug, uiState.activeThemeVariant) }
                                )
                            }
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.autoLayoutPill(lang),
                                selected = false,
                                onClick = { viewModel.autoLayoutActivePage() }
                            )
                        }

                        "Data" -> {
                            ExpressivePlan2ActionPill(
                                label = "Link Data (.xlsx / .csv / .sqlite / .json)",
                                selected = uiState.externalRecordsets.isNotEmpty(),
                                onClick = onLinkExternalDataClick
                            )
                            ExpressivePlan2ActionPill(
                                label = "Refresh Data",
                                selected = false,
                                onClick = onRefreshExternalDataClick
                            )
                            ExpressivePlan2ActionPill(
                                label = "Two-Way Writeback",
                                selected = false,
                                onClick = onTwoWayWritebackClick
                            )
                            ExpressivePlan2ActionPill(
                                label = "Data Graphics",
                                selected = uiState.dataGraphics.enabled,
                                onClick = {
                                    val dg = uiState.dataGraphics
                                    viewModel.updatePageDataGraphics(dg.copy(enabled = !dg.enabled))
                                }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Insert Legend",
                                selected = uiState.dataGraphics.legendVisible,
                                onClick = { viewModel.insertOrUpdateDataLegend() }
                            )
                            ExpressivePlan2ActionPill(
                                label = "BOM Report (.xlsx)",
                                selected = false,
                                onClick = onExportBomReportClick
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.dataVisualizerPill(lang),
                                selected = uiState.lastDataVisualizerShapeCount > 0,
                                onClick = {
                                    viewModel.importDataVisualizer(
                                        Plan2SamplePayloads.dataVisualizerCsv.toByteArray(Charsets.UTF_8)
                                    )
                                }
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.cloudDiscoveryPill(lang),
                                selected = uiState.lastCloudDiscoveryShapeCount > 0,
                                onClick = { viewModel.importCloudDiscovery(Plan2SamplePayloads.cloudDiscoveryJson) }
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.entraOrgChartPill(lang),
                                selected = uiState.lastOrgChartShapeCount > 0,
                                onClick = { viewModel.importEntraOrLdifOrgChart(Plan2SamplePayloads.entraOrgChartLdif) }
                            )
                        }

                        "Process" -> {
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.validateBpmnPill(lang),
                                selected = uiState.validationSummary.standard == "BPMN 2.0",
                                onClick = { viewModel.validateActiveDiagram("bpmn") }
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.validateUmlPill(lang),
                                selected = uiState.validationSummary.standard == "UML 2.5",
                                onClick = { viewModel.validateActiveDiagram("uml") }
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.createSubprocessDrillDown(lang),
                                selected = false,
                                onClick = { viewModel.createOrLinkSubprocess() }
                            )
                        }

                        "Review" -> {
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.markupPenPill(lang),
                                selected = uiState.activeTool == VexioTool.STYLUS_INK,
                                onClick = { viewModel.selectTool(VexioTool.STYLUS_INK) }
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.highlighterPill(lang),
                                selected = uiState.activeTool == VexioTool.HIGHLIGHTER,
                                onClick = { viewModel.selectTool(VexioTool.HIGHLIGHTER) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Ink Eraser",
                                selected = uiState.activeTool == VexioTool.INK_ERASER,
                                onClick = { viewModel.selectTool(VexioTool.INK_ERASER) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Clear Ink",
                                selected = false,
                                onClick = { viewModel.clearActivePageInk() }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Comments (${uiState.reviewComments.size})",
                                selected = uiState.showCommentsDialog,
                                onClick = { viewModel.setCommentsDialogVisible(true) }
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.diagramComparePill(lang),
                                selected = uiState.lastCompareSummary != null,
                                onClick = { viewModel.compareWithBaseline() }
                            )
                        }

                        "View" -> {
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.smartGuidesPill(lang),
                                selected = uiState.smartGuidesEnabled,
                                onClick = { viewModel.toggleSmartGuides() }
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.stencilsNavigatorPill(lang),
                                selected = uiState.isStencilDrawerExpanded,
                                onClick = viewModel::toggleStencilDrawer
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.inspectorPanePill(lang),
                                selected = uiState.isInspectorExpanded,
                                onClick = viewModel::toggleInspectorPane
                            )
                            ExpressivePlan2ActionPill(
                                label = "ShapeSheet (Shift+F5)",
                                selected = uiState.isShapeSheetDrawerExpanded,
                                onClick = viewModel::toggleShapeSheetDrawer
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.zoom100Pill(lang),
                                selected = uiState.zoomPercent == 100,
                                onClick = { viewModel.setZoomPercent(100) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Slide Snippets (${uiState.slideSnippets.size})",
                                selected = uiState.showSlideSnippetsDialog,
                                onClick = { viewModel.setSlideSnippetsDialogVisible(true) }
                            )
                            ExpressivePlan2ActionPill(
                                label = if (uiState.isPresentationModeActive) "Exit Present ✕" else "Present ▶",
                                selected = uiState.isPresentationModeActive,
                                onClick = { viewModel.setPresentationModeActive(!uiState.isPresentationModeActive) }
                            )
                        }

                        "Developer" -> {
                            ExpressivePlan2ActionPill(
                                label = "ShapeSheet Studio",
                                selected = uiState.isShapeSheetDrawerExpanded,
                                onClick = viewModel::toggleShapeSheetDrawer
                            )
                            ExpressivePlan2ActionPill(
                                label = "Mermaid / PlantUML / DOT…",
                                selected = uiState.showMarkupStudioDialog,
                                onClick = { viewModel.setMarkupStudioDialogVisible(true) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "VBA Macros (Alt+F9)",
                                selected = uiState.showVbaMacroDialog,
                                onClick = { viewModel.setVbaMacroDialogVisible(true) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Edit Vertices",
                                selected = uiState.activeTool == VexioTool.EDIT_VERTICES,
                                onClick = {
                                    viewModel.selectTool(
                                        if (uiState.activeTool == VexioTool.EDIT_VERTICES) VexioTool.POINTER else VexioTool.EDIT_VERTICES
                                    )
                                }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Conn. Points (Ctrl+Shift+1)",
                                selected = uiState.activeTool == VexioTool.CONNECTION_POINT,
                                onClick = {
                                    viewModel.selectTool(
                                        if (uiState.activeTool == VexioTool.CONNECTION_POINT) VexioTool.POINTER else VexioTool.CONNECTION_POINT
                                    )
                                }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Smart Actions (${uiState.selectedShapeActions.size})",
                                selected = uiState.showSmartActionsDialog,
                                onClick = { viewModel.setSmartActionsDialogVisible(true) }
                            )
                            ExpressivePlan2ActionPill(
                                label = "+ Control Handle",
                                selected = false,
                                onClick = { viewModel.ensureSelectedControlHandle() }
                            )
                            ExpressivePlan2ActionPill(
                                label = "Reset Route",
                                selected = false,
                                onClick = { viewModel.resetSelectedConnectorRouting() }
                            )
                            ExpressivePlan2ActionPill(
                                label = VexioI18n.plan2StripPill(lang),
                                selected = uiState.isPlan2RibbonExpanded,
                                onClick = onTogglePlan2Strip
                            )
                            VexioBooleanOp.entries.forEach { op ->
                                ExpressivePlan2ActionPill(
                                    label = VexioI18n.booleanOpName(op, lang),
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
    onCreateNewBlank: () -> Unit,
    onLoadTemplate: (String) -> Unit,
    onOpenFile: () -> Unit,
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
            // Focused Startup Rail (Open, New Blank, Continue, Updates — zero export/stencil/page clutter)
            Surface(
                color = MaterialTheme.colorScheme.surfaceContainerHigh,
                modifier = Modifier
                    .width(304.dp)
                    .fillMaxHeight()
            ) {
                Column(
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                    modifier = Modifier
                        .fillMaxSize()
                        .verticalScroll(rememberScrollState())
                        .padding(18.dp)
                ) {
                    FilledTonalButton(
                        onClick = onClose,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Text("← ${lang.continueCanvasLabel}", maxLines = 1, softWrap = false)
                    }
                    Spacer(Modifier.height(4.dp))
                    Text("VEXIO PROFESSIONAL", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                    Text(VexioI18n.suiteSubtitle(lang), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    HorizontalDivider()
                    Button(
                        onClick = onOpenFile,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Text(lang.openFileLabel, maxLines = 1, softWrap = false)
                    }
                    FilledTonalButton(
                        onClick = onCreateNewBlank,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Text(VexioI18n.blankIsoA4Short(lang), maxLines = 1, softWrap = false)
                    }
                    Spacer(Modifier.weight(1f))
                    HorizontalDivider()
                    OutlinedButton(onClick = onCheckUpdates, modifier = Modifier.fillMaxWidth()) {
                        Text(lang.checkUpdatesLabel, maxLines = 1, softWrap = false)
                    }
                }
            }

            // Main Startup Content: Hero + Single New Blank Card + Starter Templates + Language & Updates
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
                    VexioI18n.startHeroSubtitle(lang),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )

                Surface(
                    shape = RoundedCornerShape(16.dp),
                    color = MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.42f),
                    border = BorderStroke(1.dp, MaterialTheme.colorScheme.primary.copy(alpha = 0.35f)),
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(16.dp))
                        .clickable { onCreateNewBlank() }
                ) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween,
                        modifier = Modifier.padding(18.dp)
                    ) {
                        Column(verticalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.weight(1f)) {
                            Text(VexioI18n.blankIsoA4Short(lang), style = MaterialTheme.typography.titleMedium)
                            Text(
                                VexioI18n.suiteSubtitle(lang),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                        Text("→", style = MaterialTheme.typography.titleLarge, color = MaterialTheme.colorScheme.primary)
                    }
                }

                Text(VexioI18n.starterTemplatesHeading(lang), style = MaterialTheme.typography.titleMedium)
                Row(
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    modifier = Modifier.horizontalScroll(rememberScrollState())
                ) {
                    listOf("flowchart", "bpmn", "network", "floorplan", "orgchart", "uml").forEach { slug ->
                        Surface(
                            shape = RoundedCornerShape(14.dp),
                            color = MaterialTheme.colorScheme.surfaceContainer,
                            border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                            modifier = Modifier
                                .clip(RoundedCornerShape(14.dp))
                                .clickable { onLoadTemplate(slug) }
                        ) {
                            Text(
                                text = VexioI18n.templateTitle(slug, lang),
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
                        Text(VexioI18n.languageAndUpdatesHeading(lang), style = MaterialTheme.typography.titleSmall)
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
                            Text(VexioI18n.autoCheckUpdatesCheckbox(lang), style = MaterialTheme.typography.bodySmall)
                            Spacer(Modifier.width(12.dp))
                            Text(
                                text = VexioI18n.localizeUpdateStatus(
                                    uiState.updateStatusMessage,
                                    uiState.availableUpdateTag,
                                    lang
                                ),
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
    val lang = uiState.language
    var themeMenuExpanded by remember { mutableStateOf(false) }
    var exportMenuExpanded by remember { mutableStateOf(false) }
    val activeThemeDisplay = remember(uiState.activeThemeSlug, lang) {
        VexioThemePreset.fromSlug(uiState.activeThemeSlug)?.let { VexioI18n.themePresetName(it, lang) }
            ?: VexioI18n.themePresetName(VexioThemePreset.MODERN, lang)
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
                        text = VexioI18n.booleanGroupTitle(lang),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(horizontal = 4.dp)
                    )
                    VexioBooleanOp.entries.forEach { op ->
                        val active = uiState.lastBooleanOp == op.slug
                        ExpressivePlan2ActionPill(
                            label = VexioI18n.booleanOpName(op, lang),
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
                            label = VexioI18n.themeLabelPrefix(activeThemeDisplay, lang),
                            selected = true,
                            onClick = { themeMenuExpanded = true }
                        )
                        DropdownMenu(
                            expanded = themeMenuExpanded,
                            onDismissRequest = { themeMenuExpanded = false }
                        ) {
                            VexioThemePreset.entries.forEach { preset ->
                                DropdownMenuItem(
                                    text = { Text("${VexioI18n.themePresetName(preset, lang)} (${VexioI18n.variantsSuffix(lang)})") },
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
                            label = VexioI18n.visualPresetName(preset, lang),
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
                Text(VexioI18n.applyButton(uiState.language), style = MaterialTheme.typography.labelMedium)
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
    VexioTool.EDIT_VERTICES -> Icons.Default.Tune
    VexioTool.INK_ERASER -> Icons.Default.DeleteOutline
    VexioTool.CONNECTION_POINT -> Icons.Default.Add
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
    onMasterSelected: (StencilMasterItem) -> Unit,
    onMasterDoubleClicked: (StencilMasterItem) -> Unit,
    onImportStencilRequested: () -> Unit = {},
    onOpenOfficialCatalogRequested: () -> Unit = {},
    onAddToMyShapesRequested: () -> Unit = {},
    onExportMyShapesVssxRequested: () -> Unit = {},
    onOpenNewWindowRequested: () -> Unit
) {
    val lang = uiState.language
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
                    text = VexioI18n.stencilsHeader(uiState.stencilMasters.size, uiState.stencilCategories.size, lang),
                    style = MaterialTheme.typography.titleSmall,
                    modifier = Modifier.weight(1f)
                )
                val allExpanded = expandedCategories.size >= uiState.stencilCategories.size
                Text(
                    text = if (allExpanded) VexioI18n.collapseLabel(lang) else VexioI18n.expandAllLabel(lang),
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
                        text = VexioI18n.addStencilShort(lang),
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
                        text = VexioI18n.officialPacksShort(lang),
                        style = MaterialTheme.typography.labelSmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
            }

            // "My Shapes" Custom Stencil Authoring & .vssx Export Row
            Row(
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 10.dp, vertical = 2.dp)
            ) {
                FilledTonalButton(
                    onClick = onAddToMyShapesRequested,
                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 5.dp),
                    shape = RoundedCornerShape(10.dp),
                    modifier = Modifier.weight(1f)
                ) {
                    Text(
                        text = "+ Add to My Shapes",
                        style = MaterialTheme.typography.labelSmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
                FilledTonalButton(
                    onClick = onExportMyShapesVssxRequested,
                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 5.dp),
                    shape = RoundedCornerShape(10.dp)
                ) {
                    Text(
                        text = ".vssx ↑",
                        style = MaterialTheme.typography.labelSmall,
                        maxLines = 1
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
                            text = VexioI18n.searchStencilsPlaceholder(uiState.stencilMasters.size, uiState.stencilCategories.size, lang),
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
                            text = VexioI18n.localizeStencilCategory(cat, lang),
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
                    val localizedCat = VexioI18n.localizeStencilCategory(category, lang)
                    val mastersInCat = uiState.stencilMasters.filter { m ->
                        m.category == category && (
                            trimmedQuery.isEmpty() ||
                                m.title.contains(trimmedQuery, ignoreCase = true) ||
                                m.category.contains(trimmedQuery, ignoreCase = true) ||
                                localizedCat.contains(trimmedQuery, ignoreCase = true) ||
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
                                    .clickable {
                                        if (isExpanded) {
                                            expandedCategories = expandedCategories - category
                                            if (uiState.selectedStencilCategory == category) {
                                                onSelectCategory("All")
                                            }
                                        } else {
                                            expandedCategories = expandedCategories + category
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
                                        text = localizedCat,
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
                                    lang = lang,
                                    isHighlighted = uiState.highlightedStencilMasterId == master.masterId,
                                    onSelect = { onMasterSelected(master) },
                                    onDoubleClickAdd = { onMasterDoubleClicked(master) },
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

private val stencilPreviewBitmapCache = LruCache<String, ImageBitmap>(512)

private class StencilCardDragShadowBuilder(
    private val density: Float,
    private val title: String,
    private val previewBitmap: Bitmap?
) : View.DragShadowBuilder() {
    private val widthPx = (196f * density.coerceAtLeast(1f)).toInt().coerceAtLeast(120)
    private val heightPx = (52f * density.coerceAtLeast(1f)).toInt().coerceAtLeast(40)

    override fun onProvideShadowMetrics(outShadowSize: Point, outShadowTouchPoint: Point) {
        outShadowSize.set(widthPx, heightPx)
        outShadowTouchPoint.set(widthPx / 3, heightPx / 2)
    }

    override fun onDrawShadow(canvas: android.graphics.Canvas) {
        val d = density.coerceAtLeast(1f)
        val rect = RectF(1.5f * d, 1.5f * d, widthPx - 1.5f * d, heightPx - 1.5f * d)
        val bgPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.FILL
            color = android.graphics.Color.argb(244, 248, 250, 252)
        }
        val borderPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            strokeWidth = 1.8f * d
            color = android.graphics.Color.rgb(37, 99, 235)
        }
        canvas.drawRoundRect(rect, 11f * d, 11f * d, bgPaint)
        canvas.drawRoundRect(rect, 11f * d, 11f * d, borderPaint)

        val iconLeft = 8f * d
        val iconTop = 7f * d
        val iconW = 50f * d
        val iconH = heightPx - 14f * d
        val iconRect = RectF(iconLeft, iconTop, iconLeft + iconW, iconTop + iconH)
        val whitePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.FILL
            color = android.graphics.Color.WHITE
        }
        canvas.drawRoundRect(iconRect, 6f * d, 6f * d, whitePaint)

        val bmp = previewBitmap
        if (bmp != null && bmp.width > 0 && bmp.height > 0) {
            val pad = 3f * d
            val availW = (iconW - pad * 2f).coerceAtLeast(1f)
            val availH = (iconH - pad * 2f).coerceAtLeast(1f)
            val fit = minOf(availW / bmp.width.toFloat(), availH / bmp.height.toFloat())
            val dw = bmp.width * fit
            val dh = bmp.height * fit
            val dx = iconLeft + (iconW - dw) * 0.5f
            val dy = iconTop + (iconH - dh) * 0.5f
            canvas.drawBitmap(bmp, null, RectF(dx, dy, dx + dw, dy + dh), Paint(Paint.FILTER_BITMAP_FLAG))
        }

        val textPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = android.graphics.Color.rgb(15, 23, 42)
            textSize = 12.5f * d
            isFakeBoldText = true
        }
        val clippedTitle = if (title.length > 20) title.take(19) + "…" else title
        canvas.drawText(clippedTitle, iconLeft + iconW + 8f * d, heightPx * 0.57f, textPaint)
    }
}

private fun startStencilSystemDrag(
    hostView: View,
    masterTitle: String,
    jsonPayload: String,
    previewBitmap: Bitmap?,
    density: Float
): Boolean {
    val item = ClipData.Item(jsonPayload)
    val clipData = ClipData(
        masterTitle,
        arrayOf(VEXIO_MASTER_CLIP_MIME, ClipDescription.MIMETYPE_TEXT_PLAIN),
        item
    )
    val shadow = StencilCardDragShadowBuilder(density, masterTitle, previewBitmap)
    val startedGlobal = runCatching {
        hostView.startDragAndDrop(
            clipData,
            shadow,
            jsonPayload,
            View.DRAG_FLAG_GLOBAL or View.DRAG_FLAG_GLOBAL_URI_READ
        )
    }.getOrDefault(false)
    if (startedGlobal) return true
    return runCatching {
        hostView.startDragAndDrop(clipData, shadow, jsonPayload, 0)
    }.getOrDefault(false)
}

/**
 * Stencil Master card supporting:
 * - Single-click / tap: selects & highlights the master in the Stencil Navigator
 * - Double-click / double-tap: adds the stencil directly to the center of the visible diagram
 * - Immediate Mouse / Trackpad / Stylus / Touch Drag & Drop (`View.startDragAndDrop`) onto the canvas & across windows
 */
@Composable
private fun DraggableStencilMasterCard(
    windowInstanceId: String,
    master: StencilMasterItem,
    lang: VexioAppLanguage,
    isHighlighted: Boolean,
    onSelect: () -> Unit,
    onDoubleClickAdd: () -> Unit,
    onOpenNewWindowRequested: () -> Unit
) {
    var cardMenuExpanded by remember { mutableStateOf(false) }
    val hostView = LocalView.current
    val density = LocalDensity.current.density
    val primaryStroke = MaterialTheme.colorScheme.primary
    val decodedPngPreview = remember(master.masterId, master.pngDataUrl) {
        val prefix = "data:image/png;base64,"
        if (master.pngDataUrl.startsWith(prefix)) {
            val cacheKey = "${master.masterId}::${master.pngDataUrl.hashCode()}"
            stencilPreviewBitmapCache.get(cacheKey) ?: runCatching {
                val raw = Base64.decode(master.pngDataUrl.removePrefix(prefix), Base64.DEFAULT)
                BitmapFactory.decodeByteArray(raw, 0, raw.size)?.asImageBitmap()?.also { bmp ->
                    stencilPreviewBitmapCache.put(cacheKey, bmp)
                }
            }.getOrNull()
        } else {
            null
        }
    }
    val parsedSvgDoc = remember(master.svgPreview, primaryStroke, decodedPngPreview != null) {
        if (decodedPngPreview != null) {
            ParsedComposeSvgDoc(144f, 96f, emptyList())
        } else {
            parseStencilSvgDoc(master.svgPreview, primaryStroke)
        }
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

    Box(modifier = Modifier.fillMaxWidth()) {
        Surface(
            shape = RoundedCornerShape(12.dp),
            color = if (isHighlighted) {
                MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.42f)
            } else {
                MaterialTheme.colorScheme.surfaceContainerLowest
            },
            border = BorderStroke(
                width = if (isHighlighted) 1.5.dp else 1.dp,
                color = if (isHighlighted) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant
            ),
            modifier = Modifier
                .fillMaxWidth()
                .pointerInput(master.masterId) {
                    detectTapGestures(
                        onTap = { onSelect() },
                        onDoubleTap = { onDoubleClickAdd() }
                    )
                }
                .pointerInput(master.masterId, jsonPayload) {
                    var dragLaunched = false
                    detectDragGestures(
                        onDragStart = {
                            onSelect()
                            dragLaunched = startStencilSystemDrag(
                                hostView = hostView,
                                masterTitle = master.title,
                                jsonPayload = jsonPayload,
                                previewBitmap = runCatching { decodedPngPreview?.asAndroidBitmap() }.getOrNull(),
                                density = density
                            )
                        },
                        onDragEnd = { dragLaunched = false },
                        onDragCancel = { dragLaunched = false },
                        onDrag = { change, _ ->
                            if (dragLaunched) {
                                change.consume()
                            }
                        }
                    )
                }
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
                        text = VexioI18n.localizeStencilCategory(master.category, lang),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
                IconButton(
                    onClick = { cardMenuExpanded = true },
                    modifier = Modifier.size(28.dp)
                ) {
                    Icon(
                        imageVector = Icons.Default.MoreVert,
                        contentDescription = "Stencil Options",
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(16.dp)
                    )
                }
            }
        }

        DropdownMenu(
            expanded = cardMenuExpanded,
            onDismissRequest = { cardMenuExpanded = false }
        ) {
            DropdownMenuItem(
                text = { Text(VexioI18n.addMasterToPageMenu(master.title, lang)) },
                leadingIcon = { Icon(Icons.Default.Add, contentDescription = null) },
                onClick = {
                    cardMenuExpanded = false
                    onDoubleClickAdd()
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.openSideBySideWindowMenu(lang)) },
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
    onZoomReset: () -> Unit = {},
    onToggleFloating: () -> Unit,
    modifier: Modifier = Modifier
) {
    val lang = uiState.language
    val toolbarShape = rememberMorphingToolbarShape(isFloatingPill = isFloating)

    Surface(
        shape = toolbarShape,
        color = MaterialTheme.colorScheme.surfaceContainerHighest.copy(
            alpha = if (isFloating) 0.96f else 1.0f
        ),
        shadowElevation = if (isFloating) 6.dp else 1.dp,
        border = BorderStroke(0.75.dp, MaterialTheme.colorScheme.outlineVariant),
        modifier = modifier
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(2.dp),
            modifier = Modifier
                .horizontalScroll(rememberScrollState())
                .padding(horizontal = 8.dp, vertical = 4.dp)
        ) {
            VexioTool.entries.forEach { tool ->
                val isSelected = uiState.activeTool == tool
                val itemShape = RoundedCornerShape(if (isSelected) 9.dp else 7.dp)
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

                val toolTipLabel = "${VexioI18n.toolName(tool, lang)} (${tool.shortcutHint})"
                VexioTooltip(text = toolTipLabel) {
                    Surface(
                        shape = itemShape,
                        color = bgColor,
                        modifier = Modifier
                            .size(28.dp)
                            .scale(scale)
                            .clip(itemShape)
                            .clickable { onSelectTool(tool) }
                    ) {
                        Box(contentAlignment = Alignment.Center) {
                            Icon(
                                imageVector = resolveToolIcon(tool),
                                contentDescription = toolTipLabel,
                                tint = contentColor,
                                modifier = Modifier.size(16.dp)
                            )
                        }
                    }
                }
            }

            VerticalDivider(
                modifier = Modifier
                    .height(18.dp)
                    .padding(horizontal = 4.dp),
                color = MaterialTheme.colorScheme.outlineVariant
            )

            CompactHeaderIconButton(
                icon = Icons.Default.ZoomOut,
                tooltip = VexioI18n.zoomOutTooltip(lang),
                onClick = { onZoomDelta(-15) }
            )
            VexioTooltip(text = VexioI18n.zoomResetTooltip(lang)) {
                Text(
                    text = "${uiState.zoomPercent}%",
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Medium,
                    color = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier
                        .clip(RoundedCornerShape(5.dp))
                        .clickable { onZoomReset() }
                        .padding(horizontal = 6.dp, vertical = 4.dp)
                )
            }
            CompactHeaderIconButton(
                icon = Icons.Default.ZoomIn,
                tooltip = VexioI18n.zoomInTooltip(lang),
                onClick = { onZoomDelta(15) }
            )
            CompactHeaderIconButton(
                icon = Icons.Default.Tune,
                tooltip = VexioI18n.floatToolbarTooltip(isFloating, lang),
                active = isFloating,
                onClick = onToggleFloating
            )
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

    val lang = uiState.language
    var isDragHovering by remember { mutableStateOf(false) }
    var contextMenuVisible by remember { mutableStateOf(false) }
    var contextMenuOffsetDp by remember { mutableStateOf(DpOffset.Zero) }
    var lastRightClickPageIn by remember { mutableStateOf(5.5 to 4.25) }

    val hardwarePointerIcon = remember(uiState.activeCursor) {
        resolveHardwarePointerIcon(context, uiState.activeCursor)
    }

    fun handleAndroidDragDrop(dragEvent: DragEvent): Boolean {
        isDragHovering = false
        val localPayloadJson = dragEvent.localState as? String
        val clipData = dragEvent.clipData
        if ((clipData == null || clipData.itemCount == 0) && localPayloadJson.isNullOrBlank()) {
            return false
        }

        val activity = context as? Activity
        val dragPermissions = if (clipData != null) activity?.requestDragAndDropPermissions(dragEvent) else null

        try {
            val firstItem = if (clipData != null && clipData.itemCount > 0) clipData.getItemAt(0) else null

            // Case 1: Dropped .vsdx / .vsdm / .vssx / .vstx / .vsd / .dwg / .dxf file URI
            val droppedUri = firstItem?.uri
            if (droppedUri != null) {
                val lastSeg = droppedUri.lastPathSegment?.lowercase().orEmpty()
                val ext = lastSeg.substringAfterLast('.', "")
                if (ext in SUPPORTED_DIAGRAM_EXTENSIONS ||
                    clipData?.description?.hasMimeType("application/*") == true
                ) {
                    onExternalUriDropped(droppedUri)
                    return true
                }
            }

            // Case 2: Intra-window or Cross-window Vexio Stencil Master JSON payload
            val rawText = firstItem?.text?.toString()?.takeIf { it.isNotBlank() } ?: localPayloadJson
            if (!rawText.isNullOrBlank() && rawText.trimStart().startsWith("{")) {
                val payload = runCatching {
                    Json.decodeFromString<CrossWindowShapePayload>(rawText)
                }.getOrNull()
                if (payload != null) {
                    viewModel.dropStencilAtScreenPx(
                        title = payload.title,
                        widthFormula = payload.widthFormula,
                        heightFormula = payload.heightFormula,
                        fillFormula = payload.fillFormula,
                        category = payload.category,
                        screenXPx = dragEvent.x,
                        screenYPx = dragEvent.y
                    )
                    return true
                }
            }
        } finally {
            if (clipData == null || clipData.itemCount == 0 || clipData.getItemAt(0).uri == null) {
                dragPermissions?.release()
            }
        }
        return false
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
                return handleAndroidDragDrop(event.toAndroidDragEvent())
            }
        }
    }

    Box(
        modifier = modifier
            .pointerHoverIcon(PointerIcon(hardwarePointerIcon))
            .dragAndDropTarget(
                shouldStartDragAndDrop = { startEvent ->
                    val mimes = startEvent.mimeTypes()
                    mimes.isEmpty() ||
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

                    setOnDragListener { _, dragEvent ->
                        when (dragEvent.action) {
                            DragEvent.ACTION_DRAG_STARTED -> true
                            DragEvent.ACTION_DRAG_ENTERED -> {
                                isDragHovering = true
                                true
                            }
                            DragEvent.ACTION_DRAG_LOCATION -> true
                            DragEvent.ACTION_DRAG_EXITED -> {
                                isDragHovering = false
                                true
                            }
                            DragEvent.ACTION_DROP -> handleAndroidDragDrop(dragEvent)
                            DragEvent.ACTION_DRAG_ENDED -> {
                                isDragHovering = false
                                true
                            }
                            else -> false
                        }
                    }

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
                        val (xIn, yIn) = viewModel.screenPxToPageIn(xPx, yPx)
                        lastRightClickPageIn = xIn.toDouble() to yIn.toDouble()
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
                                val activeToolNow = viewModel.uiState.value.activeTool
                                if (activeToolNow != VexioTool.EDIT_VERTICES &&
                                    activeToolNow != VexioTool.CONNECTION_POINT
                                ) {
                                    showCanvasContextMenuAt(motionEvent.x, motionEvent.y)
                                    return@setOnTouchListener true
                                }
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
                            val activeToolNow = viewModel.uiState.value.activeTool
                            if (activeToolNow != VexioTool.EDIT_VERTICES &&
                                activeToolNow != VexioTool.CONNECTION_POINT
                            ) {
                                showCanvasContextMenuAt(motionEvent.x, motionEvent.y)
                                return@setOnGenericMotionListener true
                            }
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
                        text = VexioI18n.dropOverlayBanner(lang),
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
            val visibleSmartActions = uiState.selectedShapeActions.filter { !it.invisible }
            if (visibleSmartActions.isNotEmpty()) {
                visibleSmartActions.forEachIndexed { idx, act ->
                    if (act.beginGroup && idx > 0) {
                        HorizontalDivider()
                    }
                    DropdownMenuItem(
                        text = { Text(if (act.checked) "✓ ${act.menu}" else act.menu) },
                        enabled = !act.disabled,
                        onClick = {
                            contextMenuVisible = false
                            viewModel.executeSmartAction(act.rowName)
                        }
                    )
                }
                HorizontalDivider()
            }
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxEditShapeLabel(lang)) },
                leadingIcon = { Icon(Icons.Default.Edit, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.beginInlineTextEdit(uiState.selectedShapeId)
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxGroupSelected(lang)) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.groupSelectedShapes()
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxUngroupSelected(lang)) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.ungroupSelectedShapes()
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxRotateLeft90(lang)) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.rotateOrFlipSelected(deltaAngleDeg = -90.0)
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxRotateRight90(lang)) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.rotateOrFlipSelected(deltaAngleDeg = 90.0)
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxFlipHorizontal(lang)) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.rotateOrFlipSelected(flipX = true)
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxFlipVertical(lang)) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.rotateOrFlipSelected(flipY = true)
                }
            )
            HorizontalDivider()
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxDuplicate(lang)) },
                leadingIcon = { Icon(Icons.Default.ContentCopy, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.duplicateSelectedShapes()
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxDelete(lang)) },
                leadingIcon = { Icon(Icons.Default.DeleteOutline, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.deleteSelectedShape()
                }
            )
            HorizontalDivider()
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxBringToFront(lang)) },
                leadingIcon = { Icon(Icons.Default.FlipToFront, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.reorderSelectedShapes("front")
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxSendToBack(lang)) },
                leadingIcon = { Icon(Icons.Default.FlipToBack, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.reorderSelectedShapes("back")
                }
            )
            HorizontalDivider()
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxInsertPrimitive("rect", lang)) },
                leadingIcon = { Icon(Icons.Default.CropSquare, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.drawQuickPrimitive("rect", lastRightClickPageIn.first, lastRightClickPageIn.second)
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxInsertPrimitive("ellipse", lang)) },
                leadingIcon = { Icon(Icons.Default.RadioButtonUnchecked, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.drawQuickPrimitive("ellipse", lastRightClickPageIn.first, lastRightClickPageIn.second)
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxInsertPrimitive("diamond", lang)) },
                leadingIcon = { Icon(Icons.Default.Diamond, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.drawQuickPrimitive("diamond", lastRightClickPageIn.first, lastRightClickPageIn.second)
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxInsertPrimitive("hexagon", lang)) },
                leadingIcon = { Icon(Icons.Default.Hexagon, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.drawQuickPrimitive("hexagon", lastRightClickPageIn.first, lastRightClickPageIn.second)
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxInsertPrimitive("star", lang)) },
                leadingIcon = { Icon(Icons.Default.StarBorder, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.drawQuickPrimitive("star", lastRightClickPageIn.first, lastRightClickPageIn.second)
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxInsertPrimitive("line", lang)) },
                leadingIcon = { Icon(Icons.Default.HorizontalRule, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.drawQuickPrimitive("line", lastRightClickPageIn.first, lastRightClickPageIn.second)
                }
            )
            HorizontalDivider()
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxAutoLayoutPage(lang)) },
                leadingIcon = { Icon(Icons.Default.Schema, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.autoLayoutActivePage()
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxLinkSubprocess(lang)) },
                leadingIcon = { Icon(Icons.Default.AccountTree, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.createOrLinkSubprocess()
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxInspectShapeSheet(lang)) },
                leadingIcon = { Icon(Icons.Default.TableChart, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    if (!uiState.isShapeSheetDrawerExpanded) {
                        viewModel.toggleShapeSheetDrawer()
                    }
                }
            )
            DropdownMenuItem(
                text = { Text("Smart-Shape Actions…") },
                onClick = {
                    contextMenuVisible = false
                    viewModel.setSmartActionsDialogVisible(true)
                }
            )
            DropdownMenuItem(
                text = { Text("Add Selection to My Shapes") },
                leadingIcon = { Icon(Icons.Default.Add, contentDescription = null) },
                onClick = {
                    contextMenuVisible = false
                    viewModel.addSelectionToMyShapes()
                }
            )
            DropdownMenuItem(
                text = { Text("Update Custom Master from Selection") },
                onClick = {
                    contextMenuVisible = false
                    viewModel.updateCustomMasterFromSelection()
                }
            )
            DropdownMenuItem(
                text = { Text(VexioI18n.ctxOpenNewWindow(lang)) },
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
                    maxLines = 1,
                    softWrap = false,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f)
                )
                Spacer(Modifier.width(12.dp))
                Text(
                    text = VexioI18n.shapeSheetSubtitle(uiState.language),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    softWrap = false,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false)
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
private fun VexioTooltip(
    text: String,
    preferAbove: Boolean = false,
    content: @Composable () -> Unit
) {
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    var anchorHeightPx by remember { mutableStateOf(28) }
    var visible by remember { mutableStateOf(false) }
    var hoverJob by remember { mutableStateOf<Job?>(null) }

    DisposableEffect(Unit) {
        onDispose { hoverJob?.cancel() }
    }

    Box(
        modifier = Modifier
            .onSizeChanged { anchorHeightPx = it.height }
            .pointerInput(text) {
                awaitPointerEventScope {
                    while (true) {
                        val ev = awaitPointerEvent(PointerEventPass.Initial)
                        when (ev.type) {
                            PointerEventType.Enter -> {
                                hoverJob?.cancel()
                                hoverJob = scope.launch {
                                    delay(420L)
                                    visible = true
                                }
                            }
                            PointerEventType.Exit,
                            PointerEventType.Press -> {
                                hoverJob?.cancel()
                                visible = false
                            }
                        }
                    }
                }
            }
    ) {
        content()
        if (visible && text.isNotBlank()) {
            val gapPx = with(density) { 5.dp.roundToPx() }
            Popup(
                alignment = if (preferAbove) Alignment.TopCenter else Alignment.BottomCenter,
                offset = IntOffset(0, if (preferAbove) -(anchorHeightPx + gapPx) else (anchorHeightPx + gapPx)),
                properties = PopupProperties(focusable = false)
            ) {
                Surface(
                    shape = RoundedCornerShape(5.dp),
                    color = Color(0xEB1E293B),
                    border = BorderStroke(0.5.dp, Color(0x4094A3B8)),
                    shadowElevation = 2.dp
                ) {
                    Text(
                        text = text,
                        color = Color(0xFFF8FAFC),
                        fontSize = 11.sp,
                        fontWeight = FontWeight.Medium,
                        maxLines = 1,
                        softWrap = false,
                        modifier = Modifier.padding(horizontal = 7.dp, vertical = 3.dp)
                    )
                }
            }
        }
    }
}

@Composable
private fun InspectorCard(
    title: String? = null,
    trailing: (@Composable () -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit
) {
    Surface(
        shape = RoundedCornerShape(10.dp),
        color = MaterialTheme.colorScheme.surfaceContainerLowest,
        border = BorderStroke(0.75.dp, MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.75f)),
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(
            verticalArrangement = Arrangement.spacedBy(7.dp),
            modifier = Modifier.padding(10.dp)
        ) {
            if (!title.isNullOrBlank() || trailing != null) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    if (!title.isNullOrBlank()) {
                        Text(
                            text = title.uppercase(),
                            fontSize = 10.sp,
                            fontWeight = FontWeight.SemiBold,
                            letterSpacing = 0.5.sp,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.weight(1f)
                        )
                    } else {
                        Spacer(Modifier.weight(1f))
                    }
                    trailing?.invoke()
                }
            }
            content()
        }
    }
}

@Composable
private fun CompactPropertyField(
    label: String = "",
    value: String,
    onValueChange: (String) -> Unit,
    unit: String = "",
    placeholder: String = "",
    onCommit: () -> Unit = {},
    modifier: Modifier = Modifier
) {
    var isFocused by remember { mutableStateOf(false) }
    var hadFocus by remember { mutableStateOf(false) }

    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = modifier
            .height(28.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerLow)
            .border(
                width = if (isFocused) 1.5.dp else 0.75.dp,
                color = if (isFocused) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant,
                shape = RoundedCornerShape(6.dp)
            )
            .padding(horizontal = 7.dp)
    ) {
        if (label.isNotBlank()) {
            Text(
                text = label,
                fontSize = 10.5.sp,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                softWrap = false
            )
            Spacer(Modifier.width(6.dp))
        }
        Box(
            contentAlignment = Alignment.CenterStart,
            modifier = Modifier.weight(1f)
        ) {
            if (value.isEmpty() && placeholder.isNotEmpty()) {
                Text(
                    text = placeholder,
                    fontSize = 11.5.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.55f),
                    maxLines = 1,
                    softWrap = false,
                    overflow = TextOverflow.Ellipsis
                )
            }
            BasicTextField(
                value = value,
                onValueChange = onValueChange,
                singleLine = true,
                textStyle = MaterialTheme.typography.bodySmall.copy(
                    fontSize = 12.sp,
                    color = MaterialTheme.colorScheme.onSurface
                ),
                cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                modifier = Modifier
                    .fillMaxWidth()
                    .onFocusChanged { fs ->
                        isFocused = fs.isFocused
                        if (fs.isFocused) {
                            hadFocus = true
                        } else if (hadFocus) {
                            hadFocus = false
                            onCommit()
                        }
                    }
                    .onPreviewKeyEvent { ev ->
                        if (ev.type == KeyEventType.KeyDown && ev.key == Key.Enter) {
                            onCommit()
                            true
                        } else {
                            false
                        }
                    }
            )
        }
        if (unit.isNotBlank()) {
            Spacer(Modifier.width(4.dp))
            Text(
                text = unit,
                fontSize = 10.5.sp,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                softWrap = false
            )
        }
    }
}

@Composable
private fun CompactCheckRow(
    checked: Boolean,
    label: String,
    onCheckedChange: (Boolean) -> Unit
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier
            .fillMaxWidth()
            .height(24.dp)
            .clip(RoundedCornerShape(5.dp))
            .clickable { onCheckedChange(!checked) }
            .padding(horizontal = 2.dp)
    ) {
        Box(
            contentAlignment = Alignment.Center,
            modifier = Modifier
                .size(15.dp)
                .clip(RoundedCornerShape(4.dp))
                .background(
                    if (checked) MaterialTheme.colorScheme.primary else Color.Transparent
                )
                .border(
                    width = 1.25.dp,
                    color = if (checked) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline,
                    shape = RoundedCornerShape(4.dp)
                )
        ) {
            if (checked) {
                Icon(
                    imageVector = Icons.Default.Check,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onPrimary,
                    modifier = Modifier.size(11.dp)
                )
            }
        }
        Text(
            text = label,
            fontSize = 11.5.sp,
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f)
        )
    }
}

@Composable
private fun CompactSegmentPill(
    label: String,
    selected: Boolean,
    tooltip: String = "",
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    val pill: @Composable () -> Unit = {
        Surface(
            shape = RoundedCornerShape(6.dp),
            color = if (selected) {
                MaterialTheme.colorScheme.primaryContainer
            } else {
                MaterialTheme.colorScheme.surfaceContainerLow
            },
            border = BorderStroke(
                width = 0.75.dp,
                color = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant
            ),
            modifier = modifier
                .height(26.dp)
                .clip(RoundedCornerShape(6.dp))
                .clickable(onClick = onClick)
        ) {
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier.padding(horizontal = 7.dp)
            ) {
                Text(
                    text = label,
                    fontSize = 11.sp,
                    fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
                    color = if (selected) {
                        MaterialTheme.colorScheme.onPrimaryContainer
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
                    maxLines = 1,
                    softWrap = false,
                    overflow = TextOverflow.Ellipsis
                )
            }
        }
    }
    if (tooltip.isNotBlank()) {
        VexioTooltip(text = tooltip) { pill() }
    } else {
        pill()
    }
}

@Composable
private fun ShapeInspectorPane(
    uiState: WorkstationUiState,
    viewModel: VexioSessionViewModel,
    onOpenColorPicker: (VexioColorPickerTarget) -> Unit,
    onLinkExternalDataClick: () -> Unit = {},
    onRefreshExternalDataClick: () -> Unit = {},
    onTwoWayWritebackClick: (Int) -> Unit = {},
    onExportBomReportClick: () -> Unit = {},
    onSelectCell: (String) -> Unit,
    onQuickGuardToggle: (ShapeSheetCellEntry) -> Unit
) {
    val lang = uiState.language
    var selectedTab by remember { mutableStateOf(0) }
    var inspectorFontMenuOpen by remember { mutableStateOf(false) }

    val obj: InspectedObjectUiState? = if (uiState.selectedShapeId > 0L) uiState.inspectedObject else null
    var nameDraft by remember(obj?.id, obj?.name) { mutableStateOf(obj?.name.orEmpty()) }
    var textDraft by remember(obj?.id, obj?.text) { mutableStateOf(obj?.text.orEmpty()) }
    var newPropName by remember { mutableStateOf("") }
    var newPropValue by remember { mutableStateOf("") }
    var newLayerName by remember { mutableStateOf("") }

    val unitSuffix = if (uiState.metricUnits) "mm" else "in"
    val widthShortTag = when (lang) {
        VexioAppLanguage.DE -> "B"
        VexioAppLanguage.IT, VexioAppLanguage.FR -> "L"
        VexioAppLanguage.ES -> "A"
        else -> "W"
    }
    fun formatInchesNumeric(v: Double): String =
        if (uiState.metricUnits) "%.1f".format(java.util.Locale.US, v * 25.4) else "%.2f".format(java.util.Locale.US, v)
    fun toEngineFormula(raw: String): String {
        val clean = raw.trim().replace(',', '.')
        if (clean.isEmpty()) return "0 $unitSuffix"
        if (clean.startsWith("=")) return clean
        return if (clean.toDoubleOrNull() != null) "$clean $unitSuffix" else clean
    }

    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        modifier = Modifier
            .width(292.dp)
            .fillMaxHeight()
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(10.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            // 1. Sleek Object / Page Identity Header Card
            InspectorCard {
                if (obj != null) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = "Sheet.${obj.id}",
                                fontSize = 12.5.sp,
                                fontWeight = FontWeight.Bold,
                                color = MaterialTheme.colorScheme.onSurface
                            )
                            Text(
                                text = obj.localizedTypeSubtitle(lang),
                                fontSize = 10.5.sp,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis
                            )
                        }
                        CompactHeaderIconButton(
                            icon = Icons.Default.Functions,
                            tooltip = VexioI18n.toggleShapeSheetTooltip(lang),
                            active = uiState.isShapeSheetDrawerExpanded,
                            onClick = { viewModel.toggleShapeSheetDrawer() }
                        )
                    }
                    CompactPropertyField(
                        label = VexioI18n.objectNameLabel(lang),
                        value = nameDraft,
                        onValueChange = { nameDraft = it },
                        onCommit = {
                            if (nameDraft.isNotBlank() && nameDraft != obj.name) {
                                viewModel.renameSelectedShape(nameDraft)
                            }
                        },
                        modifier = Modifier.fillMaxWidth()
                    )
                    CompactPropertyField(
                        label = VexioI18n.objectCaptionLabel(lang),
                        value = textDraft,
                        onValueChange = { textDraft = it },
                        placeholder = "—",
                        onCommit = {
                            if (textDraft != obj.text) {
                                viewModel.setSelectedShapeLabelText(textDraft)
                            }
                        },
                        modifier = Modifier.fillMaxWidth()
                    )
                } else {
                    Text(
                        text = VexioI18n.pagePropertiesTitle(lang),
                        fontSize = 12.5.sp,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.onSurface
                    )
                    Text(
                        text = "${formatInchesNumeric(uiState.pageWidthInches)} × ${formatInchesNumeric(uiState.pageHeightInches)} $unitSuffix • ${uiState.pageShapeCount}",
                        fontSize = 11.sp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }

            // 2. Zero-Truncation 4-Tab Segmented Control (Format | Size | Data | Layers)
            Row(
                horizontalArrangement = Arrangement.spacedBy(2.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .height(28.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(MaterialTheme.colorScheme.surfaceContainer)
                    .padding(2.dp)
            ) {
                listOf(
                    Triple(0, lang.inspectorFormatTab, lang.inspectorFormatTab),
                    Triple(1, lang.inspectorSizePosTab, VexioI18n.transformSectionTitle(lang)),
                    Triple(2, lang.inspectorDataTab, "Shape Data (Prop.*)"),
                    Triple(3, lang.inspectorLayersTab, lang.inspectorLayersTab)
                ).forEach { (idx, shortLabel, tip) ->
                    val active = selectedTab == idx
                    Box(modifier = Modifier.weight(1f)) {
                        VexioTooltip(text = tip) {
                            Surface(
                                shape = RoundedCornerShape(6.dp),
                                color = if (active) MaterialTheme.colorScheme.surfaceContainerLowest else Color.Transparent,
                                shadowElevation = if (active) 1.dp else 0.dp,
                                modifier = Modifier
                                    .fillMaxSize()
                                    .clip(RoundedCornerShape(6.dp))
                                    .clickable { selectedTab = idx }
                            ) {
                                Box(contentAlignment = Alignment.Center, modifier = Modifier.padding(horizontal = 2.dp)) {
                                    Text(
                                        text = shortLabel,
                                        fontSize = 11.sp,
                                        fontWeight = if (active) FontWeight.SemiBold else FontWeight.Medium,
                                        maxLines = 1,
                                        softWrap = false,
                                        color = if (active) {
                                            MaterialTheme.colorScheme.primary
                                        } else {
                                            MaterialTheme.colorScheme.onSurfaceVariant
                                        }
                                    )
                                }
                            }
                        }
                    }
                }
            }

            if (obj == null && selectedTab != 2 && selectedTab != 3) {
                InspectorCard(title = VexioI18n.pagePropertiesTitle(lang)) {
                    CompactSegmentPill(
                        label = VexioI18n.fitToScreenLabel(lang),
                        selected = false,
                        modifier = Modifier.fillMaxWidth(),
                        onClick = { viewModel.fitPageToWindow() }
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
                        listOf(
                            VexioPagePresetSpec.A4_LANDSCAPE to "A4",
                            VexioPagePresetSpec.A3_LANDSCAPE to "A3",
                            VexioPagePresetSpec.LETTER_LANDSCAPE to "Letter"
                        ).forEach { (preset, code) ->
                            CompactSegmentPill(
                                label = code,
                                selected = uiState.activePagePreset == preset,
                                modifier = Modifier.weight(1f),
                                onClick = { viewModel.applyPagePreset(preset) }
                            )
                        }
                    }
                }
            } else {
                when (selectedTab) {
                    0 -> {
                        val curObj = obj ?: return@Column
                        // TAB 0: FORMAT (Fill, Line & Arrows, Typography)
                        InspectorCard(title = VexioI18n.fillSectionTitle(lang)) {
                            Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.fillMaxWidth()) {
                                CompactSegmentPill(
                                    label = VexioI18n.noFillPill(lang),
                                    selected = curObj.fillPattern == 0,
                                    modifier = Modifier.weight(1f),
                                    onClick = { viewModel.setCellForSelection("FillPattern", "0") }
                                )
                                CompactSegmentPill(
                                    label = VexioI18n.solidPill(lang),
                                    selected = curObj.fillPattern == 1,
                                    modifier = Modifier.weight(1f),
                                    onClick = { viewModel.setCellForSelection("FillPattern", "1") }
                                )
                                CompactSegmentPill(
                                    label = VexioI18n.gradientPill(lang),
                                    selected = curObj.fillPattern >= 25,
                                    modifier = Modifier.weight(1f),
                                    onClick = { viewModel.applyVisualPreset("grad_linear") }
                                )
                            }
                            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.fillMaxWidth()) {
                                Box(modifier = Modifier.weight(1f)) {
                                    ColorSwatchActionPill(
                                        label = lang.fillColorLabel,
                                        swatchColor = parseColorHexOrFallback(uiState.shapeFillColorHex, Color(0xFFE8F0FE)),
                                        isTransparent = curObj.fillPattern == 0,
                                        onClick = { onOpenColorPicker(VexioColorPickerTarget.SHAPE_FILL) }
                                    )
                                }
                                Box(modifier = Modifier.weight(1f)) {
                                    ColorSwatchActionPill(
                                        label = lang.lineColorLabel,
                                        swatchColor = parseColorHexOrFallback(uiState.shapeLineColorHex, Color(0xFF1A73E8)),
                                        isTransparent = curObj.linePattern == 0,
                                        onClick = { onOpenColorPicker(VexioColorPickerTarget.SHAPE_LINE) }
                                    )
                                }
                            }
                            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                                Text(
                                    text = VexioI18n.transparencyLabel(lang),
                                    fontSize = 11.sp,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    modifier = Modifier.weight(1f)
                                )
                                Text(
                                    text = "${curObj.fillTransPct}%",
                                    fontSize = 11.sp,
                                    fontWeight = FontWeight.Medium,
                                    color = MaterialTheme.colorScheme.onSurface
                                )
                            }
                            Slider(
                                value = curObj.fillTransPct.toFloat(),
                                onValueChange = { pct ->
                                    viewModel.setCellForSelection("FillForegndTrans", "${pct.toInt()}%")
                                },
                                valueRange = 0f..100f,
                                steps = 9,
                                modifier = Modifier.height(24.dp)
                            )
                        }

                        InspectorCard(title = VexioI18n.lineArrowsSectionTitle(lang)) {
                            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                                Text(
                                    text = VexioI18n.lineWeightLabel(lang),
                                    fontSize = 11.sp,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    modifier = Modifier.weight(1f)
                                )
                                Text(
                                    text = "${"%.2f".format(java.util.Locale.US, curObj.lineWeightPt)} pt",
                                    fontSize = 11.sp,
                                    fontWeight = FontWeight.Medium
                                )
                            }
                            Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.fillMaxWidth()) {
                                listOf("0.5 pt", "1 pt", "1.5 pt", "2.5 pt").forEach { wt ->
                                    CompactSegmentPill(
                                        label = wt,
                                        selected = kotlin.math.abs(curObj.lineWeightPt - wt.substringBefore(" ").toDouble()) < 0.08,
                                        modifier = Modifier.weight(1f),
                                        onClick = { viewModel.setCellForSelection("LineWeight", wt) }
                                    )
                                }
                            }
                            Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.fillMaxWidth()) {
                                listOf(
                                    0 to VexioI18n.noFillPill(lang),
                                    1 to VexioI18n.solidPill(lang),
                                    2 to VexioI18n.dashedPill(lang),
                                    3 to VexioI18n.dottedPill(lang)
                                ).forEach { (pat, lbl) ->
                                    CompactSegmentPill(
                                        label = lbl,
                                        selected = curObj.linePattern == pat,
                                        modifier = Modifier.weight(1f),
                                        onClick = { viewModel.setCellForSelection("LinePattern", "$pat") }
                                    )
                                }
                            }
                            Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.fillMaxWidth()) {
                                listOf(0 to "—", 1 to "›", 4 to "▶", 11 to "◆").forEach { (arr, lbl) ->
                                    CompactSegmentPill(
                                        label = lbl,
                                        selected = curObj.endArrow == arr,
                                        modifier = Modifier.weight(1f),
                                        onClick = { viewModel.setCellForSelection("EndArrow", "$arr") }
                                    )
                                }
                            }
                        }

                        InspectorCard(title = VexioI18n.typographySectionTitle(lang)) {
                            Box(modifier = Modifier.fillMaxWidth()) {
                                CompactSegmentPill(
                                    label = "${uiState.textFontFamily} • ${uiState.textFontSizePt} pt ▾",
                                    selected = false,
                                    modifier = Modifier.fillMaxWidth(),
                                    onClick = { inspectorFontMenuOpen = true }
                                )
                                DropdownMenu(
                                    expanded = inspectorFontMenuOpen,
                                    onDismissRequest = { inspectorFontMenuOpen = false },
                                    modifier = Modifier
                                        .width(248.dp)
                                        .heightIn(max = 300.dp)
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
                                                    Icon(Icons.Default.Check, contentDescription = null, modifier = Modifier.size(15.dp))
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
                            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.fillMaxWidth()) {
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
                        }
                    }

                    1 -> {
                        val curObj = obj ?: return@Column
                        // TAB 1: POSITION & SIZE + ARRANGE + PROTECTION (Polished Desktop 2x2 Grid!)
                        var xDraft by remember(curObj.id, curObj.pinXIn, uiState.metricUnits) { mutableStateOf(formatInchesNumeric(curObj.pinXIn)) }
                        var yDraft by remember(curObj.id, curObj.pinYIn, uiState.metricUnits) { mutableStateOf(formatInchesNumeric(curObj.pinYIn)) }
                        var wDraft by remember(curObj.id, curObj.widthIn, uiState.metricUnits) { mutableStateOf(formatInchesNumeric(curObj.widthIn)) }
                        var hDraft by remember(curObj.id, curObj.heightIn, uiState.metricUnits) { mutableStateOf(formatInchesNumeric(curObj.heightIn)) }
                        var angDraft by remember(curObj.id, curObj.angleDeg) { mutableStateOf("%.1f".format(java.util.Locale.US, curObj.angleDeg)) }

                        InspectorCard(title = VexioI18n.transformSectionTitle(lang)) {
                            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.fillMaxWidth()) {
                                CompactPropertyField(
                                    label = "X",
                                    value = xDraft,
                                    onValueChange = { xDraft = it },
                                    unit = unitSuffix,
                                    onCommit = { viewModel.setCellForSelection("PinX", toEngineFormula(xDraft)) },
                                    modifier = Modifier.weight(1f)
                                )
                                CompactPropertyField(
                                    label = "Y",
                                    value = yDraft,
                                    onValueChange = { yDraft = it },
                                    unit = unitSuffix,
                                    onCommit = { viewModel.setCellForSelection("PinY", toEngineFormula(yDraft)) },
                                    modifier = Modifier.weight(1f)
                                )
                            }
                            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.fillMaxWidth()) {
                                CompactPropertyField(
                                    label = widthShortTag,
                                    value = wDraft,
                                    onValueChange = { wDraft = it },
                                    unit = unitSuffix,
                                    onCommit = { viewModel.setCellForSelection("Width", toEngineFormula(wDraft)) },
                                    modifier = Modifier.weight(1f)
                                )
                                CompactPropertyField(
                                    label = "H",
                                    value = hDraft,
                                    onValueChange = { hDraft = it },
                                    unit = unitSuffix,
                                    onCommit = { viewModel.setCellForSelection("Height", toEngineFormula(hDraft)) },
                                    modifier = Modifier.weight(1f)
                                )
                            }
                            CompactPropertyField(
                                label = "∠",
                                value = angDraft,
                                onValueChange = { angDraft = it },
                                unit = "°",
                                onCommit = {
                                    val cleanAng = angDraft.trim().replace(',', '.').ifEmpty { "0" }
                                    viewModel.setCellForSelection(
                                        "Angle",
                                        if (cleanAng.contains("deg") || cleanAng.startsWith("=")) cleanAng else "$cleanAng deg"
                                    )
                                },
                                modifier = Modifier.fillMaxWidth()
                            )
                        }

                        InspectorCard(title = VexioI18n.arrangeSectionTitle(lang)) {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(4.dp),
                                modifier = Modifier.fillMaxWidth()
                            ) {
                                Box(modifier = Modifier.weight(1f)) {
                                    CompactSegmentPill(
                                        label = "↺ 90°",
                                        selected = false,
                                        tooltip = VexioI18n.rotateLeftTooltip(lang),
                                        modifier = Modifier.fillMaxWidth(),
                                        onClick = { viewModel.rotateOrFlipSelected(deltaAngleDeg = -90.0) }
                                    )
                                }
                                Box(modifier = Modifier.weight(1f)) {
                                    CompactSegmentPill(
                                        label = "↻ 90°",
                                        selected = false,
                                        tooltip = VexioI18n.rotateRightTooltip(lang),
                                        modifier = Modifier.fillMaxWidth(),
                                        onClick = { viewModel.rotateOrFlipSelected(deltaAngleDeg = 90.0) }
                                    )
                                }
                                Box(modifier = Modifier.weight(0.7f)) {
                                    CompactSegmentPill(
                                        label = "⇆",
                                        selected = curObj.flipX,
                                        tooltip = VexioI18n.flipHorizTooltip(lang),
                                        modifier = Modifier.fillMaxWidth(),
                                        onClick = { viewModel.rotateOrFlipSelected(flipX = true) }
                                    )
                                }
                                Box(modifier = Modifier.weight(0.7f)) {
                                    CompactSegmentPill(
                                        label = "⇅",
                                        selected = curObj.flipY,
                                        tooltip = VexioI18n.flipVertTooltip(lang),
                                        modifier = Modifier.fillMaxWidth(),
                                        onClick = { viewModel.rotateOrFlipSelected(flipY = true) }
                                    )
                                }
                                CompactHeaderIconButton(
                                    icon = Icons.Default.FlipToFront,
                                    tooltip = VexioI18n.bringToFrontTooltip(lang),
                                    onClick = { viewModel.reorderSelectedShapes("front") }
                                )
                                CompactHeaderIconButton(
                                    icon = Icons.Default.FlipToBack,
                                    tooltip = VexioI18n.sendToBackTooltip(lang),
                                    onClick = { viewModel.reorderSelectedShapes("back") }
                                )
                            }
                        }

                        InspectorCard(title = VexioI18n.protectionTitle(lang)) {
                            CompactCheckRow(
                                checked = curObj.lockAspect,
                                label = VexioI18n.lockAspectLabel(lang),
                                onCheckedChange = { v -> viewModel.setCellForSelection("LockAspect", if (v) "TRUE" else "FALSE", forceSetf = true) }
                            )
                            CompactCheckRow(
                                checked = curObj.lockSize,
                                label = VexioI18n.lockSizeLabel(lang),
                                onCheckedChange = { v ->
                                    val f = if (v) "TRUE" else "FALSE"
                                    viewModel.setCellForSelection("LockWidth", f, forceSetf = true)
                                    viewModel.setCellForSelection("LockHeight", f, forceSetf = true)
                                }
                            )
                            CompactCheckRow(
                                checked = curObj.lockPosition,
                                label = VexioI18n.lockPositionLabel(lang),
                                onCheckedChange = { v ->
                                    val f = if (v) "TRUE" else "FALSE"
                                    viewModel.setCellForSelection("LockMoveX", f, forceSetf = true)
                                    viewModel.setCellForSelection("LockMoveY", f, forceSetf = true)
                                }
                            )
                            CompactCheckRow(
                                checked = curObj.lockRotate,
                                label = VexioI18n.lockRotateLabel(lang),
                                onCheckedChange = { v ->
                                    viewModel.setCellForSelection("LockRotate", if (v) "TRUE" else "FALSE", forceSetf = true)
                                }
                            )
                        }
                    }

                    2 -> {
                        // TAB 2: SHAPE DATA (Prop.*) + LIVE DATA GRAPHICS & TWO-WAY EXTERNAL DATA
                        if (obj != null) {
                            InspectorCard(title = "Shape Data (Prop.*)") {
                                if (uiState.shapeDataItems.isEmpty()) {
                                    Text(
                                        text = "—",
                                        fontSize = 11.sp,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant
                                    )
                                } else {
                                    uiState.shapeDataItems.forEach { sd: ShapeDataUiItem ->
                                        var sdValDraft by remember(sd.rowName, sd.value) { mutableStateOf(sd.value) }
                                        Row(
                                            verticalAlignment = Alignment.CenterVertically,
                                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                                            modifier = Modifier.fillMaxWidth()
                                        ) {
                                            CompactPropertyField(
                                                label = sd.label,
                                                value = sdValDraft,
                                                onValueChange = { sdValDraft = it },
                                                onCommit = {
                                                    if (sdValDraft != sd.value) {
                                                        viewModel.upsertSelectedShapeData(sd.rowName, sd.label, sd.dataTypeCode, sdValDraft)
                                                    }
                                                },
                                                modifier = Modifier.weight(1f)
                                            )
                                            CompactHeaderIconButton(
                                                icon = Icons.Default.DeleteOutline,
                                                tooltip = "Delete Prop.${sd.rowName}",
                                                onClick = { viewModel.deleteSelectedShapeData(sd.rowName) }
                                            )
                                        }
                                    }
                                }

                                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))

                                CompactPropertyField(
                                    label = "Prop",
                                    value = newPropName,
                                    onValueChange = { newPropName = it },
                                    placeholder = "Owner / Status",
                                    modifier = Modifier.fillMaxWidth()
                                )
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                                    modifier = Modifier.fillMaxWidth()
                                ) {
                                    CompactPropertyField(
                                        label = "Val",
                                        value = newPropValue,
                                        onValueChange = { newPropValue = it },
                                        placeholder = "Value",
                                        onCommit = {
                                            if (newPropName.isNotBlank()) {
                                                viewModel.upsertSelectedShapeData(newPropName, newPropName, 0, newPropValue)
                                                newPropName = ""
                                                newPropValue = ""
                                            }
                                        },
                                        modifier = Modifier.weight(1f)
                                    )
                                    CompactHeaderIconButton(
                                        icon = Icons.Default.Add,
                                        tooltip = VexioI18n.addFieldLabel(lang),
                                        active = newPropName.isNotBlank(),
                                        onClick = {
                                            if (newPropName.isNotBlank()) {
                                                viewModel.upsertSelectedShapeData(newPropName, newPropName, 0, newPropValue)
                                                newPropName = ""
                                                newPropValue = ""
                                            }
                                        }
                                    )
                                }
                            }
                        }

                        val dg: DataGraphicUiState = uiState.dataGraphics
                        var iconFieldDraft by remember(dg.iconSetField) { mutableStateOf(dg.iconSetField) }
                        var barFieldDraft by remember(dg.dataBarField) { mutableStateOf(dg.dataBarField) }
                        var calloutFieldDraft by remember(dg.textCalloutField) { mutableStateOf(dg.textCalloutField) }
                        var colorFieldDraft by remember(dg.colorByValueField) { mutableStateOf(dg.colorByValueField) }

                        fun commitDg(enabled: Boolean = true, cbv: Boolean = dg.colorByValueEnabled) {
                            viewModel.updatePageDataGraphics(
                                dg.copy(
                                    enabled = enabled,
                                    colorByValueEnabled = cbv,
                                    iconSetField = iconFieldDraft,
                                    dataBarField = barFieldDraft,
                                    textCalloutField = calloutFieldDraft,
                                    colorByValueField = colorFieldDraft,
                                    legendField = colorFieldDraft.ifBlank { iconFieldDraft }
                                )
                            )
                        }

                        InspectorCard(title = "Data Graphics & Legend") {
                            CompactCheckRow(
                                checked = dg.enabled,
                                label = "Live Data Graphics",
                                onCheckedChange = { checked -> commitDg(enabled = checked) }
                            )
                            CompactPropertyField(
                                label = "Icon",
                                value = iconFieldDraft,
                                onValueChange = { iconFieldDraft = it },
                                placeholder = "Status",
                                onCommit = { if (dg.enabled) commitDg(enabled = true) },
                                modifier = Modifier.fillMaxWidth()
                            )
                            CompactPropertyField(
                                label = "Bar",
                                value = barFieldDraft,
                                onValueChange = { barFieldDraft = it },
                                placeholder = "CompletionPct",
                                onCommit = { if (dg.enabled) commitDg(enabled = true) },
                                modifier = Modifier.fillMaxWidth()
                            )
                            CompactPropertyField(
                                label = "Tag",
                                value = calloutFieldDraft,
                                onValueChange = { calloutFieldDraft = it },
                                placeholder = "Owner",
                                onCommit = { if (dg.enabled) commitDg(enabled = true) },
                                modifier = Modifier.fillMaxWidth()
                            )
                            CompactCheckRow(
                                checked = dg.colorByValueEnabled,
                                label = "Color by Value",
                                onCheckedChange = { cbv -> commitDg(enabled = true, cbv = cbv) }
                            )
                            CompactPropertyField(
                                label = "Col",
                                value = colorFieldDraft,
                                onValueChange = { colorFieldDraft = it },
                                placeholder = "Status",
                                onCommit = { if (dg.enabled) commitDg(enabled = true) },
                                modifier = Modifier.fillMaxWidth()
                            )
                            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.fillMaxWidth()) {
                                CompactSegmentPill(
                                    label = VexioI18n.applyButton(lang),
                                    selected = dg.enabled,
                                    modifier = Modifier.weight(1f),
                                    onClick = { commitDg(enabled = true) }
                                )
                                CompactSegmentPill(
                                    label = "Legend",
                                    selected = dg.legendVisible,
                                    modifier = Modifier.weight(1f),
                                    onClick = { viewModel.insertOrUpdateDataLegend(colorFieldDraft.ifBlank { iconFieldDraft }) }
                                )
                            }
                        }

                        InspectorCard(title = "External Data (.xlsx / .csv / .sqlite / .json)") {
                            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.fillMaxWidth()) {
                                CompactSegmentPill(
                                    label = "+ Link File…",
                                    selected = false,
                                    modifier = Modifier.weight(1f),
                                    onClick = onLinkExternalDataClick
                                )
                                CompactSegmentPill(
                                    label = "BOM .xlsx",
                                    selected = false,
                                    modifier = Modifier.weight(1f),
                                    onClick = onExportBomReportClick
                                )
                            }
                            uiState.externalRecordsets.forEach { rs: ExternalRecordsetUiItem ->
                                Column(
                                    verticalArrangement = Arrangement.spacedBy(4.dp),
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .border(0.75.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(8.dp))
                                        .padding(6.dp)
                                ) {
                                    Row(verticalAlignment = Alignment.CenterVertically) {
                                        Column(modifier = Modifier.weight(1f)) {
                                            Text(rs.name, fontSize = 11.5.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                            Text(
                                                text = "${rs.sourceKind.uppercase()} • ${rs.rows.size} rows",
                                                fontSize = 10.sp,
                                                color = MaterialTheme.colorScheme.onSurfaceVariant
                                            )
                                        }
                                        CompactSegmentPill(label = "↻", selected = false, tooltip = "Refresh", onClick = onRefreshExternalDataClick)
                                        Spacer(Modifier.width(3.dp))
                                        CompactSegmentPill(label = "⇪", selected = false, tooltip = "Two-Way Sync", onClick = { onTwoWayWritebackClick(rs.id) })
                                        CompactHeaderIconButton(
                                            icon = Icons.Default.DeleteOutline,
                                            tooltip = "Unlink",
                                            onClick = { viewModel.deleteExternalRecordset(rs.id) }
                                        )
                                    }
                                    rs.rows.take(25).forEachIndexed { rIdx, rowMap ->
                                        val keyVal = rowMap[rs.keyColumn]?.takeIf { it.isNotEmpty() }
                                            ?: rowMap.values.firstOrNull().orEmpty()
                                        val linkedIds = rs.boundShapeIdsByRow[rIdx].orEmpty()
                                        Surface(
                                            shape = RoundedCornerShape(6.dp),
                                            color = if (linkedIds.isNotEmpty()) {
                                                MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.45f)
                                            } else {
                                                MaterialTheme.colorScheme.surfaceContainerLow
                                            },
                                            modifier = Modifier
                                                .fillMaxWidth()
                                                .clip(RoundedCornerShape(6.dp))
                                                .clickable { viewModel.bindExternalRecordsetRow(rs.id, rIdx) }
                                        ) {
                                            Row(
                                                verticalAlignment = Alignment.CenterVertically,
                                                modifier = Modifier.padding(horizontal = 7.dp, vertical = 4.dp)
                                            ) {
                                                Text(
                                                    text = keyVal.ifEmpty { "Row #${rIdx + 1}" },
                                                    fontSize = 11.sp,
                                                    maxLines = 1,
                                                    overflow = TextOverflow.Ellipsis,
                                                    modifier = Modifier.weight(1f)
                                                )
                                                Text(
                                                    text = if (linkedIds.isNotEmpty()) "${linkedIds.size}✓" else "Bind",
                                                    fontSize = 10.sp,
                                                    fontWeight = FontWeight.Medium,
                                                    color = MaterialTheme.colorScheme.primary
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    else -> {
                        // TAB 3: LAYERS (Compact Desktop Layer List)
                        InspectorCard(title = lang.inspectorLayersTab) {
                            uiState.pageLayers.forEach { layer: DiagramLayerUiItem ->
                                val isMember = obj?.layerIndices?.contains(layer.ix) == true
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .height(26.dp)
                                ) {
                                    if (obj != null) {
                                        Box(modifier = Modifier.weight(1f)) {
                                            CompactCheckRow(
                                                checked = isMember,
                                                label = "${layer.name} (${layer.memberCount})",
                                                onCheckedChange = { checked ->
                                                    viewModel.setSelectedShapeLayerMembership(layer.ix, checked)
                                                }
                                            )
                                        }
                                    } else {
                                        Text(
                                            text = "${layer.name} (${layer.memberCount})",
                                            fontSize = 11.5.sp,
                                            maxLines = 1,
                                            overflow = TextOverflow.Ellipsis,
                                            modifier = Modifier.weight(1f)
                                        )
                                    }
                                    CompactSegmentPill(
                                        label = if (layer.visible) "Vis" else "Hid",
                                        selected = layer.visible,
                                        tooltip = if (layer.visible) "Hide Layer" else "Show Layer",
                                        onClick = {
                                            viewModel.updatePageLayerFlags(layer.ix, !layer.visible, layer.lock, layer.print, layer.snapGlue)
                                        }
                                    )
                                    CompactHeaderIconButton(
                                        icon = Icons.Default.Lock,
                                        tooltip = if (layer.lock) "Unlock Layer" else "Lock Layer",
                                        active = layer.lock,
                                        onClick = {
                                            viewModel.updatePageLayerFlags(layer.ix, layer.visible, !layer.lock, layer.print, layer.snapGlue)
                                        }
                                    )
                                }
                            }

                            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))

                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(6.dp),
                                modifier = Modifier.fillMaxWidth()
                            ) {
                                CompactPropertyField(
                                    value = newLayerName,
                                    onValueChange = { newLayerName = it },
                                    placeholder = VexioI18n.addLayerLabel(lang),
                                    onCommit = {
                                        if (newLayerName.isNotBlank()) {
                                            viewModel.addPageLayer(newLayerName)
                                            newLayerName = ""
                                        }
                                    },
                                    modifier = Modifier.weight(1f)
                                )
                                CompactHeaderIconButton(
                                    icon = Icons.Default.Add,
                                    tooltip = VexioI18n.addLayerLabel(lang),
                                    active = newLayerName.isNotBlank(),
                                    onClick = {
                                        viewModel.addPageLayer(newLayerName)
                                        newLayerName = ""
                                    }
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
private fun WorkstationBottomStatusBar(
    uiState: WorkstationUiState,
    posture: AdaptiveWorkstationPosture,
    onSelectPage: (Int) -> Unit,
    onRenamePage: (Int, String) -> Boolean,
    onAddPage: () -> Unit,
    onDuplicatePage: (Int) -> Unit = {},
    onDeletePage: (Int) -> Unit = {},
    onSetPageBackground: (Int, Boolean, Int?, String) -> Unit = { _, _, _, _ -> },
    onAddBackgroundPage: (Int) -> Unit = {},
    onOpenPageSetup: (Int) -> Unit = {},
    onCreateSubprocess: () -> Unit,
    onAutoLayoutPage: () -> Unit,
    onZoomReset: () -> Unit
) {
    val lang = uiState.language
    var editingPageId by remember { mutableStateOf<Int?>(null) }
    var editingPageDraft by remember { mutableStateOf(TextFieldValue("")) }
    val pageRenameFocusRequester = remember { FocusRequester() }
    val backgroundPages = remember(uiState.pages) { uiState.pages.filter { it.isBackground } }
    val quickPaperSwatches = remember {
        listOf("#FFFFFF", "#FAF8F5", "#F1F5F9", "#EFF6FF", "#ECFDF5", "#FEFCE8")
    }

    LaunchedEffect(editingPageId) {
        if (editingPageId != null) {
            runCatching { pageRenameFocusRequester.requestFocus() }
        }
    }

    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = Modifier
            .fillMaxWidth()
            .height(30.dp)
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 8.dp)
        ) {
            // Single-line Horizontally Scrollable Page Tabs with Right-Click & Long-Press Context Menu
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                modifier = Modifier
                    .weight(1f)
                    .horizontalScroll(rememberScrollState())
            ) {
                uiState.pages.forEach { tab ->
                    val active = tab.pageId == uiState.activePageId
                    var pageMenuExpanded by remember(tab.pageId) { mutableStateOf(false) }
                    val remainingForegroundCount = uiState.pages.count {
                        !it.isBackground && it.pageId != tab.pageId
                    }
                    val canRemoveFromForeground = tab.isBackground || remainingForegroundCount > 0

                    Box {
                        if (editingPageId == tab.pageId) {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                modifier = Modifier
                                    .height(24.dp)
                                    .clip(RoundedCornerShape(6.dp))
                                    .background(MaterialTheme.colorScheme.surfaceContainerLowest)
                                    .border(1.5.dp, MaterialTheme.colorScheme.primary, RoundedCornerShape(6.dp))
                                    .padding(horizontal = 6.dp)
                            ) {
                                BasicTextField(
                                    value = editingPageDraft,
                                    onValueChange = { editingPageDraft = it },
                                    singleLine = true,
                                    textStyle = MaterialTheme.typography.labelSmall.copy(
                                        fontSize = 11.5.sp,
                                        color = MaterialTheme.colorScheme.onSurface
                                    ),
                                    cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                                    modifier = Modifier
                                        .width(136.dp)
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
                                        .size(14.dp)
                                        .clickable {
                                            onRenamePage(tab.pageId, editingPageDraft.text)
                                            editingPageId = null
                                        }
                                )
                            }
                        } else {
                            VexioTooltip(
                                text = "${tab.name} (${tab.scaleLabel})",
                                preferAbove = true
                            ) {
                                Surface(
                                    shape = RoundedCornerShape(6.dp),
                                    color = if (active) {
                                        MaterialTheme.colorScheme.primaryContainer
                                    } else {
                                        MaterialTheme.colorScheme.surfaceContainerLow
                                    },
                                    border = if (tab.isBackground) {
                                        BorderStroke(0.75.dp, MaterialTheme.colorScheme.outlineVariant)
                                    } else {
                                        null
                                    },
                                    modifier = Modifier
                                        .height(23.dp)
                                        .clip(RoundedCornerShape(6.dp))
                                        .pointerInput(tab.pageId) {
                                            awaitPointerEventScope {
                                                while (true) {
                                                    val ev = awaitPointerEvent(PointerEventPass.Initial)
                                                    if (ev.type == PointerEventType.Press && ev.buttons.isSecondaryPressed) {
                                                        ev.changes.forEach { it.consume() }
                                                        pageMenuExpanded = true
                                                    }
                                                }
                                            }
                                        }
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
                                    Row(
                                        verticalAlignment = Alignment.CenterVertically,
                                        horizontalArrangement = Arrangement.spacedBy(5.dp),
                                        modifier = Modifier.padding(horizontal = 8.dp)
                                    ) {
                                        if (!tab.pageColorHex.equals("#FFFFFF", ignoreCase = true)) {
                                            Box(
                                                modifier = Modifier
                                                    .size(7.dp)
                                                    .clip(CircleShape)
                                                    .background(
                                                        parseColorHexOrFallback(tab.pageColorHex, Color.White)
                                                    )
                                                    .border(
                                                        0.5.dp,
                                                        MaterialTheme.colorScheme.outline,
                                                        CircleShape
                                                    )
                                            )
                                        }
                                        Text(
                                            text = tab.name,
                                            fontSize = 11.5.sp,
                                            fontWeight = if (active) FontWeight.SemiBold else FontWeight.Medium,
                                            maxLines = 1,
                                            softWrap = false,
                                            color = if (active) {
                                                MaterialTheme.colorScheme.onPrimaryContainer
                                            } else {
                                                MaterialTheme.colorScheme.onSurfaceVariant
                                            }
                                        )
                                        if (tab.isBackground) {
                                            Text(
                                                text = "BG",
                                                fontSize = 9.sp,
                                                fontWeight = FontWeight.Bold,
                                                color = MaterialTheme.colorScheme.tertiary,
                                                maxLines = 1,
                                                softWrap = false
                                            )
                                        }
                                    }
                                }
                            }
                        }

                        DropdownMenu(
                            expanded = pageMenuExpanded,
                            onDismissRequest = { pageMenuExpanded = false }
                        ) {
                            DropdownMenuItem(
                                text = { Text(VexioI18n.pageSetupMenuLabel(lang), fontSize = 12.5.sp) },
                                leadingIcon = {
                                    Icon(
                                        Icons.Default.Tune,
                                        contentDescription = null,
                                        modifier = Modifier.size(16.dp)
                                    )
                                },
                                onClick = {
                                    pageMenuExpanded = false
                                    onOpenPageSetup(tab.pageId)
                                }
                            )

                            HorizontalDivider()

                            // Page Background & Quick Paper Color Swatches
                            Column(
                                verticalArrangement = Arrangement.spacedBy(6.dp),
                                modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp)
                            ) {
                                Text(
                                    text = VexioI18n.pageBackgroundTitle(lang).uppercase(),
                                    fontSize = 10.sp,
                                    fontWeight = FontWeight.SemiBold,
                                    letterSpacing = 0.5.sp,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                                    quickPaperSwatches.forEach { swHex ->
                                        val isSelectedColor = tab.pageColorHex.equals(swHex, ignoreCase = true)
                                        Box(
                                            modifier = Modifier
                                                .size(20.dp)
                                                .clip(CircleShape)
                                                .background(parseColorHexOrFallback(swHex, Color.White))
                                                .border(
                                                    width = if (isSelectedColor) 2.dp else 1.dp,
                                                    color = if (isSelectedColor) {
                                                        MaterialTheme.colorScheme.primary
                                                    } else {
                                                        MaterialTheme.colorScheme.outlineVariant
                                                    },
                                                    shape = CircleShape
                                                )
                                                .clickable {
                                                    pageMenuExpanded = false
                                                    onSetPageBackground(
                                                        tab.pageId,
                                                        tab.isBackground,
                                                        tab.backPageId,
                                                        swHex
                                                    )
                                                }
                                        )
                                    }
                                }
                            }

                            if (!tab.isBackground && backgroundPages.isNotEmpty()) {
                                DropdownMenuItem(
                                    text = {
                                        Text(
                                            "${VexioI18n.linkedBackgroundLabel(lang)}: ${VexioI18n.noneLabel(lang)}",
                                            fontSize = 12.sp
                                        )
                                    },
                                    leadingIcon = {
                                        if (tab.backPageId == null) {
                                            Icon(
                                                Icons.Default.Check,
                                                contentDescription = null,
                                                modifier = Modifier.size(15.dp)
                                            )
                                        } else {
                                            Spacer(Modifier.size(15.dp))
                                        }
                                    },
                                    onClick = {
                                        pageMenuExpanded = false
                                        onSetPageBackground(tab.pageId, false, null, "")
                                    }
                                )
                                backgroundPages.filter { it.pageId != tab.pageId }.forEach { bgPage ->
                                    DropdownMenuItem(
                                        text = {
                                            Text(
                                                "${VexioI18n.pageBackgroundTitle(lang)}: ${bgPage.name}",
                                                fontSize = 12.sp
                                            )
                                        },
                                        leadingIcon = {
                                            if (tab.backPageId == bgPage.pageId) {
                                                Icon(
                                                    Icons.Default.Check,
                                                    contentDescription = null,
                                                    modifier = Modifier.size(15.dp)
                                                )
                                            } else {
                                                Spacer(Modifier.size(15.dp))
                                            }
                                        },
                                        onClick = {
                                            pageMenuExpanded = false
                                            onSetPageBackground(tab.pageId, false, bgPage.pageId, "")
                                        }
                                    )
                                }
                            }

                            DropdownMenuItem(
                                text = { Text(VexioI18n.markAsBackgroundLabel(lang), fontSize = 12.5.sp) },
                                enabled = canRemoveFromForeground,
                                leadingIcon = {
                                    if (tab.isBackground) {
                                        Icon(
                                            Icons.Default.Check,
                                            contentDescription = null,
                                            modifier = Modifier.size(16.dp)
                                        )
                                    } else {
                                        Spacer(Modifier.size(16.dp))
                                    }
                                },
                                onClick = {
                                    pageMenuExpanded = false
                                    onSetPageBackground(
                                        tab.pageId,
                                        !tab.isBackground,
                                        if (!tab.isBackground) null else tab.backPageId,
                                        ""
                                    )
                                }
                            )
                            DropdownMenuItem(
                                text = { Text(VexioI18n.newBackgroundPageLabel(lang), fontSize = 12.5.sp) },
                                leadingIcon = {
                                    Icon(
                                        Icons.Default.FlipToBack,
                                        contentDescription = null,
                                        modifier = Modifier.size(16.dp)
                                    )
                                },
                                onClick = {
                                    pageMenuExpanded = false
                                    onAddBackgroundPage(tab.pageId)
                                }
                            )

                            HorizontalDivider()

                            DropdownMenuItem(
                                text = { Text(VexioI18n.newPagePill(lang), fontSize = 12.5.sp) },
                                leadingIcon = {
                                    Icon(
                                        Icons.Default.Add,
                                        contentDescription = null,
                                        modifier = Modifier.size(16.dp)
                                    )
                                },
                                onClick = {
                                    pageMenuExpanded = false
                                    onAddPage()
                                }
                            )
                            DropdownMenuItem(
                                text = { Text(VexioI18n.duplicatePageMenuLabel(lang), fontSize = 12.5.sp) },
                                leadingIcon = {
                                    Icon(
                                        Icons.Default.ContentCopy,
                                        contentDescription = null,
                                        modifier = Modifier.size(16.dp)
                                    )
                                },
                                onClick = {
                                    pageMenuExpanded = false
                                    onDuplicatePage(tab.pageId)
                                }
                            )
                            DropdownMenuItem(
                                text = { Text(lang.renamePageLabel, fontSize = 12.5.sp) },
                                leadingIcon = {
                                    Icon(
                                        Icons.Default.Edit,
                                        contentDescription = null,
                                        modifier = Modifier.size(16.dp)
                                    )
                                },
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
                                text = {
                                    Text(
                                        text = VexioI18n.deletePageMenuLabel(lang),
                                        fontSize = 12.5.sp,
                                        color = if (canRemoveFromForeground) {
                                            MaterialTheme.colorScheme.error
                                        } else {
                                            MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f)
                                        }
                                    )
                                },
                                enabled = canRemoveFromForeground,
                                leadingIcon = {
                                    Icon(
                                        Icons.Default.DeleteOutline,
                                        contentDescription = null,
                                        tint = if (canRemoveFromForeground) {
                                            MaterialTheme.colorScheme.error
                                        } else {
                                            MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f)
                                        },
                                        modifier = Modifier.size(16.dp)
                                    )
                                },
                                onClick = {
                                    pageMenuExpanded = false
                                    onDeletePage(tab.pageId)
                                }
                            )

                            HorizontalDivider()

                            DropdownMenuItem(
                                text = { Text(VexioI18n.createSubprocessDrillDown(lang), fontSize = 12.5.sp) },
                                leadingIcon = {
                                    Icon(
                                        Icons.Default.AccountTree,
                                        contentDescription = null,
                                        modifier = Modifier.size(16.dp)
                                    )
                                },
                                onClick = {
                                    pageMenuExpanded = false
                                    onCreateSubprocess()
                                }
                            )
                            DropdownMenuItem(
                                text = { Text(VexioI18n.ctxAutoLayoutPage(lang), fontSize = 12.5.sp) },
                                leadingIcon = {
                                    Icon(
                                        Icons.Default.Schema,
                                        contentDescription = null,
                                        modifier = Modifier.size(16.dp)
                                    )
                                },
                                onClick = {
                                    pageMenuExpanded = false
                                    onAutoLayoutPage()
                                }
                            )
                        }
                    }
                }
                VexioTooltip(
                    text = VexioI18n.newPagePill(lang),
                    preferAbove = true
                ) {
                    Surface(
                        shape = RoundedCornerShape(6.dp),
                        color = MaterialTheme.colorScheme.surfaceContainerLow,
                        modifier = Modifier
                            .size(23.dp)
                            .clip(RoundedCornerShape(6.dp))
                            .clickable { onAddPage() }
                    ) {
                        Box(contentAlignment = Alignment.Center, modifier = Modifier.fillMaxSize()) {
                            Icon(
                                imageVector = Icons.Default.Add,
                                contentDescription = VexioI18n.newPagePill(lang),
                                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.size(14.dp)
                            )
                        }
                    }
                }
            }

            if (posture != AdaptiveWorkstationPosture.COMPACT_CONVERTIBLE) {
                Text(
                    text = VexioI18n.statusCursorAndTool(uiState.activeCursor, uiState.activeTool, lang),
                    fontSize = 11.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    softWrap = false,
                    modifier = Modifier.padding(horizontal = 8.dp)
                )
            }

            VexioTooltip(
                text = VexioI18n.zoomResetTooltip(lang),
                preferAbove = true
            ) {
                Text(
                    text = "${uiState.zoomPercent}%",
                    fontSize = 11.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.primary,
                    maxLines = 1,
                    softWrap = false,
                    modifier = Modifier
                        .clip(RoundedCornerShape(5.dp))
                        .clickable { onZoomReset() }
                        .padding(horizontal = 6.dp, vertical = 2.dp)
                )
            }
        }
    }
}

@Composable
private fun PageSetupDialog(
    pageTab: DiagramPageTab,
    allPages: List<DiagramPageTab>,
    metricUnits: Boolean,
    lang: VexioAppLanguage,
    onDismiss: () -> Unit,
    onApply: (
        widthInches: Double,
        heightInches: Double,
        metricUnits: Boolean,
        isBackground: Boolean,
        backPageId: Int?,
        pageColorHex: String
    ) -> Unit
) {
    var useMetric by remember(pageTab.pageId, metricUnits) { mutableStateOf(metricUnits) }
    var widthDraft by remember(pageTab.pageId, metricUnits) {
        val v = if (metricUnits) pageTab.widthInches * 25.4 else pageTab.widthInches
        mutableStateOf(
            if (metricUnits) "%.1f".format(java.util.Locale.US, v)
            else "%.2f".format(java.util.Locale.US, v)
        )
    }
    var heightDraft by remember(pageTab.pageId, metricUnits) {
        val v = if (metricUnits) pageTab.heightInches * 25.4 else pageTab.heightInches
        mutableStateOf(
            if (metricUnits) "%.1f".format(java.util.Locale.US, v)
            else "%.2f".format(java.util.Locale.US, v)
        )
    }
    var isBgDraft by remember(pageTab.pageId) { mutableStateOf(pageTab.isBackground) }
    var backPageIdDraft by remember(pageTab.pageId) { mutableStateOf(pageTab.backPageId) }
    var colorHexDraft by remember(pageTab.pageId) { mutableStateOf(pageTab.pageColorHex) }

    val availableBgPages = remember(allPages, pageTab.pageId) {
        allPages.filter { it.isBackground && it.pageId != pageTab.pageId }
    }
    val canMarkAsBackground = pageTab.isBackground || allPages.count {
        !it.isBackground && it.pageId != pageTab.pageId
    } > 0

    fun currentInchesPair(): Pair<Double, Double> {
        val rawW = widthDraft.replace(',', '.').toDoubleOrNull()
        val rawH = heightDraft.replace(',', '.').toDoubleOrNull()
        val wIn = if (rawW != null && rawW > 0.0) {
            if (useMetric) rawW / 25.4 else rawW
        } else {
            pageTab.widthInches
        }
        val hIn = if (rawH != null && rawH > 0.0) {
            if (useMetric) rawH / 25.4 else rawH
        } else {
            pageTab.heightInches
        }
        return wIn.coerceIn(1.0, 200.0) to hIn.coerceIn(1.0, 200.0)
    }

    fun setFromInches(wIn: Double, hIn: Double, metric: Boolean) {
        useMetric = metric
        if (metric) {
            widthDraft = "%.1f".format(java.util.Locale.US, wIn * 25.4)
            heightDraft = "%.1f".format(java.util.Locale.US, hIn * 25.4)
        } else {
            widthDraft = "%.2f".format(java.util.Locale.US, wIn)
            heightDraft = "%.2f".format(java.util.Locale.US, hIn)
        }
    }

    val (curWIn, curHIn) = currentInchesPair()
    val isLandscape = curWIn >= curHIn
    val unitSuffix = if (useMetric) "mm" else "in"
    val paperSwatches = remember {
        listOf("#FFFFFF", "#FAF8F5", "#F1F5F9", "#EFF6FF", "#ECFDF5", "#FEFCE8")
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Icon(
                    imageVector = Icons.Default.Tune,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(18.dp)
                )
                Text(
                    text = "${VexioI18n.pagePropertiesTitle(lang)} — ${pageTab.name}",
                    fontSize = 15.sp,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }
        },
        text = {
            Column(
                verticalArrangement = Arrangement.spacedBy(10.dp),
                modifier = Modifier
                    .width(380.dp)
                    .verticalScroll(rememberScrollState())
            ) {
                // 1. Paper Size & Units
                InspectorCard(
                    title = VexioI18n.paperSizeLabel(lang),
                    trailing = {
                        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            CompactSegmentPill(
                                label = "mm",
                                selected = useMetric,
                                onClick = {
                                    val (w, h) = currentInchesPair()
                                    setFromInches(w, h, metric = true)
                                }
                            )
                            CompactSegmentPill(
                                label = "in",
                                selected = !useMetric,
                                onClick = {
                                    val (w, h) = currentInchesPair()
                                    setFromInches(w, h, metric = false)
                                }
                            )
                        }
                    }
                ) {
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(5.dp),
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        val presets = listOf(
                            Triple("A4", 297.0 / 25.4 to 210.0 / 25.4, true),
                            Triple("A3", 420.0 / 25.4 to 297.0 / 25.4, true),
                            Triple("A5", 210.0 / 25.4 to 148.0 / 25.4, true),
                            Triple("Letter", 11.0 to 8.5, false)
                        )
                        presets.forEach { (pName, dims, pMetric) ->
                            val targetW = if (isLandscape) dims.first else dims.second
                            val targetH = if (isLandscape) dims.second else dims.first
                            val matchesPreset = kotlin.math.abs(curWIn - targetW) < 0.08 &&
                                kotlin.math.abs(curHIn - targetH) < 0.08
                            CompactSegmentPill(
                                label = pName,
                                selected = matchesPreset,
                                modifier = Modifier.weight(1f),
                                onClick = { setFromInches(targetW, targetH, pMetric) }
                            )
                        }
                    }
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        CompactPropertyField(
                            label = "W",
                            value = widthDraft,
                            onValueChange = { widthDraft = it },
                            unit = unitSuffix,
                            modifier = Modifier.weight(1f)
                        )
                        CompactPropertyField(
                            label = "H",
                            value = heightDraft,
                            onValueChange = { heightDraft = it },
                            unit = unitSuffix,
                            modifier = Modifier.weight(1f)
                        )
                    }
                }

                // 2. Orientation
                InspectorCard(title = VexioI18n.orientationLabel(lang)) {
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        CompactSegmentPill(
                            label = VexioI18n.landscapeLabel(lang),
                            selected = isLandscape,
                            modifier = Modifier.weight(1f),
                            onClick = {
                                val (w, h) = currentInchesPair()
                                val maxSide = kotlin.math.max(w, h)
                                val minSide = kotlin.math.min(w, h)
                                setFromInches(maxSide, minSide, useMetric)
                            }
                        )
                        CompactSegmentPill(
                            label = VexioI18n.portraitLabel(lang),
                            selected = !isLandscape,
                            modifier = Modifier.weight(1f),
                            onClick = {
                                val (w, h) = currentInchesPair()
                                val maxSide = kotlin.math.max(w, h)
                                val minSide = kotlin.math.min(w, h)
                                setFromInches(minSide, maxSide, useMetric)
                            }
                        )
                    }
                }

                // 3. Page Color & Background Assignment
                InspectorCard(title = VexioI18n.pageBackgroundTitle(lang)) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Text(
                            text = VexioI18n.paperColorLabel(lang),
                            fontSize = 11.5.sp,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.weight(1f)
                        )
                        paperSwatches.forEach { swHex ->
                            val selected = colorHexDraft.equals(swHex, ignoreCase = true)
                            Box(
                                modifier = Modifier
                                    .size(22.dp)
                                    .clip(CircleShape)
                                    .background(parseColorHexOrFallback(swHex, Color.White))
                                    .border(
                                        width = if (selected) 2.dp else 1.dp,
                                        color = if (selected) {
                                            MaterialTheme.colorScheme.primary
                                        } else {
                                            MaterialTheme.colorScheme.outlineVariant
                                        },
                                        shape = CircleShape
                                    )
                                    .clickable { colorHexDraft = swHex }
                            )
                        }
                    }

                    if (canMarkAsBackground) {
                        CompactCheckRow(
                            checked = isBgDraft,
                            label = VexioI18n.markAsBackgroundLabel(lang),
                            onCheckedChange = { checked ->
                                isBgDraft = checked
                                if (checked) backPageIdDraft = null
                            }
                        )
                    }

                    if (!isBgDraft && availableBgPages.isNotEmpty()) {
                        Text(
                            text = VexioI18n.linkedBackgroundLabel(lang),
                            fontSize = 11.sp,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(5.dp),
                            modifier = Modifier
                                .fillMaxWidth()
                                .horizontalScroll(rememberScrollState())
                        ) {
                            CompactSegmentPill(
                                label = VexioI18n.noneLabel(lang),
                                selected = backPageIdDraft == null,
                                onClick = { backPageIdDraft = null }
                            )
                            availableBgPages.forEach { bgPage ->
                                CompactSegmentPill(
                                    label = bgPage.name,
                                    selected = backPageIdDraft == bgPage.pageId,
                                    onClick = { backPageIdDraft = bgPage.pageId }
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
                    val (finalWIn, finalHIn) = currentInchesPair()
                    onApply(
                        finalWIn,
                        finalHIn,
                        useMetric,
                        isBgDraft,
                        if (isBgDraft) null else backPageIdDraft,
                        colorHexDraft
                    )
                }
            ) {
                Text(VexioI18n.applyButton(lang))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(VexioI18n.cancelButton(lang))
            }
        }
    )
}

@Composable
private fun FullSpectrumColorPickerDialog(
    title: String,
    initialHex: String,
    allowClearNone: Boolean,
    clearLabel: String,
    lang: VexioAppLanguage,
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
                    Text(VexioI18n.redChannel(red, lang), style = MaterialTheme.typography.labelSmall)
                    Slider(
                        value = red.toFloat(),
                        onValueChange = { syncHexFromRgb(it.toInt(), green, blue) },
                        valueRange = 0f..255f
                    )
                    Text(VexioI18n.greenChannel(green, lang), style = MaterialTheme.typography.labelSmall)
                    Slider(
                        value = green.toFloat(),
                        onValueChange = { syncHexFromRgb(red, it.toInt(), blue) },
                        valueRange = 0f..255f
                    )
                    Text(VexioI18n.blueChannel(blue, lang), style = MaterialTheme.typography.labelSmall)
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
                Text(VexioI18n.applyColorButton(lang))
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
                    Text(VexioI18n.cancelButton(lang))
                }
            }
        }
    )
}

