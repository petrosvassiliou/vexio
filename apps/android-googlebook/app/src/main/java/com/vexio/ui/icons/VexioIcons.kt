package com.vexio.ui.icons

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathBuilder
import androidx.compose.ui.unit.dp

/**
 * Precision `24×24` CAD & Microsoft Visio Vector Icon Set (`1.75dp` uniform optical stroke,
 * pixel-grid aligned coordinates, rounded caps & joins).
 *
 * Replaces mismatched stock Material phone glyphs with purpose-built vector iconography
 * across the Left Tool Rail, Ribbon, Stencil Navigator, Inspector, and Status Bar.
 */
object VexioIcons {

    private fun strokeIcon(
        name: String,
        strokeWidth: Float = 1.75f,
        build: PathBuilder.() -> Unit
    ): ImageVector = ImageVector.Builder(
        name = name,
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f
    ).addPath(
        pathData = PathBuilder().apply(build).nodes,
        fill = null,
        stroke = SolidColor(Color.Black),
        strokeLineWidth = strokeWidth,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = StrokeJoin.Round,
        pathFillType = PathFillType.NonZero
    ).build()

    // =========================================================================
    // 1. Core Authoring & Drawing Tools
    // =========================================================================

    /** Crisp desktop top-left selection cursor arrow (`V` / `Ctrl+1`). */
    val Pointer: ImageVector by lazy {
        strokeIcon("VexioPointer") {
            moveTo(5.0f, 3.2f)
            lineTo(5.0f, 18.2f)
            lineTo(9.3f, 14.3f)
            lineTo(12.4f, 20.5f)
            lineTo(14.8f, 19.3f)
            lineTo(11.7f, 13.1f)
            lineTo(17.5f, 13.1f)
            close()
        }
    }

    /** Clean outlined pan hand (`H` / `Space`). */
    val PanHand: ImageVector by lazy {
        strokeIcon("VexioPanHand", strokeWidth = 1.65f) {
            moveTo(8.5f, 11.0f)
            verticalLineTo(5.2f)
            arcTo(1.5f, 1.5f, 0f, false, true, 11.5f, 5.2f)
            verticalLineTo(10.0f)
            moveTo(11.5f, 4.5f)
            arcTo(1.5f, 1.5f, 0f, false, true, 14.5f, 4.5f)
            verticalLineTo(10.0f)
            moveTo(14.5f, 5.8f)
            arcTo(1.5f, 1.5f, 0f, false, true, 17.5f, 5.8f)
            verticalLineTo(13.5f)
            arcTo(5.5f, 5.5f, 0f, false, true, 12.0f, 19.5f)
            horizontalLineTo(10.2f)
            arcTo(5.2f, 5.2f, 0f, false, true, 5.6f, 16.7f)
            lineTo(3.8f, 13.2f)
            arcTo(1.5f, 1.5f, 0f, false, true, 6.3f, 11.6f)
            lineTo(8.5f, 13.5f)
            verticalLineTo(6.8f)
            arcTo(1.5f, 1.5f, 0f, false, true, 8.5f, 6.8f)
        }
    }

    /** Orthogonal Visio Dynamic Connector with endpoint ports & arrow (`C` / `Ctrl+3`). */
    val ConnectorOrthogonal: ImageVector by lazy {
        strokeIcon("VexioConnectorOrthogonal") {
            // Bottom-left port circle
            moveTo(7.0f, 18.5f)
            arcTo(2.2f, 2.2f, 0f, true, true, 2.6f, 18.5f)
            arcTo(2.2f, 2.2f, 0f, true, true, 7.0f, 18.5f)
            // Orthogonal step path
            moveTo(7.0f, 18.5f)
            horizontalLineTo(12.2f)
            verticalLineTo(5.8f)
            horizontalLineTo(19.5f)
            // Crisp arrowhead at top-right
            moveTo(16.5f, 3.2f)
            lineTo(19.8f, 5.8f)
            lineTo(16.5f, 8.4f)
        }
    }

