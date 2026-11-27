package com.vexio.core

import android.view.KeyEvent
import android.view.MotionEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.ByteArrayInputStream
import java.lang.reflect.Modifier
import java.nio.ByteOrder
import java.util.zip.ZipInputStream

class NativeBridgePacketTest {

    @Test
    fun directBatchBufferWritesExact32ByteNativeStride() {
        val buffer = MotionBatchFormat.allocateDirectBatchBuffer()
        assertTrue(buffer.isDirect)
        assertEquals(ByteOrder.nativeOrder(), buffer.order())
        assertEquals(32 * 256, buffer.capacity())

        val flags = MotionBatchFormat.packFlags(
            actionCode = MotionBatchFormat.ACTION_PREDICTED_MOVE,
            toolCode = MotionBatchFormat.TOOL_STYLUS,
            androidButtonState = MotionEvent.BUTTON_STYLUS_PRIMARY,
            androidMetaState = KeyEvent.META_CTRL_ON or KeyEvent.META_SHIFT_ON
        )

        MotionBatchFormat.writeSample(
            buffer = buffer,
            sampleIndex = 3,
            x = 512.25f,
            y = 384.75f,
            pressure = 0.82f,
            tiltRad = 0.45f,
            orientationRad = -1.12f,
            packedFlags = flags,
            timestampMicros = 987_654_321L
        )

        val base = 3 * MotionBatchFormat.SAMPLE_STRIDE_BYTES
        assertEquals(512.25f, buffer.getFloat(base), 1e-6f)
        assertEquals(384.75f, buffer.getFloat(base + 4), 1e-6f)
        assertEquals(0.82f, buffer.getFloat(base + 8), 1e-6f)
        assertEquals(0.45f, buffer.getFloat(base + 12), 1e-6f)
        assertEquals(-1.12f, buffer.getFloat(base + 16), 1e-6f)
        assertEquals(flags, buffer.getInt(base + 20))
        assertEquals(987_654_321L, buffer.getLong(base + 24))

        assertEquals(MotionBatchFormat.ACTION_PREDICTED_MOVE, MotionBatchFormat.unpackAction(flags))
        assertEquals(MotionBatchFormat.TOOL_STYLUS, MotionBatchFormat.unpackTool(flags))
        assertTrue((flags and MotionBatchFormat.BUTTON_STYLUS_PRIMARY_BIT) != 0)
        assertTrue((flags and MotionBatchFormat.MOD_CTRL_BIT) != 0)
        assertTrue((flags and MotionBatchFormat.MOD_SHIFT_BIT) != 0)
    }

    @Test
    fun stylusPalmRejectorBlocksCapacitivePalmButAllowsHardwareMouse() {
        val rejector = StylusPalmRejector(palmGuardWindowMillis = 320L)

        // Finger allowed before stylus enters proximity
        assertFalse(
            rejector.shouldRejectEvent(
                toolCode = MotionBatchFormat.TOOL_FINGER,
                actionMasked = MotionEvent.ACTION_DOWN,
                eventTimeMillis = 1_000L
            )
        )

        // USI stylus enters hover proximity
        assertFalse(
            rejector.shouldRejectEvent(
                toolCode = MotionBatchFormat.TOOL_STYLUS,
                actionMasked = MotionEvent.ACTION_HOVER_ENTER,
                eventTimeMillis = 2_000L
            )
        )
        assertTrue(rejector.isStylusProximityActive)

        // Resting palm touch during stylus hover must be rejected
        assertTrue(
            rejector.shouldRejectEvent(
                toolCode = MotionBatchFormat.TOOL_FINGER,
                actionMasked = MotionEvent.ACTION_DOWN,
                eventTimeMillis = 2_050L
            )
        )

        // Hardware mouse remains unaffected even while stylus hovers
        assertFalse(
            rejector.shouldRejectEvent(
                toolCode = MotionBatchFormat.TOOL_MOUSE,
                actionMasked = MotionEvent.ACTION_HOVER_MOVE,
                eventTimeMillis = 2_060L
            )
        )

        // Stylus touches down, strokes, and lifts at t = 2_500ms
        assertFalse(
            rejector.shouldRejectEvent(
                toolCode = MotionBatchFormat.TOOL_STYLUS,
                actionMasked = MotionEvent.ACTION_DOWN,
                eventTimeMillis = 2_200L
            )
        )
        assertFalse(
            rejector.shouldRejectEvent(
                toolCode = MotionBatchFormat.TOOL_STYLUS,
                actionMasked = MotionEvent.ACTION_UP,
                eventTimeMillis = 2_500L
            )
        )

        // Trailing palm touch within 320ms guard window (t = 2_700ms) is still rejected
        assertTrue(
            rejector.shouldRejectEvent(
                toolCode = MotionBatchFormat.TOOL_FINGER,
                actionMasked = MotionEvent.ACTION_DOWN,
                eventTimeMillis = 2_700L
            )
        )

        // Deliberate finger touch after guard window expires (t = 2_900ms > 2_500 + 320) is accepted
        assertFalse(
            rejector.shouldRejectEvent(
                toolCode = MotionBatchFormat.TOOL_FINGER,
                actionMasked = MotionEvent.ACTION_DOWN,
                eventTimeMillis = 2_900L
            )
        )
    }

    @Test
    fun guardedCellBlocksDirectOverwriteUntilSetfOverrideAndSupportsUndoRedo() {
        val vm = VexioSessionViewModel()

        // Width starts as GUARD(2.50 in)
        val blocked = vm.commitCellFormula(
            cellName = "Width",
            formula = "4.00 in",
            forceSetf = false
        )
        assertEquals(FormulaCommitStatus.GUARDED_CELL_PROTECTED, blocked)

        // With SETF override enabled, GUARD cell updates cleanly
        val overridden = vm.commitCellFormula(
            cellName = "Width",
            formula = "4.00 in",
            forceSetf = true
        )
        assertEquals(FormulaCommitStatus.OK, overridden)
        assertEquals(
            "4.00 in",
            vm.uiState.value.shapeSheetCells.first { it.cellName == "Width" }.formula
        )

        // Undo restores GUARD(2.50 in)
        vm.performUndo()
        val afterUndo = vm.uiState.value.shapeSheetCells.first { it.cellName == "Width" }
        assertEquals("GUARD(2.50 in)", afterUndo.formula)
        assertTrue(afterUndo.isGuarded)

        // Redo reapplies 4.00 in
        vm.performRedo()
        val afterRedo = vm.uiState.value.shapeSheetCells.first { it.cellName == "Width" }
        assertEquals("4.00 in", afterRedo.formula)
        assertFalse(afterRedo.isGuarded)
    }

    @Test
    fun vectorPdfExportProducesValidPdf14StreamForPrintManager() {
        val vm = VexioSessionViewModel()
        val pdfBytes = vm.exportDocumentBytes("pdf")
        val ascii = String(pdfBytes, Charsets.US_ASCII)
        assertTrue(ascii.startsWith("%PDF-1.4\n"))
        assertTrue(ascii.contains("/Type /Page"))
        assertTrue(ascii.contains("xref\n"))
        assertTrue(ascii.trimEnd().endsWith("%%EOF"))
    }

