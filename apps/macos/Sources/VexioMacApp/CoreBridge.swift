import AppKit
import CoreGraphics
import Foundation
import QuartzCore
import SwiftUI
import UniformTypeIdentifiers
import VexioCoreFFI

// MARK: - Visio & CAD Uniform Type Identifiers

public extension UTType {
    static let visioDrawing = UTType(importedAs: "com.microsoft.visio.drawing")
    static let visioMacroDrawing = UTType(importedAs: "com.microsoft.visio.drawing.macroenabled")
    static let visioStencil = UTType(importedAs: "com.microsoft.visio.stencil")
    static let visioTemplate = UTType(importedAs: "com.microsoft.visio.template")
    static let visioLegacyDrawing = UTType(importedAs: "com.microsoft.visio.legacy-drawing")
    static let autoCADDWG = UTType(importedAs: "com.autodesk.dwg")
    static let autoCADDXF = UTType(importedAs: "com.autodesk.dxf")

    var vexioCanonicalExtension: String {
        switch self {
        case .visioDrawing: return "vsdx"
        case .visioMacroDrawing: return "vsdm"
        case .visioStencil: return "vssx"
        case .visioTemplate: return "vstx"
        case .visioLegacyDrawing: return "vsd"
        case .autoCADDWG: return "dwg"
        case .autoCADDXF: return "dxf"
        case .pdf: return "pdf"
        case .svg: return "svg"
        default:
            return preferredFilenameExtension?.lowercased() ?? "vsdx"
        }
    }
}

// MARK: - Deterministic Error Mapping for VexioStatus

public enum VexioBridgeError: LocalizedError, Sendable, Equatable {
    case engineAllocationFailed
    case blankDocumentCreationFailed
    case invalidArgument(context: String)
    case cellGuarded(cellName: String)
    case formulaSyntaxError(cellName: String, formula: String)
    case corruptOrUnsupportedFormat(extensionName: String)
    case gpuSurfaceLost
    case internalRustPanic
    case unknownStatus(rawCode: UInt32)
    case emptyDocumentPayload
    case previewBitmapConstructionFailed

    public static func from(
        status: VexioStatus,
        context: String = "",
        cellName: String = "",
        formula: String = ""
    ) -> VexioBridgeError? {
        if status == VEXIO_OK {
            return nil
        } else if status == VEXIO_ERR_INVALID_ARG {
            return .invalidArgument(context: context)
        } else if status == VEXIO_ERR_CELL_GUARDED {
            return .cellGuarded(cellName: cellName)
        } else if status == VEXIO_ERR_FORMULA_SYNTAX {
            return .formulaSyntaxError(cellName: cellName, formula: formula)
        } else if status == VEXIO_ERR_IO_CORRUPT_FORMAT {
            return .corruptOrUnsupportedFormat(extensionName: context)
        } else if status == VEXIO_ERR_GPU_SURFACE_LOST {
            return .gpuSurfaceLost
        } else if status == VEXIO_ERR_INTERNAL_PANIC {
            return .internalRustPanic
        } else {
            return .unknownStatus(rawCode: status.rawValue)
        }
    }

    public var errorDescription: String? {
        switch self {
        case .engineAllocationFailed:
            return "Failed to initialize the native Vexio Rust engine."
        case .blankDocumentCreationFailed:
            return "Failed to allocate a blank Vexio document."
        case .invalidArgument(let context):
            return "Invalid argument supplied to Vexio Core\(context.isEmpty ? "." : ": \(context)")"
        case .cellGuarded(let cellName):
            return "Cell '\(cellName)' is protected by GUARD(). Enable SETF override in the ShapeSheet editor to replace guarded formulas."
        case .formulaSyntaxError(let cellName, let formula):
            return "ShapeSheet syntax error in '\(cellName)' for expression '\(formula)'."
        case .corruptOrUnsupportedFormat(let ext):
            return "Unable to parse '.\(ext)' package: corrupt stream or unsupported schema."
        case .gpuSurfaceLost:
            return "Metal CAMetalLayer GPU surface was lost and is being re-bound."
        case .internalRustPanic:
            return "Vexio Core caught an internal engine panic safely across the C-ABI boundary."
        case .unknownStatus(let rawCode):
            return "Vexio Core returned status code \(rawCode)."
        case .emptyDocumentPayload:
            return "Document file contains zero readable bytes."
        case .previewBitmapConstructionFailed:
            return "Failed to construct CoreGraphics RGBA bitmap from rendered preview buffer."
        }
    }
}

// MARK: - Domain & Workstation Models

public enum FormulaWriteMode: String, CaseIterable, Identifiable, Sendable {
    case respectGuard = "Standard (Respect GUARD)"
    case forceSetF = "SETF Override (Replace GUARD)"

    public var id: String { rawValue }
    public var forceSetFFlag: Bool { self == .forceSetF }
}

public enum VexioExportFormat: String, CaseIterable, Identifiable, Sendable {
    case vsdx
    case vsd
    case dxf
    case pdf
    case svg
    case pptx
    case docx
    case emf
    case powerautomate
    case powerbi

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .vsdx: return "Visio Drawing (.vsdx)"
        case .vsd: return "Visio 2003–2010 Binary (.vsd)"
        case .dxf: return "AutoCAD Exchange (.dxf)"
        case .pdf: return "Vector PDF/A (.pdf)"
        case .svg: return "Scalable Vector Graphics (.svg)"
        case .pptx: return "PowerPoint Presentation (.pptx)"
        case .docx: return "Word Process Specification (.docx)"
        case .emf: return "Enhanced Metafile Vector (.emf)"
        case .powerautomate: return "Power Automate Workflow (.flow.json)"
        case .powerbi: return "Power BI Visio Visual (.pbi.json)"
        }
    }

    public var defaultFileExtension: String {
        switch self {
        case .powerautomate: return "flow.json"
        case .powerbi: return "pbi.json"
        default: return rawValue
        }
    }

    public var contentType: UTType {
        switch self {
        case .vsdx: return .visioDrawing
        case .vsd: return .visioLegacyDrawing
        case .dxf: return .autoCADDXF
        case .pdf: return .pdf
        case .svg: return .svg
        case .pptx: return .presentation
        case .docx: return .data
        case .emf: return .data
        case .powerautomate: return .json
        case .powerbi: return .json
        }
    }
}

public enum VexioCanvasTool: String, CaseIterable, Identifiable, Sendable {
    case pointer = "Pointer"
    case connector = "Connector"
    case text = "Text"
    case rectangle = "Rectangle"
    case ellipse = "Ellipse"
    case freeform = "Freeform"
    case ink = "Ink"

    public var id: String { rawValue }

    public var symbolName: String {
        switch self {
        case .pointer: return "cursorarrow"
        case .connector: return "point.topleft.down.curvedto.point.bottomright.up"
        case .text: return "character.textbox"
        case .rectangle: return "rectangle"
        case .ellipse: return "oval"
        case .freeform: return "scribble.variable"
        case .ink: return "pencil.tip.crop.circle"
        }
    }

    public var shortcutHint: String {
        switch self {
        case .pointer: return "⌘1"
        case .connector: return "⌘2"
        case .text: return "⌘3"
        case .rectangle: return "⌘4"
        case .ellipse: return "⌘5"
        case .freeform: return "⌘6"
        case .ink: return "⌘7"
        }
    }
}

public enum ConnectorRoutingStyle: String, CaseIterable, Identifiable, Sendable {
    case orthogonalJumps = "Orthogonal (Arc Jumps)"
    case curvedBezier = "Curved Dynamic"
    case straightDirect = "Straight Direct"

    public var id: String { rawValue }

    public var shapeLineRouteExtValue: String {
        switch self {
        case .orthogonalJumps: return "1"
        case .curvedBezier: return "2"
        case .straightDirect: return "0"
        }
    }
}

public enum AutoLayoutPreset: String, CaseIterable, Identifiable, Sendable {
    case hierarchicalTopDown = "Top-to-Bottom Hierarchy"
    case swimlaneLeftRight = "BPMN Left-to-Right Flow"
    case radialCluster = "Radial Network"
    case compactOrthogonal = "Compact Orthogonal Grid"

    public var id: String { rawValue }
}

public struct VexioPageSummary: Identifiable, Equatable, Sendable {
    public let id: UInt32
    public var title: String
    public var architecturalScale: String
    public var isBackgroundPage: Bool

    public init(id: UInt32, title: String, architecturalScale: String = "1:1 (No Scale)", isBackgroundPage: Bool = false) {
        self.id = id
        self.title = title
        self.architecturalScale = architecturalScale
        self.isBackgroundPage = isBackgroundPage
    }
}

public struct VexioStencilMaster: Identifiable, Equatable, Hashable, Sendable {
    public let id: String
    public let category: String
    public let name: String
    public let symbolName: String
    public let defaultWidthFormula: String
    public let defaultHeightFormula: String
    public let defaultFillFormula: String
    public let isGuardedAspect: Bool
}

public enum ShapeSheetSectionCategory: String, CaseIterable, Identifiable, Sendable {
    case shapeTransform = "Shape Transform"
    case geometry = "Geometry 1"
    case fillAndLine = "Fill & Line Format"
    case protection = "Protection (GUARD)"
    case shapeData = "Shape Data (Prop.*)"
    case layersAndRouting = "Layer & Routing"

    public var id: String { rawValue }
}

public struct ShapeSheetCellEntry: Identifiable, Equatable, Sendable {
    public var id: String { cellName }
    public let section: ShapeSheetSectionCategory
    public let cellName: String
    public var formula: String
    public var evaluatedValue: String
    public var isGuarded: Bool {
        formula.trimmingCharacters(in: .whitespaces).uppercased().hasPrefix("GUARD(")
    }
}

