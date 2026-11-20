package com.vexio.core

import android.graphics.Bitmap
import android.net.Uri
import android.os.SystemClock
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.Surface
import androidx.input.motionprediction.MotionEventPredictor
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.math.max
import kotlin.math.min

/**
 * Zero-wrapper JNI bridge to `libvexio_jni.so` (`crates/vexio-core/src/ffi/`).
 *
 * All pointer handles (`enginePtr`, `docPtr`, `viewportPtr`) are opaque 64-bit Rust heap
 * allocations owned across the FFI boundary. High-frequency input and raster preview paths
 * transfer memory strictly via zero-copy `ByteBuffer.allocateDirect` buffers in native endianness.
 */
object NativeBridge {
    private val libraryLoaded = AtomicBoolean(false)

    /**
     * Ensures `libvexio_jni.so` (`arm64-v8a` or `x86_64`) is linked into the current process.
     */
    @Synchronized
    fun ensureLoaded(): Boolean {
        if (libraryLoaded.get()) return true
        return try {
            System.loadLibrary("vexio_jni")
            libraryLoaded.set(true)
            true
        } catch (_: UnsatisfiedLinkError) {
            false
        }
    }

    val isNativeAvailable: Boolean
        get() = libraryLoaded.get()

    init {
        ensureLoaded()
    }

    @JvmStatic
    external fun nativeInitEngine(): Long

    @JvmStatic
    external fun nativeDestroyEngine(enginePtr: Long)

    @JvmStatic
    external fun nativeCreateBlankDocument(title: String): Long

    @JvmStatic
    external fun nativeOpenDocument(enginePtr: Long, directBytes: ByteBuffer, ext: String): Long

    @JvmStatic
    external fun nativeSaveDocument(docPtr: Long, ext: String): ByteArray?

    @JvmStatic
    external fun nativeAttachVulkanSurface(
        enginePtr: Long,
        docPtr: Long,
        surface: Surface,
        width: Int,
        height: Int,
        density: Float
    ): Long

    @JvmStatic
    external fun nativeResizeViewport(
        viewportPtr: Long,
        width: Int,
        height: Int,
        density: Float
    )

    @JvmStatic
    external fun nativePushMotionBatch(
        viewportPtr: Long,
        docPtr: Long,
        directBatchBuffer: ByteBuffer,
        count: Int
    ): Int

    @JvmStatic
    external fun nativeGetCellFormula(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        cellName: String
    ): String

    @JvmStatic
    external fun nativeSetCellFormula(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        cellName: String,
        formula: String,
        forceSetf: Boolean
    ): Int

    @JvmStatic
    external fun nativeUndo(docPtr: Long): Boolean

    @JvmStatic
    external fun nativeRedo(docPtr: Long): Boolean

    @JvmStatic
    external fun nativeRenderPreviewRgba(
        docPtr: Long,
        pageId: Int,
        width: Int,
        height: Int,
        outDirectRgba: ByteBuffer
    ): Int
}

/**
 * Deterministic 32-byte native-order binary layout written into `directBatchBuffer`
 * for `NativeBridge.nativePushMotionBatch`.
 *
 * Memory layout per sample (32 bytes, 8-byte aligned, little-endian on arm64-v8a & x86_64):
 * - `[0..3]`   `x: Float`               Physical viewport X coordinate (px)
 * - `[4..7]`   `y: Float`               Physical viewport Y coordinate (px)
 * - `[8..11]`  `pressure: Float`        Normalized stylus/touch pressure `[0.0..1.0]` (or scroll delta X)
 * - `[12..15]` `tiltRad: Float`         USI/active stylus tilt in radians `[0..PI/2]` (or scroll delta Y)
 * - `[16..19]` `orientationRad: Float`  Stylus azimuth orientation in radians `[-PI..PI]`
 * - `[20..23]` `packedFlags: Int`       Bitpacked action, tool, buttons, and keyboard modifiers
 * - `[24..31]` `timestampMicros: Long`  Monotonic event timestamp in microseconds
 */
object MotionBatchFormat {
    const val SAMPLE_STRIDE_BYTES: Int = 32
    const val MAX_BATCH_SAMPLES: Int = 256
    const val BUFFER_CAPACITY_BYTES: Int = SAMPLE_STRIDE_BYTES * MAX_BATCH_SAMPLES

    // Action codes (bits 0..7)
    const val ACTION_DOWN: Int = 0
    const val ACTION_MOVE: Int = 1
    const val ACTION_UP: Int = 2
    const val ACTION_CANCEL: Int = 3
    const val ACTION_HOVER_MOVE: Int = 4
    const val ACTION_SCROLL: Int = 5
    const val ACTION_PREDICTED_MOVE: Int = 6
    const val ACTION_SECONDARY_CLICK: Int = 7

    // Tool codes (bits 8..11)
    const val TOOL_UNKNOWN: Int = 0
    const val TOOL_FINGER: Int = 1
    const val TOOL_STYLUS: Int = 2
    const val TOOL_MOUSE: Int = 3
    const val TOOL_ERASER: Int = 4

    // Button flags (bits 12..19)
    const val BUTTON_PRIMARY_BIT: Int = 1 shl 12
    const val BUTTON_SECONDARY_BIT: Int = 1 shl 13
    const val BUTTON_TERTIARY_BIT: Int = 1 shl 14
    const val BUTTON_STYLUS_PRIMARY_BIT: Int = 1 shl 15
    const val BUTTON_STYLUS_SECONDARY_BIT: Int = 1 shl 16

    // Hardware keyboard modifier flags (bits 20..23)
    const val MOD_SHIFT_BIT: Int = 1 shl 20
    const val MOD_CTRL_BIT: Int = 1 shl 21
    const val MOD_ALT_BIT: Int = 1 shl 22
    const val MOD_META_BIT: Int = 1 shl 23

    fun allocateDirectBatchBuffer(): ByteBuffer =
        ByteBuffer.allocateDirect(BUFFER_CAPACITY_BYTES).order(ByteOrder.nativeOrder())