    @Test
    fun cursorMappingCoversAllHardwarePointerStates() {
        assertEquals(VexioCursorKind.ARROW, VexioCursorKind.fromNativeCode(0))
        assertEquals(VexioCursorKind.CROSSHAIR, VexioCursorKind.fromNativeCode(1))
        assertEquals(VexioCursorKind.MOVE_ALL, VexioCursorKind.fromNativeCode(2))
        assertEquals(VexioCursorKind.RESIZE_NS, VexioCursorKind.fromNativeCode(3))
        assertEquals(VexioCursorKind.RESIZE_EW, VexioCursorKind.fromNativeCode(4))
        assertEquals(VexioCursorKind.RESIZE_NWSE, VexioCursorKind.fromNativeCode(5))
        assertEquals(VexioCursorKind.RESIZE_NESW, VexioCursorKind.fromNativeCode(6))
        assertEquals(VexioCursorKind.TEXT_IBEAM, VexioCursorKind.fromNativeCode(7))
        assertEquals(VexioCursorKind.GRAB_HAND, VexioCursorKind.fromNativeCode(8))
        assertEquals(VexioCursorKind.GRABBING_HAND, VexioCursorKind.fromNativeCode(9))
        assertEquals(VexioCursorKind.GLUE_SNAP, VexioCursorKind.fromNativeCode(10))
        assertEquals(VexioCursorKind.FORBIDDEN_GUARD, VexioCursorKind.fromNativeCode(11))
        assertEquals(VexioCursorKind.ARROW, VexioCursorKind.fromNativeCode(999))
    }