    /** Smooth curved Bezier connector. */
    val ConnectorCurved: ImageVector by lazy {
        strokeIcon("VexioConnectorCurved") {
            moveTo(6.6f, 18.5f)
            arcTo(2.0f, 2.0f, 0f, true, true, 2.6f, 18.5f)
            arcTo(2.0f, 2.0f, 0f, true, true, 6.6f, 18.5f)
            moveTo(6.6f, 18.5f)
            cubicTo(13.0f, 18.5f, 11.0f, 5.5f, 19.5f, 5.5f)
            moveTo(16.6f, 3.1f)
            lineTo(19.8f, 5.5f)
            lineTo(16.6f, 7.9f)
        }
    }

    /** Diagonal geometric line segment with two vertex nodes (`L`). */
    val LineSegment: ImageVector by lazy {
        strokeIcon("VexioLineSegment") {
            moveTo(6.2f, 17.8f)
            lineTo(17.8f, 6.2f)
            moveTo(6.8f, 18.8f)
            arcTo(1.8f, 1.8f, 0f, true, true, 3.2f, 18.8f)
            arcTo(1.8f, 1.8f, 0f, true, true, 6.8f, 18.8f)
            moveTo(20.8f, 5.2f)
            arcTo(1.8f, 1.8f, 0f, true, true, 17.2f, 5.2f)
            arcTo(1.8f, 1.8f, 0f, true, true, 20.8f, 5.2f)
        }
    }

    /** Quarter-circle geometric arc with endpoint nodes (`A`). */
    val CircularArc: ImageVector by lazy {
        strokeIcon("VexioCircularArc") {
            moveTo(5.0f, 18.5f)
            arcTo(13.5f, 13.5f, 0f, false, true, 18.5f, 5.0f)
            moveTo(6.6f, 19.0f)
            arcTo(1.6f, 1.6f, 0f, true, true, 3.4f, 19.0f)
            arcTo(1.6f, 1.6f, 0f, true, true, 6.6f, 19.0f)
            moveTo(20.6f, 5.0f)
            arcTo(1.6f, 1.6f, 0f, true, true, 17.4f, 5.0f)
            arcTo(1.6f, 1.6f, 0f, true, true, 20.6f, 5.0f)
        }
    }

    /** Smooth freeform spline wave (`Ctrl+5`). */
    val FreeformSpline: ImageVector by lazy {
        strokeIcon("VexioFreeformSpline") {
            moveTo(3.2f, 16.8f)
            cubicTo(7.2f, 4.2f, 14.8f, 19.8f, 20.8f, 7.2f)
        }
    }

    /** Crisp Rectangle (`Ctrl+8`). */
    val Rectangle: ImageVector by lazy {
        strokeIcon("VexioRectangle") {
            moveTo(3.5f, 5.5f)
            horizontalLineTo(20.5f)
            verticalLineTo(18.5f)
            horizontalLineTo(3.5f)
            close()
        }
    }

    /** Crisp continuous Rounded Rectangle (`R`). */
    val RoundedRect: ImageVector by lazy {
        strokeIcon("VexioRoundedRect") {
            moveTo(7.5f, 5.5f)
            horizontalLineTo(16.5f)
            arcTo(4.0f, 4.0f, 0f, false, true, 20.5f, 9.5f)
            verticalLineTo(14.5f)
            arcTo(4.0f, 4.0f, 0f, false, true, 16.5f, 18.5f)
            horizontalLineTo(7.5f)
            arcTo(4.0f, 4.0f, 0f, false, true, 3.5f, 14.5f)
            verticalLineTo(9.5f)
            arcTo(4.0f, 4.0f, 0f, false, true, 7.5f, 5.5f)
            close()
        }
    }

    /** Crisp Ellipse (`Ctrl+9`). */
    val Ellipse: ImageVector by lazy {
        strokeIcon("VexioEllipse") {
            moveTo(21.0f, 12.0f)
            arcTo(9.0f, 6.8f, 0f, true, true, 3.0f, 12.0f)
            arcTo(9.0f, 6.8f, 0f, true, true, 21.0f, 12.0f)
            close()
        }
    }

    /** Crisp outlined Decision Diamond (`D`) — matches optical weight of Rectangle & Ellipse. */
    val Diamond: ImageVector by lazy {
        strokeIcon("VexioDiamond") {
            moveTo(12.0f, 3.2f)
            lineTo(21.0f, 12.0f)
            lineTo(12.0f, 20.8f)
            lineTo(3.0f, 12.0f)
            close()
        }
    }

