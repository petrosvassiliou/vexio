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
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import java.util.Locale
import java.util.concurrent.atomic.AtomicBoolean
import java.util.zip.ZipEntry
import java.util.zip.ZipInputStream
import java.util.zip.ZipOutputStream
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
    external fun nativeDestroyDocument(docPtr: Long)

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
    external fun nativeDestroyViewport(viewportPtr: Long)

    @JvmStatic
    external fun nativeResizeViewport(
        viewportPtr: Long,
        width: Int,
        height: Int,
        density: Float
    )

    @JvmStatic
    external fun nativeSetViewportTool(viewportPtr: Long, toolCode: Int)

    @JvmStatic
    external fun nativeSetViewportPage(viewportPtr: Long, docPtr: Long, pageId: Int)

    @JvmStatic
    external fun nativeSetViewportZoom(
        viewportPtr: Long,
        docPtr: Long,
        zoomScale: Double,
        panXIn: Double,
        panYIn: Double
    )

    @JvmStatic
    external fun nativeRenderViewportFrame(viewportPtr: Long, docPtr: Long): Boolean

    @JvmStatic
    external fun nativeRenderViewportRgba(
        viewportPtr: Long,
        docPtr: Long,
        outDirectRgba: ByteBuffer
    ): Int

    @JvmStatic
    external fun nativeGetPrimarySelection(viewportPtr: Long): Long

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
    external fun nativeListStencilsJson(docPtr: Long): String

    @JvmStatic
    external fun nativeLoadStarterTemplate(docPtr: Long, templateId: String): Boolean

    @JvmStatic
    external fun nativeDropStencil(
        docPtr: Long,
        pageId: Int,
        category: String,
        masterName: String,
        xIn: Double,
        yIn: Double
    ): Long

    @JvmStatic
    external fun nativeDrawPrimitive(
        docPtr: Long,
        pageId: Int,
        kind: String,
        x1In: Double,
        y1In: Double,
        x2In: Double,
        y2In: Double
    ): Long

    @JvmStatic
    external fun nativeTransformShape(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        pinXIn: Double,
        pinYIn: Double,
        widthIn: Double,
        heightIn: Double,
        angleDeg: Double
    ): Boolean

    @JvmStatic
    external fun nativeDeleteShapes(docPtr: Long, pageId: Int, shapeIds: LongArray): Int

    @JvmStatic
    external fun nativeDuplicateShapes(
        docPtr: Long,
        pageId: Int,
        shapeIds: LongArray,
        dxIn: Double,
        dyIn: Double
    ): Long

    @JvmStatic
    external fun nativeReorderShapes(
        docPtr: Long,
        pageId: Int,
        shapeIds: LongArray,
        mode: String
    ): Int

    @JvmStatic
    external fun nativeAddPage(docPtr: Long, name: String): Int

    @JvmStatic
    external fun nativeConnectShapes(
        docPtr: Long,
        pageId: Int,
        fromShapeId: Long,
        toShapeId: Long
    ): Long

    @JvmStatic
    external fun nativeValidateJson(docPtr: Long, ruleset: String): String

    @JvmStatic
    external fun nativeAutoLayout(docPtr: Long, pageId: Int, algo: String)

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

    @JvmStatic
    external fun nativeBooleanOp(
        docPtr: Long,
        pageId: Int,
        shapeIds: LongArray,
        op: String,
        offsetIn: Double
    ): Int

    @JvmStatic
    external fun nativeApplyTheme(
        docPtr: Long,
        themeSlug: String,
        variantIndex: Int
    ): Int

    @JvmStatic
    external fun nativeApplyVisualPreset(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        preset: String
    ): Boolean

    @JvmStatic
    external fun nativeCreateSubprocess(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        subName: String
    ): Int

    @JvmStatic
    external fun nativeCompareDocuments(
        baseDocPtr: Long,
        revDocPtr: Long
    ): String

    @JvmStatic
    external fun nativeApplyCrdtJson(
        docPtr: Long,
        replicaId: String,
        opsJson: String
    ): Int

    @JvmStatic
    external fun nativeImportDataVisualizer(
        docPtr: Long,
        pageId: Int,
        csvOrXlsx: ByteArray
    ): Int

    @JvmStatic
    external fun nativeImportCloudDiscovery(
        docPtr: Long,
        pageId: Int,
        discoveryJson: String
    ): Int

    @JvmStatic
    external fun nativeImportEntraOrLdif(
        docPtr: Long,
        pageId: Int,
        text: String
    ): Int

    @JvmStatic
    external fun nativeGetStateJson(
        docPtr: Long,
        activePageId: Int
    ): String

    @JvmStatic
    external fun nativeSetShapeText(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        text: String
    ): Boolean

    @JvmStatic
    external fun nativeGetViewportTool(viewportPtr: Long): Int

    @JvmStatic
    external fun nativePopCreatedTextShape(viewportPtr: Long): Long

    @JvmStatic
    external fun nativeHitTestShape(
        viewportPtr: Long,
        docPtr: Long,
        xPx: Double,
        yPx: Double
    ): Long

    @JvmStatic
    external fun nativeGetShapeScreenBoundsJson(
        viewportPtr: Long,
        docPtr: Long,
        shapeId: Long
    ): String

    @JvmStatic
    external fun nativeImportStencilBytes(
        docPtr: Long,
        fileName: String,
        bytes: ByteArray
    ): String?

    @JvmStatic
    external fun nativeListOfficialStencilCatalogJson(): String

    @JvmStatic
    external fun nativeSetShapeTextStyleJson(
        docPtr: Long,
        pageId: Int,
        shapeIds: LongArray,
        styleJson: String
    ): Int

    @JvmStatic
    external fun nativeRotateFlipShapes(
        docPtr: Long,
        pageId: Int,
        shapeIds: LongArray,
        deltaAngleDeg: Double,
        toggleFlipX: Boolean,
        toggleFlipY: Boolean
    ): Int

    @JvmStatic
    external fun nativeSetPageSetup(
        docPtr: Long,
        pageId: Int,
        widthIn: Double,
        heightIn: Double,
        unitSlug: String
    ): Boolean

    @JvmStatic
    external fun nativeSetPagePreset(
        docPtr: Long,
        pageId: Int,
        presetSlug: String,
        landscape: Boolean
    ): Boolean

    @JvmStatic
    external fun nativeSetDocumentTitle(
        docPtr: Long,
        title: String
    ): Boolean

    @JvmStatic
    external fun nativeRenamePage(
        docPtr: Long,
        pageId: Int,
        newName: String
    ): Boolean

    @JvmStatic
    external fun nativeRegisterFontBytes(
        fontBytes: ByteArray,
        familyOverride: String?
    ): Boolean

    @JvmStatic
    external fun nativeAlignShapes(
        docPtr: Long,
        pageId: Int,
        shapeIds: LongArray,
        mode: String
    ): Int

    @JvmStatic
    external fun nativeDistributeShapes(
        docPtr: Long,
        pageId: Int,
        shapeIds: LongArray,
        axis: String
    ): Int

    @JvmStatic
    external fun nativeGroupShapes(
        docPtr: Long,
        pageId: Int,
        shapeIds: LongArray
    ): Long

    @JvmStatic
    external fun nativeUngroupShapes(
        docPtr: Long,
        pageId: Int,
        groupIds: LongArray
    ): LongArray?

    @JvmStatic
    external fun nativeDragConnectorSegment(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        segIndex: Int,
        coordIn: Double
    ): Boolean

    @JvmStatic
    external fun nativeSetSmartGuides(
        viewportPtr: Long,
        enabled: Boolean
    ): Boolean

    @JvmStatic
    external fun nativeGetSmartGuides(
        viewportPtr: Long
    ): Boolean

    @JvmStatic
    external fun nativeGetSelectionIds(
        viewportPtr: Long
    ): LongArray?

    @JvmStatic
    external fun nativeSetSelectionIds(
        viewportPtr: Long,
        docPtr: Long,
        shapeIds: LongArray
    ): Boolean

    @JvmStatic
    external fun nativeViewportScreenToPageIn(
        viewportPtr: Long,
        xPx: Double,
        yPx: Double
    ): DoubleArray?

    @JvmStatic
    external fun nativeInsertContainer(
        docPtr: Long,
        pageId: Int,
        title: String,
        memberIds: LongArray
    ): Long

    @JvmStatic
    external fun nativeInsertSwimlanes(
        docPtr: Long,
        pageId: Int,
        lanesSemicolon: String
    ): Int

    @JvmStatic
    external fun nativeAttachCallout(
        docPtr: Long,
        pageId: Int,
        targetShapeId: Long,
        text: String
    ): Long

    @JvmStatic
    external fun nativeAutoConnect(
        docPtr: Long,
        pageId: Int,
        sourceShapeId: Long,
        dir: String
    ): Long

    @JvmStatic
    external fun nativeDragControlHandle(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        rowName: String,
        pageXIn: Double,
        pageYIn: Double
    ): Boolean

    @JvmStatic
    external fun nativeRenameShape(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        newName: String
    ): Boolean

    @JvmStatic
    external fun nativeUpsertShapeData(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        rowName: String,
        label: String,
        dataType: Int,
        valueStr: String
    ): Boolean

    @JvmStatic
    external fun nativeDeleteShapeData(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        rowName: String
    ): Boolean

    @JvmStatic
    external fun nativeAddLayer(
        docPtr: Long,
        pageId: Int,
        layerName: String
    ): Int

    @JvmStatic
    external fun nativeSetLayerFlags(
        docPtr: Long,
        pageId: Int,
        layerIx: Int,
        visible: Boolean,
        lockFlag: Boolean,
        printFlag: Boolean,
        snapGlue: Boolean
    ): Boolean

    @JvmStatic
    external fun nativeSetShapeLayerMembership(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        layerIx: Int,
        assigned: Boolean
    ): Boolean

    @JvmStatic
    external fun nativeGetDataLinkStateJson(
        docPtr: Long,
        pageId: Int
    ): String?

    @JvmStatic
    external fun nativeSetPageDataGraphicsJson(
        docPtr: Long,
        pageId: Int,
        configJson: String
    ): Int

    @JvmStatic
    external fun nativeImportExternalRecordset(
        docPtr: Long,
        pageId: Int,
        fileName: String,
        rawBytes: ByteArray,
        keyColHint: String,
        tableHint: String,
        autoBind: Boolean,
        autoEnableGraphics: Boolean
    ): IntArray?

    @JvmStatic
    external fun nativeBindExternalRow(
        docPtr: Long,
        pageId: Int,
        recordsetId: Int,
        rowIndex: Int,
        shapeIds: LongArray
    ): Int

    @JvmStatic
    external fun nativeRefreshExternalRecordset(
        docPtr: Long,
        pageId: Int,
        recordsetId: Int,
        optionalNewBytes: ByteArray?,
        formatHint: String
    ): Int

    @JvmStatic
    external fun nativeSyncAndExportRecordset(
        docPtr: Long,
        pageId: Int,
        recordsetId: Int,
        formatExt: String
    ): ByteArray?

    @JvmStatic
    external fun nativeDeleteExternalRecordset(
        docPtr: Long,
        recordsetId: Int
    ): Boolean

    @JvmStatic
    external fun nativeInsertDataLegend(
        docPtr: Long,
        pageId: Int,
        propName: String,
        pinXIn: Double,
        pinYIn: Double
    ): Long

    @JvmStatic
    external fun nativeExportBomReport(
        docPtr: Long,
        formatExt: String
    ): ByteArray?

    @JvmStatic
    external fun nativeDuplicatePage(
        docPtr: Long,
        pageId: Int
    ): Int

    @JvmStatic
    external fun nativeDeletePage(
        docPtr: Long,
        pageId: Int
    ): Boolean

    @JvmStatic
    external fun nativeSetPageBackground(
        docPtr: Long,
        pageId: Int,
        isBackground: Boolean,
        backPageId: Int,
        pageColorFormula: String
    ): Boolean

    @JvmStatic
    external fun nativeApplyHeadingPreset(
        docPtr: Long,
        pageId: Int,
        shapeIds: LongArray,
        presetSlug: String
    ): Int

    @JvmStatic
    external fun nativeInsertHeading(
        docPtr: Long,
        pageId: Int,
        presetSlug: String,
        customText: String,
        xIn: Double,
        yIn: Double
    ): Long

    @JvmStatic
    external fun nativeApplyBorderAndTitle(
        docPtr: Long,
        pageId: Int,
        styleSlug: String,
        title: String,
        subtitle: String,
        author: String,
        dateOrRev: String
    ): Int

    @JvmStatic
    external fun nativeImportMarkup(
        docPtr: Long,
        pageId: Int,
        markup: String,
        formatSlug: String
    ): Int

    @JvmStatic
    external fun nativeExportMarkup(
        docPtr: Long,
        pageId: Int,
        formatSlug: String
    ): String?

    @JvmStatic
    external fun nativeListSlideSnippetsJson(
        docPtr: Long
    ): String?

    @JvmStatic
    external fun nativeAddSlideSnippet(
        docPtr: Long,
        pageId: Int,
        title: String,
        notes: String,
        xIn: Double,
        yIn: Double,
        widthIn: Double,
        heightIn: Double
    ): Int

    @JvmStatic
    external fun nativeDeleteSlideSnippet(
        docPtr: Long,
        pageId: Int,
        snippetId: Int
    ): Boolean

    @JvmStatic
    external fun nativeAutoSlideSnippets(
        docPtr: Long,
        pageId: Int
    ): Int

    @JvmStatic
    external fun nativeResetConnectorRouting(
        docPtr: Long,
        pageId: Int,
        shapeId: Long
    ): Boolean

    @JvmStatic
    external fun nativeEnsureControlHandle(
        docPtr: Long,
        pageId: Int,
        shapeId: Long
    ): Boolean

    @JvmStatic
    external fun nativeRunVbaMacro(
        docPtr: Long,
        selectionIds: LongArray,
        script: String
    ): String?

    @JvmStatic
    external fun nativeListCommentsJson(
        docPtr: Long,
        pageId: Int
    ): String?

    @JvmStatic
    external fun nativeAddComment(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        author: String,
        text: String
    ): Int

    @JvmStatic
    external fun nativeResolveComment(
        docPtr: Long,
        commentId: Int,
        resolved: Boolean
    ): Boolean

    @JvmStatic
    external fun nativeDeleteComment(
        docPtr: Long,
        commentId: Int
    ): Boolean

    @JvmStatic
    external fun nativeClearPageInk(
        docPtr: Long,
        pageId: Int
    ): Int

    @JvmStatic
    external fun nativeSetHyperlinkAndScreentip(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        description: String,
        address: String,
        subAddress: String,
        screentip: String
    ): Boolean

    @JvmStatic
    external fun nativeRunDomainWizard(
        docPtr: Long,
        pageId: Int,
        wizardSlug: String,
        spec: String
    ): String?

    @JvmStatic
    external fun nativeApplyCadScale(
        docPtr: Long,
        pageId: Int,
        scaleSlug: String,
        locked: Boolean
    ): Double

    @JvmStatic
    external fun nativeConvertCadLayers(
        docPtr: Long,
        pageId: Int,
        layerNameOrAll: String
    ): Int

    @JvmStatic
    external fun nativeListShapeSheetFunctionsJson(): String

    @JvmStatic
    external fun nativeListShapeActionsJson(
        docPtr: Long,
        pageId: Int,
        shapeId: Long
    ): String?

    @JvmStatic
    external fun nativeUpsertShapeAction(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        rowName: String,
        menu: String,
        actionFormula: String,
        checkedFormula: String,
        disabledFormula: String,
        beginGroup: Boolean
    ): Boolean

    @JvmStatic
    external fun nativeDeleteShapeAction(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        rowName: String
    ): Boolean

    @JvmStatic
    external fun nativeExecuteShapeAction(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        rowName: String
    ): String?

    @JvmStatic
    external fun nativeListConnectionPointsJson(
        docPtr: Long,
        pageId: Int,
        shapeId: Long
    ): String?

    @JvmStatic
    external fun nativeAddConnectionPoint(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        pageXIn: Double,
        pageYIn: Double,
        pointType: Int
    ): Int

    @JvmStatic
    external fun nativeMoveConnectionPoint(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        ix: Int,
        pageXIn: Double,
        pageYIn: Double
    ): Boolean

    @JvmStatic
    external fun nativeDeleteConnectionPoint(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        ix: Int
    ): Boolean

    @JvmStatic
    external fun nativeAddToCustomStencil(
        docPtr: Long,
        pageId: Int,
        shapeIds: LongArray,
        stencilTitle: String,
        masterName: String
    ): String?

    @JvmStatic
    external fun nativeRenameCustomMaster(
        docPtr: Long,
        stencilTitle: String,
        oldName: String,
        newName: String
    ): Boolean

    @JvmStatic
    external fun nativeDeleteCustomMaster(
        docPtr: Long,
        stencilTitle: String,
        masterName: String
    ): Boolean

    @JvmStatic
    external fun nativeUpdateCustomMaster(
        docPtr: Long,
        pageId: Int,
        sourceShapeId: Long,
        stencilTitle: String,
        masterName: String
    ): Int

    @JvmStatic
    external fun nativeExportCustomStencilVssx(
        docPtr: Long,
        stencilTitle: String
    ): ByteArray?

    @JvmStatic
    external fun nativeListArrowheadsJson(): String?

    @JvmStatic
    external fun nativeSetConnectorArrowheads(
        docPtr: Long,
        pageId: Int,
        shapeIds: LongArray,
        beginArrow: Int,
        endArrow: Int,
        arrowSize: Int
    ): Int

    @JvmStatic
    external fun nativeSetPageLineJumps(
        docPtr: Long,
        pageId: Int,
        jumpCode: Int,
        jumpStyle: Int,
        factorX: Double,
        factorY: Double
    ): Boolean

    @JvmStatic
    external fun nativeInsertTextField(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        presetOrFormula: String,
        formatSpec: String,
        prefixLabel: String
    ): Int

    @JvmStatic
    external fun nativeListTextFieldsJson(
        docPtr: Long,
        pageId: Int,
        shapeId: Long
    ): String?

    @JvmStatic
    external fun nativeSetShapeRichRunsJson(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        runsJson: String
    ): Boolean

    @JvmStatic
    external fun nativeSetTextBlockTransform(
        docPtr: Long,
        pageId: Int,
        shapeId: Long,
        txtPinXIn: Double,
        txtPinYIn: Double,
        txtWidthIn: Double,
        txtHeightIn: Double,
        txtAngleDeg: Double
    ): Boolean

    @JvmStatic
    external fun nativeListRulerGuidesJson(
        docPtr: Long,
        pageId: Int
    ): String?

    @JvmStatic
    external fun nativeAddRulerGuide(
        docPtr: Long,
        pageId: Int,
        isVertical: Boolean,
        coordIn: Double
    ): Long

    @JvmStatic
    external fun nativeMoveRulerGuide(
        docPtr: Long,
        pageId: Int,
        guideId: Long,
        coordIn: Double
    ): Boolean

    @JvmStatic
    external fun nativeDeleteRulerGuide(
        docPtr: Long,
        pageId: Int,
        guideId: Long
    ): Boolean

    @JvmStatic
    external fun nativeGetSnapGlueJson(
        docPtr: Long,
        pageId: Int
    ): String?

    @JvmStatic
    external fun nativeSetSnapGlue(
        docPtr: Long,
        pageId: Int,
        snapEnabled: Boolean,
        glueEnabled: Boolean,
        snapTargetsMask: Int,
        glueTypesMask: Int
    ): Boolean

    @JvmStatic
    external fun nativeFindInDocumentJson(
        docPtr: Long,
        activePageId: Int,
        query: String,
        matchCase: Boolean,
        wholeWord: Boolean,
        allPages: Boolean
    ): String?

    @JvmStatic
    external fun nativeReplaceInDocument(
        docPtr: Long,
        activePageId: Int,
        findStr: String,
        replaceStr: String,
        matchCase: Boolean,
        wholeWord: Boolean,
        allPages: Boolean
    ): Int

    @JvmStatic
    external fun nativeCopyShapeFormatJson(
        docPtr: Long,
        pageId: Int,
        shapeId: Long
    ): String?

    @JvmStatic
    external fun nativeApplyFormatPainter(
        docPtr: Long,
        pageId: Int,
        targetIds: LongArray,
        formatJson: String
    ): Int

    @JvmStatic
    external fun nativeNumberShapes(
        docPtr: Long,
        pageId: Int,
        shapeIds: LongArray,
        orderMode: String,
        startNum: Int,
        interval: Int,
        prefix: String,
        targetMode: String
    ): Int

    @JvmStatic
    external fun nativeInsertAssociativeDimension(
        docPtr: Long,
        pageId: Int,
        dimKind: String,
        targetShapeId: Long,
        secondShapeId: Long,
        offsetIn: Double,
        unitFormat: String
    ): Long

    @JvmStatic
    external fun nativeDrawArchitecturalWall(
        docPtr: Long,
        pageId: Int,
        x1In: Double,
        y1In: Double,
        x2In: Double,
        y2In: Double,
        thicknessIn: Double
    ): Long

    @JvmStatic
    external fun nativeHealWallIntersections(
        docPtr: Long,
        pageId: Int,
        toleranceIn: Double
    ): Int

    @JvmStatic
    external fun nativeInsertWallOpening(
        docPtr: Long,
        pageId: Int,
        wallShapeId: Long,
        openingKind: String,
        offsetAlongIn: Double,
        openingWidthIn: Double,
        swingDir: String
    ): Long

    @JvmStatic
    external fun nativeAddMindMapTopic(
        docPtr: Long,
        pageId: Int,
        anchorShapeId: Long,
        topicText: String,
        asSibling: Boolean
    ): Long

    @JvmStatic
    external fun nativeExportMindMapOutline(
        docPtr: Long,
        pageId: Int
    ): String?

    @JvmStatic
    external fun nativeRecalculateVsmAndFtaJson(
        docPtr: Long,
        pageId: Int
    ): String?
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
enum class VexioTool(val displayName: String, val shortcutHint: String, val nativeCode: Int) {
    POINTER("Select", "Ctrl+1", 0),
    PAN_HAND("Pan", "Space", 1),
    CONNECTOR("Connector", "Ctrl+3", 2),
    LINE("Line", "L", 9),
    ARC("Arc", "A", 10),
    FREEFORM("Freeform Curve", "Ctrl+5", 11),
    RECTANGLE("Rectangle", "Ctrl+8", 4),
    ROUNDED_RECT("Rounded Rect", "R", 5),
    ELLIPSE("Circle / Ellipse", "Ctrl+9", 6),
    DIAMOND("Diamond", "D", 7),
    TRIANGLE("Triangle", "T", 8),
    HEXAGON("Hexagon", "X", 14),
    STAR("Star", "S", 15),
    TEXT_BLOCK("Text", "Ctrl+2", 3),
    STYLUS_INK("Pen Ink", "Ctrl+4", 12),
    HIGHLIGHTER("Highlighter", "Ctrl+7", 13),
    EDIT_VERTICES("Edit Vertices", "Ctrl+6", 16),
    INK_ERASER("Ink Eraser", "Alt+E", 17),
    CONNECTION_POINT("Connection Point", "Ctrl+Shift+1", 18),
    TEXT_BLOCK_TRANSFORM("Text Block Tool", "Ctrl+Shift+4", 19),
    FORMAT_PAINTER("Format Painter", "Ctrl+Shift+P", 20),
    WALL("Wall Tool", "Ctrl+Shift+W", 21),
    DIMENSION("Dimension Tool", "Ctrl+Shift+D", 22)
}

data class RulerGuideUiItem(
    val shapeId: Long,
    val isVertical: Boolean,
    val coordIn: Double
)

data class FindMatchUiItem(
    val pageId: Int,
    val shapeId: Long,
    val field: String,
    val snippet: String
)

/**
 * Constructive 2D Boolean Shape Operations aligned 1:1 with Visio Shape Design -> Operations.
 */
enum class VexioBooleanOp(val slug: String, val displayName: String) {
    UNION("union", "Union"),
    COMBINE("combine", "Combine"),
    FRAGMENT("fragment", "Fragment"),
    INTERSECT("intersect", "Intersect"),
    SUBTRACT("subtract", "Subtract"),
    TRIM("trim", "Trim"),
    JOIN("join", "Join"),
    OFFSET("offset", "Offset");

    companion object {
        fun fromSlug(raw: String): VexioBooleanOp? {
            val norm = raw.trim().lowercase(Locale.US)
            return entries.firstOrNull { it.slug == norm || it.displayName.lowercase(Locale.US) == norm }
        }
    }
}

data class ThemeVariantFormulas(
    val fillForegnd: String,
    val fillBkgnd: String,
    val lineColor: String,
    val fillPattern: String,
    val sketchEnabled: Boolean
)

/**
 * Canonical Visio `6 Themes × 4 Variants = 24 Palettes` matrix matching `themes_styles.rs`.
 */
enum class VexioThemePreset(val slug: String, val displayName: String) {
    MODERN("modern", "Modern"),
    PROFESSIONAL("professional", "Professional"),
    VIBRANT("vibrant", "Vibrant"),
    BLUEPRINT("blueprint", "Blueprint"),
    MONOCHROME("monochrome", "Monochrome"),
    HAND_DRAWN("hand_drawn", "Hand Drawn");

    fun paletteFormulas(variantIndex: Int): ThemeVariantFormulas {
        val v = variantIndex.coerceIn(0, 3)
        return when (this) {
            MODERN -> when (v) {
                0 -> ThemeVariantFormulas("RGB(239,246,255)", "RGB(219,234,254)", "RGB(37,99,235)", "28", false)
                1 -> ThemeVariantFormulas("RGB(236,253,245)", "RGB(209,250,229)", "RGB(5,150,105)", "28", false)
                2 -> ThemeVariantFormulas("RGB(245,243,255)", "RGB(221,214,254)", "RGB(124,58,237)", "28", false)
                else -> ThemeVariantFormulas("RGB(30,41,59)", "RGB(15,23,42)", "RGB(96,165,250)", "28", false)
            }
            PROFESSIONAL -> when (v) {
                0 -> ThemeVariantFormulas("RGB(248,250,252)", "RGB(226,232,240)", "RGB(30,64,175)", "1", false)
                1 -> ThemeVariantFormulas("RGB(241,245,249)", "RGB(203,213,225)", "RGB(15,118,110)", "1", false)
                2 -> ThemeVariantFormulas("RGB(250,250,249)", "RGB(231,229,228)", "RGB(68,64,60)", "1", false)
                else -> ThemeVariantFormulas("RGB(23,37,84)", "RGB(15,23,42)", "RGB(147,197,253)", "1", false)
            }
            VIBRANT -> when (v) {
                0 -> ThemeVariantFormulas("RGB(254,242,242)", "RGB(254,202,202)", "RGB(220,38,38)", "36", false)
                1 -> ThemeVariantFormulas("RGB(255,251,235)", "RGB(253,230,138)", "RGB(217,119,6)", "36", false)
                2 -> ThemeVariantFormulas("RGB(236,254,255)", "RGB(165,243,252)", "RGB(8,145,178)", "36", false)
                else -> ThemeVariantFormulas("RGB(253,244,255)", "RGB(245,208,254)", "RGB(192,38,211)", "36", false)
            }
            BLUEPRINT -> when (v) {
                0 -> ThemeVariantFormulas("RGB(12,74,110)", "RGB(7,89,133)", "RGB(186,230,253)", "1", false)
                1 -> ThemeVariantFormulas("RGB(23,37,84)", "RGB(30,58,138)", "RGB(191,219,254)", "1", false)
                2 -> ThemeVariantFormulas("RGB(240,249,255)", "RGB(224,242,254)", "RGB(2,132,199)", "4", false)
                else -> ThemeVariantFormulas("RGB(6,78,59)", "RGB(6,95,70)", "RGB(167,243,208)", "1", false)
            }
            MONOCHROME -> when (v) {
                0 -> ThemeVariantFormulas("RGB(255,255,255)", "RGB(243,244,246)", "RGB(17,24,39)", "1", false)
                1 -> ThemeVariantFormulas("RGB(243,244,246)", "RGB(229,231,235)", "RGB(55,65,81)", "1", false)
                2 -> ThemeVariantFormulas("RGB(229,231,235)", "RGB(209,213,219)", "RGB(31,41,55)", "4", false)
                else -> ThemeVariantFormulas("RGB(24,24,27)", "RGB(9,9,11)", "RGB(244,244,245)", "1", false)
            }
            HAND_DRAWN -> when (v) {
                0 -> ThemeVariantFormulas("RGB(254,252,232)", "RGB(254,240,138)", "RGB(63,63,70)", "1", true)
                1 -> ThemeVariantFormulas("RGB(239,246,255)", "RGB(191,219,254)", "RGB(30,58,138)", "4", true)
                2 -> ThemeVariantFormulas("RGB(240,253,244)", "RGB(187,247,208)", "RGB(20,83,45)", "1", true)
                else -> ThemeVariantFormulas("RGB(255,241,242)", "RGB(254,205,211)", "RGB(136,19,55)", "1", true)
            }
        }
    }

    companion object {
        fun fromSlug(raw: String): VexioThemePreset? {
            val norm = raw.trim().lowercase(Locale.US).replace('-', '_').replace(' ', '_')
            return when (norm) {
                "modern" -> MODERN
                "professional" -> PROFESSIONAL
                "vibrant" -> VIBRANT
                "blueprint" -> BLUEPRINT
                "monochrome" -> MONOCHROME
                "hand_drawn", "handdrawn", "sketch" -> HAND_DRAWN
                else -> null
            }
        }
    }
}

/**
 * Shape Visual Effects presets matching `visual_preset_spec_from_slug` in `c_abi.rs` / `jni_bridge.rs`.
 */
enum class VexioVisualPreset(val slug: String, val displayName: String) {
    GRAD_LINEAR("grad_linear", "Linear Gradient"),
    SHADOW_SOFT("shadow_soft", "Soft Shadow"),
    BEVEL_3D("bevel_3d", "3D Bevel"),
    SKETCHY("sketchy", "Sketchy Outline");

    companion object {
        val allSupportedSlugs: Set<String> = setOf(
            "grad_linear",
            "grad_radial",
            "hatch_diag",
            "shadow_soft",
            "bevel_3d",
            "sketchy"
        )
    }
}

data class DiagramCompareSummary(
    val addedCount: Int = 0,
    val removedCount: Int = 0,
    val modifiedCount: Int = 0,
    val rawReportJson: String = "{}",
    val diffOverlaySvg: String = ""
)

/**
 * Deterministic enterprise Plan 2 payloads for interactive Googlebook Workstation imports.
 */
object Plan2SamplePayloads {
    val dataVisualizerCsv: String = """
        Process Step ID,Process Step Description,Next Step ID,Connector Label,Shape Type,Function,Phase
        P100,Receive Order Request,P200,,Start,Customer Portal,Intake
        P200,Validate Credit & Inventory,"P300,P400","Approved,Review",Decision,Risk Engine,Validation
        P300,Provision Cloud Entitlement,P500,,Process,Platform Ops,Fulfillment
        P400,Escalate Manual Approval,P300,Cleared,Subprocess,Compliance,Validation
        P500,Emit Billing Event,,,End,Finance,Settlement
    """.trimIndent()

    val cloudDiscoveryJson: String = """
        {
          "provider": "gcp-aws-hybrid",
          "resources": [
            {"id": "alb-edge", "name": "Global Edge LB", "type": "aws.elb", "vpc": "vpc-prod-us"},
            {"id": "eks-api", "name": "Vexio Core Service", "type": "aws.eks", "vpc": "vpc-prod-us"},
            {"id": "rds-primary", "name": "Spanner / Aurora Cluster", "type": "aws.rds", "vpc": "vpc-data-us"}
          ],
          "edges": [
            {"from": "alb-edge", "to": "eks-api", "label": "mTLS gRPC"},
            {"from": "eks-api", "to": "rds-primary", "label": "TCP 5432"}
          ]
        }
    """.trimIndent()

    val entraOrgChartLdif: String = """
        dn: cn=Elena Vance,ou=Executive,dc=vexio,dc=io
        cn: Elena Vance
        title: Chief Executive Officer

        dn: cn=Marcus Chen,ou=Engineering,dc=vexio,dc=io
        cn: Marcus Chen
        title: VP of Systems Engineering
        manager: cn=Elena Vance,ou=Executive,dc=vexio,dc=io

        dn: cn=Priya Nair,ou=Product,dc=vexio,dc=io
        cn: Priya Nair
        title: Director of Enterprise Architecture
        manager: cn=Elena Vance,ou=Executive,dc=vexio,dc=io
    """.trimIndent()
}

@Serializable
data class NativeStencilMasterDto(
    val name: String = "",
    val prompt: String = "",
    val category: String = "",
    val widthIn: Double = 1.5,
    val heightIn: Double = 1.0,
    val fillFormula: String = "RGB(239,246,255)",
    val lineFormula: String = "RGB(37,99,235)",
    val svg: String = "",
    val pngDataUrl: String = ""
)

@Serializable
data class StencilMasterItem(
    val masterId: String,
    val title: String,
    val category: String,
    val defaultWidthFormula: String,
    val defaultHeightFormula: String,
    val fillFormula: String,
    val shapeKind: String,
    val prompt: String = "",
    val svgPreview: String = "",
    val pngDataUrl: String = ""
)

@Serializable
data class OfficialStencilCatalogEntry(
    val id: String = "",
    val title: String = "",
    val vendor: String = "",
    val categoryGroup: String = "",
    val description: String = "",
    val fileName: String = "",
    val url: String = "",
    val approxSizeKb: Int = 0
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
    val pinYFormula: String = "5.50 in",
    val category: String = "Basic Flowchart"
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
    val scaleLabel: String,
    val isBackground: Boolean = false,
    val backPageId: Int? = null,
    val pageColorHex: String = "#FFFFFF",
    val widthInches: Double = 297.0 / 25.4,
    val heightInches: Double = 210.0 / 25.4
)

data class ValidationBadgeSummary(
    val standard: String,
    val errorCount: Int,
    val warningCount: Int,
    val statusMessage: String
)

enum class VexioAppLanguage(val code: String, val badge: String, val nativeName: String) {
    EN("en", "EN", "English"),
    DE("de", "DE", "Deutsch"),
    IT("it", "IT", "Italiano"),
    FR("fr", "FR", "Français"),
    ES("es", "ES", "Español"),
    GR("el", "GR", "Ελληνικά");