    @Test
    fun plan2JniExternalDeclarationsMatchRustBridgeOneToOne() {
        val cls = NativeBridge::class.java
        val longT = Long::class.javaPrimitiveType!!
        val intT = Int::class.javaPrimitiveType!!
        val doubleT = Double::class.javaPrimitiveType!!
        val boolT = Boolean::class.javaPrimitiveType!!

        fun verifyNative(name: String, returnType: Class<*>, vararg params: Class<*>) {
            val method = cls.getDeclaredMethod(name, *params)
            assertTrue("$name must be native", Modifier.isNative(method.modifiers))
            assertTrue("$name must be static (@JvmStatic)", Modifier.isStatic(method.modifiers))
            assertEquals(returnType, method.returnType)
        }

        verifyNative("nativeSetViewportTool", Void.TYPE, longT, intT)
        verifyNative("nativeSetViewportPage", Void.TYPE, longT, longT, intT)
        verifyNative("nativeSetViewportZoom", Void.TYPE, longT, longT, doubleT, doubleT, doubleT)
        verifyNative("nativeRenderViewportFrame", boolT, longT, longT)
        verifyNative("nativeRenderViewportRgba", intT, longT, longT, java.nio.ByteBuffer::class.java)
        verifyNative("nativeGetPrimarySelection", longT, longT)
        verifyNative("nativeListStencilsJson", String::class.java, longT)
        verifyNative("nativeLoadStarterTemplate", boolT, longT, String::class.java)
        verifyNative("nativeDropStencil", longT, longT, intT, String::class.java, String::class.java, doubleT, doubleT)
        verifyNative("nativeDrawPrimitive", longT, longT, intT, String::class.java, doubleT, doubleT, doubleT, doubleT)
        verifyNative("nativeDeleteShapes", intT, longT, intT, LongArray::class.java)
        verifyNative("nativeDuplicateShapes", longT, longT, intT, LongArray::class.java, doubleT, doubleT)
        verifyNative("nativeReorderShapes", intT, longT, intT, LongArray::class.java, String::class.java)
        verifyNative("nativeBooleanOp", intT, longT, intT, LongArray::class.java, String::class.java, doubleT)
        verifyNative("nativeApplyTheme", intT, longT, String::class.java, intT)
        verifyNative("nativeApplyVisualPreset", boolT, longT, intT, longT, String::class.java)
        verifyNative("nativeCreateSubprocess", intT, longT, intT, longT, String::class.java)
        verifyNative("nativeCompareDocuments", String::class.java, longT, longT)
        verifyNative("nativeApplyCrdtJson", intT, longT, String::class.java, String::class.java)
        verifyNative("nativeImportDataVisualizer", intT, longT, intT, ByteArray::class.java)
        verifyNative("nativeImportCloudDiscovery", intT, longT, intT, String::class.java)
        verifyNative("nativeImportEntraOrLdif", intT, longT, intT, String::class.java)
        verifyNative("nativeGetStateJson", String::class.java, longT, intT)
        verifyNative("nativeSetShapeText", boolT, longT, intT, longT, String::class.java)
        verifyNative("nativeGetViewportTool", intT, longT)
        verifyNative("nativePopCreatedTextShape", longT, longT)
        verifyNative("nativeHitTestShape", longT, longT, longT, doubleT, doubleT)
        verifyNative("nativeGetShapeScreenBoundsJson", String::class.java, longT, longT, longT)
        verifyNative("nativeImportStencilBytes", String::class.java, longT, String::class.java, ByteArray::class.java)
        verifyNative("nativeSetShapeTextStyleJson", intT, longT, intT, LongArray::class.java, String::class.java)
        verifyNative("nativeRotateFlipShapes", intT, longT, intT, LongArray::class.java, doubleT, boolT, boolT)
        verifyNative("nativeSetPageSetup", boolT, longT, intT, doubleT, doubleT, String::class.java)
        verifyNative("nativeSetPagePreset", boolT, longT, intT, String::class.java, boolT)
        verifyNative("nativeSetDocumentTitle", boolT, longT, String::class.java)
        verifyNative("nativeRenamePage", boolT, longT, intT, String::class.java)
        verifyNative("nativeConnectShapes", longT, longT, intT, longT, longT)
        verifyNative("nativeValidateJson", String::class.java, longT, String::class.java)
        verifyNative("nativeRegisterFontBytes", boolT, ByteArray::class.java, String::class.java)
        verifyNative("nativeAlignShapes", intT, longT, intT, LongArray::class.java, String::class.java)
        verifyNative("nativeDistributeShapes", intT, longT, intT, LongArray::class.java, String::class.java)
        verifyNative("nativeGroupShapes", longT, longT, intT, LongArray::class.java)
        verifyNative("nativeUngroupShapes", LongArray::class.java, longT, intT, LongArray::class.java)
        verifyNative("nativeDragConnectorSegment", boolT, longT, intT, longT, intT, doubleT)
        verifyNative("nativeSetSmartGuides", boolT, longT, boolT)
        verifyNative("nativeGetSmartGuides", boolT, longT)
        verifyNative("nativeGetSelectionIds", LongArray::class.java, longT)
        verifyNative("nativeSetSelectionIds", boolT, longT, longT, LongArray::class.java)
        verifyNative("nativeImportMarkup", intT, longT, intT, String::class.java, String::class.java)
        verifyNative("nativeExportMarkup", String::class.java, longT, intT, String::class.java)
        verifyNative("nativeListSlideSnippetsJson", String::class.java, longT)
        verifyNative("nativeAddSlideSnippet", intT, longT, intT, String::class.java, String::class.java, doubleT, doubleT, doubleT, doubleT)
        verifyNative("nativeDeleteSlideSnippet", boolT, longT, intT, intT)
        verifyNative("nativeAutoSlideSnippets", intT, longT, intT)
        verifyNative("nativeResetConnectorRouting", boolT, longT, intT, longT)
        verifyNative("nativeEnsureControlHandle", boolT, longT, intT, longT)
        verifyNative("nativeRunVbaMacro", String::class.java, longT, LongArray::class.java, String::class.java)
        verifyNative("nativeListCommentsJson", String::class.java, longT, intT)
        verifyNative("nativeAddComment", intT, longT, intT, longT, String::class.java, String::class.java)
        verifyNative("nativeResolveComment", boolT, longT, intT, boolT)
        verifyNative("nativeDeleteComment", boolT, longT, intT)
        verifyNative("nativeClearPageInk", intT, longT, intT)
        verifyNative("nativeSetHyperlinkAndScreentip", boolT, longT, intT, longT, String::class.java, String::class.java, String::class.java, String::class.java)
        verifyNative("nativeRunDomainWizard", String::class.java, longT, intT, String::class.java, String::class.java)
        verifyNative("nativeApplyCadScale", doubleT, longT, intT, String::class.java, boolT)
        verifyNative("nativeConvertCadLayers", intT, longT, intT, String::class.java)
        verifyNative("nativeListShapeSheetFunctionsJson", String::class.java)
        verifyNative("nativeListShapeActionsJson", String::class.java, longT, intT, longT)
        verifyNative("nativeUpsertShapeAction", boolT, longT, intT, longT, String::class.java, String::class.java, String::class.java, String::class.java, String::class.java, boolT)
        verifyNative("nativeDeleteShapeAction", boolT, longT, intT, longT, String::class.java)
        verifyNative("nativeExecuteShapeAction", String::class.java, longT, intT, longT, String::class.java)
        verifyNative("nativeListConnectionPointsJson", String::class.java, longT, intT, longT)
        verifyNative("nativeAddConnectionPoint", intT, longT, intT, longT, doubleT, doubleT, intT)
        verifyNative("nativeMoveConnectionPoint", boolT, longT, intT, longT, intT, doubleT, doubleT)
        verifyNative("nativeDeleteConnectionPoint", boolT, longT, intT, longT, intT)
        verifyNative("nativeAddToCustomStencil", String::class.java, longT, intT, LongArray::class.java, String::class.java, String::class.java)
        verifyNative("nativeRenameCustomMaster", boolT, longT, String::class.java, String::class.java, String::class.java)
        verifyNative("nativeDeleteCustomMaster", boolT, longT, String::class.java, String::class.java)
        verifyNative("nativeUpdateCustomMaster", intT, longT, intT, longT, String::class.java, String::class.java)
        verifyNative("nativeExportCustomStencilVssx", ByteArray::class.java, longT, String::class.java)
        verifyNative("nativeListArrowheadsJson", String::class.java)
        verifyNative("nativeSetConnectorArrowheads", intT, longT, intT, LongArray::class.java, intT, intT, intT)
        verifyNative("nativeSetPageLineJumps", boolT, longT, intT, intT, intT, doubleT, doubleT)
        verifyNative("nativeInsertTextField", intT, longT, intT, longT, String::class.java, String::class.java, String::class.java)
        verifyNative("nativeListTextFieldsJson", String::class.java, longT, intT, longT)
        verifyNative("nativeSetShapeRichRunsJson", boolT, longT, intT, longT, String::class.java)
        verifyNative("nativeSetTextBlockTransform", boolT, longT, intT, longT, doubleT, doubleT, doubleT, doubleT, doubleT)
        verifyNative("nativeListRulerGuidesJson", String::class.java, longT, intT)
        verifyNative("nativeAddRulerGuide", longT, longT, intT, boolT, doubleT)
        verifyNative("nativeMoveRulerGuide", boolT, longT, intT, longT, doubleT)
        verifyNative("nativeDeleteRulerGuide", boolT, longT, intT, longT)
        verifyNative("nativeGetSnapGlueJson", String::class.java, longT, intT)
        verifyNative("nativeSetSnapGlue", boolT, longT, intT, boolT, boolT, intT, intT)
        verifyNative("nativeFindInDocumentJson", String::class.java, longT, intT, String::class.java, boolT, boolT, boolT)
        verifyNative("nativeReplaceInDocument", intT, longT, intT, String::class.java, String::class.java, boolT, boolT, boolT)
        verifyNative("nativeCopyShapeFormatJson", String::class.java, longT, intT, longT)
        verifyNative("nativeApplyFormatPainter", intT, longT, intT, LongArray::class.java, String::class.java)
        verifyNative("nativeNumberShapes", intT, longT, intT, LongArray::class.java, String::class.java, intT, intT, String::class.java, String::class.java)
        verifyNative("nativeInsertAssociativeDimension", longT, longT, intT, String::class.java, longT, longT, doubleT, String::class.java)
        verifyNative("nativeDrawArchitecturalWall", longT, longT, intT, doubleT, doubleT, doubleT, doubleT, doubleT)
        verifyNative("nativeHealWallIntersections", intT, longT, intT, doubleT)
        verifyNative("nativeInsertWallOpening", longT, longT, intT, longT, String::class.java, doubleT, doubleT, String::class.java)
        verifyNative("nativeAddMindMapTopic", longT, longT, intT, longT, String::class.java, boolT)
        verifyNative("nativeExportMindMapOutline", String::class.java, longT, intT)
        verifyNative("nativeRecalculateVsmAndFtaJson", String::class.java, longT, intT)
    }

    @Test
    fun jpegExportProducesValidJfifStream() {
        val vm = VexioSessionViewModel()
        val jpgBytes = vm.exportDocumentBytes("jpg")
        assertTrue(jpgBytes.size > 100)
        assertEquals(0xFF.toByte(), jpgBytes[0])
        assertEquals(0xD8.toByte(), jpgBytes[1])
        assertEquals(0xFF.toByte(), jpgBytes[2])
        assertEquals(0xE0.toByte(), jpgBytes[3])
        assertEquals("JFIF", String(jpgBytes, 6, 4, Charsets.US_ASCII))
        assertEquals(0xFF.toByte(), jpgBytes[jpgBytes.size - 2])
        assertEquals(0xD9.toByte(), jpgBytes[jpgBytes.size - 1])
    }