    /** Crisp outlined Triangle (`T`). */
    val Triangle: ImageVector by lazy {
        strokeIcon("VexioTriangle") {
            moveTo(12.0f, 3.8f)
            lineTo(20.6f, 19.2f)
            lineTo(3.4f, 19.2f)
            close()
        }
    }

    /** Crisp outlined Preparation Hexagon (`X`) — uniform stroke, not solid filled. */
    val Hexagon: ImageVector by lazy {
        strokeIcon("VexioHexagon") {
            moveTo(7.2f, 4.2f)
            horizontalLineTo(16.8f)
            lineTo(21.2f, 12.0f)
            lineTo(16.8f, 19.8f)
            horizontalLineTo(7.2f)
            lineTo(2.8f, 12.0f)
            close()
        }
    }

    /** Crisp 5-point Star (`S`). */
    val Star: ImageVector by lazy {
        strokeIcon("VexioStar", strokeWidth = 1.65f) {
            moveTo(12.0f, 3.0f)
            lineTo(14.7f, 8.6f)
            lineTo(20.8f, 9.4f)
            lineTo(16.3f, 13.7f)
            lineTo(17.5f, 19.8f)
            lineTo(12.0f, 16.8f)
            lineTo(6.5f, 19.8f)
            lineTo(7.7f, 13.7f)
            lineTo(3.2f, 9.4f)
            lineTo(9.3f, 8.6f)
            close()
        }
    }

    /** Precision Typography `T` with subtle frame corners (`Ctrl+2`). */
    val TextTool: ImageVector by lazy {
        strokeIcon("VexioTextTool") {
            moveTo(5.0f, 6.5f)
            verticalLineTo(4.5f)
            horizontalLineTo(19.0f)
            verticalLineTo(6.5f)
            moveTo(12.0f, 4.5f)
            verticalLineTo(19.5f)
            moveTo(9.0f, 19.5f)
            horizontalLineTo(15.0f)
        }
    }

    /** Text Block Transform tool (`Ctrl+Shift+4`) — dashed bounding frame with `T` inside. */
    val TextBlockTransform: ImageVector by lazy {
        strokeIcon("VexioTextBlockTransform", strokeWidth = 1.65f) {
            // Corner brackets
            moveTo(3.5f, 7.5f); verticalLineTo(4.0f); horizontalLineTo(7.0f)
            moveTo(17.0f, 4.0f); horizontalLineTo(20.5f); verticalLineTo(7.5f)
            moveTo(20.5f, 16.5f); verticalLineTo(20.0f); horizontalLineTo(17.0f)
            moveTo(7.0f, 20.0f); horizontalLineTo(3.5f); verticalLineTo(16.5f)
            // Inner T
            moveTo(8.5f, 8.5f); horizontalLineTo(15.5f)
            moveTo(12.0f, 8.5f); verticalLineTo(16.0f)
        }
    }

    /** Paint Roller Format Painter (`Ctrl+Shift+P`). */
    val FormatPainter: ImageVector by lazy {
        strokeIcon("VexioFormatPainter") {
            moveTo(4.0f, 4.0f)
            horizontalLineTo(16.0f)
            verticalLineTo(8.8f)
            horizontalLineTo(4.0f)
            close()
            moveTo(16.0f, 6.4f)
            horizontalLineTo(19.0f)
            verticalLineTo(11.5f)
            horizontalLineTo(10.5f)
            verticalLineTo(14.0f)
            moveTo(8.8f, 14.0f)
            horizontalLineTo(12.2f)
            verticalLineTo(20.2f)
            horizontalLineTo(8.8f)
            close()
        }
    }

    /** Architectural Double-Line Plan Wall with structural section ticks (`Ctrl+Shift+W`). */
    val ArchitecturalWall: ImageVector by lazy {
        strokeIcon("VexioArchitecturalWall") {
            moveTo(2.8f, 7.5f)
            horizontalLineTo(21.2f)
            verticalLineTo(16.5f)
            horizontalLineTo(2.8f)
            close()
            // 45-degree architectural wall hatch strokes
            moveTo(6.5f, 16.5f); lineTo(10.5f, 7.5f)
            moveTo(13.0f, 16.5f); lineTo(17.0f, 7.5f)
        }
    }