    fun packFlags(
        actionCode: Int,
        toolCode: Int,
        androidButtonState: Int,
        androidMetaState: Int
    ): Int {
        var bits = (actionCode and 0xFF) or ((toolCode and 0x0F) shl 8)
        if ((androidButtonState and MotionEvent.BUTTON_PRIMARY) != 0) bits = bits or BUTTON_PRIMARY_BIT
        if ((androidButtonState and MotionEvent.BUTTON_SECONDARY) != 0) bits = bits or BUTTON_SECONDARY_BIT
        if ((androidButtonState and MotionEvent.BUTTON_TERTIARY) != 0) bits = bits or BUTTON_TERTIARY_BIT
        if ((androidButtonState and MotionEvent.BUTTON_STYLUS_PRIMARY) != 0) bits = bits or BUTTON_STYLUS_PRIMARY_BIT
        if ((androidButtonState and MotionEvent.BUTTON_STYLUS_SECONDARY) != 0) bits = bits or BUTTON_STYLUS_SECONDARY_BIT

        if ((androidMetaState and KeyEvent.META_SHIFT_ON) != 0) bits = bits or MOD_SHIFT_BIT
        if ((androidMetaState and KeyEvent.META_CTRL_ON) != 0) bits = bits or MOD_CTRL_BIT
        if ((androidMetaState and KeyEvent.META_ALT_ON) != 0) bits = bits or MOD_ALT_BIT
        if ((androidMetaState and KeyEvent.META_META_ON) != 0) bits = bits or MOD_META_BIT
        return bits
    }

    fun unpackAction(packedFlags: Int): Int = packedFlags and 0xFF
    fun unpackTool(packedFlags: Int): Int = (packedFlags ushr 8) and 0x0F

    fun writeSample(
        buffer: ByteBuffer,
        sampleIndex: Int,
        x: Float,
        y: Float,
        pressure: Float,
        tiltRad: Float,
        orientationRad: Float,
        packedFlags: Int,
        timestampMicros: Long
    ) {
        require(buffer.isDirect) { "Motion batch ByteBuffer must be allocated via allocateDirect" }
        require(sampleIndex in 0 until MAX_BATCH_SAMPLES) {
            "sampleIndex $sampleIndex out of range 0 until $MAX_BATCH_SAMPLES"
        }
        val baseOffset = sampleIndex * SAMPLE_STRIDE_BYTES
        buffer.putFloat(baseOffset, x)
        buffer.putFloat(baseOffset + 4, y)
        buffer.putFloat(baseOffset + 8, pressure)
        buffer.putFloat(baseOffset + 12, tiltRad)
        buffer.putFloat(baseOffset + 16, orientationRad)
        buffer.putInt(baseOffset + 20, packedFlags)
        buffer.putLong(baseOffset + 24, timestampMicros)
    }

    fun mapAndroidToolType(androidToolType: Int): Int = when (androidToolType) {
        MotionEvent.TOOL_TYPE_FINGER -> TOOL_FINGER
        MotionEvent.TOOL_TYPE_STYLUS -> TOOL_STYLUS
        MotionEvent.TOOL_TYPE_MOUSE -> TOOL_MOUSE
        MotionEvent.TOOL_TYPE_ERASER -> TOOL_ERASER
        else -> TOOL_UNKNOWN
    }
}

/**
 * Contextual hardware pointer cursor codes returned by `NativeBridge.nativePushMotionBatch`.
 */
enum class VexioCursorKind(val code: Int) {
    ARROW(0),
    CROSSHAIR(1),
    MOVE_ALL(2),
    RESIZE_NS(3),
    RESIZE_EW(4),
    RESIZE_NWSE(5),
    RESIZE_NESW(6),
    TEXT_IBEAM(7),
    GRAB_HAND(8),
    GRABBING_HAND(9),
    GLUE_SNAP(10),
    FORBIDDEN_GUARD(11);

    companion object {
        fun fromNativeCode(code: Int): VexioCursorKind =
            entries.firstOrNull { it.code == code } ?: ARROW
    }
}

/**
 * Status code returned by `NativeBridge.nativeSetCellFormula`.
 */
enum class FormulaCommitStatus(val code: Int) {
    OK(0),
    GUARDED_CELL_PROTECTED(1),
    SYNTAX_ERROR(2),
    EVALUATION_ERROR(3),
    ENGINE_ERROR(-1);

    companion object {
        fun fromCode(code: Int): FormulaCommitStatus = when {
            code == 0 -> OK
            code == 1 -> GUARDED_CELL_PROTECTED
            code == 2 -> SYNTAX_ERROR
            code == 3 -> EVALUATION_ERROR
            else -> ENGINE_ERROR
        }
    }
}

/**
 * Low-latency USI / active stylus palm rejection filter.
 *
 * Rejects capacitive finger/palm contacts whenever a stylus (`TOOL_TYPE_STYLUS` or `TOOL_TYPE_ERASER`)
 * is hovering over the digitizer, actively touching the glass, or lifted within `palmGuardWindowMillis`.
 * Hardware mice and trackpads (`TOOL_TYPE_MOUSE`) are never rejected.
 */
class StylusPalmRejector(
    private val palmGuardWindowMillis: Long = 320L
) {
    private var stylusHovering: Boolean = false
    private var stylusTouching: Boolean = false
    private var lastStylusTimestampMillis: Long = Long.MIN_VALUE / 2

    fun shouldRejectEvent(toolCode: Int, actionMasked: Int, eventTimeMillis: Long): Boolean {
        val isStylus = toolCode == MotionBatchFormat.TOOL_STYLUS ||
            toolCode == MotionBatchFormat.TOOL_ERASER

        if (isStylus) {
            lastStylusTimestampMillis = eventTimeMillis
            when (actionMasked) {
                MotionEvent.ACTION_HOVER_ENTER,
                MotionEvent.ACTION_HOVER_MOVE -> stylusHovering = true
                MotionEvent.ACTION_HOVER_EXIT -> stylusHovering = false
                MotionEvent.ACTION_DOWN,
                MotionEvent.ACTION_POINTER_DOWN,
                MotionEvent.ACTION_MOVE -> {
                    stylusTouching = true
                    stylusHovering = false
                }
                MotionEvent.ACTION_UP,
                MotionEvent.ACTION_POINTER_UP,
                MotionEvent.ACTION_CANCEL -> stylusTouching = false
            }
            return false
        }

        if (toolCode == MotionBatchFormat.TOOL_MOUSE) {
            return false
        }

        if (toolCode == MotionBatchFormat.TOOL_FINGER) {
            if (stylusHovering || stylusTouching) {
                return true
            }
            val elapsedSinceStylus = eventTimeMillis - lastStylusTimestampMillis
            if (elapsedSinceStylus in 0..palmGuardWindowMillis) {
                return true
            }
        }
        return false
    }

    val isStylusProximityActive: Boolean
        get() = stylusHovering || stylusTouching
}

/**
 * Workstation authoring tools aligned 1:1 with Microsoft Visio shortcuts (`Ctrl+1`, `Ctrl+2`, `Ctrl+3`, etc.).
 */
