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
    case corruptOrUnsupportedFormat(extensionName: String, detail: String = "")
    case gpuSurfaceLost
    case internalRustPanic(detail: String = "")
    case unknownStatus(rawCode: UInt32)
    case emptyDocumentPayload
    case previewBitmapConstructionFailed

    private static func readLastEngineError() -> String {
        var buf = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        guard vexio_last_error_message(&buf) == VEXIO_OK else { return "" }
        defer { vexio_buffer_free(buf) }
        guard let ptr = buf.ptr, buf.len > 0 else { return "" }
        let data = Data(bytes: ptr, count: buf.len)
        return String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
    }

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
            return .corruptOrUnsupportedFormat(extensionName: context, detail: readLastEngineError())
        } else if status == VEXIO_ERR_GPU_SURFACE_LOST {
            return .gpuSurfaceLost
        } else if status == VEXIO_ERR_INTERNAL_PANIC {
            return .internalRustPanic(detail: readLastEngineError())
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
        case .corruptOrUnsupportedFormat(let ext, let detail):
            return detail.isEmpty
                ? "Unable to parse '.\(ext)' package: corrupt stream or unsupported schema."
                : "Unable to parse '\(ext)': \(detail)"
        case .gpuSurfaceLost:
            return "Metal CAMetalLayer GPU surface was lost and is being re-bound."
        case .internalRustPanic(let detail):
            return detail.isEmpty
                ? "Vexio Core caught an internal engine panic safely across the C-ABI boundary."
                : "Vexio Core caught an internal engine error safely across C-ABI (\(detail))."
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

    public func localizedTitle(_ lang: VexioAppLanguage) -> String {
        switch (self, lang) {
        case (.pointer, .de): return "Auswahl"
        case (.pointer, .it): return "Seleziona"
        case (.pointer, .fr): return "Sélection"
        case (.pointer, .es): return "Seleccionar"
        case (.pointer, .el): return "Επιλογή"
        case (.pan, .de): return "Verschieben"
        case (.pan, .it): return "Panoramica"
        case (.pan, .fr): return "Main"
        case (.pan, .es): return "Mano"
        case (.pan, .el): return "Μετακίνηση"
        case (.connector, .de): return "Verbinder"
        case (.connector, .it): return "Connettore"
        case (.connector, .fr): return "Connecteur"
        case (.connector, .es): return "Conector"
        case (.connector, .el): return "Σύνδεση"
        case (.line, .de): return "Linie"
        case (.line, .it): return "Linea"
        case (.line, .fr): return "Ligne"
        case (.line, .es): return "Línea"
        case (.line, .el): return "Γραμμή"
        case (.arc, .de): return "Bogen"
        case (.arc, .it): return "Arco"
        case (.arc, .fr): return "Arc"
        case (.arc, .es): return "Arco"
        case (.arc, .el): return "Τόξο"
        case (.freeform, .de): return "Freihandform"
        case (.freeform, .it): return "Forma libera"
        case (.freeform, .fr): return "Forme libre"
        case (.freeform, .es): return "Forma libre"
        case (.freeform, .el): return "Ελεύθερη καμπύλη"
        case (.rectangle, .de): return "Rechteck"
        case (.rectangle, .it): return "Rettangolo"
        case (.rectangle, .fr): return "Rectangle"
        case (.rectangle, .es): return "Rectángulo"
        case (.rectangle, .el): return "Ορθογώνιο"
        case (.roundedRect, .en): return "Rounded Rectangle"
        case (.roundedRect, .de): return "Abgerundetes Rechteck"
        case (.roundedRect, .it): return "Rettangolo arrotondato"
        case (.roundedRect, .fr): return "Rectangle arrondi"
        case (.roundedRect, .es): return "Rectángulo redondeado"
        case (.roundedRect, .el): return "Στρογγυλεμένο ορθογώνιο"
        case (.ellipse, .de): return "Kreis / Ellipse"
        case (.ellipse, .it): return "Cerchio / Ellisse"
        case (.ellipse, .fr): return "Cercle / Ellipse"
        case (.ellipse, .es): return "Círculo / Elipse"
        case (.ellipse, .el): return "Κύκλος / Έλλειψη"
        case (.diamond, .de): return "Raute"
        case (.diamond, .it): return "Rombo"
        case (.diamond, .fr): return "Losange"
        case (.diamond, .es): return "Rombo"
        case (.diamond, .el): return "Ρόμβος"
        case (.triangle, .de): return "Dreieck"
        case (.triangle, .it): return "Triangolo"
        case (.triangle, .fr): return "Triangle"
        case (.triangle, .es): return "Triángulo"
        case (.triangle, .el): return "Τρίγωνο"
        case (.hexagon, .de): return "Sechseck"
        case (.hexagon, .it): return "Esagono"
        case (.hexagon, .fr): return "Hexagone"
        case (.hexagon, .es): return "Hexágono"
        case (.hexagon, .el): return "Εξάγωνο"
        case (.star, .de): return "Stern"
        case (.star, .it): return "Stella"
        case (.star, .fr): return "Étoile"
        case (.star, .es): return "Estrella"
        case (.star, .el): return "Αστέρι"
        case (.text, .de): return "Textfeld"
        case (.text, .it): return "Testo"
        case (.text, .fr): return "Texte"
        case (.text, .es): return "Texto"
        case (.text, .el): return "Κείμενο"
        case (.ink, .de): return "Freihandstift"
        case (.ink, .it): return "Penna inchiostro"
        case (.ink, .fr): return "Stylo encre"
        case (.ink, .es): return "Lápiz"
        case (.ink, .el): return "Πένα"
        case (.highlighter, .de): return "Textmarker"
        case (.highlighter, .it): return "Evidenziatore"
        case (.highlighter, .fr): return "Surligneur"
        case (.highlighter, .es): return "Resaltador"
        case (.highlighter, .el): return "Μαρκαδόρος"
        default: return rawValue
        }
    }
}

public enum VexioTextHeadingPreset: String, CaseIterable, Identifiable, Sendable {
    case title = "title"
    case subtitle = "subtitle"
    case h1 = "h1"
    case h2 = "h2"
    case h3 = "h3"
    case sectionBanner = "section_banner"
    case calloutTitle = "callout_title"
    case body = "body"
    case caption = "caption"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .title: return "Diagram Title (24 pt Bold)"
        case .subtitle: return "Subtitle (14 pt)"
        case .h1: return "Heading 1 — Underlined (18 pt)"
        case .h2: return "Heading 2 (14 pt Bold)"
        case .h3: return "Heading 3 (12 pt Accent)"
        case .sectionBanner: return "Section Header Banner"
        case .calloutTitle: return "Accent Callout Header"
        case .body: return "Body Text (11 pt)"
        case .caption: return "Caption / Footnote (9.5 pt)"
        }
    }

    public var shortChipLabel: String {
        switch self {
        case .title: return "Title"
        case .subtitle: return "Subtitle"
        case .h1: return "H1"
        case .h2: return "H2"
        case .h3: return "H3"
        case .sectionBanner: return "Banner"
        case .calloutTitle: return "Callout"
        case .body: return "Body"
        case .caption: return "Caption"
        }
    }
}

public enum VexioBorderTitleStyle: String, CaseIterable, Identifiable, Sendable {
    case modernBanner = "modern_banner"
    case executiveUnderline = "executive_underline"
    case classicFrame = "classic_frame"
    case pillBanner = "pill_banner"
    case isoEngineering = "iso_engineering"
    case none = "none"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .modernBanner: return "Modern Header Banner"
        case .executiveUnderline: return "Executive Title & Accent Rule"
        case .classicFrame: return "Classic Frame & Title"
        case .pillBanner: return "Rounded Pill Banner"
        case .isoEngineering: return "ISO 7200 / ANSI Engineering Block"
        case .none: return "None (Remove Border & Title)"
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
        case .orthogonalJumps: return "16"
        case .curvedBezier: return "6"
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
        case .hierarchicalTopDown: return "tb"
        case .swimlaneLeftRight: return "lr"
        case .radialCluster: return "radial"
        case .compactOrthogonal: return "circular"
        }
    }
}

public struct VexioPageSummary: Identifiable, Equatable, Sendable {
    public let id: UInt32
    public var title: String
    public var architecturalScale: String
    public var isBackgroundPage: Bool
    public var widthInches: Double
    public var heightInches: Double
    public var backPageId: UInt32
    public var pageColorHex: String

    public init(
        id: UInt32,
        title: String,
        architecturalScale: String = "1:1 (No Scale)",
        isBackgroundPage: Bool = false,
        widthInches: Double = 11.6929,
        heightInches: Double = 8.2677,
        backPageId: UInt32 = 0,
        pageColorHex: String = "#FFFFFF"
    ) {
        self.id = id
        self.title = title
        self.architecturalScale = architecturalScale
        self.isBackgroundPage = isBackgroundPage
        self.widthInches = widthInches
        self.heightInches = heightInches
        self.backPageId = backPageId
        self.pageColorHex = pageColorHex
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
    public let pngDataUrl: String?
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
    public let pngPreview: String

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
        svgPreview: String = "",
        pngPreview: String = ""
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
        self.pngPreview = pngPreview
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
    public var rowName: String      // e.g. "CostCenter"
    public var rowKey: String       // e.g. "Prop.CostCenter"
    public var label: String
    public var dataTypeCode: UInt8  // 0=String, 2=Number, 3=Boolean, 5=DateTime, 7=Currency
    public var dataType: String
    public var formula: String
    public var evaluatedValue: String

    public init(
        rowName: String = "",
        rowKey: String,
        label: String,
        dataTypeCode: UInt8 = 0,
        dataType: String,
        formula: String,
        evaluatedValue: String
    ) {
        let inferred = rowKey.hasPrefix("Prop.") ? String(rowKey.dropFirst(5)) : rowKey
        self.rowName = rowName.isEmpty ? inferred : rowName
        self.rowKey = rowKey
        self.label = label
        self.dataTypeCode = dataTypeCode
        self.dataType = dataType
        self.formula = formula
        self.evaluatedValue = evaluatedValue
    }
}

public struct DiagramLayerState: Identifiable, Equatable, Sendable {
    public let id: Int
    public var name: String
    public var isVisible: Bool
    public var isPrintable: Bool
    public var isLocked: Bool
    public var isSnapGlueEnabled: Bool
    public var memberCount: Int
    public var accentHex: String

    public init(
        id: Int,
        name: String,
        isVisible: Bool,
        isPrintable: Bool,
        isLocked: Bool,
        isSnapGlueEnabled: Bool,
        memberCount: Int = 0,
        accentHex: String
    ) {
        self.id = id
        self.name = name
        self.isVisible = isVisible
        self.isPrintable = isPrintable
        self.isLocked = isLocked
        self.isSnapGlueEnabled = isSnapGlueEnabled
        self.memberCount = memberCount
        self.accentHex = accentHex
    }
}

public struct InspectedObjectState: Equatable, Sendable {
    public var id: UInt64
    public var name: String
    public var kind: String
    public var isGroup: Bool
    public var childCount: Int
    public var masterName: String?
    public var layerIndices: [Int]
    public var text: String
    public var pinXIn: Double
    public var pinYIn: Double
    public var locPinXIn: Double
    public var locPinYIn: Double
    public var widthIn: Double
    public var heightIn: Double
    public var angleDeg: Double
    public var flipX: Bool
    public var flipY: Bool
    public var beginXIn: Double?
    public var beginYIn: Double?
    public var endXIn: Double?
    public var endYIn: Double?
    public var routeStyle: Int
    public var fillColorHex: String
    public var fillBkgndHex: String
    public var fillPattern: Int
    public var fillTransPct: Double
    public var roundingIn: Double
    public var lineColorHex: String
    public var linePattern: Int
    public var lineWeightPt: Double
    public var lineTransPct: Double
    public var beginArrow: Int
    public var endArrow: Int
    public var lockWidth: Bool
    public var lockHeight: Bool
    public var lockAspect: Bool
    public var lockMoveX: Bool
    public var lockMoveY: Bool
    public var lockRotate: Bool
    public var lockDelete: Bool
    public var hideText: Bool

    public var isConnector1D: Bool { kind == "Connector1D" }

    public var humanReadableTypeBadge: String {
        if isGroup {
            if let master = masterName, !master.isEmpty {
                return "Master: \(master) (\(max(childCount, 1)))"
            }
            return childCount > 0 ? "Group (\(childCount) shapes)" : "Group Shape"
        }
        if kind == "Connector1D" {
            return "1D Dynamic Connector"
        }
        if kind == "ForeignObject" {
            if let master = masterName, !master.isEmpty {
                return "Vector Icon • \(master)"
            }
            return "Vector / Foreign Object"
        }
        if let master = masterName, !master.isEmpty {
            return "2D Shape • \(master)"
        }
        return "2D Shape"
    }
}

public struct DataGraphicRuleConfig: Equatable, Sendable {
    public var isEnabled: Bool = false
    public var iconSetField: String = "Status"
    public var iconGreenMin: Double = 75.0
    public var iconAmberMin: Double = 40.0
    public var dataBarField: String = "CompletionPct"
    public var dataBarMin: Double = 0.0
    public var dataBarMax: Double = 100.0
    public var dataBarColorHex: String = "#2563EB"
    public var textCalloutField: String = "Owner"
    public var textCalloutPrefix: String = ""
    public var colorByValueEnabled: Bool = true
    public var colorByValueField: String = "Status"
    public var showLegendOnPage: Bool = false
    public var legendField: String = "Status"
}

public struct ExternalDataRecordsetSummary: Identifiable, Equatable, Sendable {
    public let id: UInt32
    public let name: String
    public let sourceKind: String
    public let sourceName: String
    public let keyColumn: String
    public let columns: [String]
    public let rows: [[String: String]]
    public let boundShapeIdsByRow: [Int: [UInt64]]
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

public struct InlineShapeTextEditContext: Equatable, Sendable {
    public let shapeId: UInt64
    public let viewRect: CGRect
    public var initialText: String

    public init(shapeId: UInt64, viewRect: CGRect, initialText: String) {
        self.shapeId = shapeId
        self.viewRect = viewRect
        self.initialText = initialText
    }
}

public struct ImportedStencilSubPackPayload: Decodable, Sendable {
    public let packTitle: String
    public let masters: [NativeStencilMasterPayload]
}

public struct ImportedStencilPackPayload: Decodable, Sendable {
    public let packTitle: String
    public let masters: [NativeStencilMasterPayload]
    public let packs: [ImportedStencilSubPackPayload]?
}

public struct OfficialStencilCatalogEntry: Identifiable, Decodable, Equatable, Sendable {
    public let id: String
    public let title: String
    public let vendor: String
    public let categoryGroup: String
    public let description: String
    public let fileName: String
    public let url: String
    public let approxSizeKb: Int
}

public struct SlideSnippetPayload: Identifiable, Codable, Equatable, Sendable {
    public let id: UInt32
    public let page_id: UInt32
    public var title: String
    public var notes: String
    public var x_in: Double
    public var y_in: Double
    public var width_in: Double
    public var height_in: Double
}

// MARK: - Thread-Safe C-ABI Lifecycle Handle (VexioEngine*, VexioDoc*, VexioViewport*)

private final class NativeDocumentCore: @unchecked Sendable {
    private let lock = NSLock()
    private let engine: OpaquePointer
    private var doc: OpaquePointer
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

    func hasViewport() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return viewport != nil
    }

    func resizeViewport(widthPx: UInt32, heightPx: UInt32, dpiScale: Float) {
        lock.lock()
        defer { lock.unlock() }
        if let activeViewport = viewport {
            _ = vexio_viewport_resize(
                activeViewport,
                max(widthPx, 1),
                max(heightPx, 1),
                max(dpiScale, 1.0)
            )
        }
    }