    /** Architectural 90° Door Swing Opening for floor plans. */
    val WallOpeningDoor: ImageVector by lazy {
        strokeIcon("VexioWallOpeningDoor") {
            moveTo(2.5f, 18.5f); horizontalLineTo(7.0f)
            moveTo(17.0f, 18.5f); horizontalLineTo(21.5f)
            // Open door leaf
            moveTo(7.0f, 18.5f); verticalLineTo(7.5f)
            // Quarter-circle door swing arc
            moveTo(7.0f, 7.5f)
            arcTo(10.5f, 10.5f, 0f, false, true, 17.5f, 18.5f)
        }
    }

    /** Associative Engineering Dimension witness lines & double arrows (`Ctrl+Shift+D`). */
    val AssociativeDimension: ImageVector by lazy {
        strokeIcon("VexioAssociativeDimension") {
            // Left and right witness extension lines
            moveTo(3.5f, 5.0f); verticalLineTo(19.0f)
            moveTo(20.5f, 5.0f); verticalLineTo(19.0f)
            // Dimension span line + arrowheads
            moveTo(4.0f, 12.0f); horizontalLineTo(20.0f)
            moveTo(7.2f, 9.2f); lineTo(4.0f, 12.0f); lineTo(7.2f, 14.8f)
            moveTo(16.8f, 9.2f); lineTo(20.0f, 12.0f); lineTo(16.8f, 14.8f)
        }
    }

    /** Edit Vertices polygon contour with square vertex handles (`Ctrl+6`). */
    val EditVertices: ImageVector by lazy {
        strokeIcon("VexioEditVertices", strokeWidth = 1.65f) {
            // Vertex 1 (top-left)
            moveTo(3.5f, 4.5f); horizontalLineTo(7.5f); verticalLineTo(8.5f); horizontalLineTo(3.5f); close()
            // Vertex 2 (top-right)
            moveTo(16.5f, 3.5f); horizontalLineTo(20.5f); verticalLineTo(7.5f); horizontalLineTo(16.5f); close()
            // Vertex 3 (bottom)
            moveTo(10.0f, 16.5f); horizontalLineTo(14.0f); verticalLineTo(20.5f); horizontalLineTo(10.0f); close()
            // Connecting segments
            moveTo(7.5f, 6.2f); lineTo(16.5f, 5.6f)
            moveTo(17.8f, 7.5f); lineTo(13.2f, 16.5f)
            moveTo(10.8f, 16.5f); lineTo(6.2f, 8.5f)
        }
    }

    /** Visio Connection Point port `×` on a shape edge (`Ctrl+Shift+1`). */
    val ConnectionPoint: ImageVector by lazy {
        strokeIcon("VexioConnectionPoint") {
            // Partial shape edge
            moveTo(4.0f, 6.0f)
            horizontalLineTo(12.0f)
            verticalLineTo(18.0f)
            horizontalLineTo(4.0f)
            // Crisp Visio 'X' connection port at (12, 12) + glue indicator
            moveTo(15.0f, 9.0f); lineTo(21.0f, 15.0f)
            moveTo(21.0f, 9.0f); lineTo(15.0f, 15.0f)
            moveTo(12.0f, 12.0f); horizontalLineTo(15.2f)
        }
    }

    /** Precision Stylus / Pen Ink tool (`Ctrl+4`). */
    val MarkupPen: ImageVector by lazy {
        strokeIcon("VexioMarkupPen") {
            moveTo(15.8f, 3.6f)
            lineTo(20.4f, 8.2f)
            lineTo(8.8f, 19.8f)
            lineTo(3.6f, 20.4f)
            lineTo(4.2f, 15.2f)
            close()
            moveTo(13.2f, 6.2f)
            lineTo(17.8f, 10.8f)
        }
    }

    /** Chisel-tip Highlighter marker (`Ctrl+7`). */
    val Highlighter: ImageVector by lazy {
        strokeIcon("VexioHighlighter") {
            moveTo(14.2f, 3.5f)
            lineTo(19.5f, 8.8f)
            lineTo(10.5f, 17.8f)
            lineTo(5.2f, 12.5f)
            close()
            moveTo(5.2f, 12.5f)
            lineTo(3.5f, 17.8f)
            lineTo(10.5f, 17.8f)
            moveTo(3.5f, 20.8f)
            horizontalLineTo(14.5f)
        }
    }