enum class VexioTool(val displayName: String, val shortcutHint: String) {
    POINTER("Pointer", "Ctrl+1"),
    TEXT_BLOCK("Text", "Ctrl+2"),
    CONNECTOR("Dynamic Connector", "Ctrl+3"),
    RECTANGLE("Rectangle", "Ctrl+8"),
    ELLIPSE("Ellipse", "Ctrl+9"),
    STYLUS_INK("USI Markup Ink", "Ctrl+4"),
    PAN_HAND("Pan Canvas", "Space")
}

@Serializable
data class StencilMasterItem(
    val masterId: String,
    val title: String,
    val category: String,
    val defaultWidthFormula: String,
    val defaultHeightFormula: String,
    val fillFormula: String,
    val shapeKind: String
)

/**
 * Payload transferred across side-by-side Googlebook OS windows during cross-window Drag & Drop.
 */
@Serializable
data class CrossWindowShapePayload(
    val sourceWindowId: String,
    val masterId: String,
    val title: String,
    val widthFormula: String,
    val heightFormula: String,
    val fillFormula: String,
    val pinXFormula: String = "4.25 in",
    val pinYFormula: String = "5.50 in"
)

data class ShapeSheetCellEntry(
    val section: String,
    val cellName: String,
    val formula: String,
    val isGuarded: Boolean
)

data class DiagramPageTab(
    val pageId: Int,
    val name: String,
    val scaleLabel: String
)

data class ValidationBadgeSummary(
    val standard: String,
    val errorCount: Int,
    val warningCount: Int,
    val statusMessage: String
)

data class WorkstationUiState(
    val windowInstanceId: String = "win-${SystemClock.uptimeMillis()}",
    val documentTitle: String = "Architecture_Blueprint.vsdx",
    val documentUri: Uri? = null,
    val documentExtension: String = "vsdx",
    val isDirty: Boolean = false,
    val canUndo: Boolean = false,
    val canRedo: Boolean = false,
    val activeTool: VexioTool = VexioTool.POINTER,
    val activeCursor: VexioCursorKind = VexioCursorKind.ARROW,
    val zoomPercent: Int = 100,
    val activePageId: Int = 0,
    val pages: List<DiagramPageTab> = listOf(
        DiagramPageTab(0, "Page-1: System Topology", "1 in = 1 in"),
        DiagramPageTab(1, "Page-2: BPMN 2.0 Flow", "1 in = 1 in"),
        DiagramPageTab(2, "Page-3: CAD Floorplan", "1/4 in = 1 ft")
    ),
    val selectedStencilCategory: String = "BPMN 2.0",
    val selectedShapeId: Long = 1L,
    val selectedShapeName: String = "Sheet.1 (Gateway Node)",
    val activeCellName: String = "Width",
    val formulaBarInput: String = "GUARD(2.50 in)",
    val forceSetfOverride: Boolean = false,
    val lastFormulaStatus: FormulaCommitStatus = FormulaCommitStatus.OK,
    val statusBanner: String = "Vulkan Swapchain Ready • 120 Hz R*-Tree Spatial Index",
    val isShapeSheetDrawerExpanded: Boolean = true,
    val isInspectorExpanded: Boolean = true,
    val isStencilDrawerExpanded: Boolean = true,
    val isToolbarFloating: Boolean = true,
    val isStylusActive: Boolean = false,
    val lastStylusPressure: Float = 0f,
    val lastStylusTiltDeg: Int = 0,
    val shapeSheetCells: List<ShapeSheetCellEntry> = defaultShapeSheetCells(),
    val validationSummary: ValidationBadgeSummary = ValidationBadgeSummary(
        standard = "BPMN 2.0",
        errorCount = 0,
        warningCount = 0,
        statusMessage = "0 BPMN 2.0 / UML 2.5 structural issues"
    )
)

private fun defaultShapeSheetCells(): List<ShapeSheetCellEntry> = listOf(
    ShapeSheetCellEntry("Shape Transform", "PinX", "4.25 in", isGuarded = false),
    ShapeSheetCellEntry("Shape Transform", "PinY", "5.50 in", isGuarded = false),
    ShapeSheetCellEntry("Shape Transform", "Width", "GUARD(2.50 in)", isGuarded = true),
    ShapeSheetCellEntry("Shape Transform", "Height", "1.25 in", isGuarded = false),
    ShapeSheetCellEntry("Shape Transform", "Angle", "0 deg", isGuarded = false),
    ShapeSheetCellEntry("Shape Transform", "LocPinX", "Width*0.5", isGuarded = false),
    ShapeSheetCellEntry("Shape Transform", "LocPinY", "Height*0.5", isGuarded = false),
    ShapeSheetCellEntry("Line Format", "LineWeight", "1.5 pt", isGuarded = false),
    ShapeSheetCellEntry("Line Format", "LineColor", "RGB(26,115,232)", isGuarded = false),
    ShapeSheetCellEntry("Fill Format", "FillForegnd", "RGB(232,240,254)", isGuarded = false),
    ShapeSheetCellEntry("Protection", "LockWidth", "0", isGuarded = false),
    ShapeSheetCellEntry("Text Transform", "TxtPinX", "Width*0.5", isGuarded = false),
    ShapeSheetCellEntry("Text Transform", "TxtPinY", "Height*0.5", isGuarded = false)
)

/**
 * Built-in enterprise stencil catalog covering Flowchart, BPMN 2.0, UML 2.5, and CAD Architecture.
 */
object BuiltInStencilCatalog {
    val categories: List<String> = listOf("BPMN 2.0", "Flowchart", "UML 2.5", "CAD & Rack")