    @Test
    fun lightModeDefaultCleanWorkspaceSixteenToolsAndTwentyOneStencilCategories() {
        val vm = VexioSessionViewModel()
        val state = vm.uiState.value

        // Light mode default and uncluttered workspace defaults
        assertFalse(state.isDarkTheme)
        assertFalse(state.isPlan2RibbonExpanded)
        assertFalse(state.isShapeSheetDrawerExpanded)
        assertTrue(state.isStencilDrawerExpanded)
        assertTrue(state.isInspectorExpanded)

        // All 23 drawing, wall, dimension, vertex, connection point, text block, format painter & markup tools map 1:1 to engine tool codes 0..22
        assertEquals(23, VexioTool.entries.size)
        assertEquals((0..22).toSet(), VexioTool.entries.map { it.nativeCode }.toSet())

        // All 33 Visio Professional & Awesome-Visio stencil categories are present
        assertEquals(33, BuiltInStencilCatalog.categories.size)
        assertEquals(33, state.stencilCategories.size)
        val distinctCategories = state.stencilMasters.map { it.category }.distinct()
        assertEquals(33, distinctCategories.size)
        assertTrue(state.stencilCategories.contains("Microsoft 365 & Office"))
        assertTrue(state.stencilCategories.contains("Microsoft Servers & Active Directory"))
        assertTrue(state.stencilCategories.contains("Microsoft Azure & Hybrid Cloud"))
        assertTrue(state.stencilCategories.contains("Microsoft Power Platform & Dynamics 365"))
        assertTrue(state.stencilCategories.contains("Servers (Standard Roles)"))
        assertTrue(state.stencilCategories.contains("Network & Rack Equipment"))

        // Theme toggle flips deterministically
        vm.toggleDarkTheme()
        assertTrue(vm.uiState.value.isDarkTheme)
        vm.toggleDarkTheme()
        assertFalse(vm.uiState.value.isDarkTheme)
    }

    @Test
    fun freeformToolAndConstructiveBooleanOperationsUpdateStateDeterministically() {
        val vm = VexioSessionViewModel()

        // Verify Freeform Curve tool (Ctrl+5)
        vm.selectTool(VexioTool.FREEFORM)
        assertEquals(VexioTool.FREEFORM, vm.uiState.value.activeTool)
        assertEquals("Freeform Curve", VexioTool.FREEFORM.displayName)
        assertEquals("Ctrl+5", VexioTool.FREEFORM.shortcutHint)
        assertEquals(VexioCursorKind.CROSSHAIR, vm.uiState.value.activeCursor)

        // Binary boolean op requires >= 2 shapes
        assertEquals(0, vm.performBooleanOp("Union", longArrayOf(1L)))

        // Union, Combine, Intersect, Subtract, Join produce 1 composite shape
        for (opName in listOf("Union", "Combine", "Intersect", "Subtract", "Join")) {
            val count = vm.performBooleanOp(opName, longArrayOf(1L, 2L))
            assertEquals("Expected 1 composite shape for $opName", 1, count)
            assertEquals(opName.lowercase(), vm.uiState.value.lastBooleanOp)
        }

        // Fragment on 2 overlapping shapes produces 3 fragments; Trim produces 3 segments; Offset works on 1+ shapes
        assertEquals(3, vm.performBooleanOp(VexioBooleanOp.FRAGMENT, longArrayOf(1L, 2L)))
        assertEquals(3, vm.performBooleanOp(VexioBooleanOp.TRIM, longArrayOf(1L, 2L)))
        assertEquals(1, vm.performBooleanOp(VexioBooleanOp.OFFSET, longArrayOf(1L), offsetInches = 0.25))
        assertTrue(vm.uiState.value.isDirty)
    }

    @Test
    fun themesSixByFourMatrixAndVisualEffectPresetsApplyCleanly() {
        val vm = VexioSessionViewModel()

        // Verify all 6 themes × 4 variants = 24 palettes apply deterministically
        for (preset in VexioThemePreset.entries) {
            for (variant in 0..3) {
                val updated = vm.applyDocumentTheme(preset.slug, variant)
                assertTrue("Theme ${preset.slug} v$variant must update >= 1 shape", updated >= 1)
                assertEquals(preset.slug, vm.uiState.value.activeThemeSlug)
                assertEquals(variant, vm.uiState.value.activeThemeVariant)
            }
        }
        assertEquals(0, vm.applyDocumentTheme("nonexistent_theme", 0))

        // Verify visual presets: grad_linear, shadow_soft, bevel_3d, sketchy
        for (effect in listOf("grad_linear", "shadow_soft", "bevel_3d", "sketchy")) {
            assertTrue("Preset $effect must succeed", vm.applyVisualPreset(effect))
            assertTrue(effect in vm.uiState.value.activeVisualPresets)
        }
        assertFalse(vm.applyVisualPreset("invalid_preset"))
    }

    @Test
    fun subprocessDrillDownAndDiagramCompareGenerateStructuredDiffAndSvg() {
        val vm = VexioSessionViewModel()
        val initialPages = vm.uiState.value.pages.size

        // Create subprocess page for Sheet.1
        val subPageId = vm.createOrLinkSubprocess("Order Validation Subprocess", shapeId = 1L)
        assertTrue(subPageId > 0)
        assertEquals(initialPages + 1, vm.uiState.value.pages.size)
        assertEquals(subPageId, vm.uiState.value.activePageId)
        assertEquals(subPageId, vm.uiState.value.subprocessLinks[1L])

        // Re-linking same subprocess name is idempotent (reuses existing page)
        val reusedId = vm.createOrLinkSubprocess("Order Validation Subprocess", shapeId = 1L)
        assertEquals(subPageId, reusedId)
        assertEquals(initialPages + 1, vm.uiState.value.pages.size)

        // Modify a cell and delete a shape, then run Diagram Compare
        vm.commitCellFormula("Height", "2.75 in", forceSetf = false)
        vm.deleteSelectedShape()

        val diff = vm.compareWithBaseline()
        assertNotNull(vm.uiState.value.lastCompareSummary)
        assertTrue(diff.addedCount >= 1)
        assertTrue(diff.modifiedCount >= 1)
        assertTrue(diff.removedCount >= 1)
        assertTrue(diff.diffOverlaySvg.startsWith("<svg"))
        assertTrue(diff.rawReportJson.contains("\"added_count\":"))
    }

    @Test
    fun dataVisualizerCloudDiscoveryAndEntraLdifConnectorsImportShapes() {
        val vm = VexioSessionViewModel()

        val dvShapes = vm.importDataVisualizer(
            Plan2SamplePayloads.dataVisualizerCsv.toByteArray(Charsets.UTF_8)
        )
        // 5 swimlanes + 1 pool header + 5 steps + 5 connectors = 16 shapes
        assertEquals(16, dvShapes)
        assertEquals(16, vm.uiState.value.lastDataVisualizerShapeCount)

        val cloudShapes = vm.importCloudDiscovery(Plan2SamplePayloads.cloudDiscoveryJson)
        // 2 distinct VPCs + 3 cloud resources + 2 edges = 7 shapes
        assertEquals(7, cloudShapes)
        assertEquals(7, vm.uiState.value.lastCloudDiscoveryShapeCount)

        val ldifShapes = vm.importEntraOrLdifOrgChart(Plan2SamplePayloads.entraOrgChartLdif)
        // 3 directory entries + 2 manager reporting lines = 5 shapes
        assertEquals(5, ldifShapes)

        val entraGraphJson = """
            {
              "value": [
                {"id": "u1", "displayName": "Ada Lovelace", "jobTitle": "CTO"},
                {"id": "u2", "displayName": "Alan Turing", "jobTitle": "Principal Architect", "managerId": "u1"}
              ]
            }
        """.trimIndent()
        assertEquals(3, vm.importEntraOrLdifOrgChart(entraGraphJson))
    }