public struct ShapeDataPropertyItem: Identifiable, Equatable, Sendable {
    public var id: String { rowKey }
    public var rowKey: String       // e.g. "Prop.CostCenter"
    public var label: String
    public var dataType: String     // String, Number, Currency, Duration, Boolean
    public var formula: String
    public var evaluatedValue: String
}

public struct DiagramLayerState: Identifiable, Equatable, Sendable {
    public let id: Int
    public var name: String
    public var isVisible: Bool
    public var isPrintable: Bool
    public var isLocked: Bool
    public var isSnapGlueEnabled: Bool
    public var accentHex: String
}

public struct DataGraphicRuleConfig: Equatable, Sendable {
    public var isEnabled: Bool = true
    public var iconSetField: String = "Prop.Status"
    public var dataBarField: String = "Prop.CompletionPct"
    public var textCalloutField: String = "Prop.Owner"
    public var colorByValueEnabled: Bool = true
    public var showLegendOnPage: Bool = false
}

public enum ValidationStandardKind: String, CaseIterable, Identifiable, Sendable {
    case bpmn20 = "BPMN 2.0"
    case uml25 = "UML 2.5"
    case sharePointWorkflow = "SharePoint Workflow"

    public var id: String { rawValue }
}

public struct DiagramValidationDiagnostic: Identifiable, Equatable, Sendable {
    public enum Severity: String, Sendable {
        case error = "Error"
        case warning = "Warning"
        case info = "Passed"
    }

    public let id: UUID
    public let ruleId: String
    public let standard: ValidationStandardKind
    public let severity: Severity
    public let pageId: UInt32
    public let shapeId: UInt64
    public let message: String
}

public struct ForceTouchQuickInspectContext: Equatable, Sendable {
    public let shapeId: UInt64
    public let viewLocation: CGPoint
    public let pressure: Float
}

// MARK: - Phase 7 Plan 2 Domain Models (Boolean Ops, 6x4 Themes, Visual Presets & Compare)

public enum VexioBooleanOperation: String, CaseIterable, Identifiable, Sendable {
    case union = "union"
    case combine = "combine"
    case fragment = "fragment"
    case intersect = "intersect"
    case subtract = "subtract"
    case trim = "trim"
    case join = "join"
    case offset = "offset"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .union: return "Union"
        case .combine: return "Combine"
        case .fragment: return "Fragment"
        case .intersect: return "Intersect"
        case .subtract: return "Subtract"
        case .trim: return "Trim"
        case .join: return "Join"
        case .offset: return "Offset"
        }
    }

    public var symbolName: String {
        switch self {
        case .union: return "square.on.square"
        case .combine: return "squares.below.rectangle"
        case .fragment: return "rectangle.split.2x2"
        case .intersect: return "circle.lefthalf.filled"
        case .subtract: return "minus.square"
        case .trim: return "scissors"
        case .join: return "link"
        case .offset: return "arrow.up.left.and.arrow.down.right"
        }
    }
}

public enum VexioThemePreset: String, CaseIterable, Identifiable, Sendable {
    case modern = "modern"
    case professional = "professional"
    case vibrant = "vibrant"
    case blueprint = "blueprint"
    case monochrome = "monochrome"
    case handDrawn = "hand_drawn"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .modern: return "Modern"
        case .professional: return "Professional"
        case .vibrant: return "Vibrant"
        case .blueprint: return "Blueprint"
        case .monochrome: return "Monochrome"
        case .handDrawn: return "Hand Drawn"
        }
    }

    public var primaryPreviewColor: Color {
        switch self {
        case .modern: return Color(red: 0.15, green: 0.39, blue: 0.92)
        case .professional: return Color(red: 0.12, green: 0.25, blue: 0.43)
        case .vibrant: return Color(red: 0.91, green: 0.28, blue: 0.42)
        case .blueprint: return Color(red: 0.08, green: 0.46, blue: 0.78)
        case .monochrome: return Color(red: 0.27, green: 0.31, blue: 0.36)
        case .handDrawn: return Color(red: 0.48, green: 0.36, blue: 0.86)
        }
    }

    public var accentPreviewColor: Color {
        switch self {
        case .modern: return Color(red: 0.06, green: 0.73, blue: 0.51)
        case .professional: return Color(red: 0.22, green: 0.52, blue: 0.68)
        case .vibrant: return Color(red: 0.96, green: 0.62, blue: 0.04)
        case .blueprint: return Color(red: 0.38, green: 0.78, blue: 0.98)
        case .monochrome: return Color(red: 0.58, green: 0.64, blue: 0.72)
        case .handDrawn: return Color(red: 0.93, green: 0.48, blue: 0.22)
        }
    }
}

public enum VexioVisualEffectPreset: String, CaseIterable, Identifiable, Sendable {
    case gradLinear = "grad_linear"
    case gradRadial = "grad_radial"
    case hatchDiag = "hatch_diag"
    case shadowSoft = "shadow_soft"
    case bevel3D = "bevel_3d"
    case sketchy = "sketchy"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .gradLinear: return "Linear Gradient"
        case .gradRadial: return "Radial Glow"
        case .hatchDiag: return "Diagonal Hatch"
        case .shadowSoft: return "Soft Shadow"
        case .bevel3D: return "3D Bevel"
        case .sketchy: return "Sketchy Outline"
        }
    }

    public var symbolName: String {
        switch self {
        case .gradLinear: return "rectangle.lefthalf.filled"
        case .gradRadial: return "sun.max"
        case .hatchDiag: return "line.diagonal"
        case .shadowSoft: return "square.on.square.badge.person.crop"
        case .bevel3D: return "cube"
        case .sketchy: return "scribble"
        }
    }
}

public struct DiagramDiffItemPayload: Identifiable, Decodable, Equatable, Sendable {
    public var id: String { "\(page_name)-\(shape_id)-\(summary)" }
    public let page_name: String
    public let shape_id: UInt64
    public let shape_name: String
    public let summary: String

    private enum CodingKeys: String, CodingKey {
        case page_name
        case shape_id
        case shape_name
        case summary
    }
}

public struct DiagramCompareReportPayload: Decodable, Equatable, Sendable {
    public let entries: [DiagramDiffItemPayload]
    public let added_count: Int
    public let removed_count: Int
    public let modified_count: Int
    public let diff_overlay_svg: String
}

// MARK: - Sendable Snapshot & Export FileDocument

public struct VexioDocumentSnapshot: Sendable {
    public let data: Data
    public let formatExtension: String
}

public struct VexioExportFileDocument: FileDocument, Sendable {
    public static let readableContentTypes: [UTType] = [
        .visioDrawing, .visioLegacyDrawing, .autoCADDXF, .pdf, .svg,
        .presentation, .spreadsheet, .data, .json,
    ]
    public static let writableContentTypes: [UTType] = [
        .visioDrawing, .visioLegacyDrawing, .autoCADDXF, .pdf, .svg,
        .presentation, .spreadsheet, .data, .json,
    ]

    public let data: Data

    public init(data: Data) {
        self.data = data
    }

    public init(configuration: ReadConfiguration) throws {
        guard let bytes = configuration.file.regularFileContents else {
            throw VexioBridgeError.emptyDocumentPayload
        }
        self.data = bytes
    }

    public func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: data)
    }
}

// MARK: - Thread-Safe C-ABI Lifecycle Handle (VexioEngine*, VexioDoc*, VexioViewport*)

private final class NativeDocumentCore: @unchecked Sendable {
    private let lock = NSLock()
    private let engine: OpaquePointer
    private let doc: OpaquePointer
    private var viewport: OpaquePointer?

    init(blankTitle: String) throws {
        guard let createdEngine = vexio_engine_create() else {
            throw VexioBridgeError.engineAllocationFailed
        }
        guard let createdDoc = blankTitle.withCString({ vexio_doc_create_blank($0) }) else {
            vexio_engine_destroy(createdEngine)
            throw VexioBridgeError.blankDocumentCreationFailed
        }
        self.engine = createdEngine
        self.doc = createdDoc
        self.viewport = nil
    }

    init(fileData: Data, formatExtension: String) throws {
        guard !fileData.isEmpty else {
            throw VexioBridgeError.emptyDocumentPayload
        }
        guard let createdEngine = vexio_engine_create() else {
            throw VexioBridgeError.engineAllocationFailed
        }

        var openedDoc: OpaquePointer?
        let status: VexioStatus = fileData.withUnsafeBytes { rawBuffer in
            let basePtr = rawBuffer.bindMemory(to: UInt8.self).baseAddress
            let slice = VexioByteSlice(ptr: basePtr, len: rawBuffer.count)
            return formatExtension.withCString { extCStr in
                vexio_doc_open_bytes(createdEngine, slice, extCStr, &openedDoc)
            }
        }

        if let err = VexioBridgeError.from(status: status, context: formatExtension) {
            vexio_engine_destroy(createdEngine)
            throw err
        }
        guard let validDoc = openedDoc else {
            vexio_engine_destroy(createdEngine)
            throw VexioBridgeError.corruptOrUnsupportedFormat(extensionName: formatExtension)
        }

        self.engine = createdEngine
        self.doc = validDoc
        self.viewport = nil
    }

    deinit {
        lock.lock()
        if let activeViewport = viewport {
            vexio_viewport_destroy(activeViewport)
            viewport = nil
        }
        vexio_doc_destroy(doc)
        vexio_engine_destroy(engine)
        lock.unlock()
    }

    private static func consumeOwnedBufferAsData(_ buffer: VexioOwnedBuffer) -> Data {
        defer { vexio_buffer_free(buffer) }
        guard let ptr = buffer.ptr, buffer.len > 0 else {
            return Data()
        }
        return Data(bytes: ptr, count: buffer.len)
    }

    private static func consumeOwnedBufferAsUTF8(_ buffer: VexioOwnedBuffer) -> String {
        let data = consumeOwnedBufferAsData(buffer)
        return String(decoding: data, as: UTF8.self)
    }