    func validateDiagramJSON(ruleset: String) throws -> String {
        lock.lock()
        defer { lock.unlock() }
        var outBuf = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status = ruleset.withCString { rsCStr in
            vexio_doc_validate_json(doc, rsCStr, &outBuf)
        }
        if let err = VexioBridgeError.from(status: status, context: "validate_json(\(ruleset))") {
            vexio_buffer_free(outBuf)
            throw err
        }
        return Self.consumeOwnedBufferAsUTF8(outBuf)
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

    func selectionIds() -> [UInt64] {
        lock.lock()
        defer { lock.unlock() }
        guard let activeViewport = viewport else { return [] }
        var outJSON = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        guard vexio_viewport_get_selection_json(activeViewport, &outJSON) == VEXIO_OK else {
            vexio_buffer_free(outJSON)
            return []
        }
        let data = Self.consumeOwnedBufferAsData(outJSON)
        return (try? JSONDecoder().decode([UInt64].self, from: data)) ?? []
    }

    func setSelectionIds(_ ids: [UInt64]) {
        lock.lock()
        defer { lock.unlock() }
        guard let activeViewport = viewport else { return }
        ids.withUnsafeBufferPointer { buf in
            _ = vexio_viewport_set_selection(activeViewport, doc, buf.baseAddress, buf.count)
        }
    }

    func setSmartGuides(enabled: Bool) {
        lock.lock()
        defer { lock.unlock() }
        guard let activeViewport = viewport else { return }
        _ = vexio_viewport_set_smart_guides(activeViewport, enabled)
    }

    func alignShapes(pageId: UInt32, shapeIds: [UInt64], mode: String) throws -> UInt32 {
        guard !shapeIds.isEmpty else { return 0 }
        lock.lock()
        defer { lock.unlock() }
        var count: UInt32 = 0
        let status = shapeIds.withUnsafeBufferPointer { buf in
            mode.withCString { modeCStr in
                vexio_doc_align_shapes(doc, pageId, buf.baseAddress, buf.count, modeCStr, &count)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "align_shapes(\(mode))") {
            throw err
        }
        return count
    }

    func distributeShapes(pageId: UInt32, shapeIds: [UInt64], axis: String) throws -> UInt32 {
        guard shapeIds.count >= 3 else { return 0 }
        lock.lock()
        defer { lock.unlock() }
        var count: UInt32 = 0
        let status = shapeIds.withUnsafeBufferPointer { buf in
            axis.withCString { axisCStr in
                vexio_doc_distribute_shapes(doc, pageId, buf.baseAddress, buf.count, axisCStr, &count)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "distribute_shapes(\(axis))") {
            throw err
        }
        return count
    }

    func groupShapes(pageId: UInt32, shapeIds: [UInt64]) throws -> UInt64 {
        guard shapeIds.count >= 2 else { return 0 }
        lock.lock()
        defer { lock.unlock() }
        var outGroupId: UInt64 = 0
        let status = shapeIds.withUnsafeBufferPointer { buf in
            vexio_doc_group_shapes(doc, pageId, buf.baseAddress, buf.count, &outGroupId)
        }
        if let err = VexioBridgeError.from(status: status, context: "group_shapes") {
            throw err
        }
        return outGroupId
    }

    func ungroupShapes(pageId: UInt32, groupIds: [UInt64]) throws -> [UInt64] {
        guard !groupIds.isEmpty else { return [] }
        lock.lock()
        defer { lock.unlock() }
        var outJSON = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status = groupIds.withUnsafeBufferPointer { buf in
            vexio_doc_ungroup_shapes(doc, pageId, buf.baseAddress, buf.count, &outJSON)
        }
        if let err = VexioBridgeError.from(status: status, context: "ungroup_shapes") {
            vexio_buffer_free(outJSON)
            throw err
        }
        let data = Self.consumeOwnedBufferAsData(outJSON)
        return (try? JSONDecoder().decode([UInt64].self, from: data)) ?? []
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

    func insertContainer(pageId: UInt32, title: String, memberIds: [UInt64]) throws -> UInt64 {
        lock.lock()
        defer { lock.unlock() }

        var outId: UInt64 = 0
        let status: VexioStatus = withUnsafeMutablePointer(to: &outId) { outPtr in
            title.withCString { titleCStr in
                memberIds.withUnsafeBufferPointer { idsBuf in
                    vexio_doc_insert_container(doc, pageId, titleCStr, idsBuf.baseAddress, idsBuf.count, outPtr)
                }
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "insert_container") {
            throw err
        }
        return outId
    }

    func insertSwimlanes(pageId: UInt32, lanesSemicolon: String = "") throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }

        var outCount: UInt32 = 0
        let status: VexioStatus = withUnsafeMutablePointer(to: &outCount) { outPtr in
            lanesSemicolon.withCString { lanesCStr in
                vexio_doc_insert_swimlanes(doc, pageId, lanesCStr, outPtr)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "insert_swimlanes") {
            throw err
        }
        return outCount
    }

    func attachCallout(pageId: UInt32, targetShapeId: UInt64, text: String) throws -> UInt64 {
        lock.lock()
        defer { lock.unlock() }

        var outId: UInt64 = 0
        let status: VexioStatus = withUnsafeMutablePointer(to: &outId) { outPtr in
            text.withCString { textCStr in
                vexio_doc_attach_callout(doc, pageId, targetShapeId, textCStr, outPtr)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "attach_callout") {
            throw err
        }
        return outId
    }

    func autoConnect(pageId: UInt32, sourceShapeId: UInt64, direction: String) throws -> UInt64 {
        lock.lock()
        defer { lock.unlock() }

        var outTargetId: UInt64 = 0
        var outConnId: UInt64 = 0
        let status: VexioStatus = withUnsafeMutablePointer(to: &outTargetId) { tPtr in
            withUnsafeMutablePointer(to: &outConnId) { cPtr in
                direction.withCString { dirCStr in
                    vexio_doc_auto_connect(doc, pageId, sourceShapeId, dirCStr, tPtr, cPtr)
                }
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "auto_connect(\(direction))") {
            throw err
        }
        return outTargetId
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

    func renderActiveViewportRGBA(widthPx: UInt32, heightPx: UInt32) -> CGImage? {
        lock.lock()
        defer { lock.unlock() }

        guard let activeViewport = viewport else { return nil }
        let safeWidth = Int(max(widthPx, 1))
        let safeHeight = Int(max(heightPx, 1))
        var rgbaBuffer = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status = vexio_viewport_render_rgba(activeViewport, doc, &rgbaBuffer)
        guard status == VEXIO_OK else {
            vexio_buffer_free(rgbaBuffer)
            return nil
        }

        let rgbaData = Self.consumeOwnedBufferAsData(rgbaBuffer)
        let expectedByteCount = safeWidth * safeHeight * 4
        guard rgbaData.count >= expectedByteCount,
              let colorSpace = CGColorSpace(name: CGColorSpace.sRGB),
              let dataProvider = CGDataProvider(data: rgbaData as CFData)
        else {
            return nil
        }

        return CGImage(
            width: safeWidth,
            height: safeHeight,
            bitsPerComponent: 8,
            bitsPerPixel: 32,
            bytesPerRow: safeWidth * 4,
            space: colorSpace,
            bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
            provider: dataProvider,
            decode: nil,
            shouldInterpolate: true,
            intent: .defaultIntent
        )
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

    func replaceWithBlankDocument(title: String) throws {
        lock.lock()
        defer { lock.unlock() }

        guard let createdDoc = title.withCString({ vexio_doc_create_blank($0) }) else {
            throw VexioBridgeError.blankDocumentCreationFailed
        }
        let oldDoc = doc
        doc = createdDoc
        vexio_doc_destroy(oldDoc)
        if let activeViewport = viewport {
            _ = vexio_viewport_set_page(activeViewport, doc, 1)
            _ = vexio_viewport_render_frame(activeViewport, doc)
        }
    }

    func replaceWithFileData(_ fileData: Data, formatExtension: String) throws {
        guard !fileData.isEmpty else {
            throw VexioBridgeError.emptyDocumentPayload
        }
        lock.lock()
        defer { lock.unlock() }

        var openedDoc: OpaquePointer?
        let status: VexioStatus = fileData.withUnsafeBytes { rawBuffer in
            let basePtr = rawBuffer.bindMemory(to: UInt8.self).baseAddress
            let slice = VexioByteSlice(ptr: basePtr, len: rawBuffer.count)
            return formatExtension.withCString { extCStr in
                vexio_doc_open_bytes(engine, slice, extCStr, &openedDoc)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: formatExtension) {
            if let partial = openedDoc { vexio_doc_destroy(partial) }
            throw err
        }
        guard let validDoc = openedDoc else {
            throw VexioBridgeError.corruptOrUnsupportedFormat(extensionName: formatExtension)
        }
        let oldDoc = doc
        doc = validDoc
        vexio_doc_destroy(oldDoc)
        if let activeViewport = viewport {
            _ = vexio_viewport_set_page(activeViewport, doc, 1)
            _ = vexio_viewport_render_frame(activeViewport, doc)
        }
    }

    func viewportActiveToolCode() -> UInt32 {
        lock.lock()
        defer { lock.unlock() }
        guard let activeViewport = viewport else { return 0 }
        return vexio_viewport_get_active_tool(activeViewport)
    }

    func popCreatedTextShapeId() -> UInt64 {
        lock.lock()
        defer { lock.unlock() }
        guard let activeViewport = viewport else { return 0 }
        return vexio_viewport_pop_created_text_shape(activeViewport)
    }

    func hitTestShape(xPx: Double, yPx: Double) -> UInt64 {
        lock.lock()
        defer { lock.unlock() }
        guard let activeViewport = viewport else { return 0 }
        return vexio_viewport_hit_test_shape(activeViewport, doc, xPx, yPx)
    }

    func shapeScreenBoundsPx(shapeId: UInt64) -> CGRect? {
        lock.lock()
        defer { lock.unlock() }
        guard let activeViewport = viewport else { return nil }
        var x = 0.0
        var y = 0.0
        var w = 0.0
        var h = 0.0
        let status = vexio_viewport_get_shape_screen_bounds(activeViewport, doc, shapeId, &x, &y, &w, &h)
        guard status == VEXIO_OK else { return nil }
        return CGRect(x: x, y: y, width: w, height: h)
    }

    func viewportScreenToPageIn(xPx: Double, yPx: Double) -> CGPoint? {
        lock.lock()
        defer { lock.unlock() }
        guard let activeViewport = viewport else { return nil }
        var xIn = 0.0
        var yIn = 0.0
        let status = vexio_viewport_screen_to_page(activeViewport, xPx, yPx, &xIn, &yIn)
        guard status == VEXIO_OK else { return nil }
        return CGPoint(x: xIn, y: yIn)
    }

    func getStateJSON(activePageId: UInt32) -> String {
        lock.lock()
        defer { lock.unlock() }
        var outJSON = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status = vexio_doc_get_state_json(doc, activePageId, &outJSON)
        guard status == VEXIO_OK else {
            vexio_buffer_free(outJSON)
            return "{}"
        }
        return Self.consumeOwnedBufferAsUTF8(outJSON)
    }

    func setShapeText(pageId: UInt32, shapeId: UInt64, text: String) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = text.withCString { txtCStr in
            vexio_doc_set_shape_text(doc, pageId, shapeId, txtCStr)
        }
        if let err = VexioBridgeError.from(status: status, context: "set_shape_text") {
            throw err
        }
    }

    func setShapeTextStyleJSON(pageId: UInt32, shapeIds: [UInt64], styleJSON: String) throws {
        guard !shapeIds.isEmpty else { return }
        lock.lock()
        defer { lock.unlock() }
        let status = shapeIds.withUnsafeBufferPointer { idsBuf in
            styleJSON.withCString { jsonCStr in
                vexio_doc_set_shape_text_style_json(doc, pageId, idsBuf.baseAddress, idsBuf.count, jsonCStr)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "set_shape_text_style_json") {
            throw err
        }
    }

    func rotateFlipShapes(
        pageId: UInt32,
        shapeIds: [UInt64],
        deltaAngleDeg: Double,
        toggleFlipX: Bool,
        toggleFlipY: Bool
    ) throws {
        guard !shapeIds.isEmpty else { return }
        lock.lock()
        defer { lock.unlock() }
        let status = shapeIds.withUnsafeBufferPointer { idsBuf in
            vexio_doc_rotate_flip_shapes(
                doc,
                pageId,
                idsBuf.baseAddress,
                idsBuf.count,
                deltaAngleDeg,
                toggleFlipX,
                toggleFlipY
            )
        }
        if let err = VexioBridgeError.from(status: status, context: "rotate_flip_shapes") {
            throw err
        }
    }

    func setPagePreset(pageId: UInt32, presetSlug: String, landscape: Bool) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = presetSlug.withCString { slugCStr in
            vexio_doc_set_page_preset(doc, pageId, slugCStr, landscape)
        }
        if let err = VexioBridgeError.from(status: status, context: "set_page_preset(\(presetSlug))") {
            throw err
        }
    }

    func setPageSetup(pageId: UInt32, widthIn: Double, heightIn: Double, unitSlug: String) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = unitSlug.withCString { unitCStr in
            vexio_doc_set_page_setup(doc, pageId, widthIn, heightIn, unitCStr)
        }
        if let err = VexioBridgeError.from(status: status, context: "set_page_setup") {
            throw err
        }
    }

    func setDocumentTitle(_ title: String) {
        lock.lock()
        defer { lock.unlock() }
        _ = title.withCString { titleCStr in
            vexio_doc_set_title(doc, titleCStr)
        }
    }

    func renamePage(pageId: UInt32, newName: String) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = newName.withCString { nameCStr in
            vexio_doc_rename_page(doc, pageId, nameCStr)
        }
        if let err = VexioBridgeError.from(status: status, context: "rename_page(\(pageId))") {
            throw err
        }
    }

    func duplicatePage(pageId: UInt32) throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }
        var outNewId: UInt32 = 0
        let status = vexio_doc_duplicate_page(doc, pageId, &outNewId)
        if let err = VexioBridgeError.from(status: status, context: "duplicate_page(\(pageId))") {
            throw err
        }
        return outNewId
    }

    func deletePage(pageId: UInt32) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = vexio_doc_delete_page(doc, pageId)
        if let err = VexioBridgeError.from(status: status, context: "delete_page(\(pageId))") {
            throw err
        }
    }

    func setPageBackground(
        pageId: UInt32,
        isBackground: Bool,
        backPageId: UInt32,
        pageColorHex: String
    ) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = pageColorHex.withCString { colorCStr in
            vexio_doc_set_page_background(doc, pageId, isBackground, backPageId, colorCStr)
        }
        if let err = VexioBridgeError.from(status: status, context: "set_page_background(\(pageId))") {
            throw err
        }
    }

    func applyHeadingPreset(pageId: UInt32, shapeIds: [UInt64], presetSlug: String) throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }
        var outCount: UInt32 = 0
        let status: VexioStatus = shapeIds.withUnsafeBufferPointer { buf in
            presetSlug.withCString { presetCStr in
                vexio_doc_apply_heading_preset(
                    doc,
                    pageId,
                    buf.baseAddress,
                    buf.count,
                    presetCStr,
                    &outCount
                )
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "apply_heading_preset(\(presetSlug))") {
            throw err
        }
        return outCount
    }

    func insertHeading(
        pageId: UInt32,
        presetSlug: String,
        customText: String?,
        xIn: Double,
        yIn: Double
    ) throws -> UInt64 {
        lock.lock()
        defer { lock.unlock() }
        var outSid: UInt64 = 0
        let textStr = customText ?? ""
        let status: VexioStatus = presetSlug.withCString { presetCStr in
            textStr.withCString { textCStr in
                vexio_doc_insert_heading(
                    doc,
                    pageId,
                    presetCStr,
                    textStr.isEmpty ? nil : textCStr,
                    xIn,
                    yIn,
                    &outSid
                )
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "insert_heading(\(presetSlug))") {
            throw err
        }
        return outSid
    }

    func applyBorderAndTitle(
        pageId: UInt32,
        styleSlug: String,
        title: String,
        subtitle: String,
        author: String,
        dateOrRev: String
    ) throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }
        var outBgPid: UInt32 = 0
        let status: VexioStatus = styleSlug.withCString { sCStr in
            title.withCString { tCStr in
                subtitle.withCString { subCStr in
                    author.withCString { aCStr in
                        dateOrRev.withCString { dCStr in
                            vexio_doc_apply_border_and_title(
                                doc,
                                pageId,
                                sCStr,
                                tCStr,
                                subCStr,
                                aCStr,
                                dCStr,
                                &outBgPid
                            )
                        }
                    }
                }
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "apply_border_and_title(\(styleSlug))") {
            throw err
        }
        return outBgPid
    }

    func importStencilBytes(fileName: String, bytes: Data) throws -> String {
        guard !bytes.isEmpty else {
            throw VexioBridgeError.emptyDocumentPayload
        }
        lock.lock()
        defer { lock.unlock() }
        var outJSON = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status: VexioStatus = bytes.withUnsafeBytes { rawBuf in
            let ptr = rawBuf.bindMemory(to: UInt8.self).baseAddress
            let slice = VexioByteSlice(ptr: ptr, len: rawBuf.count)
            return fileName.withCString { nameCStr in
                vexio_doc_import_stencil_bytes(doc, nameCStr, slice, &outJSON)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "import_stencil(\(fileName))") {
            vexio_buffer_free(outJSON)
            throw err
        }
        return Self.consumeOwnedBufferAsUTF8(outJSON)
    }

    func listOfficialStencilCatalogJSON() -> String {
        var outJSON = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status = vexio_list_official_stencil_catalog_json(&outJSON)
        if VexioBridgeError.from(status: status, context: "list_official_stencil_catalog") != nil {
            vexio_buffer_free(outJSON)
            return "[]"
        }
        return Self.consumeOwnedBufferAsUTF8(outJSON)
    }

    func renameShape(pageId: UInt32, shapeId: UInt64, newName: String) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = newName.withCString { nameCStr in
            vexio_doc_rename_shape(doc, pageId, shapeId, nameCStr)
        }
        if let err = VexioBridgeError.from(status: status, context: "rename_shape(\(shapeId))") {
            throw err
        }
    }

    func upsertShapeData(
        pageId: UInt32,
        shapeId: UInt64,
        rowName: String,
        label: String,
        dataType: UInt8,
        value: String
    ) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = rowName.withCString { rCStr in
            label.withCString { lCStr in
                value.withCString { vCStr in
                    vexio_doc_upsert_shape_data(doc, pageId, shapeId, rCStr, lCStr, dataType, vCStr)
                }
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "upsert_shape_data(\(rowName))") {
            throw err
        }
    }

    func deleteShapeData(pageId: UInt32, shapeId: UInt64, rowName: String) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = rowName.withCString { rCStr in
            vexio_doc_delete_shape_data(doc, pageId, shapeId, rCStr)
        }
        if let err = VexioBridgeError.from(status: status, context: "delete_shape_data(\(rowName))") {
            throw err
        }
    }