    @Test
    fun hlcCrdtBatchConvergesOutOfOrderAndRespectsGuardProtection() {
        val vmA = VexioSessionViewModel()
        val vmB = VexioSessionViewModel()
        val shapeId = vmA.uiState.value.selectedShapeId

        val opOlder = """
            {"SetCell":{"ts":{"wall_ms":1000,"logical":1,"replica_id":"peer-a"},"page_id":1,"shape_id":$shapeId,"cell_name":"PinX","formula":"5.25 in","force_setf":false}}
        """.trimIndent()
        val opNewer = """
            {"SetCell":{"ts":{"wall_ms":1000,"logical":2,"replica_id":"peer-b"},"page_id":1,"shape_id":$shapeId,"cell_name":"PinX","formula":"7.75 in","force_setf":false}}
        """.trimIndent()
        val opGuardBlocked = """
            {"SetCell":{"ts":{"wall_ms":2000,"logical":1,"replica_id":"peer-b"},"page_id":1,"shape_id":$shapeId,"cell_name":"Width","formula":"9.00 in","force_setf":false}}
        """.trimIndent()

        // Apply [older, newer] on vmA, and [newer] then [older] on vmB
        assertEquals(2, vmA.applyCrdtBatchJson("[$opOlder,$opNewer,$opGuardBlocked]", "peer-local-a"))
        assertEquals(1, vmB.applyCrdtBatchJson("[$opNewer]", "peer-local-b"))
        assertEquals(0, vmB.applyCrdtBatchJson("[$opOlder,$opGuardBlocked]", "peer-local-b"))

        val pinXA = vmA.uiState.value.shapeSheetCells.first { it.cellName == "PinX" }.formula
        val pinXB = vmB.uiState.value.shapeSheetCells.first { it.cellName == "PinX" }.formula
        assertEquals("7.75 in", pinXA)
        assertEquals(pinXA, pinXB)

        // Guarded Width remained protected against non-SETF CRDT write
        assertEquals(
            "GUARD(2.50 in)",
            vmA.uiState.value.shapeSheetCells.first { it.cellName == "Width" }.formula
        )
    }

    @Test
    fun plan2ExportsProduceValidOpenXmlZipAndWorkflowJsonPackages() {
        val vm = VexioSessionViewModel()

        fun listZipEntries(bytes: ByteArray): Set<String> {
            val names = linkedSetOf<String>()
            ZipInputStream(ByteArrayInputStream(bytes)).use { zis ->
                var entry = zis.nextEntry
                while (entry != null) {
                    names.add(entry.name)
                    entry = zis.nextEntry
                }
            }
            return names
        }

        val pptxBytes = vm.exportPlan2Artifact("pptx")
        val pptxEntries = listZipEntries(pptxBytes)
        assertTrue(pptxEntries.contains("[Content_Types].xml"))
        assertTrue(pptxEntries.contains("ppt/presentation.xml"))
        assertTrue(pptxEntries.contains("ppt/slides/slide1.xml"))

        val docxBytes = vm.exportPlan2Artifact("docx")
        val docxEntries = listZipEntries(docxBytes)
        assertTrue(docxEntries.contains("[Content_Types].xml"))
        assertTrue(docxEntries.contains("word/document.xml"))

        val flowJson = String(vm.exportPlan2Artifact("flow.json"), Charsets.UTF_8)
        assertTrue(flowJson.contains("workflowdefinition.json#"))
        assertTrue(flowJson.contains("\"triggers\""))
        assertTrue(flowJson.contains("\"actions\""))

        val pbiJson = String(vm.exportPlan2Artifact("pbi.json"), Charsets.UTF_8)
        assertTrue(pbiJson.contains("vexioPlan2Visual"))
        assertTrue(pbiJson.contains("\"shapeBindings\""))
    }

    @Test
    fun workstationOverhaulRibbonLanguagesMetricPresetsAndInlineEditWorkDeterministically() {
        val vm = VexioSessionViewModel()

        // 1. In-app Start Backstage is active on launch inside the window (no external file picker)
        assertTrue(vm.uiState.value.showStartBackstage)
        vm.createBlankWithPagePreset("ISO_Architecture.vsdx", VexioPagePresetSpec.A4_LANDSCAPE)
        assertFalse(vm.uiState.value.showStartBackstage)
        assertTrue(vm.uiState.value.metricUnits)
        assertEquals(VexioPagePresetSpec.A4_LANDSCAPE, vm.uiState.value.activePagePreset)

        // 2. ISO A3 & US Letter page presets switch deterministically
        vm.applyPagePreset(VexioPagePresetSpec.A3_LANDSCAPE)
        assertEquals(VexioPagePresetSpec.A3_LANDSCAPE, vm.uiState.value.activePagePreset)
        assertTrue(vm.uiState.value.metricUnits)
        vm.applyPagePreset(VexioPagePresetSpec.LETTER_LANDSCAPE)
        assertEquals(VexioPagePresetSpec.LETTER_LANDSCAPE, vm.uiState.value.activePagePreset)
        assertFalse(vm.uiState.value.metricUnits)

        // 3. All 6 languages (EN, DE, IT, FR, ES, GR) localize UI strings live
        assertEquals(6, VexioAppLanguage.entries.size)
        for (lang in VexioAppLanguage.entries) {
            vm.setLanguage(lang)
            assertEquals(lang, vm.uiState.value.language)
            assertTrue(lang.fileLabel.isNotBlank())
            assertTrue(lang.homeLabel.isNotBlank())
            assertTrue(lang.importStencilLabel.isNotBlank())
        }

        // 4. Classic collapsible Visio Ribbon tab switching & collapse toggle
        vm.selectRibbonTab("Design")
        assertEquals("Design", vm.uiState.value.activeRibbonTab)
        assertFalse(vm.uiState.value.isRibbonCollapsed)
        vm.toggleRibbonCollapsed()
        assertTrue(vm.uiState.value.isRibbonCollapsed)

        // 5. Double-click top-left document title inline rename
        vm.renameDocument("Enterprise_Firewall_Map")
        assertEquals("Enterprise_Firewall_Map.vsdx", vm.uiState.value.documentTitle)

        // 6. Standard Rich Text formatting options
        vm.applyRichTextStyle(
            fontFamily = "Roboto",
            fontSizePt = 16,
            bold = true,
            italic = true,
            underline = true,
            strikethrough = true,
            horizAlign = 0,
            vertAlign = 0,
            bullets = true,
            colorHex = "#1D4ED8",
            bgColorHex = "#FEF08A"
        )
        val styled = vm.uiState.value
        assertEquals("Roboto", styled.textFontFamily)
        assertEquals(16, styled.textFontSizePt)
        assertTrue(styled.textBold)
        assertTrue(styled.textItalic)
        assertTrue(styled.textUnderline)
        assertTrue(styled.textStrikethrough)
        assertEquals(0, styled.textHorizAlign)
        assertEquals(0, styled.textVertAlign)
        assertTrue(styled.textBullets)
        assertEquals("#1D4ED8", styled.textColorHex)
        assertEquals("#FEF08A", styled.textBgColorHex)

        // 7. Rotate & Flip stencils + post-insert auto-return to Pointer cursor + inline label edit
        vm.selectTool(VexioTool.RECTANGLE)
        assertEquals(VexioTool.RECTANGLE, vm.uiState.value.activeTool)
        vm.instantiateMasterDrop(
            title = "Next-Gen Firewall (NGFW)",
            widthFormula = "1.90 in",
            heightFormula = "0.85 in",
            fillFormula = "RGB(254,242,242)",
            pinXInches = 4.0f,
            pinYInches = 4.0f,
            category = "Network & Rack Equipment"
        )
        assertEquals(VexioTool.POINTER, vm.uiState.value.activeTool)
        assertEquals(VexioCursorKind.ARROW, vm.uiState.value.activeCursor)

        vm.rotateOrFlipSelected(deltaAngleDeg = 90.0)
        assertTrue(vm.uiState.value.statusBanner.contains("Rotated Right 90°"))
        vm.rotateOrFlipSelected(flipX = true)
        assertTrue(vm.uiState.value.statusBanner.contains("Flipped Horizontally"))

        // 8. Inserting Text object immediately returns to Pointer and opens inline text editor
        vm.selectTool(VexioTool.TEXT_BLOCK)
        vm.drawQuickPrimitive("text", 3.0, 3.0)
        assertEquals(VexioTool.POINTER, vm.uiState.value.activeTool)
        assertNotNull(vm.uiState.value.inlineTextEditor)
        assertTrue(vm.uiState.value.inlineTextEditor!!.isNewlyCreatedText)
        vm.updateInlineTextDraft("Core Switch Rack A1")
        vm.commitInlineTextEdit("Core Switch Rack A1")
        assertEquals(null, vm.uiState.value.inlineTextEditor)
    }

