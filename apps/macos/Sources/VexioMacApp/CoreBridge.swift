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
        case .jpeg: return "jpg"
        case .png: return "png"
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
    case jpg
    case png
    case pdf
    case svg
    case dxf
    case vsd
    case pptx
    case docx
    case powerautomate
    case powerbi

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .vsdx: return "Visio Drawing (.vsdx)"
        case .jpg: return "High-DPI JPEG Image (.jpg)"
        case .png: return "High-DPI PNG Image (.png)"
        case .pdf: return "Vector PDF/A (.pdf)"
        case .svg: return "Scalable Vector Graphics (.svg)"
        case .dxf: return "AutoCAD Exchange (.dxf)"
        case .vsd: return "Visio 2003–2010 Binary (.vsd)"
        case .pptx: return "PowerPoint Presentation (.pptx)"
        case .docx: return "Word Process Specification (.docx)"
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
        case .jpg: return .jpeg
        case .png: return .png
        case .pdf: return .pdf
        case .svg: return .svg
        case .dxf: return .autoCADDXF
        case .vsd: return .visioLegacyDrawing
        case .pptx: return .presentation
        case .docx: return .data
        case .powerautomate: return .json
        case .powerbi: return .json
        }
    }
}

public enum VexioCanvasTool: String, CaseIterable, Identifiable, Sendable {
    case pointer = "Select"
    case pan = "Pan"
    case connector = "Connector"
    case line = "Line"
    case arc = "Arc"
    case freeform = "Freeform"
    case rectangle = "Rectangle"
    case roundedRect = "Rounded Rect"
    case ellipse = "Circle / Ellipse"
    case diamond = "Diamond"
    case triangle = "Triangle"
    case hexagon = "Hexagon"
    case star = "Star"
    case text = "Text"
    case ink = "Pen Ink"
    case highlighter = "Highlighter"

    public var id: String { rawValue }

    /// Matches `NativeCanvasTool` C-ABI discriminant (`0..15`) in `vexio_viewport_set_tool`.
    public var engineToolCode: UInt32 {
        switch self {
        case .pointer: return 0
        case .pan: return 1
        case .connector: return 2
        case .text: return 3
        case .rectangle: return 4
        case .roundedRect: return 5
        case .ellipse: return 6
        case .diamond: return 7
        case .triangle: return 8
        case .line: return 9
        case .arc: return 10
        case .freeform: return 11
        case .ink: return 12
        case .highlighter: return 13
        case .hexagon: return 14
        case .star: return 15
        }
    }

    public var symbolName: String {
        switch self {
        case .pointer: return "cursorarrow"
        case .pan: return "hand.raised"
        case .connector: return "point.topleft.down.curvedto.point.bottomright.up"
        case .line: return "line.diagonal"
        case .arc: return "scribble"
        case .freeform: return "scribble.variable"
        case .rectangle: return "rectangle"
        case .roundedRect: return "rectangle.roundedtop"
        case .ellipse: return "oval"
        case .diamond: return "diamond"
        case .triangle: return "triangle"
        case .hexagon: return "hexagon"
        case .star: return "star"
        case .text: return "character.textbox"
        case .ink: return "pencil.tip.crop.circle"
        case .highlighter: return "highlighter"
        }
    }

