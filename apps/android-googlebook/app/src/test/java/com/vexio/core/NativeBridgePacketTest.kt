package com.vexio.core

import android.view.KeyEvent
import android.view.MotionEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.ByteOrder

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
}