    @Test
    fun pageRenameOsFontsSaveAsLocalizationAndShapeColorPickersWorkDeterministically() {
        val vm = VexioSessionViewModel()

        // 1. OS Font Catalog discovers system & metrics-compatible font families offline
        val fonts = vm.uiState.value.availableFontFamilies
        assertTrue("Expected >= 20 OS font families, got ${fonts.size}", fonts.size >= 20)
        assertTrue(fonts.contains("Roboto"))
        assertTrue(fonts.contains("Calibri"))

        // 2. Rename Page updates page tab label and marks document dirty
        val renamed = vm.renamePage(1, "MSSC Gesamtarchitektur")
        assertTrue(renamed)
        assertEquals("MSSC Gesamtarchitektur", vm.uiState.value.pages.first { it.pageId == 1 }.name)
        assertTrue(vm.uiState.value.isDirty)

        // 3. Full Color Picker applies FillForegnd & LineColor RGB() formulas
        vm.applyShapeFillColor("#DBEAFE")
        assertEquals("#DBEAFE", vm.uiState.value.shapeFillColorHex)
        assertEquals(
            "RGB(219,234,254)",
            vm.uiState.value.shapeSheetCells.first { it.cellName == "FillForegnd" }.formula
        )

        vm.applyShapeLineColor("#1E40AF")
        assertEquals("#1E40AF", vm.uiState.value.shapeLineColorHex)
        assertEquals(
            "RGB(30,64,175)",
            vm.uiState.value.shapeSheetCells.first { it.cellName == "LineColor" }.formula
        )

        // 4. All 6 languages expose distinct non-empty Save, Save As, Rename Page, and Inspector tab labels
        for (lang in VexioAppLanguage.entries) {
            assertTrue(lang.saveLabel.isNotBlank())
            assertTrue(lang.saveAsLabel.isNotBlank())
            assertFalse(lang.saveLabel == lang.saveAsLabel)
            assertTrue(lang.renamePageLabel.isNotBlank())
            assertTrue(lang.inspectorFormatTab.isNotBlank())
            assertTrue(lang.inspectorGeometryTab.isNotBlank())
            assertTrue(lang.inspectorValidateTab.isNotBlank())
        }
    }

    @Test
    fun v020ArrangeGroupSmartGuidesAndFontRegistrationWorkDeterministically() {
        val vm = VexioSessionViewModel()

        // 1. Smart Guides enabled by default and toggles deterministically
        assertTrue(vm.uiState.value.smartGuidesEnabled)
        vm.toggleSmartGuides()
        assertFalse(vm.uiState.value.smartGuidesEnabled)
        vm.toggleSmartGuides(true)
        assertTrue(vm.uiState.value.smartGuidesEnabled)

        // 2. Align & Distribute across multi-selection
        vm.setSelectionIds(listOf(1L, 2L, 3L))
        assertEquals(listOf(1L, 2L, 3L), vm.uiState.value.selectedShapeIds)
        for (mode in listOf("left", "center", "right", "top", "middle", "bottom")) {
            assertEquals(3, vm.alignSelectedShapes(mode))
        }
        assertEquals(3, vm.distributeSelectedShapes("horizontal"))
        assertEquals(3, vm.distributeSelectedShapes("vertical"))

        // 3. Group (Ctrl+G) and Ungroup (Ctrl+Shift+G) preserve member IDs
        val gid = vm.groupSelectedShapes()
        assertTrue(gid > 0L)
        assertEquals(listOf(gid), vm.uiState.value.selectedShapeIds)
        val restored = vm.ungroupSelectedShapes()
        assertEquals(listOf(1L, 2L, 3L), restored)
        assertEquals(listOf(1L, 2L, 3L), vm.uiState.value.selectedShapeIds)

        // 4. 1D Orthogonal Connector Segment Drag & Custom TrueType Font Registration
        assertTrue(vm.dragConnectorSegment(shapeId = 2L, segIndex = 1, coordInches = 4.25))
        assertTrue(vm.registerCustomFontBytes(ByteArray(64) { 1 }, "Custom Corporate Sans"))
        assertTrue(vm.uiState.value.availableFontFamilies.contains("Custom Corporate Sans"))
    }