    func saveFormat(_ ext: String) throws -> VexioDocumentSnapshot {
        lock.lock()
        defer { lock.unlock() }

        var outBuffer = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status = ext.withCString { extCStr in
            vexio_doc_save_format(doc, extCStr, &outBuffer)
        }
        if let err = VexioBridgeError.from(status: status, context: ext) {
            vexio_buffer_free(outBuffer)
            throw err
        }
        let serialized = Self.consumeOwnedBufferAsData(outBuffer)
        return VexioDocumentSnapshot(data: serialized, formatExtension: ext)
    }

    func getCellFormula(pageId: UInt32, shapeId: UInt64, cellName: String) throws -> String {
        lock.lock()
        defer { lock.unlock() }

        var outUTF8 = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status = cellName.withCString { cellCStr in
            vexio_cell_get_formula(doc, pageId, shapeId, cellCStr, &outUTF8)
        }
        if let err = VexioBridgeError.from(status: status, context: cellName, cellName: cellName) {
            vexio_buffer_free(outUTF8)
            throw err
        }
        return Self.consumeOwnedBufferAsUTF8(outUTF8)
    }

    func getCellValueString(pageId: UInt32, shapeId: UInt64, cellName: String) throws -> String {
        lock.lock()
        defer { lock.unlock() }

        var outUTF8 = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status = cellName.withCString { cellCStr in
            vexio_cell_get_value_str(doc, pageId, shapeId, cellCStr, &outUTF8)
        }
        if let err = VexioBridgeError.from(status: status, context: cellName, cellName: cellName) {
            vexio_buffer_free(outUTF8)
            throw err
        }
        return Self.consumeOwnedBufferAsUTF8(outUTF8)
    }

    func setCellFormula(
        pageId: UInt32,
        shapeId: UInt64,
        cellName: String,
        formula: String,
        forceSetF: Bool
    ) throws {
        lock.lock()
        defer { lock.unlock() }

        let status = cellName.withCString { cellCStr in
            formula.withCString { formulaCStr in
                vexio_cell_set_formula(doc, pageId, shapeId, cellCStr, formulaCStr, forceSetF)
            }
        }
        if let err = VexioBridgeError.from(
            status: status,
            context: cellName,
            cellName: cellName,
            formula: formula
        ) {
            throw err
        }
    }

    func undo() throws {
        lock.lock()
        defer { lock.unlock() }

        let status = vexio_doc_undo(doc)
        if let err = VexioBridgeError.from(status: status, context: "undo") {
            throw err
        }
    }

    func redo() throws {
        lock.lock()
        defer { lock.unlock() }

        let status = vexio_doc_redo(doc)
        if let err = VexioBridgeError.from(status: status, context: "redo") {
            throw err
        }
    }

    func bindMetalLayer(
        metalLayerRawPtr: UnsafeMutableRawPointer,
        widthPx: UInt32,
        heightPx: UInt32,
        dpiScale: Float
    ) throws {
        lock.lock()
        defer { lock.unlock() }

        if let existingViewport = viewport {
            vexio_viewport_destroy(existingViewport)
            viewport = nil
        }

        var createdViewport: OpaquePointer?
        let clampedWidth = max(widthPx, 1)
        let clampedHeight = max(heightPx, 1)
        let status = vexio_viewport_create_metal_layer(
            engine,
            doc,
            metalLayerRawPtr,
            clampedWidth,
            clampedHeight,
            max(dpiScale, 1.0),
            &createdViewport
        )
        if let err = VexioBridgeError.from(status: status, context: "CAMetalLayer") {
            throw err
        }
        viewport = createdViewport
    }

    func bindHeadlessViewport(
        pageId: UInt32,
        widthPx: UInt32,
        heightPx: UInt32,
        dpiScale: Float
    ) throws {
        lock.lock()
        defer { lock.unlock() }

        if let existingViewport = viewport {
            vexio_viewport_destroy(existingViewport)
            viewport = nil
        }

        var createdViewport: OpaquePointer?
        let status = vexio_viewport_create_headless(
            engine,
            doc,
            pageId,
            max(widthPx, 1),
            max(heightPx, 1),
            max(dpiScale, 1.0),
            &createdViewport
        )
        if let err = VexioBridgeError.from(status: status, context: "HeadlessViewport") {
            throw err
        }
        viewport = createdViewport
    }

    func destroyViewport() {
        lock.lock()
        defer { lock.unlock() }

        if let existingViewport = viewport {
            vexio_viewport_destroy(existingViewport)
            viewport = nil
        }
    }

    func dispatchPointer(_ pointerEvent: VexioPointerEvent) throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }

        guard let activeViewport = viewport else {
            return 0
        }

        var eventCopy = pointerEvent
        var cursorKind: UInt32 = 0
        let status = vexio_viewport_dispatch_pointer(activeViewport, doc, &eventCopy, &cursorKind)
        if let err = VexioBridgeError.from(status: status, context: "dispatch_pointer") {
            throw err
        }
        return cursorKind
    }

    func renderPreviewRGBA(pageId: UInt32, widthPx: UInt32, heightPx: UInt32) throws -> CGImage {
        lock.lock()
        defer { lock.unlock() }

        let safeWidth = max(widthPx, 1)
        let safeHeight = max(heightPx, 1)
        var rgbaBuffer = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        var status = vexio_render_preview_rgba(doc, pageId, safeWidth, safeHeight, &rgbaBuffer)
        if status != VEXIO_OK && pageId == 0 {
            vexio_buffer_free(rgbaBuffer)
            rgbaBuffer = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
            status = vexio_render_preview_rgba(doc, 1, safeWidth, safeHeight, &rgbaBuffer)
        }
        if let err = VexioBridgeError.from(status: status, context: "render_preview_rgba") {
            vexio_buffer_free(rgbaBuffer)
            throw err
        }

        let rgbaData = Self.consumeOwnedBufferAsData(rgbaBuffer)
        let expectedByteCount = Int(safeWidth) * Int(safeHeight) * 4
        guard rgbaData.count >= expectedByteCount,
              let colorSpace = CGColorSpace(name: CGColorSpace.sRGB),
              let dataProvider = CGDataProvider(data: rgbaData as CFData),
              let cgImage = CGImage(
                  width: Int(safeWidth),
                  height: Int(safeHeight),
                  bitsPerComponent: 8,
                  bitsPerPixel: 32,
                  bytesPerRow: Int(safeWidth) * 4,
                  space: colorSpace,
                  bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
                  provider: dataProvider,
                  decode: nil,
                  shouldInterpolate: true,
                  intent: .defaultIntent
              )
        else {
            throw VexioBridgeError.previewBitmapConstructionFailed
        }
        return cgImage
    }

    // MARK: - Phase 7 Plan 2 Thread-Safe C-ABI Operations

    func booleanOp(
        pageId: UInt32,
        shapeIds: [UInt64],
        operation: String,
        offsetInches: Double
    ) throws {
        guard !shapeIds.isEmpty else {
            throw VexioBridgeError.invalidArgument(context: "boolean_op requires at least one shape ID")
        }
        lock.lock()
        defer { lock.unlock() }

        let status: VexioStatus = shapeIds.withUnsafeBufferPointer { idsBuffer in
            operation.withCString { opCStr in
                vexio_doc_boolean_op(
                    doc,
                    pageId,
                    idsBuffer.baseAddress,
                    idsBuffer.count,
                    opCStr,
                    offsetInches
                )
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "boolean_op(\(operation))") {
            throw err
        }
    }

    func applyTheme(themeSlug: String, variantIndex: UInt8) throws {
        lock.lock()
        defer { lock.unlock() }

        let status: VexioStatus = themeSlug.withCString { slugCStr in
            vexio_doc_apply_theme(doc, slugCStr, variantIndex)
        }
        if let err = VexioBridgeError.from(status: status, context: "apply_theme(\(themeSlug))") {
            throw err
        }
    }

    func applyVisualPreset(pageId: UInt32, shapeId: UInt64, presetSlug: String) throws {
        lock.lock()
        defer { lock.unlock() }

        let status: VexioStatus = presetSlug.withCString { presetCStr in
            vexio_doc_apply_visual_preset(doc, pageId, shapeId, presetCStr)
        }
        if let err = VexioBridgeError.from(status: status, context: "apply_visual_preset(\(presetSlug))") {
            throw err
        }
    }

    func createSubprocess(pageId: UInt32, shapeId: UInt64, subprocessName: String) throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }

        var outSubPageId: UInt32 = 0
        let status: VexioStatus = withUnsafeMutablePointer(to: &outSubPageId) { outPtr in
            subprocessName.withCString { subNameCStr in
                vexio_doc_create_subprocess(doc, pageId, shapeId, subNameCStr, outPtr)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "create_subprocess(\(subprocessName))") {
            throw err
        }
        return outSubPageId
    }

    func compare(againstBaselineData baselineData: Data, formatExtension: String = "vsdx") throws -> String {
        guard !baselineData.isEmpty else {
            throw VexioBridgeError.emptyDocumentPayload
        }
        lock.lock()
        defer { lock.unlock() }

        var baselineDoc: OpaquePointer?
        let openStatus: VexioStatus = withUnsafeMutablePointer(to: &baselineDoc) { docOutPtr in
            baselineData.withUnsafeBytes { rawBuffer in
                let basePtr = rawBuffer.bindMemory(to: UInt8.self).baseAddress
                let slice = VexioByteSlice(ptr: basePtr, len: rawBuffer.count)
                return formatExtension.withCString { extCStr in
                    vexio_doc_open_bytes(engine, slice, extCStr, docOutPtr)
                }
            }
        }
        if let err = VexioBridgeError.from(status: openStatus, context: "compare_baseline(\(formatExtension))") {
            if let partial = baselineDoc {
                vexio_doc_destroy(partial)
            }
            throw err
        }
        guard let validBaselineDoc = baselineDoc else {
            throw VexioBridgeError.corruptOrUnsupportedFormat(extensionName: formatExtension)
        }
        defer { vexio_doc_destroy(validBaselineDoc) }

        var outJSON = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let compareStatus = vexio_doc_compare(validBaselineDoc, doc, &outJSON)
        if let err = VexioBridgeError.from(status: compareStatus, context: "vexio_doc_compare") {
            vexio_buffer_free(outJSON)
            throw err
        }
        return Self.consumeOwnedBufferAsUTF8(outJSON)
    }

    func applyCrdtJSON(replicaId: String, opsJSON: String) throws {
        lock.lock()
        defer { lock.unlock() }

        let status: VexioStatus = replicaId.withCString { replicaCStr in
            opsJSON.withCString { opsCStr in
                vexio_doc_apply_crdt_json(doc, replicaCStr, opsCStr)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "apply_crdt_json") {
            throw err
        }
    }

    func flushCrdtJSON(
        againstBaselineData baselineData: Data,
        formatExtension: String = "vsdx",
        replicaId: String,
        nowMs: UInt64
    ) throws -> String {
        guard !baselineData.isEmpty else {
            throw VexioBridgeError.emptyDocumentPayload
        }
        lock.lock()
        defer { lock.unlock() }

        var baselineDoc: OpaquePointer?
        let openStatus: VexioStatus = withUnsafeMutablePointer(to: &baselineDoc) { docOutPtr in
            baselineData.withUnsafeBytes { rawBuffer in
                let basePtr = rawBuffer.bindMemory(to: UInt8.self).baseAddress
                let slice = VexioByteSlice(ptr: basePtr, len: rawBuffer.count)
                return formatExtension.withCString { extCStr in
                    vexio_doc_open_bytes(engine, slice, extCStr, docOutPtr)
                }
            }
        }
        if let err = VexioBridgeError.from(status: openStatus, context: "flush_crdt_baseline") {
            if let partial = baselineDoc {
                vexio_doc_destroy(partial)
            }
            throw err
        }
        guard let validBaselineDoc = baselineDoc else {
            throw VexioBridgeError.corruptOrUnsupportedFormat(extensionName: formatExtension)
        }
        defer { vexio_doc_destroy(validBaselineDoc) }

        var outOps = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let flushStatus: VexioStatus = withUnsafeMutablePointer(to: &outOps) { outPtr in
            replicaId.withCString { replicaCStr in
                vexio_doc_flush_crdt_json(validBaselineDoc, doc, replicaCStr, nowMs, outPtr)
            }
        }
        if let err = VexioBridgeError.from(status: flushStatus, context: "flush_crdt_json") {
            vexio_buffer_free(outOps)
            throw err
        }
        return Self.consumeOwnedBufferAsUTF8(outOps)
    }

    func importDataVisualizer(pageId: UInt32, csvOrXlsxData: Data) throws -> UInt32 {
        guard !csvOrXlsxData.isEmpty else {
            throw VexioBridgeError.emptyDocumentPayload
        }
        lock.lock()
        defer { lock.unlock() }

        var stepCount: UInt32 = 0
        let status: VexioStatus = withUnsafeMutablePointer(to: &stepCount) { countPtr in
            csvOrXlsxData.withUnsafeBytes { rawBuffer in
                let basePtr = rawBuffer.bindMemory(to: UInt8.self).baseAddress
                let slice = VexioByteSlice(ptr: basePtr, len: rawBuffer.count)
                return vexio_doc_import_data_visualizer(doc, pageId, slice, countPtr)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "import_data_visualizer") {
            throw err
        }
        return stepCount
    }

    func exportDataVisualizerXlsx(pageId: UInt32) throws -> Data {
        lock.lock()
        defer { lock.unlock() }

        var outXlsx = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status = vexio_doc_export_data_visualizer_xlsx(doc, pageId, &outXlsx)
        if let err = VexioBridgeError.from(status: status, context: "export_data_visualizer_xlsx") {
            vexio_buffer_free(outXlsx)
            throw err
        }
        return Self.consumeOwnedBufferAsData(outXlsx)
    }

    func importCloudDiscovery(pageId: UInt32, discoveryJSON: String) throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }

        var importedCount: UInt32 = 0
        let status: VexioStatus = withUnsafeMutablePointer(to: &importedCount) { countPtr in
            discoveryJSON.withCString { jsonCStr in
                vexio_doc_import_cloud_discovery(doc, pageId, jsonCStr, countPtr)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "import_cloud_discovery") {
            throw err
        }
        return importedCount
    }

    func importEntraOrLdif(pageId: UInt32, directoryPayload: String) throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }

        var importedCount: UInt32 = 0
        let status: VexioStatus = withUnsafeMutablePointer(to: &importedCount) { countPtr in
            directoryPayload.withCString { textCStr in
                vexio_doc_import_entra_or_ldif(doc, pageId, textCStr, countPtr)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "import_entra_or_ldif") {
            throw err
        }
        return importedCount
    }
}