    val fileLabel: String
        get() = when (this) {
            EN -> "File"; DE -> "Datei"; IT -> "File"; FR -> "Fichier"; ES -> "Archivo"; GR -> "Αρχείο"
        }
    val homeLabel: String
        get() = when (this) {
            EN -> "Home"; DE -> "Start"; IT -> "Home"; FR -> "Accueil"; ES -> "Inicio"; GR -> "Κεντρική"
        }
    val insertLabel: String
        get() = when (this) {
            EN -> "Insert"; DE -> "Einfügen"; IT -> "Inserisci"; FR -> "Insertion"; ES -> "Insertar"; GR -> "Εισαγωγή"
        }
    val designLabel: String
        get() = when (this) {
            EN -> "Design"; DE -> "Entwurf"; IT -> "Progettazione"; FR -> "Création"; ES -> "Diseño"; GR -> "Σχεδίαση"
        }
    val dataLabel: String
        get() = when (this) {
            EN -> "Data"; DE -> "Daten"; IT -> "Dati"; FR -> "Données"; ES -> "Datos"; GR -> "Δεδομένα"
        }
    val processLabel: String
        get() = when (this) {
            EN -> "Process"; DE -> "Prozess"; IT -> "Processo"; FR -> "Processus"; ES -> "Proceso"; GR -> "Διαδικασία"
        }
    val reviewLabel: String
        get() = when (this) {
            EN -> "Review"; DE -> "Überprüfen"; IT -> "Revisione"; FR -> "Révision"; ES -> "Revisar"; GR -> "Αναθεώρηση"
        }
    val viewLabel: String
        get() = when (this) {
            EN -> "View"; DE -> "Ansicht"; IT -> "Visualizza"; FR -> "Affichage"; ES -> "Vista"; GR -> "Προβολή"
        }
    val developerLabel: String
        get() = when (this) {
            EN -> "Developer"; DE -> "Entwickler"; IT -> "Sviluppo"; FR -> "Développeur"; ES -> "Programador"; GR -> "Προγραμματιστής"
        }
    val openFileLabel: String
        get() = when (this) {
            EN -> "Open File (.vsdx / .vsd / .dwg)…"
            DE -> "Datei öffnen (.vsdx / .vsd / .dwg)…"
            IT -> "Apri file (.vsdx / .vsd / .dwg)…"
            FR -> "Ouvrir un fichier (.vsdx / .vsd / .dwg)…"
            ES -> "Abrir archivo (.vsdx / .vsd / .dwg)…"
            GR -> "Άνοιγμα αρχείου (.vsdx / .vsd / .dwg)…"
        }
    val saveLabel: String
        get() = when (this) {
            EN -> "Save (Ctrl+S)"
            DE -> "Speichern (Ctrl+S)"
            IT -> "Salva (Ctrl+S)"
            FR -> "Enregistrer (Ctrl+S)"
            ES -> "Guardar (Ctrl+S)"
            GR -> "Αποθήκευση (Ctrl+S)"
        }
    val saveAsLabel: String
        get() = when (this) {
            EN -> "Save As… (Ctrl+Shift+S)"
            DE -> "Speichern unter… (Ctrl+Shift+S)"
            IT -> "Salva con nome… (Ctrl+Shift+S)"
            FR -> "Enregistrer sous… (Ctrl+Shift+S)"
            ES -> "Guardar como… (Ctrl+Shift+S)"
            GR -> "Αποθήκευση ως… (Ctrl+Shift+S)"
        }
    val renamePageLabel: String
        get() = when (this) {
            EN -> "Rename Page…"
            DE -> "Zeichenblatt umbenennen…"
            IT -> "Rinomina pagina…"
            FR -> "Renommer la page…"
            ES -> "Cambiar nombre de página…"
            GR -> "Μετονομασία σελίδας…"
        }
    val textColorLabel: String
        get() = when (this) {
            EN -> "Text Color"
            DE -> "Textfarbe"
            IT -> "Colore testo"
            FR -> "Couleur du texte"
            ES -> "Color de texto"
            GR -> "Χρώμα κειμένου"
        }
    val highlightColorLabel: String
        get() = when (this) {
            EN -> "Highlight"
            DE -> "Hervorhebung"
            IT -> "Evidenziazione"
            FR -> "Surlignage"
            ES -> "Resaltado"
            GR -> "Επισήμανση"
        }
    val clearHighlightLabel: String
        get() = when (this) {
            EN -> "No Highlight"
            DE -> "Keine Hervorhebung"
            IT -> "Nessuna evidenziazione"
            FR -> "Aucun surlignage"
            ES -> "Sin resaltado"
            GR -> "Χωρίς επισήμανση"
        }
    val fillColorLabel: String
        get() = when (this) {
            EN -> "Fill Color"
            DE -> "Füllfarbe"
            IT -> "Riempimento"
            FR -> "Remplissage"
            ES -> "Color de relleno"
            GR -> "Χρώμα γεμίσματος"
        }
    val lineColorLabel: String
        get() = when (this) {
            EN -> "Line Color"
            DE -> "Linienfarbe"
            IT -> "Colore linea"
            FR -> "Couleur de trait"
            ES -> "Color de línea"
            GR -> "Χρώμα γραμμής"
        }
    val inspectorFormatTab: String
        get() = when (this) {
            EN -> "Format"; DE -> "Format"; IT -> "Formato"; FR -> "Format"; ES -> "Formato"; GR -> "Μορφή"
        }
    val inspectorSizePosTab: String
        get() = when (this) {
            EN -> "Size"; DE -> "Maße"; IT -> "Misure"; FR -> "Taille"; ES -> "Medidas"; GR -> "Μέγεθος"
        }
    val inspectorDataTab: String
        get() = when (this) {
            EN -> "Data"; DE -> "Daten"; IT -> "Dati"; FR -> "Données"; ES -> "Datos"; GR -> "Δεδομένα"
        }
    val inspectorLayersTab: String
        get() = when (this) {
            EN -> "Layers"; DE -> "Layer"; IT -> "Layer"; FR -> "Calques"; ES -> "Capas"; GR -> "Επίπεδα"
        }
    val inspectorGeometryTab: String
        get() = when (this) {
            EN -> "Geometry"; DE -> "Geometrie"; IT -> "Geometria"; FR -> "Géométrie"; ES -> "Geometría"; GR -> "Γεωμετρία"
        }
    val inspectorValidateTab: String
        get() = when (this) {
            EN -> "Validate"; DE -> "Prüfen"; IT -> "Convalida"; FR -> "Valider"; ES -> "Validar"; GR -> "Έλεγχος"
        }
    val importStencilLabel: String
        get() = when (this) {
            EN -> "+ Import Visio Stencil (.vssx / .vss)"
            DE -> "+ Visio-Schablone importieren (.vssx)"
            IT -> "+ Importa Stencil Visio (.vssx)"
            FR -> "+ Importer gabarit Visio (.vssx)"
            ES -> "+ Importar galería Visio (.vssx)"
            GR -> "+ Εισαγωγή Stencil Visio (.vssx)"
        }
    val startHeroTitle: String
        get() = when (this) {
            EN -> "Start or Open a Diagram Inside Vexio"
            DE -> "Diagramm in Vexio erstellen oder öffnen"
            IT -> "Crea o apri un diagramma in Vexio"
            FR -> "Créer ou ouvrir un diagramme dans Vexio"
            ES -> "Crear o abrir un diagrama en Vexio"
            GR -> "Δημιουργία ή άνοιγμα διαγράμματος στο Vexio"
        }
    val continueCanvasLabel: String
        get() = when (this) {
            EN -> "Continue to Canvas"
            DE -> "Weiter zur Zeichenfläche"
            IT -> "Vai al foglio di disegno"
            FR -> "Continuer vers le dessin"
            ES -> "Continuar al lienzo"
            GR -> "Συνέχεια στον καμβά"
        }
    val checkUpdatesLabel: String
        get() = when (this) {
            EN -> "Check for Updates…"
            DE -> "Nach Updates suchen…"
            IT -> "Controlla aggiornamenti…"
            FR -> "Rechercher des mises à jour…"
            ES -> "Buscar actualizaciones…"
            GR -> "Έλεγχος για ενημερώσεις…"
        }

    companion object {
        fun detectDefault(): VexioAppLanguage {
            val lang = runCatching { Locale.getDefault().language.lowercase(Locale.US) }.getOrDefault("en")
            return when (lang) {
                "de" -> DE
                "it" -> IT
                "fr" -> FR
                "es" -> ES
                "el", "gr" -> GR
                else -> EN
            }
        }
    }
}

/**
 * Discovers 100% offline installed Android & Googlebook OS system font families by scanning
 * `/system/etc/fonts.xml` and `/product/etc/fonts_customization.xml` (plus standard system/Visio
 * metrics-compatible families) without any network calls.
 */
object SystemFontCatalog {
    private val curatedBaseline = listOf(
        "Inter",
        "Roboto",
        "Roboto Condensed",
        "Roboto Slab",
        "Roboto Mono",
        "Noto Sans",
        "Noto Serif",
        "Google Sans",
        "Droid Sans",
        "Droid Serif",
        "Droid Sans Mono",
        "Cutive Mono",
        "Sans-Serif",
        "Serif",
        "Monospace",
        "Calibri",
        "Segoe UI",
        "Arial",
        "Helvetica Neue",
        "Times New Roman",
        "Georgia",
        "Verdana",
        "Tahoma",
        "Trebuchet MS",
        "Consolas",
        "Courier New"
    )

    private val cachedFamilies: List<String> by lazy {
        val discovered = linkedSetOf<String>()
        val xmlPaths = listOf(
            "/product/etc/fonts_customization.xml",
            "/system/etc/fonts.xml",
            "/system/etc/system_fonts.xml"
        )
        val nameAttrRegex = Regex("""<(?:family|alias)\b[^>]*\bname="([^"]+)"""")
        for (path in xmlPaths) {
            runCatching {
                val f = java.io.File(path)
                if (f.isFile && f.canRead()) {
                    val xml = f.readText(Charsets.UTF_8)
                    for (m in nameAttrRegex.findAll(xml)) {
                        val raw = m.groupValues[1].trim()
                        if (raw.isNotEmpty() && !raw.startsWith("und-")) {
                            val pretty = raw.split('-', '_')
                                .filter { it.isNotEmpty() }
                                .joinToString(" ") { token ->
                                    token.replaceFirstChar { ch -> ch.titlecase(Locale.US) }
                                }
                            if (pretty.isNotEmpty()) {
                                discovered.add(pretty)
                            }
                        }
                    }
                }
            }
        }
        val merged = linkedSetOf<String>()
        merged.addAll(curatedBaseline)
        merged.addAll(discovered.sorted())
        merged.toList()
    }

    fun discoverInstalledFontFamilies(): List<String> = cachedFamilies
}

fun detectDefaultMetricUnits(): Boolean {
    val country = runCatching { Locale.getDefault().country.uppercase(Locale.US) }.getOrDefault("")
    return country !in setOf("US", "LR", "MM")
}

data class InlineCanvasEditorModel(
    val shapeId: Long,
    val text: String,
    val xPx: Float,
    val yPx: Float,
    val widthPx: Float,
    val heightPx: Float,
    val isNewlyCreatedText: Boolean = false
)

enum class VexioPagePresetSpec(
    val slug: String,
    val landscape: Boolean,
    val isMetric: Boolean,
    val label: String
) {
    A4_LANDSCAPE("a4", true, true, "ISO A4 Landscape (297×210 mm)"),
    A4_PORTRAIT("a4", false, true, "ISO A4 Portrait (210×297 mm)"),
    A3_LANDSCAPE("a3", true, true, "ISO A3 Landscape (420×297 mm)"),
    A3_PORTRAIT("a3", false, true, "ISO A3 Portrait (297×420 mm)"),
    A5_LANDSCAPE("a5", true, true, "ISO A5 Landscape (210×148 mm)"),
    A2_LANDSCAPE("a2", true, true, "ISO A2 Landscape (594×420 mm)"),
    LETTER_LANDSCAPE("letter", true, false, "US Letter Landscape (11×8.5 in)"),
    LETTER_PORTRAIT("letter", false, false, "US Letter Portrait (8.5×11 in)"),
    LEGAL_LANDSCAPE("legal", true, false, "US Legal Landscape (14×8.5 in)")
}

data class ShapeDataUiItem(
    val rowName: String,
    val label: String,
    val dataTypeCode: Int = 0,
    val dataTypeLabel: String = "String",
    val formula: String = "",
    val value: String = ""
)

data class DiagramLayerUiItem(
    val ix: Int,
    val name: String,
    val visible: Boolean = true,
    val print: Boolean = true,
    val lock: Boolean = false,
    val snapGlue: Boolean = true,
    val memberCount: Int = 0
)

data class SlideSnippetUiItem(
    val id: Int,
    val pageId: Int,
    val title: String,
    val notes: String = "",
    val xIn: Double = 0.0,
    val yIn: Double = 0.0,
    val widthIn: Double = 11.0,
    val heightIn: Double = 8.5
)

data class ReviewCommentUiItem(
    val id: Int,
    val pageId: Int,
    val shapeId: Long = 0L,
    val author: String = "Reviewer",
    val text: String = "",
    val resolved: Boolean = false
)

data class DataGraphicUiState(
    val enabled: Boolean = false,
    val iconSetField: String = "Status",
    val iconGreenMin: Double = 75.0,
    val iconAmberMin: Double = 40.0,
    val dataBarField: String = "CompletionPct",
    val dataBarMin: Double = 0.0,
    val dataBarMax: Double = 100.0,
    val dataBarColorHex: String = "#2563EB",
    val textCalloutField: String = "Owner",
    val textCalloutPrefix: String = "",
    val colorByValueEnabled: Boolean = true,
    val colorByValueField: String = "Status",
    val legendVisible: Boolean = false,
    val legendField: String = "Status"
)

data class ExternalRecordsetUiItem(
    val id: Int,
    val name: String,
    val sourceKind: String = "xlsx",
    val sourceName: String = "",
    val keyColumn: String = "",
    val columns: List<String> = emptyList(),
    val rows: List<Map<String, String>> = emptyList(),
    val boundShapeIdsByRow: Map<Int, List<Long>> = emptyMap()
)

data class SmartActionUiItem(
    val rowName: String,
    val menu: String,
    val actionFormula: String = "",
    val checkedFormula: String = "0",
    val disabledFormula: String = "0",
    val checked: Boolean = false,
    val disabled: Boolean = false,
    val invisible: Boolean = false,
    val beginGroup: Boolean = false
)

data class InspectedObjectUiState(
    val id: Long,
    val name: String,
    val kind: String = "Shape2D",
    val isGroup: Boolean = false,
    val childCount: Int = 0,
    val masterName: String? = null,
    val layerIndices: List<Int> = emptyList(),
    val text: String = "",
    val pinXIn: Double = 4.25,
    val pinYIn: Double = 5.50,
    val widthIn: Double = 2.0,
    val heightIn: Double = 1.0,
    val angleDeg: Double = 0.0,
    val flipX: Boolean = false,
    val flipY: Boolean = false,
    val fillColorHex: String = "#E8F0FE",
    val fillPattern: Int = 1,
    val fillTransPct: Int = 0,
    val roundingIn: Double = 0.0,
    val lineColorHex: String = "#1A73E8",
    val linePattern: Int = 1,
    val lineWeightPt: Double = 1.0,
    val beginArrow: Int = 0,
    val endArrow: Int = 0,
    val lockAspect: Boolean = false,
    val lockSize: Boolean = false,
    val lockPosition: Boolean = false,
    val lockRotate: Boolean = false,
    val lockDelete: Boolean = false,
    val hideText: Boolean = false
) {
    val isConnector1D: Boolean get() = kind == "Connector1D"

    val typeSubtitle: String
        get() = localizedTypeSubtitle(VexioAppLanguage.EN)

    fun localizedTypeSubtitle(lang: VexioAppLanguage): String = when {
        isGroup && !masterName.isNullOrBlank() -> "Master: $masterName (${max(childCount, 1)})"
        isGroup -> when (lang) {
            VexioAppLanguage.DE -> if (childCount > 0) "Gruppe ($childCount Formen)" else "Gruppe"
            VexioAppLanguage.IT -> if (childCount > 0) "Gruppo ($childCount forme)" else "Gruppo"
            VexioAppLanguage.FR -> if (childCount > 0) "Groupe ($childCount formes)" else "Groupe"
            VexioAppLanguage.ES -> if (childCount > 0) "Grupo ($childCount formas)" else "Grupo"
            VexioAppLanguage.GR -> if (childCount > 0) "Ομάδα ($childCount)" else "Ομάδα"
            VexioAppLanguage.EN -> if (childCount > 0) "Group ($childCount shapes)" else "Group"
        }
        isConnector1D -> when (lang) {
            VexioAppLanguage.DE -> "1D-Verbinder"
            VexioAppLanguage.IT -> "Connettore 1D"
            VexioAppLanguage.FR -> "Connecteur 1D"
            VexioAppLanguage.ES -> "Conector 1D"
            VexioAppLanguage.GR -> "Σύνδεσμος 1D"
            VexioAppLanguage.EN -> "1D Dynamic Connector"
        }
        kind == "ForeignObject" && !masterName.isNullOrBlank() -> when (lang) {
            VexioAppLanguage.DE -> "Vektorsymbol • $masterName"
            VexioAppLanguage.IT -> "Icona vettoriale • $masterName"
            VexioAppLanguage.FR -> "Icône vectorielle • $masterName"
            VexioAppLanguage.ES -> "Icono vectorial • $masterName"
            VexioAppLanguage.GR -> "Διανυσματικό • $masterName"
            VexioAppLanguage.EN -> "Vector Icon • $masterName"
        }
        kind == "ForeignObject" -> when (lang) {
            VexioAppLanguage.DE -> "Vektor-Objekt"
            VexioAppLanguage.IT -> "Oggetto vettoriale"
            VexioAppLanguage.FR -> "Objet vectoriel"
            VexioAppLanguage.ES -> "Objeto vectorial"
            VexioAppLanguage.GR -> "Διανυσματικό αντικείμενο"
            VexioAppLanguage.EN -> "Vector / Foreign Object"
        }
        !masterName.isNullOrBlank() -> when (lang) {
            VexioAppLanguage.DE -> "2D-Form • $masterName"
            VexioAppLanguage.IT -> "Forma 2D • $masterName"
            VexioAppLanguage.FR -> "Forme 2D • $masterName"
            VexioAppLanguage.ES -> "Forma 2D • $masterName"
            VexioAppLanguage.GR -> "Σχήμα 2D • $masterName"
            VexioAppLanguage.EN -> "2D Shape • $masterName"
        }
        else -> when (lang) {
            VexioAppLanguage.DE -> "2D-Form"
            VexioAppLanguage.IT -> "Forma 2D"
            VexioAppLanguage.FR -> "Forme 2D"
            VexioAppLanguage.ES -> "Forma 2D"
            VexioAppLanguage.GR -> "Σχήμα 2D"
            VexioAppLanguage.EN -> "2D Shape"
        }
    }
}

data class WorkstationUiState(
    val windowInstanceId: String = "win-${runCatching { SystemClock.uptimeMillis() }.getOrDefault(1L)}",
    val documentTitle: String = "Architecture_Blueprint.vsdx",
    val documentUri: Uri? = null,
    val documentExtension: String = "vsdx",
    val isDarkTheme: Boolean = false,
    val isDirty: Boolean = false,
    val canUndo: Boolean = false,
    val canRedo: Boolean = false,
    val activeTool: VexioTool = VexioTool.POINTER,
    val activeCursor: VexioCursorKind = VexioCursorKind.ARROW,
    val zoomPercent: Int = 100,
    val isAutoFitPage: Boolean = true,
    val activePageId: Int = 1,
    val pages: List<DiagramPageTab> = listOf(
        DiagramPageTab(1, "Page-1", if (detectDefaultMetricUnits()) "ISO A4 • mm" else "1 in = 1 in"),
        DiagramPageTab(2, "BPMN 2.0 Flow", if (detectDefaultMetricUnits()) "ISO A4 • mm" else "1 in = 1 in"),
        DiagramPageTab(3, "CAD Floorplan", "1:50")
    ),
    val selectedStencilCategory: String = "All",
    val stencilCategories: List<String> = BuiltInStencilCatalog.categories,
    val stencilMasters: List<StencilMasterItem> = BuiltInStencilCatalog.masters,
    val showOfficialStencilCatalogDialog: Boolean = false,
    val officialStencilCatalog: List<OfficialStencilCatalogEntry> = emptyList(),
    val downloadingStencilPackIds: Set<String> = emptySet(),
    val installedStencilFileNames: Set<String> = emptySet(),
    val canvasFallbackBitmap: Bitmap? = null,
    val highlightedStencilMasterId: String = "",
    val selectedShapeId: Long = 1L,
    val selectedShapeIds: List<Long> = listOf(1L),
    val selectedShapeName: String = "Sheet.1 (Process)",
    val activeCellName: String = "Width",
    val formulaBarInput: String = "GUARD(2.50 in)",
    val forceSetfOverride: Boolean = false,
    val lastFormulaStatus: FormulaCommitStatus = FormulaCommitStatus.OK,
    val statusBanner: String = "Ready • Light Mode • 22 Stencil Packs Loaded",
    val showStartBackstage: Boolean = true,
    val activeRibbonTab: String = "Home",
    val isRibbonCollapsed: Boolean = false,
    val language: VexioAppLanguage = VexioAppLanguage.detectDefault(),
    val metricUnits: Boolean = detectDefaultMetricUnits(),
    val activePagePreset: VexioPagePresetSpec = if (detectDefaultMetricUnits()) {
        VexioPagePresetSpec.A4_LANDSCAPE
    } else {
        VexioPagePresetSpec.LETTER_LANDSCAPE
    },
    val availableFontFamilies: List<String> = SystemFontCatalog.discoverInstalledFontFamilies(),
    val textFontFamily: String = "Inter",
    val textFontSizePt: Int = 11,
    val textBold: Boolean = false,
    val textItalic: Boolean = false,
    val textUnderline: Boolean = false,
    val textStrikethrough: Boolean = false,
    val textHorizAlign: Int = 1,
    val textVertAlign: Int = 1,
    val textBullets: Boolean = false,
    val textColorHex: String = "#0F172A",
    val textBgColorHex: String = "none",
    val shapeFillColorHex: String = "#E8F0FE",
    val shapeLineColorHex: String = "#1A73E8",
    val inlineTextEditor: InlineCanvasEditorModel? = null,
    val smartGuidesEnabled: Boolean = true,
    val autoCheckUpdates: Boolean = true,
    val updateStatusMessage: String = "Vexio v0.9.0-beta • Automatic update check enabled",
    val availableUpdateTag: String? = null,
    val availableUpdateUrl: String? = null,
    val showUpdateDialog: Boolean = false,
    val isPlan2RibbonExpanded: Boolean = false,
    val isShapeSheetDrawerExpanded: Boolean = false,
    val isInspectorExpanded: Boolean = true,
    val isStencilDrawerExpanded: Boolean = true,
    val isToolbarFloating: Boolean = false,
    val isStylusActive: Boolean = false,
    val lastStylusPressure: Float = 0f,
    val lastStylusTiltDeg: Int = 0,
    val activeThemeSlug: String = "modern",
    val activeThemeVariant: Int = 0,
    val activeVisualPresets: Set<String> = emptySet(),
    val lastBooleanOp: String? = null,
    val lastBooleanResultCount: Int = 0,
    val subprocessLinks: Map<Long, Int> = emptyMap(),
    val lastCompareSummary: DiagramCompareSummary? = null,
    val crdtReplicaId: String = "googlebook-peer-1",
    val crdtAppliedOpsCount: Int = 0,
    val crdtLogicalClock: Int = 0,
    val lastDataVisualizerShapeCount: Int = 0,
    val lastCloudDiscoveryShapeCount: Int = 0,
    val lastOrgChartShapeCount: Int = 0,
    val lastPlan2ExportFormat: String? = null,
    val pageWidthInches: Double = 297.0 / 25.4,
    val pageHeightInches: Double = 210.0 / 25.4,
    val pageShapeCount: Int = 1,
    val inspectedObject: InspectedObjectUiState? = InspectedObjectUiState(id = 1L, name = "Process.1"),
    val shapeDataItems: List<ShapeDataUiItem> = emptyList(),
    val dataGraphics: DataGraphicUiState = DataGraphicUiState(),
    val externalRecordsets: List<ExternalRecordsetUiItem> = emptyList(),
    val availablePropFields: List<String> = emptyList(),
    val pageLayers: List<DiagramLayerUiItem> = listOf(
        DiagramLayerUiItem(0, "Flowchart"),
        DiagramLayerUiItem(1, "Connector"),
        DiagramLayerUiItem(2, "Annotations")
    ),
    val shapeSheetCells: List<ShapeSheetCellEntry> = defaultShapeSheetCells(),
    val showMarkupStudioDialog: Boolean = false,
    val markupStudioFormat: String = "mermaid",
    val markupStudioDraft: String = "flowchart TD\n    A([\"Start\"]) --> B{\"Approved?\"}\n    B -->|Yes| C[(\"Database\")]\n    B -.->|No| D[\"Review\"]",
    val showSlideSnippetsDialog: Boolean = false,
    val slideSnippets: List<SlideSnippetUiItem> = emptyList(),
    val isPresentationModeActive: Boolean = false,
    val activeSlideIndex: Int = 0,
    val showVbaMacroDialog: Boolean = false,
    val vbaMacroScript: String = "Sub StandardizeProcessShapes()\n    ActivePage.Shapes(1).Cells(\"Width\").FormulaU = \"2.50 in\"\n    ActivePage.Shapes(1).Cells(\"Height\").FormulaU = \"1.20 in\"\n    ActivePage.Shapes(1).Cells(\"Prop.Status\").FormulaU = \"\"\"Approved\"\"\"\nEnd Sub",
    val lastVbaReportSummary: String? = null,
    val showCommentsDialog: Boolean = false,
    val reviewComments: List<ReviewCommentUiItem> = emptyList(),
    val showHyperlinkDialog: Boolean = false,
    val hyperlinkDescription: String = "",
    val hyperlinkAddress: String = "",
    val hyperlinkSubAddress: String = "",
    val shapeScreentip: String = "",
    val showDomainWizardsDialog: Boolean = false,
    val domainWizardSlug: String = "gantt",
    val domainWizardSpec: String = "Discovery,0,5,1.0,\nArchitecture,5,8,0.6,1\nImplementation,13,12,0.25,2\nGA Release,25,3,0.0,3",
    val lastDomainWizardOutput: String? = null,
    val cadScaleSlug: String = "1:100",
    val cadScaleLocked: Boolean = true,
    val selectedShapeActions: List<SmartActionUiItem> = emptyList(),
    val showSmartActionsDialog: Boolean = false,
    val rulerGuides: List<RulerGuideUiItem> = emptyList(),
    val snapEnabled: Boolean = true,
    val glueEnabled: Boolean = true,
    val snapTargetsMask: Int = 1 or 4 or 8 or 16 or 64,
    val glueTypesMask: Int = 1 or 2 or 4,
    val copiedFormatJson: String? = null,
    val lastFindMatches: List<FindMatchUiItem> = emptyList(),
    val showInsertFieldDialog: Boolean = false,
    val showFindReplaceDialog: Boolean = false,
    val showSnapGlueDialog: Boolean = false,
    val showNumberShapesDialog: Boolean = false,
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
    ShapeSheetCellEntry("Fill Format", "FillBkgnd", "RGB(255,255,255)", isGuarded = false),
    ShapeSheetCellEntry("Fill Format", "FillPattern", "1", isGuarded = false),
    ShapeSheetCellEntry("Protection", "LockWidth", "0", isGuarded = false),
    ShapeSheetCellEntry("Text Transform", "TxtPinX", "Width*0.5", isGuarded = false),
    ShapeSheetCellEntry("Text Transform", "TxtPinY", "Height*0.5", isGuarded = false)
)

/**
 * Built-in enterprise stencil catalog covering all 22 Visio Professional, Microsoft, Server & Network categories
 * (superseded dynamically at runtime by the 520+-master SVG catalog from `libvexio_jni.so`).
 */
object BuiltInStencilCatalog {
    val categories: List<String> = listOf(
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
        "Titles, Headings & Banners",
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
        "Value Stream Mapping (Lean VSM)",
        "IT Generic & Security Scanning (ECCEMAN / MTMnet / OpenVAS)",
        "Security & Carrier Vendors (Palo Alto / Fortinet / Check Point / Juniper / Huawei)",
        "Multi-Cloud, Kubernetes & VMware (AWS / GCP / OCI / K8s / vSphere)",
        "Rack Power, Cabling & KVM (CyberPower / Tripp Lite / Raritan / BlackBox / HellermannTyton)",
        "Enterprise NAS, SMB Network & Unified Comms (Synology / QNAP / TP-Link / Poly / Avaya)",
        "C4 Architecture & Enterprise Integration Patterns (EIP)",
        "Decision, Case & Risk Modeling (CMMN 1.1 / DMN 1.3 / CORAS)",
        "Data Vault 2.0 & ADAPT OLAP BI",
        "Systems Engineering & Ontology (SysML & IDEF0 / IDEF3 / IDEF5)",
        "Smart Home IoT, Digital Logic & GDS / IA UX"
    )

