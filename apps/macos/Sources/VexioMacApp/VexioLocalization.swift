import Foundation
import SwiftUI

// MARK: - 6-Language Localization (`EN`, `DE`, `IT`, `FR`, `ES`, `GR`) & OS Metric/Unit Resolver

public enum VexioAppLanguage: String, CaseIterable, Identifiable, Sendable {
    case en = "en"
    case de = "de"
    case it = "it"
    case fr = "fr"
    case es = "es"
    case el = "el"

    public var id: String { rawValue }

    public var badgeCode: String {
        switch self {
        case .en: return "EN"
        case .de: return "DE"
        case .it: return "IT"
        case .fr: return "FR"
        case .es: return "ES"
        case .el: return "GR"
        }
    }

    public var nativeDisplayName: String {
        switch self {
        case .en: return "English (EN)"
        case .de: return "Deutsch (DE)"
        case .it: return "Italiano (IT)"
        case .fr: return "Français (FR)"
        case .es: return "Español (ES)"
        case .el: return "Ελληνικά (GR)"
        }
    }

    public static func detectFromSystem() -> VexioAppLanguage {
        let preferred = Locale.preferredLanguages.first?.lowercased() ?? "en"
        if preferred.hasPrefix("de") { return .de }
        if preferred.hasPrefix("it") { return .it }
        if preferred.hasPrefix("fr") { return .fr }
        if preferred.hasPrefix("es") { return .es }
        if preferred.hasPrefix("el") || preferred.hasPrefix("gr") { return .el }
        return .en
    }
}

public enum VexioMeasurementUnit: String, CaseIterable, Identifiable, Sendable {
    case osDefault = "os"
    case mm = "mm"
    case cm = "cm"
    case m = "m"
    case inch = "in"

    public var id: String { rawValue }

    /// Resolves `osDefault` against macOS `Locale.current.measurementSystem`, defaulting to Metric (`mm`)
    /// whenever the OS uses Metric or UK/ISO conventions.
    public var resolvedEngineSlug: String {
        switch self {
        case .osDefault:
            if Locale.current.measurementSystem == .us {
                return "in"
            }
            return "mm"
        case .mm: return "mm"
        case .cm: return "cm"
        case .m: return "m"
        case .inch: return "in"
        }
    }

    public var isMetric: Bool {
        resolvedEngineSlug != "in"
    }

    public func formatFromInches(_ inches: Double) -> String {
        switch resolvedEngineSlug {
        case "mm":
            return String(format: "%.1f mm", inches * 25.4)
        case "cm":
            return String(format: "%.2f cm", inches * 2.54)
        case "m":
            return String(format: "%.3f m", inches * 0.0254)
        default:
            return String(format: "%.2f in", inches)
        }
    }

    public func numericDisplayFromInches(_ inches: Double) -> String {
        switch resolvedEngineSlug {
        case "mm":
            return String(format: "%.1f", inches * 25.4)
        case "cm":
            return String(format: "%.2f", inches * 2.54)
        case "m":
            return String(format: "%.3f", inches * 0.0254)
        default:
            return String(format: "%.2f", inches)
        }
    }

    public func formulaFromUserInput(_ raw: String) -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return "0 \(resolvedEngineSlug)" }
        if trimmed.hasPrefix("=") {
            return trimmed
        }
        let normalized = trimmed.replacingOccurrences(of: ",", with: ".")
        if Double(normalized) != nil {
            return "\(normalized) \(resolvedEngineSlug)"
        }
        return normalized
    }
}

public enum VexioPagePreset: String, CaseIterable, Identifiable, Sendable {
    case a4 = "a4"
    case a3 = "a3"
    case a5 = "a5"
    case a2 = "a2"
    case a1 = "a1"
    case a0 = "a0"
    case letter = "letter"
    case legal = "legal"
    case tabloid = "tabloid"

    public var id: String { rawValue }

    public var displayLabel: String {
        switch self {
        case .a4: return "ISO A4 (210 × 297 mm)"
        case .a3: return "ISO A3 (297 × 420 mm)"
        case .a5: return "ISO A5 (148 × 210 mm)"
        case .a2: return "ISO A2 (420 × 594 mm)"
        case .a1: return "ISO A1 (594 × 841 mm)"
        case .a0: return "ISO A0 (841 × 1189 mm)"
        case .letter: return "US Letter (8.5 × 11 in)"
        case .legal: return "US Legal (8.5 × 14 in)"
        case .tabloid: return "Tabloid / ANSI B (11 × 17 in)"
        }
    }