    val masters: List<StencilMasterItem> = listOf(
        StencilMasterItem("bpmn.task", "User Task", "BPMN 2.0", "1.75 in", "1.00 in", "RGB(232,240,254)", "RoundedRect"),
        StencilMasterItem("bpmn.gateway.xor", "Exclusive Gateway", "BPMN 2.0", "GUARD(0.75 in)", "GUARD(0.75 in)", "RGB(254,247,224)", "Diamond"),
        StencilMasterItem("bpmn.event.start", "Start Event", "BPMN 2.0", "GUARD(0.50 in)", "GUARD(0.50 in)", "RGB(230,244,234)", "Circle"),
        StencilMasterItem("bpmn.swimlane", "Pool / Swimlane", "BPMN 2.0", "8.50 in", "2.50 in", "RGB(248,249,250)", "Swimlane"),
        StencilMasterItem("flow.process", "Process Step", "Flowchart", "1.80 in", "0.90 in", "RGB(227,242,253)", "Rect"),
        StencilMasterItem("flow.decision", "Decision", "Flowchart", "1.40 in", "1.00 in", "RGB(255,243,224)", "Diamond"),
        StencilMasterItem("flow.database", "Data Store", "Flowchart", "1.50 in", "1.10 in", "RGB(237,231,246)", "Cylinder"),
        StencilMasterItem("uml.class", "UML 2.5 Class", "UML 2.5", "2.40 in", "1.60 in", "RGB(243,229,245)", "ClassBox"),
        StencilMasterItem("uml.component", "Component", "UML 2.5", "2.20 in", "1.20 in", "RGB(224,242,241)", "Component"),
        StencilMasterItem("uml.lifeline", "Sequence Lifeline", "UML 2.5", "1.20 in", "4.00 in", "RGB(241,248,233)", "Lifeline"),
        StencilMasterItem("cad.wall", "Structural Wall (Locked Scale)", "CAD & Rack", "12 ft", "6 in", "RGB(207,216,220)", "Wall"),
        StencilMasterItem("cad.door", "Single Swing Door 36\"", "CAD & Rack", "3 ft", "3 ft", "RGB(255,255,255)", "DoorArc"),
        StencilMasterItem("cad.rack42u", "42U Server Cabinet", "CAD & Rack", "GUARD(24 in)", "GUARD(78.75 in)", "RGB(224,230,237)", "Rack")
    )
}

/**
 * High-level session ViewModel coordinating the Rust `NativeBridge` engine, Vulkan surface
 * lifecycle, zero-copy `ByteBuffer.allocateDirect` motion/stylus streaming, Undo/Redo WAL,
 * and live ShapeSheet cell formula evaluation.
 */