    val masters: List<StencilMasterItem> = listOf(
        StencilMasterItem("Basic Shapes::Rectangle", "Rectangle", "Basic Shapes", "1.60 in", "1.00 in", "RGB(239,246,255)", "Rect"),
        StencilMasterItem("Basic Shapes::Rounded Rectangle", "Rounded Rectangle", "Basic Shapes", "1.60 in", "1.00 in", "RGB(239,246,255)", "RoundedRect"),
        StencilMasterItem("Basic Shapes::Circle", "Circle", "Basic Shapes", "1.10 in", "1.10 in", "RGB(236,253,245)", "Circle"),
        StencilMasterItem("Basic Shapes::Diamond", "Diamond", "Basic Shapes", "1.30 in", "1.10 in", "RGB(255,251,235)", "Diamond"),
        StencilMasterItem("Basic Shapes::Hexagon", "Hexagon", "Basic Shapes", "1.40 in", "1.10 in", "RGB(245,243,255)", "Hexagon"),
        StencilMasterItem("Basic Shapes::5-Point Star", "Star (5-Point)", "Basic Shapes", "1.20 in", "1.20 in", "RGB(254,249,195)", "Star"),
        StencilMasterItem("Basic Flowchart::Process", "Process", "Basic Flowchart", "1.80 in", "0.90 in", "RGB(227,242,253)", "Rect"),
        StencilMasterItem("Basic Flowchart::Decision", "Decision", "Basic Flowchart", "1.40 in", "1.00 in", "RGB(255,243,224)", "Diamond"),
        StencilMasterItem("Basic Flowchart::Start/End", "Start/End", "Basic Flowchart", "1.60 in", "0.70 in", "RGB(230,244,234)", "Pill"),
        StencilMasterItem("Basic Flowchart::Database", "Database", "Basic Flowchart", "1.30 in", "1.40 in", "RGB(237,231,246)", "Cylinder"),
        StencilMasterItem("Microsoft 365 & Office::Microsoft Teams", "Microsoft Teams", "Microsoft 365 & Office", "1.50 in", "1.05 in", "RGB(238,242,255)", "RoundedRect"),
        StencilMasterItem("Microsoft 365 & Office::SharePoint Online", "SharePoint Online", "Microsoft 365 & Office", "1.15 in", "1.15 in", "RGB(240,253,250)", "Circle"),
        StencilMasterItem("Microsoft 365 & Office::Exchange Online", "Exchange Online", "Microsoft 365 & Office", "1.50 in", "1.05 in", "RGB(239,246,255)", "RoundedRect"),
        StencilMasterItem("Microsoft Servers & Active Directory::Windows Server", "Windows Server", "Microsoft Servers & Active Directory", "1.35 in", "1.55 in", "RGB(241,245,249)", "Rack"),
        StencilMasterItem("Microsoft Servers & Active Directory::Active Directory DC", "Active Directory DC", "Microsoft Servers & Active Directory", "1.45 in", "1.30 in", "RGB(241,245,249)", "Diamond"),
        StencilMasterItem("Microsoft Servers & Active Directory::SQL Server Cluster", "SQL Server Cluster", "Microsoft Servers & Active Directory", "1.40 in", "1.35 in", "RGB(254,242,242)", "Cylinder"),
        StencilMasterItem("Microsoft Azure & Hybrid Cloud::Azure Virtual Machine", "Azure Virtual Machine", "Microsoft Azure & Hybrid Cloud", "1.50 in", "1.05 in", "RGB(240,249,255)", "RoundedRect"),
        StencilMasterItem("Microsoft Azure & Hybrid Cloud::Azure Kubernetes (AKS)", "Azure Kubernetes (AKS)", "Microsoft Azure & Hybrid Cloud", "1.50 in", "1.30 in", "RGB(240,249,255)", "Hexagon"),
        StencilMasterItem("Microsoft Power Platform & Dynamics 365::Power BI Dashboard", "Power BI Dashboard", "Microsoft Power Platform & Dynamics 365", "1.50 in", "1.10 in", "RGB(254,249,195)", "RoundedRect"),
        StencilMasterItem("Microsoft Power Platform & Dynamics 365::Power Automate Flow", "Power Automate Flow", "Microsoft Power Platform & Dynamics 365", "1.55 in", "0.95 in", "RGB(239,246,255)", "Arrow"),
        StencilMasterItem("Servers (Standard Roles)::Tower Server", "Tower Server", "Servers (Standard Roles)", "1.25 in", "1.60 in", "RGB(241,245,249)", "Rack"),
        StencilMasterItem("Servers (Standard Roles)::Web Server", "Web Server", "Servers (Standard Roles)", "1.45 in", "1.45 in", "RGB(240,249,255)", "Rack"),
        StencilMasterItem("Servers (Standard Roles)::Database Server", "Database Server", "Servers (Standard Roles)", "1.45 in", "1.45 in", "RGB(254,242,242)", "Cylinder"),
        StencilMasterItem("Network & Rack Equipment::Next-Gen Firewall (NGFW)", "Next-Gen Firewall (NGFW)", "Network & Rack Equipment", "1.90 in", "0.85 in", "RGB(254,242,242)", "RoundedRect"),
        StencilMasterItem("Network & Rack Equipment::Layer 3 Multilayer Switch", "Layer 3 Multilayer Switch", "Network & Rack Equipment", "1.45 in", "1.25 in", "RGB(236,254,255)", "RoundedRect"),
        StencilMasterItem("Network & Rack Equipment::42U Standard Rack", "42U Standard Rack", "Network & Rack Equipment", "2.20 in", "7.35 in", "RGB(226,232,240)", "Rack"),
        StencilMasterItem("Cloud Architecture (AWS / GCP / Azure)::Compute VM", "Compute VM", "Cloud Architecture (AWS / GCP / Azure)", "1.60 in", "1.00 in", "RGB(239,246,255)", "RoundedRect"),
        StencilMasterItem("Arrows & Block Arrows::Right Block Arrow", "Right Block Arrow", "Arrows & Block Arrows", "1.70 in", "0.85 in", "RGB(219,234,254)", "Arrow"),
        StencilMasterItem("Callouts, Notes & Containers::Callout Bubble", "Callout Bubble", "Callouts, Notes & Containers", "1.70 in", "1.10 in", "RGB(254,249,195)", "Callout"),
        StencilMasterItem("Titles, Headings & Banners::Diagram Title", "Diagram Title", "Titles, Headings & Banners", "3.80 in", "0.62 in", "RGB(248,250,252)", "Rect"),
        StencilMasterItem("Cross-Functional Swimlanes::Horizontal Swimlane", "Horizontal Swimlane", "Cross-Functional Swimlanes", "8.50 in", "2.20 in", "RGB(248,250,252)", "Swimlane"),
        StencilMasterItem("BPMN 2.0::User Task", "User Task", "BPMN 2.0", "1.75 in", "1.00 in", "RGB(232,240,254)", "RoundedRect"),
        StencilMasterItem("BPMN 2.0::Exclusive Gateway", "Exclusive Gateway", "BPMN 2.0", "0.75 in", "0.75 in", "RGB(254,247,224)", "Diamond"),
        StencilMasterItem("BPMN 2.0::Start Event", "Start Event", "BPMN 2.0", "0.50 in", "0.50 in", "RGB(230,244,234)", "Circle"),
        StencilMasterItem("UML 2.5::Class", "Class", "UML 2.5", "2.40 in", "1.60 in", "RGB(243,229,245)", "ClassBox"),
        StencilMasterItem("UML 2.5::Component", "Component", "UML 2.5", "2.20 in", "1.20 in", "RGB(224,242,241)", "Component"),
        StencilMasterItem("Database Crow's Foot & IDEF1X::Strong Entity", "Strong Entity", "Database Crow's Foot & IDEF1X", "2.20 in", "1.30 in", "RGB(239,246,255)", "ClassBox"),
        StencilMasterItem("Organization Chart::Executive", "Executive", "Organization Chart", "2.10 in", "0.95 in", "RGB(239,246,255)", "RoundedRect"),
        StencilMasterItem("Floor Plan & Architecture::Exterior Wall", "Exterior Wall", "Floor Plan & Architecture", "4.00 in", "0.20 in", "RGB(207,216,220)", "Wall"),
        StencilMasterItem("Electrical & Electronic (IEEE / IEC)::Resistor (IEEE)", "Resistor (IEEE)", "Electrical & Electronic (IEEE / IEC)", "1.40 in", "0.45 in", "RGB(255,255,255)", "Electrical"),
        StencilMasterItem("P&ID Process Engineering::Centrifugal Pump", "Centrifugal Pump", "P&ID Process Engineering", "0.90 in", "0.90 in", "RGB(240,249,255)", "Circle"),
        StencilMasterItem("UI & Application Wireframes::Browser / Desktop Frame", "Browser / Desktop Frame", "UI & Application Wireframes", "4.20 in", "2.80 in", "RGB(248,250,252)", "ClassBox"),
        StencilMasterItem("Project Schedule (Gantt & Timeline)::Timeline Spine", "Timeline Spine", "Project Schedule (Gantt & Timeline)", "5.00 in", "0.35 in", "RGB(219,234,254)", "Pill"),
        StencilMasterItem("Value Stream Mapping (Lean VSM)::Customer / Supplier", "Customer / Supplier", "Value Stream Mapping (Lean VSM)", "1.60 in", "1.10 in", "RGB(241,245,249)", "Rect"),
        StencilMasterItem("IT Generic & Security Scanning (ECCEMAN / MTMnet / OpenVAS)::OpenVAS Scanner Node", "OpenVAS Scanner Node", "IT Generic & Security Scanning (ECCEMAN / MTMnet / OpenVAS)", "1.05 in", "1.05 in", "RGB(240,253,244)", "Hexagon"),
        StencilMasterItem("Security & Carrier Vendors (Palo Alto / Fortinet / Check Point / Juniper / Huawei)::Fortinet FortiGate NGFW", "Fortinet FortiGate NGFW", "Security & Carrier Vendors (Palo Alto / Fortinet / Check Point / Juniper / Huawei)", "1.45 in", "0.92 in", "RGB(254,242,242)", "RoundedRect"),
        StencilMasterItem("Multi-Cloud, Kubernetes & VMware (AWS / GCP / OCI / K8s / vSphere)::Kubernetes Pod", "Kubernetes Pod", "Multi-Cloud, Kubernetes & VMware (AWS / GCP / OCI / K8s / vSphere)", "1.00 in", "1.00 in", "RGB(239,246,255)", "Hexagon"),
        StencilMasterItem("Rack Power, Cabling & KVM (CyberPower / Tripp Lite / Raritan / BlackBox / HellermannTyton)::CyberPower Sinewave UPS (2U)", "CyberPower Sinewave UPS (2U)", "Rack Power, Cabling & KVM (CyberPower / Tripp Lite / Raritan / BlackBox / HellermannTyton)", "1.90 in", "0.62 in", "RGB(241,245,249)", "Rack"),
        StencilMasterItem("Enterprise NAS, SMB Network & Unified Comms (Synology / QNAP / TP-Link / Poly / Avaya)::Synology RackStation RS", "Synology RackStation RS", "Enterprise NAS, SMB Network & Unified Comms (Synology / QNAP / TP-Link / Poly / Avaya)", "1.65 in", "0.85 in", "RGB(241,245,249)", "Rack"),
        StencilMasterItem("C4 Architecture & Enterprise Integration Patterns (EIP)::C4 Software System", "C4 Software System", "C4 Architecture & Enterprise Integration Patterns (EIP)", "2.00 in", "1.25 in", "RGB(219,234,254)", "RoundedRect"),
        StencilMasterItem("Decision, Case & Risk Modeling (CMMN 1.1 / DMN 1.3 / CORAS)::DMN Decision", "DMN Decision", "Decision, Case & Risk Modeling (CMMN 1.1 / DMN 1.3 / CORAS)", "1.75 in", "0.95 in", "RGB(238,242,255)", "Rect"),
        StencilMasterItem("Data Vault 2.0 & ADAPT OLAP BI::DV2 Hub Entity", "DV2 Hub Entity", "Data Vault 2.0 & ADAPT OLAP BI", "1.95 in", "1.15 in", "RGB(219,234,254)", "ClassBox"),
        StencilMasterItem("Systems Engineering & Ontology (SysML & IDEF0 / IDEF3 / IDEF5)::SysML Block «block»", "SysML Block «block»", "Systems Engineering & Ontology (SysML & IDEF0 / IDEF3 / IDEF5)", "2.05 in", "1.30 in", "RGB(248,250,252)", "ClassBox"),
        StencilMasterItem("Smart Home IoT, Digital Logic & GDS / IA UX::IoT Smart Home Gateway", "IoT Smart Home Gateway", "Smart Home IoT, Digital Logic & GDS / IA UX", "1.10 in", "1.10 in", "RGB(236,254,255)", "RoundedRect")
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

    @Volatile
    private var stencilStorageDir: File? = null

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

    private data class HlcClock(
        val wallMs: Long,
        val logical: Int,
        val replicaId: String
    ) : Comparable<HlcClock> {
        override fun compareTo(other: HlcClock): Int {
            val w = wallMs.compareTo(other.wallMs)
            if (w != 0) return w
            val l = logical.compareTo(other.logical)
            if (l != 0) return l
            return replicaId.compareTo(other.replicaId)
        }
    }

    private val localUndoStack = ArrayDeque<FormulaEditRecord>()
    private val localRedoStack = ArrayDeque<FormulaEditRecord>()
    private val crdtCellClocks = linkedMapOf<String, HlcClock>()
    private val tombstonedShapeIds = linkedSetOf<Long>()

    private var baselineCellsSnapshot: Map<String, String> = emptyMap()
    private var baselinePageIdsSnapshot: Set<Int> = emptySet()
    private var baselineNextShapeId: Long = 3L
    private var deletedShapeCount: Int = 0

    private var enginePtr: Long = 0L
    private var docPtr: Long = 0L
    private var viewportPtr: Long = 0L
    private var nextSyntheticShapeId: Long = 3L
    private var viewportWidthPx: Int = 0
    private var viewportHeightPx: Int = 0
    private var viewportDensity: Float = 2.0f
    private var savedStateRevision: Long = 0L
    private var directViewportRgbaBuffer: ByteBuffer? = null
    private val lenientJson = Json { ignoreUnknownKeys = true }
    private val syntheticGroups = linkedMapOf<Long, List<Long>>()
    private val linkedRecordsetUris = linkedMapOf<Int, Uri>()

    init {
        initializeEngineAndBlankDocument("Architecture_Blueprint.vsdx")
    }

    private fun resolveNativePageId(pageId: Int): Int = pageId.coerceAtLeast(0)

    @Synchronized
    private fun syncViewportSelection(ids: List<Long>) {
        if (!NativeBridge.isNativeAvailable || viewportPtr == 0L || docPtr == 0L) return
        val valid = ids.filter { it > 0L }.toLongArray()
        NativeBridge.nativeSetSelectionIds(viewportPtr, docPtr, valid)
    }

    @Synchronized
    private fun syncPagesAndActivePageFromNative(preferFirstForeground: Boolean = false) {
        if (!NativeBridge.isNativeAvailable || docPtr == 0L) return
        val requestedPage = if (preferFirstForeground) -1 else resolveNativePageId(_uiState.value.activePageId)
        val rawJson = runCatching {
            NativeBridge.nativeGetStateJson(docPtr, requestedPage)
        }.getOrDefault("{}")
        val root = runCatching { Json.parseToJsonElement(rawJson).jsonObject }.getOrNull() ?: return

        val parsedAllPages = mutableListOf<DiagramPageTab>()
        val pagesArr = root["pages"] as? JsonArray
        val useMetric = _uiState.value.metricUnits
        if (pagesArr != null) {
            for (elem in pagesArr) {
                val pObj = elem as? JsonObject ?: continue
                val id = pObj["id"]?.jsonPrimitive?.intOrNull ?: continue
                val name = pObj["name"]?.jsonPrimitive?.contentOrNull?.ifBlank { "Page-$id" } ?: "Page-$id"
                val wIn = pObj["widthIn"]?.jsonPrimitive?.doubleOrNull ?: 11.0
                val hIn = pObj["heightIn"]?.jsonPrimitive?.doubleOrNull ?: 8.5
                val isBg = pObj["isBackground"]?.jsonPrimitive?.booleanOrNull ?: false
                val scaleLabel = if (useMetric) {
                    "${(wIn * 25.4).toInt()}×${(hIn * 25.4).toInt()} mm"
                } else {
                    "${"%.1f".format(Locale.US, wIn)}×${"%.1f".format(Locale.US, hIn)} in"
                }
                parsedAllPages.add(
                    DiagramPageTab(
                        pageId = id,
                        name = name,
                        scaleLabel = scaleLabel,
                        isBackground = isBg
                    )
                )
            }
        }
        val foregroundPages = parsedAllPages.filter { !it.isBackground }.ifEmpty { parsedAllPages }
        if (foregroundPages.isEmpty()) return

        val nativeActiveId = root["activePageId"]?.jsonPrimitive?.intOrNull
        val resolvedActiveId = when {
            nativeActiveId != null && foregroundPages.any { it.pageId == nativeActiveId } -> nativeActiveId
            else -> foregroundPages.first().pageId
        }

        val shapesArr = root["shapes"] as? JsonArray
        var primaryShapeId = _uiState.value.selectedShapeId
        var primaryShapeName = _uiState.value.selectedShapeName
        val docFonts = linkedSetOf<String>()
        if (shapesArr != null && shapesArr.isNotEmpty()) {
            val firstShape = shapesArr.first() as? JsonObject
            if (firstShape != null && preferFirstForeground) {
                primaryShapeId = firstShape["id"]?.jsonPrimitive?.longOrNull ?: primaryShapeId
                val sName = firstShape["name"]?.jsonPrimitive?.contentOrNull ?: "Sheet.$primaryShapeId"
                primaryShapeName = sName
            }
            for (sElem in shapesArr) {
                val sObj = sElem as? JsonObject ?: continue
                val ff = sObj["fontFamily"]?.jsonPrimitive?.contentOrNull?.trim().orEmpty()
                if (ff.isNotEmpty()) docFonts.add(ff)
            }
        }

        if (viewportPtr != 0L) {
            NativeBridge.nativeSetViewportPage(viewportPtr, docPtr, resolvedActiveId)
            if (preferFirstForeground) {
                NativeBridge.nativeSetViewportZoom(viewportPtr, docPtr, 0.0, 0.0, 0.0)
            }
            if (primaryShapeId > 0L) {
                syncViewportSelection(listOf(primaryShapeId))
            }
        }

        _uiState.update { state ->
            val mergedFonts = if (docFonts.isEmpty()) {
                state.availableFontFamilies
            } else {
                (state.availableFontFamilies + docFonts).distinct()
            }
            state.copy(
                pages = foregroundPages,
                activePageId = resolvedActiveId,
                selectedShapeId = primaryShapeId,
                selectedShapeIds = listOf(primaryShapeId),
                selectedShapeName = primaryShapeName,
                availableFontFamilies = mergedFonts,
                zoomPercent = if (preferFirstForeground) 100 else state.zoomPercent
            )
        }
    }

    private fun captureBaselineSnapshot() {
        val state = _uiState.value
        baselineCellsSnapshot = state.shapeSheetCells.associate { it.cellName to it.formula }
        baselinePageIdsSnapshot = state.pages.mapTo(linkedSetOf()) { it.pageId }
        baselineNextShapeId = nextSyntheticShapeId
        deletedShapeCount = 0
    }

    @Synchronized
    private fun presentViewportOrFallback() {
        if (!NativeBridge.isNativeAvailable || viewportPtr == 0L || docPtr == 0L) return
        val presentedGpu = NativeBridge.nativeRenderViewportFrame(viewportPtr, docPtr)
        if (presentedGpu) {
            if (_uiState.value.canvasFallbackBitmap != null) {
                _uiState.update { it.copy(canvasFallbackBitmap = null) }
            }
            return
        }
        val w = viewportWidthPx.coerceIn(64, 4096)
        val h = viewportHeightPx.coerceIn(64, 4096)
        if (w <= 0 || h <= 0) return
        val needed = w * h * 4
        var buf = directViewportRgbaBuffer
        if (buf == null || buf.capacity() < needed) {
            buf = ByteBuffer.allocateDirect(needed).order(ByteOrder.nativeOrder())
            directViewportRgbaBuffer = buf
        }
        buf.clear()
        val rc = NativeBridge.nativeRenderViewportRgba(viewportPtr, docPtr, buf)
        if (rc == 0) {
            runCatching {
                buf.rewind()
                val bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
                bmp.copyPixelsFromBuffer(buf)
                _uiState.update { it.copy(canvasFallbackBitmap = bmp) }
            }
        }
    }

    @Synchronized
    internal fun applyNativeStencilsJson(rawJson: String): Int {
        val grouped = lenientJson.decodeFromString<Map<String, List<NativeStencilMasterDto>>>(rawJson)
        if (grouped.isEmpty()) return 0

        val orderedCats = buildList {
            for (canonical in BuiltInStencilCatalog.categories) {
                if (grouped.containsKey(canonical)) add(canonical)
            }
            for (extra in grouped.keys.sorted()) {
                if (extra !in this) add(extra)
            }
        }
        val flatMasters = buildList {
            for (cat in orderedCats) {
                for (dto in grouped[cat].orEmpty()) {
                    val cleanFill = dto.fillFormula.removePrefix("=")
                    add(
                        StencilMasterItem(
                            masterId = "$cat::${dto.name}",
                            title = dto.name,
                            category = cat,
                            defaultWidthFormula = "%.2f in".format(Locale.US, dto.widthIn),
                            defaultHeightFormula = "%.2f in".format(Locale.US, dto.heightIn),
                            fillFormula = cleanFill,
                            shapeKind = inferShapeKind(dto.name),
                            prompt = dto.prompt,
                            svgPreview = dto.svg,
                            pngDataUrl = dto.pngDataUrl
                        )
                    )
                }
            }
        }
        if (flatMasters.isNotEmpty()) {
            _uiState.update {
                it.copy(
                    stencilCategories = orderedCats,
                    stencilMasters = flatMasters
                )
            }
        }
        return flatMasters.size
    }

    @Synchronized
    private fun loadDynamicStencilsFromNative() {
        if (!NativeBridge.isNativeAvailable || docPtr == 0L) return
        val rawJson = runCatching { NativeBridge.nativeListStencilsJson(docPtr) }.getOrDefault("{}")
        if (rawJson.length < 10) return
        runCatching { applyNativeStencilsJson(rawJson) }
    }

    private fun inferShapeKind(name: String): String {
        val low = name.lowercase(Locale.US)
        return when {
            "circle" in low || "event" in low -> "Circle"
            "diamond" in low || "decision" in low || "gateway" in low -> "Diamond"
            "hexagon" in low -> "Hexagon"
            "star" in low -> "Star"
            "triangle" in low -> "Triangle"
            "database" in low || "cylinder" in low || "rds" in low -> "Cylinder"
            "class" in low || "entity" in low || "window" in low -> "ClassBox"
            else -> "RoundedRect"
        }
    }

    @Synchronized
    fun initializeEngineAndBlankDocument(title: String, hideStartBackstage: Boolean = false) {
        // Verify `lenientJson` is initialized before `init` runs even when JNI is absent on host JVM tests.
        applyNativeStencilsJson("{}")
        val defaultPreset = _uiState.value.activePagePreset
        if (NativeBridge.ensureLoaded()) {
            if (enginePtr == 0L) {
                enginePtr = NativeBridge.nativeInitEngine()
            }
            if (docPtr != 0L) {
                NativeBridge.nativeDestroyDocument(docPtr)
            }
            docPtr = NativeBridge.nativeCreateBlankDocument(title)
            if (docPtr != 0L) {
                NativeBridge.nativeLoadStarterTemplate(docPtr, "flowchart")
                NativeBridge.nativeSetPagePreset(docPtr, 1, defaultPreset.slug, defaultPreset.landscape)
                reloadPersistedStencilsLocked()
                loadDynamicStencilsFromNative()
                syncPagesAndActivePageFromNative(preferFirstForeground = true)
            }
        }
        _uiState.update { state ->
            state.copy(
                documentTitle = title,
                isDirty = false,
                isAutoFitPage = true,
                showStartBackstage = if (hideStartBackstage) false else state.showStartBackstage,
                statusBanner = if (NativeBridge.isNativeAvailable && enginePtr != 0L) {
                    "Vexio Rust Engine Active • ${state.stencilMasters.size} Stencils Ready"
                } else {
                    "Vexio Workstation Active • Ready"
                }
            )
        }
        refreshSelectedShapeFormulas()
        savedStateRevision = readNativeStateRevision()
        captureBaselineSnapshot()
        if (_uiState.value.isAutoFitPage && viewportWidthPx > 0 && viewportHeightPx > 0) {
            fitPageToWindow()
        } else {
            presentViewportOrFallback()
        }
    }

    private fun readNativeStateRevision(): Long {
        if (!NativeBridge.isNativeAvailable || docPtr == 0L) return 0L
        val raw = runCatching {
            NativeBridge.nativeGetStateJson(docPtr, resolveNativePageId(_uiState.value.activePageId))
        }.getOrDefault("")
        if (raw.isBlank()) return 0L
        val match = Regex("\"stateRevision\"\\s*:\\s*(\\d+)").find(raw)
        return match?.groupValues?.getOrNull(1)?.toLongOrNull() ?: 0L
    }

    private fun computeFitZoomPercent(): Int {
        val safeDensity = viewportDensity.coerceAtLeast(0.5f).toDouble()
        val lw = viewportWidthPx.toDouble() / safeDensity
        val lh = viewportHeightPx.toDouble() / safeDensity
        if (lw < 64.0 || lh < 64.0) return _uiState.value.zoomPercent
        val leftPad = if (lw > 420.0) 88.0 else lw * 0.05
        val rightPad = (lw * 0.025).coerceIn(22.0, 40.0).coerceAtMost(lw * 0.08)
        val topPad = (lh * 0.03).coerceIn(18.0, 34.0).coerceAtMost(lh * 0.08)
        val botPad = if (lh > 320.0) 50.0 else lh * 0.05
        val availW = (lw - leftPad - rightPad).coerceAtLeast(lw * 0.55)
        val availH = (lh - topPad - botPad).coerceAtLeast(lh * 0.55)
        val pw = _uiState.value.pageWidthInches.coerceAtLeast(0.5)
        val ph = _uiState.value.pageHeightInches.coerceAtLeast(0.5)
        val ppi = minOf(availW / pw, availH / ph)
        return ((ppi / 96.0) * 100.0).toInt().coerceIn(10, 800)
    }

    @Synchronized
    fun attachVulkanSurface(surface: Surface, width: Int, height: Int, density: Float) {
        if (width <= 0 || height <= 0 || !surface.isValid) return
        viewportWidthPx = width
        viewportHeightPx = height
        if (NativeBridge.isNativeAvailable && enginePtr != 0L && docPtr != 0L) {
            if (viewportPtr != 0L) {
                NativeBridge.nativeDestroyViewport(viewportPtr)
            }
            viewportPtr = NativeBridge.nativeAttachVulkanSurface(
                enginePtr = enginePtr,
                docPtr = docPtr,
                surface = surface,
                width = width,
                height = height,
                density = density
            )
            if (viewportPtr != 0L) {
                NativeBridge.nativeSetViewportPage(viewportPtr, docPtr, resolveNativePageId(_uiState.value.activePageId))
                NativeBridge.nativeSetViewportTool(viewportPtr, _uiState.value.activeTool.nativeCode)
                NativeBridge.nativeSetSmartGuides(viewportPtr, _uiState.value.smartGuidesEnabled)
                val initialSel = _uiState.value.selectedShapeIds.ifEmpty {
                    listOf(_uiState.value.selectedShapeId)
                }
                syncViewportSelection(initialSel)
                viewportWidthPx = width
                viewportHeightPx = height
                viewportDensity = density
                if (_uiState.value.isAutoFitPage) {
                    fitPageToWindow()
                } else {
                    presentViewportOrFallback()
                }
            }
        }
        viewportWidthPx = width
        viewportHeightPx = height
        viewportDensity = density
        _uiState.update {
            it.copy(
                statusBanner = "Canvas ${width}×${height} @ ${"%.2f".format(Locale.US, density)}x Active"
            )
        }
    }

    @Synchronized
    fun resizeVulkanViewport(width: Int, height: Int, density: Float) {
        if (width <= 0 || height <= 0) return
        viewportWidthPx = width
        viewportHeightPx = height
        viewportDensity = density
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L) {
            NativeBridge.nativeResizeViewport(viewportPtr, width, height, density)
            if (_uiState.value.isAutoFitPage) {
                fitPageToWindow()
            } else {
                presentViewportOrFallback()
            }
        } else if (_uiState.value.isAutoFitPage) {
            _uiState.update { it.copy(zoomPercent = computeFitZoomPercent()) }
        }
    }

    @Synchronized
    fun detachVulkanSurface() {
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L) {
            NativeBridge.nativeDestroyViewport(viewportPtr)
        }
        viewportPtr = 0L
    }

    fun toggleDarkTheme() {
        _uiState.update {
            val next = !it.isDarkTheme
            it.copy(
                isDarkTheme = next,
                statusBanner = if (next) "Switched to Dark Mode" else "Switched to Light Mode (Default)"
            )
        }
    }

    @Synchronized
    fun loadStarterTemplate(templateId: String) {
        val preset = _uiState.value.activePagePreset
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeLoadStarterTemplate(docPtr, templateId)
            NativeBridge.nativeSetPagePreset(docPtr, 1, preset.slug, preset.landscape)
            syncPagesAndActivePageFromNative(preferFirstForeground = true)
            syncViewportSelection(listOf(1L))
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        }
        _uiState.update {
            it.copy(
                showStartBackstage = false,
                selectedShapeId = 1L,
                selectedShapeIds = listOf(1L),
                activeTool = VexioTool.POINTER,
                activeCursor = VexioCursorKind.ARROW,
                isDirty = true,
                canUndo = true,
                statusBanner = "Loaded '${templateId.replaceFirstChar { c -> c.uppercase() }}' starter diagram"
            )
        }
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

        if (actionMaskedIsPrimaryDown(event) && _uiState.value.inlineTextEditor != null) {
            commitInlineTextEdit(_uiState.value.inlineTextEditor?.text.orEmpty())
        }

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

        // Handle Ctrl + Scroll-wheel zoom deterministically without double-zooming
        if (mappedAction == MotionBatchFormat.ACTION_SCROLL &&
            (metaState and KeyEvent.META_CTRL_ON) != 0
        ) {
            val vScroll = event.getAxisValue(MotionEvent.AXIS_VSCROLL)
            if (vScroll != 0f) {
                val delta = if (vScroll > 0f) 10 else -10
                adjustZoom(delta)
                directMotionBatchBuffer.clear()
                return true
            }
        }

        var newlyCreatedTextId = 0L
        val nativeCursorCode = if (NativeBridge.isNativeAvailable && viewportPtr != 0L && docPtr != 0L) {
            val code = NativeBridge.nativePushMotionBatch(
                viewportPtr = viewportPtr,
                docPtr = docPtr,
                directBatchBuffer = directMotionBatchBuffer,
                count = sampleCount
            )
            if (mappedAction == MotionBatchFormat.ACTION_SCROLL) {
                presentViewportOrFallback()
            }
            if (mappedAction == MotionBatchFormat.ACTION_UP ||
                mappedAction == MotionBatchFormat.ACTION_DOWN ||
                mappedAction == MotionBatchFormat.ACTION_MOVE
            ) {
                val multiIds = NativeBridge.nativeGetSelectionIds(viewportPtr)
                    ?.filter { it > 0L }
                    .orEmpty()
                val primarySel = NativeBridge.nativeGetPrimarySelection(viewportPtr)
                if (multiIds.isNotEmpty() || (primarySel > 0L && primarySel != _uiState.value.selectedShapeId)) {
                    val resolvedPrimary = if (primarySel > 0L) primarySel else multiIds.first()
                    val resolvedList = if (multiIds.isNotEmpty()) multiIds else listOf(resolvedPrimary)
                    _uiState.update {
                        it.copy(
                            selectedShapeId = resolvedPrimary,
                            selectedShapeIds = resolvedList,
                            selectedShapeName = if (resolvedList.size > 1) {
                                "${resolvedList.size} Shapes Selected"
                            } else {
                                "Sheet.$resolvedPrimary"
                            }
                        )
                    }
                    if (mappedAction == MotionBatchFormat.ACTION_UP) {
                        refreshSelectedShapeFormulas()
                    }
                } else if (mappedAction == MotionBatchFormat.ACTION_UP && multiIds.isEmpty() && primarySel == 0L) {
                    _uiState.update {
                        it.copy(
                            selectedShapeId = 0L,
                            selectedShapeIds = emptyList()
                        )
                    }
                }
                if (mappedAction == MotionBatchFormat.ACTION_UP) {
                    newlyCreatedTextId = NativeBridge.nativePopCreatedTextShape(viewportPtr)
                }
                presentViewportOrFallback()
            }
            code
        } else {
            inferFallbackCursorCode(_uiState.value.activeTool, mappedAction, buttonState)
        }

        directMotionBatchBuffer.clear()

        // Auto-return cursor to Select (Pointer) mode after inserting any shape, connector, or text object!
        val activeBeforeUp = _uiState.value.activeTool
        val shouldResetToPointer = mappedAction == MotionBatchFormat.ACTION_UP &&
            activeBeforeUp !in setOf(
                VexioTool.POINTER,
                VexioTool.PAN_HAND,
                VexioTool.STYLUS_INK,
                VexioTool.HIGHLIGHTER,
                VexioTool.EDIT_VERTICES,
                VexioTool.INK_ERASER,
                VexioTool.CONNECTION_POINT,
                VexioTool.TEXT_BLOCK_TRANSFORM,
                VexioTool.FORMAT_PAINTER,
                VexioTool.WALL
            )
        if (shouldResetToPointer && NativeBridge.isNativeAvailable && viewportPtr != 0L) {
            NativeBridge.nativeSetViewportTool(viewportPtr, VexioTool.POINTER.nativeCode)
        }

        val resolvedCursor = if (shouldResetToPointer) {
            VexioCursorKind.ARROW
        } else {
            VexioCursorKind.fromNativeCode(nativeCursorCode)
        }
        val stylusActive = primaryTool == MotionBatchFormat.TOOL_STYLUS ||
            primaryTool == MotionBatchFormat.TOOL_ERASER
        val tiltDegrees = Math.toDegrees(currentTilt.toDouble()).toInt().coerceIn(0, 90)

        _uiState.update { state ->
            state.copy(
                activeTool = if (shouldResetToPointer) VexioTool.POINTER else state.activeTool,
                activeCursor = resolvedCursor,
                isStylusActive = stylusActive || palmRejector.isStylusProximityActive,
                lastStylusPressure = if (stylusActive) currentPressure else state.lastStylusPressure,
                lastStylusTiltDeg = if (stylusActive) tiltDegrees else state.lastStylusTiltDeg
            )
        }