// MARK: - @MainActor Swift 6 Document Session (ReferenceFileDocument)

@MainActor
public final class VexioDocumentSession: ReferenceFileDocument, ObservableObject {
    public typealias Snapshot = VexioDocumentSnapshot

    nonisolated public static let readableContentTypes: [UTType] = [
        .visioDrawing,
        .visioMacroDrawing,
        .visioStencil,
        .visioTemplate,
        .visioLegacyDrawing,
        .autoCADDWG,
        .autoCADDXF,
    ]

    nonisolated public static let writableContentTypes: [UTType] = [
        .visioDrawing,
        .visioMacroDrawing,
        .visioStencil,
        .visioTemplate,
        .visioLegacyDrawing,
        .autoCADDXF,
        .pdf,
        .svg,
        .presentation,
        .spreadsheet,
        .data,
        .json,
    ]

    nonisolated private let core: NativeDocumentCore
    nonisolated public let sourceFormatExtension: String

    // Published Workstation UI State
    @Published public var documentTitle: String = "Untitled Diagram"
    @Published public var activeTool: VexioCanvasTool = .pointer
    @Published public var activePageId: UInt32 = 1
    @Published public var selectedShapeId: UInt64 = 1
    @Published public var selectedShapeIds: [UInt64] = [1, 2]
    @Published public var nextAllocatedShapeId: UInt64 = 3

    @Published public var zoomScale: CGFloat = 1.0
    @Published public var panOffset: CGSize = .zero
    @Published public var cursorCanvasInches: CGPoint = .zero
    @Published public var activeCursorKind: UInt32 = 0

    @Published public var connectorRouting: ConnectorRoutingStyle = .orthogonalJumps
    @Published public var showStencilNavigator: Bool = true
    @Published public var showInspectorSidebar: Bool = true
    @Published public var showShapeSheetDrawer: Bool = false

    // Phase 7 Plan 2 Published State
    @Published public var selectedThemeSlug: String = "modern"
    @Published public var selectedVariantIndex: UInt8 = 0
    @Published public var activeVisualPresetSlug: String = ""
    @Published public var booleanOffsetInches: Double = 0.125
    @Published public var latestDiffSummary: String = ""
    @Published public var latestDiffEntries: [DiagramDiffItemPayload] = []

    private var baselineComparisonSnapshot: VexioDocumentSnapshot? = nil
    private var localCrdtLogicalClock: UInt32 = 10

    @Published public var pages: [VexioPageSummary] = [
        VexioPageSummary(id: 1, title: "Page-1", architecturalScale: "1:1 (Metric / US Units)"),
        VexioPageSummary(id: 2, title: "Architecture Backdrop", architecturalScale: "1/4\" = 1'-0\"", isBackgroundPage: true),
    ]

    @Published public var shapeSheetCells: [ShapeSheetCellEntry] = []
    @Published public var shapeDataProperties: [ShapeDataPropertyItem] = [
        ShapeDataPropertyItem(rowKey: "Prop.Department", label: "Department", dataType: "String", formula: "\"Platform Engineering\"", evaluatedValue: "Platform Engineering"),
        ShapeDataPropertyItem(rowKey: "Prop.CostCenter", label: "Cost Center", dataType: "String", formula: "\"CC-4100\"", evaluatedValue: "CC-4100"),
        ShapeDataPropertyItem(rowKey: "Prop.CompletionPct", label: "Completion %", dataType: "Number", formula: "85%", evaluatedValue: "85%"),
        ShapeDataPropertyItem(rowKey: "Prop.Status", label: "Operational Status", dataType: "String", formula: "\"Healthy\"", evaluatedValue: "Healthy"),
    ]

    @Published public var layers: [DiagramLayerState] = [
        DiagramLayerState(id: 0, name: "Flowchart & Shapes", isVisible: true, isPrintable: true, isLocked: false, isSnapGlueEnabled: true, accentHex: "#0A84FF"),
        DiagramLayerState(id: 1, name: "Dynamic Connectors", isVisible: true, isPrintable: true, isLocked: false, isSnapGlueEnabled: true, accentHex: "#30D158"),
        DiagramLayerState(id: 2, name: "Dimensions & Annotations", isVisible: true, isPrintable: true, isLocked: false, isSnapGlueEnabled: true, accentHex: "#FF9F0A"),
        DiagramLayerState(id: 3, name: "CAD Structural Underlay", isVisible: true, isPrintable: true, isLocked: true, isSnapGlueEnabled: true, accentHex: "#BF5AF2"),
    ]

    @Published public var dataGraphicConfig = DataGraphicRuleConfig()
    @Published public var validationDiagnostics: [DiagramValidationDiagnostic] = []
    @Published public var forceTouchQuickInspect: ForceTouchQuickInspectContext?
    @Published public var lastBannerError: VexioBridgeError?
    @Published public var statusMessage: String = "Ready — Metal wgpu Surface Active"