    /** Angled two-part drafting Block Eraser (`Alt+E`). */
    val InkEraser: ImageVector by lazy {
        strokeIcon("VexioInkEraser") {
            moveTo(13.8f, 4.2f)
            lineTo(20.2f, 10.6f)
            lineTo(11.4f, 19.4f)
            horizontalLineTo(6.2f)
            lineTo(3.6f, 16.8f)
            close()
            // Two-tone eraser band seam
            moveTo(8.5f, 9.5f)
            lineTo(14.9f, 15.9f)
            // Baseline
            moveTo(11.4f, 19.4f)
            horizontalLineTo(20.5f)
        }
    }

    // =========================================================================
    // 2. Arrange, Group, Align, Distribute, Order & Transform Icons
    // =========================================================================

    /** Group Selected Shapes (`Ctrl+G`) — outer selection frame with two inner shapes. */
    val GroupShapes: ImageVector by lazy {
        strokeIcon("VexioGroupShapes", strokeWidth = 1.65f) {
            // Outer group frame ticks
            moveTo(3.0f, 6.5f); verticalLineTo(3.0f); horizontalLineTo(6.5f)
            moveTo(17.5f, 3.0f); horizontalLineTo(21.0f); verticalLineTo(6.5f)
            moveTo(21.0f, 17.5f); verticalLineTo(21.0f); horizontalLineTo(17.5f)
            moveTo(6.5f, 21.0f); horizontalLineTo(3.0f); verticalLineTo(17.5f)
            // Inner sub-shape 1
            moveTo(6.2f, 6.2f); horizontalLineTo(13.2f); verticalLineTo(12.5f); horizontalLineTo(6.2f); close()
            // Inner sub-shape 2
            moveTo(10.8f, 11.5f); horizontalLineTo(17.8f); verticalLineTo(17.8f); horizontalLineTo(10.8f); close()
        }
    }

    /** Ungroup Shapes (`Ctrl+Shift+G`) — two separated independent shapes with handles. */
    val UngroupShapes: ImageVector by lazy {
        strokeIcon("VexioUngroupShapes", strokeWidth = 1.65f) {
            moveTo(3.5f, 3.5f); horizontalLineTo(11.5f); verticalLineTo(11.5f); horizontalLineTo(3.5f); close()
            moveTo(12.5f, 12.5f); horizontalLineTo(20.5f); verticalLineTo(20.5f); horizontalLineTo(12.5f); close()
            // Separation arrows
            moveTo(15.0f, 5.0f); horizontalLineTo(19.0f); verticalLineTo(9.0f)
            moveTo(9.0f, 19.0f); horizontalLineTo(5.0f); verticalLineTo(15.0f)
        }
    }

    /** Bring to Front — prominent front square overlapping back square. */
    val BringToFront: ImageVector by lazy {
        strokeIcon("VexioBringToFront") {
            // Front full square
            moveTo(8.5f, 8.5f); horizontalLineTo(20.5f); verticalLineTo(20.5f); horizontalLineTo(8.5f); close()
            // Back partial square behind
            moveTo(8.5f, 15.5f); horizontalLineTo(3.5f); verticalLineTo(3.5f); horizontalLineTo(15.5f); verticalLineTo(8.5f)
        }
    }

    /** Send to Back — front outline with back target square. */
    val SendToBack: ImageVector by lazy {
        strokeIcon("VexioSendToBack") {
            // Front square at top-left
            moveTo(3.5f, 3.5f); horizontalLineTo(15.5f); verticalLineTo(15.5f); horizontalLineTo(3.5f); close()
            // Back square peeking bottom-right
            moveTo(15.5f, 9.0f); horizontalLineTo(20.5f); verticalLineTo(20.5f); horizontalLineTo(9.0f); verticalLineTo(15.5f)
        }
    }