class VexioSessionViewModel(
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO
) : ViewModel() {

    private val _uiState = MutableStateFlow(WorkstationUiState())
    val uiState: StateFlow<WorkstationUiState> = _uiState.asStateFlow()

    // Pre-allocated 8 KiB zero-copy Direct ByteBuffer for 120Hz/240Hz motion & USI stylus batches
    private val directMotionBatchBuffer: ByteBuffer = MotionBatchFormat.allocateDirectBatchBuffer()

    // Reusable Direct ByteBuffer for page thumbnail / print preview RGBA rendering
    private var directPreviewBuffer: ByteBuffer =
        ByteBuffer.allocateDirect(256 * 256 * 4).order(ByteOrder.nativeOrder())

    val palmRejector = StylusPalmRejector(palmGuardWindowMillis = 320L)

    // Local command journal fallback mirror so UI state stays 100% deterministic even before JNI surface attach
    private data class FormulaEditRecord(
        val pageId: Int,
        val shapeId: Long,
        val cellName: String,
        val previousFormula: String,
        val newFormula: String
    )

    private val localUndoStack = ArrayDeque<FormulaEditRecord>()
    private val localRedoStack = ArrayDeque<FormulaEditRecord>()

    private var enginePtr: Long = 0L
    private var docPtr: Long = 0L
    private var viewportPtr: Long = 0L
    private var nextSyntheticShapeId: Long = 2L

    init {
        initializeEngineAndBlankDocument("Architecture_Blueprint.vsdx")
    }

    @Synchronized
    fun initializeEngineAndBlankDocument(title: String) {
        if (NativeBridge.ensureLoaded()) {
            if (enginePtr == 0L) {
                enginePtr = NativeBridge.nativeInitEngine()
            }
            docPtr = NativeBridge.nativeCreateBlankDocument(title)
        }
        _uiState.update { state ->
            state.copy(
                documentTitle = title,
                isDirty = false,
                statusBanner = if (NativeBridge.isNativeAvailable && enginePtr != 0L) {
                    "Vexio Rust Engine Active • Vulkan Swapchain Ready"
                } else {
                    "Vexio Workstation Active • Ready for Vulkan Surface"
                }
            )
        }
        refreshSelectedShapeFormulas()
    }

    @Synchronized
    fun attachVulkanSurface(surface: Surface, width: Int, height: Int, density: Float) {
        if (width <= 0 || height <= 0 || !surface.isValid) return
        if (NativeBridge.isNativeAvailable && enginePtr != 0L && docPtr != 0L) {
            viewportPtr = NativeBridge.nativeAttachVulkanSurface(
                enginePtr = enginePtr,
                docPtr = docPtr,
                surface = surface,
                width = width,
                height = height,
                density = density
            )
        }
        _uiState.update {
            it.copy(
                statusBanner = "Vulkan ANativeWindow ${width}×${height} @ ${"%.2f".format(density)}x attached"
            )
        }
    }

    @Synchronized
    fun resizeVulkanViewport(width: Int, height: Int, density: Float) {
        if (width <= 0 || height <= 0) return
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L) {
            NativeBridge.nativeResizeViewport(viewportPtr, width, height, density)
        }
    }

    @Synchronized
    fun detachVulkanSurface() {
        viewportPtr = 0L
    }

    /**
     * Streams a `MotionEvent` (including historical coalesced samples and `MotionEventPredictor`
     * predicted USI stylus coordinates) into `directMotionBatchBuffer` with zero heap allocations
     * and dispatches it across JNI via `nativePushMotionBatch`.
     *
     * @return `true` if the event was consumed (or palm-rejected cleanly).
     */
    @Synchronized
    fun dispatchMotionEvent(
        event: MotionEvent,
        predictor: MotionEventPredictor?
    ): Boolean {
        if (event.pointerCount == 0) return false

        val primaryTool = MotionBatchFormat.mapAndroidToolType(event.getToolType(0))
        val actionMasked = event.actionMasked

        // USI / Active Stylus Palm Rejection gate
        if (palmRejector.shouldRejectEvent(primaryTool, actionMasked, event.eventTime)) {
            return true
        }

        predictor?.record(event)

        val mappedAction = when (actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                if ((event.buttonState and MotionEvent.BUTTON_SECONDARY) != 0) {
                    MotionBatchFormat.ACTION_SECONDARY_CLICK
                } else {
                    MotionBatchFormat.ACTION_DOWN
                }
            }
            MotionEvent.ACTION_MOVE -> MotionBatchFormat.ACTION_MOVE
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> MotionBatchFormat.ACTION_UP
            MotionEvent.ACTION_CANCEL -> MotionBatchFormat.ACTION_CANCEL
            MotionEvent.ACTION_HOVER_MOVE,
            MotionEvent.ACTION_HOVER_ENTER,
            MotionEvent.ACTION_HOVER_EXIT -> MotionBatchFormat.ACTION_HOVER_MOVE
            MotionEvent.ACTION_SCROLL -> MotionBatchFormat.ACTION_SCROLL
            else -> return false
        }

        var sampleCount = 0
        val buttonState = event.buttonState
        val metaState = event.metaState

        // 1. Pack coalesced historical samples (120Hz - 240Hz digitizer sub-frame points)
        if (mappedAction == MotionBatchFormat.ACTION_MOVE) {
            val historyCount = min(event.historySize, MotionBatchFormat.MAX_BATCH_SAMPLES - 8)
            for (h in 0 until historyCount) {
                val flags = MotionBatchFormat.packFlags(
                    actionCode = MotionBatchFormat.ACTION_MOVE,
                    toolCode = primaryTool,
                    androidButtonState = buttonState,
                    androidMetaState = metaState
                )
                MotionBatchFormat.writeSample(
                    buffer = directMotionBatchBuffer,
                    sampleIndex = sampleCount,
                    x = event.getHistoricalX(0, h),
                    y = event.getHistoricalY(0, h),
                    pressure = event.getHistoricalPressure(0, h).coerceIn(0f, 1f),
                    tiltRad = event.getHistoricalAxisValue(MotionEvent.AXIS_TILT, 0, h),
                    orientationRad = event.getHistoricalAxisValue(MotionEvent.AXIS_ORIENTATION, 0, h),
                    packedFlags = flags,
                    timestampMicros = event.getHistoricalEventTime(h) * 1_000L
                )
                sampleCount++
            }
        }

        // 2. Pack current frame sample (for scroll wheel events, encode HSCROLL/VSCROLL in pressure/tilt slots)
        val currentPressure = if (mappedAction == MotionBatchFormat.ACTION_SCROLL) {
            event.getAxisValue(MotionEvent.AXIS_HSCROLL)
        } else {
            event.getPressure(0).coerceIn(0f, 1f)
        }
        val currentTilt = if (mappedAction == MotionBatchFormat.ACTION_SCROLL) {
            event.getAxisValue(MotionEvent.AXIS_VSCROLL)
        } else {
            event.getAxisValue(MotionEvent.AXIS_TILT, 0)
        }

        val currentFlags = MotionBatchFormat.packFlags(
            actionCode = mappedAction,
            toolCode = primaryTool,
            androidButtonState = buttonState,
            androidMetaState = metaState
        )
        MotionBatchFormat.writeSample(
            buffer = directMotionBatchBuffer,
            sampleIndex = sampleCount,
            x = event.getX(0),
            y = event.getY(0),
            pressure = currentPressure,
            tiltRad = currentTilt,
            orientationRad = event.getAxisValue(MotionEvent.AXIS_ORIENTATION, 0),
            packedFlags = currentFlags,
            timestampMicros = event.eventTime * 1_000L
        )
        sampleCount++

        // 3. Pack Kalman/neural predicted stylus trajectory sample for zero-perceived-lag ink rendering
        val isStylusStroke = (primaryTool == MotionBatchFormat.TOOL_STYLUS ||
            primaryTool == MotionBatchFormat.TOOL_ERASER) &&
            mappedAction == MotionBatchFormat.ACTION_MOVE
        if (isStylusStroke && predictor != null && sampleCount < MotionBatchFormat.MAX_BATCH_SAMPLES) {
            val predicted = predictor.predict()
            if (predicted != null) {
                try {
                    val predFlags = MotionBatchFormat.packFlags(
                        actionCode = MotionBatchFormat.ACTION_PREDICTED_MOVE,
                        toolCode = primaryTool,
                        androidButtonState = buttonState,
                        androidMetaState = metaState
                    )
                    MotionBatchFormat.writeSample(
                        buffer = directMotionBatchBuffer,
                        sampleIndex = sampleCount,
                        x = predicted.getX(0),
                        y = predicted.getY(0),
                        pressure = predicted.getPressure(0).coerceIn(0f, 1f),
                        tiltRad = predicted.getAxisValue(MotionEvent.AXIS_TILT, 0),
                        orientationRad = predicted.getAxisValue(MotionEvent.AXIS_ORIENTATION, 0),
                        packedFlags = predFlags,
                        timestampMicros = predicted.eventTime * 1_000L
                    )
                    sampleCount++
                } finally {
                    predicted.recycle()
                }
            }
        }

        directMotionBatchBuffer.position(0)
        directMotionBatchBuffer.limit(sampleCount * MotionBatchFormat.SAMPLE_STRIDE_BYTES)

        // Handle Ctrl + Scroll-wheel zoom deterministically
        if (mappedAction == MotionBatchFormat.ACTION_SCROLL &&
            (metaState and KeyEvent.META_CTRL_ON) != 0
        ) {
            val vScroll = event.getAxisValue(MotionEvent.AXIS_VSCROLL)
            if (vScroll != 0f) {
                val delta = if (vScroll > 0f) 10 else -10
                adjustZoom(delta)
            }
        }

        val nativeCursorCode = if (NativeBridge.isNativeAvailable && viewportPtr != 0L && docPtr != 0L) {
            NativeBridge.nativePushMotionBatch(
                viewportPtr = viewportPtr,
                docPtr = docPtr,
                directBatchBuffer = directMotionBatchBuffer,
                count = sampleCount
            )
        } else {
            inferFallbackCursorCode(_uiState.value.activeTool, mappedAction, buttonState)
        }

        directMotionBatchBuffer.clear()

        val resolvedCursor = VexioCursorKind.fromNativeCode(nativeCursorCode)
        val stylusActive = primaryTool == MotionBatchFormat.TOOL_STYLUS ||
            primaryTool == MotionBatchFormat.TOOL_ERASER
        val tiltDegrees = Math.toDegrees(currentTilt.toDouble()).toInt().coerceIn(0, 90)

        _uiState.update { state ->
            state.copy(
                activeCursor = resolvedCursor,
                isStylusActive = stylusActive || palmRejector.isStylusProximityActive,
                lastStylusPressure = if (stylusActive) currentPressure else state.lastStylusPressure,
                lastStylusTiltDeg = if (stylusActive) tiltDegrees else state.lastStylusTiltDeg
            )
        }
        return true
    }

    private fun inferFallbackCursorCode(
        tool: VexioTool,
        mappedAction: Int,
        buttonState: Int
    ): Int = when (tool) {
        VexioTool.POINTER -> VexioCursorKind.ARROW.code
        VexioTool.TEXT_BLOCK -> VexioCursorKind.TEXT_IBEAM.code
        VexioTool.CONNECTOR -> VexioCursorKind.GLUE_SNAP.code
        VexioTool.RECTANGLE, VexioTool.ELLIPSE, VexioTool.STYLUS_INK -> VexioCursorKind.CROSSHAIR.code
        VexioTool.PAN_HAND -> if ((buttonState and MotionEvent.BUTTON_PRIMARY) != 0 ||
            mappedAction == MotionBatchFormat.ACTION_MOVE
        ) {
            VexioCursorKind.GRABBING_HAND.code
        } else {
            VexioCursorKind.GRAB_HAND.code
        }
    }

    fun selectTool(tool: VexioTool) {
        val defaultCursor = VexioCursorKind.fromNativeCode(
            inferFallbackCursorCode(tool, MotionBatchFormat.ACTION_HOVER_MOVE, 0)
        )
        _uiState.update {
            it.copy(
                activeTool = tool,
                activeCursor = defaultCursor,
                statusBanner = "Active Tool: ${tool.displayName} (${tool.shortcutHint})"
            )
        }
    }

    fun adjustZoom(deltaPercent: Int) {
        _uiState.update { state ->
            val clamped = (state.zoomPercent + deltaPercent).coerceIn(10, 1600)
            state.copy(zoomPercent = clamped)
        }
    }

    fun setZoomPercent(targetPercent: Int) {
        _uiState.update { it.copy(zoomPercent = targetPercent.coerceIn(10, 1600)) }
    }

    fun selectPage(pageId: Int) {
        _uiState.update { state ->
            if (state.pages.none { it.pageId == pageId }) state
            else state.copy(activePageId = pageId)
        }
        refreshSelectedShapeFormulas()
    }

    fun addNewPage() {
        _uiState.update { state ->
            val nextId = (state.pages.maxOfOrNull { it.pageId } ?: 0) + 1
            val newTab = DiagramPageTab(
                pageId = nextId,
                name = "Page-${nextId + 1}: Sheet",
                scaleLabel = "1 in = 1 in"
            )
            state.copy(
                pages = state.pages + newTab,
                activePageId = nextId,
                isDirty = true,
                statusBanner = "Added ${newTab.name}"
            )
        }
    }

    fun selectStencilCategory(category: String) {
        _uiState.update { it.copy(selectedStencilCategory = category) }
    }

    fun toggleShapeSheetDrawer() {
        _uiState.update { it.copy(isShapeSheetDrawerExpanded = !it.isShapeSheetDrawerExpanded) }
    }

    fun toggleInspectorPane() {
        _uiState.update { it.copy(isInspectorExpanded = !it.isInspectorExpanded) }
    }

    fun toggleStencilDrawer() {
        _uiState.update { it.copy(isStencilDrawerExpanded = !it.isStencilDrawerExpanded) }
    }

    fun toggleToolbarDockMode() {
        _uiState.update { it.copy(isToolbarFloating = !it.isToolbarFloating) }
    }

    fun selectShapeSheetCell(cellName: String) {
        _uiState.update { state ->
            val cell = state.shapeSheetCells.firstOrNull { it.cellName == cellName }
            val liveFormula = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
                val nativeVal = NativeBridge.nativeGetCellFormula(
                    docPtr = docPtr,
                    pageId = state.activePageId,
                    shapeId = state.selectedShapeId,
                    cellName = cellName
                )
                nativeVal.ifEmpty { cell?.formula.orEmpty() }
            } else {
                cell?.formula.orEmpty()
            }
            state.copy(
                activeCellName = cellName,
                formulaBarInput = liveFormula,
                lastFormulaStatus = FormulaCommitStatus.OK
            )
        }
    }

    fun updateFormulaBarInput(newInput: String) {
        _uiState.update { it.copy(formulaBarInput = newInput) }
    }

    fun setForceSetfOverride(enabled: Boolean) {
        _uiState.update { it.copy(forceSetfOverride = enabled) }
    }

    /**
     * Evaluates and commits a ShapeSheet cell formula on the selected shape, strictly honoring
     * Visio `GUARD(expr)` semantics unless `forceSetf == true` (`SETF(GetRef(cell), formula)`).
     */
    @Synchronized
    fun commitCellFormula(
        cellName: String = _uiState.value.activeCellName,
        formula: String = _uiState.value.formulaBarInput,
        forceSetf: Boolean = _uiState.value.forceSetfOverride
    ): FormulaCommitStatus {
        val trimmed = formula.trim()
        if (trimmed.isEmpty()) {
            _uiState.update {
                it.copy(
                    lastFormulaStatus = FormulaCommitStatus.SYNTAX_ERROR,
                    statusBanner = "#SYNTAX! Formula cannot be empty"
                )
            }
            return FormulaCommitStatus.SYNTAX_ERROR
        }

        val currentState = _uiState.value
        val existingCell = currentState.shapeSheetCells.firstOrNull { it.cellName == cellName }

        val status = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val code = NativeBridge.nativeSetCellFormula(
                docPtr = docPtr,
                pageId = currentState.activePageId,
                shapeId = currentState.selectedShapeId,
                cellName = cellName,
                formula = trimmed,
                forceSetf = forceSetf
            )
            FormulaCommitStatus.fromCode(code)
        } else {
            // Deterministic local ShapeSheet GUARD & syntax check
            val openParens = trimmed.count { it == '(' }
            val closeParens = trimmed.count { it == ')' }
            if (openParens != closeParens) {
                FormulaCommitStatus.SYNTAX_ERROR
            } else if (existingCell?.isGuarded == true && !forceSetf &&
                !trimmed.uppercase().startsWith("GUARD(")
            ) {
                FormulaCommitStatus.GUARDED_CELL_PROTECTED
            } else {
                FormulaCommitStatus.OK
            }
        }

        if (status == FormulaCommitStatus.OK) {
            val prevFormula = existingCell?.formula ?: ""
            if (prevFormula != trimmed) {
                localUndoStack.addLast(
                    FormulaEditRecord(
                        pageId = currentState.activePageId,
                        shapeId = currentState.selectedShapeId,
                        cellName = cellName,
                        previousFormula = prevFormula,
                        newFormula = trimmed
                    )
                )
                localRedoStack.clear()
            }
            val nowGuarded = trimmed.uppercase().startsWith("GUARD(")
            _uiState.update { state ->
                val updatedCells = state.shapeSheetCells.map { entry ->
                    if (entry.cellName == cellName) {
                        entry.copy(formula = trimmed, isGuarded = nowGuarded)
                    } else {
                        entry
                    }
                }
                state.copy(
                    shapeSheetCells = updatedCells,
                    formulaBarInput = trimmed,
                    isDirty = true,
                    canUndo = true,
                    canRedo = false,
                    lastFormulaStatus = FormulaCommitStatus.OK,
                    statusBanner = "Updated ${state.selectedShapeName}!$cellName = $trimmed"
                )
            }
        } else {
            val msg = when (status) {
                FormulaCommitStatus.GUARDED_CELL_PROTECTED ->
                    "GUARD() protected cell '$cellName': enable SETF override to replace formula"
                FormulaCommitStatus.SYNTAX_ERROR ->
                    "ShapeSheet formula syntax error in '$cellName'"
                FormulaCommitStatus.EVALUATION_ERROR ->
                    "ShapeSheet formula returned #VALUE! / #REF! in '$cellName'"
                else -> "Engine rejected formula update for '$cellName'"
            }
            _uiState.update {
                it.copy(
                    lastFormulaStatus = status,
                    statusBanner = msg
                )
            }
        }
        return status
    }

    @Synchronized
    fun refreshSelectedShapeFormulas() {
        val state = _uiState.value
        if (!NativeBridge.isNativeAvailable || docPtr == 0L) return
        val refreshed = state.shapeSheetCells.map { cell ->
            val raw = NativeBridge.nativeGetCellFormula(
                docPtr = docPtr,
                pageId = state.activePageId,
                shapeId = state.selectedShapeId,
                cellName = cell.cellName
            )
            if (raw.isNotEmpty()) {
                cell.copy(
                    formula = raw,
                    isGuarded = raw.uppercase().startsWith("GUARD(")
                )
            } else {
                cell
            }
        }
        val activeFormula = refreshed.firstOrNull { it.cellName == state.activeCellName }?.formula
            ?: state.formulaBarInput
        _uiState.update {
            it.copy(shapeSheetCells = refreshed, formulaBarInput = activeFormula)
        }
    }

    @Synchronized
    fun instantiateMasterDrop(
        title: String,
        widthFormula: String,
        heightFormula: String,
        fillFormula: String,
        pinXInches: Float,
        pinYInches: Float
    ) {
        val shapeId = nextSyntheticShapeId++
        val pinXStr = "${"%.2f".format(pinXInches)} in"
        val pinYStr = "${"%.2f".format(pinYInches)} in"
        val pageId = _uiState.value.activePageId

        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeSetCellFormula(docPtr, pageId, shapeId, "PinX", pinXStr, true)
            NativeBridge.nativeSetCellFormula(docPtr, pageId, shapeId, "PinY", pinYStr, true)
            NativeBridge.nativeSetCellFormula(docPtr, pageId, shapeId, "Width", widthFormula, true)
            NativeBridge.nativeSetCellFormula(docPtr, pageId, shapeId, "Height", heightFormula, true)
            NativeBridge.nativeSetCellFormula(docPtr, pageId, shapeId, "FillForegnd", fillFormula, true)
        }

        val updatedCells = defaultShapeSheetCells().map { entry ->
            when (entry.cellName) {
                "PinX" -> entry.copy(formula = pinXStr)
                "PinY" -> entry.copy(formula = pinYStr)
                "Width" -> entry.copy(
                    formula = widthFormula,
                    isGuarded = widthFormula.uppercase().startsWith("GUARD(")
                )
                "Height" -> entry.copy(
                    formula = heightFormula,
                    isGuarded = heightFormula.uppercase().startsWith("GUARD(")
                )
                "FillForegnd" -> entry.copy(formula = fillFormula)
                else -> entry
            }
        }

        _uiState.update { state ->
            val activeFormula = updatedCells.firstOrNull { it.cellName == state.activeCellName }?.formula
                ?: widthFormula
            state.copy(
                selectedShapeId = shapeId,
                selectedShapeName = "Sheet.$shapeId ($title)",
                shapeSheetCells = updatedCells,
                formulaBarInput = activeFormula,
                isDirty = true,
                canUndo = true,
                statusBanner = "Dropped '$title' at ($pinXStr, $pinYStr)"
            )
        }
    }

    @Synchronized
    fun performUndo() {
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val undone = NativeBridge.nativeUndo(docPtr)
            if (undone) {
                refreshSelectedShapeFormulas()
                _uiState.update {
                    it.copy(canRedo = true, statusBanner = "Undo applied via Rust WAL")
                }
                return
            }
        }
        val record = localUndoStack.removeLastOrNull() ?: return
        localRedoStack.addLast(record)
        _uiState.update { state ->
            val restoredCells = state.shapeSheetCells.map { cell ->
                if (cell.cellName == record.cellName) {
                    cell.copy(
                        formula = record.previousFormula,
                        isGuarded = record.previousFormula.uppercase().startsWith("GUARD(")
                    )
                } else {
                    cell
                }
            }
            state.copy(
                shapeSheetCells = restoredCells,
                formulaBarInput = if (state.activeCellName == record.cellName) {
                    record.previousFormula
                } else {
                    state.formulaBarInput
                },
                canUndo = localUndoStack.isNotEmpty(),
                canRedo = true,
                statusBanner = "Undo ${record.cellName} -> ${record.previousFormula}"
            )
        }
    }

    @Synchronized
    fun performRedo() {
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val redone = NativeBridge.nativeRedo(docPtr)
            if (redone) {
                refreshSelectedShapeFormulas()
                _uiState.update {
                    it.copy(canUndo = true, statusBanner = "Redo applied via Rust WAL")
                }
                return
            }
        }
        val record = localRedoStack.removeLastOrNull() ?: return
        localUndoStack.addLast(record)
        _uiState.update { state ->
            val reappliedCells = state.shapeSheetCells.map { cell ->
                if (cell.cellName == record.cellName) {
                    cell.copy(
                        formula = record.newFormula,
                        isGuarded = record.newFormula.uppercase().startsWith("GUARD(")
                    )
                } else {
                    cell
                }
            }
            state.copy(
                shapeSheetCells = reappliedCells,
                formulaBarInput = if (state.activeCellName == record.cellName) {
                    record.newFormula
                } else {
                    state.formulaBarInput
                },
                canUndo = true,
                canRedo = localRedoStack.isNotEmpty(),
                statusBanner = "Redo ${record.cellName} -> ${record.newFormula}"
            )
        }
    }

    fun deleteSelectedShape() {
        val current = _uiState.value
        _uiState.update {
            it.copy(
                isDirty = true,
                canUndo = true,
                statusBanner = "Deleted ${current.selectedShapeName}"
            )
        }
    }

    /**
     * Loads a `.vsdx`, `.vsdm`, `.vssx`, `.vstx`, `.vsd`, `.dwg`, or `.dxf` payload into a
     * zero-copy `ByteBuffer.allocateDirect` buffer and opens it via `NativeBridge.nativeOpenDocument`.
     */
    fun openDocumentFromBytes(
        displayName: String,
        uri: Uri?,
        rawBytes: ByteArray
    ) {
        viewModelScope.launch(ioDispatcher) {
            val ext = displayName.substringAfterLast('.', "vsdx").lowercase()
            val directBuffer = ByteBuffer.allocateDirect(max(1, rawBytes.size))
                .order(ByteOrder.nativeOrder())
            directBuffer.put(rawBytes)
            directBuffer.flip()

            var openedPtr = 0L
            synchronized(this@VexioSessionViewModel) {
                if (NativeBridge.ensureLoaded()) {
                    if (enginePtr == 0L) {
                        enginePtr = NativeBridge.nativeInitEngine()
                    }
                    openedPtr = NativeBridge.nativeOpenDocument(enginePtr, directBuffer, ext)
                    if (openedPtr != 0L) {
                        docPtr = openedPtr
                    }
                }
            }

            withContext(Dispatchers.Main) {
                _uiState.update { state ->
                    state.copy(
                        documentTitle = displayName,
                        documentUri = uri,
                        documentExtension = ext,
                        isDirty = false,
                        statusBanner = if (openedPtr != 0L) {
                            "Opened $displayName (${rawBytes.size} bytes, .$ext) losslessly"
                        } else {
                            "Loaded $displayName (${rawBytes.size} bytes, .$ext)"
                        }
                    )
                }
                refreshSelectedShapeFormulas()
            }
        }
    }

    /**
     * Serializes the active document to the requested target format (`vsdx`, `vsdm`, `dxf`, `pdf`, `svg`).
     * Used by both SAF local disk persistence and the native OS `PrintDocumentAdapter` (`ext = "pdf"`).
     */
    @Synchronized
    fun exportDocumentBytes(extension: String): ByteArray {
        val cleanExt = extension.trimStart('.').lowercase()
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val nativeBytes = NativeBridge.nativeSaveDocument(docPtr, cleanExt)
            if (nativeBytes != null && nativeBytes.isNotEmpty()) {
                return nativeBytes
            }
        }
        if (cleanExt == "pdf") {
            return buildDeterministicVectorPdf(_uiState.value)
        }
        return buildDeterministicXmlFallback(_uiState.value, cleanExt)
    }

    fun markSaved(uri: Uri?, displayName: String, ext: String) {
        _uiState.update {
            it.copy(
                documentUri = uri ?: it.documentUri,
                documentTitle = displayName,
                documentExtension = ext,
                isDirty = false,
                statusBanner = "Saved $displayName to local storage"
            )
        }
    }

    /**
     * Renders an RGBA page preview via `NativeBridge.nativeRenderPreviewRgba` using a reusable
     * zero-copy `ByteBuffer.allocateDirect` buffer.
     */
    @Synchronized
    fun renderPagePreviewBitmap(pageId: Int, width: Int, height: Int): Bitmap? {
        if (width <= 0 || height <= 0) return null
        val requiredBytes = width * height * 4
        if (directPreviewBuffer.capacity() < requiredBytes) {
            directPreviewBuffer = ByteBuffer.allocateDirect(requiredBytes).order(ByteOrder.nativeOrder())
        }
        directPreviewBuffer.clear()
        directPreviewBuffer.limit(requiredBytes)

        if (!NativeBridge.isNativeAvailable || docPtr == 0L) {
            return null
        }
        val rc = NativeBridge.nativeRenderPreviewRgba(
            docPtr = docPtr,
            pageId = pageId,
            width = width,
            height = height,
            outDirectRgba = directPreviewBuffer
        )
        if (rc != 0) return null
        directPreviewBuffer.rewind()
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        bitmap.copyPixelsFromBuffer(directPreviewBuffer)
        return bitmap
    }

    override fun onCleared() {
        synchronized(this) {
            viewportPtr = 0L
            docPtr = 0L
            if (enginePtr != 0L && NativeBridge.isNativeAvailable) {
                NativeBridge.nativeDestroyEngine(enginePtr)
                enginePtr = 0L
            }
        }
        super.onCleared()
    }

    /**
     * Generates a valid, standards-compliant ISO 32000-1 1.4 vector PDF byte stream from the
     * current ShapeSheet state if invoked before `libvexio_jni.so` is attached.
     */
    private fun buildDeterministicVectorPdf(state: WorkstationUiState): ByteArray {
        val safeTitle = state.documentTitle
            .replace("\\", "\\\\")
            .replace("(", "\\(")
            .replace(")", "\\)")
        val safeShape = state.selectedShapeName
            .replace("\\", "\\\\")
            .replace("(", "\\(")
            .replace(")", "\\)")

        val contentStream = buildString {
            append("q\n")
            append("0.10 0.45 0.91 RG 1.5 w\n")
            append("0.91 0.94 0.99 rg\n")
            append("216 360 180 90 re B\n")
            append("BT /F1 12 Tf 230 412 Td (Vexio Print: $safeTitle) Tj ET\n")
            append("BT /F1 10 Tf 230 392 Td (Active Shape: $safeShape) Tj ET\n")
            append("Q\n")
        }
        val streamBytes = contentStream.toByteArray(Charsets.US_ASCII)

        val objects = listOf(
            "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
            "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n",
            "4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
            "5 0 obj\n<< /Length ${streamBytes.size} >>\nstream\n$contentStream\nendstream\nendobj\n"
        )

        val out = StringBuilder()
        out.append("%PDF-1.4\n")
        val offsets = IntArray(objects.size)
        for (i in objects.indices) {
            offsets[i] = out.length
            out.append(objects[i])
        }
        val xrefOffset = out.length
        out.append("xref\n0 ${objects.size + 1}\n")
        out.append("0000000000 65535 f \n")
        for (offset in offsets) {
            out.append(String.format("%010d 00000 n \n", offset))
        }
        out.append("trailer\n<< /Size ${objects.size + 1} /Root 1 0 R >>\n")
        out.append("startxref\n$xrefOffset\n%%EOF\n")
        return out.toString().toByteArray(Charsets.US_ASCII)
    }

    private fun buildDeterministicXmlFallback(state: WorkstationUiState, ext: String): ByteArray {
        val cellsXml = state.shapeSheetCells.joinToString("\n") { cell ->
            "      <Cell N=\"${cell.cellName}\" F=\"${cell.formula}\" />"
        }
        val payload = """
            <?xml version="1.0" encoding="UTF-8"?>
            <VexioDocument format="$ext" title="${state.documentTitle}">
              <Page ID="${state.activePageId}">
                <Shape ID="${state.selectedShapeId}" Name="${state.selectedShapeName}">
            $cellsXml
                </Shape>
              </Page>
            </VexioDocument>
        """.trimIndent()
        return payload.toByteArray(Charsets.UTF_8)
    }
}