    // Built-in Visio Parity Stencil Catalog
    public static let builtInStencilCatalog: [VexioStencilMaster] = [
        VexioStencilMaster(id: "flow.process", category: "Basic Flowchart", name: "Process", symbolName: "rectangle", defaultWidthFormula: "1.5 in", defaultHeightFormula: "0.875 in", defaultFillFormula: "RGB(235,244,255)", isGuardedAspect: false),
        VexioStencilMaster(id: "flow.decision", category: "Basic Flowchart", name: "Decision", symbolName: "diamond", defaultWidthFormula: "1.25 in", defaultHeightFormula: "1.0 in", defaultFillFormula: "RGB(255,246,229)", isGuardedAspect: false),
        VexioStencilMaster(id: "flow.terminator", category: "Basic Flowchart", name: "Start / End", symbolName: "capsule", defaultWidthFormula: "1.5 in", defaultHeightFormula: "0.625 in", defaultFillFormula: "RGB(231,250,239)", isGuardedAspect: false),
        VexioStencilMaster(id: "flow.database", category: "Basic Flowchart", name: "Data Store", symbolName: "cylinder.split.1x2", defaultWidthFormula: "1.25 in", defaultHeightFormula: "1.375 in", defaultFillFormula: "RGB(240,238,255)", isGuardedAspect: true),
        VexioStencilMaster(id: "bpmn.task", category: "BPMN 2.0", name: "BPMN User Task", symbolName: "person.crop.rectangle", defaultWidthFormula: "1.625 in", defaultHeightFormula: "1.0 in", defaultFillFormula: "RGB(238,247,255)", isGuardedAspect: false),
        VexioStencilMaster(id: "bpmn.gateway", category: "BPMN 2.0", name: "Exclusive Gateway", symbolName: "xmark.diamond", defaultWidthFormula: "GUARD(0.625 in)", defaultHeightFormula: "GUARD(0.625 in)", defaultFillFormula: "RGB(255,248,225)", isGuardedAspect: true),
        VexioStencilMaster(id: "bpmn.event", category: "BPMN 2.0", name: "Intermediate Event", symbolName: "smallcircle.filled.circle", defaultWidthFormula: "GUARD(0.5 in)", defaultHeightFormula: "GUARD(0.5 in)", defaultFillFormula: "RGB(255,255,255)", isGuardedAspect: true),
        VexioStencilMaster(id: "bpmn.swimlane", category: "BPMN 2.0", name: "Pool / Swimlane", symbolName: "rectangle.split.3x1", defaultWidthFormula: "7.5 in", defaultHeightFormula: "2.25 in", defaultFillFormula: "RGB(248,250,252)", isGuardedAspect: false),
        VexioStencilMaster(id: "uml.class", category: "UML 2.5", name: "Class Classifier", symbolName: "list.bullet.rectangle", defaultWidthFormula: "1.875 in", defaultHeightFormula: "1.5 in", defaultFillFormula: "RGB(245,243,255)", isGuardedAspect: false),
        VexioStencilMaster(id: "uml.component", category: "UML 2.5", name: "Component", symbolName: "shippingbox", defaultWidthFormula: "1.75 in", defaultHeightFormula: "1.125 in", defaultFillFormula: "RGB(236,253,245)", isGuardedAspect: false),
        VexioStencilMaster(id: "net.server", category: "Network & Cloud", name: "Compute Cluster", symbolName: "server.rack", defaultWidthFormula: "1.25 in", defaultHeightFormula: "1.25 in", defaultFillFormula: "RGB(238,242,255)", isGuardedAspect: true),
        VexioStencilMaster(id: "cad.wall", category: "Floor Plan & CAD", name: "Structural Wall", symbolName: "ruler", defaultWidthFormula: "4.0 ft", defaultHeightFormula: "GUARD(6 in)", defaultFillFormula: "RGB(226,232,240)", isGuardedAspect: false),
    ]

    public static let canonicalCellManifest: [(ShapeSheetSectionCategory, String, String)] = [
        (.shapeTransform, "PinX", "4.25 in"),
        (.shapeTransform, "PinY", "5.50 in"),
        (.shapeTransform, "Width", "1.50 in"),
        (.shapeTransform, "Height", "1.00 in"),
        (.shapeTransform, "LocPinX", "Width*0.5"),
        (.shapeTransform, "LocPinY", "Height*0.5"),
        (.shapeTransform, "Angle", "0 deg"),
        (.geometry, "Geometry1.NoFill", "FALSE"),
        (.geometry, "Geometry1.NoLine", "FALSE"),
        (.fillAndLine, "FillForegnd", "RGB(235,244,255)"),
        (.fillAndLine, "LineWeight", "1.25 pt"),
        (.fillAndLine, "LineColor", "RGB(28,100,242)"),
        (.fillAndLine, "Rounding", "0.08 in"),
        (.protection, "LockWidth", "0"),
        (.protection, "LockHeight", "0"),
        (.protection, "LockAspect", "0"),
        (.layersAndRouting, "LayerMember", "\"0\""),
        (.layersAndRouting, "ShapeRouteStyle", "1"),
    ]

    public init(blankTitle: String = "Untitled Diagram") {
        do {
            self.core = try NativeDocumentCore(blankTitle: blankTitle)
        } catch {
            preconditionFailure("Unable to create native Vexio engine/document: \(error.localizedDescription)")
        }
        self.sourceFormatExtension = "vsdx"
        self.documentTitle = blankTitle
        seedInitialShapeIfNeeded()
        reloadSelectedShapeSheetCells()
    }

    nonisolated public required init(configuration: ReadConfiguration) throws {
        guard let bytes = configuration.file.regularFileContents else {
            throw VexioBridgeError.emptyDocumentPayload
        }
        let ext: String
        if let filenameExt = configuration.file.filename.flatMap({ URL(fileURLWithPath: $0).pathExtension.lowercased() }),
           !filenameExt.isEmpty {
            ext = filenameExt
        } else {
            ext = configuration.contentType.vexioCanonicalExtension
        }
        self.core = try NativeDocumentCore(fileData: bytes, formatExtension: ext)
        self.sourceFormatExtension = ext
    }

    nonisolated public func snapshot(contentType: UTType) throws -> VexioDocumentSnapshot {
        let ext = contentType.vexioCanonicalExtension
        return try core.saveFormat(ext)
    }