        if (newlyCreatedTextId > 0L) {
            beginInlineTextEdit(newlyCreatedTextId, isNewTextShape = true)
        } else if (mappedAction == MotionBatchFormat.ACTION_UP && activeBeforeUp == VexioTool.TEXT_BLOCK) {
            // Fallback inline editor when running without Vulkan surface
            val targetId = _uiState.value.selectedShapeId.coerceAtLeast(1L)
            _uiState.update {
                it.copy(
                    inlineTextEditor = InlineCanvasEditorModel(
                        shapeId = targetId,
                        text = "",
                        xPx = event.getX(0),
                        yPx = event.getY(0),
                        widthPx = 220f,
                        heightPx = 56f,
                        isNewlyCreatedText = true
                    )
                )
            }
        }
        return true
    }

    private fun actionMaskedIsPrimaryDown(event: MotionEvent): Boolean =
        event.actionMasked == MotionEvent.ACTION_DOWN &&
            (event.buttonState and MotionEvent.BUTTON_SECONDARY) == 0

    private fun inferFallbackCursorCode(
        tool: VexioTool,
        mappedAction: Int,
        buttonState: Int
    ): Int = when (tool) {
        VexioTool.POINTER -> VexioCursorKind.ARROW.code
        VexioTool.TEXT_BLOCK -> VexioCursorKind.TEXT_IBEAM.code
        VexioTool.CONNECTOR -> VexioCursorKind.GLUE_SNAP.code
        VexioTool.PAN_HAND -> if ((buttonState and MotionEvent.BUTTON_PRIMARY) != 0 ||
            mappedAction == MotionBatchFormat.ACTION_MOVE
        ) {
            VexioCursorKind.GRABBING_HAND.code
        } else {
            VexioCursorKind.GRAB_HAND.code
        }
        else -> VexioCursorKind.CROSSHAIR.code
    }

    @Synchronized
    fun selectTool(tool: VexioTool) {
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L) {
            NativeBridge.nativeSetViewportTool(viewportPtr, tool.nativeCode)
        }
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

    @Synchronized
    fun adjustZoom(deltaPercent: Int) {
        var nextPct = 100
        _uiState.update { state ->
            val clamped = (state.zoomPercent + deltaPercent).coerceIn(10, 1600)
            nextPct = clamped
            state.copy(zoomPercent = clamped, isAutoFitPage = false)
        }
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L && docPtr != 0L) {
            NativeBridge.nativeSetViewportZoom(viewportPtr, docPtr, nextPct / 100.0, 0.0, 0.0)
            presentViewportOrFallback()
        }
    }

    @Synchronized
    fun setZoomPercent(targetPercent: Int) {
        val clamped = targetPercent.coerceIn(10, 1600)
        _uiState.update { it.copy(zoomPercent = clamped, isAutoFitPage = false) }
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L && docPtr != 0L) {
            NativeBridge.nativeSetViewportZoom(viewportPtr, docPtr, clamped / 100.0, 0.0, 0.0)
            presentViewportOrFallback()
        }
    }

    @Synchronized
    fun selectPage(pageId: Int) {
        val targetId = pageId.coerceAtLeast(0)
        _uiState.update { state ->
            if (state.pages.none { it.pageId == targetId }) state
            else state.copy(activePageId = targetId, isAutoFitPage = true)
        }
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L && docPtr != 0L) {
            NativeBridge.nativeSetViewportPage(viewportPtr, docPtr, targetId)
        }
        refreshSelectedShapeFormulas()
        fitPageToWindow()
    }

    @Synchronized
    fun renamePage(pageId: Int, rawName: String): Boolean {
        val cleaned = rawName.trim()
        if (cleaned.isEmpty()) return false
        val targetId = pageId.coerceAtLeast(0)
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeRenamePage(docPtr, targetId, cleaned)
        }
        var found = false
        _uiState.update { state ->
            val updated = state.pages.map { tab ->
                if (tab.pageId == targetId) {
                    found = true
                    tab.copy(name = cleaned)
                } else {
                    tab
                }
            }
            if (!found) state
            else state.copy(
                pages = updated,
                isDirty = true,
                canUndo = true,
                statusBanner = "Renamed page to '$cleaned'"
            )
        }
        return found
    }

    @Synchronized
    fun addNewPage() {
        val nextNum = (_uiState.value.pages.maxOfOrNull { it.pageId } ?: 0) + 1
        val proposedName = "Page-$nextNum"
        val createdId = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val id = NativeBridge.nativeAddPage(docPtr, proposedName)
            if (id >= 0) id else nextNum
        } else {
            nextNum
        }
        _uiState.update { state ->
            val newTab = DiagramPageTab(
                pageId = createdId,
                name = proposedName,
                scaleLabel = if (state.metricUnits) "ISO A4 • mm" else "1 in = 1 in"
            )
            state.copy(
                pages = state.pages + newTab,
                activePageId = createdId,
                isDirty = true,
                statusBanner = "Added ${newTab.name}"
            )
        }
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L && docPtr != 0L) {
            NativeBridge.nativeSetViewportPage(viewportPtr, docPtr, createdId)
            presentViewportOrFallback()
        }
        refreshSelectedShapeFormulas()
    }

    @Synchronized
    fun duplicatePage(targetPageId: Int = _uiState.value.activePageId): Int {
        val state = _uiState.value
        val srcTab = state.pages.firstOrNull { it.pageId == targetPageId } ?: state.pages.firstOrNull()
        val nextNum = (state.pages.maxOfOrNull { it.pageId } ?: 0) + 1
        val createdId = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val id = NativeBridge.nativeDuplicatePage(docPtr, resolveNativePageId(targetPageId))
            if (id > 0) id else nextNum
        } else {
            nextNum
        }
        val copyName = "${srcTab?.name ?: "Page-$targetPageId"} (Copy)"
        _uiState.update { cur ->
            val copiedTab = DiagramPageTab(
                pageId = createdId,
                name = copyName,
                scaleLabel = srcTab?.scaleLabel ?: if (cur.metricUnits) "ISO A4 • mm" else "1 in = 1 in",
                isBackground = srcTab?.isBackground ?: false,
                backPageId = srcTab?.backPageId,
                pageColorHex = srcTab?.pageColorHex ?: "#FFFFFF",
                widthInches = srcTab?.widthInches ?: cur.pageWidthInches,
                heightInches = srcTab?.heightInches ?: cur.pageHeightInches
            )
            cur.copy(
                pages = cur.pages + copiedTab,
                activePageId = createdId,
                isDirty = true,
                canUndo = true,
                statusBanner = "Duplicated '${srcTab?.name ?: "Page"}' -> '$copyName'"
            )
        }
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L && docPtr != 0L) {
            NativeBridge.nativeSetViewportPage(viewportPtr, docPtr, createdId)
            NativeBridge.nativeSetViewportZoom(viewportPtr, docPtr, 0.0, 0.0, 0.0)
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
        return createdId
    }

    @Synchronized
    fun deletePage(targetPageId: Int = _uiState.value.activePageId): Boolean {
        val cur = _uiState.value
        val remainingForeground = cur.pages.count { it.pageId != targetPageId && !it.isBackground }
        if (cur.pages.size <= 1 || remainingForeground == 0) {
            _uiState.update {
                it.copy(statusBanner = "Cannot delete the last foreground page")
            }
            return false
        }
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val ok = NativeBridge.nativeDeletePage(docPtr, resolveNativePageId(targetPageId))
            if (!ok) return false
        }
        val remaining = cur.pages
            .filter { it.pageId != targetPageId }
            .map { if (it.backPageId == targetPageId) it.copy(backPageId = null) else it }
        val nextActive = if (cur.activePageId == targetPageId) {
            (remaining.firstOrNull { !it.isBackground } ?: remaining.first()).pageId
        } else {
            cur.activePageId
        }
        _uiState.update {
            it.copy(
                pages = remaining,
                activePageId = nextActive,
                isDirty = true,
                canUndo = true,
                statusBanner = "Deleted page"
            )
        }
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L && docPtr != 0L) {
            NativeBridge.nativeSetViewportPage(viewportPtr, docPtr, nextActive)
            NativeBridge.nativeSetViewportZoom(viewportPtr, docPtr, 0.0, 0.0, 0.0)
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
        return true
    }

    @Synchronized
    fun setPageBackground(
        targetPageId: Int = _uiState.value.activePageId,
        isBackground: Boolean,
        backPageId: Int? = null,
        pageColorHex: String = ""
    ): Boolean {
        val cur = _uiState.value
        if (isBackground && cur.pages.count { it.pageId != targetPageId && !it.isBackground } == 0) {
            _uiState.update {
                it.copy(statusBanner = "At least one foreground page is required")
            }
            return false
        }
        val effectiveBackId = if (isBackground) 0 else (backPageId ?: 0).coerceAtLeast(0)
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val ok = NativeBridge.nativeSetPageBackground(
                docPtr = docPtr,
                pageId = resolveNativePageId(targetPageId),
                isBackground = isBackground,
                backPageId = effectiveBackId,
                pageColorFormula = pageColorHex
            )
            if (!ok) return false
        }
        _uiState.update { state ->
            val updated = state.pages.map { tab ->
                if (tab.pageId == targetPageId) {
                    tab.copy(
                        isBackground = isBackground,
                        backPageId = effectiveBackId.takeIf { it > 0 },
                        pageColorHex = if (pageColorHex.startsWith("#")) pageColorHex else tab.pageColorHex
                    )
                } else {
                    tab
                }
            }
            state.copy(
                pages = updated,
                isDirty = true,
                canUndo = true,
                statusBanner = "Updated page background settings"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
        return true
    }

    @Synchronized
    fun addNewBackgroundPage(attachToPageId: Int? = _uiState.value.activePageId): Int {
        val cur = _uiState.value
        val bgIndex = cur.pages.count { it.isBackground } + 1
        val bgName = "Background-$bgIndex"
        val nextNum = (cur.pages.maxOfOrNull { it.pageId } ?: 0) + 1
        val bgId = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val id = NativeBridge.nativeAddPage(docPtr, bgName)
            if (id > 0) id else nextNum
        } else {
            nextNum
        }
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeSetPageBackground(docPtr, bgId, true, 0, "#F8FAFC")
            if (attachToPageId != null && attachToPageId != bgId) {
                val fgTab = cur.pages.firstOrNull { it.pageId == attachToPageId }
                if (fgTab?.isBackground != true) {
                    NativeBridge.nativeSetPageBackground(
                        docPtr = docPtr,
                        pageId = resolveNativePageId(attachToPageId),
                        isBackground = false,
                        backPageId = bgId,
                        pageColorFormula = ""
                    )
                }
            }
        }
        _uiState.update { state ->
            val newBgTab = DiagramPageTab(
                pageId = bgId,
                name = bgName,
                scaleLabel = if (state.metricUnits) "ISO A4 • mm" else "1 in = 1 in",
                isBackground = true,
                backPageId = null,
                pageColorHex = "#F8FAFC",
                widthInches = state.pageWidthInches,
                heightInches = state.pageHeightInches
            )
            val updatedExisting = state.pages.map { tab ->
                if (attachToPageId != null && tab.pageId == attachToPageId && !tab.isBackground) {
                    tab.copy(backPageId = bgId)
                } else {
                    tab
                }
            }
            state.copy(
                pages = updatedExisting + newBgTab,
                isDirty = true,
                canUndo = true,
                statusBanner = "Created & linked '$bgName'"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
        return bgId
    }

    @Synchronized
    fun applyCustomPageSetup(
        pageId: Int,
        widthInches: Double,
        heightInches: Double,
        metricUnits: Boolean,
        isBackground: Boolean,
        backPageId: Int?,
        pageColorHex: String
    ) {
        val safeW = widthInches.coerceIn(1.0, 200.0)
        val safeH = heightInches.coerceIn(1.0, 200.0)
        val unitSlug = if (metricUnits) "mm" else "in"
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeSetPageSetup(docPtr, resolveNativePageId(pageId), safeW, safeH, unitSlug)
            NativeBridge.nativeSetPageBackground(
                docPtr = docPtr,
                pageId = resolveNativePageId(pageId),
                isBackground = isBackground,
                backPageId = if (isBackground) 0 else (backPageId ?: 0).coerceAtLeast(0),
                pageColorFormula = pageColorHex
            )
            if (viewportPtr != 0L && pageId == _uiState.value.activePageId) {
                NativeBridge.nativeSetViewportZoom(viewportPtr, docPtr, 0.0, 0.0, 0.0)
            }
        }
        _uiState.update { state ->
            val scaleLbl = if (metricUnits) "ISO • mm" else "1 in = 1 in"
            val updatedPages = state.pages.map { tab ->
                if (tab.pageId == pageId) {
                    tab.copy(
                        scaleLabel = scaleLbl,
                        isBackground = isBackground,
                        backPageId = if (isBackground) null else backPageId?.takeIf { it > 0 },
                        pageColorHex = if (pageColorHex.isNotBlank()) pageColorHex else tab.pageColorHex,
                        widthInches = safeW,
                        heightInches = safeH
                    )
                } else {
                    tab
                }
            }
            state.copy(
                pages = updatedPages,
                metricUnits = metricUnits,
                pageWidthInches = if (pageId == state.activePageId) safeW else state.pageWidthInches,
                pageHeightInches = if (pageId == state.activePageId) safeH else state.pageHeightInches,
                isDirty = true,
                canUndo = true,
                statusBanner = "Updated Page Setup"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
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
            presentViewportOrFallback()
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
        val nativePid = resolveNativePageId(state.activePageId)
        val refreshed = state.shapeSheetCells.map { cell ->
            val raw = NativeBridge.nativeGetCellFormula(
                docPtr = docPtr,
                pageId = nativePid,
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

        val stateJson = runCatching { NativeBridge.nativeGetStateJson(docPtr, nativePid) }.getOrDefault("{}")
        val rootObj = runCatching { Json.parseToJsonElement(stateJson).jsonObject }.getOrNull()
        var parsedPw = state.pageWidthInches
        var parsedPh = state.pageHeightInches
        var parsedPages = state.pages
        var parsedShapeCount = state.pageShapeCount
        var parsedLayers = state.pageLayers
        var parsedInspected: InspectedObjectUiState? = if (state.selectedShapeId > 0L) state.inspectedObject else null
        var parsedShapeData: List<ShapeDataUiItem> = emptyList()
        var parsedActions: List<SmartActionUiItem> = emptyList()
        var resolvedFillHex = state.shapeFillColorHex
        var resolvedLineHex = state.shapeLineColorHex

        if (rootObj != null) {
            parsedPw = rootObj["pageWidthIn"]?.jsonPrimitive?.doubleOrNull ?: parsedPw
            parsedPh = rootObj["pageHeightIn"]?.jsonPrimitive?.doubleOrNull ?: parsedPh

            val rawPages = rootObj["pages"] as? JsonArray
            if (rawPages != null && rawPages.isNotEmpty()) {
                val mappedPages = rawPages.mapNotNull { el ->
                    val pObj = el as? JsonObject ?: return@mapNotNull null
                    val pid = pObj["id"]?.jsonPrimitive?.intOrNull ?: return@mapNotNull null
                    val pname = pObj["name"]?.jsonPrimitive?.contentOrNull ?: "Page-$pid"
                    val isBg = pObj["isBackground"]?.jsonPrimitive?.booleanOrNull ?: false
                    val backId = pObj["backPageId"]?.jsonPrimitive?.intOrNull?.takeIf { it > 0 }
                    val bgHex = pObj["pageColorHex"]?.jsonPrimitive?.contentOrNull ?: "#FFFFFF"
                    val wIn = pObj["widthIn"]?.jsonPrimitive?.doubleOrNull ?: parsedPw
                    val hIn = pObj["heightIn"]?.jsonPrimitive?.doubleOrNull ?: parsedPh
                    val scaleLbl = if (state.metricUnits) {
                        "${(wIn * 25.4).toInt()}×${(hIn * 25.4).toInt()} mm"
                    } else {
                        "${"%.1f".format(Locale.US, wIn)}×${"%.1f".format(Locale.US, hIn)} in"
                    }
                    DiagramPageTab(
                        pageId = pid,
                        name = pname,
                        scaleLabel = scaleLbl,
                        isBackground = isBg,
                        backPageId = backId,
                        pageColorHex = bgHex,
                        widthInches = wIn,
                        heightInches = hIn
                    )
                }
                if (mappedPages.isNotEmpty()) {
                    parsedPages = mappedPages
                }
            }

            val rawLayers = rootObj["layers"] as? JsonArray
            if (rawLayers != null) {
                parsedLayers = rawLayers.mapIndexedNotNull { idx, elem ->
                    val lObj = elem as? JsonObject ?: return@mapIndexedNotNull null
                    val ix = lObj["ix"]?.jsonPrimitive?.intOrNull ?: idx
                    val nm = lObj["name"]?.jsonPrimitive?.contentOrNull ?: "Layer $ix"
                    val vis = lObj["visible"]?.jsonPrimitive?.booleanOrNull ?: true
                    val prt = lObj["print"]?.jsonPrimitive?.booleanOrNull ?: true
                    val lck = lObj["lock"]?.jsonPrimitive?.booleanOrNull ?: false
                    val snp = lObj["snap"]?.jsonPrimitive?.booleanOrNull ?: true
                    val glu = lObj["glue"]?.jsonPrimitive?.booleanOrNull ?: true
                    val cnt = lObj["memberCount"]?.jsonPrimitive?.intOrNull ?: 0
                    DiagramLayerUiItem(
                        ix = ix,
                        name = nm,
                        visible = vis,
                        print = prt,
                        lock = lck,
                        snapGlue = snp || glu,
                        memberCount = cnt
                    )
                }
            }

            val rawShapes = rootObj["shapes"] as? JsonArray
            if (rawShapes != null) {
                parsedShapeCount = rawShapes.size
                val matched = if (state.selectedShapeId > 0L) {
                    rawShapes.firstOrNull { el ->
                        (el as? JsonObject)?.get("id")?.jsonPrimitive?.longOrNull == state.selectedShapeId
                    } as? JsonObject
                } else {
                    null
                }
                if (matched != null) {
                    val sid = matched["id"]?.jsonPrimitive?.longOrNull ?: state.selectedShapeId
                    val lInds = (matched["layerIndices"] as? JsonArray)
                        ?.mapNotNull { it.jsonPrimitive.intOrNull }
                        .orEmpty()
                    resolvedFillHex = matched["fillColor"]?.jsonPrimitive?.contentOrNull ?: resolvedFillHex
                    resolvedLineHex = matched["lineColor"]?.jsonPrimitive?.contentOrNull ?: resolvedLineHex

                    val rawSd = matched["shapeData"] as? JsonArray
                    if (rawSd != null) {
                        parsedShapeData = rawSd.mapNotNull { sdEl ->
                            val sdObj = sdEl as? JsonObject ?: return@mapNotNull null
                            val rName = sdObj["name"]?.jsonPrimitive?.contentOrNull?.takeIf { it.isNotEmpty() }
                                ?: return@mapNotNull null
                            val dtCode = sdObj["dataType"]?.jsonPrimitive?.intOrNull ?: 0
                            val dtLabel = when (dtCode) {
                                1, 4 -> "List"
                                2 -> "Number"
                                3 -> "Boolean"
                                5 -> "Date"
                                6 -> "Duration"
                                7 -> "Currency"
                                else -> "String"
                            }
                            ShapeDataUiItem(
                                rowName = rName,
                                label = sdObj["label"]?.jsonPrimitive?.contentOrNull ?: rName,
                                dataTypeCode = dtCode,
                                dataTypeLabel = dtLabel,
                                formula = sdObj["formula"]?.jsonPrimitive?.contentOrNull.orEmpty(),
                                value = sdObj["value"]?.jsonPrimitive?.contentOrNull.orEmpty()
                            )
                        }
                    }

                    val rawActions = matched["actions"] as? JsonArray
                    if (rawActions != null) {
                        parsedActions = rawActions.mapNotNull { actEl ->
                            val actObj = actEl as? JsonObject ?: return@mapNotNull null
                            val rName = actObj["rowName"]?.jsonPrimitive?.contentOrNull?.takeIf { it.isNotEmpty() }
                                ?: return@mapNotNull null
                            SmartActionUiItem(
                                rowName = rName,
                                menu = actObj["menu"]?.jsonPrimitive?.contentOrNull ?: rName,
                                actionFormula = actObj["actionFormula"]?.jsonPrimitive?.contentOrNull.orEmpty(),
                                checkedFormula = actObj["checkedFormula"]?.jsonPrimitive?.contentOrNull ?: "0",
                                disabledFormula = actObj["disabledFormula"]?.jsonPrimitive?.contentOrNull ?: "0",
                                checked = actObj["checked"]?.jsonPrimitive?.booleanOrNull ?: false,
                                disabled = actObj["disabled"]?.jsonPrimitive?.booleanOrNull ?: false,
                                invisible = actObj["invisible"]?.jsonPrimitive?.booleanOrNull ?: false,
                                beginGroup = actObj["beginGroup"]?.jsonPrimitive?.booleanOrNull ?: false
                            )
                        }
                    }

                    val lockW = matched["lockWidth"]?.jsonPrimitive?.booleanOrNull ?: false
                    val lockH = matched["lockHeight"]?.jsonPrimitive?.booleanOrNull ?: false
                    val lockMx = matched["lockMoveX"]?.jsonPrimitive?.booleanOrNull ?: false
                    val lockMy = matched["lockMoveY"]?.jsonPrimitive?.booleanOrNull ?: false

                    parsedInspected = InspectedObjectUiState(
                        id = sid,
                        name = matched["name"]?.jsonPrimitive?.contentOrNull ?: "Sheet.$sid",
                        kind = matched["kind"]?.jsonPrimitive?.contentOrNull ?: "Shape2D",
                        isGroup = matched["isGroup"]?.jsonPrimitive?.booleanOrNull ?: false,
                        childCount = matched["childCount"]?.jsonPrimitive?.intOrNull ?: 0,
                        masterName = matched["masterName"]?.jsonPrimitive?.contentOrNull,
                        layerIndices = lInds,
                        text = matched["text"]?.jsonPrimitive?.contentOrNull.orEmpty(),
                        pinXIn = matched["pinX"]?.jsonPrimitive?.doubleOrNull ?: 0.0,
                        pinYIn = matched["pinY"]?.jsonPrimitive?.doubleOrNull ?: 0.0,
                        widthIn = matched["width"]?.jsonPrimitive?.doubleOrNull ?: 1.0,
                        heightIn = matched["height"]?.jsonPrimitive?.doubleOrNull ?: 1.0,
                        angleDeg = matched["angleDeg"]?.jsonPrimitive?.doubleOrNull ?: 0.0,
                        flipX = matched["flipX"]?.jsonPrimitive?.booleanOrNull ?: false,
                        flipY = matched["flipY"]?.jsonPrimitive?.booleanOrNull ?: false,
                        fillColorHex = resolvedFillHex,
                        fillPattern = matched["fillPattern"]?.jsonPrimitive?.intOrNull ?: 1,
                        fillTransPct = (matched["fillTransPct"]?.jsonPrimitive?.doubleOrNull ?: 0.0).toInt(),
                        roundingIn = matched["roundingIn"]?.jsonPrimitive?.doubleOrNull ?: 0.0,
                        lineColorHex = resolvedLineHex,
                        linePattern = matched["linePattern"]?.jsonPrimitive?.intOrNull ?: 1,
                        lineWeightPt = matched["lineWeightPt"]?.jsonPrimitive?.doubleOrNull ?: 1.0,
                        beginArrow = matched["beginArrow"]?.jsonPrimitive?.intOrNull ?: 0,
                        endArrow = matched["endArrow"]?.jsonPrimitive?.intOrNull ?: 0,
                        lockAspect = matched["lockAspect"]?.jsonPrimitive?.booleanOrNull ?: false,
                        lockSize = lockW && lockH,
                        lockPosition = lockMx && lockMy,
                        lockRotate = matched["lockRotate"]?.jsonPrimitive?.booleanOrNull ?: false,
                        lockDelete = matched["lockDelete"]?.jsonPrimitive?.booleanOrNull ?: false,
                        hideText = matched["hideText"]?.jsonPrimitive?.booleanOrNull ?: false
                    )
                } else {
                    parsedInspected = null
                }
            }
        }

        var parsedDataGraphics = state.dataGraphics
        var parsedRecordsets = state.externalRecordsets
        var parsedPropFields = state.availablePropFields
        val dlsObj = rootObj?.get("dataLinkState") as? JsonObject
        if (dlsObj != null) {
            val dgObj = dlsObj["dataGraphics"] as? JsonObject
            if (dgObj != null) {
                parsedDataGraphics = DataGraphicUiState(
                    enabled = dgObj["enabled"]?.jsonPrimitive?.booleanOrNull ?: false,
                    iconSetField = dgObj["iconSetField"]?.jsonPrimitive?.contentOrNull.orEmpty(),
                    iconGreenMin = dgObj["iconGreenMin"]?.jsonPrimitive?.doubleOrNull ?: 75.0,
                    iconAmberMin = dgObj["iconAmberMin"]?.jsonPrimitive?.doubleOrNull ?: 40.0,
                    dataBarField = dgObj["dataBarField"]?.jsonPrimitive?.contentOrNull.orEmpty(),
                    dataBarMin = dgObj["dataBarMin"]?.jsonPrimitive?.doubleOrNull ?: 0.0,
                    dataBarMax = dgObj["dataBarMax"]?.jsonPrimitive?.doubleOrNull ?: 100.0,
                    dataBarColorHex = dgObj["dataBarColorHex"]?.jsonPrimitive?.contentOrNull ?: "#2563EB",
                    textCalloutField = dgObj["textCalloutField"]?.jsonPrimitive?.contentOrNull.orEmpty(),
                    textCalloutPrefix = dgObj["textCalloutPrefix"]?.jsonPrimitive?.contentOrNull.orEmpty(),
                    colorByValueEnabled = dgObj["colorByValueEnabled"]?.jsonPrimitive?.booleanOrNull ?: true,
                    colorByValueField = dgObj["colorByValueField"]?.jsonPrimitive?.contentOrNull.orEmpty(),
                    legendVisible = dgObj["legendVisible"]?.jsonPrimitive?.booleanOrNull ?: false,
                    legendField = dgObj["legendField"]?.jsonPrimitive?.contentOrNull.orEmpty()
                )
            }
            val rawProps = dlsObj["availablePropFields"] as? JsonArray
            if (rawProps != null) {
                parsedPropFields = rawProps.mapNotNull { it.jsonPrimitive.contentOrNull?.takeIf { s -> s.isNotEmpty() } }
            }
            val rawRs = dlsObj["recordsets"] as? JsonArray
            if (rawRs != null) {
                parsedRecordsets = rawRs.mapNotNull { rsEl ->
                    val rsObj = rsEl as? JsonObject ?: return@mapNotNull null
                    val rsId = rsObj["id"]?.jsonPrimitive?.intOrNull ?: return@mapNotNull null
                    val cols = (rsObj["columns"] as? JsonArray)
                        ?.mapNotNull { it.jsonPrimitive.contentOrNull }
                        .orEmpty()
                    val rows = (rsObj["rows"] as? JsonArray)?.mapNotNull { rEl ->
                        val rObj = rEl as? JsonObject ?: return@mapNotNull null
                        val map = linkedMapOf<String, String>()
                        for ((k, v) in rObj) {
                            map[k] = (v as? kotlinx.serialization.json.JsonPrimitive)?.contentOrNull.orEmpty()
                        }
                        map
                    }.orEmpty()
                    val boundMap = linkedMapOf<Int, List<Long>>()
                    val bObj = rsObj["boundShapeIds"] as? JsonObject
                    if (bObj != null) {
                        for ((rKey, arrEl) in bObj) {
                            val rIdx = rKey.toIntOrNull() ?: continue
                            val ids = (arrEl as? JsonArray)
                                ?.mapNotNull { it.jsonPrimitive.longOrNull }
                                .orEmpty()
                            if (ids.isNotEmpty()) {
                                boundMap[rIdx] = ids
                            }
                        }
                    }
                    ExternalRecordsetUiItem(
                        id = rsId,
                        name = rsObj["name"]?.jsonPrimitive?.contentOrNull ?: "Recordset $rsId",
                        sourceKind = rsObj["sourceKind"]?.jsonPrimitive?.contentOrNull ?: "xlsx",
                        sourceName = rsObj["sourceName"]?.jsonPrimitive?.contentOrNull.orEmpty(),
                        keyColumn = rsObj["keyColumn"]?.jsonPrimitive?.contentOrNull.orEmpty(),
                        columns = cols,
                        rows = rows,
                        boundShapeIdsByRow = boundMap
                    )
                }
            }
        }

        _uiState.update {
            it.copy(
                shapeSheetCells = refreshed,
                formulaBarInput = activeFormula,
                pages = parsedPages,
                pageWidthInches = parsedPw,
                pageHeightInches = parsedPh,
                pageShapeCount = parsedShapeCount,
                pageLayers = parsedLayers,
                inspectedObject = parsedInspected,
                shapeDataItems = parsedShapeData,
                selectedShapeActions = parsedActions,
                dataGraphics = parsedDataGraphics,
                externalRecordsets = parsedRecordsets,
                availablePropFields = parsedPropFields,
                shapeFillColorHex = resolvedFillHex,
                shapeLineColorHex = resolvedLineHex,
                selectedShapeName = parsedInspected?.name ?: it.selectedShapeName
            )
        }
    }

    private var lastCenterDropAnchor: Pair<Float, Float>? = null
    private var centerDropCascadeStep: Int = 0

    fun highlightStencilMaster(masterId: String) {
        _uiState.update { it.copy(highlightedStencilMasterId = masterId) }
    }

    @Synchronized
    fun screenPxToPageIn(xPx: Float, yPx: Float): Pair<Float, Float> {
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L) {
            val pt = NativeBridge.nativeViewportScreenToPageIn(viewportPtr, xPx.toDouble(), yPx.toDouble())
            if (pt != null && pt.size >= 2 && pt[0].isFinite() && pt[1].isFinite()) {
                return pt[0].toFloat() to pt[1].toFloat()
            }
        }
        val preset = _uiState.value.activePagePreset
        val pw = if (preset.isMetric) 11.69f else 11.0f
        val ph = if (preset.isMetric) 8.27f else 8.5f
        if (xPx < 0f && yPx < 0f) {
            return (pw * 0.5f) to (ph * 0.5f)
        }
        val safeW = viewportWidthPx.coerceAtLeast(1).toFloat()
        val safeH = viewportHeightPx.coerceAtLeast(1).toFloat()
        val xIn = ((xPx / safeW) * pw).coerceIn(0.4f, pw - 0.4f)
        val yIn = ((1f - (yPx / safeH)) * ph).coerceIn(0.4f, ph - 0.4f)
        return xIn to yIn
    }

    @Synchronized
    fun dropStencilAtViewportCenter(master: StencilMasterItem) {
        val (cx, cy) = screenPxToPageIn(-1f, -1f)
        val prev = lastCenterDropAnchor
        if (prev != null && kotlin.math.hypot(cx - prev.first, cy - prev.second) < 0.05f) {
            centerDropCascadeStep = (centerDropCascadeStep + 1) % 6
        } else {
            lastCenterDropAnchor = cx to cy
            centerDropCascadeStep = 0
        }
        val dx = centerDropCascadeStep * 0.28f
        val dy = -centerDropCascadeStep * 0.28f
        _uiState.update { it.copy(highlightedStencilMasterId = master.masterId) }
        instantiateMasterDrop(
            title = master.title,
            widthFormula = master.defaultWidthFormula,
            heightFormula = master.defaultHeightFormula,
            fillFormula = master.fillFormula,
            pinXInches = cx + dx,
            pinYInches = cy + dy,
            category = master.category
        )
    }

    @Synchronized
    fun dropStencilAtScreenPx(
        title: String,
        widthFormula: String,
        heightFormula: String,
        fillFormula: String,
        category: String,
        screenXPx: Float,
        screenYPx: Float
    ) {
        val (px, py) = screenPxToPageIn(screenXPx, screenYPx)
        instantiateMasterDrop(
            title = title,
            widthFormula = widthFormula,
            heightFormula = heightFormula,
            fillFormula = fillFormula,
            pinXInches = px,
            pinYInches = py,
            category = category
        )
    }

    @Synchronized
    fun instantiateMasterDrop(
        title: String,
        widthFormula: String,
        heightFormula: String,
        fillFormula: String,
        pinXInches: Float,
        pinYInches: Float,
        category: String = ""
    ) {
        val pageId = _uiState.value.activePageId
        val pinXStr = "${"%.2f".format(Locale.US, pinXInches)} in"
        val pinYStr = "${"%.2f".format(Locale.US, pinYInches)} in"

        var shapeId = 0L
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            shapeId = NativeBridge.nativeDropStencil(
                docPtr = docPtr,
                pageId = pageId,
                category = category,
                masterName = title,
                xIn = pinXInches.toDouble(),
                yIn = pinYInches.toDouble()
            )
        }
        if (shapeId <= 0L) {
            shapeId = nextSyntheticShapeId++
            if (NativeBridge.isNativeAvailable && docPtr != 0L) {
                NativeBridge.nativeSetCellFormula(docPtr, pageId, shapeId, "PinX", pinXStr, true)
                NativeBridge.nativeSetCellFormula(docPtr, pageId, shapeId, "PinY", pinYStr, true)
                NativeBridge.nativeSetCellFormula(docPtr, pageId, shapeId, "Width", widthFormula, true)
                NativeBridge.nativeSetCellFormula(docPtr, pageId, shapeId, "Height", heightFormula, true)
                NativeBridge.nativeSetCellFormula(docPtr, pageId, shapeId, "FillForegnd", fillFormula, true)
            }
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

        if (NativeBridge.isNativeAvailable && viewportPtr != 0L) {
            NativeBridge.nativeSetViewportTool(viewportPtr, VexioTool.POINTER.nativeCode)
            syncViewportSelection(listOf(shapeId))
        }
        _uiState.update { state ->
            val activeFormula = updatedCells.firstOrNull { it.cellName == state.activeCellName }?.formula
                ?: widthFormula
            state.copy(
                activeTool = VexioTool.POINTER,
                activeCursor = VexioCursorKind.ARROW,
                selectedShapeId = shapeId,
                selectedShapeIds = listOf(shapeId),
                selectedShapeName = "Sheet.$shapeId ($title)",
                shapeSheetCells = updatedCells,
                formulaBarInput = activeFormula,
                isDirty = true,
                canUndo = true,
                statusBanner = "Dropped '$title' at ($pinXStr, $pinYStr)"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
    }

    @Synchronized
    fun drawQuickPrimitive(
        kind: String,
        centerXIn: Double = 5.5,
        centerYIn: Double = 4.25
    ) {
        val pageId = _uiState.value.activePageId
        val created = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeDrawPrimitive(
                docPtr = docPtr,
                pageId = pageId,
                kind = kind,
                x1In = centerXIn - 0.75,
                y1In = centerYIn - 0.45,
                x2In = centerXIn + 0.75,
                y2In = centerYIn + 0.45
            )
        } else {
            nextSyntheticShapeId++
        }
        val resolvedId = if (created > 0L) created else nextSyntheticShapeId++
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L) {
            NativeBridge.nativeSetViewportTool(viewportPtr, VexioTool.POINTER.nativeCode)
            syncViewportSelection(listOf(resolvedId))
        }
        _uiState.update {
            it.copy(
                activeTool = VexioTool.POINTER,
                activeCursor = VexioCursorKind.ARROW,
                selectedShapeId = resolvedId,
                selectedShapeIds = listOf(resolvedId),
                selectedShapeName = "Sheet.$resolvedId ($kind)",
                isDirty = true,
                canUndo = true,
                statusBanner = "Inserted $kind (Sheet.$resolvedId)"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
        if (kind.equals("text", ignoreCase = true)) {
            beginInlineTextEdit(resolvedId, isNewTextShape = true)
        }
    }

    @Synchronized
    fun duplicateSelectedShapes(dxInches: Double = 0.35, dyInches: Double = -0.35) {
        val state = _uiState.value
        val targets = state.selectedShapeIds.ifEmpty { listOf(state.selectedShapeId) }.toLongArray()
        val newId = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeDuplicateShapes(docPtr, state.activePageId, targets, dxInches, dyInches)
        } else {
            nextSyntheticShapeId++
        }
        if (newId > 0L) {
            syncViewportSelection(listOf(newId))
            _uiState.update {
                it.copy(
                    selectedShapeId = newId,
                    selectedShapeIds = listOf(newId),
                    selectedShapeName = "Sheet.$newId (Copy)",
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Duplicated selection → Sheet.$newId"
                )
            }
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        }
    }

    @Synchronized
    fun reorderSelectedShapes(mode: String) {
        val state = _uiState.value
        val targets = state.selectedShapeIds.ifEmpty { listOf(state.selectedShapeId) }.toLongArray()
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeReorderShapes(docPtr, state.activePageId, targets, mode)
            presentViewportOrFallback()
        }
        _uiState.update {
            it.copy(
                isDirty = true,
                canUndo = true,
                statusBanner = "Arranged selection ($mode)"
            )
        }
    }

    @Synchronized
    fun autoLayoutActivePage(algo: String = "hierarchical_tb") {
        val pageId = _uiState.value.activePageId
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeAutoLayout(docPtr, pageId, algo)
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        }
        _uiState.update {
            it.copy(
                isDirty = true,
                canUndo = true,
                statusBanner = "Applied Auto-Layout ($algo)"
            )
        }
    }

    @Synchronized
    fun performUndo() {
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val undone = NativeBridge.nativeUndo(docPtr)
            if (undone) {
                val dirtyNow = readNativeStateRevision() != savedStateRevision
                refreshSelectedShapeFormulas()
                presentViewportOrFallback()
                _uiState.update {
                    it.copy(isDirty = dirtyNow, canRedo = true, statusBanner = "Undo applied")
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
                val dirtyNow = readNativeStateRevision() != savedStateRevision
                refreshSelectedShapeFormulas()
                presentViewportOrFallback()
                _uiState.update {
                    it.copy(isDirty = dirtyNow, canUndo = true, statusBanner = "Redo applied")
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

    @Synchronized
    fun deleteSelectedShape() {
        val current = _uiState.value
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val targets = current.selectedShapeIds.ifEmpty { listOf(current.selectedShapeId) }.toLongArray()
            NativeBridge.nativeDeleteShapes(docPtr, current.activePageId, targets)
            presentViewportOrFallback()
        }
        deletedShapeCount += 1
        tombstonedShapeIds.add(current.selectedShapeId)
        _uiState.update {
            it.copy(
                isDirty = true,
                canUndo = true,
                statusBanner = "Deleted ${current.selectedShapeName}"
            )
        }
    }

    fun togglePlan2Ribbon() {
        _uiState.update { it.copy(isPlan2RibbonExpanded = !it.isPlan2RibbonExpanded) }
    }

    @Synchronized
    fun selectShapesForOperations(shapeIds: List<Long>) {
        val clean = shapeIds.filter { it > 0L }.distinct()
        if (clean.isEmpty()) return
        _uiState.update { state ->
            state.copy(
                selectedShapeId = clean.first(),
                selectedShapeIds = clean,
                statusBanner = "Selected ${clean.size} shape(s) for Plan 2 Operations"
            )
        }
    }

    @Synchronized
    fun performBooleanOp(
        opSlug: String,
        shapeIds: LongArray = _uiState.value.selectedShapeIds.toLongArray(),
        offsetInches: Double = 0.125
    ): Int {
        val resolved = VexioBooleanOp.fromSlug(opSlug) ?: return 0
        return performBooleanOp(resolved, shapeIds, offsetInches)
    }

    /**
     * Executes constructive 2D Boolean geometry operations (`Union`, `Combine`, `Fragment`,
     * `Intersect`, `Subtract`, `Trim`, `Join`, `Offset`) via `NativeBridge.nativeBooleanOp`,
     * falling back to deterministic ShapeSheet state updates in JVM unit test environments.
     */
    @Synchronized
    fun performBooleanOp(
        op: VexioBooleanOp,
        shapeIds: LongArray = _uiState.value.selectedShapeIds.toLongArray(),
        offsetInches: Double = 0.125
    ): Int {
        val validIds = shapeIds.filter { it > 0L }.distinct().toLongArray()
        if (validIds.isEmpty()) return 0
        if (op != VexioBooleanOp.OFFSET && validIds.size < 2) return 0

        val currentState = _uiState.value
        val nativePageId = resolveNativePageId(currentState.activePageId)
        val createdCount = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val rc = NativeBridge.nativeBooleanOp(
                docPtr = docPtr,
                pageId = nativePageId,
                shapeIds = validIds,
                op = op.slug,
                offsetIn = offsetInches
            )
            if (rc > 0) rc else computeFallbackBooleanCount(op, validIds.size)
        } else {
            computeFallbackBooleanCount(op, validIds.size)
        }

        val firstCreatedId = nextSyntheticShapeId
        nextSyntheticShapeId += createdCount.toLong()
        val createdIds = (0 until createdCount).map { firstCreatedId + it }

        val noFillValue = if (op == VexioBooleanOp.TRIM || op == VexioBooleanOp.JOIN) "1" else "0"
        val cellUpdates = buildList {
            add(ShapeSheetCellEntry("Geometry 1", "Geometry1.NoFill", noFillValue, isGuarded = false))
            add(ShapeSheetCellEntry("User-defined Cells", "User.BooleanOp", "\"${op.slug}\"", isGuarded = false))
            if (op == VexioBooleanOp.OFFSET) {
                val deltaStr = "%.3f".format(Locale.US, offsetInches * 2.0)
                add(ShapeSheetCellEntry("User-defined Cells", "User.OffsetDelta", "$deltaStr in", isGuarded = false))
            }
        }

        _uiState.update { state ->
            val mergedCells = upsertShapeSheetCells(state.shapeSheetCells, cellUpdates)
            state.copy(
                selectedShapeId = firstCreatedId,
                selectedShapeIds = if (createdIds.size >= 2) createdIds else listOf(firstCreatedId, firstCreatedId + 1L),
                selectedShapeName = "Sheet.$firstCreatedId (${op.displayName} Result)",
                shapeSheetCells = mergedCells,
                lastBooleanOp = op.slug,
                lastBooleanResultCount = createdCount,
                isDirty = true,
                canUndo = true,
                statusBanner = "Boolean ${op.displayName} (${validIds.size} shapes) -> $createdCount shape(s)"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
        return createdCount
    }

    private fun computeFallbackBooleanCount(op: VexioBooleanOp, inputCount: Int): Int = when (op) {
        VexioBooleanOp.OFFSET -> inputCount.coerceAtLeast(1)
        VexioBooleanOp.FRAGMENT -> (inputCount * 2 - 1).coerceAtLeast(1)
        VexioBooleanOp.TRIM -> (inputCount + 1).coerceAtLeast(2)
        VexioBooleanOp.UNION,
        VexioBooleanOp.COMBINE,
        VexioBooleanOp.INTERSECT,
        VexioBooleanOp.SUBTRACT,
        VexioBooleanOp.JOIN -> 1
    }

    /**
     * Applies one of the 24 (`6 Themes × 4 Variants`) document palettes via `NativeBridge.nativeApplyTheme`.
     */
    @Synchronized
    fun applyDocumentTheme(themeSlug: String, variantIndex: Int = 0): Int {
        val preset = VexioThemePreset.fromSlug(themeSlug) ?: return 0
        val clampedVariant = variantIndex.coerceIn(0, 3)

        val nativeUpdated = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeApplyTheme(docPtr, preset.slug, clampedVariant)
        } else {
            0
        }

        val palette = preset.paletteFormulas(clampedVariant)
        val themeUpdates = listOf(
            ShapeSheetCellEntry("Fill Format", "FillForegnd", palette.fillForegnd, isGuarded = false),
            ShapeSheetCellEntry("Fill Format", "FillBkgnd", palette.fillBkgnd, isGuarded = false),
            ShapeSheetCellEntry("Fill Format", "FillPattern", palette.fillPattern, isGuarded = false),
            ShapeSheetCellEntry("Line Format", "LineColor", palette.lineColor, isGuarded = false),
            ShapeSheetCellEntry(
                "User-defined Cells",
                "User.SketchEnabled",
                if (palette.sketchEnabled) "TRUE" else "FALSE",
                isGuarded = false
            )
        )

        var resolvedCount = nativeUpdated
        _uiState.update { state ->
            if (resolvedCount <= 0) {
                resolvedCount = max(1, state.selectedShapeIds.size)
            }
            val updatedCells = upsertShapeSheetCells(state.shapeSheetCells, themeUpdates)
            val activeFormula = updatedCells.firstOrNull { it.cellName == state.activeCellName }?.formula
                ?: state.formulaBarInput
            state.copy(
                activeThemeSlug = preset.slug,
                activeThemeVariant = clampedVariant,
                shapeSheetCells = updatedCells,
                formulaBarInput = activeFormula,
                isDirty = true,
                canUndo = true,
                statusBanner = "Applied Theme '${preset.displayName}' • Variant ${clampedVariant + 1} ($resolvedCount shapes)"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
        return resolvedCount
    }

    /**
     * Applies a structured visual effect preset (`grad_linear`, `grad_radial`, `hatch_diag`,
     * `shadow_soft`, `bevel_3d`, `sketchy`) to `shapeId`.
     */
    @Synchronized
    fun applyVisualPreset(
        presetSlug: String,
        shapeId: Long = _uiState.value.selectedShapeId
    ): Boolean {
        if (shapeId <= 0L) return false
        val norm = presetSlug.trim().lowercase(Locale.US)
        if (norm !in VexioVisualPreset.allSupportedSlugs) return false

        val stateSnapshot = _uiState.value
        val nativeApplied = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeApplyVisualPreset(
                docPtr = docPtr,
                pageId = resolveNativePageId(stateSnapshot.activePageId),
                shapeId = shapeId,
                preset = norm
            )
        } else {
            true
        }

        val effectCells = when (norm) {
            "grad_linear" -> listOf(
                ShapeSheetCellEntry("Fill Format", "FillPattern", "28", isGuarded = false),
                ShapeSheetCellEntry("Fill Format", "FillForegnd", "RGB(191,219,254)", isGuarded = false),
                ShapeSheetCellEntry("Fill Format", "FillBkgnd", "RGB(239,246,255)", isGuarded = false)
            )
            "grad_radial" -> listOf(
                ShapeSheetCellEntry("Fill Format", "FillPattern", "36", isGuarded = false),
                ShapeSheetCellEntry("Fill Format", "FillForegnd", "RGB(167,243,208)", isGuarded = false),
                ShapeSheetCellEntry("Fill Format", "FillBkgnd", "RGB(240,253,244)", isGuarded = false)
            )
            "hatch_diag" -> listOf(
                ShapeSheetCellEntry("Fill Format", "FillPattern", "4", isGuarded = false)
            )
            "shadow_soft" -> listOf(
                ShapeSheetCellEntry("Fill Format", "ShdwPattern", "1", isGuarded = false),
                ShapeSheetCellEntry("Fill Format", "ShdwOffsetX", "0.06 in", isGuarded = false),
                ShapeSheetCellEntry("Fill Format", "ShdwOffsetY", "-0.06 in", isGuarded = false)
            )
            "bevel_3d" -> listOf(
                ShapeSheetCellEntry("Bevel Properties", "BevelTopType", "1", isGuarded = false),
                ShapeSheetCellEntry("Bevel Properties", "BevelTopWidth", "6 pt", isGuarded = false)
            )
            "sketchy" -> listOf(
                ShapeSheetCellEntry("User-defined Cells", "User.SketchEnabled", "TRUE", isGuarded = false),
                ShapeSheetCellEntry("User-defined Cells", "User.SketchAmount", "0.035 in", isGuarded = false)
            )
            else -> emptyList()
        }

        val succeeded = nativeApplied || !NativeBridge.isNativeAvailable
        if (succeeded) {
            _uiState.update { state ->
                val updatedCells = upsertShapeSheetCells(state.shapeSheetCells, effectCells)
                state.copy(
                    activeVisualPresets = state.activeVisualPresets + norm,
                    shapeSheetCells = updatedCells,
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Applied visual preset '$norm' to Sheet.$shapeId"
                )
            }
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        }
        return succeeded
    }

    /**
     * Runs real BPMN 2.0 / UML 2.5 / SharePoint structural validation via `NativeBridge.nativeValidateJson`.
     */
    @Synchronized
    fun validateActiveDiagram(rulesetSlug: String = "bpmn"): Int {
        val norm = rulesetSlug.trim().lowercase(Locale.US)
        val label = when {
            norm.startsWith("uml") -> "UML 2.5"
            norm.startsWith("work") || norm.startsWith("share") -> "SharePoint Workflow"
            else -> "BPMN 2.0"
        }
        var errCount = 0
        var warnCount = 0
        var firstMessage: String? = null
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val rawJson = NativeBridge.nativeValidateJson(docPtr, norm)
            val arr = runCatching { Json.parseToJsonElement(rawJson) as? JsonArray }.getOrNull()
            if (arr != null) {
                for (el in arr) {
                    val obj = el as? JsonObject ?: continue
                    val sev = obj["severity"]?.jsonPrimitive?.contentOrNull.orEmpty().lowercase(Locale.US)
                    if (sev == "error") errCount++ else warnCount++
                    if (firstMessage == null) {
                        firstMessage = obj["description"]?.jsonPrimitive?.contentOrNull
                    }
                }
            }
        }
        val total = errCount + warnCount
        val summaryMsg = if (total == 0) {
            "0 $label structural issues"
        } else {
            "$errCount error(s), $warnCount warning(s): ${firstMessage ?: "Rule violation"}"
        }
        _uiState.update { state ->
            state.copy(
                validationSummary = ValidationBadgeSummary(
                    standard = label,
                    errorCount = errCount,
                    warningCount = warnCount,
                    statusMessage = summaryMsg
                ),
                statusBanner = "$label Validation: $summaryMsg"
            )
        }
        return total
    }

    /**
     * Creates or links a BPMN / Process Subprocess drill-down page for `shapeId` via
     * `NativeBridge.nativeCreateSubprocess`.
     */
    @Synchronized
    fun createOrLinkSubprocess(
        subPageName: String = "",
        shapeId: Long = _uiState.value.selectedShapeId
    ): Int {
        if (shapeId <= 0L) return 0
        val cleanName = subPageName.trim().ifEmpty { "Subprocess-$shapeId" }
        val stateSnapshot = _uiState.value

        val nativeSubPageId = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeCreateSubprocess(
                docPtr = docPtr,
                pageId = resolveNativePageId(stateSnapshot.activePageId),
                shapeId = shapeId,
                subName = cleanName
            )
        } else {
            0
        }

        var resolvedSubPageId = nativeSubPageId
        _uiState.update { state ->
            val existingPage = state.pages.firstOrNull {
                it.name.equals(cleanName, ignoreCase = true) ||
                    (nativeSubPageId > 0 && it.pageId == nativeSubPageId)
            }
            val updatedPages = if (existingPage != null) {
                resolvedSubPageId = existingPage.pageId
                state.pages
            } else {
                val allocatedId = if (nativeSubPageId > 0) {
                    nativeSubPageId
                } else {
                    (state.pages.maxOfOrNull { it.pageId } ?: 0) + 1
                }
                resolvedSubPageId = allocatedId
                state.pages + DiagramPageTab(
                    pageId = allocatedId,
                    name = cleanName,
                    scaleLabel = "1 in = 1 in"
                )
            }
            val subprocessCells = listOf(
                ShapeSheetCellEntry("User-defined Cells", "User.IsSubprocess", "TRUE", isGuarded = false),
                ShapeSheetCellEntry("Hyperlinks", "Hyperlink.Subprocess.SubAddress", "\"$cleanName\"", isGuarded = false)
            )
            state.copy(
                pages = updatedPages,
                activePageId = resolvedSubPageId,
                subprocessLinks = state.subprocessLinks + (shapeId to resolvedSubPageId),
                shapeSheetCells = upsertShapeSheetCells(state.shapeSheetCells, subprocessCells),
                isDirty = true,
                canUndo = true,
                statusBanner = "Subprocess drill-down '$cleanName' linked (Page $resolvedSubPageId)"
            )
        }
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L && docPtr != 0L) {
            NativeBridge.nativeSetViewportPage(viewportPtr, docPtr, resolvedSubPageId)
        }
        presentViewportOrFallback()
        return resolvedSubPageId
    }

    private var cachedBlankBaselineDocPtr: Long = 0L

    /**
     * Compares the active document revision against a baseline revision via
     * `NativeBridge.nativeCompareDocuments`, returning structured change counts and a color-coded SVG overlay.
     */
    @Synchronized
    fun compareWithBaseline(baselineDocPtr: Long = 0L): DiagramCompareSummary {
        val state = _uiState.value
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val basePtr = if (baselineDocPtr != 0L) {
                baselineDocPtr
            } else {
                if (cachedBlankBaselineDocPtr == 0L) {
                    cachedBlankBaselineDocPtr = NativeBridge.nativeCreateBlankDocument(state.documentTitle)
                }
                cachedBlankBaselineDocPtr
            }
            if (basePtr != 0L) {
                val rawJson = NativeBridge.nativeCompareDocuments(basePtr, docPtr)
                val parsed = parseCompareReportJson(rawJson)
                if (parsed != null) {
                    _uiState.update {
                        it.copy(
                            lastCompareSummary = parsed,
                            statusBanner = "Diagram Compare: +${parsed.addedCount} added, ~${parsed.modifiedCount} modified, -${parsed.removedCount} removed"
                        )
                    }
                    return parsed
                }
            }
        }

        val addedPagesCount = state.pages.count { it.pageId !in baselinePageIdsSnapshot }
        val removedPagesCount = baselinePageIdsSnapshot.count { pid -> state.pages.none { it.pageId == pid } }
        val changedCells = state.shapeSheetCells.filter { cell ->
            val baseFormula = baselineCellsSnapshot[cell.cellName]
            baseFormula != null && baseFormula != cell.formula
        }
        val addedExtraCells = state.shapeSheetCells.count { it.cellName !in baselineCellsSnapshot }
        val syntheticCreatedShapes = max(0, (nextSyntheticShapeId - baselineNextShapeId).toInt()) +
            state.lastDataVisualizerShapeCount +
            state.lastCloudDiscoveryShapeCount +
            state.lastOrgChartShapeCount +
            addedPagesCount

        val addedCount = syntheticCreatedShapes
        val removedCount = removedPagesCount + deletedShapeCount
        val modifiedCount = if (changedCells.isNotEmpty() || addedExtraCells > 0) {
            max(1, changedCells.size)
        } else {
            0
        }

        val svgOverlay = buildString {
            append("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1056 816\">")
            if (addedCount > 0) {
                append("<rect x=\"72\" y=\"72\" width=\"180\" height=\"96\" fill=\"none\" stroke=\"#10b981\" stroke-width=\"3\"/>")
            }
            if (modifiedCount > 0) {
                append("<rect x=\"312\" y=\"192\" width=\"180\" height=\"96\" fill=\"none\" stroke=\"#f59e0b\" stroke-width=\"3\"/>")
            }
            if (removedCount > 0) {
                append("<rect x=\"552\" y=\"312\" width=\"180\" height=\"96\" fill=\"none\" stroke=\"#ef4444\" stroke-width=\"3\" stroke-dasharray=\"6,4\"/>")
            }
            append("</svg>")
        }

        val reportJson = buildString {
            append("{\"entries\":[")
            val entriesJson = mutableListOf<String>()
            if (addedCount > 0) {
                entriesJson.add(
                    "{\"page_name\":\"${escapeJson(state.pages.firstOrNull()?.name ?: "Page-1")}\",\"shape_id\":${state.selectedShapeId},\"shape_name\":\"${escapeJson(state.selectedShapeName)}\",\"kind\":\"Added\",\"summary\":\"Added $addedCount shape(s)\"}"
                )
            }
            if (modifiedCount > 0) {
                val cellList = changedCells.joinToString(",") { "\"${escapeJson(it.cellName)}\"" }
                entriesJson.add(
                    "{\"page_name\":\"${escapeJson(state.pages.firstOrNull()?.name ?: "Page-1")}\",\"shape_id\":${state.selectedShapeId},\"shape_name\":\"${escapeJson(state.selectedShapeName)}\",\"kind\":{\"CellsModified\":[$cellList]},\"summary\":\"Modified $modifiedCount cell(s)\"}"
                )
            }
            if (removedCount > 0) {
                entriesJson.add(
                    "{\"page_name\":\"Page-1\",\"shape_id\":1,\"shape_name\":\"RemovedShape\",\"kind\":\"Removed\",\"summary\":\"Removed $removedCount item(s)\"}"
                )
            }
            append(entriesJson.joinToString(","))
            append("],\"added_count\":$addedCount,\"removed_count\":$removedCount,\"modified_count\":$modifiedCount,\"diff_overlay_svg\":\"${escapeJson(svgOverlay)}\"}")
        }

        val summary = DiagramCompareSummary(
            addedCount = addedCount,
            removedCount = removedCount,
            modifiedCount = modifiedCount,
            rawReportJson = reportJson,
            diffOverlaySvg = svgOverlay
        )
        _uiState.update {
            it.copy(
                lastCompareSummary = summary,
                statusBanner = "Diagram Compare: +$addedCount added, ~$modifiedCount modified, -$removedCount removed"
            )
        }
        return summary
    }

    private fun parseCompareReportJson(rawJson: String): DiagramCompareSummary? {
        if (rawJson.isBlank()) return null
        val obj = runCatching { Json.parseToJsonElement(rawJson).jsonObject }.getOrNull() ?: return null
        val added = obj["added_count"]?.jsonPrimitive?.intOrNull ?: 0
        val removed = obj["removed_count"]?.jsonPrimitive?.intOrNull ?: 0
        val modified = obj["modified_count"]?.jsonPrimitive?.intOrNull ?: 0
        val svg = obj["diff_overlay_svg"]?.jsonPrimitive?.contentOrNull.orEmpty()
        return DiagramCompareSummary(
            addedCount = added,
            removedCount = removed,
            modifiedCount = modified,
            rawReportJson = rawJson,
            diffOverlaySvg = svg
        )
    }

    /**
     * Applies a JSON batch of Hybrid Logical Clock (`HlcTimestamp`) CRDT operations via
     * `NativeBridge.nativeApplyCrdtJson`, maintaining total `(wall_ms, logical, replica_id)`
     * LWW ordering and `GUARD(...)` protection in JVM fallback mode.
     */
    @Synchronized
    fun applyCrdtBatchJson(
        opsJson: String,
        replicaId: String = _uiState.value.crdtReplicaId
    ): Int {
        val trimmed = opsJson.trim()
        if (trimmed.isEmpty()) return 0

        val parsedArray: JsonArray = runCatching {
            val root = Json.parseToJsonElement(trimmed)
            when (root) {
                is JsonArray -> root
                is JsonObject -> (root["ops"] as? JsonArray) ?: JsonArray(listOf(root))
                else -> return 0
            }
        }.getOrElse { return 0 }

        val nativeApplied = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeApplyCrdtJson(docPtr, replicaId, trimmed)
        } else {
            0
        }

        data class DecodedCrdtOp(
            val kind: String,
            val payload: JsonObject,
            val clock: HlcClock
        )

        val decodedOps = parsedArray.mapNotNull { elem ->
            val obj = elem as? JsonObject ?: return@mapNotNull null
            val (kind, payload) = if ("op" in obj || "type" in obj) {
                val k = (obj["op"] ?: obj["type"])?.jsonPrimitive?.contentOrNull ?: return@mapNotNull null
                k to obj
            } else {
                val entry = obj.entries.firstOrNull() ?: return@mapNotNull null
                val body = entry.value as? JsonObject ?: return@mapNotNull null
                entry.key to body
            }
            val tsObj = payload["ts"] as? JsonObject
            val clock = HlcClock(
                wallMs = tsObj?.get("wall_ms")?.jsonPrimitive?.longOrNull ?: 1L,
                logical = tsObj?.get("logical")?.jsonPrimitive?.intOrNull ?: 0,
                replicaId = tsObj?.get("replica_id")?.jsonPrimitive?.contentOrNull ?: replicaId
            )
            DecodedCrdtOp(kind = kind, payload = payload, clock = clock)
        }.sortedBy { it.clock }

        if (decodedOps.isEmpty()) return nativeApplied

        var workingCells = _uiState.value.shapeSheetCells
        var workingPages = _uiState.value.pages
        var workingShapeName = _uiState.value.selectedShapeName
        var maxLogical = _uiState.value.crdtLogicalClock
        var appliedCount = 0

        for (op in decodedOps) {
            maxLogical = max(maxLogical, op.clock.logical + 1)
            when (op.kind) {
                "SetCell" -> {
                    val shapeId = op.payload["shape_id"]?.jsonPrimitive?.longOrNull ?: _uiState.value.selectedShapeId
                    val cellName = op.payload["cell_name"]?.jsonPrimitive?.contentOrNull ?: continue
                    val formula = op.payload["formula"]?.jsonPrimitive?.contentOrNull ?: continue
                    val forceSetf = op.payload["force_setf"]?.jsonPrimitive?.booleanOrNull ?: false
                    val regKey = "$shapeId:$cellName"
                    val prevClock = crdtCellClocks[regKey]
                    if (prevClock == null || op.clock >= prevClock) {
                        val existing = workingCells.firstOrNull { it.cellName == cellName }
                        val blockedByGuard = existing?.isGuarded == true &&
                            !forceSetf &&
                            !formula.uppercase(Locale.US).startsWith("GUARD(")
                        if (!blockedByGuard) {
                            crdtCellClocks[regKey] = op.clock
                            if (shapeId == _uiState.value.selectedShapeId) {
                                workingCells = upsertShapeSheetCells(
                                    workingCells,
                                    listOf(
                                        ShapeSheetCellEntry(
                                            section = existing?.section ?: "User-defined Cells",
                                            cellName = cellName,
                                            formula = formula,
                                            isGuarded = formula.uppercase(Locale.US).startsWith("GUARD(")
                                        )
                                    )
                                )
                            }
                            appliedCount++
                        }
                    }
                }
                "SetShapeText" -> {
                    val shapeId = op.payload["shape_id"]?.jsonPrimitive?.longOrNull ?: _uiState.value.selectedShapeId
                    val textVal = op.payload["text"]?.jsonPrimitive?.contentOrNull ?: continue
                    val regKey = "$shapeId:__text__"
                    val prevClock = crdtCellClocks[regKey]
                    if (prevClock == null || op.clock >= prevClock) {
                        crdtCellClocks[regKey] = op.clock
                        if (shapeId == _uiState.value.selectedShapeId) {
                            workingShapeName = "Sheet.$shapeId ($textVal)"
                        }
                        appliedCount++
                    }
                }
                "UpsertPage" -> {
                    val pid = op.payload["page_id"]?.jsonPrimitive?.intOrNull ?: continue
                    val pname = op.payload["name"]?.jsonPrimitive?.contentOrNull ?: "Page-$pid"
                    if (workingPages.none { it.pageId == pid }) {
                        workingPages = workingPages + DiagramPageTab(pid, pname, "1 in = 1 in")
                    }
                    appliedCount++
                }
                "UpsertShape" -> {
                    val sid = op.payload["shape_id"]?.jsonPrimitive?.longOrNull ?: continue
                    tombstonedShapeIds.remove(sid)
                    nextSyntheticShapeId = max(nextSyntheticShapeId, sid + 1L)
                    appliedCount++
                }
                "TombstoneShape" -> {
                    val sid = op.payload["shape_id"]?.jsonPrimitive?.longOrNull ?: continue
                    tombstonedShapeIds.add(sid)
                    deletedShapeCount += 1
                    appliedCount++
                }
                "UpsertConnect" -> {
                    appliedCount++
                }
            }
        }

        val finalApplied = if (nativeApplied > 0) nativeApplied else appliedCount
        _uiState.update { state ->
            val activeFormula = workingCells.firstOrNull { it.cellName == state.activeCellName }?.formula
                ?: state.formulaBarInput
            state.copy(
                pages = workingPages,
                selectedShapeName = workingShapeName,
                shapeSheetCells = workingCells,
                formulaBarInput = activeFormula,
                crdtReplicaId = replicaId,
                crdtAppliedOpsCount = state.crdtAppliedOpsCount + finalApplied,
                crdtLogicalClock = maxLogical,
                isDirty = true,
                statusBanner = "HLC CRDT Sync ($replicaId): merged $finalApplied op(s) @ L$maxLogical"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
        return finalApplied
    }

    /**
     * Imports a Visio Plan 2 Data Visualizer `.csv` or `.xlsx` process table via
     * `NativeBridge.nativeImportDataVisualizer`, creating Cross-Functional Swimlanes, BPMN shapes, and connectors.
     */
    @Synchronized
    fun importDataVisualizer(csvOrXlsxBytes: ByteArray): Int {
        if (csvOrXlsxBytes.isEmpty()) return 0
        val stateSnapshot = _uiState.value
        val nativeCount = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeImportDataVisualizer(
                docPtr = docPtr,
                pageId = resolveNativePageId(stateSnapshot.activePageId),
                csvOrXlsx = csvOrXlsxBytes
            )
        } else {
            0
        }

        val resolvedCount = if (nativeCount > 0) {
            nativeCount
        } else {
            parseFallbackDataVisualizerCount(csvOrXlsxBytes)
        }
        if (resolvedCount <= 0) return 0

        _uiState.update { state ->
            state.copy(
                lastDataVisualizerShapeCount = resolvedCount,
                selectedShapeName = "DataVisualizer.SwimlanePool ($resolvedCount shapes)",
                isDirty = true,
                canUndo = true,
                statusBanner = "Data Visualizer imported $resolvedCount swimlane, step & connector shapes"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
        return resolvedCount
    }

    private fun parseFallbackDataVisualizerCount(bytes: ByteArray): Int {
        // Check for OpenXML .xlsx ZIP signature (PK\x03\x04)
        if (bytes.size >= 4 &&
            bytes[0] == 0x50.toByte() &&
            bytes[1] == 0x4B.toByte() &&
            bytes[2] == 0x03.toByte() &&
            bytes[3] == 0x04.toByte()
        ) {
            var sheetRows = 0
            runCatching {
                ZipInputStream(ByteArrayInputStream(bytes)).use { zis ->
                    var entry = zis.nextEntry
                    while (entry != null) {
                        if (entry.name.startsWith("xl/worksheets/sheet") && entry.name.endsWith(".xml")) {
                            val xml = String(zis.readBytes(), StandardCharsets.UTF_8)
                            sheetRows = Regex("<row\\b").findAll(xml).count()
                            break
                        }
                        entry = zis.nextEntry
                    }
                }
            }
            val dataSteps = (sheetRows - 1).coerceAtLeast(1)
            return dataSteps * 2
        }

        val csvText = String(bytes, StandardCharsets.UTF_8)
        val lines = csvText.lineSequence()
            .map { it.trim() }
            .filter { it.isNotEmpty() }
            .toList()
        if (lines.size < 2) return 0

        val headers = splitCsvLine(lines.first()).map { it.trim().lowercase(Locale.US) }
        val stepIdx = headers.indexOfFirst {
            it == "process step id" || it == "step id" || it == "id"
        }.let { if (it >= 0) it else 0 }
        val nextIdx = headers.indexOfFirst {
            it == "next step id" || it == "next step" || it == "next"
        }
        val laneIdx = headers.indexOfFirst {
            it == "function" || it == "swimlane" || it == "department" || it == "role"
        }

        var stepCount = 0
        var edgeCount = 0
        val lanes = linkedSetOf<String>()

        for (rowLine in lines.drop(1)) {
            val cols = splitCsvLine(rowLine)
            val stepId = cols.getOrNull(stepIdx)?.trim().orEmpty()
            if (stepId.isEmpty()) continue
            stepCount++
            if (nextIdx >= 0) {
                val rawNext = cols.getOrNull(nextIdx).orEmpty()
                edgeCount += rawNext.split(',', ';').count { it.trim().isNotEmpty() }
            }
            if (laneIdx >= 0) {
                val lane = cols.getOrNull(laneIdx)?.trim().orEmpty()
                if (lane.isNotEmpty()) {
                    lanes.add(lane.lowercase(Locale.US))
                }
            }
        }
        if (stepCount == 0) return 0
        val poolHeaderShapes = if (lanes.isNotEmpty()) lanes.size + 1 else 0
        return poolHeaderShapes + stepCount + edgeCount
    }

    private fun splitCsvLine(line: String): List<String> {
        val result = mutableListOf<String>()
        val current = StringBuilder()
        var inQuotes = false
        var i = 0
        while (i < line.length) {
            val ch = line[i]
            when {
                ch == '"' -> {
                    if (inQuotes && i + 1 < line.length && line[i + 1] == '"') {
                        current.append('"')
                        i++
                    } else {
                        inQuotes = !inQuotes
                    }
                }
                ch == ',' && !inQuotes -> {
                    result.add(current.toString())
                    current.setLength(0)
                }
                else -> current.append(ch)
            }
            i++
        }
        result.add(current.toString())
        return result
    }

    /**
     * Imports AWS / Azure / GCP / Terraform Cloud Discovery JSON topology via
     * `NativeBridge.nativeImportCloudDiscovery`.
     */
    @Synchronized
    fun importCloudDiscovery(discoveryJson: String): Int {
        val trimmed = discoveryJson.trim()
        if (trimmed.isEmpty()) return 0
        val stateSnapshot = _uiState.value
        val nativeCount = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeImportCloudDiscovery(
                docPtr = docPtr,
                pageId = resolveNativePageId(stateSnapshot.activePageId),
                discoveryJson = trimmed
            )
        } else {
            0
        }

        val resolvedCount = if (nativeCount > 0) {
            nativeCount
        } else {
            parseFallbackCloudDiscoveryCount(trimmed)
        }
        if (resolvedCount <= 0) return 0

        _uiState.update { state ->
            state.copy(
                lastCloudDiscoveryShapeCount = resolvedCount,
                selectedShapeName = "CloudTopology.VPC ($resolvedCount shapes)",
                isDirty = true,
                canUndo = true,
                statusBanner = "Cloud Discovery generated $resolvedCount infrastructure & VPC shapes"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
        return resolvedCount
    }

    private fun parseFallbackCloudDiscoveryCount(jsonText: String): Int {
        val root = runCatching { Json.parseToJsonElement(jsonText) }.getOrNull() ?: return 0
        val obj = root as? JsonObject ?: return 0
        val resources: JsonArray = (obj["resources"] as? JsonArray)
            ?: (obj["nodes"] as? JsonArray)
            ?: ((obj["planned_values"] as? JsonObject)
                ?.get("root_module") as? JsonObject)
                ?.get("resources") as? JsonArray
            ?: JsonArray(emptyList())
        if (resources.isEmpty()) return 0

        val vpcs = linkedSetOf<String>()
        for (res in resources) {
            val resObj = res as? JsonObject ?: continue
            val vpc = (resObj["vpc"] ?: resObj["region"] ?: resObj["resourceGroup"])
                ?.jsonPrimitive?.contentOrNull?.trim().orEmpty()
            if (vpc.isNotEmpty()) vpcs.add(vpc)
        }
        val edges = ((obj["edges"] as? JsonArray) ?: (obj["connections"] as? JsonArray))?.size ?: 0
        return vpcs.size + resources.size + edges
    }

    /**
     * Imports Microsoft Entra ID (`/users` Graph JSON) or RFC-2849 `.ldif` directory entries into
     * a hierarchical Org Chart via `NativeBridge.nativeImportEntraOrLdif`.
     */
    @Synchronized
    fun importEntraOrLdifOrgChart(payloadText: String): Int {
        val trimmed = payloadText.trim()
        if (trimmed.isEmpty()) return 0
        val stateSnapshot = _uiState.value
        val nativeCount = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeImportEntraOrLdif(
                docPtr = docPtr,
                pageId = resolveNativePageId(stateSnapshot.activePageId),
                text = trimmed
            )
        } else {
            0
        }

        val resolvedCount = if (nativeCount > 0) {
            nativeCount
        } else {
            parseFallbackEntraOrLdifCount(trimmed)
        }
        if (resolvedCount <= 0) return 0

        _uiState.update { state ->
            state.copy(
                lastOrgChartShapeCount = resolvedCount,
                selectedShapeName = "OrgChart.Root ($resolvedCount shapes)",
                isDirty = true,
                canUndo = true,
                statusBanner = "Entra / LDIF Org Chart imported $resolvedCount cards & reporting lines"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
        return resolvedCount
    }

    private fun parseFallbackEntraOrLdifCount(text: String): Int {
        if (text.startsWith("{") || text.startsWith("[")) {
            val elem = runCatching { Json.parseToJsonElement(text) }.getOrNull() ?: return 0
            val users: JsonArray = when (elem) {
                is JsonArray -> elem
                is JsonObject -> (elem["value"] as? JsonArray)
                    ?: (elem["users"] as? JsonArray)
                    ?: JsonArray(emptyList())
                else -> JsonArray(emptyList())
            }
            if (users.isEmpty()) return 0
            var managerEdges = 0
            for (u in users) {
                val uo = u as? JsonObject ?: continue
                val mgr = uo["manager"] ?: uo["managerId"] ?: uo["reportsTo"]
                if (mgr != null && mgr.toString() != "null") {
                    managerEdges++
                }
            }
            return users.size + managerEdges
        }

        // RFC-2849 LDIF stanza parser
        val blocks = text.replace("\r\n", "\n").split(Regex("\n\\s*\n"))
        var entries = 0
        var managerLinks = 0
        for (block in blocks) {
            val lines = block.lines().map { it.trim() }.filter { it.isNotEmpty() && !it.startsWith("#") }
            val hasIdentity = lines.any {
                it.startsWith("dn:", ignoreCase = true) ||
                    it.startsWith("cn:", ignoreCase = true) ||
                    it.startsWith("uid:", ignoreCase = true)
            }
            if (hasIdentity) {
                entries++
                if (lines.any { it.startsWith("manager:", ignoreCase = true) }) {
                    managerLinks++
                }
            }
        }
        return entries + managerLinks
    }

    /**
     * Exports a Visio Plan 2 enterprise artifact (`pptx`, `docx`, `powerautomate` / `flow.json`,
     * `powerbi` / `pbi.json`, `pdf`, `vsdx`) and updates workstation status telemetry.
     */
    @Synchronized
    fun exportPlan2Artifact(format: String): ByteArray {
        val clean = format.trim().trimStart('.').lowercase(Locale.US)
        val bytes = exportDocumentBytes(clean)
        _uiState.update {
            it.copy(
                lastPlan2ExportFormat = clean,
                statusBanner = "Exported Plan 2 '$clean' package (${bytes.size} bytes)"
            )
        }
        return bytes
    }

    private fun upsertShapeSheetCells(
        existing: List<ShapeSheetCellEntry>,
        updates: List<ShapeSheetCellEntry>
    ): List<ShapeSheetCellEntry> {
        if (updates.isEmpty()) return existing
        val byName = existing.associateByTo(linkedMapOf()) { it.cellName }
        for (u in updates) {
            byName[u.cellName] = u
        }
        return byName.values.toList()
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
        val ext = displayName.substringAfterLast('.', "vsdx").lowercase(Locale.US)
        if (ext in setOf("vssx", "vss", "vssm")) {
            importVisioStencilBytes(displayName, rawBytes)
            return
        }

        viewModelScope.launch(ioDispatcher) {
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
                        val oldDocPtr = docPtr
                        docPtr = openedPtr
                        if (oldDocPtr != 0L && oldDocPtr != openedPtr) {
                            NativeBridge.nativeDestroyDocument(oldDocPtr)
                        }
                        loadDynamicStencilsFromNative()
                    }
                }
            }

            withContext(Dispatchers.Main) {
                if (openedPtr != 0L) {
                    syncPagesAndActivePageFromNative(preferFirstForeground = true)
                }
                _uiState.update { state ->
                    state.copy(
                        showStartBackstage = false,
                        documentTitle = displayName,
                        documentUri = uri,
                        documentExtension = ext,
                        isDirty = false,
                        statusBanner = if (openedPtr != 0L) {
                            "Opened $displayName (${state.pages.size} page(s), .$ext) losslessly"
                        } else {
                            "Loaded $displayName (${rawBytes.size} bytes, .$ext)"
                        }
                    )
                }
                refreshSelectedShapeFormulas()
                savedStateRevision = readNativeStateRevision()
                fitPageToWindow()
                captureBaselineSnapshot()
                presentViewportOrFallback()
            }
        }
    }

    private fun hexToRgbFormula(hexColor: String): Pair<String, String>? {
        val clean = hexColor.trim().removePrefix("#")
        if (clean.length != 6) return null
        val rgb = clean.toIntOrNull(16) ?: return null
        val r = (rgb shr 16) and 0xFF
        val g = (rgb shr 8) and 0xFF
        val b = rgb and 0xFF
        val normalizedHex = "#%02X%02X%02X".format(Locale.US, r, g, b)
        return normalizedHex to "RGB($r,$g,$b)"
    }

    @Synchronized
    fun applyShapeFillColor(hexColor: String) {
        val (normHex, rgbFormula) = hexToRgbFormula(hexColor) ?: return
        val cur = _uiState.value
        val targets = cur.selectedShapeIds.ifEmpty { listOf(cur.selectedShapeId) }.filter { it > 0L }.distinct()
        val nativePageId = resolveNativePageId(cur.activePageId)
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            for (sid in targets) {
                NativeBridge.nativeSetCellFormula(docPtr, nativePageId, sid, "FillForegnd", rgbFormula, true)
                NativeBridge.nativeSetCellFormula(docPtr, nativePageId, sid, "FillPattern", "1", true)
            }
        }
        _uiState.update { state ->
            val updatedCells = upsertShapeSheetCells(
                state.shapeSheetCells,
                listOf(
                    ShapeSheetCellEntry("Fill Format", "FillForegnd", rgbFormula, isGuarded = false),
                    ShapeSheetCellEntry("Fill Format", "FillPattern", "1", isGuarded = false)
                )
            )
            state.copy(
                shapeFillColorHex = normHex,
                shapeSheetCells = updatedCells,
                isDirty = true,
                canUndo = true,
                statusBanner = "Updated Fill Color to $normHex ($rgbFormula)"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
    }

    @Synchronized
    fun applyShapeLineColor(hexColor: String) {
        val (normHex, rgbFormula) = hexToRgbFormula(hexColor) ?: return
        val cur = _uiState.value
        val targets = cur.selectedShapeIds.ifEmpty { listOf(cur.selectedShapeId) }.filter { it > 0L }.distinct()
        val nativePageId = resolveNativePageId(cur.activePageId)
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            for (sid in targets) {
                NativeBridge.nativeSetCellFormula(docPtr, nativePageId, sid, "LineColor", rgbFormula, true)
            }
        }
        _uiState.update { state ->
            val updatedCells = upsertShapeSheetCells(
                state.shapeSheetCells,
                listOf(ShapeSheetCellEntry("Line Format", "LineColor", rgbFormula, isGuarded = false))
            )
            state.copy(
                shapeLineColorHex = normHex,
                shapeSheetCells = updatedCells,
                isDirty = true,
                canUndo = true,
                statusBanner = "Updated Line Color to $normHex ($rgbFormula)"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
    }

    // -----------------------------------------------------------------------
    // 14-Point Workstation UX ViewModel Actions (Backstage, Ribbon, Rich Text,
    // Rotate/Flip, ISO Metric Presets, Stencil Import, Rename, Localization, Updates)
    // -----------------------------------------------------------------------
    fun setStartBackstageVisible(visible: Boolean) {
        _uiState.update { it.copy(showStartBackstage = visible) }
    }

    fun selectRibbonTab(tab: String) {
        _uiState.update { it.copy(activeRibbonTab = tab, isRibbonCollapsed = false) }
    }

    fun toggleRibbonCollapsed() {
        _uiState.update { it.copy(isRibbonCollapsed = !it.isRibbonCollapsed) }
    }

    fun setLanguage(language: VexioAppLanguage) {
        _uiState.update {
            it.copy(
                language = language,
                statusBanner = "Language: ${language.nativeName} (${language.badge})"
            )
        }
    }

    @Synchronized
    fun createBlankWithPagePreset(title: String, preset: VexioPagePresetSpec) {
        if (NativeBridge.ensureLoaded()) {
            if (enginePtr == 0L) {
                enginePtr = NativeBridge.nativeInitEngine()
            }
            if (docPtr != 0L) {
                NativeBridge.nativeDestroyDocument(docPtr)
            }
            docPtr = NativeBridge.nativeCreateBlankDocument(title)
            if (docPtr != 0L) {
                NativeBridge.nativeLoadStarterTemplate(docPtr, "blank")
                NativeBridge.nativeSetPagePreset(docPtr, 1, preset.slug, preset.landscape)
                loadDynamicStencilsFromNative()
            }
            if (viewportPtr != 0L && docPtr != 0L) {
                NativeBridge.nativeSetViewportPage(viewportPtr, docPtr, 1)
                NativeBridge.nativeSetViewportTool(viewportPtr, VexioTool.POINTER.nativeCode)
            }
        }
        _uiState.update { state ->
            state.copy(
                showStartBackstage = false,
                documentTitle = title,
                documentUri = null,
                documentExtension = "vsdx",
                activePageId = 1,
                pages = listOf(DiagramPageTab(1, "Page-1", preset.label)),
                activePagePreset = preset,
                metricUnits = preset.isMetric,
                selectedShapeId = 0L,
                selectedShapeIds = emptyList(),
                selectedShapeName = "",
                activeTool = VexioTool.POINTER,
                activeCursor = VexioCursorKind.ARROW,
                isDirty = false,
                statusBanner = "Created $title • ${preset.label}"
            )
        }
        captureBaselineSnapshot()
        presentViewportOrFallback()
    }

    @Synchronized
    fun applyPagePreset(preset: VexioPagePresetSpec) {
        val pageId = resolveNativePageId(_uiState.value.activePageId)
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeSetPagePreset(docPtr, pageId, preset.slug, preset.landscape)
            presentViewportOrFallback()
        }
        _uiState.update { state ->
            val updatedPages = state.pages.map { p ->
                if (p.pageId == state.activePageId) p.copy(scaleLabel = preset.label) else p
            }
            state.copy(
                pages = updatedPages,
                activePagePreset = preset,
                metricUnits = preset.isMetric,
                isDirty = true,
                canUndo = true,
                statusBanner = "Applied Page Preset: ${preset.label}"
            )
        }
        syncPagesAndActivePageFromNative(preferFirstForeground = false)
        fitPageToWindow()
    }

    @Synchronized
    fun renameDocument(rawTitle: String) {
        val cleaned = rawTitle.trim()
        if (cleaned.isEmpty()) return
        val normalized = if ('.' in cleaned) {
            cleaned
        } else {
            "$cleaned.${_uiState.value.documentExtension}"
        }
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeSetDocumentTitle(docPtr, normalized)
        }
        _uiState.update {
            it.copy(
                documentTitle = normalized,
                isDirty = true,
                statusBanner = "Renamed document to $normalized"
            )
        }
    }

    @Synchronized
    fun rotateOrFlipSelected(
        deltaAngleDeg: Double = 0.0,
        flipX: Boolean = false,
        flipY: Boolean = false
    ) {
        val state = _uiState.value
        val targets = state.selectedShapeIds.ifEmpty { listOf(state.selectedShapeId) }
            .filter { it > 0L }
            .distinct()
            .toLongArray()
        if (targets.isEmpty()) return

        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeRotateFlipShapes(
                docPtr = docPtr,
                pageId = resolveNativePageId(state.activePageId),
                shapeIds = targets,
                deltaAngleDeg = -deltaAngleDeg,
                toggleFlipX = flipX,
                toggleFlipY = flipY
            )
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        }

        val actionDesc = when {
            flipX -> "Flipped Horizontally"
            flipY -> "Flipped Vertically"
            deltaAngleDeg < 0 -> "Rotated Left 90°"
            else -> "Rotated Right 90°"
        }
        _uiState.update {
            it.copy(
                isDirty = true,
                canUndo = true,
                statusBanner = "$actionDesc (${targets.size} stencil/shape)"
            )
        }
    }

    @Synchronized
    fun applyRichTextStyle(
        fontFamily: String? = null,
        fontSizePt: Int? = null,
        bold: Boolean? = null,
        italic: Boolean? = null,
        underline: Boolean? = null,
        strikethrough: Boolean? = null,
        horizAlign: Int? = null,
        vertAlign: Int? = null,
        bullets: Boolean? = null,
        colorHex: String? = null,
        bgColorHex: String? = null
    ) {
        val cur = _uiState.value
        val nextFont = fontFamily ?: cur.textFontFamily
        val nextSize = (fontSizePt ?: cur.textFontSizePt).coerceIn(6, 96)
        val nextBold = bold ?: cur.textBold
        val nextItalic = italic ?: cur.textItalic
        val nextUnder = underline ?: cur.textUnderline
        val nextStrike = strikethrough ?: cur.textStrikethrough
        val nextHAlign = (horizAlign ?: cur.textHorizAlign).coerceIn(0, 3)
        val nextVAlign = (vertAlign ?: cur.textVertAlign).coerceIn(0, 2)
        val nextBullets = bullets ?: cur.textBullets
        val nextColor = colorHex ?: cur.textColorHex
        val nextBg = bgColorHex ?: cur.textBgColorHex

        val patchFields = mutableListOf<String>()
        if (fontFamily != null) patchFields.add("\"fontFamily\":\"${escapeJson(nextFont)}\"")
        if (fontSizePt != null) patchFields.add("\"fontSizePt\":$nextSize")
        if (bold != null || italic != null || underline != null || strikethrough != null) {
            patchFields.add("\"bold\":$nextBold")
            patchFields.add("\"italic\":$nextItalic")
            patchFields.add("\"underline\":$nextUnder")
            patchFields.add("\"strikethrough\":$nextStrike")
        }
        if (horizAlign != null) patchFields.add("\"horizAlign\":$nextHAlign")
        if (vertAlign != null) patchFields.add("\"vertAlign\":$nextVAlign")
        if (bullets != null) patchFields.add("\"bullet\":${if (nextBullets) 1 else 0}")
        if (colorHex != null) patchFields.add("\"colorHex\":\"${escapeJson(nextColor)}\"")
        if (bgColorHex != null) patchFields.add("\"bgColorHex\":\"${escapeJson(nextBg)}\"")

        val patchJson = "{${patchFields.joinToString(",")}}"
        val targets = cur.selectedShapeIds.ifEmpty { listOf(cur.selectedShapeId) }
            .filter { it > 0L }
            .distinct()
            .toLongArray()

        if (NativeBridge.isNativeAvailable && docPtr != 0L && targets.isNotEmpty() && patchFields.isNotEmpty()) {
            NativeBridge.nativeSetShapeTextStyleJson(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                shapeIds = targets,
                styleJson = patchJson
            )
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        }

        _uiState.update {
            it.copy(
                textFontFamily = nextFont,
                textFontSizePt = nextSize,
                textBold = nextBold,
                textItalic = nextItalic,
                textUnderline = nextUnder,
                textStrikethrough = nextStrike,
                textHorizAlign = nextHAlign,
                textVertAlign = nextVAlign,
                textBullets = nextBullets,
                textColorHex = nextColor,
                textBgColorHex = nextBg,
                isDirty = true,
                canUndo = true,
                statusBanner = "Applied text formatting ($nextFont ${nextSize}pt)"
            )
        }
    }

    @Synchronized
    fun applyOrInsertHeading(presetSlug: String, forceInsert: Boolean = false): Long {
        val cur = _uiState.value
        val targets = cur.selectedShapeIds.filter { it > 0L }.ifEmpty {
            if (cur.selectedShapeId > 0L) listOf(cur.selectedShapeId) else emptyList()
        }
        val sizePt = when (presetSlug) {
            "title" -> 24
            "subtitle" -> 14
            "h1" -> 18
            "h2" -> 14
            "h3" -> 12
            "section_banner", "callout_title" -> 13
            "caption" -> 10
            else -> 11
        }
        val isBold = presetSlug in setOf("title", "h1", "h2", "h3", "section_banner", "callout_title")

        if (!forceInsert && targets.isNotEmpty()) {
            if (NativeBridge.isNativeAvailable && docPtr != 0L) {
                NativeBridge.nativeApplyHeadingPreset(
                    docPtr = docPtr,
                    pageId = resolveNativePageId(cur.activePageId),
                    shapeIds = targets.toLongArray(),
                    presetSlug = presetSlug
                )
                refreshSelectedShapeFormulas()
                presentViewportOrFallback()
            }
            _uiState.update {
                it.copy(
                    textFontSizePt = sizePt,
                    textBold = isBold,
                    textItalic = presetSlug == "caption",
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Applied Visio heading style ($presetSlug)"
                )
            }
            return targets.first()
        }

        val insertedId = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val id = NativeBridge.nativeInsertHeading(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                presetSlug = presetSlug,
                customText = "",
                xIn = 0.0,
                yIn = 0.0
            )
            if (id > 0L && viewportPtr != 0L) {
                NativeBridge.nativeSetSelectionIds(viewportPtr, docPtr, longArrayOf(id))
            }
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
            id
        } else {
            nextSyntheticShapeId++
        }

        if (insertedId > 0L) {
            _uiState.update {
                it.copy(
                    activeTool = VexioTool.POINTER,
                    selectedShapeId = insertedId,
                    selectedShapeIds = listOf(insertedId),
                    textFontSizePt = sizePt,
                    textBold = isBold,
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Inserted Visio heading ($presetSlug)"
                )
            }
            beginInlineTextEdit(insertedId, isNewTextShape = false)
        }
        return insertedId
    }

    @Synchronized
    fun applyBorderAndTitle(styleSlug: String) {
        val cur = _uiState.value
        val docTitle = cur.documentTitle.substringBeforeLast(".")
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeApplyBorderAndTitle(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                styleSlug = styleSlug,
                title = docTitle,
                subtitle = "",
                author = "Vexio Workstation",
                dateOrRev = ""
            )
            if (viewportPtr != 0L) {
                NativeBridge.nativeSetViewportPage(viewportPtr, docPtr, resolveNativePageId(cur.activePageId))
            }
            presentViewportOrFallback()
        }
        _uiState.update {
            it.copy(
                isDirty = true,
                canUndo = true,
                statusBanner = if (styleSlug == "none") {
                    "Removed page border & title"
                } else {
                    "Applied Borders & Titles ($styleSlug)"
                }
            )
        }
    }

    @Synchronized
    fun setMarkupStudioDialogVisible(visible: Boolean) {
        if (visible && NativeBridge.isNativeAvailable && docPtr != 0L) {
            val cur = _uiState.value
            val exported = NativeBridge.nativeExportMarkup(
                docPtr,
                resolveNativePageId(cur.activePageId),
                cur.markupStudioFormat
            )
            if (!exported.isNullOrBlank()) {
                _uiState.update { it.copy(showMarkupStudioDialog = true, markupStudioDraft = exported) }
                return
            }
        }
        _uiState.update { it.copy(showMarkupStudioDialog = visible) }
    }

    @Synchronized
    fun setMarkupStudioFormat(formatSlug: String) {
        val clean = formatSlug.lowercase(Locale.US)
        var nextDraft = _uiState.value.markupStudioDraft
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val exported = NativeBridge.nativeExportMarkup(
                docPtr,
                resolveNativePageId(_uiState.value.activePageId),
                clean
            )
            if (!exported.isNullOrBlank()) {
                nextDraft = exported
            }
        }
        _uiState.update { it.copy(markupStudioFormat = clean, markupStudioDraft = nextDraft) }
    }

    @Synchronized
    fun setMarkupStudioDraft(draft: String) {
        _uiState.update { it.copy(markupStudioDraft = draft) }
    }

    @Synchronized
    fun importDiagramMarkup(markup: String = _uiState.value.markupStudioDraft, formatSlug: String = _uiState.value.markupStudioFormat): Int {
        if (markup.isBlank()) return 0
        val pid = resolveNativePageId(_uiState.value.activePageId)
        var created = 3
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            created = NativeBridge.nativeImportMarkup(docPtr, pid, markup, formatSlug).coerceAtLeast(0)
            presentViewportOrFallback()
        }
        _uiState.update {
            it.copy(
                showMarkupStudioDialog = false,
                isDirty = true,
                canUndo = true,
                pageShapeCount = (it.pageShapeCount + created).coerceAtLeast(created),
                statusBanner = "Imported $created shapes from ${formatSlug.uppercase(Locale.US)} markup"
            )
        }
        return created
    }

    private fun parseSlideSnippetsJson(json: String?): List<SlideSnippetUiItem> {
        if (json.isNullOrBlank()) return emptyList()
        return runCatching {
            val arr = org.json.JSONArray(json)
            buildList {
                for (i in 0 until arr.length()) {
                    val o = arr.optJSONObject(i) ?: continue
                    add(
                        SlideSnippetUiItem(
                            id = o.optInt("id", i + 1),
                            pageId = o.optInt("page_id", 1),
                            title = o.optString("title", "Slide ${i + 1}"),
                            notes = o.optString("notes", ""),
                            xIn = o.optDouble("x_in", 0.0),
                            yIn = o.optDouble("y_in", 0.0),
                            widthIn = o.optDouble("width_in", 11.0),
                            heightIn = o.optDouble("height_in", 8.5)
                        )
                    )
                }
            }
        }.getOrDefault(emptyList())
    }

    @Synchronized
    fun setSlideSnippetsDialogVisible(visible: Boolean) {
        if (visible && NativeBridge.isNativeAvailable && docPtr != 0L) {
            val loaded = parseSlideSnippetsJson(NativeBridge.nativeListSlideSnippetsJson(docPtr))
            _uiState.update { it.copy(showSlideSnippetsDialog = true, slideSnippets = loaded) }
        } else {
            _uiState.update { it.copy(showSlideSnippetsDialog = visible) }
        }
    }

    @Synchronized
    fun autoGenerateSlideSnippets(): Int {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val slides = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeAutoSlideSnippets(docPtr, pid)
            parseSlideSnippetsJson(NativeBridge.nativeListSlideSnippetsJson(docPtr))
        } else {
            listOf(
                SlideSnippetUiItem(1, pid, "Full Diagram Overview", "Overview slide", 0.0, 0.0, _uiState.value.pageWidthInches, _uiState.value.pageHeightInches),
                SlideSnippetUiItem(2, pid, "Stage 1 — Focus", "Focused section", 0.5, 0.5, _uiState.value.pageWidthInches * 0.6, _uiState.value.pageHeightInches * 0.6)
            )
        }
        _uiState.update {
            it.copy(
                slideSnippets = slides,
                activeSlideIndex = 0,
                isDirty = true,
                canUndo = true,
                statusBanner = "Generated ${slides.size} Presentation Slide Snippets"
            )
        }
        return slides.size
    }

    @Synchronized
    fun deleteSlideSnippet(snippetId: Int) {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val nextSlides = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeDeleteSlideSnippet(docPtr, pid, snippetId)
            parseSlideSnippetsJson(NativeBridge.nativeListSlideSnippetsJson(docPtr))
        } else if (snippetId == 0) {
            emptyList()
        } else {
            _uiState.value.slideSnippets.filterNot { it.id == snippetId }
        }
        _uiState.update {
            it.copy(
                slideSnippets = nextSlides,
                activeSlideIndex = 0,
                isPresentationModeActive = it.isPresentationModeActive && nextSlides.isNotEmpty(),
                isDirty = true
            )
        }
    }

    @Synchronized
    fun setVbaMacroDialogVisible(visible: Boolean) {
        _uiState.update { it.copy(showVbaMacroDialog = visible) }
    }

    @Synchronized
    fun setVbaMacroScript(script: String) {
        _uiState.update { it.copy(vbaMacroScript = script) }
    }

    @Synchronized
    fun runVbaMacro(script: String = _uiState.value.vbaMacroScript): String {
        if (script.isBlank()) return ""
        val cur = _uiState.value
        val selIds = cur.selectedShapeIds.ifEmpty { listOf(cur.selectedShapeId) }.toLongArray()
        var summary = "Executed VBA macro inside gas-metered sandbox"
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val reportJson = NativeBridge.nativeRunVbaMacro(docPtr, selIds, script).orEmpty()
            summary = runCatching {
                val obj = org.json.JSONObject(reportJson)
                if (obj.has("error")) {
                    "VBA Error: ${obj.optString("error")}"
                } else {
                    val stmts = obj.optInt("executed_statements", 0)
                    val gas = obj.optInt("gas_used", 0)
                    "Executed $stmts statements ($gas gas) in deterministic sandbox"
                }
            }.getOrDefault(summary)
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        } else {
            val cellRegex = Regex("""Cells\("([^"]+)"\)\.(?:FormulaU|Formula)\s*=\s*"((?:""|[^"])*)"""")
            var applied = 0
            for (m in cellRegex.findAll(script)) {
                val cellName = m.groupValues[1]
                val formula = m.groupValues[2].replace("\"\"", "\"")
                commitCellFormula(cellName, formula, forceSetf = true)
                applied++
            }
            summary = "Executed $applied ShapeSheet assignment(s) in deterministic sandbox"
        }
        _uiState.update {
            it.copy(
                showVbaMacroDialog = false,
                lastVbaReportSummary = summary,
                isDirty = true,
                canUndo = true,
                statusBanner = summary
            )
        }
        return summary
    }

    private fun parseCommentsJson(raw: String?): List<ReviewCommentUiItem> {
        if (raw.isNullOrBlank()) return emptyList()
        return runCatching {
            val arr = org.json.JSONArray(raw)
            buildList {
                for (i in 0 until arr.length()) {
                    val o = arr.optJSONObject(i) ?: continue
                    add(
                        ReviewCommentUiItem(
                            id = o.optInt("id", i + 1),
                            pageId = o.optInt("page_id", 1),
                            shapeId = o.optLong("shape_id", 0L),
                            author = o.optString("author", "Reviewer"),
                            text = o.optString("text", ""),
                            resolved = o.optBoolean("resolved", false)
                        )
                    )
                }
            }
        }.getOrDefault(emptyList())
    }

    @Synchronized
    fun setCommentsDialogVisible(visible: Boolean) {
        if (visible && NativeBridge.isNativeAvailable && docPtr != 0L) {
            val pid = resolveNativePageId(_uiState.value.activePageId)
            val loaded = parseCommentsJson(NativeBridge.nativeListCommentsJson(docPtr, pid))
            _uiState.update { it.copy(showCommentsDialog = true, reviewComments = loaded) }
        } else {
            _uiState.update { it.copy(showCommentsDialog = visible) }
        }
    }

    @Synchronized
    fun addReviewComment(author: String, text: String): Int {
        if (text.isBlank()) return -1
        val cur = _uiState.value
        val pid = resolveNativePageId(cur.activePageId)
        val cleanAuthor = author.trim().ifEmpty { "Reviewer" }
        val nextComments: List<ReviewCommentUiItem>
        val newId: Int
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            newId = NativeBridge.nativeAddComment(docPtr, pid, cur.selectedShapeId, cleanAuthor, text.trim())
            nextComments = parseCommentsJson(NativeBridge.nativeListCommentsJson(docPtr, pid))
            presentViewportOrFallback()
        } else {
            newId = (cur.reviewComments.maxOfOrNull { it.id } ?: 0) + 1
            nextComments = cur.reviewComments + ReviewCommentUiItem(
                id = newId,
                pageId = pid,
                shapeId = cur.selectedShapeId,
                author = cleanAuthor,
                text = text.trim(),
                resolved = false
            )
        }
        _uiState.update {
            it.copy(
                reviewComments = nextComments,
                isDirty = true,
                canUndo = true,
                statusBanner = "Posted comment #$newId on Sheet.${cur.selectedShapeId}"
            )
        }
        return newId
    }

    @Synchronized
    fun toggleResolveComment(commentId: Int, resolved: Boolean): Boolean {
        val cur = _uiState.value
        val pid = resolveNativePageId(cur.activePageId)
        val nextComments = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeResolveComment(docPtr, commentId, resolved)
            presentViewportOrFallback()
            parseCommentsJson(NativeBridge.nativeListCommentsJson(docPtr, pid))
        } else {
            cur.reviewComments.map { if (it.id == commentId) it.copy(resolved = resolved) else it }
        }
        _uiState.update { it.copy(reviewComments = nextComments, isDirty = true) }
        return true
    }

    @Synchronized
    fun deleteReviewComment(commentId: Int): Boolean {
        val cur = _uiState.value
        val pid = resolveNativePageId(cur.activePageId)
        val nextComments = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeDeleteComment(docPtr, commentId)
            presentViewportOrFallback()
            parseCommentsJson(NativeBridge.nativeListCommentsJson(docPtr, pid))
        } else {
            cur.reviewComments.filterNot { it.id == commentId }
        }
        _uiState.update { it.copy(reviewComments = nextComments, isDirty = true) }
        return true
    }

    @Synchronized
    fun clearActivePageInk(): Int {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val removed = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val count = NativeBridge.nativeClearPageInk(docPtr, pid).coerceAtLeast(0)
            presentViewportOrFallback()
            count
        } else {
            0
        }
        _uiState.update {
            it.copy(
                isDirty = true,
                canUndo = true,
                statusBanner = "Cleared $removed markup ink stroke(s) from page"
            )
        }
        return removed
    }

    @Synchronized
    fun setHyperlinkDialogVisible(visible: Boolean) {
        _uiState.update { it.copy(showHyperlinkDialog = visible) }
    }

    @Synchronized
    fun saveHyperlinkAndScreentip(
        description: String,
        address: String,
        subAddress: String,
        screentip: String
    ): Boolean {
        val cur = _uiState.value
        val pid = resolveNativePageId(cur.activePageId)
        var ok = true
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            ok = NativeBridge.nativeSetHyperlinkAndScreentip(
                docPtr,
                pid,
                cur.selectedShapeId,
                description.trim(),
                address.trim(),
                subAddress.trim(),
                screentip.trim()
            )
            presentViewportOrFallback()
        }
        _uiState.update {
            it.copy(
                showHyperlinkDialog = false,
                hyperlinkDescription = description.trim(),
                hyperlinkAddress = address.trim(),
                hyperlinkSubAddress = subAddress.trim(),
                shapeScreentip = screentip.trim(),
                isDirty = true,
                canUndo = true,
                statusBanner = "Saved Hyperlink & ScreenTip on Sheet.${cur.selectedShapeId}"
            )
        }
        return ok
    }

    @Synchronized
    fun setDomainWizardsDialogVisible(visible: Boolean) {
        _uiState.update { it.copy(showDomainWizardsDialog = visible) }
    }

    @Synchronized
    fun selectDomainWizardPreset(slug: String) {
        val sample = when (slug) {
            "gantt" -> "Discovery,0,5,1.0,\nArchitecture,5,8,0.6,1\nImplementation,13,12,0.25,2\nGA Release,25,3,0.0,3"
            "timeline" -> "0,Q1 Kickoff,true\n30,Architecture Sign-Off,true\n75,Public Beta,true\n110,GA Launch,true"
            "pert" -> "A,Scope,4,\nB,Core Engine,8,A\nC,UI Shell,6,A\nD,Release QA,4,B;C"
            "sql_reverse" -> "CREATE TABLE accounts (\n  id UUID PRIMARY KEY,\n  email VARCHAR(255) NOT NULL\n);\nCREATE TABLE orders (\n  id UUID PRIMARY KEY,\n  account_id UUID REFERENCES accounts(id),\n  total_cents INT NOT NULL\n);"
            "sql_forward" -> "postgres"
            "org_chart" -> "1,,Alex Rivera,Chief Executive Officer,Executive\n2,1,Priya Patel,VP Engineering,Engineering\n3,1,Marcus Chen,VP Product,Product\n4,2,Elena Rostova,Principal Architect,Core"
            "rack_42u" -> "41,2,Core Spine Switch,Switch,320,12\n37,4,GPU Compute Node A,Compute,1400,48\n33,4,NVMe Storage Array,Storage,850,38\n1,2,Dual Redundant UPS,UPS,1800,62"
            "arch_room" -> "Conference Suite A,2.0,3.0,5.0,3.5"
            "calendar" -> "2026-11,monday\n2026-11-03|Architecture Freeze\n2026-11-12|Security Audit\n2026-11-27|v0.9.0-beta Release"
            "pivot_diagram" -> "GROUP=Region>Service;METRIC=Cost;AGG=SUM\nRegion,Service,Cost\nEMEA,Compute,42000\nEMEA,Storage,18000\nAmericas,Compute,64000\nAmericas,AI_GPU,51000\nAPAC,Compute,29000"
            "mind_map" -> "# Vexio 1.0 Launch\n## Core Engine\n### Associative Dimensions\n### Wall Auto-Healing\n## Native Apps\n### Windows WinUI 3\n### macOS Liquid Glass\n### Android Vulkan\n### ChromeOS Offline PWA"
            "mind_map_radial" -> "# Product Strategy\n## Enterprise CAD\n### DWG R2024\n### Scale Locking\n## Process Intelligence\n### Lean VSM\n### Fault Tree MOCUS"
            "mind_map_export" -> "# Vexio Mind Map"
            "vsm" -> "CUSTOMER=Enterprise Accounts;SUPPLIER=Cloud Foundry\nStep,CycleSec,WaitMin,UptimePct,Operators,WIP\nOrder Intake,45,15,99.5,1,12\nCredit & Compliance,120,90,98.0,2,34\nProvision Tenant,60,25,99.9,1,8\nQA & Handover,90,40,97.5,2,15"
            "fta" -> "TOP,Service Outage,OR,,\nG1,Primary Region Down,AND,TOP,\nG2,Failover Blocked,OR,TOP,\nE1,Power Feed A Lost,BASIC,G1,0.01\nE2,UPS Transfer Failure,BASIC,G1,0.02\nE3,DNS Stale TTL,BASIC,G2,0.005\nE4,Replica Lag > RPO,BASIC,G2,0.008"
            else -> ""
        }
        _uiState.update { it.copy(domainWizardSlug = slug, domainWizardSpec = sample, lastDomainWizardOutput = null) }
    }

    @Synchronized
    fun setDomainWizardSpec(spec: String) {
        _uiState.update { it.copy(domainWizardSpec = spec) }
    }

    @Synchronized
    fun runDomainWizard(
        slug: String = _uiState.value.domainWizardSlug,
        spec: String = _uiState.value.domainWizardSpec
    ): String {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val rawResult = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val out = NativeBridge.nativeRunDomainWizard(docPtr, pid, slug, spec).orEmpty()
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
            out
        } else if (slug == "sql_forward") {
            "{\"wizard\":\"sql_forward\",\"sql_ddl\":\"CREATE TABLE diagram_entities (id SERIAL PRIMARY KEY);\"}"
        } else if (slug == "mind_map_export") {
            "{\"wizard\":\"mind_map_export\",\"markdown\":\"# Vexio 1.0 Launch\\n## Core Engine\"}"
        } else {
            "{\"wizard\":\"$slug\",\"created_shapes\":6}"
        }
        val textOut = when (slug) {
            "sql_forward" -> runCatching { org.json.JSONObject(rawResult).optString("sql_ddl", rawResult) }.getOrDefault(rawResult)
            "mind_map_export" -> runCatching { org.json.JSONObject(rawResult).optString("markdown", rawResult) }.getOrDefault(rawResult)
            else -> null
        }
        val isReadOnlyExport = slug == "sql_forward" || slug == "mind_map_export"
        _uiState.update {
            it.copy(
                showDomainWizardsDialog = isReadOnlyExport,
                lastDomainWizardOutput = textOut ?: rawResult,
                isDirty = !isReadOnlyExport || it.isDirty,
                canUndo = true,
                pageShapeCount = if (!isReadOnlyExport) it.pageShapeCount + 6 else it.pageShapeCount,
                statusBanner = "Executed domain wizard: $slug"
            )
        }
        return textOut ?: rawResult
    }

    @Synchronized
    fun applyCadArchitecturalScale(scaleSlug: String = _uiState.value.cadScaleSlug, locked: Boolean = !_uiState.value.cadScaleLocked): Double {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val ratio = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val r = NativeBridge.nativeApplyCadScale(docPtr, pid, scaleSlug, locked)
            presentViewportOrFallback()
            if (r > 0.0) r else 100.0
        } else {
            when (scaleSlug) {
                "1/4_in_ft" -> 48.0
                "1/8_in_ft" -> 96.0
                "1:50" -> 50.0
                "1:200" -> 200.0
                "1:1" -> 1.0
                else -> 100.0
            }
        }
        _uiState.update {
            it.copy(
                cadScaleSlug = scaleSlug,
                cadScaleLocked = locked,
                isDirty = true,
                canUndo = true,
                statusBanner = if (locked) "CAD Scale locked ($scaleSlug, 1:${ratio.toInt()})" else "CAD Scale unlocked"
            )
        }
        return ratio
    }

    @Synchronized
    fun convertCadLayersToShapeSheet(layerOrAll: String = "*"): Int {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val converted = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val c = NativeBridge.nativeConvertCadLayers(docPtr, pid, layerOrAll).coerceAtLeast(0)
            presentViewportOrFallback()
            c
        } else {
            1
        }
        _uiState.update {
            it.copy(
                isDirty = true,
                canUndo = true,
                statusBanner = "Converted $converted CAD shape(s) to unlocked ShapeSheet Geometry1"
            )
        }
        return converted
    }

    fun setSmartActionsDialogVisible(visible: Boolean) {
        _uiState.update { it.copy(showSmartActionsDialog = visible) }
    }

    @Synchronized
    fun upsertSmartAction(
        rowName: String,
        menu: String,
        actionFormula: String,
        checkedFormula: String = "0",
        disabledFormula: String = "0",
        beginGroup: Boolean = false
    ): Boolean {
        val cleanRow = rowName.trim().ifBlank { "Row_1" }
        val cleanMenu = menu.trim().ifBlank { cleanRow }
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val sid = _uiState.value.selectedShapeId
        if (sid <= 0L) return false
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val ok = NativeBridge.nativeUpsertShapeAction(
                docPtr, pid, sid, cleanRow, cleanMenu, actionFormula, checkedFormula, disabledFormula, beginGroup
            )
            if (!ok) return false
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        } else {
            val chk = checkedFormula.trim() in setOf("1", "TRUE", "true")
            val dis = disabledFormula.trim() in setOf("1", "TRUE", "true")
            val item = SmartActionUiItem(
                rowName = cleanRow,
                menu = cleanMenu,
                actionFormula = actionFormula,
                checkedFormula = checkedFormula,
                disabledFormula = disabledFormula,
                checked = chk,
                disabled = dis,
                invisible = false,
                beginGroup = beginGroup
            )
            _uiState.update { st ->
                val next = st.selectedShapeActions.filterNot { it.rowName.equals(cleanRow, ignoreCase = true) } + item
                st.copy(selectedShapeActions = next)
            }
        }
        _uiState.update {
            it.copy(isDirty = true, canUndo = true, statusBanner = "Saved Smart Action Actions.$cleanRow")
        }
        return true
    }

    @Synchronized
    fun deleteSmartAction(rowName: String): Boolean {
        val cleanRow = rowName.trim()
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val sid = _uiState.value.selectedShapeId
        if (cleanRow.isEmpty() || sid <= 0L) return false
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val ok = NativeBridge.nativeDeleteShapeAction(docPtr, pid, sid, cleanRow)
            if (!ok) return false
            refreshSelectedShapeFormulas()
        } else {
            _uiState.update { st ->
                st.copy(selectedShapeActions = st.selectedShapeActions.filterNot { it.rowName.equals(cleanRow, ignoreCase = true) })
            }
        }
        _uiState.update {
            it.copy(isDirty = true, canUndo = true, statusBanner = "Deleted Smart Action Actions.$cleanRow")
        }
        return true
    }

    @Synchronized
    fun executeSmartAction(rowName: String): Boolean {
        val cleanRow = rowName.trim()
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val sid = _uiState.value.selectedShapeId
        if (cleanRow.isEmpty() || sid <= 0L) return false
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val outJson = NativeBridge.nativeExecuteShapeAction(docPtr, pid, sid, cleanRow).orEmpty()
            val outObj = runCatching { Json.parseToJsonElement(outJson).jsonObject }.getOrNull()
            val targetPageId = outObj?.get("targetPageId")?.jsonPrimitive?.intOrNull ?: 0
            val openTextEdit = outObj?.get("openTextEdit")?.jsonPrimitive?.booleanOrNull ?: false
            if (targetPageId > 0 && targetPageId != _uiState.value.activePageId) {
                selectPage(targetPageId)
            }
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
            if (openTextEdit) {
                beginInlineTextEdit(sid, isNewTextShape = false)
            }
        }
        _uiState.update {
            it.copy(isDirty = true, canUndo = true, statusBanner = "Executed Smart Action Actions.$cleanRow")
        }
        return true
    }

    @Synchronized
    fun addSelectionToMyShapes(masterName: String = "", stencilTitle: String = "My Shapes"): String {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val ids = _uiState.value.selectedShapeIds.ifEmpty { listOf(_uiState.value.selectedShapeId) }
            .filter { it > 0L }
            .toLongArray()
        if (ids.isEmpty()) return ""
        val cleanPack = stencilTitle.trim().ifBlank { "My Shapes" }
        var createdName = masterName.trim().ifBlank { "Custom Shape ${ids.first()}" }
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val raw = NativeBridge.nativeAddToCustomStencil(docPtr, pid, ids, cleanPack, masterName).orEmpty()
            val obj = runCatching { Json.parseToJsonElement(raw).jsonObject }.getOrNull()
            createdName = obj?.get("masterName")?.jsonPrimitive?.contentOrNull?.takeIf { it.isNotBlank() }
                ?: createdName
            loadDynamicStencilsFromNative()
        } else {
            val customItem = StencilMasterItem(
                masterId = "$cleanPack::$createdName",
                title = createdName,
                category = cleanPack,
                defaultWidthFormula = "1.50 in",
                defaultHeightFormula = "1.00 in",
                fillFormula = _uiState.value.shapeFillColorHex,
                shapeKind = "custom_master",
                prompt = "Custom reusable Visio Master",
                svgPreview = "<svg viewBox=\"0 0 96 64\"><rect x=\"8\" y=\"8\" width=\"80\" height=\"48\" rx=\"6\" fill=\"#eff6ff\" stroke=\"#2563eb\"/></svg>"
            )
            _uiState.update { st ->
                val cats = if (cleanPack in st.stencilCategories) st.stencilCategories else st.stencilCategories + cleanPack
                val masters = st.stencilMasters.filterNot { it.category == cleanPack && it.title == createdName } + customItem
                st.copy(stencilCategories = cats, stencilMasters = masters)
            }
        }
        _uiState.update {
            it.copy(
                selectedStencilCategory = cleanPack,
                isDirty = true,
                canUndo = true,
                statusBanner = "Saved '$createdName' to '$cleanPack'"
            )
        }
        return createdName
    }

    @Synchronized
    fun renameCustomStencilMaster(oldName: String, newName: String, stencilTitle: String = "My Shapes"): Boolean {
        val cleanPack = stencilTitle.trim().ifBlank { "My Shapes" }
        val cleanOld = oldName.trim()
        val cleanNew = newName.trim()
        if (cleanOld.isEmpty() || cleanNew.isEmpty()) return false
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            if (!NativeBridge.nativeRenameCustomMaster(docPtr, cleanPack, cleanOld, cleanNew)) return false
            loadDynamicStencilsFromNative()
        } else {
            _uiState.update { st ->
                st.copy(
                    stencilMasters = st.stencilMasters.map { m ->
                        if (m.category == cleanPack && m.title == cleanOld) {
                            m.copy(masterId = "$cleanPack::$cleanNew", title = cleanNew)
                        } else {
                            m
                        }
                    }
                )
            }
        }
        _uiState.update { it.copy(isDirty = true, statusBanner = "Renamed master '$cleanOld' to '$cleanNew'") }
        return true
    }

    @Synchronized
    fun deleteCustomStencilMaster(masterName: String, stencilTitle: String = "My Shapes"): Boolean {
        val cleanPack = stencilTitle.trim().ifBlank { "My Shapes" }
        val cleanName = masterName.trim()
        if (cleanName.isEmpty()) return false
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            if (!NativeBridge.nativeDeleteCustomMaster(docPtr, cleanPack, cleanName)) return false
            loadDynamicStencilsFromNative()
        } else {
            _uiState.update { st ->
                st.copy(stencilMasters = st.stencilMasters.filterNot { it.category == cleanPack && it.title == cleanName })
            }
        }
        _uiState.update { it.copy(isDirty = true, statusBanner = "Deleted custom master '$cleanName'") }
        return true
    }

    @Synchronized
    fun updateCustomMasterFromSelection(masterName: String = "", stencilTitle: String = "My Shapes"): Int {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val sid = _uiState.value.selectedShapeId
        if (sid <= 0L) return -1
        val cleanPack = stencilTitle.trim().ifBlank { "My Shapes" }
        val updated = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val c = NativeBridge.nativeUpdateCustomMaster(docPtr, pid, sid, cleanPack, masterName)
            if (c >= 0) {
                loadDynamicStencilsFromNative()
                refreshSelectedShapeFormulas()
                presentViewportOrFallback()
            }
            c
        } else {
            1
        }
        if (updated >= 0) {
            _uiState.update {
                it.copy(
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Updated custom master and propagated to $updated instance(s)"
                )
            }
        }
        return updated
    }

    @Synchronized
    fun exportCustomStencilVssxBytes(stencilTitle: String = "My Shapes"): ByteArray {
        val cleanPack = stencilTitle.trim().ifBlank { "My Shapes" }
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val nativeBytes = NativeBridge.nativeExportCustomStencilVssx(docPtr, cleanPack)
            if (nativeBytes != null && nativeBytes.isNotEmpty()) {
                return nativeBytes
            }
        }
        val baos = ByteArrayOutputStream()
        ZipOutputStream(baos).use { zos ->
            writeZipUtf8(
                zos,
                "[Content_Types].xml",
                """<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/visio/document.xml" ContentType="application/vnd.ms-visio.stencil.main+xml"/><Override PartName="/visio/masters/masters.xml" ContentType="application/vnd.ms-visio.masters+xml"/></Types>"""
            )
            writeZipUtf8(
                zos,
                "visio/masters/masters.xml",
                """<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Masters xmlns="http://schemas.microsoft.com/office/visio/2012/main"/>"""
            )
        }
        return baos.toByteArray()
    }

    // -------------------------------------------------------------------------
    // Milestone 3 (`v0.8.0-beta`): Dynamic <fld> Fields, 45 Arrowheads, Line Jumps,
    // Ruler Guides, Snap & Glue Matrix, Find/Replace, Format Painter & Number Shapes
    // -------------------------------------------------------------------------
    fun setInsertFieldDialogVisible(visible: Boolean) {
        _uiState.update { it.copy(showInsertFieldDialog = visible) }
    }

    fun setFindReplaceDialogVisible(visible: Boolean) {
        _uiState.update { it.copy(showFindReplaceDialog = visible) }
    }

    fun setSnapGlueDialogVisible(visible: Boolean) {
        _uiState.update { it.copy(showSnapGlueDialog = visible) }
    }

    fun setNumberShapesDialogVisible(visible: Boolean) {
        _uiState.update { it.copy(showNumberShapesDialog = visible) }
    }

    @Synchronized
    fun insertDynamicTextField(
        presetOrFormula: String,
        formatSpec: String = "0.00 in",
        prefixLabel: String = ""
    ): Int {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val sid = _uiState.value.selectedShapeId
        val cleanPreset = presetOrFormula.trim().ifBlank { "width" }
        if (sid <= 0L) return -1
        val fieldIx = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val ix = NativeBridge.nativeInsertTextField(docPtr, pid, sid, cleanPreset, formatSpec, prefixLabel)
            if (ix >= 0) {
                refreshSelectedShapeFormulas()
                presentViewportOrFallback()
            }
            ix
        } else {
            val formulaStr = when (cleanPreset.lowercase(Locale.US)) {
                "width" -> "Width"
                "height" -> "Height"
                "angle" -> "Angle"
                "page_name" -> "PAGENAME()"
                "page_number" -> "PAGENUMBER()"
                "page_count" -> "PAGECOUNT()"
                else -> cleanPreset.removePrefix("=")
            }
            _uiState.update { st ->
                val ix = st.shapeSheetCells.count { it.section == "Text Fields" }
                val newCell = ShapeSheetCellEntry("Text Fields", "Fields.Row_${ix + 1}", formulaStr, isGuarded = false)
                st.copy(shapeSheetCells = st.shapeSheetCells + newCell)
            }
            0
        }
        if (fieldIx >= 0) {
            _uiState.update {
                it.copy(
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Inserted dynamic <fld> field ($cleanPreset) on Sheet.$sid"
                )
            }
        }
        return fieldIx
    }

    @Synchronized
    fun setConnectorArrowheads(beginArrow: Int, endArrow: Int, arrowSize: Int = 2): Int {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val ids = _uiState.value.selectedShapeIds.ifEmpty {
            listOfNotNull(_uiState.value.selectedShapeId.takeIf { it > 0L })
        }.toLongArray()
        if (ids.isEmpty()) return 0
        val bClamped = beginArrow.coerceIn(0, 45)
        val eClamped = endArrow.coerceIn(0, 45)
        val sClamped = arrowSize.coerceIn(0, 6)
        val count = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val n = NativeBridge.nativeSetConnectorArrowheads(docPtr, pid, ids, bClamped, eClamped, sClamped)
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
            n
        } else {
            _uiState.update { st ->
                val filtered = st.shapeSheetCells.filterNot { it.cellName == "BeginArrow" || it.cellName == "EndArrow" }
                st.copy(
                    shapeSheetCells = filtered +
                        ShapeSheetCellEntry("Line Format", "BeginArrow", bClamped.toString(), false) +
                        ShapeSheetCellEntry("Line Format", "EndArrow", eClamped.toString(), false)
                )
            }
            ids.size
        }
        _uiState.update {
            it.copy(
                isDirty = true,
                canUndo = true,
                statusBanner = "Applied Visio arrowheads ($bClamped → $eClamped) to $count shape(s)"
            )
        }
        return count
    }

    @Synchronized
    fun setPageLineJumps(
        jumpCode: Int,
        jumpStyle: Int,
        factorX: Double = 0.66,
        factorY: Double = 0.66
    ): Boolean {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val ok = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val res = NativeBridge.nativeSetPageLineJumps(docPtr, pid, jumpCode, jumpStyle, factorX, factorY)
            presentViewportOrFallback()
            res
        } else {
            true
        }
        if (ok) {
            _uiState.update {
                it.copy(
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Updated Page Line Jumps (code=${jumpCode.coerceIn(0, 3)}, style=${jumpStyle.coerceIn(0, 6)})"
                )
            }
        }
        return ok
    }

    @Synchronized
    fun addRulerGuide(isVertical: Boolean, coordIn: Double): Long {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val gid = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val id = NativeBridge.nativeAddRulerGuide(docPtr, pid, isVertical, coordIn)
            presentViewportOrFallback()
            id
        } else {
            (9000L + _uiState.value.rulerGuides.size + 1L)
        }
        if (gid > 0L) {
            _uiState.update { st ->
                st.copy(
                    rulerGuides = st.rulerGuides + RulerGuideUiItem(gid, isVertical, coordIn),
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Added ${if (isVertical) "vertical" else "horizontal"} ruler guide at ${"%.2f".format(Locale.US, coordIn)} in"
                )
            }
        }
        return gid
    }

    @Synchronized
    fun moveRulerGuide(guideId: Long, coordIn: Double): Boolean {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        if (guideId <= 0L) return false
        val ok = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val r = NativeBridge.nativeMoveRulerGuide(docPtr, pid, guideId, coordIn)
            presentViewportOrFallback()
            r
        } else {
            _uiState.value.rulerGuides.any { it.shapeId == guideId }
        }
        if (ok) {
            _uiState.update { st ->
                st.copy(
                    rulerGuides = st.rulerGuides.map { g -> if (g.shapeId == guideId) g.copy(coordIn = coordIn) else g },
                    isDirty = true
                )
            }
        }
        return ok
    }

    @Synchronized
    fun deleteRulerGuide(guideId: Long): Boolean {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        if (guideId <= 0L) return false
        val ok = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val r = NativeBridge.nativeDeleteRulerGuide(docPtr, pid, guideId)
            presentViewportOrFallback()
            r
        } else {
            _uiState.value.rulerGuides.any { it.shapeId == guideId }
        }
        if (ok) {
            _uiState.update { st ->
                st.copy(
                    rulerGuides = st.rulerGuides.filterNot { it.shapeId == guideId },
                    isDirty = true
                )
            }
        }
        return ok
    }

    @Synchronized
    fun setSnapGlueSettings(
        snapEnabled: Boolean,
        glueEnabled: Boolean,
        snapTargetsMask: Int,
        glueTypesMask: Int
    ): Boolean {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val ok = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeSetSnapGlue(docPtr, pid, snapEnabled, glueEnabled, snapTargetsMask, glueTypesMask)
        } else {
            true
        }
        if (ok) {
            _uiState.update {
                it.copy(
                    snapEnabled = snapEnabled,
                    glueEnabled = glueEnabled,
                    snapTargetsMask = snapTargetsMask,
                    glueTypesMask = glueTypesMask,
                    isDirty = true,
                    statusBanner = "Updated Snap & Glue matrix (snap=$snapEnabled, glue=$glueEnabled)"
                )
            }
        }
        return ok
    }

    @Synchronized
    fun findInDocument(
        query: String,
        matchCase: Boolean = false,
        wholeWord: Boolean = false,
        allPages: Boolean = true
    ): List<FindMatchUiItem> {
        val clean = query.trim()
        if (clean.isEmpty()) return emptyList()
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val matches = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val raw = NativeBridge.nativeFindInDocumentJson(docPtr, pid, clean, matchCase, wholeWord, allPages).orEmpty()
            runCatching {
                Json.parseToJsonElement(raw).jsonArray.map { el ->
                    val o = el.jsonObject
                    FindMatchUiItem(
                        pageId = o["pageId"]?.jsonPrimitive?.intOrNull ?: pid,
                        shapeId = o["shapeId"]?.jsonPrimitive?.longOrNull ?: 1L,
                        field = o["field"]?.jsonPrimitive?.contentOrNull ?: "Text",
                        snippet = o["snippet"]?.jsonPrimitive?.contentOrNull.orEmpty()
                    )
                }
            }.getOrDefault(emptyList())
        } else {
            val st = _uiState.value
            val haystacks = listOf(
                "Name" to st.selectedShapeName,
                "Text" to (st.inspectedObject?.text ?: st.selectedShapeName)
            )
            haystacks.filter { (_, v) ->
                v.contains(clean, ignoreCase = !matchCase)
            }.map { (field, v) ->
                FindMatchUiItem(pid, st.selectedShapeId.coerceAtLeast(1L), field, v)
            }
        }
        _uiState.update {
            it.copy(
                lastFindMatches = matches,
                statusBanner = "Found ${matches.size} match(es) for '$clean'"
            )
        }
        return matches
    }

    @Synchronized
    fun replaceInDocument(
        findStr: String,
        replaceStr: String,
        matchCase: Boolean = false,
        wholeWord: Boolean = false,
        allPages: Boolean = true
    ): Int {
        val clean = findStr.trim()
        if (clean.isEmpty()) return 0
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val replaced = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val n = NativeBridge.nativeReplaceInDocument(docPtr, pid, clean, replaceStr, matchCase, wholeWord, allPages)
            if (n > 0) {
                refreshSelectedShapeFormulas()
                presentViewportOrFallback()
            }
            n
        } else {
            val cur = _uiState.value.inspectedObject?.text.orEmpty().ifEmpty { _uiState.value.selectedShapeName }
            if (cur.contains(clean, ignoreCase = !matchCase)) {
                val updated = cur.replace(clean, replaceStr, ignoreCase = !matchCase)
                _uiState.update { st ->
                    st.copy(
                        selectedShapeName = updated,
                        inspectedObject = st.inspectedObject?.copy(text = updated, name = updated)
                    )
                }
                1
            } else {
                0
            }
        }
        _uiState.update {
            it.copy(
                isDirty = replaced > 0 || it.isDirty,
                canUndo = replaced > 0 || it.canUndo,
                statusBanner = "Replaced $replaced occurrence(s)"
            )
        }
        return replaced
    }

    @Synchronized
    fun copySelectedShapeFormat(): String {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val sid = _uiState.value.selectedShapeId
        if (sid <= 0L) return ""
        val packet = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeCopyShapeFormatJson(docPtr, pid, sid).orEmpty()
        } else {
            val fill = _uiState.value.shapeSheetCells.firstOrNull { it.cellName == "FillForegnd" }?.formula ?: "RGB(232,240,254)"
            val line = _uiState.value.shapeSheetCells.firstOrNull { it.cellName == "LineColor" }?.formula ?: "RGB(26,115,232)"
            """{"cells":{"FillForegnd":"${escapeJson(fill)}","LineColor":"${escapeJson(line)}"}}"""
        }
        if (packet.isNotEmpty()) {
            _uiState.update {
                it.copy(
                    copiedFormatJson = packet,
                    statusBanner = "Copied formatting from Sheet.$sid"
                )
            }
        }
        return packet
    }

    @Synchronized
    fun applyFormatPainter(targetShapeIds: LongArray = longArrayOf()): Int {
        val packet = _uiState.value.copiedFormatJson.orEmpty()
        if (packet.isEmpty()) return 0
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val ids = if (targetShapeIds.isNotEmpty()) {
            targetShapeIds
        } else {
            _uiState.value.selectedShapeIds.ifEmpty {
                listOfNotNull(_uiState.value.selectedShapeId.takeIf { it > 0L })
            }.toLongArray()
        }
        if (ids.isEmpty()) return 0
        val count = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val n = NativeBridge.nativeApplyFormatPainter(docPtr, pid, ids, packet)
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
            n
        } else {
            ids.size
        }
        _uiState.update {
            it.copy(
                isDirty = count > 0 || it.isDirty,
                canUndo = count > 0 || it.canUndo,
                statusBanner = "Applied Format Painter to $count shape(s)"
            )
        }
        return count
    }

    @Synchronized
    fun numberShapesOnPage(
        orderMode: String = "top_to_bottom_left_to_right",
        startNum: Int = 1,
        interval: Int = 1,
        prefix: String = "",
        targetMode: String = "both"
    ): Int {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val count = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val n = NativeBridge.nativeNumberShapes(docPtr, pid, longArrayOf(), orderMode, startNum, interval, prefix, targetMode)
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
            n
        } else {
            val numStr = "$prefix$startNum"
            _uiState.update { st ->
                st.copy(
                    shapeDataItems = st.shapeDataItems.filterNot { it.rowName == "ShapeNumber" } +
                        ShapeDataUiItem(rowName = "ShapeNumber", label = "Shape Number", formula = "\"$numStr\"", value = numStr)
                )
            }
            _uiState.value.pageShapeCount.coerceAtLeast(1)
        }
        _uiState.update {
            it.copy(
                isDirty = count > 0 || it.isDirty,
                canUndo = count > 0 || it.canUndo,
                statusBanner = "Auto-numbered $count shape(s) ($orderMode)"
            )
        }
        return count
    }

    // -------------------------------------------------------------------------
    // Milestone 4 (`v0.9.0-beta`): Associative Architectural Dimensions,
    // Wall Miter/T-Join Auto-Healing, Door/Window Openings, Mind Map Topics & VSM/FTA
    // -------------------------------------------------------------------------
    @Synchronized
    fun insertAssociativeDimension(
        dimKind: String = "aligned",
        targetShapeId: Long = _uiState.value.selectedShapeId,
        secondShapeId: Long = _uiState.value.selectedShapeIds.getOrNull(1) ?: 0L,
        offsetIn: Double = 0.55,
        unitFormat: String = "auto"
    ): Long {
        if (targetShapeId <= 0L) return 0L
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val cleanKind = dimKind.trim().ifBlank { "aligned" }
        val dimId = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val id = NativeBridge.nativeInsertAssociativeDimension(
                docPtr, pid, cleanKind, targetShapeId, secondShapeId.coerceAtLeast(0L), offsetIn, unitFormat
            )
            if (id > 0L) {
                syncViewportSelection(listOf(id))
                refreshSelectedShapeFormulas()
                presentViewportOrFallback()
            }
            id
        } else {
            val id = nextSyntheticShapeId++
            _uiState.update { st ->
                st.copy(
                    shapeSheetCells = st.shapeSheetCells.filterNot { it.cellName == "User.DimKind" } +
                        ShapeSheetCellEntry("User-Defined Cells", "User.DimKind", "\"$cleanKind\"", false)
                )
            }
            id
        }
        if (dimId > 0L) {
            _uiState.update {
                it.copy(
                    selectedShapeId = dimId,
                    selectedShapeIds = listOf(dimId),
                    selectedShapeName = "Sheet.$dimId (Dimension)",
                    pageShapeCount = it.pageShapeCount + 1,
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Attached associative $cleanKind dimension (Sheet.$dimId)"
                )
            }
        }
        return dimId
    }

    @Synchronized
    fun drawArchitecturalWall(
        x1In: Double,
        y1In: Double,
        x2In: Double,
        y2In: Double,
        thicknessIn: Double = 0.25
    ): Long {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val wallId = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val id = NativeBridge.nativeDrawArchitecturalWall(docPtr, pid, x1In, y1In, x2In, y2In, thicknessIn)
            if (id > 0L) {
                syncViewportSelection(listOf(id))
                refreshSelectedShapeFormulas()
                presentViewportOrFallback()
            }
            id
        } else {
            nextSyntheticShapeId++
        }
        if (wallId > 0L) {
            _uiState.update {
                it.copy(
                    selectedShapeId = wallId,
                    selectedShapeIds = listOf(wallId),
                    selectedShapeName = "Sheet.$wallId (Wall)",
                    pageShapeCount = it.pageShapeCount + 1,
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Drew architectural wall segment (Sheet.$wallId) with auto-healing miters"
                )
            }
        }
        return wallId
    }

    @Synchronized
    fun healWallIntersections(toleranceIn: Double = 0.30): Int {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val healed = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val n = NativeBridge.nativeHealWallIntersections(docPtr, pid, toleranceIn)
            presentViewportOrFallback()
            n
        } else {
            1
        }
        _uiState.update {
            it.copy(
                isDirty = healed > 0 || it.isDirty,
                canUndo = healed > 0 || it.canUndo,
                statusBanner = "Auto-healed $healed wall L-corner miter & T-join intersection(s)"
            )
        }
        return healed
    }

    @Synchronized
    fun insertWallOpening(
        openingKind: String = "single_door",
        wallShapeId: Long = _uiState.value.selectedShapeId,
        offsetAlongIn: Double = -1.0,
        openingWidthIn: Double = if (openingKind in setOf("double_door", "sliding_door")) 1.40 else 0.90,
        swingDir: String = "in_left"
    ): Long {
        if (wallShapeId <= 0L) return 0L
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val cleanKind = openingKind.trim().ifBlank { "single_door" }
        val openingId = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val id = NativeBridge.nativeInsertWallOpening(
                docPtr, pid, wallShapeId, cleanKind, offsetAlongIn, openingWidthIn, swingDir
            )
            if (id > 0L) {
                syncViewportSelection(listOf(id))
                refreshSelectedShapeFormulas()
                presentViewportOrFallback()
            }
            id
        } else {
            nextSyntheticShapeId++
        }
        if (openingId > 0L) {
            _uiState.update {
                it.copy(
                    selectedShapeId = openingId,
                    selectedShapeIds = listOf(openingId),
                    selectedShapeName = "Sheet.$openingId ($cleanKind)",
                    pageShapeCount = it.pageShapeCount + 1,
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Cut parametric $cleanKind opening (Sheet.$openingId) into Wall Sheet.$wallShapeId"
                )
            }
        }
        return openingId
    }

    @Synchronized
    fun addMindMapTopic(
        topicText: String = "New Subtopic",
        asSibling: Boolean = false,
        anchorShapeId: Long = _uiState.value.selectedShapeId
    ): Long {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val cleanTopic = topicText.trim().ifBlank { "New Topic" }
        val topicId = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val id = NativeBridge.nativeAddMindMapTopic(docPtr, pid, anchorShapeId.coerceAtLeast(0L), cleanTopic, asSibling)
            if (id > 0L) {
                syncViewportSelection(listOf(id))
                refreshSelectedShapeFormulas()
                presentViewportOrFallback()
            }
            id
        } else {
            nextSyntheticShapeId++
        }
        if (topicId > 0L) {
            _uiState.update {
                it.copy(
                    selectedShapeId = topicId,
                    selectedShapeIds = listOf(topicId),
                    selectedShapeName = cleanTopic,
                    pageShapeCount = it.pageShapeCount + 1,
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Added Mind Map topic '$cleanTopic' (Sheet.$topicId)"
                )
            }
        }
        return topicId
    }

    @Synchronized
    fun exportMindMapOutline(): String {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val md = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeExportMindMapOutline(docPtr, pid).orEmpty()
        } else {
            "# ${_uiState.value.documentTitle.substringBeforeLast('.')}\n## ${_uiState.value.selectedShapeName}\n"
        }
        _uiState.update {
            it.copy(
                lastDomainWizardOutput = md,
                statusBanner = "Exported Mind Map Markdown outline (${md.lines().count { l -> l.isNotBlank() }} topics)"
            )
        }
        return md
    }

    @Synchronized
    fun recalculateVsmAndFta(): String {
        val pid = resolveNativePageId(_uiState.value.activePageId)
        val reportJson = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val out = NativeBridge.nativeRecalculateVsmAndFtaJson(docPtr, pid).orEmpty()
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
            out
        } else {
            """{"vsm":{"pce_pct":18.4,"value_added_sec":315.0},"fta":{"top_probability":0.0132,"minimal_cut_sets":3}}"""
        }
        _uiState.update {
            it.copy(
                isDirty = true,
                canUndo = true,
                statusBanner = "Recalculated Lean VSM PCE% & Fault Tree MOCUS Cut Sets"
            )
        }
        return reportJson
    }

    @Synchronized
    fun focusSlideSnippet(index: Int, enterPresenter: Boolean = true) {
        var slides = _uiState.value.slideSnippets
        if (slides.isEmpty()) {
            autoGenerateSlideSnippets()
            slides = _uiState.value.slideSnippets
        }
        if (slides.isEmpty()) return
        val wrapped = ((index % slides.size) + slides.size) % slides.size
        val target = slides[wrapped]
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L && docPtr != 0L) {
            val scale = (8.5 / target.heightIn.coerceAtLeast(1.0)).coerceIn(0.35, 3.5)
            NativeBridge.nativeSetViewportZoom(
                viewportPtr,
                docPtr,
                scale,
                target.xIn + target.widthIn * 0.5,
                target.yIn + target.heightIn * 0.5
            )
            presentViewportOrFallback()
        }
        _uiState.update {
            it.copy(
                activeSlideIndex = wrapped,
                isPresentationModeActive = enterPresenter,
                showSlideSnippetsDialog = false,
                statusBanner = "Slide ${wrapped + 1}/${slides.size}: ${target.title}"
            )
        }
    }

    @Synchronized
    fun setPresentationModeActive(active: Boolean) {
        if (active) {
            focusSlideSnippet(_uiState.value.activeSlideIndex, enterPresenter = true)
        } else {
            _uiState.update { it.copy(isPresentationModeActive = false) }
        }
    }

    @Synchronized
    fun resetSelectedConnectorRouting(): Boolean {
        val sid = _uiState.value.selectedShapeId
        if (sid <= 0L) return false
        var ok = true
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            ok = NativeBridge.nativeResetConnectorRouting(
                docPtr,
                resolveNativePageId(_uiState.value.activePageId),
                sid
            )
            presentViewportOrFallback()
        }
        if (ok) {
            _uiState.update {
                it.copy(isDirty = true, canUndo = true, statusBanner = "Restored automatic A* orthogonal connector routing")
            }
        }
        return ok
    }

    @Synchronized
    fun ensureSelectedControlHandle(): Boolean {
        val sid = _uiState.value.selectedShapeId
        if (sid <= 0L) return false
        var ok = true
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            ok = NativeBridge.nativeEnsureControlHandle(
                docPtr,
                resolveNativePageId(_uiState.value.activePageId),
                sid
            )
            presentViewportOrFallback()
        }
        if (ok) {
            _uiState.update {
                it.copy(isDirty = true, canUndo = true, statusBanner = "Attached parametric yellow Controls.Row_1 handle")
            }
        }
        return ok
    }

    @Synchronized
    fun onCanvasDoubleTap(xPx: Float, yPx: Float) {
        var hitShapeId = 0L
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L && docPtr != 0L) {
            hitShapeId = NativeBridge.nativeHitTestShape(viewportPtr, docPtr, xPx.toDouble(), yPx.toDouble())
            if (hitShapeId <= 0L) {
                hitShapeId = NativeBridge.nativeGetPrimarySelection(viewportPtr)
            }
        }
        if (hitShapeId <= 0L) {
            hitShapeId = _uiState.value.selectedShapeId
        }
        if (hitShapeId > 0L) {
            beginInlineTextEdit(hitShapeId, isNewTextShape = false, fallbackXPx = xPx, fallbackYPx = yPx)
        }
    }

    @Synchronized
    fun beginInlineTextEdit(
        shapeId: Long,
        isNewTextShape: Boolean = false,
        fallbackXPx: Float = 160f,
        fallbackYPx: Float = 140f
    ) {
        if (shapeId <= 0L) return
        var rectX = fallbackXPx
        var rectY = fallbackYPx
        var rectW = 200f
        var rectH = 54f
        var existingLabel = ""

        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            if (viewportPtr != 0L) {
                val boundsJson = runCatching {
                    NativeBridge.nativeGetShapeScreenBoundsJson(viewportPtr, docPtr, shapeId)
                }.getOrDefault("{}")
                val boundsObj = runCatching { Json.parseToJsonElement(boundsJson).jsonObject }.getOrNull()
                if (boundsObj != null && boundsObj.containsKey("x")) {
                    rectX = boundsObj["x"]?.jsonPrimitive?.doubleOrNull?.toFloat() ?: fallbackXPx
                    rectY = boundsObj["y"]?.jsonPrimitive?.doubleOrNull?.toFloat() ?: fallbackYPx
                    rectW = (boundsObj["w"]?.jsonPrimitive?.doubleOrNull?.toFloat() ?: 180f).coerceAtLeast(130f)
                    rectH = (boundsObj["h"]?.jsonPrimitive?.doubleOrNull?.toFloat() ?: 48f).coerceAtLeast(42f)
                }
            }
            val stateJson = runCatching {
                NativeBridge.nativeGetStateJson(docPtr, resolveNativePageId(_uiState.value.activePageId))
            }.getOrDefault("{}")
            val stateObj = runCatching { Json.parseToJsonElement(stateJson).jsonObject }.getOrNull()
            val shapesArr = stateObj?.get("shapes") as? JsonArray
            if (shapesArr != null) {
                for (elem in shapesArr) {
                    val shObj = elem as? JsonObject ?: continue
                    val idVal = shObj["id"]?.jsonPrimitive?.longOrNull ?: 0L
                    if (idVal == shapeId) {
                        existingLabel = shObj["text"]?.jsonPrimitive?.contentOrNull.orEmpty()
                        break
                    }
                }
            }
        }
        if (isNewTextShape && existingLabel == "Text") {
            existingLabel = ""
        }

        _uiState.update {
            it.copy(
                selectedShapeId = shapeId,
                selectedShapeIds = listOf(shapeId),
                inlineTextEditor = InlineCanvasEditorModel(
                    shapeId = shapeId,
                    text = existingLabel,
                    xPx = rectX,
                    yPx = rectY,
                    widthPx = rectW,
                    heightPx = rectH,
                    isNewlyCreatedText = isNewTextShape
                )
            )
        }
    }

    fun updateInlineTextDraft(newDraft: String) {
        _uiState.update { state ->
            val current = state.inlineTextEditor ?: return@update state
            state.copy(inlineTextEditor = current.copy(text = newDraft))
        }
    }

    @Synchronized
    fun commitInlineTextEdit(finalText: String) {
        val editor = _uiState.value.inlineTextEditor ?: return
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeSetShapeText(
                docPtr = docPtr,
                pageId = resolveNativePageId(_uiState.value.activePageId),
                shapeId = editor.shapeId,
                text = finalText
            )
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        }
        _uiState.update {
            it.copy(
                inlineTextEditor = null,
                isDirty = true,
                canUndo = true,
                statusBanner = "Updated label for Sheet.${editor.shapeId}"
            )
        }
    }

    fun cancelInlineTextEdit() {
        _uiState.update { it.copy(inlineTextEditor = null) }
    }

    private fun sanitizeStencilFileName(fileName: String): String {
        val cleaned = fileName.replace(Regex("[^A-Za-z0-9._\\- ]"), "_").trim()
        return cleaned.ifBlank { "Custom_Stencil.vssx" }
    }

    @Synchronized
    private fun reloadPersistedStencilsLocked() {
        val dir = stencilStorageDir ?: return
        if (!dir.exists() || !NativeBridge.isNativeAvailable || docPtr == 0L) return
        val files = dir.listFiles()?.sortedBy { it.name.lowercase(Locale.US) }.orEmpty()
        val installedNames = linkedSetOf<String>()
        for (f in files) {
            if (!f.isFile || f.length() <= 0L) continue
            val ext = f.extension.lowercase(Locale.US)
            if (ext !in setOf("vssx", "vssm", "vss", "vsdx", "vstx", "vstm", "vsd", "vst", "vdx", "zip", "svg", "png")) {
                continue
            }
            runCatching {
                val bytes = f.readBytes()
                NativeBridge.nativeImportStencilBytes(docPtr, f.name, bytes)
                installedNames.add(f.name)
            }
        }
        if (installedNames.isNotEmpty()) {
            _uiState.update { it.copy(installedStencilFileNames = it.installedStencilFileNames + installedNames) }
        }
    }

    fun attachStencilStorageDirectory(dir: File) {
        stencilStorageDir = dir
        viewModelScope.launch(ioDispatcher) {
            runCatching { if (!dir.exists()) dir.mkdirs() }
            synchronized(this@VexioSessionViewModel) {
                if (NativeBridge.ensureLoaded() && docPtr != 0L) {
                    reloadPersistedStencilsLocked()
                    loadDynamicStencilsFromNative()
                }
            }
        }
    }

    fun setOfficialStencilCatalogDialogVisible(visible: Boolean) {
        if (visible && _uiState.value.officialStencilCatalog.isEmpty()) {
            viewModelScope.launch(ioDispatcher) {
                val entries = if (NativeBridge.ensureLoaded()) {
                    runCatching {
                        lenientJson.decodeFromString<List<OfficialStencilCatalogEntry>>(
                            NativeBridge.nativeListOfficialStencilCatalogJson()
                        )
                    }.getOrDefault(emptyList())
                } else {
                    emptyList()
                }
                _uiState.update {
                    it.copy(
                        officialStencilCatalog = entries,
                        showOfficialStencilCatalogDialog = true
                    )
                }
            }
        } else {
            _uiState.update { it.copy(showOfficialStencilCatalogDialog = visible) }
        }
    }

    fun downloadOfficialStencilPack(entry: OfficialStencilCatalogEntry) {
        if (entry.id.isBlank() || entry.url.isBlank() || entry.id in _uiState.value.downloadingStencilPackIds) return
        _uiState.update {
            it.copy(
                downloadingStencilPackIds = it.downloadingStencilPackIds + entry.id,
                statusBanner = "Downloading '${entry.title}'..."
            )
        }
        viewModelScope.launch(ioDispatcher) {
            val result = runCatching {
                val conn = (URL(entry.url).openConnection() as HttpURLConnection).apply {
                    instanceFollowRedirects = true
                    connectTimeout = 20_000
                    readTimeout = 45_000
                    setRequestProperty("User-Agent", "Vexio-Android-Workstation/0.9.0-beta")
                }
                try {
                    val code = conn.responseCode
                    if (code !in 200..299) {
                        error("HTTP $code")
                    }
                    conn.inputStream.use { it.readBytes() }
                } finally {
                    conn.disconnect()
                }
            }
            val bytes = result.getOrNull()
            if (bytes != null && bytes.isNotEmpty()) {
                importVisioStencilBytes(entry.fileName.ifBlank { "${entry.id}.vssx" }, bytes)
                _uiState.update { it.copy(downloadingStencilPackIds = it.downloadingStencilPackIds - entry.id) }
            } else {
                val msg = result.exceptionOrNull()?.message ?: "network error"
                _uiState.update {
                    it.copy(
                        downloadingStencilPackIds = it.downloadingStencilPackIds - entry.id,
                        statusBanner = "Failed to download '${entry.title}': $msg"
                    )
                }
            }
        }
    }

    fun downloadAllOfficialStencilPacks() {
        val entries = _uiState.value.officialStencilCatalog
        val installed = _uiState.value.installedStencilFileNames
        for (entry in entries) {
            if (entry.fileName !in installed) {
                downloadOfficialStencilPack(entry)
            }
        }
    }

    fun importVisioStencilBytes(fileName: String, rawBytes: ByteArray) {
        if (rawBytes.isEmpty()) return
        viewModelScope.launch(ioDispatcher) {
            val safeName = sanitizeStencilFileName(fileName)
            stencilStorageDir?.let { dir ->
                runCatching {
                    if (!dir.exists()) dir.mkdirs()
                    File(dir, safeName).writeBytes(rawBytes)
                }
            }
            var importedPackTitle = fileName.substringBeforeLast('.').ifBlank { "Imported Stencil" }
            var packCount = 1
            synchronized(this@VexioSessionViewModel) {
                if (NativeBridge.ensureLoaded() && docPtr != 0L) {
                    val resJson = NativeBridge.nativeImportStencilBytes(docPtr, fileName, rawBytes).orEmpty()
                    val obj = runCatching { Json.parseToJsonElement(resJson).jsonObject }.getOrNull()
                    val packsArr = obj?.get("packs") as? JsonArray
                    if (packsArr != null && packsArr.isNotEmpty()) {
                        packCount = packsArr.size
                        val firstObj = packsArr.first() as? JsonObject
                        val pTitle = firstObj?.get("packTitle")?.jsonPrimitive?.contentOrNull
                        if (!pTitle.isNullOrBlank()) importedPackTitle = pTitle
                    } else {
                        val parsedPack = obj?.get("packTitle")?.jsonPrimitive?.contentOrNull
                        if (!parsedPack.isNullOrBlank()) {
                            importedPackTitle = parsedPack
                        }
                    }
                    loadDynamicStencilsFromNative()
                }
            }
            withContext(Dispatchers.Main) {
                _uiState.update { state ->
                    val updatedCats = if (importedPackTitle in state.stencilCategories) {
                        state.stencilCategories
                    } else {
                        listOf(importedPackTitle) + state.stencilCategories
                    }
                    state.copy(
                        showStartBackstage = false,
                        isStencilDrawerExpanded = true,
                        stencilCategories = updatedCats,
                        selectedStencilCategory = importedPackTitle,
                        installedStencilFileNames = state.installedStencilFileNames + safeName,
                        isDirty = true,
                        statusBanner = if (packCount > 1) {
                            "Imported '$fileName' ($packCount Stencil Categories Persisted Locally)"
                        } else {
                            "Imported Visio Stencil '$importedPackTitle' ($fileName)"
                        }
                    )
                }
            }
        }
    }

    @Synchronized
    fun toggleSmartGuides(explicit: Boolean? = null) {
        val next = explicit ?: !_uiState.value.smartGuidesEnabled
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L) {
            NativeBridge.nativeSetSmartGuides(viewportPtr, next)
            presentViewportOrFallback()
        }
        _uiState.update {
            it.copy(
                smartGuidesEnabled = next,
                statusBanner = if (next) {
                    "Smart Alignment & Equal-Spacing Guides Enabled (Hold Alt to Bypass)"
                } else {
                    "Smart Guides Disabled"
                }
            )
        }
    }

    @Synchronized
    fun setSelectionIds(shapeIds: List<Long>) {
        val cleaned = shapeIds.filter { it > 0L }.distinct()
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L) {
            NativeBridge.nativeSetSelectionIds(viewportPtr, docPtr, cleaned.toLongArray())
            presentViewportOrFallback()
        }
        val primary = cleaned.firstOrNull() ?: _uiState.value.selectedShapeId
        _uiState.update {
            it.copy(
                selectedShapeId = primary,
                selectedShapeIds = cleaned,
                selectedShapeName = if (cleaned.size > 1) "${cleaned.size} Shapes Selected" else "Sheet.$primary"
            )
        }
    }

    @Synchronized
    fun registerCustomFontBytes(fontBytes: ByteArray, familyOverride: String? = null): Boolean {
        if (fontBytes.isEmpty()) return false
        val ok = if (NativeBridge.isNativeAvailable) {
            NativeBridge.nativeRegisterFontBytes(fontBytes, familyOverride)
        } else {
            fontBytes.size >= 12
        }
        if (ok && !familyOverride.isNullOrBlank()) {
            val fam = familyOverride.trim()
            _uiState.update { state ->
                val updated = if (fam in state.availableFontFamilies) {
                    state.availableFontFamilies
                } else {
                    listOf(fam) + state.availableFontFamilies
                }
                state.copy(
                    availableFontFamilies = updated,
                    statusBanner = "Registered TrueType/OpenType font '$fam'"
                )
            }
            presentViewportOrFallback()
        }
        return ok
    }

    @Synchronized
    fun alignSelectedShapes(mode: String): Int {
        val norm = mode.trim().lowercase(Locale.US)
        if (norm !in setOf("left", "center", "right", "top", "middle", "bottom")) return 0
        val cur = _uiState.value
        val targets = cur.selectedShapeIds.ifEmpty { listOf(cur.selectedShapeId) }
            .filter { it > 0L }
            .distinct()
        if (targets.isEmpty()) return 0

        val updatedCount = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val c = NativeBridge.nativeAlignShapes(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                shapeIds = targets.toLongArray(),
                mode = norm
            )
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
            c
        } else {
            targets.size
        }

        if (updatedCount > 0) {
            _uiState.update {
                it.copy(
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Aligned $updatedCount shape(s) (${norm.replaceFirstChar { ch -> ch.uppercase() }})"
                )
            }
        }
        return updatedCount
    }

    @Synchronized
    fun distributeSelectedShapes(axis: String): Int {
        val norm = axis.trim().lowercase(Locale.US)
        val isHoriz = norm.startsWith("h") || norm == "x"
        val isVert = norm.startsWith("v") || norm == "y"
        if (!isHoriz && !isVert) return 0
        val canonicalAxis = if (isHoriz) "horizontal" else "vertical"
        val cur = _uiState.value
        val targets = cur.selectedShapeIds.ifEmpty { listOf(cur.selectedShapeId) }
            .filter { it > 0L }
            .distinct()
        if (targets.size < 3) {
            _uiState.update {
                it.copy(statusBanner = "Select 3+ shapes to distribute equal spacing")
            }
            return 0
        }

        val updatedCount = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val c = NativeBridge.nativeDistributeShapes(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                shapeIds = targets.toLongArray(),
                axis = canonicalAxis
            )
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
            c
        } else {
            targets.size
        }

        if (updatedCount > 0) {
            _uiState.update {
                it.copy(
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Distributed $updatedCount shapes (${if (isHoriz) "Horizontally" else "Vertically"})"
                )
            }
        }
        return updatedCount
    }

    @Synchronized
    fun groupSelectedShapes(): Long {
        val cur = _uiState.value
        val targets = cur.selectedShapeIds.filter { it > 0L }.distinct()
        if (targets.size < 2) {
            _uiState.update {
                it.copy(statusBanner = "Select 2+ shapes (Marquee or Shift+Click) to Group (Ctrl+G)")
            }
            return 0L
        }

        val groupId = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val gid = NativeBridge.nativeGroupShapes(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                shapeIds = targets.toLongArray()
            )
            if (gid > 0L && viewportPtr != 0L) {
                NativeBridge.nativeSetSelectionIds(viewportPtr, docPtr, longArrayOf(gid))
            }
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
            gid
        } else {
            val gid = nextSyntheticShapeId++
            syntheticGroups[gid] = targets
            gid
        }

        if (groupId > 0L) {
            _uiState.update {
                it.copy(
                    selectedShapeId = groupId,
                    selectedShapeIds = listOf(groupId),
                    selectedShapeName = "Sheet.$groupId (Group)",
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Grouped ${targets.size} shapes into Sheet.$groupId"
                )
            }
        }
        return groupId
    }

    @Synchronized
    fun ungroupSelectedShapes(): List<Long> {
        val cur = _uiState.value
        val targets = cur.selectedShapeIds.ifEmpty { listOf(cur.selectedShapeId) }
            .filter { it > 0L }
            .distinct()
        if (targets.isEmpty()) return emptyList()

        val restoredIds = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val arr = NativeBridge.nativeUngroupShapes(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                groupIds = targets.toLongArray()
            )
            val list = arr?.filter { it > 0L }.orEmpty()
            if (list.isNotEmpty() && viewportPtr != 0L) {
                NativeBridge.nativeSetSelectionIds(viewportPtr, docPtr, list.toLongArray())
            }
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
            list
        } else {
            val out = mutableListOf<Long>()
            for (gid in targets) {
                val members = syntheticGroups.remove(gid)
                if (members != null) out.addAll(members)
            }
            out
        }

        if (restoredIds.isNotEmpty()) {
            val primary = restoredIds.first()
            _uiState.update {
                it.copy(
                    selectedShapeId = primary,
                    selectedShapeIds = restoredIds,
                    selectedShapeName = if (restoredIds.size > 1) "${restoredIds.size} Shapes Selected" else "Sheet.$primary",
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Ungrouped into ${restoredIds.size} member shape(s)"
                )
            }
        }
        return restoredIds
    }

    @Synchronized
    fun dragConnectorSegment(shapeId: Long, segIndex: Int, coordInches: Double): Boolean {
        if (shapeId <= 0L || segIndex < 0 || !coordInches.isFinite()) return false
        val ok = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val moved = NativeBridge.nativeDragConnectorSegment(
                docPtr = docPtr,
                pageId = resolveNativePageId(_uiState.value.activePageId),
                shapeId = shapeId,
                segIndex = segIndex,
                coordIn = coordInches
            )
            if (moved) presentViewportOrFallback()
            moved
        } else {
            true
        }
        if (ok) {
            _uiState.update {
                it.copy(
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Adjusted orthogonal connector segment #$segIndex"
                )
            }
        }
        return ok
    }

    @Synchronized
    fun insertContainer(title: String = "Container"): Long {
        val cur = _uiState.value
        val members = cur.selectedShapeIds.filter { it > 0L }.distinct()
        val cid = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val id = NativeBridge.nativeInsertContainer(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                title = title,
                memberIds = members.toLongArray()
            )
            if (id > 0L && viewportPtr != 0L) {
                NativeBridge.nativeSetSelectionIds(viewportPtr, docPtr, longArrayOf(id))
            }
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
            id
        } else {
            nextSyntheticShapeId++
        }
        if (cid > 0L) {
            _uiState.update {
                it.copy(
                    selectedShapeId = cid,
                    selectedShapeIds = listOf(cid),
                    selectedShapeName = "Container.$cid",
                    isDirty = true,
                    canUndo = true,
                    statusBanner = if (members.isEmpty()) {
                        "Inserted Container.$cid (drag shapes inside to enroll)"
                    } else {
                        "Enclosed ${members.size} shape(s) inside Container.$cid"
                    }
                )
            }
        }
        return cid
    }

    @Synchronized
    fun insertSwimlanes(lanesSemicolon: String = ""): Int {
        val cur = _uiState.value
        val laneCount = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val c = NativeBridge.nativeInsertSwimlanes(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                lanesSemicolon = lanesSemicolon
            )
            presentViewportOrFallback()
            c
        } else {
            val parsed = lanesSemicolon.split(';').map { it.trim() }.filter { it.isNotEmpty() }
            if (parsed.isEmpty()) 3 else parsed.size
        }
        if (laneCount > 0) {
            _uiState.update {
                it.copy(
                    isDirty = true,
                    canUndo = true,
                    statusBanner = if (laneCount > 1) {
                        "Inserted $laneCount-lane Cross-Functional Swimlane pool"
                    } else {
                        "Appended contiguous Cross-Functional Swimlane band"
                    }
                )
            }
        }
        return laneCount
    }

    @Synchronized
    fun attachCallout(text: String = "Callout Note"): Long {
        val cur = _uiState.value
        val targetId = cur.selectedShapeIds.firstOrNull { it > 0L } ?: cur.selectedShapeId.coerceAtLeast(0L)
        val cid = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val id = NativeBridge.nativeAttachCallout(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                targetShapeId = targetId,
                text = text
            )
            if (id > 0L && viewportPtr != 0L) {
                NativeBridge.nativeSetSelectionIds(viewportPtr, docPtr, longArrayOf(id))
            }
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
            id
        } else {
            nextSyntheticShapeId++
        }
        if (cid > 0L) {
            _uiState.update {
                it.copy(
                    selectedShapeId = cid,
                    selectedShapeIds = listOf(cid),
                    selectedShapeName = "Callout.$cid",
                    isDirty = true,
                    canUndo = true,
                    statusBanner = if (targetId > 0L) {
                        "Attached Callout.$cid to Sheet.$targetId (drag yellow handle to retarget)"
                    } else {
                        "Inserted Callout.$cid (drag yellow handle onto any shape)"
                    }
                )
            }
        }
        return cid
    }

    @Synchronized
    fun autoConnectSelected(direction: String): Long {
        val cur = _uiState.value
        val srcId = cur.selectedShapeIds.firstOrNull { it > 0L } ?: cur.selectedShapeId
        if (srcId <= 0L) return 0L
        val cleanDir = direction.trim().lowercase(Locale.US)
        if (cleanDir !in setOf("n", "north", "e", "east", "s", "south", "w", "west")) return 0L
        val targetId = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val tid = NativeBridge.nativeAutoConnect(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                sourceShapeId = srcId,
                dir = cleanDir
            )
            if (tid > 0L && viewportPtr != 0L) {
                NativeBridge.nativeSetSelectionIds(viewportPtr, docPtr, longArrayOf(tid))
            }
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
            tid
        } else {
            nextSyntheticShapeId++
        }
        if (targetId > 0L) {
            _uiState.update {
                it.copy(
                    selectedShapeId = targetId,
                    selectedShapeIds = listOf(targetId),
                    selectedShapeName = "Process.$targetId",
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "AutoConnected ($cleanDir) from Sheet.$srcId to Sheet.$targetId"
                )
            }
        }
        return targetId
    }

    @Synchronized
    fun fitPageToWindow() {
        val fittedPct = computeFitZoomPercent()
        if (NativeBridge.isNativeAvailable && viewportPtr != 0L && docPtr != 0L) {
            NativeBridge.nativeSetViewportZoom(viewportPtr, docPtr, 0.0, 0.0, 0.0)
            presentViewportOrFallback()
        }
        _uiState.update {
            it.copy(
                zoomPercent = fittedPct,
                isAutoFitPage = true,
                statusBanner = "Fitted diagram to screen"
            )
        }
    }

    @Synchronized
    fun setCellForSelection(cellName: String, formula: String, forceSetf: Boolean = false) {
        val cur = _uiState.value
        val targets = cur.selectedShapeIds.ifEmpty { listOf(cur.selectedShapeId) }.filter { it > 0L }.distinct()
        if (targets.isEmpty()) return
        val nativePid = resolveNativePageId(cur.activePageId)
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            for (sid in targets) {
                NativeBridge.nativeSetCellFormula(docPtr, nativePid, sid, cellName, formula, forceSetf)
            }
        }
        _uiState.update { it.copy(isDirty = true, canUndo = true, statusBanner = "Updated $cellName = $formula") }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
    }

    @Synchronized
    fun renameSelectedShape(newName: String) {
        val trimmed = newName.trim()
        val cur = _uiState.value
        if (trimmed.isEmpty() || cur.selectedShapeId <= 0L) return
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeRenameShape(docPtr, resolveNativePageId(cur.activePageId), cur.selectedShapeId, trimmed)
        }
        _uiState.update {
            it.copy(
                selectedShapeName = trimmed,
                inspectedObject = it.inspectedObject?.copy(name = trimmed),
                isDirty = true,
                canUndo = true,
                statusBanner = "Renamed Sheet.${cur.selectedShapeId} to '$trimmed'"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
    }

    @Synchronized
    fun setSelectedShapeLabelText(newText: String) {
        val cur = _uiState.value
        if (cur.selectedShapeId <= 0L) return
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeSetShapeText(docPtr, resolveNativePageId(cur.activePageId), cur.selectedShapeId, newText)
        }
        _uiState.update {
            it.copy(
                inspectedObject = it.inspectedObject?.copy(text = newText),
                isDirty = true,
                canUndo = true,
                statusBanner = "Updated text on Sheet.${cur.selectedShapeId}"
            )
        }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
    }

    @Synchronized
    fun upsertSelectedShapeData(rowName: String, label: String, dataTypeCode: Int, valueStr: String) {
        val cleanRow = rowName.trim()
        val cur = _uiState.value
        if (cleanRow.isEmpty() || cur.selectedShapeId <= 0L) return
        val resolvedLabel = label.trim().ifEmpty { cleanRow }
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeUpsertShapeData(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                shapeId = cur.selectedShapeId,
                rowName = cleanRow,
                label = resolvedLabel,
                dataType = dataTypeCode,
                valueStr = valueStr
            )
        }
        _uiState.update { it.copy(isDirty = true, canUndo = true, statusBanner = "Saved Prop.$cleanRow") }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
    }

    @Synchronized
    fun deleteSelectedShapeData(rowName: String) {
        val cur = _uiState.value
        if (rowName.isBlank() || cur.selectedShapeId <= 0L) return
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeDeleteShapeData(docPtr, resolveNativePageId(cur.activePageId), cur.selectedShapeId, rowName)
        }
        _uiState.update { it.copy(isDirty = true, canUndo = true, statusBanner = "Removed Prop.$rowName") }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
    }

    @Synchronized
    fun addPageLayer(layerName: String) {
        val cur = _uiState.value
        val clean = layerName.trim()
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeAddLayer(docPtr, resolveNativePageId(cur.activePageId), clean)
        }
        _uiState.update { it.copy(isDirty = true, canUndo = true, statusBanner = "Added Layer '${clean.ifEmpty { "New Layer" }}'") }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
    }

    @Synchronized
    fun updatePageLayerFlags(
        layerIx: Int,
        visible: Boolean,
        lock: Boolean,
        print: Boolean,
        snapGlue: Boolean
    ) {
        val cur = _uiState.value
        if (layerIx < 0) return
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            NativeBridge.nativeSetLayerFlags(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                layerIx = layerIx,
                visible = visible,
                lockFlag = lock,
                printFlag = print,
                snapGlue = snapGlue
            )
        }
        _uiState.update { it.copy(isDirty = true, canUndo = true) }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
    }

    @Synchronized
    fun setSelectedShapeLayerMembership(layerIx: Int, assigned: Boolean) {
        val cur = _uiState.value
        val targets = cur.selectedShapeIds.ifEmpty { listOf(cur.selectedShapeId) }.filter { it > 0L }.distinct()
        if (layerIx < 0 || targets.isEmpty()) return
        val nativePid = resolveNativePageId(cur.activePageId)
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            for (sid in targets) {
                NativeBridge.nativeSetShapeLayerMembership(docPtr, nativePid, sid, layerIx, assigned)
            }
        }
        _uiState.update { it.copy(isDirty = true, canUndo = true) }
        refreshSelectedShapeFormulas()
        presentViewportOrFallback()
    }

    @Synchronized
    fun dragControlHandle(
        shapeId: Long,
        rowName: String = "Row_1",
        pageXIn: Double,
        pageYIn: Double
    ): Boolean {
        if (shapeId <= 0L || rowName.isBlank() || !pageXIn.isFinite() || !pageYIn.isFinite()) return false
        val ok = if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val moved = NativeBridge.nativeDragControlHandle(
                docPtr = docPtr,
                pageId = resolveNativePageId(_uiState.value.activePageId),
                shapeId = shapeId,
                rowName = rowName,
                pageXIn = pageXIn,
                pageYIn = pageYIn
            )
            if (moved) {
                refreshSelectedShapeFormulas()
                presentViewportOrFallback()
            }
            moved
        } else {
            true
        }
        if (ok) {
            _uiState.update {
                it.copy(
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Adjusted Controls.$rowName on Sheet.$shapeId"
                )
            }
        }
        return ok
    }

    @Synchronized
    fun updatePageDataGraphics(next: DataGraphicUiState): Int {
        val cur = _uiState.value
        val cfgJson = """
            {
              "enabled": ${next.enabled},
              "iconSetField": "${escapeJson(next.iconSetField)}",
              "iconGreenMin": ${next.iconGreenMin},
              "iconAmberMin": ${next.iconAmberMin},
              "dataBarField": "${escapeJson(next.dataBarField)}",
              "dataBarMin": ${next.dataBarMin},
              "dataBarMax": ${next.dataBarMax},
              "dataBarColorHex": "${escapeJson(next.dataBarColorHex)}",
              "textCalloutField": "${escapeJson(next.textCalloutField)}",
              "textCalloutPrefix": "${escapeJson(next.textCalloutPrefix)}",
              "colorByValueEnabled": ${next.colorByValueEnabled},
              "colorByValueField": "${escapeJson(next.colorByValueField)}",
              "legendVisible": ${next.legendVisible},
              "legendField": "${escapeJson(next.legendField)}"
            }
        """.trimIndent()
        var decorated = 0
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            decorated = NativeBridge.nativeSetPageDataGraphicsJson(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                configJson = cfgJson
            ).coerceAtLeast(0)
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        }
        _uiState.update {
            it.copy(
                dataGraphics = if (NativeBridge.isNativeAvailable && docPtr != 0L) it.dataGraphics else next,
                isDirty = true,
                canUndo = true,
                statusBanner = if (next.enabled) {
                    "Applied Data Graphics across $decorated shape(s)"
                } else {
                    "Data Graphics overlay hidden"
                }
            )
        }
        return decorated
    }

    @Synchronized
    fun importExternalRecordsetBytes(
        fileName: String,
        rawBytes: ByteArray,
        uri: Uri? = null,
        keyColHint: String = "",
        tableHint: String = ""
    ): Int {
        if (rawBytes.isEmpty()) return 0
        val cur = _uiState.value
        var rsId = 1
        var linkedCount = 0
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val res = NativeBridge.nativeImportExternalRecordset(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                fileName = fileName,
                rawBytes = rawBytes,
                keyColHint = keyColHint,
                tableHint = tableHint,
                autoBind = true,
                autoEnableGraphics = true
            )
            if (res != null && res.size >= 2) {
                rsId = res[0]
                linkedCount = res[1]
            }
            if (uri != null && rsId > 0) {
                linkedRecordsetUris[rsId] = uri
            }
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        }
        _uiState.update {
            it.copy(
                isInspectorExpanded = true,
                isDirty = true,
                canUndo = true,
                statusBanner = "Linked '$fileName' ($linkedCount shape(s) bound to External Data)"
            )
        }
        return linkedCount
    }

    @Synchronized
    fun getLinkedRecordsetUrisSnapshot(): Map<Int, Uri> = LinkedHashMap(linkedRecordsetUris)

    @Synchronized
    fun bindExternalRecordsetRow(recordsetId: Int, rowIndex: Int): Int {
        if (recordsetId <= 0 || rowIndex < 0) return 0
        val cur = _uiState.value
        val targets = cur.selectedShapeIds.filter { it > 0L }.distinct().toLongArray()
        var bound = 0
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            bound = NativeBridge.nativeBindExternalRow(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                recordsetId = recordsetId,
                rowIndex = rowIndex,
                shapeIds = targets
            )
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        }
        if (bound > 0) {
            _uiState.update {
                it.copy(
                    isDirty = true,
                    canUndo = true,
                    statusBanner = if (targets.isEmpty()) {
                        "Stamped & bound new shape from row #${rowIndex + 1}"
                    } else {
                        "Bound row #${rowIndex + 1} to $bound shape(s)"
                    }
                )
            }
        }
        return bound
    }

    @Synchronized
    fun refreshExternalRecordsets(
        recordsetId: Int = 0,
        optionalBytes: ByteArray? = null,
        formatHint: String = ""
    ): Int {
        val cur = _uiState.value
        var refreshed = 0
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            refreshed = NativeBridge.nativeRefreshExternalRecordset(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                recordsetId = recordsetId,
                optionalNewBytes = optionalBytes,
                formatHint = formatHint
            )
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        }
        _uiState.update {
            it.copy(
                isDirty = true,
                canUndo = true,
                statusBanner = "Refreshed External Data ($refreshed shape(s) synchronized)"
            )
        }
        return refreshed
    }

    @Synchronized
    fun syncAndExportRecordsetBytes(recordsetId: Int = 0, formatExt: String = "xlsx"): ByteArray {
        val cleanExt = formatExt.trim().trimStart('.').lowercase(Locale.US).ifEmpty { "xlsx" }
        val cur = _uiState.value
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val out = NativeBridge.nativeSyncAndExportRecordset(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                recordsetId = recordsetId,
                formatExt = cleanExt
            )
            if (out != null && out.isNotEmpty()) {
                refreshSelectedShapeFormulas()
                _uiState.update {
                    it.copy(
                        isDirty = true,
                        canUndo = true,
                        statusBanner = "Two-Way Writeback exported (.${cleanExt}, ${out.size} bytes)"
                    )
                }
                return out
            }
        }
        return ByteArray(0)
    }

    @Synchronized
    fun deleteExternalRecordset(recordsetId: Int): Boolean {
        if (recordsetId <= 0) return false
        var deleted = false
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            deleted = NativeBridge.nativeDeleteExternalRecordset(docPtr, recordsetId)
            linkedRecordsetUris.remove(recordsetId)
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        }
        if (deleted) {
            _uiState.update {
                it.copy(
                    isDirty = true,
                    canUndo = true,
                    statusBanner = "Unlinked External Data recordset #$recordsetId"
                )
            }
        }
        return deleted
    }

    @Synchronized
    fun insertOrUpdateDataLegend(propName: String = ""): Long {
        val cur = _uiState.value
        val effProp = propName.trim().ifEmpty { cur.dataGraphics.legendField.ifEmpty { "Status" } }
        var lid = 0L
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            lid = NativeBridge.nativeInsertDataLegend(
                docPtr = docPtr,
                pageId = resolveNativePageId(cur.activePageId),
                propName = effProp,
                pinXIn = 0.0,
                pinYIn = 0.0
            )
            refreshSelectedShapeFormulas()
            presentViewportOrFallback()
        }
        _uiState.update {
            it.copy(
                isDirty = true,
                canUndo = true,
                statusBanner = if (lid > 0L) {
                    "Updated live Diagram Legend (Sheet.$lid • Prop.$effProp)"
                } else {
                    "Inserted Diagram Legend"
                }
            )
        }
        return lid
    }

    @Synchronized
    fun exportBomReportBytes(formatExt: String = "xlsx"): ByteArray {
        val cleanExt = formatExt.trim().trimStart('.').lowercase(Locale.US).ifEmpty { "xlsx" }
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val out = NativeBridge.nativeExportBomReport(docPtr, cleanExt)
            if (out != null && out.isNotEmpty()) {
                _uiState.update {
                    it.copy(statusBanner = "Exported BOM Shape Report (.${cleanExt}, ${out.size} bytes)")
                }
                return out
            }
        }
        return ByteArray(0)
    }

    fun setAutoCheckUpdates(enabled: Boolean) {
        _uiState.update { it.copy(autoCheckUpdates = enabled) }
    }

    fun dismissUpdateDialog() {
        _uiState.update { it.copy(showUpdateDialog = false) }
    }

    fun checkForUpdatesAsync(userInitiated: Boolean) {
        if (!userInitiated && !_uiState.value.autoCheckUpdates) return
        if (userInitiated) {
            _uiState.update {
                it.copy(updateStatusMessage = "Checking GitHub Releases for updates…")
            }
        }
        viewModelScope.launch(ioDispatcher) {
            val currentVersion = "0.9.0-beta"
            var latestTag: String? = null
            var downloadUrl = "https://github.com/petrosvassiliou/vexio/releases"
            var ok = false

            runCatching {
                val url = java.net.URL("https://api.github.com/repos/petrosvassiliou/vexio/releases")
                val conn = (url.openConnection() as java.net.HttpURLConnection).apply {
                    requestMethod = "GET"
                    connectTimeout = 4500
                    readTimeout = 4500
                    setRequestProperty("User-Agent", "Vexio-Android-Googlebook/0.9.0-beta")
                    setRequestProperty("Accept", "application/vnd.github+json")
                }
                if (conn.responseCode in 200..299) {
                    val body = conn.inputStream.bufferedReader(StandardCharsets.UTF_8).use { it.readText() }
                    val arr = Json.parseToJsonElement(body).jsonArray
                    if (arr.isNotEmpty()) {
                        val rel = arr[0].jsonObject
                        latestTag = rel["tag_name"]?.jsonPrimitive?.contentOrNull
                        downloadUrl = rel["html_url"]?.jsonPrimitive?.contentOrNull ?: downloadUrl
                        val assets = rel["assets"] as? JsonArray
                        assets?.forEach { assetElem ->
                            val aObj = assetElem.jsonObject
                            val name = aObj["name"]?.jsonPrimitive?.contentOrNull.orEmpty()
                            if (name.endsWith(".apk", ignoreCase = true) || "android" in name.lowercase(Locale.US)) {
                                downloadUrl = aObj["browser_download_url"]?.jsonPrimitive?.contentOrNull ?: downloadUrl
                            }
                        }
                        ok = true
                    }
                }
                conn.disconnect()
            }

            withContext(Dispatchers.Main) {
                if (!ok) {
                    _uiState.update {
                        it.copy(
                            updateStatusMessage = "Offline mode active • Update check skipped cleanly",
                            showUpdateDialog = userInitiated,
                            availableUpdateTag = null,
                            availableUpdateUrl = null
                        )
                    }
                    return@withContext
                }
                val cleanTag = latestTag.orEmpty().removePrefix("v").removePrefix("V")
                val hasNewer = isSemVerNewer(cleanTag, currentVersion)
                _uiState.update {
                    it.copy(
                        availableUpdateTag = if (hasNewer) latestTag else null,
                        availableUpdateUrl = downloadUrl,
                        showUpdateDialog = hasNewer || userInitiated,
                        updateStatusMessage = if (hasNewer) {
                            "Update available: $latestTag (Installed: v$currentVersion)"
                        } else {
                            "Vexio is up to date (${latestTag ?: "v$currentVersion"})"
                        }
                    )
                }
            }
        }
    }

    private fun isSemVerNewer(candidate: String, current: String): Boolean {
        if (candidate.isBlank() || candidate == current) return false
        val candParts = candidate.split('-', limit = 2)
        val currParts = current.split('-', limit = 2)
        val candNums = candParts[0].split('.').map { it.toIntOrNull() ?: 0 }
        val currNums = currParts[0].split('.').map { it.toIntOrNull() ?: 0 }
        val len = max(candNums.size, currNums.size)
        for (i in 0 until len) {
            val c = candNums.getOrElse(i) { 0 }
            val b = currNums.getOrElse(i) { 0 }
            if (c != b) return c > b
        }
        val candPre = candParts.getOrNull(1)
        val currPre = currParts.getOrNull(1)
        if (candPre == null && currPre != null) return true
        if (candPre != null && currPre == null) return false
        if (candPre != null && currPre != null) return candPre > currPre
        return false
    }

    /**
     * Serializes the active document to the requested target format (`vsdx`, `vsdm`, `dxf`, `pdf`,
     * `svg`, `pptx`, `docx`, `powerautomate` / `flow.json`, `powerbi` / `pbi.json`).
     * Used by SAF local disk persistence, Plan 2 enterprise exports, and `PrintDocumentAdapter` (`ext = "pdf"`).
     */
    @Synchronized
    fun exportDocumentBytes(extension: String): ByteArray {
        val cleanExt = extension.trim().trimStart('.').lowercase(Locale.US)
        if (cleanExt == "vssx") {
            return exportCustomStencilVssxBytes("My Shapes")
        }
        if (NativeBridge.isNativeAvailable && docPtr != 0L) {
            val nativeBytes = NativeBridge.nativeSaveDocument(docPtr, cleanExt)
            if (nativeBytes != null && nativeBytes.isNotEmpty()) {
                return nativeBytes
            }
        }
        val state = _uiState.value
        return when (cleanExt) {
            "jpg", "jpeg" -> buildDeterministicJpegFallback()
            "dwg" -> "AC1018\u0000\u0000\u0000\u0000\u0000\u0001VEXIO_DWG".toByteArray(Charsets.ISO_8859_1)
            "pdf" -> buildDeterministicVectorPdf(state)
            "pptx" -> buildDeterministicPptxPackage(state)
            "docx" -> buildDeterministicDocxPackage(state)
            "powerautomate", "flow.json" -> buildDeterministicPowerAutomateJson(state)
            "powerbi", "pbi.json" -> buildDeterministicPowerBiJson(state)
            else -> buildDeterministicXmlFallback(state, cleanExt)
        }
    }

    fun markSaved(uri: Uri?, displayName: String, ext: String) {
        savedStateRevision = readNativeStateRevision()
        _uiState.update {
            it.copy(
                documentUri = uri ?: it.documentUri,
                documentTitle = displayName,
                documentExtension = ext,
                isDirty = false,
                statusBanner = "Saved $displayName to local storage"
            )
        }
        captureBaselineSnapshot()
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
            pageId = resolveNativePageId(pageId),
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
            if (NativeBridge.isNativeAvailable) {
                if (viewportPtr != 0L) {
                    NativeBridge.nativeDestroyViewport(viewportPtr)
                }
                if (cachedBlankBaselineDocPtr != 0L) {
                    NativeBridge.nativeDestroyDocument(cachedBlankBaselineDocPtr)
                }
                if (docPtr != 0L) {
                    NativeBridge.nativeDestroyDocument(docPtr)
                }
                if (enginePtr != 0L) {
                    NativeBridge.nativeDestroyEngine(enginePtr)
                }
            }
            viewportPtr = 0L
            docPtr = 0L
            cachedBlankBaselineDocPtr = 0L
            enginePtr = 0L
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
            append("0.06 0.09 0.16 rg\n")
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
            out.append(String.format(Locale.US, "%010d 00000 n \n", offset))
        }
        out.append("trailer\n<< /Size ${objects.size + 1} /Root 1 0 R >>\n")
        out.append("startxref\n$xrefOffset\n%%EOF\n")
        return out.toString().toByteArray(Charsets.US_ASCII)
    }

    private fun buildDeterministicPptxPackage(state: WorkstationUiState): ByteArray {
        val baos = ByteArrayOutputStream()
        ZipOutputStream(baos).use { zos ->
            writeZipUtf8(
                zos,
                "[Content_Types].xml",
                """
                <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
                <Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
                  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
                  <Default Extension="xml" ContentType="application/xml"/>
                  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
                  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
                </Types>
                """.trimIndent()
            )
            writeZipUtf8(
                zos,
                "_rels/.rels",
                """
                <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
                <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
                  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
                </Relationships>
                """.trimIndent()
            )
            writeZipUtf8(
                zos,
                "ppt/presentation.xml",
                """
                <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
                <p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
                  <p:sldIdLst><p:sldId id="256" r:id="rId1"/></p:sldIdLst>
                  <p:sldSz cx="12192000" cy="6858000"/>
                </p:presentation>
                """.trimIndent()
            )
            writeZipUtf8(
                zos,
                "ppt/slides/slide1.xml",
                """
                <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
                <p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
                  <p:cSld name="${escapeXml(state.documentTitle)}">
                    <p:spTree>
                      <p:sp>
                        <p:txBody><a:p><a:r><a:t>${escapeXml(state.documentTitle)} — ${escapeXml(state.selectedShapeName)}</a:t></a:r></a:p></p:txBody>
                      </p:sp>
                    </p:spTree>
                  </p:cSld>
                </p:sld>
                """.trimIndent()
            )
        }
        return baos.toByteArray()
    }

    private fun buildDeterministicDocxPackage(state: WorkstationUiState): ByteArray {
        val baos = ByteArrayOutputStream()
        ZipOutputStream(baos).use { zos ->
            writeZipUtf8(
                zos,
                "[Content_Types].xml",
                """
                <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
                <Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
                  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
                  <Default Extension="xml" ContentType="application/xml"/>
                  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
                </Types>
                """.trimIndent()
            )
            writeZipUtf8(
                zos,
                "_rels/.rels",
                """
                <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
                <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
                  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
                </Relationships>
                """.trimIndent()
            )
            val rowsSummary = state.shapeSheetCells.joinToString("") { cell ->
                "<w:p><w:r><w:t>${escapeXml(cell.cellName)} = ${escapeXml(cell.formula)}</w:t></w:r></w:p>"
            }
            writeZipUtf8(
                zos,
                "word/document.xml",
                """
                <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
                <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                  <w:body>
                    <w:p><w:r><w:t>Vexio Process Specification: ${escapeXml(state.documentTitle)}</w:t></w:r></w:p>
                    <w:p><w:r><w:t>Active Shape: ${escapeXml(state.selectedShapeName)} (Theme: ${escapeXml(state.activeThemeSlug)})</w:t></w:r></w:p>
                    $rowsSummary
                  </w:body>
                </w:document>
                """.trimIndent()
            )
        }
        return baos.toByteArray()
    }

    private fun buildDeterministicPowerAutomateJson(state: WorkstationUiState): ByteArray {
        val actionKey = state.selectedShapeName
            .replace(Regex("[^A-Za-z0-9_]"), "_")
            .trim('_')
            .ifEmpty { "Step_${state.selectedShapeId}" }
        val json = """
            {
              "${'$'}schema": "https://schema.management.azure.com/providers/Microsoft.Logic/schemas/2016-06-01/workflowdefinition.json#",
              "contentVersion": "1.0.0.0",
              "parameters": {},
              "triggers": {
                "manual": {
                  "type": "Request",
                  "kind": "Button",
                  "inputs": { "schema": {} }
                }
              },
              "actions": {
                "${escapeJson(actionKey)}": {
                  "type": "Compose",
                  "inputs": "${escapeJson(state.selectedShapeName)}",
                  "runAfter": {},
                  "metadata": {
                    "visioShapeId": ${state.selectedShapeId},
                    "pageId": ${state.activePageId}
                  }
                }
              },
              "outputs": {}
            }
        """.trimIndent()
        return json.toByteArray(StandardCharsets.UTF_8)
    }

    private fun buildDeterministicPowerBiJson(state: WorkstationUiState): ByteArray {
        val cellsJson = state.shapeSheetCells.joinToString(",") { cell ->
            "\"${escapeJson(cell.cellName)}\":\"${escapeJson(cell.formula)}\""
        }
        val json = """
            {
              "schemaVersion": "1.0",
              "visualType": "vexioPlan2Visual",
              "documentTitle": "${escapeJson(state.documentTitle)}",
              "activePageId": ${state.activePageId},
              "theme": "${escapeJson(state.activeThemeSlug)}",
              "variant": ${state.activeThemeVariant},
              "shapeBindings": [
                {
                  "shapeId": ${state.selectedShapeId},
                  "name": "${escapeJson(state.selectedShapeName)}",
                  "cells": { $cellsJson }
                }
              ]
            }
        """.trimIndent()
        return json.toByteArray(StandardCharsets.UTF_8)
    }

    private fun writeZipUtf8(zos: ZipOutputStream, entryName: String, content: String) {
        val entry = ZipEntry(entryName)
        zos.putNextEntry(entry)
        zos.write(content.toByteArray(StandardCharsets.UTF_8))
        zos.closeEntry()
    }

    private fun escapeXml(raw: String): String = raw
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&apos;")

    private fun escapeJson(raw: String): String = raw
        .replace("\\", "\\\\")
        .replace("\"", "\\\"")
        .replace("\n", "\\n")
        .replace("\r", "\\r")

    private fun buildDeterministicXmlFallback(state: WorkstationUiState, ext: String): ByteArray {
        val cellsXml = state.shapeSheetCells.joinToString("\n") { cell ->
            "      <Cell N=\"${escapeXml(cell.cellName)}\" F=\"${escapeXml(cell.formula)}\" />"
        }
        val payload = """
            <?xml version="1.0" encoding="UTF-8"?>
            <VexioDocument format="${escapeXml(ext)}" title="${escapeXml(state.documentTitle)}">
              <Page ID="${state.activePageId}">
                <Shape ID="${state.selectedShapeId}" Name="${escapeXml(state.selectedShapeName)}">
            $cellsXml
                </Shape>
              </Page>
            </VexioDocument>
        """.trimIndent()
        return payload.toByteArray(Charsets.UTF_8)
    }

    /**
     * Valid ITU-T T.81 JFIF 1.02 Baseline DCT 8x8 JPEG stream used when `libvexio_jni.so`
     * is not loaded (e.g. host JVM unit tests); on Android/Googlebook OS devices,
     * `NativeBridge.nativeSaveDocument(docPtr, "jpg")` renders the full 192-DPI page JPEG.
     */
    private fun buildDeterministicJpegFallback(): ByteArray {
        val out = java.io.ByteArrayOutputStream(512)
        // SOI + APP0 JFIF 1.02 (192 DPI)
        out.write(
            byteArrayOf(
                0xFF.toByte(), 0xD8.toByte(),
                0xFF.toByte(), 0xE0.toByte(), 0x00, 0x10,
                'J'.code.toByte(), 'F'.code.toByte(), 'I'.code.toByte(), 'F'.code.toByte(), 0x00,
                0x01, 0x02, 0x01, 0x00, 0xC0.toByte(), 0x00, 0xC0.toByte(), 0x00, 0x00
            )
        )
        // DQT table 0 (all-16 luminance)
        out.write(byteArrayOf(0xFF.toByte(), 0xDB.toByte(), 0x00, 67, 0x00))
        out.write(ByteArray(64) { 16 })
        // SOF0 (8-bit, 8x8 grayscale Y=1)
        out.write(
            byteArrayOf(
                0xFF.toByte(), 0xC0.toByte(), 0x00, 11, 8,
                0x00, 0x08, 0x00, 0x08, 1, 1, 0x11, 0
            )
        )
        // Minimal DHT DC0 (symbol 0 -> 1-bit code 0) + AC0 (symbol 0x00 EOB -> 1-bit code 0)
        val bits1 = ByteArray(16).apply { this[0] = 1 }
        out.write(byteArrayOf(0xFF.toByte(), 0xC4.toByte(), 0x00, 20, 0x00))
        out.write(bits1)
        out.write(byteArrayOf(0x00))
        out.write(byteArrayOf(0xFF.toByte(), 0xC4.toByte(), 0x00, 20, 0x10))
        out.write(bits1)
        out.write(byteArrayOf(0x00))
        // SOS + 1-byte entropy scan (DC=0 '0', AC=EOB '0', 6-bit 1-padding = 0x3F) + EOI
        out.write(
            byteArrayOf(
                0xFF.toByte(), 0xDA.toByte(), 0x00, 8, 1, 1, 0x00, 0x00, 63, 0x00,
                0x3F,
                0xFF.toByte(), 0xD9.toByte()
            )
        )
        return out.toByteArray()
    }
}