    /** Distribute Horizontally — left/right witness lines with 3 evenly spaced columns. */
    val DistributeHorizontally: ImageVector by lazy {
        strokeIcon("VexioDistributeH", strokeWidth = 1.65f) {
            moveTo(3.0f, 4.0f); verticalLineTo(20.0f)
            moveTo(21.0f, 4.0f); verticalLineTo(20.0f)
            moveTo(7.0f, 7.0f); horizontalLineTo(10.0f); verticalLineTo(17.0f); horizontalLineTo(7.0f); close()
            moveTo(14.0f, 7.0f); horizontalLineTo(17.0f); verticalLineTo(17.0f); horizontalLineTo(14.0f); close()
        }
    }

    /** Distribute Vertically — top/bottom witness lines with 2 evenly spaced bars. */
    val DistributeVertically: ImageVector by lazy {
        strokeIcon("VexioDistributeV", strokeWidth = 1.65f) {
            moveTo(4.0f, 3.0f); horizontalLineTo(20.0f)
            moveTo(4.0f, 21.0f); horizontalLineTo(20.0f)
            moveTo(7.0f, 7.0f); horizontalLineTo(17.0f); verticalLineTo(10.0f); horizontalLineTo(7.0f); close()
            moveTo(7.0f, 14.0f); horizontalLineTo(17.0f); verticalLineTo(17.0f); horizontalLineTo(7.0f); close()
        }
    }

    /** Rotate 90° Counter-Clockwise. */
    val RotateLeft90: ImageVector by lazy {
        strokeIcon("VexioRotateLeft90") {
            moveTo(8.5f, 5.0f)
            lineTo(5.2f, 8.0f)
            lineTo(8.5f, 11.0f)
            moveTo(5.5f, 8.0f)
            horizontalLineTo(13.5f)
            arcTo(5.5f, 5.5f, 0f, false, true, 19.0f, 13.5f)
            verticalLineTo(19.0f)
            moveTo(5.0f, 14.0f); horizontalLineTo(12.0f); verticalLineTo(20.0f); horizontalLineTo(5.0f); close()
        }
    }

    /** Rotate 90° Clockwise. */
    val RotateRight90: ImageVector by lazy {
        strokeIcon("VexioRotateRight90") {
            moveTo(15.5f, 5.0f)
            lineTo(18.8f, 8.0f)
            lineTo(15.5f, 11.0f)
            moveTo(18.5f, 8.0f)
            horizontalLineTo(10.5f)
            arcTo(5.5f, 5.5f, 0f, false, false, 5.0f, 13.5f)
            verticalLineTo(19.0f)
            moveTo(12.0f, 14.0f); horizontalLineTo(19.0f); verticalLineTo(20.0f); horizontalLineTo(12.0f); close()
        }
    }

    /** Flip Horizontal across vertical mirror axis. */
    val FlipHorizontal: ImageVector by lazy {
        strokeIcon("VexioFlipH", strokeWidth = 1.65f) {
            moveTo(12.0f, 3.0f); verticalLineTo(21.0f)
            moveTo(9.0f, 6.5f); lineTo(3.2f, 18.0f); horizontalLineTo(9.0f); close()
            moveTo(15.0f, 6.5f); lineTo(20.8f, 18.0f); horizontalLineTo(15.0f); close()
        }
    }

    /** Flip Vertical across horizontal mirror axis. */
    val FlipVertical: ImageVector by lazy {
        strokeIcon("VexioFlipV", strokeWidth = 1.65f) {
            moveTo(3.0f, 12.0f); horizontalLineTo(21.0f)
            moveTo(6.5f, 9.0f); lineTo(18.0f, 3.2f); verticalLineTo(9.0f); close()
            moveTo(6.5f, 15.0f); lineTo(18.0f, 20.8f); verticalLineTo(15.0f); close()
        }
    }

    // =========================================================================
    // 3. Diagram Parts, Panes, Layers, Rulers, Views & Line-Ends
    // =========================================================================

    /** Visio Container frame with header band. */
    val ContainerFrame: ImageVector by lazy {
        strokeIcon("VexioContainerFrame") {
            moveTo(3.5f, 4.5f); horizontalLineTo(20.5f); verticalLineTo(19.5f); horizontalLineTo(3.5f); close()
            moveTo(3.5f, 8.8f); horizontalLineTo(20.5f)
        }
    }