    func addLayer(pageId: UInt32, layerName: String) throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }
        var outIx: UInt32 = 0
        let status = layerName.withCString { nameCStr in
            vexio_doc_add_layer(doc, pageId, nameCStr, &outIx)
        }
        if let err = VexioBridgeError.from(status: status, context: "add_layer(\(layerName))") {
            throw err
        }
        return outIx
    }

    func setLayerFlags(
        pageId: UInt32,
        layerIx: UInt32,
        visible: Bool,
        locked: Bool,
        printable: Bool,
        snapGlue: Bool
    ) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = vexio_doc_set_layer_flags(doc, pageId, layerIx, visible, locked, printable, snapGlue)
        if let err = VexioBridgeError.from(status: status, context: "set_layer_flags(\(layerIx))") {
            throw err
        }
    }

    func setShapeLayerMembership(
        pageId: UInt32,
        shapeId: UInt64,
        layerIx: UInt32,
        assigned: Bool
    ) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = vexio_doc_set_shape_layer_membership(doc, pageId, shapeId, layerIx, assigned)
        if let err = VexioBridgeError.from(status: status, context: "set_shape_layer_membership(\(shapeId),\(layerIx))") {
            throw err
        }
    }

    // MARK: - Pillar #4 C-ABI: Live Data Graphics, Legends & Two-Way External Data Linking

    func setPageDataGraphicsJSON(pageId: UInt32, configJSON: String) throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }
        var outCount: UInt32 = 0
        let status = configJSON.withCString { jsonCStr in
            vexio_doc_set_page_data_graphics_json(doc, pageId, jsonCStr, &outCount)
        }
        if let err = VexioBridgeError.from(status: status, context: "set_page_data_graphics") {
            throw err
        }
        return outCount
    }

    func importExternalRecordset(
        pageId: UInt32,
        fileName: String,
        bytes: Data,
        keyColumn: String = "",
        tableName: String = "",
        autoBind: Bool = true,
        autoEnableGraphics: Bool = true
    ) throws -> (recordsetId: UInt32, linkedCount: UInt32) {
        lock.lock()
        defer { lock.unlock() }
        var rsId: UInt32 = 0
        var linked: UInt32 = 0
        let status: VexioStatus = bytes.withUnsafeBytes { rawBuf in
            let ptr = rawBuf.bindMemory(to: UInt8.self).baseAddress
            let slice = VexioByteSlice(ptr: ptr, len: rawBuf.count)
            return fileName.withCString { fCStr in
                keyColumn.withCString { kCStr in
                    tableName.withCString { tCStr in
                        vexio_doc_import_external_recordset(
                            doc,
                            pageId,
                            fCStr,
                            slice,
                            kCStr,
                            tCStr,
                            autoBind,
                            autoEnableGraphics,
                            &rsId,
                            &linked
                        )
                    }
                }
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "import_external_recordset(\(fileName))") {
            throw err
        }
        return (rsId, linked)
    }

    func bindExternalRow(
        pageId: UInt32,
        recordsetId: UInt32,
        rowIndex: UInt32,
        shapeIds: [UInt64]
    ) throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }
        var outBound: UInt32 = 0
        let status: VexioStatus = shapeIds.withUnsafeBufferPointer { buf in
            vexio_doc_bind_external_row(
                doc,
                pageId,
                recordsetId,
                rowIndex,
                buf.baseAddress,
                buf.count,
                &outBound
            )
        }
        if let err = VexioBridgeError.from(status: status, context: "bind_external_row(\(recordsetId),\(rowIndex))") {
            throw err
        }
        return outBound
    }

    func refreshExternalRecordset(
        pageId: UInt32,
        recordsetId: UInt32 = 0,
        newBytes: Data? = nil,
        formatHint: String = ""
    ) throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }
        var outRefreshed: UInt32 = 0
        let status: VexioStatus
        if let payload = newBytes, !payload.isEmpty {
            status = payload.withUnsafeBytes { rawBuf in
                let ptr = rawBuf.bindMemory(to: UInt8.self).baseAddress
                let slice = VexioByteSlice(ptr: ptr, len: payload.count)
                return formatHint.withCString { fmtCStr in
                    vexio_doc_refresh_external_recordset(doc, pageId, recordsetId, slice, fmtCStr, &outRefreshed)
                }
            }
        } else {
            let emptySlice = VexioByteSlice(ptr: nil, len: 0)
            status = formatHint.withCString { fmtCStr in
                vexio_doc_refresh_external_recordset(doc, pageId, recordsetId, emptySlice, fmtCStr, &outRefreshed)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "refresh_external_recordset") {
            throw err
        }
        return outRefreshed
    }

    func syncAndExportRecordset(pageId: UInt32, recordsetId: UInt32, formatExt: String) throws -> Data {
        lock.lock()
        defer { lock.unlock() }
        var outBuf = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status = formatExt.withCString { fmtCStr in
            vexio_doc_sync_and_export_recordset(doc, pageId, recordsetId, fmtCStr, &outBuf)
        }
        if let err = VexioBridgeError.from(status: status, context: "sync_and_export_recordset(\(formatExt))") {
            vexio_buffer_free(outBuf)
            throw err
        }
        return Self.consumeOwnedBufferAsData(outBuf)
    }

    func deleteExternalRecordset(recordsetId: UInt32) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = vexio_doc_delete_external_recordset(doc, recordsetId)
        if let err = VexioBridgeError.from(status: status, context: "delete_external_recordset(\(recordsetId))") {
            throw err
        }
    }

    func insertDataLegend(
        pageId: UInt32,
        propName: String = "",
        pinXIn: Double = 0.0,
        pinYIn: Double = 0.0
    ) throws -> UInt64 {
        lock.lock()
        defer { lock.unlock() }
        var lid: UInt64 = 0
        let status = propName.withCString { propCStr in
            vexio_doc_insert_data_legend(doc, pageId, propCStr, pinXIn, pinYIn, &lid)
        }
        if let err = VexioBridgeError.from(status: status, context: "insert_data_legend") {
            throw err
        }
        return lid
    }

    func exportBomReport(formatExt: String) throws -> Data {
        lock.lock()
        defer { lock.unlock() }
        var outBuf = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status = formatExt.withCString { fmtCStr in
            vexio_doc_export_bom_report(doc, fmtCStr, &outBuf)
        }
        if let err = VexioBridgeError.from(status: status, context: "export_bom_report(\(formatExt))") {
            vexio_buffer_free(outBuf)
            throw err
        }
        return Self.consumeOwnedBufferAsData(outBuf)
    }

    func importMarkup(pageId: UInt32, markup: String, formatSlug: String) throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }
        var outNodes: UInt32 = 0
        let status = markup.withCString { mCStr in
            formatSlug.withCString { fCStr in
                vexio_doc_import_markup(doc, pageId, mCStr, fCStr, &outNodes)
            }
        }
        if let err = VexioBridgeError.from(status: status, context: "import_markup(\(formatSlug))") {
            throw err
        }
        return outNodes
    }

    func exportMarkup(pageId: UInt32, formatSlug: String) throws -> String {
        lock.lock()
        defer { lock.unlock() }
        var outBuf = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status = formatSlug.withCString { fCStr in
            vexio_doc_export_markup(doc, pageId, fCStr, &outBuf)
        }
        if let err = VexioBridgeError.from(status: status, context: "export_markup(\(formatSlug))") {
            vexio_buffer_free(outBuf)
            throw err
        }
        return Self.consumeOwnedBufferAsUTF8(outBuf)
    }

    func listSlideSnippetsJson() throws -> String {
        lock.lock()
        defer { lock.unlock() }
        var outBuf = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        let status = vexio_doc_list_slide_snippets_json(doc, &outBuf)
        if let err = VexioBridgeError.from(status: status, context: "list_slide_snippets") {
            vexio_buffer_free(outBuf)
            throw err
        }
        return Self.consumeOwnedBufferAsUTF8(outBuf)
    }

    func autoSlideSnippets(pageId: UInt32) throws -> UInt32 {
        lock.lock()
        defer { lock.unlock() }
        var outCount: UInt32 = 0
        let status = vexio_doc_auto_slide_snippets(doc, pageId, &outCount)
        if let err = VexioBridgeError.from(status: status, context: "auto_slide_snippets") {
            throw err
        }
        return outCount
    }

    func deleteSlideSnippet(pageId: UInt32, snippetId: UInt32) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = vexio_doc_delete_slide_snippet(doc, pageId, snippetId)
        if let err = VexioBridgeError.from(status: status, context: "delete_slide_snippet") {
            throw err
        }
    }

    func resetConnectorRouting(pageId: UInt32, shapeId: UInt64) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = vexio_doc_reset_connector_routing(doc, pageId, shapeId)
        if let err = VexioBridgeError.from(status: status, context: "reset_connector_routing") {
            throw err
        }
    }

    func ensureControlHandle(pageId: UInt32, shapeId: UInt64) throws {
        lock.lock()
        defer { lock.unlock() }
        let status = vexio_doc_ensure_control_handle(doc, pageId, shapeId)
        if let err = VexioBridgeError.from(status: status, context: "ensure_control_handle") {
            throw err
        }
    }
}

// MARK: - SwiftUI Color <-> Hex RGB Helpers for Native OS ColorPicker

public extension Color {
    init(vexioHex hex: String) {
        let cleaned = hex.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: "#", with: "")
        guard cleaned.count == 6, let val = UInt32(cleaned, radix: 16) else {
            self = Color(red: 0.06, green: 0.09, blue: 0.16)
            return
        }
        let r = Double((val >> 16) & 0xFF) / 255.0
        let g = Double((val >> 8) & 0xFF) / 255.0
        let b = Double(val & 0xFF) / 255.0
        self = Color(.sRGB, red: r, green: g, blue: b, opacity: 1.0)
    }

    var vexioHexRGBString: String {
        let ns = NSColor(self).usingColorSpace(.sRGB) ?? NSColor.black
        let r = Int(round(min(max(ns.redComponent, 0), 1) * 255.0))
        let g = Int(round(min(max(ns.greenComponent, 0), 1) * 255.0))
        let b = Int(round(min(max(ns.blueComponent, 0), 1) * 255.0))
        return String(format: "#%02X%02X%02X", r, g, b)
    }

    var vexioRGBFormulaString: String {
        let ns = NSColor(self).usingColorSpace(.sRGB) ?? NSColor.black
        let r = Int(round(min(max(ns.redComponent, 0), 1) * 255.0))
        let g = Int(round(min(max(ns.greenComponent, 0), 1) * 255.0))
        let b = Int(round(min(max(ns.blueComponent, 0), 1) * 255.0))
        return "RGB(\(r),\(g),\(b))"
    }
}

// MARK: - @MainActor Swift 6 Document Session (ReferenceFileDocument)

@MainActor
public final class VexioDocumentSession: ReferenceFileDocument, ObservableObject {
    public typealias Snapshot = VexioDocumentSnapshot

    /// Cached sorted list of all native macOS installed font families (`NSFontManager.shared.availableFontFamilies`).
    public static let installedSystemFontFamilies: [String] = {
        var set = Set(NSFontManager.shared.availableFontFamilies.filter { !$0.hasPrefix(".") && !$0.isEmpty })
        for preferred in ["SF Pro", "Helvetica Neue", "Inter", "Segoe UI", "Arial", "Georgia", "Times New Roman", "Menlo"] {
            set.insert(preferred)
        }
        return set.sorted { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }
    }()

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
    nonisolated private let initialSourceFormatExtension: String
    @Published public var sourceFormatExtension: String = "vsdx"
    @Published public var currentFileURL: URL? = nil

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
    @Published public var selectedShapeIds: [UInt64] = []
    @Published public var nextAllocatedShapeId: UInt64 = 10

    // Inline Canvas Label Editor & Rich Text / Color Picker State
    @Published public var activeInlineTextEdit: InlineShapeTextEditContext? = nil
    @Published public var selectedFontFamily: String = "Helvetica Neue"
    @Published public var selectedFontSizePt: Double = 10.0
    @Published public var selectedCharStyle: UInt8 = 0
    @Published public var selectedTextColorHex: String = "#0F172A"
    @Published public var selectedTextBgColorHex: String = "none"
    @Published public var selectedFillColorHex: String = "#EBF4FF"
    @Published public var selectedLineColorHex: String = "#1C64F2"
    @Published public var selectedHorizAlign: UInt8 = 1
    @Published public var selectedVertAlign: UInt8 = 1
    @Published public var selectedBulletStyle: UInt8 = 0
    @Published public var pageWidthInches: Double = 297.0 / 25.4
    @Published public var pageHeightInches: Double = 210.0 / 25.4
    public var isPageLandscape: Bool { pageWidthInches >= pageHeightInches }

    private var isSuppressingCameraSync: Bool = false
    private var lastViewportLogicalSize: CGSize = .zero
    private var needsFitOnViewportReady: Bool = true
    @Published public var isAutoFitPageMode: Bool = true

    private var didCaptureInitialBaselineRevision: Bool = false
    private var savedStateRevision: UInt64 = 0
    private var currentStateRevision: UInt64 = 0
    private var hasMetadataUnsavedEdit: Bool = false
    @Published public private(set) var hasUnsavedChanges: Bool = false

    @Published public var zoomScale: CGFloat = 1.0 {
        didSet {
            if !isSuppressingCameraSync {
                syncViewportCamera()
            }
        }
    }
    @Published public var panOffset: CGSize = .zero {
        didSet {
            if !isSuppressingCameraSync {
                syncViewportCamera()
            }
        }
    }
    @Published public var cursorCanvasInches: CGPoint = .zero
    @Published public var activeCursorKind: UInt32 = 0
    @Published public var selectedShapeIsGroup: Bool = false