    @Test
    fun v021StencilCatalogInitDynamicJsonAndVersionMetadataWorkDeterministically() {
        val vm = VexioSessionViewModel()
        assertEquals(33, vm.uiState.value.stencilCategories.size)
        assertTrue(vm.uiState.value.stencilMasters.isNotEmpty())
        assertTrue(
            "Expected 0.9.0-beta in updateStatusMessage, got: ${vm.uiState.value.updateStatusMessage}",
            vm.uiState.value.updateStatusMessage.contains("0.9.0-beta")
        )

        val sampleNativeJson = """
            {
              "BPMN 2.0": [
                {"name": "User Task", "prompt": "Human workflow step", "widthIn": 1.6, "heightIn": 0.9, "fillFormula": "=RGB(239,246,255)", "svg": "<svg viewBox=\"0 0 144 96\"><rect x=\"8\" y=\"8\" width=\"128\" height=\"80\" fill=\"#eff6ff\" stroke=\"#1d4ed8\"/></svg>"},
                {"name": "Exclusive Gateway", "prompt": "XOR branch", "widthIn": 0.8, "heightIn": 0.8, "fillFormula": "=RGB(254,249,195)", "svg": "<svg viewBox=\"0 0 96 96\"><polygon points=\"48,6 90,48 48,90 6,48\" fill=\"#fef9c3\" stroke=\"#ca8a04\"/></svg>"}
              ],
              "Basic Flowchart": [
                {"name": "Process", "prompt": "Standard step", "widthIn": 1.5, "heightIn": 0.8, "fillFormula": "=RGB(219,234,254)", "svg": "<svg viewBox=\"0 0 144 96\"><path d=\"M 10 10 L 134 10 L 134 86 L 10 86 Z\" fill=\"#dbeafe\" stroke=\"#2563eb\"/></svg>"}
              ],
              "CustomImportedPack": [
                {"name": "Telemetry Node", "prompt": "Custom sensor", "widthIn": 1.0, "heightIn": 1.0, "fillFormula": "RGB(220,252,231)", "svg": "<svg viewBox=\"0 0 64 64\"><circle cx=\"32\" cy=\"32\" r=\"26\" fill=\"#dcfce7\" stroke=\"#16a34a\"/></svg>"}
              ]
            }
        """.trimIndent()

        val loadedCount = vm.applyNativeStencilsJson(sampleNativeJson)
        assertEquals(4, loadedCount)
        assertEquals(
            listOf("Basic Flowchart", "BPMN 2.0", "CustomImportedPack"),
            vm.uiState.value.stencilCategories
        )
        assertEquals(4, vm.uiState.value.stencilMasters.size)
        assertEquals("RGB(239,246,255)", vm.uiState.value.stencilMasters.first { it.title == "User Task" }.fillFormula)
    }

    @Test
    fun v030Pillar3ContainersSwimlanesCalloutsControlHandlesAndAutoConnectWork() {
        val vm = VexioSessionViewModel()
        vm.setSelectionIds(listOf(1L))

        val contId = vm.insertContainer("VPC Zone")
        assertTrue(contId > 0L)
        assertEquals(listOf(contId), vm.uiState.value.selectedShapeIds)

        val lanes = vm.insertSwimlanes("")
        assertEquals(3, lanes)

        val calloutId = vm.attachCallout("SLA < 10ms")
        assertTrue(calloutId > contId)
        assertEquals(listOf(calloutId), vm.uiState.value.selectedShapeIds)
        assertTrue(vm.dragControlHandle(calloutId, "Row_1", 3.5, 4.2))

        val nextStepId = vm.autoConnectSelected("east")
        assertTrue(nextStepId > calloutId)
        assertEquals(listOf(nextStepId), vm.uiState.value.selectedShapeIds)
    }

    @Test
    fun v037I18nAndViewportCenterDropCascadeWorkDeterministically() {
        val vm = VexioSessionViewModel()
        for (lang in VexioAppLanguage.entries) {
            assertTrue(com.vexio.ui.i18n.VexioI18n.startHeroSubtitle(lang).isNotBlank())
            assertTrue(com.vexio.ui.i18n.VexioI18n.blankA4LandscapeTitle(lang).isNotBlank())
            assertTrue(com.vexio.ui.i18n.VexioI18n.autoCheckUpdatesCheckbox(lang).isNotBlank())
            assertTrue(
                com.vexio.ui.i18n.VexioI18n.localizeUpdateStatus(
                    "Offline mode active • Update check skipped cleanly",
                    null,
                    lang
                ).isNotBlank()
            )
        }
        assertEquals(
            "Leeres ISO A4 Querformat",
            com.vexio.ui.i18n.VexioI18n.blankA4LandscapeTitle(VexioAppLanguage.DE)
        )
        val firstMaster = vm.uiState.value.stencilMasters.first()
        vm.dropStencilAtViewportCenter(firstMaster)
        val id1 = vm.uiState.value.selectedShapeId
        assertEquals(listOf(id1), vm.uiState.value.selectedShapeIds)
        val pinX1 = vm.uiState.value.shapeSheetCells.first { it.cellName == "PinX" }.formula
        vm.dropStencilAtViewportCenter(firstMaster)
        val id2 = vm.uiState.value.selectedShapeId
        assertTrue(id2 != id1)
        val pinX2 = vm.uiState.value.shapeSheetCells.first { it.cellName == "PinX" }.formula
        assertTrue("Expected cascaded PinX on consecutive double-click drops: $pinX1 vs $pinX2", pinX1 != pinX2)
    }

    @Test
    fun v060Milestone1VbaCommentsHyperlinksWizardsAndCadToolsWorkDeterministically() {
        val vm = VexioSessionViewModel()

        // 1. Edit Vertices (16) and Ink Eraser (17) tools
        vm.selectTool(VexioTool.EDIT_VERTICES)
        assertEquals(16, vm.uiState.value.activeTool.nativeCode)
        vm.selectTool(VexioTool.INK_ERASER)
        assertEquals(17, vm.uiState.value.activeTool.nativeCode)

        // 2. VBA Sandbox execution
        val vbaSummary = vm.runVbaMacro(
            """
            Sub ResizeHero()
                ActivePage.Shapes(1).Cells("Width").FormulaU = "3.75 in"
            End Sub
            """.trimIndent()
        )
        assertTrue(vbaSummary.isNotBlank())
        assertEquals("3.75 in", vm.uiState.value.shapeSheetCells.first { it.cellName == "Width" }.formula)

        // 3. Threaded Review Comments
        val cid = vm.addReviewComment("Alice", "Verify failover SLA")
        assertTrue(cid > 0)
        assertEquals(1, vm.uiState.value.reviewComments.size)
        vm.toggleResolveComment(cid, true)
        assertTrue(vm.uiState.value.reviewComments.first().resolved)
        vm.deleteReviewComment(cid)
        assertTrue(vm.uiState.value.reviewComments.isEmpty())

        // 4. Hyperlink & ScreenTip + Domain Wizards + CAD Scale & Binary DWG export
        assertTrue(vm.saveHyperlinkAndScreentip("Runbook", "https://example.com/runbook", "Page-2", "99.99% SLA"))
        assertEquals("https://example.com/runbook", vm.uiState.value.hyperlinkAddress)
        vm.selectDomainWizardPreset("pert")
        assertTrue(vm.runDomainWizard().isNotBlank())
        assertEquals(48.0, vm.applyCadArchitecturalScale("1/4_in_ft", true), 1e-6)
        assertTrue(vm.convertCadLayersToShapeSheet("*") >= 1)
        val dwgBytes = vm.exportDocumentBytes("dwg")
        assertEquals("AC1018", String(dwgBytes, 0, 6, Charsets.ISO_8859_1))
    }