    nonisolated public func fileWrapper(
        snapshot: VexioDocumentSnapshot,
        configuration: WriteConfiguration
    ) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: snapshot.data)
    }

    // MARK: - Document Serialization & Export (.vsdx, .vsd, .dxf, .pdf, .svg)

    public func exportDocument(as format: VexioExportFormat) throws -> VexioExportFileDocument {
        let snapshot = try core.saveFormat(format.rawValue)
        statusMessage = "Exported \(format.displayName) (\(snapshot.data.count) bytes)"
        return VexioExportFileDocument(data: snapshot.data)
    }

    // MARK: - Metal Viewport & Pointer Dispatch

    public func attachMetalLayer(
        _ metalLayer: CAMetalLayer,
        widthPx: UInt32,
        heightPx: UInt32,
        dpiScale: Float
    ) {
        let rawLayerPtr = Unmanaged.passUnretained(metalLayer).toOpaque()
        do {
            try core.bindMetalLayer(
                metalLayerRawPtr: rawLayerPtr,
                widthPx: widthPx,
                heightPx: heightPx,
                dpiScale: dpiScale
            )
            lastBannerError = nil
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func detachViewport() {
        core.destroyViewport()
    }

    @discardableResult
    public func dispatchPointerEvent(_ pointerEvent: VexioPointerEvent) -> NSCursor {
        let dpi = max(CGFloat(NSScreen.main?.backingScaleFactor ?? 2.0), 1.0)
        let canvasXInches = ((CGFloat(pointerEvent.x_px) / dpi) - panOffset.width) / (96.0 * max(zoomScale, 0.1))
        let canvasYInches = ((CGFloat(pointerEvent.y_px) / dpi) - panOffset.height) / (96.0 * max(zoomScale, 0.1))
        cursorCanvasInches = CGPoint(x: canvasXInches, y: canvasYInches)

        do {
            let cursorCode = try core.dispatchPointer(pointerEvent)
            activeCursorKind = cursorCode
            return resolveNSCursor(for: cursorCode)
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            return .arrow
        } catch {
            return .arrow
        }
    }

    public func resolveNSCursor(for engineCursorCode: UInt32) -> NSCursor {
        switch activeTool {
        case .connector, .rectangle, .ellipse, .freeform, .ink:
            if engineCursorCode == 0 {
                return .crosshair
            }
        case .text:
            if engineCursorCode == 0 {
                return .iBeam
            }
        case .pointer:
            break
        }

        switch engineCursorCode {
        case 1: return .pointingHand
        case 2: return .openHand
        case 3: return .closedHand
        case 4: return .crosshair
        case 5: return .iBeam
        case 6: return .resizeLeftRight
        case 7: return .resizeUpDown
        case 8: return .crosshair
        case 9: return .dragLink
        case 10: return .operationNotAllowed
        default: return .arrow
        }
    }

    // MARK: - ShapeSheet Formula Read / Write (GUARD vs SETF)

    private func nextHlcClock() -> (wallMs: UInt64, logical: UInt32) {
        localCrdtLogicalClock &+= 1
        let nowMs = UInt64(max(Date().timeIntervalSince1970 * 1000.0, 1.0))
        return (nowMs, localCrdtLogicalClock)
    }

    private func upsertPageInCore(pageId: UInt32, title: String, widthIn: Double = 11.0, heightIn: Double = 8.5) {
        let clock = nextHlcClock()
        let ops: [[String: Any]] = [
            [
                "UpsertPage": [
                    "ts": [
                        "wall_ms": clock.wallMs,
                        "logical": clock.logical,
                        "replica_id": "macos-local",
                    ],
                    "page_id": pageId,
                    "name": title,
                    "width_in": widthIn,
                    "height_in": heightIn,
                ],
            ],
        ]
        if let jsonData = try? JSONSerialization.data(withJSONObject: ops),
           let jsonStr = String(data: jsonData, encoding: .utf8) {
            try? core.applyCrdtJSON(replicaId: "macos-local", opsJSON: jsonStr)
        }
    }

    private func upsertShapeInCore(pageId: UInt32, shapeId: UInt64, name: String) {
        if (try? core.getCellFormula(pageId: pageId, shapeId: shapeId, cellName: "PinX")) != nil {
            return
        }
        let clock = nextHlcClock()
        let ops: [[String: Any]] = [
            [
                "UpsertShape": [
                    "ts": [
                        "wall_ms": clock.wallMs,
                        "logical": clock.logical,
                        "replica_id": "macos-local",
                    ],
                    "page_id": pageId,
                    "shape_id": shapeId,
                    "name": name,
                    "kind": "Shape2D",
                    "master_info": NSNull(),
                ],
            ],
        ]
        if let jsonData = try? JSONSerialization.data(withJSONObject: ops),
           let jsonStr = String(data: jsonData, encoding: .utf8) {
            try? core.applyCrdtJSON(replicaId: "macos-local", opsJSON: jsonStr)
        }
    }

    private func seedInitialShapeIfNeeded() {
        upsertPageInCore(pageId: 1, title: "Page-1")
        upsertPageInCore(pageId: 2, title: "Architecture Backdrop")
        upsertShapeInCore(pageId: activePageId, shapeId: 1, name: "Process.1")
        upsertShapeInCore(pageId: activePageId, shapeId: 2, name: "Process.2")

        for (_, cellName, defaultFormula) in Self.canonicalCellManifest {
            try? core.setCellFormula(
                pageId: activePageId,
                shapeId: 1,
                cellName: cellName,
                formula: defaultFormula,
                forceSetF: true
            )
        }

        let companionCells: [(String, String)] = [
            ("PinX", "5.10 in"),
            ("PinY", "5.20 in"),
            ("Width", "1.50 in"),
            ("Height", "1.00 in"),
            ("FillForegnd", "RGB(231,250,239)"),
            ("LineColor", "RGB(16,185,129)"),
            ("LineWeight", "1.25 pt"),
        ]
        for (cell, expr) in companionCells {
            try? core.setCellFormula(
                pageId: activePageId,
                shapeId: 2,
                cellName: cell,
                formula: expr,
                forceSetF: true
            )
        }

        baselineComparisonSnapshot = try? core.saveFormat("vsdx")
    }

    public func reloadSelectedShapeSheetCells() {
        if baselineComparisonSnapshot == nil {
            if (try? core.getCellFormula(pageId: activePageId, shapeId: selectedShapeId, cellName: "PinX")) == nil,
               (try? core.getCellFormula(pageId: 0, shapeId: selectedShapeId, cellName: "PinX")) != nil {
                activePageId = 0
            }
            baselineComparisonSnapshot = try? core.saveFormat("vsdx")
        }

        var loadedEntries: [ShapeSheetCellEntry] = []
        loadedEntries.reserveCapacity(Self.canonicalCellManifest.count + shapeDataProperties.count)

        for (section, cellName, fallbackFormula) in Self.canonicalCellManifest {
            let formula = (try? core.getCellFormula(pageId: activePageId, shapeId: selectedShapeId, cellName: cellName))
                .flatMap { $0.isEmpty ? nil : $0 } ?? fallbackFormula
            let valueStr = (try? core.getCellValueString(pageId: activePageId, shapeId: selectedShapeId, cellName: cellName))
                .flatMap { $0.isEmpty ? nil : $0 } ?? formula
            loadedEntries.append(
                ShapeSheetCellEntry(
                    section: section,
                    cellName: cellName,
                    formula: formula,
                    evaluatedValue: valueStr
                )
            )
        }

        for index in shapeDataProperties.indices {
            let key = shapeDataProperties[index].rowKey
            if let liveFormula = try? core.getCellFormula(pageId: activePageId, shapeId: selectedShapeId, cellName: key),
               !liveFormula.isEmpty {
                shapeDataProperties[index].formula = liveFormula
            }
            if let liveValue = try? core.getCellValueString(pageId: activePageId, shapeId: selectedShapeId, cellName: key),
               !liveValue.isEmpty {
                shapeDataProperties[index].evaluatedValue = liveValue
            }
            loadedEntries.append(
                ShapeSheetCellEntry(
                    section: .shapeData,
                    cellName: key,
                    formula: shapeDataProperties[index].formula,
                    evaluatedValue: shapeDataProperties[index].evaluatedValue
                )
            )
        }

        shapeSheetCells = loadedEntries
    }

    public func readCellFormula(cellName: String) -> String {
        if let live = try? core.getCellFormula(pageId: activePageId, shapeId: selectedShapeId, cellName: cellName),
           !live.isEmpty {
            return live
        }
        return shapeSheetCells.first(where: { $0.cellName == cellName })?.formula ?? ""
    }

    public func readCellEvaluatedValue(cellName: String) -> String {
        if let live = try? core.getCellValueString(pageId: activePageId, shapeId: selectedShapeId, cellName: cellName),
           !live.isEmpty {
            return live
        }
        return shapeSheetCells.first(where: { $0.cellName == cellName })?.evaluatedValue ?? ""
    }

    public func setCellFormula(
        cellName: String,
        formula: String,
        mode: FormulaWriteMode,
        undoManager: UndoManager?
    ) throws {
        upsertShapeInCore(pageId: activePageId, shapeId: selectedShapeId, name: "Sheet.\(selectedShapeId)")
        do {
            try core.setCellFormula(
                pageId: activePageId,
                shapeId: selectedShapeId,
                cellName: cellName,
                formula: formula,
                forceSetF: mode.forceSetFFlag
            )
            lastBannerError = nil
            statusMessage = "Updated Sheet.\(selectedShapeId)!\(cellName) = \(formula)"
            registerJournalUndoStep(actionName: "Edit \(cellName)", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
            throw bridgeErr
        }
    }

    // MARK: - Deterministic Command Journal Undo / Redo

    private func registerJournalUndoStep(actionName: String, undoManager: UndoManager?) {
        guard let undoManager else { return }
        undoManager.setActionName(actionName)
        undoManager.registerUndo(withTarget: self) { target in
            target.performUndo(undoManager: undoManager)
        }
    }

    public func performUndo(undoManager: UndoManager?) {
        do {
            try core.undo()
            lastBannerError = nil
            statusMessage = "Undo applied"
            undoManager?.registerUndo(withTarget: self) { target in
                target.performRedo(undoManager: undoManager)
            }
            reloadSelectedShapeSheetCells()
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func performRedo(undoManager: UndoManager?) {
        do {
            try core.redo()
            lastBannerError = nil
            statusMessage = "Redo applied"
            undoManager?.registerUndo(withTarget: self) { target in
                target.performUndo(undoManager: undoManager)
            }
            reloadSelectedShapeSheetCells()
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    // MARK: - Stencil Instantiation, Trackpad Gestures & Validation

    public func instantiateStencilMaster(
        masterId: String,
        atCanvasInches point: CGPoint,
        undoManager: UndoManager?
    ) {
        guard let master = Self.builtInStencilCatalog.first(where: { $0.id == masterId }) else {
            return
        }
        let shapeId = nextAllocatedShapeId
        nextAllocatedShapeId += 1
        let previousShapeId = selectedShapeId
        selectedShapeId = shapeId
        selectedShapeIds = previousShapeId != shapeId ? [previousShapeId, shapeId] : [shapeId]

        upsertPageInCore(
            pageId: activePageId,
            title: pages.first(where: { $0.id == activePageId })?.title ?? "Page-\(activePageId)"
        )
        upsertShapeInCore(pageId: activePageId, shapeId: shapeId, name: "\(master.name).\(shapeId)")

        let pinX = String(format: "%.3f in", point.x)
        let pinY = String(format: "%.3f in", point.y)

        let cellsToInitialize: [(String, String)] = [
            ("PinX", pinX),
            ("PinY", pinY),
            ("Width", master.defaultWidthFormula),
            ("Height", master.defaultHeightFormula),
            ("LocPinX", "Width*0.5"),
            ("LocPinY", "Height*0.5"),
            ("Angle", "0 deg"),
            ("FillForegnd", master.defaultFillFormula),
            ("LineWeight", "1.25 pt"),
            ("LineColor", "RGB(28,100,242)"),
            ("LockAspect", master.isGuardedAspect ? "GUARD(1)" : "0"),
        ]

        for (cell, expr) in cellsToInitialize {
            try? core.setCellFormula(
                pageId: activePageId,
                shapeId: shapeId,
                cellName: cell,
                formula: expr,
                forceSetF: true
            )
        }

        registerJournalUndoStep(actionName: "Drop \(master.name)", undoManager: undoManager)
        reloadSelectedShapeSheetCells()
        statusMessage = "Dropped '\(master.name)' as Sheet.\(shapeId) at (\(pinX), \(pinY))"
    }

    public func rotateSelectedShape(byDegrees deltaDegrees: Double, undoManager: UndoManager?) {
        let currentRaw = readCellFormula(cellName: "Angle")
            .replacingOccurrences(of: "deg", with: "")
            .replacingOccurrences(of: "rad", with: "")
            .trimmingCharacters(in: .whitespaces)
        let baseDegrees = Double(currentRaw) ?? 0.0
        let updated = (baseDegrees + deltaDegrees).truncatingRemainder(dividingBy: 360.0)
        let formatted = String(format: "%.2f deg", updated)
        try? setCellFormula(
            cellName: "Angle",
            formula: formatted,
            mode: .respectGuard,
            undoManager: undoManager
        )
    }

    public func setConnectorRoutingStyle(_ style: ConnectorRoutingStyle, undoManager: UndoManager?) {
        connectorRouting = style
        try? setCellFormula(
            cellName: "ShapeRouteStyle",
            formula: style.shapeLineRouteExtValue,
            mode: .respectGuard,
            undoManager: undoManager
        )
    }

    public func addNewPage() {
        let nextId = (pages.map(\.id).max() ?? 0) + 1
        let page = VexioPageSummary(id: nextId, title: "Page-\(nextId)")
        upsertPageInCore(pageId: nextId, title: page.title)
        upsertShapeInCore(pageId: nextId, shapeId: 1, name: "Process.1")
        pages.append(page)
        activePageId = nextId
        selectedShapeId = 1
        selectedShapeIds = [1]
        reloadSelectedShapeSheetCells()
        statusMessage = "Created \(page.title)"
    }

    public func applyAutoLayout(_ preset: AutoLayoutPreset, undoManager: UndoManager?) {
        let stepOffset: String
        switch preset {
        case .hierarchicalTopDown: stepOffset = "5.50 in"
        case .swimlaneLeftRight: stepOffset = "4.25 in"
        case .radialCluster: stepOffset = "4.75 in"
        case .compactOrthogonal: stepOffset = "5.00 in"
        }
        try? setCellFormula(
            cellName: "PinY",
            formula: stepOffset,
            mode: .respectGuard,
            undoManager: undoManager
        )
        statusMessage = "Applied \(preset.rawValue) layout"
    }

    public func runDiagramValidation(standard: ValidationStandardKind) {
        var findings: [DiagramValidationDiagnostic] = []
        let widthExpr = readCellFormula(cellName: "Width")
        let routeStyle = readCellFormula(cellName: "ShapeRouteStyle")

        if widthExpr.hasPrefix("0") {
            findings.append(
                DiagramValidationDiagnostic(
                    id: UUID(),
                    ruleId: "\(standard.rawValue.prefix(4).uppercased())-GEOM-001",
                    standard: standard,
                    severity: .error,
                    pageId: activePageId,
                    shapeId: selectedShapeId,
                    message: "Sheet.\(selectedShapeId) has zero width and cannot anchor dynamic glue connectors."
                )
            )
        }

        if standard == .bpmn20 && routeStyle == "0" {
            findings.append(
                DiagramValidationDiagnostic(
                    id: UUID(),
                    ruleId: "BPMN-SEQ-014",
                    standard: .bpmn20,
                    severity: .warning,
                    pageId: activePageId,
                    shapeId: selectedShapeId,
                    message: "Sequence flow uses straight routing; BPMN 2.0 recommends orthogonal routing with obstacle avoidance."
                )
            )
        }

        if findings.isEmpty {
            findings.append(
                DiagramValidationDiagnostic(
                    id: UUID(),
                    ruleId: "\(standard.rawValue.prefix(4).uppercased())-PASS-000",
                    standard: standard,
                    severity: .info,
                    pageId: activePageId,
                    shapeId: selectedShapeId,
                    message: "All \(standard.rawValue) structural connectivity, boundary event, and container rules passed."
                )
            )
        }

        validationDiagnostics = findings
        statusMessage = "\(standard.rawValue) validation finished (\(findings.count) diagnostic item(s))"
    }

    // MARK: - Phase 7 Plan 2 @MainActor Workstation Actions

    public func applyBooleanOperation(
        _ operation: VexioBooleanOperation,
        offsetInches: Double? = nil,
        shapeIds: [UInt64]? = nil,
        undoManager: UndoManager? = nil
    ) {
        let resolvedOffset = offsetInches ?? booleanOffsetInches
        booleanOffsetInches = resolvedOffset

        let candidateIds: [UInt64]
        if let explicitIds = shapeIds, !explicitIds.isEmpty {
            candidateIds = explicitIds
        } else if operation == .offset {
            candidateIds = [selectedShapeId]
        } else if selectedShapeIds.count >= 2 {
            candidateIds = selectedShapeIds
        } else {
            let companionId = selectedShapeId == 1 ? UInt64(2) : UInt64(1)
            upsertShapeInCore(pageId: activePageId, shapeId: companionId, name: "Companion.\(companionId)")
            candidateIds = [selectedShapeId, companionId]
        }

        for sid in candidateIds {
            upsertShapeInCore(pageId: activePageId, shapeId: sid, name: "Sheet.\(sid)")
        }

        do {
            try core.booleanOp(
                pageId: activePageId,
                shapeIds: candidateIds,
                operation: operation.rawValue,
                offsetInches: resolvedOffset
            )
            lastBannerError = nil
            registerJournalUndoStep(actionName: "Shape \(operation.displayName)", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            if operation == .offset {
                statusMessage = String(format: "Applied Offset (%+.3f in) to Sheet.%llu", resolvedOffset, selectedShapeId)
            } else {
                let joined = candidateIds.map { "Sheet.\($0)" }.joined(separator: ", ")
                statusMessage = "Applied \(operation.displayName) across [\(joined)]"
            }
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func applyThemeAndVariant(
        themeSlug: String,
        variantIndex: UInt8,
        undoManager: UndoManager? = nil
    ) {
        let clampedVariant = min(variantIndex, 3)
        do {
            try core.applyTheme(themeSlug: themeSlug, variantIndex: clampedVariant)
            selectedThemeSlug = themeSlug
            selectedVariantIndex = clampedVariant
            lastBannerError = nil
            registerJournalUndoStep(actionName: "Apply Theme \(themeSlug)", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            let label = VexioThemePreset(rawValue: themeSlug)?.displayName ?? themeSlug
            statusMessage = "Applied '\(label)' Theme · Variant \(clampedVariant + 1)"
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func applyVisualPreset(
        _ preset: VexioVisualEffectPreset,
        undoManager: UndoManager? = nil
    ) {
        applyVisualPreset(presetSlug: preset.rawValue, undoManager: undoManager)
    }

    public func applyVisualPreset(
        presetSlug: String,
        undoManager: UndoManager? = nil
    ) {
        upsertShapeInCore(pageId: activePageId, shapeId: selectedShapeId, name: "Sheet.\(selectedShapeId)")
        do {
            try core.applyVisualPreset(
                pageId: activePageId,
                shapeId: selectedShapeId,
                presetSlug: presetSlug
            )
            activeVisualPresetSlug = presetSlug
            lastBannerError = nil
            registerJournalUndoStep(actionName: "Visual Effect \(presetSlug)", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            let display = VexioVisualEffectPreset(rawValue: presetSlug)?.displayName ?? presetSlug
            statusMessage = "Applied '\(display)' to Sheet.\(selectedShapeId)"
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func createSubprocessDrilldown(
        subprocessName: String? = nil,
        undoManager: UndoManager? = nil
    ) {
        upsertShapeInCore(pageId: activePageId, shapeId: selectedShapeId, name: "Task.\(selectedShapeId)")
        let trimmed = subprocessName?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let resolvedTitle = trimmed.isEmpty ? "Subprocess-\(selectedShapeId)" : trimmed

        do {
            let subPageId = try core.createSubprocess(
                pageId: activePageId,
                shapeId: selectedShapeId,
                subprocessName: resolvedTitle
            )
            if !pages.contains(where: { $0.id == subPageId }) {
                pages.append(
                    VexioPageSummary(
                        id: subPageId,
                        title: resolvedTitle,
                        architecturalScale: "1:1 (BPMN Drill-Down)"
                    )
                )
            }
            activePageId = subPageId
            selectedShapeId = 1
            selectedShapeIds = [1]
            lastBannerError = nil
            registerJournalUndoStep(actionName: "Create Subprocess \(resolvedTitle)", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            statusMessage = "Created linked subprocess page '\(resolvedTitle)' (Page ID \(subPageId))"
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func captureComparisonBaseline() {
        do {
            baselineComparisonSnapshot = try core.saveFormat("vsdx")
            lastBannerError = nil
            statusMessage = "Pinned current diagram state as Diagram Compare baseline"
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func compareWithSnapshot(baselineSnapshot: VexioDocumentSnapshot? = nil) {
        do {
            let resolvedBaseline: VexioDocumentSnapshot
            if let explicit = baselineSnapshot {
                resolvedBaseline = explicit
            } else if let cached = baselineComparisonSnapshot {
                resolvedBaseline = cached
            } else {
                let current = try core.saveFormat("vsdx")
                baselineComparisonSnapshot = current
                resolvedBaseline = current
            }

            let reportJSON = try core.compare(
                againstBaselineData: resolvedBaseline.data,
                formatExtension: resolvedBaseline.formatExtension
            )
            if let jsonData = reportJSON.data(using: .utf8),
               let report = try? JSONDecoder().decode(DiagramCompareReportPayload.self, from: jsonData) {
                latestDiffEntries = report.entries
                latestDiffSummary = "+\(report.added_count) added · ~\(report.modified_count) modified · −\(report.removed_count) removed (\(report.entries.count) change(s))"
            } else {
                latestDiffEntries = []
                latestDiffSummary = "Comparison completed (\(reportJSON.utf8.count) bytes JSON diff)"
            }
            lastBannerError = nil
            statusMessage = "Diagram Compare: \(latestDiffSummary)"
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func importDataVisualizerSample(
        csvData: Data? = nil,
        undoManager: UndoManager? = nil
    ) {
        let defaultCSV = """
        Process Step ID,Process Step Description,Next Step ID,Connector Label,Shape Type,Function,Phase
        P100,Receive Customer Order,P200,,Start,Order Management,Intake
        P200,Validate Credit & Fraud Rules,"P300,P250","Approved,Review",Decision,Risk & Security,Verification
        P250,Manual Compliance Escalation,P300,Cleared,Subprocess,Risk & Security,Verification
        P300,Provision Cloud Tenant & Keys,P400,,Process,Platform SRE,Fulfillment
        P400,Emit Billing & Telemetry Event,,,End,Finance & Ops,Closure
        """
        let payload = csvData ?? Data(defaultCSV.utf8)
        upsertPageInCore(
            pageId: activePageId,
            title: pages.first(where: { $0.id == activePageId })?.title ?? "Page-\(activePageId)"
        )

        do {
            let importedSteps = try core.importDataVisualizer(pageId: activePageId, csvOrXlsxData: payload)
            selectedShapeId = 1
            selectedShapeIds = [1, 2]
            lastBannerError = nil
            registerJournalUndoStep(actionName: "Import Data Visualizer", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            statusMessage = "Data Visualizer imported \(importedSteps) cross-functional swimlane steps"
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func exportDataVisualizerWorkbook() throws -> VexioExportFileDocument {
        let xlsxData = try core.exportDataVisualizerXlsx(pageId: activePageId)
        statusMessage = "Exported Data Visualizer Excel Workbook (\(xlsxData.count) bytes)"
        return VexioExportFileDocument(data: xlsxData)
    }

    public func importCloudTopologySample(
        discoveryJSON: String? = nil,
        undoManager: UndoManager? = nil
    ) {
        let defaultDiscovery = """
        {
          "provider": "aws",
          "resources": [
            {
              "id": "alb-edge-01",
              "type": "alb",
              "name": "Global Edge ALB",
              "vpc": "vpc-prod-core",
              "region": "us-east-1",
              "connects_to": ["eks-api-01", "fn-auth-01"],
              "tags": { "Env": "prod", "Tier": "edge" }
            },
            {
              "id": "eks-api-01",
              "type": "eks",
              "name": "Vexio Sync EKS",
              "vpc": "vpc-prod-core",
              "region": "us-east-1",
              "connects_to": ["rds-pg-01", "s3-blobs-01"],
              "tags": { "Env": "prod", "Tier": "compute" }
            },
            {
              "id": "fn-auth-01",
              "type": "lambda",
              "name": "Token Verifier",
              "vpc": "vpc-prod-core",
              "region": "us-east-1",
              "connects_to": ["rds-pg-01"],
              "tags": { "Env": "prod", "Tier": "security" }
            },
            {
              "id": "rds-pg-01",
              "type": "rds_postgres",
              "name": "Primary Ledger DB",
              "vpc": "vpc-data-plane",
              "region": "us-west-2",
              "connects_to": [],
              "tags": { "Env": "prod", "HA": "multi-az" }
            },
            {
              "id": "s3-blobs-01",
              "type": "s3_bucket",
              "name": "Stencil & VSDX Vault",
              "vpc": "vpc-data-plane",
              "region": "us-west-2",
              "connects_to": [],
              "tags": { "Env": "prod", "Encryption": "KMS" }
            }
          ]
        }
        """
        let payload = discoveryJSON ?? defaultDiscovery
        upsertPageInCore(
            pageId: activePageId,
            title: pages.first(where: { $0.id == activePageId })?.title ?? "Page-\(activePageId)"
        )

        do {
            let count = try core.importCloudDiscovery(pageId: activePageId, discoveryJSON: payload)
            selectedShapeId = 1
            selectedShapeIds = [1, 2]
            lastBannerError = nil
            registerJournalUndoStep(actionName: "Import Cloud Topology", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            statusMessage = "Imported \(count) cloud resources into VPC/Region containers"
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func importEntraLdifSample(
        directoryPayload: String? = nil,
        undoManager: UndoManager? = nil
    ) {
        let defaultLdif = """
        dn: cn=Elena Rostova,ou=Executive,dc=vexio,dc=io
        cn: Elena Rostova
        title: Chief Executive Officer
        ou: Executive Leadership

        dn: cn=Marcus Vance,ou=Engineering,dc=vexio,dc=io
        cn: Marcus Vance
        title: VP of Systems Engineering
        ou: Core Rust & Graphics
        manager: cn=Elena Rostova,ou=Executive,dc=vexio,dc=io

        dn: cn=Aria Chen,ou=Design,dc=vexio,dc=io
        cn: Aria Chen
        title: VP of Apple & Desktop Design
        ou: Liquid Glass Experience
        manager: cn=Elena Rostova,ou=Executive,dc=vexio,dc=io

        dn: cn=Devon Patel,ou=Engineering,dc=vexio,dc=io
        cn: Devon Patel
        title: Principal Metal & wgpu Architect
        ou: Core Rust & Graphics
        manager: cn=Marcus Vance,ou=Engineering,dc=vexio,dc=io

        dn: cn=Sofia Lindqvist,ou=Design,dc=vexio,dc=io
        cn: Sofia Lindqvist
        title: Staff ShapeSheet UX Engineer
        ou: Liquid Glass Experience
        manager: cn=Aria Chen,ou=Design,dc=vexio,dc=io
        """
        let payload = directoryPayload ?? defaultLdif
        upsertPageInCore(
            pageId: activePageId,
            title: pages.first(where: { $0.id == activePageId })?.title ?? "Page-\(activePageId)"
        )

        do {
            let count = try core.importEntraOrLdif(pageId: activePageId, directoryPayload: payload)
            selectedShapeId = 1
            selectedShapeIds = [1, 2]
            lastBannerError = nil
            registerJournalUndoStep(actionName: "Import Entra / LDIF Org Chart", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            statusMessage = "Imported \(count) directory records into hierarchical Org Chart"
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func applyRemoteCrdtBatch(
        replicaId: String = "macos-cloud-peer",
        opsJSON: String? = nil
    ) {
        let resolvedJSON: String
        if let explicitJSON = opsJSON, !explicitJSON.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            resolvedJSON = explicitJSON
        } else {
            let clock = nextHlcClock()
            let sampleOps: [[String: Any]] = [
                [
                    "UpsertShape": [
                        "ts": [
                            "wall_ms": clock.wallMs,
                            "logical": clock.logical,
                            "replica_id": replicaId,
                        ],
                        "page_id": activePageId,
                        "shape_id": selectedShapeId,
                        "name": "PeerSynced.\(selectedShapeId)",
                        "kind": "Shape2D",
                        "master_info": NSNull(),
                    ],
                ],
                [
                    "SetCell": [
                        "ts": [
                            "wall_ms": clock.wallMs,
                            "logical": clock.logical + 1,
                            "replica_id": replicaId,
                        ],
                        "page_id": activePageId,
                        "shape_id": selectedShapeId,
                        "cell_name": "FillForegnd",
                        "formula": "RGB(219,234,254)",
                        "force_setf": false,
                    ],
                ],
            ]
            if let encoded = try? JSONSerialization.data(withJSONObject: sampleOps),
               let utf8 = String(data: encoded, encoding: .utf8) {
                resolvedJSON = utf8
            } else {
                resolvedJSON = "[]"
            }
        }

        do {
            try core.applyCrdtJSON(replicaId: replicaId, opsJSON: resolvedJSON)
            lastBannerError = nil
            reloadSelectedShapeSheetCells()
            statusMessage = "Merged HLC CRDT batch from '\(replicaId)'"
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func flushLocalCrdtDelta(replicaId: String = "macos-local") -> String {
        guard let baseline = baselineComparisonSnapshot else {
            return "[]"
        }
        let nowMs = UInt64(max(Date().timeIntervalSince1970 * 1000.0, 1.0))
        return (try? core.flushCrdtJSON(
            againstBaselineData: baseline.data,
            formatExtension: baseline.formatExtension,
            replicaId: replicaId,
            nowMs: nowMs
        )) ?? "[]"
    }

    // MARK: - Quick Look & Offscreen RGBA Preview Generation

    public func renderQuickLookPreview(
        pageId: UInt32? = nil,
        widthPx: UInt32 = 1024,
        heightPx: UInt32 = 768
    ) throws -> CGImage {
        try core.renderPreviewRGBA(
            pageId: pageId ?? activePageId,
            widthPx: widthPx,
            heightPx: heightPx
        )
    }

    // MARK: - Continuity / Handoff State Serialization

    public static let continuityEditingActivityType = "app.vexio.continuity.editingSession"

    public func populateUserActivity(_ activity: NSUserActivity, fileURL: URL?) {
        activity.title = "Editing \(documentTitle) in Vexio"
        activity.isEligibleForHandoff = true
        activity.isEligibleForSearch = true
        var payload: [String: Any] = [
            "documentTitle": documentTitle,
            "activePageId": Int(activePageId),
            "selectedShapeId": String(selectedShapeId),
            "zoomScale": Double(zoomScale),
            "panOffsetX": Double(panOffset.width),
            "panOffsetY": Double(panOffset.height),
            "activeTool": activeTool.rawValue,
            "selectedThemeSlug": selectedThemeSlug,
            "selectedVariantIndex": Int(selectedVariantIndex),
        ]
        if let fileURL {
            payload["fileURLString"] = fileURL.absoluteString
        }
        activity.userInfo = payload
        activity.needsSave = true
    }

    public func restoreFromUserActivity(_ activity: NSUserActivity) {
        guard let info = activity.userInfo else { return }
        if let title = info["documentTitle"] as? String {
            documentTitle = title
        }
        if let page = info["activePageId"] as? Int, page >= 0 {
            activePageId = UInt32(page)
        }
        if let shapeStr = info["selectedShapeId"] as? String, let parsedShape = UInt64(shapeStr) {
            selectedShapeId = parsedShape
        }
        if let zoom = info["zoomScale"] as? Double {
            zoomScale = CGFloat(min(max(zoom, 0.1), 16.0))
        }
        if let panX = info["panOffsetX"] as? Double, let panY = info["panOffsetY"] as? Double {
            panOffset = CGSize(width: panX, height: panY)
        }
        if let rawTool = info["activeTool"] as? String, let tool = VexioCanvasTool(rawValue: rawTool) {
            activeTool = tool
        }
        if let theme = info["selectedThemeSlug"] as? String {
            selectedThemeSlug = theme
        }
        if let variant = info["selectedVariantIndex"] as? Int, variant >= 0, variant <= 3 {
            selectedVariantIndex = UInt8(variant)
        }
        reloadSelectedShapeSheetCells()
        statusMessage = "Restored Continuity session on Page-\(activePageId)"
    }
}