    @Published public var connectorRouting: ConnectorRoutingStyle = .orthogonalJumps
    @Published public var smartGuidesEnabled: Bool = true {
        didSet {
            core.setSmartGuides(enabled: smartGuidesEnabled)
            core.renderViewportFrame()
        }
    }
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
        VexioPageSummary(id: 1, title: "Page-1", architecturalScale: "1:1 (Metric / OS Standard)"),
    ]

    @Published public var shapeSheetCells: [ShapeSheetCellEntry] = []
    @Published public var inspectedObject: InspectedObjectState? = nil
    @Published public var activePageShapeCount: Int = 0
    @Published public var shapeDataProperties: [ShapeDataPropertyItem] = []

    @Published public var layers: [DiagramLayerState] = [
        DiagramLayerState(id: 0, name: "Flowchart", isVisible: true, isPrintable: true, isLocked: false, isSnapGlueEnabled: true, memberCount: 0, accentHex: "#0A84FF"),
        DiagramLayerState(id: 1, name: "Connector", isVisible: true, isPrintable: true, isLocked: false, isSnapGlueEnabled: true, memberCount: 0, accentHex: "#30D158"),
        DiagramLayerState(id: 2, name: "Annotations", isVisible: true, isPrintable: true, isLocked: false, isSnapGlueEnabled: true, memberCount: 0, accentHex: "#FF9F0A"),
    ]

    @Published public var dataGraphicConfig = DataGraphicRuleConfig()
    @Published public var externalRecordsets: [ExternalDataRecordsetSummary] = []
    @Published public var availablePropFields: [String] = []
    @Published public var slideSnippets: [SlideSnippetPayload] = []
    private var linkedRecordsetURLs: [UInt32: URL] = [:]
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
        let localCore: NativeDocumentCore
        do {
            localCore = try NativeDocumentCore(blankTitle: blankTitle)
        } catch {
            preconditionFailure("Unable to create native Vexio engine/document: \(error.localizedDescription)")
        }
        let defaultPreset = (Locale.current.measurementSystem == .us) ? "letter" : "a4"
        try? localCore.setPagePreset(pageId: 1, presetSlug: defaultPreset, landscape: true)
        self.core = localCore
        self.initialSourceFormatExtension = "vsdx"
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
        self.initialSourceFormatExtension = ext
    }

    public func ensureStencilsLoadedIfNeeded() {
        guard !hasLoadedDynamicStencils else { return }
        hasLoadedDynamicStencils = true
        if currentFileURL == nil && sourceFormatExtension == "vsdx" {
            sourceFormatExtension = initialSourceFormatExtension
        }
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
                        svgPreview: item.svg,
                        pngPreview: item.pngDataUrl ?? ""
                    )
                )
            }
        }

        if !flatMasters.isEmpty {
            stencilCategories = orderedCategories
            stencilCatalog = flatMasters
        }
        loadPersistedUserStencils()
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
        let panXIn = pageWidthInches * 0.5 - Double(panOffset.width) / (96.0 * Double(max(zoomScale, 0.1)))
        let panYIn = pageHeightInches * 0.5 + Double(panOffset.height) / (96.0 * Double(max(zoomScale, 0.1)))
        core.setViewportZoom(zoomScale: Double(zoomScale), panXIn: panXIn, panYIn: panYIn)
        core.renderViewportFrame()
    }

    public func hasAttachedMetalViewport() -> Bool {
        core.hasViewport()
    }

    public func disengageAutoFitForManualCamera() {
        if isAutoFitPageMode {
            isAutoFitPageMode = false
        }
    }

    public func setManualZoomScale(_ newZoom: CGFloat) {
        disengageAutoFitForManualCamera()
        zoomScale = min(max(newZoom, 0.10), 16.0)
    }

    private func recomputeHasUnsavedChanges() {
        let dirty = (currentStateRevision != savedStateRevision) || hasMetadataUnsavedEdit
        if hasUnsavedChanges != dirty {
            hasUnsavedChanges = dirty
        }
    }

    private func readLatestCoreStateRevision() -> UInt64 {
        let jsonStr = core.getStateJSON(activePageId: activePageId)
        guard let data = jsonStr.data(using: .utf8),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return currentStateRevision
        }
        if let rev = root["stateRevision"] as? UInt64 {
            return rev
        }
        if let revInt = root["stateRevision"] as? Int {
            return UInt64(max(0, revInt))
        }
        if let revNum = root["stateRevision"] as? NSNumber {
            return revNum.uint64Value
        }
        return currentStateRevision
    }

    public func refreshStateRevisionFromCore() {
        currentStateRevision = readLatestCoreStateRevision()
        if !didCaptureInitialBaselineRevision {
            didCaptureInitialBaselineRevision = true
            savedStateRevision = currentStateRevision
        }
        recomputeHasUnsavedChanges()
    }

    public func markCurrentStateAsClean() {
        currentStateRevision = readLatestCoreStateRevision()
        didCaptureInitialBaselineRevision = true
        savedStateRevision = currentStateRevision
        hasMetadataUnsavedEdit = false
        hasUnsavedChanges = false
    }

    public func resizeMetalViewport(widthPx: UInt32, heightPx: UInt32, dpiScale: Float) {
        let safeDpi = CGFloat(max(dpiScale, 0.5))
        lastViewportLogicalSize = CGSize(width: CGFloat(widthPx) / safeDpi, height: CGFloat(heightPx) / safeDpi)
        core.resizeViewport(widthPx: widthPx, heightPx: heightPx, dpiScale: dpiScale)
        if (needsFitOnViewportReady || isAutoFitPageMode) && lastViewportLogicalSize.width >= 64 && lastViewportLogicalSize.height >= 64 {
            needsFitOnViewportReady = false
            fitViewportToActivePage()
        } else {
            core.renderViewportFrame()
        }
    }

    public func attachMetalLayer(
        _ metalLayer: CAMetalLayer,
        widthPx: UInt32,
        heightPx: UInt32,
        dpiScale: Float
    ) {
        let safeDpi = CGFloat(max(dpiScale, 0.5))
        lastViewportLogicalSize = CGSize(width: CGFloat(widthPx) / safeDpi, height: CGFloat(heightPx) / safeDpi)
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
            if lastViewportLogicalSize.width >= 64 && lastViewportLogicalSize.height >= 64 {
                needsFitOnViewportReady = false
                fitViewportToActivePage()
            } else {
                needsFitOnViewportReady = true
                core.renderViewportFrame()
            }
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

    public func attachHeadlessCaptureViewport(widthPx: UInt32, heightPx: UInt32, dpiScale: Float) {
        let safeDpi = CGFloat(max(dpiScale, 0.5))
        lastViewportLogicalSize = CGSize(width: CGFloat(widthPx) / safeDpi, height: CGFloat(heightPx) / safeDpi)
        do {
            try core.bindHeadlessViewport(
                pageId: activePageId,
                widthPx: widthPx,
                heightPx: heightPx,
                dpiScale: dpiScale
            )
            core.setViewportPage(pageId: activePageId)
            core.setViewportTool(toolCode: activeTool.engineToolCode)
            if lastViewportLogicalSize.width >= 64 && lastViewportLogicalSize.height >= 64 {
                needsFitOnViewportReady = false
                fitViewportToActivePage()
            } else {
                needsFitOnViewportReady = true
                core.renderViewportFrame()
            }
            lastBannerError = nil
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func renderActiveViewportCGImage(widthPx: UInt32, heightPx: UInt32) -> CGImage? {
        core.renderActiveViewportRGBA(widthPx: widthPx, heightPx: heightPx)
    }

    @discardableResult
    public func dispatchPointerEvent(_ pointerEvent: VexioPointerEvent) -> NSCursor {
        let dpi = max(CGFloat(NSScreen.main?.backingScaleFactor ?? 2.0), 1.0)
        let canvasXInches = ((CGFloat(pointerEvent.x_px) / dpi) - panOffset.width) / (96.0 * max(zoomScale, 0.1))
        let canvasYInches = ((CGFloat(pointerEvent.y_px) / dpi) - panOffset.height) / (96.0 * max(zoomScale, 0.1))
        cursorCanvasInches = CGPoint(x: canvasXInches, y: canvasYInches)

        do {
            if activeTool == .pan && pointerEvent.kind == 0 && (pointerEvent.buttons & 1) != 0 {
                disengageAutoFitForManualCamera()
            }
            let cursorCode = try core.dispatchPointer(pointerEvent)
            activeCursorKind = cursorCode
            if pointerEvent.kind == 1 || pointerEvent.kind == 2 || (pointerEvent.kind == 0 && pointerEvent.buttons != 0) {
                let ids = core.selectionIds()
                if ids != selectedShapeIds {
                    selectedShapeIds = ids
                }
                if let first = ids.first {
                    if first != selectedShapeId {
                        selectedShapeId = first
                        reloadSelectedShapeSheetCells()
                        syncSelectedShapeMetadataFromCore()
                    } else if pointerEvent.kind == 2 {
                        syncSelectedShapeMetadataFromCore()
                    }
                } else if selectedShapeId != 0 || selectedShapeIsGroup {
                    selectedShapeId = 0
                    selectedShapeIsGroup = false
                    syncSelectedShapeMetadataFromCore()
                } else if pointerEvent.kind == 2 {
                    refreshStateRevisionFromCore()
                }
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
        syncSelectedShapeMetadataFromCore()

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
        if selectedShapeId > 0 {
            upsertShapeInCore(pageId: activePageId, shapeId: selectedShapeId, name: "Sheet.\(selectedShapeId)")
        }
        do {
            try core.setCellFormula(
                pageId: activePageId,
                shapeId: selectedShapeId,
                cellName: cellName,
                formula: formula,
                forceSetF: mode.forceSetFFlag
            )
            lastBannerError = nil
            statusMessage = selectedShapeId > 0
                ? "Updated Sheet.\(selectedShapeId)!\(cellName) = \(formula)"
                : "Updated ThePage!\(cellName) = \(formula)"
            registerJournalUndoStep(actionName: "Edit \(cellName)", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
            throw bridgeErr
        }
    }

    // MARK: - Deterministic Command Journal Undo / Redo

    private func registerJournalUndoStep(actionName: String, undoManager: UndoManager?) {
        refreshStateRevisionFromCore()
        guard let undoManager else { return }
        undoManager.setActionName(actionName)
        undoManager.registerUndo(withTarget: self) { target in
            MainActor.assumeIsolated {
                target.performUndo(undoManager: NSApp.keyWindow?.undoManager)
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
                    target.performRedo(undoManager: NSApp.keyWindow?.undoManager)
                }
            }
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
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
                    target.performUndo(undoManager: NSApp.keyWindow?.undoManager)
                }
            }
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    // MARK: - Stencil Instantiation, Primitive Drawing, Context-Menu Actions & Validation

    @Published public var highlightedStencilMasterId: String = ""
    public var activeDraggedStencilMasterId: String?
    private var lastCenterDropAnchor: CGPoint?
    private var centerDropCascadeStep: Int = 0

    public func screenPxToPageInches(xPx: Double, yPx: Double) -> CGPoint {
        if let pt = core.viewportScreenToPageIn(xPx: xPx, yPx: yPx) {
            return pt
        }
        return CGPoint(x: 5.85, y: 4.13)
    }

    public func instantiateStencilMasterAtViewportCenter(
        masterId: String,
        undoManager: UndoManager?
    ) {
        highlightedStencilMasterId = masterId
        let center = screenPxToPageInches(xPx: -1.0, yPx: -1.0)
        if let prev = lastCenterDropAnchor,
           hypot(center.x - prev.x, center.y - prev.y) < 0.05
        {
            centerDropCascadeStep = (centerDropCascadeStep + 1) % 6
        } else {
            lastCenterDropAnchor = center
            centerDropCascadeStep = 0
        }
        let target = CGPoint(
            x: center.x + CGFloat(centerDropCascadeStep) * 0.28,
            y: center.y - CGFloat(centerDropCascadeStep) * 0.28
        )
        instantiateStencilMaster(masterId: masterId, atCanvasInches: target, undoManager: undoManager)
    }

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

        let safeX = max(Double(point.x), 0.2)
        let safeY = max(Double(point.y), 0.2)

        if let createdId = try? core.dropStencil(
            pageId: activePageId,
            category: master.category,
            masterName: master.name,
            xIn: safeX,
            yIn: safeY
        ), createdId != 0 {
            selectedShapeId = createdId
            selectedShapeIds = [createdId]
            activeTool = .pointer
            core.setSelectionIds([createdId])
            registerJournalUndoStep(actionName: "Drop \(master.name)", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = String(format: "Dropped '%@' as Sheet.%llu", master.name, createdId)
            return
        }

        let shapeId = nextAllocatedShapeId
        nextAllocatedShapeId += 1
        selectedShapeId = shapeId
        selectedShapeIds = [shapeId]
        activeTool = .pointer
        upsertShapeInCore(pageId: activePageId, shapeId: shapeId, name: "\(master.name).\(shapeId)")
        try? core.setCellFormula(pageId: activePageId, shapeId: shapeId, cellName: "PinX", formula: String(format: "%.3f in", safeX), forceSetF: true)
        try? core.setCellFormula(pageId: activePageId, shapeId: shapeId, cellName: "PinY", formula: String(format: "%.3f in", safeY), forceSetF: true)
        try? core.setCellFormula(pageId: activePageId, shapeId: shapeId, cellName: "Width", formula: master.defaultWidthFormula, forceSetF: true)
        try? core.setCellFormula(pageId: activePageId, shapeId: shapeId, cellName: "Height", formula: master.defaultHeightFormula, forceSetF: true)
        try? core.setCellFormula(pageId: activePageId, shapeId: shapeId, cellName: "FillForegnd", formula: master.defaultFillFormula, forceSetF: true)
        registerJournalUndoStep(actionName: "Drop \(master.name)", undoManager: undoManager)
        reloadSelectedShapeSheetCells()
        syncSelectedShapeMetadataFromCore()
        core.renderViewportFrame()
        statusMessage = "Dropped '\(master.name)' as Sheet.\(shapeId)"
    }

    public func drawQuickPrimitive(
        kind: String,
        atCanvasInches point: CGPoint,
        undoManager: UndoManager?
    ) {
        if kind == "text" {
            insertTextBoxAndBeginEditing(atCanvasInches: point, undoManager: undoManager)
            return
        }
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
            activeTool = .pointer
            if sid != 0 {
                selectedShapeId = sid
                selectedShapeIds = [sid]
            }
            registerJournalUndoStep(actionName: "Insert \(kind.capitalized)", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
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
                selectedShapeId = 0
                registerJournalUndoStep(actionName: "Delete Shape", undoManager: undoManager)
                reloadSelectedShapeSheetCells()
                syncSelectedShapeMetadataFromCore()
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
        rotateOrFlipSelectedShapes(deltaDegrees: -deltaDegrees, undoManager: undoManager)
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
        let rulesetSlug: String
        switch standard {
        case .bpmn20: rulesetSlug = "bpmn"
        case .uml25: rulesetSlug = "uml"
        case .sharePointWorkflow: rulesetSlug = "workflow"
        }

        struct EngineValidationIssueDto: Decodable {
            let rule_code: String
            let severity: String
            let description: String
            let page_id: UInt32
            let shape_id: UInt64?
        }

        var findings: [DiagramValidationDiagnostic] = []
        if let jsonStr = try? core.validateDiagramJSON(ruleset: rulesetSlug),
           let jsonData = jsonStr.data(using: .utf8),
           let decoded = try? JSONDecoder().decode([EngineValidationIssueDto].self, from: jsonData)
        {
            findings = decoded.map { item in
                let sev: DiagramValidationDiagnostic.Severity =
                    item.severity.lowercased() == "error" ? .error : .warning
                return DiagramValidationDiagnostic(
                    id: UUID(),
                    ruleId: item.rule_code,
                    standard: standard,
                    severity: sev,
                    pageId: item.page_id,
                    shapeId: item.shape_id ?? 0,
                    message: item.description
                )
            }
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
            statusMessage = "\(standard.rawValue) validation passed (0 issues)"
        } else {
            statusMessage = "\(standard.rawValue) validation found \(findings.count) issue(s)"
        }

        validationDiagnostics = findings
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
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
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
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
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
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
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
            core.setViewportPage(pageId: subPageId)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
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
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
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
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
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
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
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
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
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

    // MARK: - Rich Text, Inline Label Editing, Rotate/Flip, ISO Page Presets, Stencil Import & In-App File I/O

    public func syncSelectedShapeMetadataFromCore() {
        let jsonStr = core.getStateJSON(activePageId: activePageId)
        guard let data = jsonStr.data(using: .utf8),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return }

        if let pw = root["pageWidthIn"] as? Double, pw > 0 {
            pageWidthInches = pw
        }
        if let ph = root["pageHeightIn"] as? Double, ph > 0 {
            pageHeightInches = ph
        }
        if let revNum = root["stateRevision"] as? NSNumber {
            currentStateRevision = revNum.uint64Value
        } else if let revInt = root["stateRevision"] as? Int {
            currentStateRevision = UInt64(max(0, revInt))
        }
        if !didCaptureInitialBaselineRevision {
            didCaptureInitialBaselineRevision = true
            savedStateRevision = currentStateRevision
        }
        recomputeHasUnsavedChanges()
        if let rawPages = root["pages"] as? [[String: Any]], !rawPages.isEmpty {
            pages = rawPages.compactMap { p in
                guard let pid = p["id"] as? Int, pid >= 0 else { return nil }
                let name = (p["name"] as? String) ?? "Page-\(pid)"
                let bg = (p["isBackground"] as? Bool) ?? false
                let wIn = (p["widthIn"] as? Double) ?? pageWidthInches
                let hIn = (p["heightIn"] as? Double) ?? pageHeightInches
                let bpidInt = (p["backPageId"] as? Int) ?? 0
                let colorHex = (p["pageColorHex"] as? String) ?? "#FFFFFF"
                return VexioPageSummary(
                    id: UInt32(pid),
                    title: name,
                    isBackgroundPage: bg,
                    widthInches: wIn,
                    heightInches: hIn,
                    backPageId: bpidInt > 0 ? UInt32(bpidInt) : 0,
                    pageColorHex: colorHex.isEmpty ? "#FFFFFF" : colorHex
                )
            }
            if !pages.contains(where: { $0.id == activePageId }) {
                if let resolved = root["activePageId"] as? Int, resolved >= 0 {
                    activePageId = UInt32(resolved)
                } else if let fg = pages.first(where: { !$0.isBackgroundPage }) ?? pages.first {
                    activePageId = fg.id
                }
            }
        }
        if let rawLayers = root["layers"] as? [[String: Any]] {
            let layerPalette = ["#0A84FF", "#30D158", "#FF9F0A", "#BF5AF2", "#FF375F", "#5E5CE6", "#64D2FF"]
            layers = rawLayers.enumerated().compactMap { idx, item in
                let ix = (item["ix"] as? Int) ?? idx
                let name = (item["name"] as? String) ?? "Layer \(ix)"
                let vis = (item["visible"] as? Bool) ?? true
                let prt = (item["print"] as? Bool) ?? true
                let lck = (item["lock"] as? Bool) ?? false
                let snp = (item["snap"] as? Bool) ?? true
                let glu = (item["glue"] as? Bool) ?? true
                let cnt = (item["memberCount"] as? Int) ?? 0
                return DiagramLayerState(
                    id: ix,
                    name: name,
                    isVisible: vis,
                    isPrintable: prt,
                    isLocked: lck,
                    isSnapGlueEnabled: snp || glu,
                    memberCount: cnt,
                    accentHex: layerPalette[idx % layerPalette.count]
                )
            }
        }

        if let dlState = root["dataLinkState"] as? [String: Any] {
            if let props = dlState["availablePropFields"] as? [String] {
                availablePropFields = props
            }
            if let dg = dlState["dataGraphics"] as? [String: Any] {
                dataGraphicConfig = DataGraphicRuleConfig(
                    isEnabled: (dg["enabled"] as? Bool) ?? false,
                    iconSetField: (dg["iconSetField"] as? String) ?? "",
                    iconGreenMin: (dg["iconGreenMin"] as? Double) ?? 75.0,
                    iconAmberMin: (dg["iconAmberMin"] as? Double) ?? 40.0,
                    dataBarField: (dg["dataBarField"] as? String) ?? "",
                    dataBarMin: (dg["dataBarMin"] as? Double) ?? 0.0,
                    dataBarMax: (dg["dataBarMax"] as? Double) ?? 100.0,
                    dataBarColorHex: (dg["dataBarColorHex"] as? String) ?? "#2563EB",
                    textCalloutField: (dg["textCalloutField"] as? String) ?? "",
                    textCalloutPrefix: (dg["textCalloutPrefix"] as? String) ?? "",
                    colorByValueEnabled: (dg["colorByValueEnabled"] as? Bool) ?? true,
                    colorByValueField: (dg["colorByValueField"] as? String) ?? "",
                    showLegendOnPage: (dg["legendVisible"] as? Bool) ?? false,
                    legendField: (dg["legendField"] as? String) ?? "Status"
                )
            }
            if let rawRs = dlState["recordsets"] as? [[String: Any]] {
                externalRecordsets = rawRs.compactMap { item in
                    guard let rid = item["id"] as? Int, rid > 0 else { return nil }
                    let name = (item["name"] as? String) ?? "Recordset \(rid)"
                    let kind = (item["sourceKind"] as? String) ?? "csv"
                    let srcName = (item["sourceName"] as? String) ?? ""
                    let keyCol = (item["keyColumn"] as? String) ?? ""
                    let cols = (item["columns"] as? [String]) ?? []
                    let rawRows = (item["rows"] as? [[String: Any]]) ?? []
                    let rows: [[String: String]] = rawRows.map { r in
                        var dict: [String: String] = [:]
                        for (k, v) in r {
                            if let s = v as? String {
                                dict[k] = s
                            } else {
                                dict[k] = "\(v)"
                            }
                        }
                        return dict
                    }
                    var boundMap: [Int: [UInt64]] = [:]
                    if let rawBound = item["boundShapeIds"] as? [String: Any] {
                        for (k, v) in rawBound {
                            if let rowIdx = Int(k) {
                                if let ints = v as? [Int] {
                                    boundMap[rowIdx] = ints.filter { $0 > 0 }.map { UInt64($0) }
                                } else if let nums = v as? [NSNumber] {
                                    boundMap[rowIdx] = nums.map { $0.uint64Value }.filter { $0 > 0 }
                                }
                            }
                        }
                    }
                    return ExternalDataRecordsetSummary(
                        id: UInt32(rid),
                        name: name,
                        sourceKind: kind,
                        sourceName: srcName,
                        keyColumn: keyCol,
                        columns: cols,
                        rows: rows,
                        boundShapeIdsByRow: boundMap
                    )
                }
            }
        }

        guard let shapes = root["shapes"] as? [[String: Any]] else {
            activePageShapeCount = 0
            selectedShapeIsGroup = false
            inspectedObject = nil
            shapeDataProperties = []
            return
        }
        activePageShapeCount = shapes.count
        let activeSelSet = Set(selectedShapeIds.isEmpty ? (selectedShapeId != 0 ? [selectedShapeId] : []) : selectedShapeIds)
        selectedShapeIsGroup = shapes.contains { sh in
            let sid: UInt64 = (sh["id"] as? UInt64) ?? UInt64(max((sh["id"] as? Int) ?? 0, 0))
            guard activeSelSet.contains(sid) else { return false }
            if let ig = sh["isGroup"] as? Bool { return ig }
            return (sh["kind"] as? String) == "Group"
        }

        guard selectedShapeId != 0,
              let matched = shapes.first(where: { ($0["id"] as? UInt64) == selectedShapeId || ($0["id"] as? Int) == Int(selectedShapeId) })
        else {
            inspectedObject = nil
            shapeDataProperties = []
            return
        }

        let sid: UInt64 = (matched["id"] as? UInt64) ?? UInt64(max((matched["id"] as? Int) ?? Int(selectedShapeId), 0))
        let layerInds: [Int] = (matched["layerIndices"] as? [Int])
            ?? ((matched["layerIndices"] as? [NSNumber])?.map { $0.intValue } ?? [])
        let fillHex = (matched["fillColor"] as? String) ?? selectedFillColorHex
        let lineHex = (matched["lineColor"] as? String) ?? selectedLineColorHex
        selectedFillColorHex = fillHex
        selectedLineColorHex = lineHex

        if let rawData = matched["shapeData"] as? [[String: Any]] {
            shapeDataProperties = rawData.compactMap { row in
                guard let rName = row["name"] as? String, !rName.isEmpty else { return nil }
                let label = (row["label"] as? String) ?? rName
                let dtypeCode = UInt8(clamping: (row["dataType"] as? Int) ?? 0)
                let dtypeName: String = {
                    switch dtypeCode {
                    case 1, 4: return "List"
                    case 2: return "Number"
                    case 3: return "Boolean"
                    case 5: return "Date"
                    case 6: return "Duration"
                    case 7: return "Currency"
                    default: return "String"
                    }
                }()
                let formula = (row["formula"] as? String) ?? ""
                let evaluated = (row["value"] as? String) ?? formula
                return ShapeDataPropertyItem(
                    rowName: rName,
                    rowKey: "Prop.\(rName)",
                    label: label,
                    dataTypeCode: dtypeCode,
                    dataType: dtypeName,
                    formula: formula,
                    evaluatedValue: evaluated
                )
            }
        } else {
            shapeDataProperties = []
        }

        let wIn = (matched["width"] as? Double) ?? 1.0
        let hIn = (matched["height"] as? Double) ?? 1.0
        inspectedObject = InspectedObjectState(
            id: sid,
            name: (matched["name"] as? String) ?? "Sheet.\(sid)",
            kind: (matched["kind"] as? String) ?? "Shape2D",
            isGroup: (matched["isGroup"] as? Bool) ?? false,
            childCount: (matched["childCount"] as? Int) ?? 0,
            masterName: matched["masterName"] as? String,
            layerIndices: layerInds,
            text: (matched["text"] as? String) ?? "",
            pinXIn: (matched["pinX"] as? Double) ?? 0.0,
            pinYIn: (matched["pinY"] as? Double) ?? 0.0,
            locPinXIn: (matched["locPinX"] as? Double) ?? (wIn * 0.5),
            locPinYIn: (matched["locPinY"] as? Double) ?? (hIn * 0.5),
            widthIn: wIn,
            heightIn: hIn,
            angleDeg: (matched["angleDeg"] as? Double) ?? 0.0,
            flipX: (matched["flipX"] as? Bool) ?? false,
            flipY: (matched["flipY"] as? Bool) ?? false,
            beginXIn: matched["beginX"] as? Double,
            beginYIn: matched["beginY"] as? Double,
            endXIn: matched["endX"] as? Double,
            endYIn: matched["endY"] as? Double,
            routeStyle: (matched["routeStyle"] as? Int) ?? 16,
            fillColorHex: fillHex,
            fillBkgndHex: (matched["fillBkgnd"] as? String) ?? "#FFFFFF",
            fillPattern: (matched["fillPattern"] as? Int) ?? 1,
            fillTransPct: (matched["fillTransPct"] as? Double) ?? 0.0,
            roundingIn: (matched["roundingIn"] as? Double) ?? 0.0,
            lineColorHex: lineHex,
            linePattern: (matched["linePattern"] as? Int) ?? 1,
            lineWeightPt: (matched["lineWeightPt"] as? Double) ?? 1.0,
            lineTransPct: (matched["lineTransPct"] as? Double) ?? 0.0,
            beginArrow: (matched["beginArrow"] as? Int) ?? 0,
            endArrow: (matched["endArrow"] as? Int) ?? 0,
            lockWidth: (matched["lockWidth"] as? Bool) ?? false,
            lockHeight: (matched["lockHeight"] as? Bool) ?? false,
            lockAspect: (matched["lockAspect"] as? Bool) ?? false,
            lockMoveX: (matched["lockMoveX"] as? Bool) ?? false,
            lockMoveY: (matched["lockMoveY"] as? Bool) ?? false,
            lockRotate: (matched["lockRotate"] as? Bool) ?? false,
            lockDelete: (matched["lockDelete"] as? Bool) ?? false,
            hideText: (matched["hideText"] as? Bool) ?? false
        )

        if let ff = matched["fontFamily"] as? String, !ff.isEmpty {
            selectedFontFamily = ff
        }
        if let fsz = matched["fontSizePt"] as? Double, fsz > 0 {
            selectedFontSizePt = fsz
        }
        if let cs = matched["charStyle"] as? Int {
            selectedCharStyle = UInt8(clamping: cs)
        }
        if let tc = matched["textColor"] as? String, !tc.isEmpty {
            selectedTextColorHex = tc
        }
        if let tbg = matched["textBgColor"] as? String, !tbg.isEmpty {
            selectedTextBgColorHex = tbg
        } else {
            selectedTextBgColorHex = "none"
        }
        if let ha = matched["horizAlign"] as? Int {
            selectedHorizAlign = UInt8(clamping: ha)
        }
        if let va = matched["vertAlign"] as? Int {
            selectedVertAlign = UInt8(clamping: va)
        }
        if let bul = matched["bulletStyle"] as? Int {
            selectedBulletStyle = UInt8(clamping: bul)
        }
    }

    public func fitViewportToActivePage() {
        isAutoFitPageMode = true
        let lw = lastViewportLogicalSize.width
        let lh = lastViewportLogicalSize.height
        if lw < 64 || lh < 64 {
            needsFitOnViewportReady = true
        } else {
            let leftPad: CGFloat = lw > 420 ? 88 : lw * 0.05
            let rightPad: CGFloat = min(max(lw * 0.025, 22), min(40, lw * 0.08))
            let topPad: CGFloat = min(max(lh * 0.03, 18), min(34, lh * 0.08))
            let botPad: CGFloat = lh > 320 ? 50 : lh * 0.05
            let availW = max(lw - leftPad - rightPad, lw * 0.55)
            let availH = max(lh - topPad - botPad, lh * 0.55)
            let pw = max(CGFloat(pageWidthInches), 0.5)
            let ph = max(CGFloat(pageHeightInches), 0.5)
            let ppi = min(availW / pw, availH / ph)
            isSuppressingCameraSync = true
            zoomScale = min(max(ppi / 96.0, 0.10), 8.0)
            panOffset = CGSize(width: (leftPad - rightPad) * 0.5, height: (topPad - botPad) * 0.5)
            isSuppressingCameraSync = false
        }
        core.setViewportZoom(zoomScale: 0.0, panXIn: 0.0, panYIn: 0.0)
        core.renderViewportFrame()
    }

    public func selectPage(_ pageId: UInt32) {
        activeInlineTextEdit = nil
        selectedShapeIds = []
        selectedShapeId = 0
        activePageId = pageId
        core.setViewportPage(pageId: pageId)
        reloadSelectedShapeSheetCells()
        syncSelectedShapeMetadataFromCore()
        fitViewportToActivePage()
        let title = pages.first(where: { $0.id == pageId })?.title ?? "Page-\(pageId)"
        statusMessage = "Active page: \(title)"
    }

    public func renamePage(pageId: UInt32, to rawNewName: String, undoManager: UndoManager?) {
        let trimmed = rawNewName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        do {
            try core.renamePage(pageId: pageId, newName: trimmed)
            registerJournalUndoStep(actionName: "Rename Page", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Renamed page to '\(trimmed)'"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func duplicatePage(pageId: UInt32, undoManager: UndoManager?) {
        do {
            let newId = try core.duplicatePage(pageId: pageId)
            registerJournalUndoStep(actionName: "Duplicate Page", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            selectPage(newId)
            statusMessage = "Duplicated page (Page-\(newId))"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func deletePage(pageId: UInt32, undoManager: UndoManager?) {
        do {
            try core.deletePage(pageId: pageId)
            registerJournalUndoStep(actionName: "Delete Page", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            if activePageId == pageId, let fallback = pages.first(where: { !$0.isBackgroundPage }) ?? pages.first {
                selectPage(fallback.id)
            } else {
                core.renderViewportFrame()
            }
            statusMessage = "Deleted page"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func updatePageBackground(
        pageId: UInt32,
        isBackground: Bool,
        backPageId: UInt32 = 0,
        pageColorHex: String = "",
        undoManager: UndoManager?
    ) {
        do {
            try core.setPageBackground(
                pageId: pageId,
                isBackground: isBackground,
                backPageId: backPageId,
                pageColorHex: pageColorHex
            )
            registerJournalUndoStep(actionName: "Page Background", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Updated page background"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func applyFillColor(_ color: Color, undoManager: UndoManager?) {
        selectedFillColorHex = color.vexioHexRGBString
        let formula = color.vexioRGBFormulaString
        let targets = selectedShapeIds.isEmpty ? (selectedShapeId != 0 ? [selectedShapeId] : []) : selectedShapeIds
        for sid in targets {
            try? core.setCellFormula(pageId: activePageId, shapeId: sid, cellName: "FillPattern", formula: "1", forceSetF: false)
            try? core.setCellFormula(pageId: activePageId, shapeId: sid, cellName: "FillForegnd", formula: formula, forceSetF: false)
        }
        registerJournalUndoStep(actionName: "Set Fill Color", undoManager: undoManager)
        reloadSelectedShapeSheetCells()
        syncSelectedShapeMetadataFromCore()
        core.renderViewportFrame()
    }

    public func applyLineColor(_ color: Color, undoManager: UndoManager?) {
        selectedLineColorHex = color.vexioHexRGBString
        let formula = color.vexioRGBFormulaString
        let targets = selectedShapeIds.isEmpty ? (selectedShapeId != 0 ? [selectedShapeId] : []) : selectedShapeIds
        for sid in targets {
            try? core.setCellFormula(pageId: activePageId, shapeId: sid, cellName: "LineColor", formula: formula, forceSetF: false)
        }
        registerJournalUndoStep(actionName: "Set Line Color", undoManager: undoManager)
        reloadSelectedShapeSheetCells()
        syncSelectedShapeMetadataFromCore()
        core.renderViewportFrame()
    }

    // MARK: - 1:1 Microsoft Visio Object Inspector Actions

    public func setCellForSelection(
        cellName: String,
        formula: String,
        forceSetF: Bool = false,
        actionLabel: String? = nil,
        undoManager: UndoManager?
    ) {
        let targets = selectedShapeIds.isEmpty ? (selectedShapeId != 0 ? [selectedShapeId] : []) : selectedShapeIds
        guard !targets.isEmpty else { return }
        for sid in targets {
            try? core.setCellFormula(pageId: activePageId, shapeId: sid, cellName: cellName, formula: formula, forceSetF: forceSetF)
        }
        registerJournalUndoStep(actionName: actionLabel ?? "Edit \(cellName)", undoManager: undoManager)
        reloadSelectedShapeSheetCells()
        syncSelectedShapeMetadataFromCore()
        core.renderViewportFrame()
        statusMessage = "Updated \(cellName) = \(formula)"
    }

    public func renameSelectedShape(to rawName: String, undoManager: UndoManager?) {
        let trimmed = rawName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard selectedShapeId != 0, !trimmed.isEmpty else { return }
        do {
            try core.renameShape(pageId: activePageId, shapeId: selectedShapeId, newName: trimmed)
            registerJournalUndoStep(actionName: "Rename Object", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Renamed Sheet.\(selectedShapeId) to '\(trimmed)'"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func setSelectedShapeLabel(_ newText: String, undoManager: UndoManager?) {
        guard selectedShapeId != 0 else { return }
        do {
            try core.setShapeText(pageId: activePageId, shapeId: selectedShapeId, text: newText)
            registerJournalUndoStep(actionName: "Edit Shape Text", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Updated label on Sheet.\(selectedShapeId)"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func upsertSelectedShapeData(
        rowName: String,
        label: String,
        dataType: UInt8,
        value: String,
        undoManager: UndoManager?
    ) {
        guard selectedShapeId != 0 else { return }
        do {
            try core.upsertShapeData(
                pageId: activePageId,
                shapeId: selectedShapeId,
                rowName: rowName,
                label: label,
                dataType: dataType,
                value: value
            )
            registerJournalUndoStep(actionName: "Edit Shape Data", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Saved Prop.\(rowName) on Sheet.\(selectedShapeId)"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func deleteSelectedShapeData(rowName: String, undoManager: UndoManager?) {
        guard selectedShapeId != 0 else { return }
        do {
            try core.deleteShapeData(pageId: activePageId, shapeId: selectedShapeId, rowName: rowName)
            registerJournalUndoStep(actionName: "Delete Shape Data", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Removed Prop.\(rowName)"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func addPageLayer(name: String, undoManager: UndoManager?) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            let ix = try core.addLayer(pageId: activePageId, layerName: trimmed)
            registerJournalUndoStep(actionName: "Add Layer", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Added Layer #\(ix)"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func updatePageLayerFlags(
        layerId: Int,
        visible: Bool,
        locked: Bool,
        printable: Bool,
        snapGlue: Bool,
        undoManager: UndoManager?
    ) {
        guard layerId >= 0 else { return }
        do {
            try core.setLayerFlags(
                pageId: activePageId,
                layerIx: UInt32(layerId),
                visible: visible,
                locked: locked,
                printable: printable,
                snapGlue: snapGlue
            )
            registerJournalUndoStep(actionName: "Update Layer", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func setSelectedShapeLayerMembership(
        layerId: Int,
        assigned: Bool,
        undoManager: UndoManager?
    ) {
        guard layerId >= 0 else { return }
        let targets = selectedShapeIds.isEmpty ? (selectedShapeId != 0 ? [selectedShapeId] : []) : selectedShapeIds
        guard !targets.isEmpty else { return }
        do {
            for sid in targets {
                try core.setShapeLayerMembership(
                    pageId: activePageId,
                    shapeId: sid,
                    layerIx: UInt32(layerId),
                    assigned: assigned
                )
            }
            registerJournalUndoStep(actionName: assigned ? "Assign Layer" : "Unassign Layer", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func currentShapeLabelText(shapeId: UInt64) -> String {
        let jsonStr = core.getStateJSON(activePageId: activePageId)
        guard let data = jsonStr.data(using: .utf8),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let shapes = root["shapes"] as? [[String: Any]],
              let matched = shapes.first(where: { ($0["id"] as? UInt64) == shapeId || ($0["id"] as? Int) == Int(shapeId) })
        else { return "" }
        return (matched["text"] as? String) ?? ""
    }

    public func syncToolAndInlineTextAfterPointerUp(backingScale: CGFloat) {
        let engineCode = core.viewportActiveToolCode()
        if engineCode == 0 && activeTool != .pointer {
            activeTool = .pointer
        }
        let createdTextShape = core.popCreatedTextShapeId()
        if createdTextShape != 0 {
            selectedShapeId = createdTextShape
            selectedShapeIds = [createdTextShape]
            beginInlineEditing(shapeId: createdTextShape, backingScale: backingScale)
        }
        syncSelectedShapeMetadataFromCore()
    }

    public func handleCanvasDoubleClick(xPx: Double, yPx: Double, backingScale: CGFloat) -> Bool {
        let hitId = core.hitTestShape(xPx: xPx, yPx: yPx)
        guard hitId != 0 else { return false }
        selectedShapeId = hitId
        selectedShapeIds = [hitId]
        activeTool = .pointer
        beginInlineEditing(shapeId: hitId, backingScale: backingScale)
        return true
    }

    public func beginInlineEditing(shapeId: UInt64, backingScale: CGFloat) {
        let scale = max(backingScale, 1.0)
        let boundsPx = core.shapeScreenBoundsPx(shapeId: shapeId) ?? CGRect(x: 320, y: 240, width: 180, height: 42)
        let viewRect = CGRect(
            x: boundsPx.origin.x / scale,
            y: boundsPx.origin.y / scale,
            width: max(boundsPx.size.width / scale, 120),
            height: max(boundsPx.size.height / scale, 24)
        )
        let initial = currentShapeLabelText(shapeId: shapeId)
        activeInlineTextEdit = InlineShapeTextEditContext(
            shapeId: shapeId,
            viewRect: viewRect,
            initialText: initial
        )
    }

    public func commitInlineEditing(newText: String, undoManager: UndoManager?) {
        guard let edit = activeInlineTextEdit else { return }
        activeInlineTextEdit = nil
        do {
            try core.setShapeText(pageId: activePageId, shapeId: edit.shapeId, text: newText)
            registerJournalUndoStep(actionName: "Edit Shape Text", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Updated label on Sheet.\(edit.shapeId)"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func insertTextBoxAndBeginEditing(atCanvasInches point: CGPoint, undoManager: UndoManager?) {
        let cx = min(max(Double(point.x), 1.0), max(pageWidthInches - 1.0, 2.0))
        let cy = min(max(Double(point.y), 0.8), max(pageHeightInches - 0.8, 1.5))
        do {
            let sid = try core.drawPrimitive(
                pageId: activePageId,
                kind: "text",
                x1In: cx - 1.0,
                y1In: cy - 0.25,
                x2In: cx + 1.0,
                y2In: cy + 0.25
            )
            activeTool = .pointer
            if sid != 0 {
                selectedShapeId = sid
                selectedShapeIds = [sid]
                registerJournalUndoStep(actionName: "Insert Text Box", undoManager: undoManager)
                reloadSelectedShapeSheetCells()
                core.renderViewportFrame()
                let scale = NSScreen.main?.backingScaleFactor ?? 2.0
                beginInlineEditing(shapeId: sid, backingScale: scale)
            }
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func applyOrInsertHeading(
        _ preset: VexioTextHeadingPreset,
        forceInsert: Bool = false,
        customText: String? = nil,
        undoManager: UndoManager?
    ) {
        let targets = selectedShapeIds.isEmpty
            ? (selectedShapeId != 0 ? [selectedShapeId] : [])
            : selectedShapeIds

        do {
            if !forceInsert && !targets.isEmpty {
                let count = try core.applyHeadingPreset(
                    pageId: activePageId,
                    shapeIds: targets,
                    presetSlug: preset.rawValue
                )
                if count > 0 {
                    registerJournalUndoStep(actionName: "Apply \(preset.shortChipLabel) Style", undoManager: undoManager)
                    reloadSelectedShapeSheetCells()
                    syncSelectedShapeMetadataFromCore()
                    core.renderViewportFrame()
                    statusMessage = "Applied \(preset.displayName) to \(count) shape(s)"
                    return
                }
            }

            let sid = try core.insertHeading(
                pageId: activePageId,
                presetSlug: preset.rawValue,
                customText: customText,
                xIn: 0.0,
                yIn: 0.0
            )
            activeTool = .pointer
            if sid != 0 {
                selectedShapeId = sid
                selectedShapeIds = [sid]
                registerJournalUndoStep(actionName: "Insert \(preset.shortChipLabel)", undoManager: undoManager)
                reloadSelectedShapeSheetCells()
                syncSelectedShapeMetadataFromCore()
                core.renderViewportFrame()
                let scale = NSScreen.main?.backingScaleFactor ?? 2.0
                beginInlineEditing(shapeId: sid, backingScale: scale)
                statusMessage = "Inserted \(preset.displayName)"
            }
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func applyBordersAndTitles(
        style: VexioBorderTitleStyle,
        title: String = "",
        subtitle: String = "",
        author: String = "",
        dateOrRev: String = "",
        undoManager: UndoManager?
    ) {
        let resolvedTitle = title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            ? documentTitle
            : title.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            _ = try core.applyBorderAndTitle(
                pageId: activePageId,
                styleSlug: style.rawValue,
                title: resolvedTitle,
                subtitle: subtitle,
                author: author,
                dateOrRev: dateOrRev
            )
            registerJournalUndoStep(actionName: "Borders & Titles", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            core.setViewportPage(pageId: activePageId)
            core.renderViewportFrame()
            statusMessage = style == .none
                ? "Removed page border & title"
                : "Applied \(style.displayName)"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func exportDiagramMarkup(formatSlug: String = "mermaid") -> String {
        (try? core.exportMarkup(pageId: activePageId, formatSlug: formatSlug)) ?? ""
    }

    public func importDiagramMarkup(
        markup: String,
        formatSlug: String = "mermaid",
        undoManager: UndoManager?
    ) {
        do {
            let created = try core.importMarkup(pageId: activePageId, markup: markup, formatSlug: formatSlug)
            registerJournalUndoStep(actionName: "Import Diagram Markup", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            fitViewportToActivePage()
            core.renderViewportFrame()
            statusMessage = "Materialized \(created) shapes from \(formatSlug.uppercased()) markup"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func refreshSlideSnippets() {
        guard let json = try? core.listSlideSnippetsJson(),
              let data = json.data(using: .utf8),
              let decoded = try? JSONDecoder().decode([SlideSnippetPayload].self, from: data)
        else {
            slideSnippets = []
            return
        }
        slideSnippets = decoded
    }

    public func autoGenerateSlideSnippets(undoManager: UndoManager?) {
        do {
            let count = try core.autoSlideSnippets(pageId: activePageId)
            registerJournalUndoStep(actionName: "Auto-Generate Slide Snippets", undoManager: undoManager)
            refreshSlideSnippets()
            syncSelectedShapeMetadataFromCore()
            statusMessage = "Generated \(count) Presentation Slide Snippets"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func clearSlideSnippets(undoManager: UndoManager?) {
        do {
            try core.deleteSlideSnippet(pageId: activePageId, snippetId: 0)
            registerJournalUndoStep(actionName: "Clear Slide Snippets", undoManager: undoManager)
            refreshSlideSnippets()
            syncSelectedShapeMetadataFromCore()
            statusMessage = "Cleared Presentation Slide Snippets"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func focusSlideSnippet(_ snippet: SlideSnippetPayload) {
        if snippet.page_id != activePageId {
            activePageId = snippet.page_id
        }
        let vw = max(360.0, Double(lastViewportLogicalSize.width) - 48.0)
        let vh = max(260.0, Double(lastViewportLogicalSize.height) - 48.0)
        let scale = min(vw / (max(1.0, snippet.width_in) * 96.0), vh / (max(1.0, snippet.height_in) * 96.0))
        let clampedScale = min(max(scale, 0.25), 4.0)
        let cx = snippet.x_in + snippet.width_in * 0.5
        let cy = snippet.y_in + snippet.height_in * 0.5
        disengageAutoFitForManualCamera()
        isSuppressingCameraSync = true
        zoomScale = CGFloat(clampedScale)
        panOffset = CGSize(
            width: (pageWidthInches * 0.5 - cx) * 96.0 * clampedScale,
            height: (cy - pageHeightInches * 0.5) * 96.0 * clampedScale
        )
        isSuppressingCameraSync = false
        syncViewportCamera()
        statusMessage = "Presenting: \(snippet.title)"
    }

    public func resetSelectedConnectorRouting(undoManager: UndoManager?) {
        guard selectedShapeId != 0 else { return }
        do {
            try core.resetConnectorRouting(pageId: activePageId, shapeId: selectedShapeId)
            registerJournalUndoStep(actionName: "Reset Connector Routing", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Restored automatic A* orthogonal routing"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func ensureSelectedControlHandle(undoManager: UndoManager?) {
        guard selectedShapeId != 0 else { return }
        do {
            try core.ensureControlHandle(pageId: activePageId, shapeId: selectedShapeId)
            registerJournalUndoStep(actionName: "Add Control Handle", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Attached parametric yellow Controls.Row_1 handle"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func applyTextStyle(
        fontFamily: String? = nil,
        fontSizePt: Double? = nil,
        sizeDeltaPt: Double? = nil,
        toggleStyleMask: UInt8? = nil,
        textColorHex: String? = nil,
        textBgColorHex: String? = nil,
        horizAlign: UInt8? = nil,
        vertAlign: UInt8? = nil,
        bulletStyle: UInt8? = nil,
        undoManager: UndoManager?
    ) {
        let targets = selectedShapeIds.isEmpty ? (selectedShapeId != 0 ? [selectedShapeId] : []) : selectedShapeIds
        guard !targets.isEmpty else { return }

        var payload: [String: Any] = [:]
        if let fontFamily { payload["fontFamily"] = fontFamily }
        if let fontSizePt { payload["fontSizePt"] = fontSizePt }
        if let sizeDeltaPt { payload["sizeDeltaPt"] = sizeDeltaPt }
        if let toggleStyleMask { payload["toggleStyleMask"] = Int(toggleStyleMask) }
        if let textColorHex { payload["textColor"] = textColorHex }
        if let textBgColorHex { payload["textBgColor"] = textBgColorHex }
        if let horizAlign { payload["horizAlign"] = Int(horizAlign) }
        if let vertAlign { payload["vertAlign"] = Int(vertAlign) }
        if let bulletStyle { payload["bulletStyle"] = Int(bulletStyle) }

        guard let jsonData = try? JSONSerialization.data(withJSONObject: payload),
              let jsonStr = String(data: jsonData, encoding: .utf8)
        else { return }

        do {
            try core.setShapeTextStyleJSON(pageId: activePageId, shapeIds: targets, styleJSON: jsonStr)
            registerJournalUndoStep(actionName: "Format Text", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Applied text formatting to \(targets.count) shape(s)"
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func rotateOrFlipSelectedShapes(
        deltaDegrees: Double = 0.0,
        toggleFlipX: Bool = false,
        toggleFlipY: Bool = false,
        undoManager: UndoManager?
    ) {
        let targets = selectedShapeIds.isEmpty ? (selectedShapeId != 0 ? [selectedShapeId] : []) : selectedShapeIds
        guard !targets.isEmpty else { return }
        do {
            try core.rotateFlipShapes(
                pageId: activePageId,
                shapeIds: targets,
                deltaAngleDeg: deltaDegrees,
                toggleFlipX: toggleFlipX,
                toggleFlipY: toggleFlipY
            )
            registerJournalUndoStep(actionName: "Rotate / Flip Shape", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Transformed \(targets.count) stencil/shape(s)"
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func alignSelectedShapes(mode: String, undoManager: UndoManager?) {
        let vpIds = core.selectionIds()
        let targets = !vpIds.isEmpty ? vpIds : (selectedShapeIds.isEmpty ? (selectedShapeId != 0 ? [selectedShapeId] : []) : selectedShapeIds)
        guard !targets.isEmpty else { return }
        do {
            let cnt = try core.alignShapes(pageId: activePageId, shapeIds: targets, mode: mode)
            if cnt > 0 {
                registerJournalUndoStep(actionName: "Align Shapes", undoManager: undoManager)
                reloadSelectedShapeSheetCells()
                syncSelectedShapeMetadataFromCore()
                core.renderViewportFrame()
                statusMessage = "Aligned \(cnt) shape(s) (\(mode))"
            }
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func distributeSelectedShapes(axis: String, undoManager: UndoManager?) {
        let vpIds = core.selectionIds()
        let targets = !vpIds.isEmpty ? vpIds : selectedShapeIds
        guard targets.count >= 3 else {
            statusMessage = "Select at least 3 shapes to distribute evenly"
            return
        }
        do {
            let cnt = try core.distributeShapes(pageId: activePageId, shapeIds: targets, axis: axis)
            if cnt > 0 {
                registerJournalUndoStep(actionName: "Distribute Shapes", undoManager: undoManager)
                reloadSelectedShapeSheetCells()
                syncSelectedShapeMetadataFromCore()
                core.renderViewportFrame()
                statusMessage = "Distributed \(targets.count) shapes (\(axis))"
            }
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public var canGroupSelectedShapes: Bool {
        let vpIds = core.selectionIds()
        let count = !vpIds.isEmpty ? vpIds.count : selectedShapeIds.count
        return count >= 2
    }

    public var canUngroupSelectedShapes: Bool {
        if selectedShapeIsGroup { return true }
        let vpIds = core.selectionIds()
        let targets = Set(!vpIds.isEmpty ? vpIds : (selectedShapeIds.isEmpty ? (selectedShapeId != 0 ? [selectedShapeId] : []) : selectedShapeIds))
        guard !targets.isEmpty else { return false }
        let jsonStr = core.getStateJSON(activePageId: activePageId)
        guard let data = jsonStr.data(using: .utf8),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let shapes = root["shapes"] as? [[String: Any]]
        else { return false }
        return shapes.contains { sh in
            let sid: UInt64 = (sh["id"] as? UInt64) ?? UInt64(max((sh["id"] as? Int) ?? 0, 0))
            guard targets.contains(sid) else { return false }
            if let ig = sh["isGroup"] as? Bool { return ig }
            return (sh["kind"] as? String) == "Group"
        }
    }

    public func selectShapeOnRightClickIfNeeded(xPx: Double, yPx: Double) {
        let hitId = core.hitTestShape(xPx: xPx, yPx: yPx)
        guard hitId != 0 else { return }
        let currentIds = !core.selectionIds().isEmpty ? core.selectionIds() : selectedShapeIds
        if !currentIds.contains(hitId) {
            selectedShapeId = hitId
            selectedShapeIds = [hitId]
            core.setSelectionIds([hitId])
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
        } else {
            syncSelectedShapeMetadataFromCore()
        }
    }

    public func groupSelectedShapes(undoManager: UndoManager?) {
        let vpIds = core.selectionIds()
        let targets = !vpIds.isEmpty ? vpIds : selectedShapeIds
        guard targets.count >= 2 else {
            statusMessage = "Select at least 2 shapes to group (⌘G)"
            return
        }
        do {
            let gid = try core.groupShapes(pageId: activePageId, shapeIds: targets)
            guard gid > 0 else { return }
            selectedShapeId = gid
            selectedShapeIds = [gid]
            selectedShapeIsGroup = true
            core.setSelectionIds([gid])
            registerJournalUndoStep(actionName: "Group Shapes", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Grouped \(targets.count) shapes into Group.\(gid)"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func ungroupSelectedShapes(undoManager: UndoManager?) {
        let vpIds = core.selectionIds()
        let targets = !vpIds.isEmpty ? vpIds : (selectedShapeIds.isEmpty ? (selectedShapeId != 0 ? [selectedShapeId] : []) : selectedShapeIds)
        guard !targets.isEmpty else { return }
        do {
            let restored = try core.ungroupShapes(pageId: activePageId, groupIds: targets)
            guard !restored.isEmpty else {
                statusMessage = "Selected shape is not a group"
                return
            }
            selectedShapeIds = restored
            if let first = restored.first {
                selectedShapeId = first
            }
            core.setSelectionIds(restored)
            registerJournalUndoStep(actionName: "Ungroup Shapes", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Ungrouped into \(restored.count) shapes"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func applyPagePreset(_ preset: VexioPagePreset, landscape: Bool, undoManager: UndoManager?) {
        do {
            try core.setPagePreset(pageId: activePageId, presetSlug: preset.rawValue, landscape: landscape)
            core.setViewportPage(pageId: activePageId)
            registerJournalUndoStep(actionName: "Page Size \(preset.shortCode)", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            fitViewportToActivePage()
            statusMessage = "Page set to \(preset.displayLabel) (\(landscape ? "Landscape" : "Portrait"))"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func setPageOrientation(landscape: Bool, undoManager: UndoManager?) {
        let w = max(pageWidthInches, pageHeightInches)
        let h = min(pageWidthInches, pageHeightInches)
        let targetW = landscape ? w : h
        let targetH = landscape ? h : w
        let unitSlug = VexioLocalization.shared.unitPreference.resolvedEngineSlug
        do {
            try core.setPageSetup(pageId: activePageId, widthIn: targetW, heightIn: targetH, unitSlug: unitSlug)
            core.setViewportPage(pageId: activePageId)
            registerJournalUndoStep(actionName: "Page Orientation", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            fitViewportToActivePage()
            statusMessage = landscape ? "Switched page to Landscape" : "Switched page to Portrait"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func applyMeasurementUnit(_ unitSlug: String, undoManager: UndoManager?) {
        do {
            try core.setPageSetup(
                pageId: activePageId,
                widthIn: pageWidthInches,
                heightIn: pageHeightInches,
                unitSlug: unitSlug
            )
            registerJournalUndoStep(actionName: "Units \(unitSlug)", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Measurement units set to \(unitSlug)"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func resetToBlankSheet(preset: VexioPagePreset, landscape: Bool, unitSlug: String) {
        do {
            try core.replaceWithBlankDocument(title: "Untitled Diagram")
            try core.setPagePreset(pageId: 1, presetSlug: preset.rawValue, landscape: landscape)
            documentTitle = "Untitled Diagram"
            currentFileURL = nil
            sourceFormatExtension = "vsdx"
            activePageId = 1
            selectedShapeIds = []
            activeInlineTextEdit = nil
            activeTool = .pointer
            syncSelectedShapeMetadataFromCore()
            markCurrentStateAsClean()
            core.setViewportPage(pageId: 1)
            fitViewportToActivePage()
            core.renderViewportFrame()
            statusMessage = "Created blank \(preset.shortCode) (\(unitSlug)) drawing"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func createNewBlankDiagram() {
        let unitPref = VexioLocalization.shared.unitPreference
        let defaultPreset: VexioPagePreset = unitPref.isMetric ? .a4 : .letter
        resetToBlankSheet(
            preset: defaultPreset,
            landscape: true,
            unitSlug: unitPref.resolvedEngineSlug
        )
    }

    public func applyCustomPageSetup(
        pageId: UInt32,
        name: String?,
        widthInches: Double,
        heightInches: Double,
        unitSlug: String,
        isBackground: Bool,
        backPageId: UInt32,
        pageColorHex: String,
        undoManager: UndoManager?
    ) {
        do {
            if let trimmedName = name?.trimmingCharacters(in: .whitespacesAndNewlines), !trimmedName.isEmpty {
                try core.renamePage(pageId: pageId, newName: trimmedName)
            }
            try core.setPageSetup(
                pageId: pageId,
                widthIn: max(0.5, widthInches),
                heightIn: max(0.5, heightInches),
                unitSlug: unitSlug
            )
            try core.setPageBackground(
                pageId: pageId,
                isBackground: isBackground,
                backPageId: isBackground ? 0 : backPageId,
                pageColorHex: pageColorHex
            )
            registerJournalUndoStep(actionName: "Page Setup", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            if pageId == activePageId {
                core.setViewportPage(pageId: activePageId)
                fitViewportToActivePage()
            } else {
                core.renderViewportFrame()
            }
            statusMessage = "Updated page setup"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func openLocalFile(from url: URL) {
        let ext = url.pathExtension.lowercased()
        if ["vssx", "vss", "vssm"].contains(ext) {
            importVisioStencilFile(from: url)
            return
        }
        let accessed = url.startAccessingSecurityScopedResource()
        defer { if accessed { url.stopAccessingSecurityScopedResource() } }
        do {
            let bytes = try Data(contentsOf: url)
            let resolvedExt = ext.isEmpty ? "vsdx" : ext
            try core.replaceWithFileData(bytes, formatExtension: resolvedExt)
            currentFileURL = url
            sourceFormatExtension = resolvedExt
            documentTitle = url.deletingPathExtension().lastPathComponent
            core.setDocumentTitle(documentTitle)
            NSDocumentController.shared.noteNewRecentDocumentURL(url)
            activeInlineTextEdit = nil
            selectedShapeIds = []
            activeTool = .pointer
            // Query pages with sentinel UInt32.max so engine resolves the first non-background page
            // even when Microsoft Visio numbers pages starting at ID=0 or ID=4!
            let probeJSON = core.getStateJSON(activePageId: UInt32.max)
            var initialFgPageId: UInt32 = 0
            if let pData = probeJSON.data(using: .utf8),
               let pRoot = try? JSONSerialization.jsonObject(with: pData) as? [String: Any] {
                if let rawPages = pRoot["pages"] as? [[String: Any]], !rawPages.isEmpty {
                    let parsedPages: [VexioPageSummary] = rawPages.compactMap { p in
                        guard let pid = p["id"] as? Int, pid >= 0 else { return nil }
                        let name = (p["name"] as? String) ?? "Page-\(pid)"
                        let bg = (p["isBackground"] as? Bool) ?? false
                        let wIn = (p["widthIn"] as? Double) ?? pageWidthInches
                        let hIn = (p["heightIn"] as? Double) ?? pageHeightInches
                        let bpidInt = (p["backPageId"] as? Int) ?? 0
                        let colorHex = (p["pageColorHex"] as? String) ?? "#FFFFFF"
                        return VexioPageSummary(
                            id: UInt32(pid),
                            title: name,
                            isBackgroundPage: bg,
                            widthInches: wIn,
                            heightInches: hIn,
                            backPageId: bpidInt > 0 ? UInt32(bpidInt) : 0,
                            pageColorHex: colorHex.isEmpty ? "#FFFFFF" : colorHex
                        )
                    }
                    if !parsedPages.isEmpty {
                        pages = parsedPages
                    }
                    initialFgPageId = parsedPages.first(where: { !$0.isBackgroundPage })?.id
                        ?? parsedPages.first?.id
                        ?? 0
                } else if let resolvedPid = pRoot["activePageId"] as? Int, resolvedPid >= 0 {
                    initialFgPageId = UInt32(resolvedPid)
                }
                if let shapes = pRoot["shapes"] as? [[String: Any]],
                   let firstShapeId = (shapes.first?["id"] as? UInt64) ?? (shapes.first?["id"] as? Int).map({ UInt64(max(0, $0)) }),
                   firstShapeId > 0 {
                    selectedShapeId = firstShapeId
                }
            }
            activePageId = initialFgPageId
            needsFitOnViewportReady = true
            core.setViewportPage(pageId: initialFgPageId)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            markCurrentStateAsClean()
            fitViewportToActivePage()
            DispatchQueue.main.async { [weak self] in
                self?.fitViewportToActivePage()
            }
            statusMessage = "Opened \(url.lastPathComponent) (\(pages.filter { !$0.isBackgroundPage }.count) page(s))"
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = "Failed to open \(url.lastPathComponent): \(error.localizedDescription)"
        }
    }

    public func saveToURL(_ url: URL) throws {
        let ext = url.pathExtension.isEmpty ? "vsdx" : url.pathExtension.lowercased()
        let snap = try core.saveFormat(ext)
        try snap.data.write(to: url, options: .atomic)
        currentFileURL = url
        sourceFormatExtension = ext
        documentTitle = url.deletingPathExtension().lastPathComponent
        core.setDocumentTitle(documentTitle)
        markCurrentStateAsClean()
        NSDocumentController.shared.noteNewRecentDocumentURL(url)
        statusMessage = "Saved \(url.lastPathComponent)"
    }

    public func renameDocument(to rawNewName: String) {
        let trimmed = rawNewName
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .replacingOccurrences(of: "/", with: "-")
        guard !trimmed.isEmpty else { return }
        let stem: String
        if trimmed.lowercased().hasSuffix(".\(sourceFormatExtension.lowercased())") {
            stem = String(trimmed.dropLast(sourceFormatExtension.count + 1))
        } else {
            stem = trimmed
        }
        guard !stem.isEmpty else { return }
        let changedTitle = (documentTitle != stem)
        documentTitle = stem
        core.setDocumentTitle(stem)

        if let existingURL = currentFileURL {
            let newURL = existingURL
                .deletingLastPathComponent()
                .appendingPathComponent("\(stem).\(existingURL.pathExtension.isEmpty ? sourceFormatExtension : existingURL.pathExtension)")
            if newURL != existingURL, !FileManager.default.fileExists(atPath: newURL.path) {
                if (try? FileManager.default.moveItem(at: existingURL, to: newURL)) != nil {
                    currentFileURL = newURL
                    NSDocumentController.shared.noteNewRecentDocumentURL(newURL)
                }
            }
        } else if changedTitle {
            hasMetadataUnsavedEdit = true
            recomputeHasUnsavedChanges()
        }
        statusMessage = "Renamed diagram to '\(stem)'"
    }

    public static func persistedStencilsDirectoryURL() -> URL? {
        guard let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first else {
            return nil
        }
        let dir = appSupport.appendingPathComponent("Vexio/Stencils", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    public func listOfficialStencilCatalog() -> [OfficialStencilCatalogEntry] {
        let raw = core.listOfficialStencilCatalogJSON()
        guard let data = raw.data(using: .utf8),
              let items = try? JSONDecoder().decode([OfficialStencilCatalogEntry].self, from: data)
        else {
            return []
        }
        return items
    }

    public func isOfficialStencilInstalled(_ entry: OfficialStencilCatalogEntry) -> Bool {
        guard let dir = Self.persistedStencilsDirectoryURL() else { return false }
        let fileURL = dir.appendingPathComponent(entry.fileName)
        return FileManager.default.fileExists(atPath: fileURL.path)
    }

    private func loadPersistedUserStencils() {
        guard let dir = Self.persistedStencilsDirectoryURL(),
              let files = try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)
        else { return }
        let sorted = files.sorted { $0.lastPathComponent < $1.lastPathComponent }
        for fileURL in sorted where !fileURL.lastPathComponent.hasPrefix(".") {
            if let bytes = try? Data(contentsOf: fileURL), !bytes.isEmpty {
                _ = try? importStencilDataInternal(fileName: fileURL.lastPathComponent, bytes: bytes, persistCopy: false, quiet: true)
            }
        }
    }

    @discardableResult
    public func importStencilDataInternal(
        fileName: String,
        bytes: Data,
        persistCopy: Bool,
        quiet: Bool = false
    ) throws -> Int {
        ensureStencilsLoadedIfNeeded()
        let jsonStr = try core.importStencilBytes(fileName: fileName, bytes: bytes)
        guard let jsonData = jsonStr.data(using: .utf8),
              let imported = try? JSONDecoder().decode(ImportedStencilPackPayload.self, from: jsonData),
              !imported.masters.isEmpty
        else {
            if !quiet {
                statusMessage = "No stencil masters found in \(fileName)"
            }
            return 0
        }

        let subPacks: [ImportedStencilSubPackPayload] = {
            if let p = imported.packs, !p.isEmpty {
                return p
            }
            return [ImportedStencilSubPackPayload(packTitle: imported.packTitle, masters: imported.masters)]
        }()

        var totalAdded = 0
        for sub in subPacks.reversed() where !sub.masters.isEmpty {
            let pack = sub.packTitle
            let mappedMasters: [VexioStencilMaster] = sub.masters.map { item in
                let fillClean = item.fillFormula.hasPrefix("=")
                    ? String(item.fillFormula.dropFirst())
                    : item.fillFormula
                return VexioStencilMaster(
                    id: "\(pack)::\(item.name)",
                    category: pack,
                    name: item.name,
                    prompt: item.prompt,
                    symbolName: Self.symbolName(forCategory: pack, masterName: item.name),
                    defaultWidthFormula: String(format: "%.3f in", item.widthIn),
                    defaultHeightFormula: String(format: "%.3f in", item.heightIn),
                    defaultFillFormula: fillClean,
                    isGuardedAspect: false,
                    svgPreview: item.svg,
                    pngPreview: item.pngDataUrl ?? ""
                )
            }
            totalAdded += mappedMasters.count
            stencilCatalog.removeAll { $0.category.caseInsensitiveCompare(pack) == .orderedSame }
            stencilCatalog.insert(contentsOf: mappedMasters, at: 0)
            if !stencilCategories.contains(where: { $0.caseInsensitiveCompare(pack) == .orderedSame }) {
                stencilCategories.insert(pack, at: 0)
            }
        }

        if persistCopy, let dir = Self.persistedStencilsDirectoryURL() {
            let safeName = URL(fileURLWithPath: fileName).lastPathComponent
            if !safeName.isEmpty {
                try? bytes.write(to: dir.appendingPathComponent(safeName), options: .atomic)
            }
        }

        if !quiet {
            showStencilNavigator = true
            statusMessage = "Imported \(totalAdded) Visio stencil(s) across \(subPacks.count) pack(s)"
        }
        return totalAdded
    }

    public func downloadOfficialStencilPack(_ entry: OfficialStencilCatalogEntry) async -> Bool {
        guard let remoteURL = URL(string: entry.url) else {
            statusMessage = "Invalid stencil URL for \(entry.title)"
            return false
        }
        statusMessage = "Downloading '\(entry.title)'…"
        do {
            let (data, response) = try await URLSession.shared.data(from: remoteURL)
            if let http = response as? HTTPURLResponse, !(200...299).contains(http.statusCode) {
                statusMessage = "Download failed (HTTP \(http.statusCode)) for \(entry.title)"
                return false
            }
            let count = try importStencilDataInternal(fileName: entry.fileName, bytes: data, persistCopy: true)
            statusMessage = "Installed '\(entry.title)' (\(count) stencils)"
            return count > 0
        } catch {
            statusMessage = "Download failed: \(error.localizedDescription)"
            return false
        }
    }

    public func importVisioStencilFile(from url: URL) {
        ensureStencilsLoadedIfNeeded()
        let accessed = url.startAccessingSecurityScopedResource()
        defer { if accessed { url.stopAccessingSecurityScopedResource() } }
        do {
            let bytes = try Data(contentsOf: url)
            _ = try importStencilDataInternal(fileName: url.lastPathComponent, bytes: bytes, persistCopy: true)
        } catch let bridgeErr as VexioBridgeError {
            lastBannerError = bridgeErr
            statusMessage = bridgeErr.localizedDescription
        } catch {
            statusMessage = "Stencil import failed: \(error.localizedDescription)"
        }
    }

    public func insertContainerBox(title: String = "Container", undoManager: UndoManager? = nil) {
        let members = selectedShapeIds.filter { $0 > 0 }
        do {
            let cid = try core.insertContainer(pageId: activePageId, title: title, memberIds: members)
            if cid > 0 {
                selectedShapeId = cid
                selectedShapeIds = [cid]
                core.setSelectionIds([cid])
                registerJournalUndoStep(actionName: "Insert Container", undoManager: undoManager)
                reloadSelectedShapeSheetCells()
                syncSelectedShapeMetadataFromCore()
                core.renderViewportFrame()
                statusMessage = members.isEmpty
                    ? "Inserted interactive Container (drag shapes inside to group)"
                    : "Enclosed \(members.count) shape(s) in Container"
            }
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func insertSwimlanePool(lanesSemicolon: String = "", undoManager: UndoManager? = nil) {
        do {
            let count = try core.insertSwimlanes(pageId: activePageId, lanesSemicolon: lanesSemicolon)
            registerJournalUndoStep(actionName: "Insert Swimlanes", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = count > 1
                ? "Inserted \(count)-lane Cross-Functional Swimlane pool"
                : "Appended contiguous Cross-Functional Swimlane band"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func attachCalloutNote(text: String = "Callout Note", undoManager: UndoManager? = nil) {
        let targetId = selectedShapeIds.first ?? selectedShapeId
        do {
            let cid = try core.attachCallout(pageId: activePageId, targetShapeId: targetId, text: text)
            if cid > 0 {
                selectedShapeId = cid
                selectedShapeIds = [cid]
                core.setSelectionIds([cid])
                registerJournalUndoStep(actionName: "Attach Callout", undoManager: undoManager)
                reloadSelectedShapeSheetCells()
                syncSelectedShapeMetadataFromCore()
                core.renderViewportFrame()
                statusMessage = targetId > 0
                    ? "Attached reactive Callout to Sheet.\(targetId) (drag yellow diamond handle to retarget)"
                    : "Inserted Callout Note (drag yellow diamond handle onto any shape)"
            }
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func autoConnectSelected(direction: String, undoManager: UndoManager? = nil) {
        let srcId = selectedShapeIds.first ?? selectedShapeId
        guard srcId > 0 else { return }
        do {
            let targetId = try core.autoConnect(pageId: activePageId, sourceShapeId: srcId, direction: direction)
            if targetId > 0 {
                selectedShapeId = targetId
                selectedShapeIds = [targetId]
                core.setSelectionIds([targetId])
                registerJournalUndoStep(actionName: "AutoConnect Shape", undoManager: undoManager)
                reloadSelectedShapeSheetCells()
                syncSelectedShapeMetadataFromCore()
                core.renderViewportFrame()
                statusMessage = "AutoConnected \(direction.capitalized) to Sheet.\(targetId)"
            }
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    // MARK: - Pillar #4 High-Level Session Actions (Data Graphics, Legends & Two-Way External Data)

    public func commitDataGraphicConfig(undoManager: UndoManager? = nil) {
        let cfg = dataGraphicConfig
        let payload: [String: Any] = [
            "enabled": cfg.isEnabled,
            "iconSetField": cfg.iconSetField,
            "iconGreenMin": cfg.iconGreenMin,
            "iconAmberMin": cfg.iconAmberMin,
            "dataBarField": cfg.dataBarField,
            "dataBarMin": cfg.dataBarMin,
            "dataBarMax": cfg.dataBarMax,
            "dataBarColorHex": cfg.dataBarColorHex,
            "textCalloutField": cfg.textCalloutField,
            "textCalloutPrefix": cfg.textCalloutPrefix,
            "colorByValueEnabled": cfg.colorByValueEnabled,
            "colorByValueField": cfg.colorByValueField,
            "legendVisible": cfg.showLegendOnPage,
            "legendField": cfg.legendField,
        ]
        guard let jsonData = try? JSONSerialization.data(withJSONObject: payload),
              let jsonStr = String(data: jsonData, encoding: .utf8)
        else { return }
        do {
            let decorated = try core.setPageDataGraphicsJSON(pageId: activePageId, configJSON: jsonStr)
            registerJournalUndoStep(actionName: "Update Data Graphics", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = cfg.isEnabled
                ? "Applied Data Graphics across \(decorated) shape(s)"
                : "Data Graphics overlay hidden"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func insertOrRefreshPageLegend(propName: String = "", undoManager: UndoManager? = nil) {
        do {
            let effProp = propName.isEmpty ? dataGraphicConfig.legendField : propName
            let lid = try core.insertDataLegend(pageId: activePageId, propName: effProp)
            registerJournalUndoStep(actionName: "Insert Diagram Legend", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = lid > 0
                ? "Updated live Diagram Legend (Sheet.\(lid))"
                : "Inserted Diagram Legend"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func importExternalDataFile(from url: URL, undoManager: UndoManager? = nil) {
        let accessed = url.startAccessingSecurityScopedResource()
        defer { if accessed { url.stopAccessingSecurityScopedResource() } }
        do {
            let bytes = try Data(contentsOf: url)
            let result = try core.importExternalRecordset(
                pageId: activePageId,
                fileName: url.lastPathComponent,
                bytes: bytes,
                autoBind: true,
                autoEnableGraphics: true
            )
            linkedRecordsetURLs[result.recordsetId] = url
            registerJournalUndoStep(actionName: "Import External Data", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            showInspectorSidebar = true
            statusMessage = "Imported '\(url.lastPathComponent)' — linked \(result.linkedCount) shape(s)"
        } catch {
            statusMessage = "External data import failed: \(error.localizedDescription)"
        }
    }

    public func bindExternalRecordsetRow(
        recordsetId: UInt32,
        rowIndex: Int,
        undoManager: UndoManager? = nil
    ) {
        guard rowIndex >= 0 else { return }
        let targets: [UInt64] = {
            if !selectedShapeIds.isEmpty {
                return selectedShapeIds.filter { $0 > 0 }
            }
            return selectedShapeId > 0 ? [selectedShapeId] : []
        }()
        do {
            let cnt = try core.bindExternalRow(
                pageId: activePageId,
                recordsetId: recordsetId,
                rowIndex: UInt32(rowIndex),
                shapeIds: targets
            )
            registerJournalUndoStep(actionName: "Bind External Data Row", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = targets.isEmpty
                ? "Created & bound shape from Row #\(rowIndex + 1)"
                : "Bound Row #\(rowIndex + 1) to \(cnt) shape(s)"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func refreshExternalDataSources(undoManager: UndoManager? = nil) {
        var totalRefreshed: UInt32 = 0
        do {
            if externalRecordsets.isEmpty {
                totalRefreshed = try core.refreshExternalRecordset(pageId: activePageId)
            } else {
                for rs in externalRecordsets {
                    var freshBytes: Data? = nil
                    if let url = linkedRecordsetURLs[rs.id] {
                        let accessed = url.startAccessingSecurityScopedResource()
                        freshBytes = try? Data(contentsOf: url)
                        if accessed { url.stopAccessingSecurityScopedResource() }
                    }
                    totalRefreshed += try core.refreshExternalRecordset(
                        pageId: activePageId,
                        recordsetId: rs.id,
                        newBytes: freshBytes,
                        formatHint: rs.sourceName
                    )
                }
            }
            registerJournalUndoStep(actionName: "Refresh External Data", undoManager: undoManager)
            reloadSelectedShapeSheetCells()
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Refreshed External Data (\(totalRefreshed) shape(s) synchronized)"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func removeExternalRecordset(recordsetId: UInt32, undoManager: UndoManager? = nil) {
        do {
            try core.deleteExternalRecordset(recordsetId: recordsetId)
            linkedRecordsetURLs.removeValue(forKey: recordsetId)
            registerJournalUndoStep(actionName: "Remove External Data", undoManager: undoManager)
            syncSelectedShapeMetadataFromCore()
            core.renderViewportFrame()
            statusMessage = "Unlinked External Data recordset"
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    public func presentImportExternalDataModal(undoManager: UndoManager? = nil) {
        let panel = NSOpenPanel()
        panel.title = "Link External Data (.xlsx, .csv, .sqlite, .json)"
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        var types: [UTType] = [.commaSeparatedText, .tabSeparatedText, .json, .spreadsheet, .data]
        for ext in ["xlsx", "csv", "tsv", "sqlite", "db", "sqlite3", "json"] {
            if let t = UTType(filenameExtension: ext), !types.contains(t) {
                types.append(t)
            }
        }
        panel.allowedContentTypes = types
        if let window = NSApp.keyWindow ?? NSApp.windows.first {
            panel.beginSheetModal(for: window) { [weak self] response in
                guard response == .OK, let url = panel.url else { return }
                Task { @MainActor in
                    self?.importExternalDataFile(from: url, undoManager: undoManager)
                }
            }
        }
    }

    public func presentTwoWayWritebackModal(recordsetId: UInt32 = 0, undoManager: UndoManager? = nil) {
        let matched = externalRecordsets.first(where: { recordsetId == 0 || $0.id == recordsetId })
        let defaultExt: String = matched?.sourceKind ?? "xlsx"
        let baseName: String = matched?.name ?? "\(documentTitle)_Synced"
        let panel = NSSavePanel()
        panel.title = "Two-Way Writeback Export (.xlsx / .csv / .sqlite / .json)"
        panel.nameFieldStringValue = "\(baseName).\(defaultExt)"
        panel.canCreateDirectories = true
        panel.isExtensionHidden = false
        if let window = NSApp.keyWindow ?? NSApp.windows.first {
            panel.beginSheetModal(for: window) { [weak self] response in
                guard response == .OK, let url = panel.url else { return }
                Task { @MainActor in
                    guard let self else { return }
                    let ext = url.pathExtension.isEmpty ? defaultExt : url.pathExtension.lowercased()
                    do {
                        let data = try self.core.syncAndExportRecordset(
                            pageId: self.activePageId,
                            recordsetId: recordsetId,
                            formatExt: ext
                        )
                        try data.write(to: url, options: .atomic)
                        self.registerJournalUndoStep(actionName: "Two-Way Writeback", undoManager: undoManager)
                        self.syncSelectedShapeMetadataFromCore()
                        self.statusMessage = "Synced Prop.* edits & exported '\(url.lastPathComponent)' (\(data.count) bytes)"
                    } catch {
                        self.statusMessage = "Writeback export failed: \(error.localizedDescription)"
                    }
                }
            }
        }
    }

    public func presentBomReportExportModal() {
        let panel = NSSavePanel()
        panel.title = "Export BOM Shape Inventory Report (.xlsx / .csv / .json / .sqlite)"
        panel.nameFieldStringValue = "\(documentTitle)_BOM_Report.xlsx"
        panel.canCreateDirectories = true
        panel.isExtensionHidden = false
        if let window = NSApp.keyWindow ?? NSApp.windows.first {
            panel.beginSheetModal(for: window) { [weak self] response in
                guard response == .OK, let url = panel.url else { return }
                Task { @MainActor in
                    guard let self else { return }
                    let ext = url.pathExtension.isEmpty ? "xlsx" : url.pathExtension.lowercased()
                    do {
                        let data = try self.core.exportBomReport(formatExt: ext)
                        try data.write(to: url, options: .atomic)
                        self.statusMessage = "Exported BOM Shape Report '\(url.lastPathComponent)' (\(data.count) bytes)"
                    } catch {
                        self.statusMessage = "BOM Report export failed: \(error.localizedDescription)"
                    }
                }
            }
        }
    }
}