    @Test
    fun v070Milestone2ConnectionPointSmartActionsAndMyShapesVssxWorkDeterministically() {
        val vm = VexioSessionViewModel()

        // 1. Shift+Ctrl+1 Connection Point tool (native code 18)
        vm.selectTool(VexioTool.CONNECTION_POINT)
        assertEquals(18, vm.uiState.value.activeTool.nativeCode)
        assertEquals("Ctrl+Shift+1", VexioTool.CONNECTION_POINT.shortcutHint)

        // 2. Smart-Shape Actions.* CRUD & one-shot toggle execution
        assertTrue(
            vm.upsertSmartAction(
                rowName = "ToggleValve",
                menu = "Toggle Valve Open",
                actionFormula = "SETF(GetRef(FillForegnd),\"RGB(220,252,231)\")",
                checkedFormula = "1",
                beginGroup = true
            )
        )
        assertEquals(1, vm.uiState.value.selectedShapeActions.size)
        assertEquals("Toggle Valve Open", vm.uiState.value.selectedShapeActions.first().menu)
        assertTrue(vm.executeSmartAction("ToggleValve"))
        assertTrue(vm.deleteSmartAction("ToggleValve"))
        assertTrue(vm.uiState.value.selectedShapeActions.isEmpty())

        // 3. "My Shapes" Custom Stencil authoring, rename, update & standalone .vssx export
        val savedMaster = vm.addSelectionToMyShapes("Custom Pump")
        assertEquals("Custom Pump", savedMaster)
        assertTrue(vm.uiState.value.stencilCategories.contains("My Shapes"))
        assertTrue(vm.renameCustomStencilMaster("Custom Pump", "Redundant Pump"))
        assertTrue(vm.updateCustomMasterFromSelection("Redundant Pump") >= 0)
        val vssxBytes = vm.exportCustomStencilVssxBytes("My Shapes")
        assertTrue(vssxBytes.size > 64)
        assertEquals('P'.code.toByte(), vssxBytes[0])
        assertEquals('K'.code.toByte(), vssxBytes[1])
        assertTrue(vm.deleteCustomStencilMaster("Redundant Pump"))
    }

    @Test
    fun v080Milestone3FieldsGuidesSnapGlueFindReplaceFormatPainterAndNumberShapesWork() {
        val vm = VexioSessionViewModel()

        // 1. Shift+Ctrl+4 Text Block Transform (19) & Ctrl+Shift+P Format Painter (20)
        vm.selectTool(VexioTool.TEXT_BLOCK_TRANSFORM)
        assertEquals(19, vm.uiState.value.activeTool.nativeCode)
        assertEquals("Ctrl+Shift+4", VexioTool.TEXT_BLOCK_TRANSFORM.shortcutHint)
        vm.selectTool(VexioTool.FORMAT_PAINTER)
        assertEquals(20, vm.uiState.value.activeTool.nativeCode)
        assertEquals("Ctrl+Shift+P", VexioTool.FORMAT_PAINTER.shortcutHint)

        // 2. Dynamic <fld> text field, 45 arrowheads & line jumps
        assertTrue(vm.insertDynamicTextField("width", "0.00 in", "W: ") >= 0)
        assertEquals(1, vm.setConnectorArrowheads(29, 27, 2))
        assertTrue(vm.setPageLineJumps(1, 2, 0.66, 0.66))

        // 3. Draggable ruler guides & Snap/Glue matrix
        val gid = vm.addRulerGuide(isVertical = true, coordIn = 5.25)
        assertTrue(gid > 0L)
        assertEquals(1, vm.uiState.value.rulerGuides.size)
        assertTrue(vm.moveRulerGuide(gid, 6.0))
        assertEquals(6.0, vm.uiState.value.rulerGuides.first().coordIn, 1e-6)
        assertTrue(vm.deleteRulerGuide(gid))
        assertTrue(vm.uiState.value.rulerGuides.isEmpty())
        assertTrue(vm.setSnapGlueSettings(snapEnabled = true, glueEnabled = false, snapTargetsMask = 12, glueTypesMask = 1))
        assertFalse(vm.uiState.value.glueEnabled)
        assertEquals(12, vm.uiState.value.snapTargetsMask)

        // 4. Find & Replace, Format Painter & Number Shapes Wizard
        val hits = vm.findInDocument("Process")
        assertTrue(hits.isNotEmpty())
        assertTrue(vm.replaceInDocument("Process", "Gateway") >= 1)
        assertTrue(vm.copySelectedShapeFormat().contains("FillForegnd"))
        assertEquals(1, vm.applyFormatPainter())
        assertTrue(vm.numberShapesOnPage("connected_flow", startNum = 10, interval = 5, prefix = "P-") >= 1)
    }

    @Test
    fun v090Milestone4DimensionsWallsOpeningsWizardsAndVsmFtaWork() {
        val vm = VexioSessionViewModel()

        // 1. Ctrl+Shift+W Wall (21) & Ctrl+Shift+D Dimension (22) tools
        vm.selectTool(VexioTool.WALL)
        assertEquals(21, vm.uiState.value.activeTool.nativeCode)
        assertEquals("Ctrl+Shift+W", VexioTool.WALL.shortcutHint)
        vm.selectTool(VexioTool.DIMENSION)
        assertEquals(22, vm.uiState.value.activeTool.nativeCode)
        assertEquals("Ctrl+Shift+D", VexioTool.DIMENSION.shortcutHint)

        // 2. Architectural Walls, Auto-Healing Miters/T-Joins, Door/Window Openings & Associative Dimensions
        val wallA = vm.drawArchitecturalWall(2.0, 2.0, 6.0, 2.0, 0.25)
        val wallB = vm.drawArchitecturalWall(6.0, 2.0, 6.0, 5.0, 0.25)
        assertTrue(wallA > 0L && wallB > wallA)
        assertTrue(vm.healWallIntersections(0.30) >= 1)
        val doorId = vm.insertWallOpening("single_door", wallShapeId = wallA, offsetAlongIn = 1.5, openingWidthIn = 0.90)
        assertTrue(doorId > wallB)
        val dimId = vm.insertAssociativeDimension("aligned", targetShapeId = wallA, offsetIn = 0.55)
        assertTrue(dimId > doorId)

        // 3. Calendar, PivotDiagram, Brainstorming Mind Map, Lean VSM & Fault Tree Wizards + Calculators
        for (slug in listOf("calendar", "pivot_diagram", "mind_map", "mind_map_radial", "vsm", "fta")) {
            vm.selectDomainWizardPreset(slug)
            assertTrue("Preset $slug should populate non-empty spec", vm.uiState.value.domainWizardSpec.isNotBlank())
            assertTrue(vm.runDomainWizard().isNotBlank())
        }
        val topicId = vm.addMindMapTopic("Post-Launch Telemetry", asSibling = false)
        assertTrue(topicId > 0L)
        assertTrue(vm.exportMindMapOutline().contains("#"))
        val calcJson = vm.recalculateVsmAndFta()
        assertTrue(calcJson.contains("pce") || calcJson.contains("vsm") || calcJson.contains("fta"))
    }
}