    public var shortCode: String {
        switch self {
        case .a4: return "A4"
        case .a3: return "A3"
        case .a5: return "A5"
        case .a2: return "A2"
        case .a1: return "A1"
        case .a0: return "A0"
        case .letter: return "Letter"
        case .legal: return "Legal"
        case .tabloid: return "Tabloid"
        }
    }
}

@MainActor
public final class VexioLocalization: ObservableObject {
    public static let shared = VexioLocalization()

    @Published public var language: VexioAppLanguage {
        didSet {
            UserDefaults.standard.set(language.rawValue, forKey: "vexio.appLanguage")
        }
    }

    @Published public var unitPreference: VexioMeasurementUnit {
        didSet {
            UserDefaults.standard.set(unitPreference.rawValue, forKey: "vexio.measurementUnit")
        }
    }

    public init() {
        if let savedLang = UserDefaults.standard.string(forKey: "vexio.appLanguage"),
           let parsedLang = VexioAppLanguage(rawValue: savedLang) {
            self.language = parsedLang
        } else {
            self.language = VexioAppLanguage.detectFromSystem()
        }

        if let savedUnit = UserDefaults.standard.string(forKey: "vexio.measurementUnit"),
           let parsedUnit = VexioMeasurementUnit(rawValue: savedUnit) {
            self.unitPreference = parsedUnit
        } else {
            self.unitPreference = .osDefault
        }
    }

    public func t(_ key: String) -> String {
        Self.catalog[language]?[key] ?? Self.catalog[.en]?[key] ?? key
    }

