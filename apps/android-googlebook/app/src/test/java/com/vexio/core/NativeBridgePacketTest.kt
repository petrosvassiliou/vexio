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
    }

    @Test
    fun lightModeDefaultCleanWorkspaceSixteenToolsAndSeventeenStencilCategories() {
        val vm = VexioSessionViewModel()
        val state = vm.uiState.value

        // Light mode default and uncluttered workspace defaults
        assertFalse(state.isDarkTheme)
        assertFalse(state.isPlan2RibbonExpanded)
        assertFalse(state.isShapeSheetDrawerExpanded)
        assertTrue(state.isStencilDrawerExpanded)
        assertTrue(state.isInspectorExpanded)

        // All 16 drawing tools map 1:1 to engine tool codes 0..15
        assertEquals(16, VexioTool.entries.size)
        assertEquals((0..15).toSet(), VexioTool.entries.map { it.nativeCode }.toSet())

        // All 17 Visio Professional stencil categories are present
        assertEquals(17, BuiltInStencilCatalog.categories.size)
        assertEquals(17, state.stencilCategories.size)
        val distinctCategories = state.stencilMasters.map { it.category }.distinct()
        assertEquals(17, distinctCategories.size)

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
}