    /** Cross-Functional Swimlanes pool. */
    val Swimlanes: ImageVector by lazy {
        strokeIcon("VexioSwimlanes") {
            moveTo(3.0f, 4.5f); horizontalLineTo(21.0f); verticalLineTo(19.5f); horizontalLineTo(3.0f); close()
            moveTo(7.2f, 4.5f); verticalLineTo(19.5f)
            moveTo(3.0f, 12.0f); horizontalLineTo(21.0f)
        }
    }

    /** Callout note with leader line. */
    val CalloutNote: ImageVector by lazy {
        strokeIcon("VexioCalloutNote") {
            moveTo(5.0f, 4.0f); horizontalLineTo(20.0f); verticalLineTo(14.5f); horizontalLineTo(10.5f)
            lineTo(5.5f, 19.5f); verticalLineTo(14.5f); horizontalLineTo(5.0f); close()
        }
    }

    /** Left Sidebar Pane toggle (like VS Code / Figma / Visio). */
    val SidebarLeft: ImageVector by lazy {
        strokeIcon("VexioSidebarLeft") {
            moveTo(3.5f, 4.5f); horizontalLineTo(20.5f); verticalLineTo(19.5f); horizontalLineTo(3.5f); close()
            moveTo(9.2f, 4.5f); verticalLineTo(19.5f)
        }
    }

    /** Right Inspector Pane toggle. */
    val SidebarRight: ImageVector by lazy {
        strokeIcon("VexioSidebarRight") {
            moveTo(3.5f, 4.5f); horizontalLineTo(20.5f); verticalLineTo(19.5f); horizontalLineTo(3.5f); close()
            moveTo(14.8f, 4.5f); verticalLineTo(19.5f)
        }
    }

    /** Bottom ShapeSheet Studio Pane toggle. */
    val BottomPanel: ImageVector by lazy {
        strokeIcon("VexioBottomPanel") {
            moveTo(3.5f, 4.5f); horizontalLineTo(20.5f); verticalLineTo(19.5f); horizontalLineTo(3.5f); close()
            moveTo(3.5f, 14.2f); horizontalLineTo(20.5f)
            moveTo(9.5f, 14.2f); verticalLineTo(19.5f)
        }
    }

    /** 2-Column Visual Grid View toggle for Stencil Drawer. */
    val Grid2Col: ImageVector by lazy {
        strokeIcon("VexioGrid2Col", strokeWidth = 1.65f) {
            moveTo(3.5f, 3.5f); horizontalLineTo(10.5f); verticalLineTo(10.5f); horizontalLineTo(3.5f); close()
            moveTo(13.5f, 3.5f); horizontalLineTo(20.5f); verticalLineTo(10.5f); horizontalLineTo(13.5f); close()
            moveTo(3.5f, 13.5f); horizontalLineTo(10.5f); verticalLineTo(20.5f); horizontalLineTo(3.5f); close()
            moveTo(13.5f, 13.5f); horizontalLineTo(20.5f); verticalLineTo(20.5f); horizontalLineTo(13.5f); close()
        }
    }

    /** 1-Column Compact List View toggle for Stencil Drawer. */
    val ListView: ImageVector by lazy {
        strokeIcon("VexioListView", strokeWidth = 1.65f) {
            moveTo(4.0f, 6.0f); horizontalLineTo(7.0f)
            moveTo(10.0f, 6.0f); horizontalLineTo(20.0f)
            moveTo(4.0f, 12.0f); horizontalLineTo(7.0f)
            moveTo(10.0f, 12.0f); horizontalLineTo(20.0f)
            moveTo(4.0f, 18.0f); horizontalLineTo(7.0f)
            moveTo(10.0f, 18.0f); horizontalLineTo(20.0f)
        }
    }

    /** Layer Eye Visible icon (replaces `"Vis"` text hack). */
    val EyeVisible: ImageVector by lazy {
        strokeIcon("VexioEyeVisible", strokeWidth = 1.65f) {
            moveTo(2.5f, 12.0f)
            cubicTo(5.5f, 6.2f, 18.5f, 6.2f, 21.5f, 12.0f)
            cubicTo(18.5f, 17.8f, 5.5f, 17.8f, 2.5f, 12.0f)
            close()
            moveTo(14.5f, 12.0f)
            arcTo(2.5f, 2.5f, 0f, true, true, 9.5f, 12.0f)
            arcTo(2.5f, 2.5f, 0f, true, true, 14.5f, 12.0f)
        }
    }