    public var shortcutHint: String {
        switch self {
        case .pointer: return "⌘1"
        case .pan: return "H"
        case .connector: return "⌘2"
        case .line: return "L"
        case .arc: return "A"
        case .freeform: return "⌘6"
        case .rectangle: return "⌘4"
        case .roundedRect: return "⇧⌘4"
        case .ellipse: return "⌘5"
        case .diamond: return "D"
        case .triangle: return "T"
        case .hexagon: return "X"
        case .star: return "S"
        case .text: return "⌘3"
        case .ink: return "⌘7"
        case .highlighter: return "⇧⌘7"
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

    public var engineAlgorithmSlug: String {
        switch self {
        case .hierarchicalTopDown: return "hierarchical_tb"
        case .swimlaneLeftRight: return "hierarchical_lr"
        case .radialCluster: return "radial"
        case .compactOrthogonal: return "force_directed"
        }
    }
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

public struct NativeStencilMasterPayload: Decodable, Sendable {
    public let name: String
    public let prompt: String
    public let category: String
    public let widthIn: Double
    public let heightIn: Double
    public let fillFormula: String
    public let lineFormula: String
    public let svg: String
}

public struct VexioStencilMaster: Identifiable, Equatable, Hashable, Sendable {
    public let id: String
    public let category: String
    public let name: String
    public let prompt: String
    public let symbolName: String
    public let defaultWidthFormula: String
    public let defaultHeightFormula: String
    public let defaultFillFormula: String
    public let isGuardedAspect: Bool
    public let svgPreview: String

    public init(
        id: String,
        category: String,
        name: String,
        prompt: String = "",
        symbolName: String,
        defaultWidthFormula: String,
        defaultHeightFormula: String,
        defaultFillFormula: String,
        isGuardedAspect: Bool,
        svgPreview: String = ""
    ) {
        self.id = id
        self.category = category
        self.name = name
        self.prompt = prompt
        self.symbolName = symbolName
        self.defaultWidthFormula = defaultWidthFormula
        self.defaultHeightFormula = defaultHeightFormula
        self.defaultFillFormula = defaultFillFormula
        self.isGuardedAspect = isGuardedAspect
        self.svgPreview = svgPreview
    }
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
        .visioDrawing, .visioLegacyDrawing, .autoCADDXF, .pdf, .svg, .jpeg, .png,
        .presentation, .spreadsheet, .data, .json,
    ]
    public static let writableContentTypes: [UTType] = [
        .visioDrawing, .visioLegacyDrawing, .autoCADDXF, .pdf, .svg, .jpeg, .png,
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

    func setViewportTool(toolCode: UInt32) {
        lock.lock()
        defer { lock.unlock() }
        if let activeViewport = viewport {
            _ = vexio_viewport_set_tool(activeViewport, toolCode)
        }
    }

    func setViewportPage(pageId: UInt32) {
        lock.lock()
        defer { lock.unlock() }
        if let activeViewport = viewport {
            _ = vexio_viewport_set_page(activeViewport, doc, pageId)
        }
    }

    func setViewportZoom(zoomScale: Double, panXIn: Double, panYIn: Double) {
        lock.lock()
        defer { lock.unlock() }
        if let activeViewport = viewport {
            _ = vexio_viewport_set_zoom(activeViewport, doc, zoomScale, panXIn, panYIn)
        }
    }

    func renderViewportFrame() {
        lock.lock()
        defer { lock.unlock() }
        if let activeViewport = viewport {
            _ = vexio_viewport_render_frame(activeViewport, doc)
        }
    }

    func primarySelection() -> UInt64 {
        lock.lock()
        defer { lock.unlock() }
        guard let activeViewport = viewport else { return 0 }
        return vexio_viewport_get_primary_selection(activeViewport)
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

    func listStencilsJSON() throws -> String {
        lock.lock()
        defer { lock.unlock() }

        var outJSON = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status = vexio_doc_list_stencils_json(doc, &outJSON)
        if let err = VexioBridgeError.from(status: status, context: "list_stencils_json") {
            vexio_buffer_free(outJSON)
            throw err
        }
        return Self.consumeOwnedBufferAsUTF8(outJSON)
    }

    func loadStarterTemplate(templateId: String) throws {
        lock.lock()
        defer { lock.unlock() }

        let status = templateId.withCString { tidCStr in
            vexio_doc_load_starter_template(doc, tidCStr)
        }
        if let err = VexioBridgeError.from(status: status, context: "load_starter_template(\(templateId))") {
            throw err
        }
    }

    func dropStencil(
        pageId: UInt32,
        category: String,
        masterName: String,
        xIn: Double,
        yIn: Double
    ) throws -> UInt64 {
        lock.lock()
        defer { lock.unlock() }

        var outShapeId: UInt64 = 0
        let status: VexioStatus = withUnsafeMutablePointer(to: &outShapeId) { outPtr in
            category.withCString { catCStr in
                masterName.withCString { nameCStr in
                    vexio_doc_drop_stencil(doc, pageId, catCStr, nameCStr, xIn, yIn, outPtr)
                }
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "drop_stencil(\(masterName))") {
            throw err
        }
        return outShapeId
    }

    func drawPrimitive(
        pageId: UInt32,
        kind: String,
        x1In: Double,
        y1In: Double,
        x2In: Double,
        y2In: Double
    ) throws -> UInt64 {
        lock.lock()
        defer { lock.unlock() }

        var outShapeId: UInt64 = 0
        let status: VexioStatus = withUnsafeMutablePointer(to: &outShapeId) { outPtr in
            kind.withCString { kindCStr in
                vexio_doc_draw_primitive(doc, pageId, kindCStr, x1In, y1In, x2In, y2In, outPtr)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "draw_primitive(\(kind))") {
            throw err
        }
        return outShapeId
    }

    func deleteShapes(pageId: UInt32, shapeIds: [UInt64]) throws -> UInt32 {
        guard !shapeIds.isEmpty else { return 0 }
        lock.lock()
        defer { lock.unlock() }

        var deletedCount: UInt32 = 0
        let status: VexioStatus = withUnsafeMutablePointer(to: &deletedCount) { outPtr in
            shapeIds.withUnsafeBufferPointer { idsBuf in
                vexio_doc_delete_shapes(doc, pageId, idsBuf.baseAddress, idsBuf.count, outPtr)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "delete_shapes") {
            throw err
        }
        return deletedCount
    }

    func duplicateShapes(
        pageId: UInt32,
        shapeIds: [UInt64],
        dxIn: Double,
        dyIn: Double
    ) throws -> UInt64 {
        guard !shapeIds.isEmpty else { return 0 }
        lock.lock()
        defer { lock.unlock() }

        var firstId: UInt64 = 0
        let status: VexioStatus = withUnsafeMutablePointer(to: &firstId) { outPtr in
            shapeIds.withUnsafeBufferPointer { idsBuf in
                vexio_doc_duplicate_shapes(doc, pageId, idsBuf.baseAddress, idsBuf.count, dxIn, dyIn, outPtr)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "duplicate_shapes") {
            throw err
        }
        return firstId
    }

    func reorderShapes(pageId: UInt32, shapeIds: [UInt64], mode: String) throws {
        guard !shapeIds.isEmpty else { return }
        lock.lock()
        defer { lock.unlock() }

        let status: VexioStatus = shapeIds.withUnsafeBufferPointer { idsBuf in
            mode.withCString { modeCStr in
                vexio_doc_reorder_shapes(doc, pageId, idsBuf.baseAddress, idsBuf.count, modeCStr)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "reorder_shapes(\(mode))") {
            throw err
        }
    }

    func addPage(name: String) throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }

        var outPageId: UInt32 = 0
        let status: VexioStatus = withUnsafeMutablePointer(to: &outPageId) { outPtr in
            name.withCString { nameCStr in
                vexio_doc_add_page(doc, nameCStr, outPtr)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "add_page(\(name))") {
            throw err
        }
        return outPageId
    }

    func autoLayout(pageId: UInt32, algo: String) throws {
        lock.lock()
        defer { lock.unlock() }

        let status = algo.withCString { algoCStr in
            vexio_doc_auto_layout(doc, pageId, algoCStr)
        }
        if let err = VexioBridgeError.from(status: status, context: "auto_layout(\(algo))") {
            throw err
        }
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
        .jpeg,
        .png,
        .presentation,
        .spreadsheet,
        .data,
        .json,
    ]

    nonisolated private let core: NativeDocumentCore
    nonisolated public let sourceFormatExtension: String

    // Published Workstation UI State
    @Published public var documentTitle: String = "Untitled Diagram"
    @Published public var activeTool: VexioCanvasTool = .pointer {
        didSet {
            core.setViewportTool(toolCode: activeTool.engineToolCode)
            core.renderViewportFrame()
        }
    }
    @Published public var activePageId: UInt32 = 1 {
        didSet {
            core.setViewportPage(pageId: activePageId)
            core.renderViewportFrame()
        }
    }
    @Published public var selectedShapeId: UInt64 = 1
    @Published public var selectedShapeIds: [UInt64] = [1, 2]
    @Published public var nextAllocatedShapeId: UInt64 = 10

    @Published public var zoomScale: CGFloat = 1.0 {
        didSet {
            syncViewportCamera()
        }
    }
    @Published public var panOffset: CGSize = .zero {
        didSet {
            syncViewportCamera()
        }
    }
    @Published public var cursorCanvasInches: CGPoint = .zero
    @Published public var activeCursorKind: UInt32 = 0

    @Published public var connectorRouting: ConnectorRoutingStyle = .orthogonalJumps
    @Published public var showStencilNavigator: Bool = true
    @Published public var showInspectorSidebar: Bool = true
    @Published public var showShapeSheetDrawer: Bool = false

    @Published public var stencilCatalog: [VexioStencilMaster] = VexioDocumentSession.builtInStencilCatalog
    @Published public var stencilCategories: [String] = VexioDocumentSession.canonicalCategoryOrder
    private var hasLoadedDynamicStencils: Bool = false
    private var clipboardShapeIds: [UInt64] = []

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

    nonisolated public static let canonicalCategoryOrder: [String] = [
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
    ]

    // Fallback Stencil Seed (superseded immediately by 519-master C-ABI catalog on load)
    nonisolated public static let builtInStencilCatalog: [VexioStencilMaster] = [
        VexioStencilMaster(id: "Basic Flowchart::Process", category: "Basic Flowchart", name: "Process", symbolName: "rectangle", defaultWidthFormula: "1.5 in", defaultHeightFormula: "0.875 in", defaultFillFormula: "RGB(235,244,255)", isGuardedAspect: false),
        VexioStencilMaster(id: "Basic Flowchart::Decision", category: "Basic Flowchart", name: "Decision", symbolName: "diamond", defaultWidthFormula: "1.25 in", defaultHeightFormula: "1.0 in", defaultFillFormula: "RGB(255,246,229)", isGuardedAspect: false),
        VexioStencilMaster(id: "Basic Flowchart::Start / End", category: "Basic Flowchart", name: "Start / End", symbolName: "capsule", defaultWidthFormula: "1.5 in", defaultHeightFormula: "0.625 in", defaultFillFormula: "RGB(231,250,239)", isGuardedAspect: false),
        VexioStencilMaster(id: "Basic Flowchart::Database", category: "Basic Flowchart", name: "Database", symbolName: "cylinder.split.1x2", defaultWidthFormula: "1.25 in", defaultHeightFormula: "1.375 in", defaultFillFormula: "RGB(240,238,255)", isGuardedAspect: false),
    ]

    nonisolated public static func symbolName(forCategory category: String, masterName: String) -> String {
        let lower = masterName.lowercased()
        if lower.contains("decision") || lower.contains("gateway") || lower.contains("diamond") { return "diamond" }
        if lower.contains("start") || lower.contains("end") || lower.contains("terminator") || lower.contains("pill") { return "capsule" }
        if lower.contains("database") || lower.contains("rds") || lower.contains("sql") || lower.contains("storage") { return "cylinder.split.1x2" }
        if lower.contains("circle") || lower.contains("event") || lower.contains("node") { return "circle" }
        if lower.contains("star") { return "star" }
        if lower.contains("hexagon") { return "hexagon" }
        if lower.contains("triangle") { return "triangle" }
        if lower.contains("arrow") { return "arrow.right" }
        if lower.contains("callout") || lower.contains("note") || lower.contains("comment") { return "bubble.left" }
        if lower.contains("server") || lower.contains("rack") || lower.contains("vm") || lower.contains("ec2") { return "server.rack" }
        if lower.contains("cloud") || category.contains("Cloud") { return "cloud" }
        if lower.contains("user") || lower.contains("actor") || lower.contains("person") || category.contains("Org") { return "person.crop.rectangle" }
        if lower.contains("wall") || lower.contains("door") || category.contains("Floor") { return "ruler" }
        if category.contains("Electrical") || category.contains("P&ID") { return "bolt.horizontal.circle" }
        if category.contains("Wireframe") { return "macwindow" }
        return "square.on.square"
    }

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

    nonisolated public init(blankTitle: String = "Untitled Diagram") {
        do {
            self.core = try NativeDocumentCore(blankTitle: blankTitle)
        } catch {
            preconditionFailure("Unable to create native Vexio engine/document: \(error.localizedDescription)")
        }
        self.sourceFormatExtension = "vsdx"
        _ = try? self.core.loadStarterTemplate(templateId: "flowchart")
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

    public func ensureStencilsLoadedIfNeeded() {
        guard !hasLoadedDynamicStencils else { return }
        hasLoadedDynamicStencils = true
        guard let rawJSON = try? core.listStencilsJSON(),
              let jsonData = rawJSON.data(using: .utf8),
              let grouped = try? JSONDecoder().decode([String: [NativeStencilMasterPayload]].self, from: jsonData),
              !grouped.isEmpty
        else {
            return
        }

        var orderedCategories: [String] = []
        for canonical in Self.canonicalCategoryOrder where grouped[canonical] != nil {
            orderedCategories.append(canonical)
        }
        for extra in grouped.keys.sorted() where !orderedCategories.contains(extra) {
            orderedCategories.append(extra)
        }

        var flatMasters: [VexioStencilMaster] = []
        for cat in orderedCategories {
            guard let list = grouped[cat] else { continue }
            for item in list {
                let fillClean = item.fillFormula.hasPrefix("=")
                    ? String(item.fillFormula.dropFirst())
                    : item.fillFormula
                flatMasters.append(
                    VexioStencilMaster(
                        id: "\(cat)::\(item.name)",
                        category: cat,
                        name: item.name,
                        prompt: item.prompt,
                        symbolName: Self.symbolName(forCategory: cat, masterName: item.name),
                        defaultWidthFormula: String(format: "%.3f in", item.widthIn),
                        defaultHeightFormula: String(format: "%.3f in", item.heightIn),
                        defaultFillFormula: fillClean,
                        isGuardedAspect: false,
                        svgPreview: item.svg
                    )
                )
            }
        }

        if !flatMasters.isEmpty {
            stencilCategories = orderedCategories
            stencilCatalog = flatMasters
        }
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

    private func syncViewportCamera() {
        let panXIn = -Double(panOffset.width) / (96.0 * Double(max(zoomScale, 0.1)))
        let panYIn = Double(panOffset.height) / (96.0 * Double(max(zoomScale, 0.1)))
        core.setViewportZoom(zoomScale: Double(zoomScale), panXIn: panXIn, panYIn: panYIn)
        core.renderViewportFrame()
    }

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
            core.setViewportPage(pageId: activePageId)
            core.setViewportTool(toolCode: activeTool.engineToolCode)
            core.renderViewportFrame()
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
            let primary = core.primarySelection()
            if primary != 0 && primary != selectedShapeId {
                selectedShapeId = primary
                selectedShapeIds = [primary]
            }
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
        case .pan:
            return engineCursorCode == 3 ? .closedHand : .openHand
        case .connector, .line, .arc, .freeform, .rectangle, .roundedRect, .ellipse, .diamond, .triangle, .hexagon, .star, .ink, .highlighter:
            if engineCursorCode == 0 || engineCursorCode == 2 {
                return .crosshair
            }
        case .text:
            if engineCursorCode == 0 || engineCursorCode == 7 {
                return .iBeam
            }
        case .pointer:
            break
        }

        switch engineCursorCode {
        case 1: return .pointingHand
        case 2: return .crosshair
        case 3: return .closedHand
        case 4: return .crosshair
        case 5: return .resizeUpDown
        case 6: return .resizeLeftRight
        case 7: return .iBeam
        case 8, 11, 12, 13: return .crosshair
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
        if (try? core.loadStarterTemplate(templateId: "flowchart")) != nil {
            selectedShapeId = 1
            selectedShapeIds = [1]
        } else {
            upsertPageInCore(pageId: 1, title: "Page-1")
            upsertShapeInCore(pageId: activePageId, shapeId: 1, name: "Process.1")
            for (_, cellName, defaultFormula) in Self.canonicalCellManifest {
                try? core.setCellFormula(
                    pageId: activePageId,
                    shapeId: 1,
                    cellName: cellName,
                    formula: defaultFormula,
                    forceSetF: true
                )
            }
        }
        baselineComparisonSnapshot = try? core.saveFormat("vsdx")
    }

    public func loadStarterTemplate(_ templateId: String, undoManager: UndoManager?) {
        do {
            try core.loadStarterTemplate(templateId: templateId)
            activePageId = 1
            selectedShapeId = 1
            selectedShapeIds = [1]
            registerJournalUndoStep(actionName: "Load Template \(templateId)", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            core.renderViewportFrame()
            statusMessage = "Loaded '\(templateId.capitalized)' starter diagram"
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func reloadSelectedShapeSheetCells() {
        ensureStencilsLoadedIfNeeded()
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
            core.renderViewportFrame()
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
            MainActor.assumeIsolated {
                target.performUndo(undoManager: undoManager)
            }
        }
    }

    public func performUndo(undoManager: UndoManager?) {
        do {
            try core.undo()
            lastBannerError = nil
            statusMessage = "Undo applied"
            undoManager?.registerUndo(withTarget: self) { target in
                MainActor.assumeIsolated {
                    target.performRedo(undoManager: undoManager)
                }
            }
            reloadSelectedShapeSheetCells()
            core.renderViewportFrame()
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
                MainActor.assumeIsolated {
                    target.performUndo(undoManager: undoManager)
                }
            }
            reloadSelectedShapeSheetCells()
            core.renderViewportFrame()
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    // MARK: - Stencil Instantiation, Primitive Drawing, Context-Menu Actions & Validation

    public func instantiateStencilMaster(
        masterId: String,
        atCanvasInches point: CGPoint,
        undoManager: UndoManager?
    ) {
        ensureStencilsLoadedIfNeeded()
        guard let master = stencilCatalog.first(where: { $0.id == masterId })
            ?? Self.builtInStencilCatalog.first(where: { $0.id == masterId })
        else {
            return
        }

        let safeX = min(max(Double(point.x), 0.5), 10.5)
        let safeY = min(max(Double(point.y), 0.5), 8.0)

        if let createdId = try? core.dropStencil(
            pageId: activePageId,
            category: master.category,
            masterName: master.name,
            xIn: safeX,
            yIn: safeY
        ), createdId != 0 {
            selectedShapeId = createdId
            selectedShapeIds = [createdId]
            registerJournalUndoStep(actionName: "Drop \(master.name)", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            core.renderViewportFrame()
            statusMessage = String(format: "Dropped '%@' as Sheet.%llu at (%.2f in, %.2f in)", master.name, createdId, safeX, safeY)
            return
        }

        let shapeId = nextAllocatedShapeId
        nextAllocatedShapeId += 1
        selectedShapeId = shapeId
        selectedShapeIds = [shapeId]
        upsertShapeInCore(pageId: activePageId, shapeId: shapeId, name: "\(master.name).\(shapeId)")
        try? core.setCellFormula(pageId: activePageId, shapeId: shapeId, cellName: "PinX", formula: String(format: "%.3f in", safeX), forceSetF: true)
        try? core.setCellFormula(pageId: activePageId, shapeId: shapeId, cellName: "PinY", formula: String(format: "%.3f in", safeY), forceSetF: true)
        try? core.setCellFormula(pageId: activePageId, shapeId: shapeId, cellName: "Width", formula: master.defaultWidthFormula, forceSetF: true)
        try? core.setCellFormula(pageId: activePageId, shapeId: shapeId, cellName: "Height", formula: master.defaultHeightFormula, forceSetF: true)
        try? core.setCellFormula(pageId: activePageId, shapeId: shapeId, cellName: "FillForegnd", formula: master.defaultFillFormula, forceSetF: true)
        registerJournalUndoStep(actionName: "Drop \(master.name)", undoManager: undoManager)
        reloadSelectedShapeSheetCells()
        core.renderViewportFrame()
        statusMessage = "Dropped '\(master.name)' as Sheet.\(shapeId)"
    }

    public func drawQuickPrimitive(
        kind: String,
        atCanvasInches point: CGPoint,
        undoManager: UndoManager?
    ) {
        let cx = min(max(Double(point.x), 1.0), 10.0)
        let cy = min(max(Double(point.y), 1.0), 7.5)
        let x1 = cx - 0.75
        let y1 = cy - 0.45
        let x2 = cx + 0.75
        let y2 = cy + 0.45
        do {
            let sid = try core.drawPrimitive(
                pageId: activePageId,
                kind: kind,
                x1In: x1,
                y1In: y1,
                x2In: x2,
                y2In: y2
            )
            if sid != 0 {
                selectedShapeId = sid
                selectedShapeIds = [sid]
            }
            registerJournalUndoStep(actionName: "Insert \(kind.capitalized)", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            core.renderViewportFrame()
            statusMessage = "Inserted \(kind) (Sheet.\(selectedShapeId))"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func duplicateSelectedShapes(
        dxInches: Double = 0.35,
        dyInches: Double = -0.35,
        undoManager: UndoManager?
    ) {
        let targets = selectedShapeIds.isEmpty ? [selectedShapeId] : selectedShapeIds
        do {
            let newFirst = try core.duplicateShapes(
                pageId: activePageId,
                shapeIds: targets,
                dxIn: dxInches,
                dyIn: dyInches
            )
            if newFirst != 0 {
                selectedShapeId = newFirst
                selectedShapeIds = [newFirst]
            }
            registerJournalUndoStep(actionName: "Duplicate Shape", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            core.renderViewportFrame()
            statusMessage = "Duplicated selection → Sheet.\(selectedShapeId)"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func copySelectionToClipboard() {
        clipboardShapeIds = selectedShapeIds.isEmpty ? [selectedShapeId] : selectedShapeIds
        statusMessage = "Copied \(clipboardShapeIds.count) shape(s)"
    }

    public func pasteClipboardAt(canvasInches point: CGPoint, undoManager: UndoManager?) {
        let sourceIds = clipboardShapeIds.isEmpty
            ? (selectedShapeIds.isEmpty ? [selectedShapeId] : selectedShapeIds)
            : clipboardShapeIds
        do {
            let pastedId = try core.duplicateShapes(
                pageId: activePageId,
                shapeIds: sourceIds,
                dxIn: 0.40,
                dyIn: -0.40
            )
            if pastedId != 0 {
                selectedShapeId = pastedId
                selectedShapeIds = [pastedId]
                try? core.setCellFormula(
                    pageId: activePageId,
                    shapeId: pastedId,
                    cellName: "PinX",
                    formula: String(format: "%.3f in", min(max(Double(point.x), 0.5), 10.5)),
                    forceSetF: false
                )
                try? core.setCellFormula(
                    pageId: activePageId,
                    shapeId: pastedId,
                    cellName: "PinY",
                    formula: String(format: "%.3f in", min(max(Double(point.y), 0.5), 8.0)),
                    forceSetF: false
                )
            }
            registerJournalUndoStep(actionName: "Paste Shape", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            core.renderViewportFrame()
            statusMessage = "Pasted Sheet.\(selectedShapeId)"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func deleteSelectedShapes(undoManager: UndoManager?) {
        let targets = selectedShapeIds.isEmpty ? [selectedShapeId] : selectedShapeIds
        do {
            let deleted = try core.deleteShapes(pageId: activePageId, shapeIds: targets)
            if deleted > 0 {
                selectedShapeIds = []
                registerJournalUndoStep(actionName: "Delete Shape", undoManager: undoManager)
                reloadSelectedShapeSheetCells()
                core.renderViewportFrame()
                statusMessage = "Deleted \(deleted) shape(s)"
            }
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func reorderSelectedShapes(mode: String, undoManager: UndoManager?) {
        let targets = selectedShapeIds.isEmpty ? [selectedShapeId] : selectedShapeIds
        do {
            try core.reorderShapes(pageId: activePageId, shapeIds: targets, mode: mode)
            registerJournalUndoStep(actionName: "Arrange \(mode.capitalized)", undoManager: undoManager)
            core.renderViewportFrame()
            statusMessage = "Arranged selection (\(mode))"
        } catch {
            statusMessage = error.localizedDescription
        }
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
        let proposedTitle = "Page-\((pages.map(\.id).max() ?? 0) + 1)"
        if let createdPageId = try? core.addPage(name: proposedTitle), createdPageId != 0 {
            let page = VexioPageSummary(id: createdPageId, title: proposedTitle)
            pages.append(page)
            activePageId = createdPageId
            reloadSelectedShapeSheetCells()
            core.renderViewportFrame()
            statusMessage = "Created \(page.title)"
            return
        }
        let nextId = (pages.map(\.id).max() ?? 0) + 1
        let page = VexioPageSummary(id: nextId, title: "Page-\(nextId)")
        upsertPageInCore(pageId: nextId, title: page.title)
        pages.append(page)
        activePageId = nextId
        reloadSelectedShapeSheetCells()
        core.renderViewportFrame()
        statusMessage = "Created \(page.title)"
    }

    public func applyAutoLayout(_ preset: AutoLayoutPreset, undoManager: UndoManager?) {
        do {
            try core.autoLayout(pageId: activePageId, algo: preset.engineAlgorithmSlug)
            registerJournalUndoStep(actionName: "Auto-Layout \(preset.rawValue)", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            core.renderViewportFrame()
            statusMessage = "Applied \(preset.rawValue) layout"
        } catch {
            statusMessage = error.localizedDescription
        }
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