    private static let catalog: [VexioAppLanguage: [String: String]] = [
        .en: [
            "file": "File",
            "home": "Home",
            "insert": "Insert",
            "design": "Design",
            "data": "Data",
            "process": "Process",
            "review": "Review",
            "view": "View",
            "developer": "Developer",
            "clipboard": "Clipboard",
            "font": "Font",
            "paragraph": "Paragraph",
            "tools": "Tools",
            "shapeStyles": "Shape Styles",
            "arrange": "Arrange",
            "newBlankA4": "New Blank Drawing (A4 Metric)",
            "newBlankA3": "New Blank Drawing (A3 Metric)",
            "newBlankLetter": "New Blank Drawing (US Letter)",
            "openFile": "Open Diagram or CAD File…",
            "importStencil": "Import Stencil…",
            "saveFile": "Save",
            "saveAs": "Save As…",
            "renamePage": "Rename Page…",
            "fillColor": "Fill",
            "lineColor": "Line",
            "clearHighlight": "No Highlight",
            "fontPanel": "OS Font Panel…",
            "export": "Export",
            "recentFiles": "Recent Files",
            "templates": "Starter Templates",
            "welcomeTitle": "Welcome to Vexio",
            "welcomeSubtitle": "Start a blank diagram, pick a starter template, or open an existing Visio or CAD file.",
            "newBlankDiagram": "New Blank Diagram",
            "newBlankSubtitle": "Start immediately on a clean canvas — customize page size & units anytime inside the workspace",
            "pageSetup": "Page Setup…",
            "pageBackground": "Page Background",
            "stencilCatalog": "Stencil Catalog…",
            "continueToCanvas": "Close & Continue Editing",
            "skipStartOnLaunch": "Open directly into a blank drawing on startup",
            "rotateLeft": "Rotate Left 90°",
            "rotateRight": "Rotate Right 90°",
            "flipH": "Flip Horizontal",
            "flipV": "Flip Vertical",
            "pageSize": "Page Size",
            "orientation": "Orientation",
            "landscape": "Landscape",
            "portrait": "Portrait",
            "units": "Units",
            "osStandard": "OS Standard",
            "checkForUpdates": "Check for Updates…",
            "autoUpdates": "Automatically check for updates",
            "language": "Language",
            "doubleClickRenameHint": "Double-click to rename",
            "editLabelPlaceholder": "Type label text…",
            "done": "Done",
            "cancel": "Cancel",
            "stencils": "Stencils",
            "pointer": "Select",
            "connector": "Connector",
            "text": "Text",
            "routing": "Routing",
            "autoLayout": "Auto-Layout",
            "validate": "Validate",
            "shapeOps": "Shape Ops",
            "shapeSheet": "ShapeSheet",
            "inspectorFormat": "Format",
            "inspectorSizePos": "Size",
            "inspectorData": "Data",
            "inspectorLayers": "Layers",
            "pageInspector": "Page Properties",
            "objectName": "Name",
            "textLabel": "Text",
            "fillSection": "Fill",
            "noFill": "No Fill",
            "solidFill": "Solid Fill",
            "gradientFill": "Gradient",
            "transparency": "Transparency",
            "cornerRounding": "Corner Rounding",
            "lineArrowsSection": "Line & Arrows",
            "noLine": "No Line",
            "solidLine": "Solid",
            "dashedLine": "Dashed",
            "dottedLine": "Dotted",
            "lineWeight": "Weight",
            "beginArrow": "Begin Arrow",
            "endArrow": "End Arrow",
            "effectsSection": "Effects",
            "sizeAndPosition": "Size & Position",
            "width": "Width",
            "height": "Height",
            "angle": "Angle",
            "pinAnchor": "Pin Pos",
            "protection": "Protection",
            "lockAspect": "Lock Aspect Ratio",
            "lockSize": "Lock Width & Height",
            "lockPosition": "Lock Position",
            "lockRotation": "Lock Rotation",
            "lockDelete": "Lock from Deletion",
            "hideText": "Hide Shape Text",
            "noShapeData": "No Shape Data fields defined on this object.",
            "addDataField": "Add Field",
            "fieldName": "Field Name",
            "fieldValue": "Value",
            "layerMembership": "Object Layer Membership",
            "pageLayers": "Page Layers",
            "addLayer": "Add Layer",
            "newLayerPlaceholder": "New layer name…",
            "openShapeSheet": "Open ShapeSheet…",
        ],
        .de: [
            "file": "Datei",
            "home": "Start",
            "insert": "Einfügen",
            "design": "Entwurf",
            "data": "Daten",
            "process": "Prozess",
            "review": "Überprüfen",
            "view": "Ansicht",
            "developer": "Entwickler",
            "clipboard": "Zwischenablage",
            "font": "Schriftart",
            "paragraph": "Absatz",
            "tools": "Werkzeuge",
            "shapeStyles": "Formate",
            "arrange": "Anordnen",
            "newBlankA4": "Neue leere Zeichnung (DIN A4)",
            "newBlankA3": "Neue leere Zeichnung (DIN A3)",
            "newBlankLetter": "Neue leere Zeichnung (US Letter)",
            "openFile": "Diagramm oder CAD öffnen…",
            "importStencil": "Schablone importieren…",
            "saveFile": "Speichern",
            "saveAs": "Speichern unter…",
            "renamePage": "Seite umbenennen…",
            "fillColor": "Füllung",
            "lineColor": "Linie",
            "clearHighlight": "Keine Hervorhebung",
            "fontPanel": "Schriftenfenster…",
            "export": "Exportieren",
            "recentFiles": "Zuletzt verwendet",
            "templates": "Vorlagen",
            "welcomeTitle": "Willkommen bei Vexio",
            "welcomeSubtitle": "Starten Sie ein leeres Diagramm, wählen Sie eine Vorlage oder öffnen Sie eine Visio- oder CAD-Datei.",
            "newBlankDiagram": "Neues leeres Diagramm",
            "newBlankSubtitle": "Sofort auf einer leeren Zeichenfläche starten — Seitengröße & Einheiten jederzeit in der App anpassbar",
            "pageSetup": "Seite einrichten…",
            "pageBackground": "Seitenhintergrund",
            "stencilCatalog": "Schablonenkatalog…",
            "continueToCanvas": "Schließen & weiterarbeiten",
            "skipStartOnLaunch": "Beim Start direkt mit leerem Blatt beginnen",
            "rotateLeft": "90° links drehen",
            "rotateRight": "90° rechts drehen",
            "flipH": "Horizontal spiegeln",
            "flipV": "Vertikal spiegeln",
            "pageSize": "Seitengröße",
            "orientation": "Ausrichtung",
            "landscape": "Querformat",
            "portrait": "Hochformat",
            "units": "Einheiten",
            "osStandard": "Systemstandard",
            "checkForUpdates": "Nach Updates suchen…",
            "autoUpdates": "Automatisch nach Updates suchen",
            "language": "Sprache",
            "doubleClickRenameHint": "Doppelklicken zum Umbenennen",
            "editLabelPlaceholder": "Text eingeben…",
            "done": "Fertig",
            "cancel": "Abbrechen",
            "stencils": "Schablonen",
            "pointer": "Auswahl",
            "connector": "Verbinder",
            "text": "Text",
            "routing": "Verlegung",
            "autoLayout": "Auto-Layout",
            "validate": "Prüfen",
            "shapeOps": "Formoperationen",
            "shapeSheet": "ShapeSheet",
            "inspectorFormat": "Format",
            "inspectorSizePos": "Maße",
            "inspectorData": "Daten",
            "inspectorLayers": "Layer",
            "pageInspector": "Seiteneigenschaften",
            "objectName": "Name",
            "textLabel": "Text",
            "fillSection": "Füllung",
            "noFill": "Keine Füllung",
            "solidFill": "Vollton",
            "gradientFill": "Verlauf",
            "transparency": "Transparenz",
            "cornerRounding": "Eckenrundung",
            "lineArrowsSection": "Linie & Pfeile",
            "noLine": "Keine Linie",
            "solidLine": "Durchgezogen",
            "dashedLine": "Gestrichelt",
            "dottedLine": "Gepunktet",
            "lineWeight": "Stärke",
            "beginArrow": "Anfangspfeil",
            "endArrow": "Endpfeil",
            "effectsSection": "Effekte",
            "sizeAndPosition": "Größe & Position",
            "width": "Breite",
            "height": "Höhe",
            "angle": "Winkel",
            "pinAnchor": "Bezugspunkt",
            "protection": "Schutz",
            "lockAspect": "Seitenverhältnis sperren",
            "lockSize": "Breite & Höhe sperren",
            "lockPosition": "Position sperren",
            "lockRotation": "Drehung sperren",
            "lockDelete": "Löschen sperren",
            "hideText": "Formtext ausblenden",
            "noShapeData": "Keine Formdaten für dieses Objekt definiert.",
            "addDataField": "Feld hinzufügen",
            "fieldName": "Feldname",
            "fieldValue": "Wert",
            "layerMembership": "Layer-Zugehörigkeit",
            "pageLayers": "Seiten-Layer",
            "addLayer": "Layer hinzufügen",
            "newLayerPlaceholder": "Neuer Layer-Name…",
            "openShapeSheet": "ShapeSheet öffnen…",
        ],
        .it: [
            "file": "File",
            "home": "Home",
            "insert": "Inserisci",
            "design": "Progettazione",
            "data": "Dati",
            "process": "Processo",
            "review": "Revisione",
            "view": "Visualizza",
            "developer": "Sviluppo",
            "clipboard": "Appunti",
            "font": "Carattere",
            "paragraph": "Paragrafo",
            "tools": "Strumenti",
            "shapeStyles": "Stili forma",
            "arrange": "Disponi",
            "newBlankA4": "Nuovo disegno vuoto (A4 Metrico)",
            "newBlankA3": "Nuovo disegno vuoto (A3 Metrico)",
            "newBlankLetter": "Nuovo disegno vuoto (US Letter)",
            "openFile": "Apri diagramma o CAD…",
            "importStencil": "Importa stencil…",
            "saveFile": "Salva",
            "saveAs": "Salva con nome…",
            "renamePage": "Rinomina pagina…",
            "fillColor": "Riempimento",
            "lineColor": "Linea",
            "clearHighlight": "Nessuna evidenziazione",
            "fontPanel": "Pannello caratteri…",
            "export": "Esporta",
            "recentFiles": "File recenti",
            "templates": "Modelli",
            "welcomeTitle": "Benvenuto in Vexio",
            "welcomeSubtitle": "Inizia un diagramma vuoto, scegli un modello o apri un file Visio o CAD esistente.",
            "newBlankDiagram": "Nuovo diagramma vuoto",
            "newBlankSubtitle": "Inizia subito su un foglio pulito — formato pagina e unità configurabili nell'area di lavoro",
            "pageSetup": "Imposta pagina…",
            "pageBackground": "Sfondo pagina",
            "stencilCatalog": "Catalogo stencil…",
            "continueToCanvas": "Chiudi e continua",
            "skipStartOnLaunch": "Apri direttamente un foglio vuoto all'avvio",
            "rotateLeft": "Ruota 90° a sinistra",
            "rotateRight": "Ruota 90° a destra",
            "flipH": "Capovolgi orizzontalmente",
            "flipV": "Capovolgi verticalmente",
            "pageSize": "Formato pagina",
            "orientation": "Orientamento",
            "landscape": "Orizzontale",
            "portrait": "Verticale",
            "units": "Unità",
            "osStandard": "Standard di sistema",
            "checkForUpdates": "Controlla aggiornamenti…",
            "autoUpdates": "Controlla aggiornamenti automaticamente",
            "language": "Lingua",
            "doubleClickRenameHint": "Doppio clic per rinominare",
            "editLabelPlaceholder": "Inserisci testo…",
            "done": "Fatto",
            "cancel": "Annulla",
            "stencils": "Stencil",
            "pointer": "Seleziona",
            "connector": "Connettore",
            "text": "Testo",
            "routing": "Percorso",
            "autoLayout": "Layout auto",
            "validate": "Convalida",
            "shapeOps": "Operazioni forme",
            "shapeSheet": "ShapeSheet",
            "inspectorFormat": "Formato",
            "inspectorSizePos": "Misure",
            "inspectorData": "Dati",
            "inspectorLayers": "Layer",
            "pageInspector": "Proprietà pagina",
            "objectName": "Nome",
            "textLabel": "Testo",
            "fillSection": "Riempimento",
            "noFill": "Nessuno",
            "solidFill": "Tinta unita",
            "gradientFill": "Sfumatura",
            "transparency": "Trasparenza",
            "cornerRounding": "Arrotondamento",
            "lineArrowsSection": "Linea e frecce",
            "noLine": "Nessuna linea",
            "solidLine": "Continua",
            "dashedLine": "Tratteggiata",
            "dottedLine": "Punteggiata",
            "lineWeight": "Spessore",
            "beginArrow": "Freccia iniziale",
            "endArrow": "Freccia finale",
            "effectsSection": "Effetti",
            "sizeAndPosition": "Dimensioni e posizione",
            "width": "Larghezza",
            "height": "Altezza",
            "angle": "Angolo",
            "pinAnchor": "Perno",
            "protection": "Protezione",
            "lockAspect": "Blocca proporzioni",
            "lockSize": "Blocca dimensioni",
            "lockPosition": "Blocca posizione",
            "lockRotation": "Blocca rotazione",
            "lockDelete": "Blocca eliminazione",
            "hideText": "Nascondi testo",
            "noShapeData": "Nessun campo Dati forma definito.",
            "addDataField": "Aggiungi campo",
            "fieldName": "Nome campo",
            "fieldValue": "Valore",
            "layerMembership": "Appartenenza layer",
            "pageLayers": "Layer della pagina",
            "addLayer": "Aggiungi layer",
            "newLayerPlaceholder": "Nome nuovo layer…",
            "openShapeSheet": "Apri ShapeSheet…",
        ],
        .fr: [
            "file": "Fichier",
            "home": "Accueil",
            "insert": "Insertion",
            "design": "Création",
            "data": "Données",
            "process": "Processus",
            "review": "Révision",
            "view": "Affichage",
            "developer": "Développeur",
            "clipboard": "Presse-papiers",
            "font": "Police",
            "paragraph": "Paragraphe",
            "tools": "Outils",
            "shapeStyles": "Styles de forme",
            "arrange": "Organiser",
            "newBlankA4": "Nouveau dessin vierge (A4 Métrique)",
            "newBlankA3": "Nouveau dessin vierge (A3 Métrique)",
            "newBlankLetter": "Nouveau dessin vierge (US Letter)",
            "openFile": "Ouvrir un diagramme ou CAO…",
            "importStencil": "Importer un gabarit…",
            "saveFile": "Enregistrer",
            "saveAs": "Enregistrer sous…",
            "renamePage": "Renommer la page…",
            "fillColor": "Remplissage",
            "lineColor": "Trait",
            "clearHighlight": "Aucun surlignage",
            "fontPanel": "Panneau des polices…",
            "export": "Exporter",
            "recentFiles": "Fichiers récents",
            "templates": "Modèles",
            "welcomeTitle": "Bienvenue dans Vexio",
            "welcomeSubtitle": "Créez un diagramme vierge, choisissez un modèle ou ouvrez un fichier Visio ou CAO existant.",
            "newBlankDiagram": "Nouveau diagramme vierge",
            "newBlankSubtitle": "Commencez immédiatement sur une feuille vierge — format de page et unités réglables dans l’application",
            "pageSetup": "Mise en page…",
            "pageBackground": "Arrière-plan de page",
            "stencilCatalog": "Catalogue de gabarits…",
            "continueToCanvas": "Fermer et continuer",
            "skipStartOnLaunch": "Ouvrir directement un dessin vierge au démarrage",
            "rotateLeft": "Pivoter 90° à gauche",
            "rotateRight": "Pivoter 90° à droite",
            "flipH": "Retourner horizontalement",
            "flipV": "Retourner verticalement",
            "pageSize": "Taille de page",
            "orientation": "Orientation",
            "landscape": "Paysage",
            "portrait": "Portrait",
            "units": "Unités",
            "osStandard": "Standard système",
            "checkForUpdates": "Rechercher des mises à jour…",
            "autoUpdates": "Vérifier automatiquement les mises à jour",
            "language": "Langue",
            "doubleClickRenameHint": "Double-cliquez pour renommer",
            "editLabelPlaceholder": "Saisissez le texte…",
            "done": "Terminer",
            "cancel": "Annuler",
            "stencils": "Gabarits",
            "pointer": "Sélection",
            "connector": "Connecteur",
            "text": "Texte",
            "routing": "Routage",
            "autoLayout": "Disposition auto",
            "validate": "Valider",
            "shapeOps": "Opérations de forme",
            "shapeSheet": "ShapeSheet",
            "inspectorFormat": "Format",
            "inspectorSizePos": "Taille",
            "inspectorData": "Données",
            "inspectorLayers": "Calques",
            "pageInspector": "Propriétés de page",
            "objectName": "Nom",
            "textLabel": "Texte",
            "fillSection": "Remplissage",
            "noFill": "Aucun",
            "solidFill": "Uni",
            "gradientFill": "Dégradé",
            "transparency": "Transparence",
            "cornerRounding": "Arrondi",
            "lineArrowsSection": "Trait et flèches",
            "noLine": "Aucun trait",
            "solidLine": "Continu",
            "dashedLine": "Tirets",
            "dottedLine": "Pointillés",
            "lineWeight": "Épaisseur",
            "beginArrow": "Flèche début",
            "endArrow": "Flèche fin",
            "effectsSection": "Effets",
            "sizeAndPosition": "Taille et position",
            "width": "Largeur",
            "height": "Hauteur",
            "angle": "Angle",
            "pinAnchor": "Pivot",
            "protection": "Protection",
            "lockAspect": "Verrouiller les proportions",
            "lockSize": "Verrouiller taille",
            "lockPosition": "Verrouiller position",
            "lockRotation": "Verrouiller rotation",
            "lockDelete": "Empêcher suppression",
            "hideText": "Masquer le texte",
            "noShapeData": "Aucune donnée de forme définie.",
            "addDataField": "Ajouter un champ",
            "fieldName": "Nom du champ",
            "fieldValue": "Valeur",
            "layerMembership": "Calques de l’objet",
            "pageLayers": "Calques de la page",
            "addLayer": "Ajouter un calque",
            "newLayerPlaceholder": "Nom du calque…",
            "openShapeSheet": "Ouvrir ShapeSheet…",
        ],
        .es: [
            "file": "Archivo",
            "home": "Inicio",
            "insert": "Insertar",
            "design": "Diseño",
            "data": "Datos",
            "process": "Proceso",
            "review": "Revisar",
            "view": "Vista",
            "developer": "Programador",
            "clipboard": "Portapapeles",
            "font": "Fuente",
            "paragraph": "Párrafo",
            "tools": "Herramientas",
            "shapeStyles": "Estilos de forma",
            "arrange": "Organizar",
            "newBlankA4": "Nuevo dibujo en blanco (A4 Métrico)",
            "newBlankA3": "Nuevo dibujo en blanco (A3 Métrico)",
            "newBlankLetter": "Nuevo dibujo en blanco (US Letter)",
            "openFile": "Abrir diagrama o CAD…",
            "importStencil": "Importar símbolos…",
            "saveFile": "Guardar",
            "saveAs": "Guardar como…",
            "renamePage": "Renombrar página…",
            "fillColor": "Relleno",
            "lineColor": "Línea",
            "clearHighlight": "Sin resaltado",
            "fontPanel": "Panel de fuentes…",
            "export": "Exportar",
            "recentFiles": "Archivos recientes",
            "templates": "Plantillas",
            "welcomeTitle": "Bienvenido a Vexio",
            "welcomeSubtitle": "Inicie un diagrama en blanco, elija una plantilla o abra un archivo Visio o CAD existente.",
            "newBlankDiagram": "Nuevo diagrama en blanco",
            "newBlankSubtitle": "Empiece de inmediato en un lienzo limpio — tamaño de página y unidades ajustables dentro de la app",
            "pageSetup": "Configurar página…",
            "pageBackground": "Fondo de página",
            "stencilCatalog": "Catálogo de galerías…",
            "continueToCanvas": "Cerrar y continuar",
            "skipStartOnLaunch": "Abrir directamente un dibujo en blanco al iniciar",
            "rotateLeft": "Girar 90° a la izquierda",
            "rotateRight": "Girar 90° a la derecha",
            "flipH": "Voltear horizontalmente",
            "flipV": "Voltear verticalmente",
            "pageSize": "Tamaño de página",
            "orientation": "Orientación",
            "landscape": "Horizontal",
            "portrait": "Vertical",
            "units": "Unidades",
            "osStandard": "Estándar del sistema",
            "checkForUpdates": "Buscar actualizaciones…",
            "autoUpdates": "Buscar actualizaciones automáticamente",
            "language": "Idioma",
            "doubleClickRenameHint": "Doble clic para renombrar",
            "editLabelPlaceholder": "Escriba el texto…",
            "done": "Listo",
            "cancel": "Cancelar",
            "stencils": "Galerías",
            "pointer": "Seleccionar",
            "connector": "Conector",
            "text": "Texto",
            "routing": "Enrutamiento",
            "autoLayout": "Diseño auto",
            "validate": "Validar",
            "shapeOps": "Operaciones de forma",
            "shapeSheet": "ShapeSheet",
            "inspectorFormat": "Formato",
            "inspectorSizePos": "Medidas",
            "inspectorData": "Datos",
            "inspectorLayers": "Capas",
            "pageInspector": "Propiedades de página",
            "objectName": "Nombre",
            "textLabel": "Texto",
            "fillSection": "Relleno",
            "noFill": "Sin relleno",
            "solidFill": "Sólido",
            "gradientFill": "Degradado",
            "transparency": "Transparencia",
            "cornerRounding": "Redondeo",
            "lineArrowsSection": "Línea y flechas",
            "noLine": "Sin línea",
            "solidLine": "Continua",
            "dashedLine": "Guiones",
            "dottedLine": "Puntos",
            "lineWeight": "Grosor",
            "beginArrow": "Flecha inicial",
            "endArrow": "Flecha final",
            "effectsSection": "Efectos",
            "sizeAndPosition": "Tamaño y posición",
            "width": "Ancho",
            "height": "Alto",
            "angle": "Ángulo",
            "pinAnchor": "Pivote",
            "protection": "Protección",
            "lockAspect": "Bloquear proporción",
            "lockSize": "Bloquear tamaño",
            "lockPosition": "Bloquear posición",
            "lockRotation": "Bloquear giro",
            "lockDelete": "Bloquear eliminación",
            "hideText": "Ocultar texto",
            "noShapeData": "No hay campos de datos definidos.",
            "addDataField": "Añadir campo",
            "fieldName": "Nombre",
            "fieldValue": "Valor",
            "layerMembership": "Capas del objeto",
            "pageLayers": "Capas de la página",
            "addLayer": "Añadir capa",
            "newLayerPlaceholder": "Nombre de capa…",
            "openShapeSheet": "Abrir ShapeSheet…",
        ],
        .el: [
            "file": "Αρχείο",
            "home": "Κεντρική",
            "insert": "Εισαγωγή",
            "design": "Σχεδίαση",
            "data": "Δεδομένα",
            "process": "Διαδικασία",
            "review": "Αναθεώρηση",
            "view": "Προβολή",
            "developer": "Προγραμματιστής",
            "clipboard": "Πρόχειρο",
            "font": "Γραμματοσειρά",
            "paragraph": "Παράγραφος",
            "tools": "Εργαλεία",
            "shapeStyles": "Στυλ σχήματος",
            "arrange": "Τακτοποίηση",
            "newBlankA4": "Νέο κενό σχέδιο (A4 Μετρικό)",
            "newBlankA3": "Νέο κενό σχέδιο (A3 Μετρικό)",
            "newBlankLetter": "Νέο κενό σχέδιο (US Letter)",
            "openFile": "Άνοιγμα διαγράμματος ή CAD…",
            "importStencil": "Εισαγωγή στένσιλ…",
            "saveFile": "Αποθήκευση",
            "saveAs": "Αποθήκευση ως…",
            "renamePage": "Μετονομασία σελίδας…",
            "fillColor": "Γέμισμα",
            "lineColor": "Γραμμή",
            "clearHighlight": "Χωρίς επισήμανση",
            "fontPanel": "Πίνακας γραμματοσειρών…",
            "export": "Εξαγωγή",
            "recentFiles": "Πρόσφατα αρχεία",
            "templates": "Πρότυπα",
            "welcomeTitle": "Καλώς ήρθατε στο Vexio",
            "welcomeSubtitle": "Ξεκινήστε ένα κενό διάγραμμα, επιλέξτε πρότυπο ή ανοίξτε υπάρχον αρχείο Visio ή CAD.",
            "newBlankDiagram": "Νέο κενό διάγραμμα",
            "newBlankSubtitle": "Ξεκινήστε αμέσως σε καθαρό καμβά — μέγεθος σελίδας & μονάδες ρυθμίζονται μέσα στην εφαρμογή",
            "pageSetup": "Διαμόρφωση σελίδας…",
            "pageBackground": "Φόντο σελίδας",
            "stencilCatalog": "Κατάλογος στένσιλ…",
            "continueToCanvas": "Κλείσιμο & συνέχεια",
            "skipStartOnLaunch": "Άνοιγμα απευθείας σε κενό φύλλο κατά την εκκίνηση",
            "rotateLeft": "Περιστροφή 90° αριστερά",
            "rotateRight": "Περιστροφή 90° δεξιά",
            "flipH": "Οριζόντια αναστροφή",
            "flipV": "Κατακόρυφη αναστροφή",
            "pageSize": "Μέγεθος σελίδας",
            "orientation": "Προσανατολισμός",
            "landscape": "Οριζόντια",
            "portrait": "Κατακόρυφα",
            "units": "Μονάδες",
            "osStandard": "Προεπιλογή συστήματος",
            "checkForUpdates": "Έλεγχος για ενημερώσεις…",
            "autoUpdates": "Αυτόματος έλεγχος για ενημερώσεις",
            "language": "Γλώσσα",
            "doubleClickRenameHint": "Διπλό κλικ για μετονομασία",
            "editLabelPlaceholder": "Πληκτρολογήστε κείμενο…",
            "done": "Τέλος",
            "cancel": "Ακύρωση",
            "stencils": "Στένσιλ",
            "pointer": "Επιλογή",
            "connector": "Γραμμή σύνδεσης",
            "text": "Κείμενο",
            "routing": "Δρομολόγηση",
            "autoLayout": "Αυτόματη διάταξη",
            "validate": "Έλεγχος",
            "shapeOps": "Πράξεις σχημάτων",
            "shapeSheet": "ShapeSheet",
            "inspectorFormat": "Μορφή",
            "inspectorSizePos": "Μέγεθος",
            "inspectorData": "Δεδομένα",
            "inspectorLayers": "Επίπεδα",
            "pageInspector": "Ιδιότητες σελίδας",
            "objectName": "Όνομα",
            "textLabel": "Κείμενο",
            "fillSection": "Γέμισμα",
            "noFill": "Χωρίς γέμισμα",
            "solidFill": "Συμπαγές",
            "gradientFill": "Διαβάθμιση",
            "transparency": "Διαφάνεια",
            "cornerRounding": "Στρογγύλευση",
            "lineArrowsSection": "Γραμμή & Βέλη",
            "noLine": "Χωρίς γραμμή",
            "solidLine": "Συνεχής",
            "dashedLine": "Διακεκομμένη",
            "dottedLine": "Διάστικτη",
            "lineWeight": "Πάχος",
            "beginArrow": "Βέλος αρχής",
            "endArrow": "Βέλος τέλους",
            "effectsSection": "Εφέ",
            "sizeAndPosition": "Μέγεθος & Θέση",
            "width": "Πλάτος",
            "height": "Ύψος",
            "angle": "Γωνία",
            "pinAnchor": "Σημείο αναφοράς",
            "protection": "Προστασία",
            "lockAspect": "Κλείδωμα αναλογίας",
            "lockSize": "Κλείδωμα μεγέθους",
            "lockPosition": "Κλείδωμα θέσης",
            "lockRotation": "Κλείδωμα περιστροφής",
            "lockDelete": "Κλείδωμα διαγραφής",
            "hideText": "Απόκρυψη κειμένου",
            "noShapeData": "Δεν έχουν οριστεί δεδομένα σχήματος.",
            "addDataField": "Προσθήκη πεδίου",
            "fieldName": "Όνομα πεδίου",
            "fieldValue": "Τιμή",
            "layerMembership": "Επίπεδα αντικειμένου",
            "pageLayers": "Επίπεδα σελίδας",
            "addLayer": "Προσθήκη επιπέδου",
            "newLayerPlaceholder": "Όνομα επιπέδου…",
            "openShapeSheet": "Άνοιγμα ShapeSheet…",
        ],
    ]
}