    /** Layer Eye Hidden icon with diagonal slash (replaces `"Hid"` text hack). */
    val EyeHidden: ImageVector by lazy {
        strokeIcon("VexioEyeHidden", strokeWidth = 1.65f) {
            moveTo(2.5f, 12.0f)
            cubicTo(5.5f, 6.2f, 18.5f, 6.2f, 21.5f, 12.0f)
            cubicTo(18.5f, 17.8f, 5.5f, 17.8f, 2.5f, 12.0f)
            moveTo(4.0f, 4.0f)
            lineTo(20.0f, 20.0f)
        }
    }

    /** Crisp Fit-Page-to-Window icon. */
    val FitToPage: ImageVector by lazy {
        strokeIcon("VexioFitToPage", strokeWidth = 1.65f) {
            moveTo(3.5f, 8.5f); verticalLineTo(3.5f); horizontalLineTo(8.5f)
            moveTo(15.5f, 3.5f); horizontalLineTo(20.5f); verticalLineTo(8.5f)
            moveTo(20.5f, 15.5f); verticalLineTo(20.5f); horizontalLineTo(15.5f)
            moveTo(8.5f, 20.5f); horizontalLineTo(3.5f); verticalLineTo(15.5f)
            moveTo(8.0f, 8.0f); horizontalLineTo(16.0f); verticalLineTo(16.0f); horizontalLineTo(8.0f); close()
        }
    }

    /** Crisp L-Ruler icon for Canvas Rulers toggle. */
    val RulerCorner: ImageVector by lazy {
        strokeIcon("VexioRulerCorner", strokeWidth = 1.65f) {
            moveTo(3.5f, 3.5f)
            horizontalLineTo(20.5f)
            verticalLineTo(8.5f)
            horizontalLineTo(8.5f)
            verticalLineTo(20.5f)
            horizontalLineTo(3.5f)
            close()
            moveTo(12.5f, 3.5f); verticalLineTo(6.2f)
            moveTo(16.5f, 3.5f); verticalLineTo(6.2f)
            moveTo(3.5f, 12.5f); horizontalLineTo(6.2f)
            moveTo(3.5f, 16.5f); horizontalLineTo(6.2f)
        }
    }

    /** Line-end: No Arrowhead (`0`). */
    val ArrowEndNone: ImageVector by lazy {
        strokeIcon("VexioArrowNone") {
            moveTo(3.5f, 12.0f); horizontalLineTo(20.5f)
        }
    }

    /** Line-end: Open Arrow (`1`). */
    val ArrowEndOpen: ImageVector by lazy {
        strokeIcon("VexioArrowOpen") {
            moveTo(3.5f, 12.0f); horizontalLineTo(19.5f)
            moveTo(14.2f, 7.2f); lineTo(19.8f, 12.0f); lineTo(14.2f, 16.8f)
        }
    }

    /** Line-end: Triangle Arrow (`4`). */
    val ArrowEndTriangle: ImageVector by lazy {
        strokeIcon("VexioArrowTriangle") {
            moveTo(3.5f, 12.0f); horizontalLineTo(13.5f)
            moveTo(13.5f, 7.2f); lineTo(20.2f, 12.0f); lineTo(13.5f, 16.8f); close()
        }
    }

    /** Line-end: Diamond (`11`). */
    val ArrowEndDiamond: ImageVector by lazy {
        strokeIcon("VexioArrowDiamond") {
            moveTo(3.5f, 12.0f); horizontalLineTo(11.5f)
            moveTo(11.5f, 12.0f); lineTo(15.8f, 7.8f); lineTo(20.2f, 12.0f); lineTo(15.8f, 16.2f); close()
        }
    }

    /** Line-end: ERD Crow's Foot (`27`). */
    val ArrowEndCrowsFoot: ImageVector by lazy {
        strokeIcon("VexioArrowCrowsFoot") {
            moveTo(3.5f, 12.0f); horizontalLineTo(20.0f)
            moveTo(13.5f, 8.0f); verticalLineTo(16.0f)
            moveTo(14.5f, 12.0f); lineTo(20.2f, 7.2f)
            moveTo(14.5f, 12.0f); lineTo(20.2f, 16.8f)
        }
    }
}
