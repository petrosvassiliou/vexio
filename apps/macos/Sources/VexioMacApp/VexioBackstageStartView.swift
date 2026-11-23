import AppKit
import SwiftUI
import UniformTypeIdentifiers

// MARK: - In-App Welcome & File Backstage Hub (Replaces External OS Launch Dialog)

public struct VexioBackstageStartView: View {
    @ObservedObject public var session: VexioDocumentSession
    @ObservedObject public var i18n: VexioLocalization
    @ObservedObject public var updater: VexioUpdateManager
    @Binding public var isPresented: Bool

    @AppStorage("vexio.skipStartScreenOnLaunch") private var skipStartScreenOnLaunch: Bool = false
    @Environment(\.undoManager) private var undoManager

    public let onOpenLocalFileSheet: () -> Void
    public let onOpenURLDirectly: (URL) -> Void
    public let onImportStencilSheet: () -> Void
    public let onSaveSheet: () -> Void
    public let onSaveAsSheet: () -> Void
    public let onExportFormat: (VexioExportFormat) -> Void

    @State private var isDropTargeted: Bool = false

    public init(
        session: VexioDocumentSession,
        i18n: VexioLocalization = .shared,
        updater: VexioUpdateManager = .shared,
        isPresented: Binding<Bool>,
        onOpenLocalFileSheet: @escaping () -> Void,
        onOpenURLDirectly: @escaping (URL) -> Void,
        onImportStencilSheet: @escaping () -> Void,
        onSaveSheet: @escaping () -> Void,
        onSaveAsSheet: @escaping () -> Void,
        onExportFormat: @escaping (VexioExportFormat) -> Void
    ) {
        self.session = session
        self.i18n = i18n
        self.updater = updater
        self._isPresented = isPresented
        self.onOpenLocalFileSheet = onOpenLocalFileSheet
        self.onOpenURLDirectly = onOpenURLDirectly
        self.onImportStencilSheet = onImportStencilSheet
        self.onSaveSheet = onSaveSheet
        self.onSaveAsSheet = onSaveAsSheet
        self.onExportFormat = onExportFormat
    }

    private var recentURLs: [URL] {
        NSDocumentController.shared.recentDocumentURLs.filter {
            FileManager.default.fileExists(atPath: $0.path)
        }
    }

    private var starterTemplates: [(id: String, title: String, subtitle: String, icon: String)] {
        switch i18n.language {
        case .de:
            return [
                ("flowchart", "Standard-Flussdiagramm", "Prozess-, Entscheidungs- & Datenfluss", "arrow.triangle.branch"),
                ("bpmn", "BPMN-2.0-Prozess", "Gateways, Aufgaben & Sequenzflüsse", "point.3.connected.trianglepath.dotted"),
                ("network", "Netzwerk- & Server-Rack", "Firewalls, Switches, AD & M365", "server.rack"),
                ("floorplan", "Architektur-Grundriss", "Metrische Räume, Racks & Wände", "ruler"),
                ("orgchart", "Organigramm", "Führungs- & Teamstruktur", "person.3.sequence"),
                ("uml", "UML-2.5-Architektur", "Klassen, Schnittstellen & Komponenten", "square.stack.3d.up"),
            ]
        case .it:
            return [
                ("flowchart", "Diagramma di flusso", "Processi, decisioni e flussi dati", "arrow.triangle.branch"),
                ("bpmn", "Processo BPMN 2.0", "Gateway, attività e flussi di sequenza", "point.3.connected.trianglepath.dotted"),
                ("network", "Rete e Server Rack", "Firewall, switch, AD e M365", "server.rack"),
                ("floorplan", "Planimetria architettonica", "Stanze e pareti in scala metrica", "ruler"),
                ("orgchart", "Organigramma", "Struttura gerarchica aziendale", "person.3.sequence"),
                ("uml", "Architettura UML 2.5", "Classi, interfacce e componenti", "square.stack.3d.up"),
            ]
        case .fr:
            return [
                ("flowchart", "Organigramme de flux", "Processus, décisions et flux de données", "arrow.triangle.branch"),
                ("bpmn", "Processus BPMN 2.0", "Passerelles, tâches et flux séquentiels", "point.3.connected.trianglepath.dotted"),
                ("network", "Réseau et baie serveur", "Pare-feu, commutateurs, AD et M365", "server.rack"),
                ("floorplan", "Plan d’étage architectural", "Pièces et murs à l’échelle métrique", "ruler"),
                ("orgchart", "Organigramme hiérarchique", "Direction et équipes", "person.3.sequence"),
                ("uml", "Architecture UML 2.5", "Classes, interfaces et composants", "square.stack.3d.up"),
            ]
        case .es:
            return [
                ("flowchart", "Diagrama de flujo", "Procesos, decisiones y flujo de datos", "arrow.triangle.branch"),
                ("bpmn", "Proceso BPMN 2.0", "Compuertas, tareas y flujos de secuencia", "point.3.connected.trianglepath.dotted"),
                ("network", "Red y rack de servidores", "Firewalls, switches, AD y M365", "server.rack"),
                ("floorplan", "Plano arquitectónico", "Salas y muros a escala métrica", "ruler"),
                ("orgchart", "Organigrama", "Estructura directiva y de equipos", "person.3.sequence"),
                ("uml", "Arquitectura UML 2.5", "Clases, interfaces y componentes", "square.stack.3d.up"),
            ]
        case .el:
            return [
                ("flowchart", "Διάγραμμα ροής", "Διαδικασίες, αποφάσεις & ροή δεδομένων", "arrow.triangle.branch"),
                ("bpmn", "Διαδικασία BPMN 2.0", "Πύλες, εργασίες & ροές ακολουθίας", "point.3.connected.trianglepath.dotted"),
                ("network", "Δίκτυο & Server Rack", "Τείχη προστασίας, switches, AD & M365", "server.rack"),
                ("floorplan", "Αρχιτεκτονική κάτοψη", "Χώροι & τοίχοι σε μετρική κλίμακα", "ruler"),
                ("orgchart", "Οργανόγραμμα", "Διοικητική ιεραρχία & ομάδες", "person.3.sequence"),
                ("uml", "Αρχιτεκτονική UML 2.5", "Κλάσεις, διεπαφές & στοιχεία", "square.stack.3d.up"),
            ]
        case .en:
            return [
                ("flowchart", "Basic Flowchart", "Process, decision & data flow", "arrow.triangle.branch"),
                ("bpmn", "BPMN 2.0 Process", "Gateways, tasks & sequence flows", "point.3.connected.trianglepath.dotted"),
                ("network", "Network & Server Rack", "Firewalls, switches, AD & M365", "server.rack"),
                ("floorplan", "Architectural Floor Plan", "Metric / scaled rooms & walls", "ruler"),
                ("orgchart", "Organization Chart", "Executive & team reporting tree", "person.3.sequence"),
                ("uml", "UML 2.5 Architecture", "Classes, interfaces & components", "square.stack.3d.up"),
            ]
        }
    }

    private var dropZoneHint: String {
        switch i18n.language {
        case .de: return ".vsdx-, .vssx-, .vsd- oder .dwg-Datei hier ablegen"
        case .it: return "Rilascia qui file .vsdx, .vssx, .vsd o .dwg"
        case .fr: return "Déposez un fichier .vsdx, .vssx, .vsd ou .dwg ici"
        case .es: return "Suelte aquí archivos .vsdx, .vssx, .vsd o .dwg"
        case .el: return "Αποθέστε εδώ αρχεία .vsdx, .vssx, .vsd ή .dwg"
        case .en: return "Drop .vsdx, .vssx, .vsd, or .dwg here"
        }
    }

    private var metricLabelWord: String {
        switch i18n.language {
        case .de: return "Metrisch"
        case .it: return "Metrico"
        case .fr: return "Métrique"
        case .es: return "Métrico"
        case .el: return "Μετρικό"
        case .en: return "Metric"
        }
    }

    private var imperialLabelWord: String {
        switch i18n.language {
        case .de: return "Zoll"
        case .it: return "Imperiale"
        case .fr: return "Impérial"
        case .es: return "Imperial"
        case .el: return "Ίντσες"
        case .en: return "Imperial"
        }
    }

    public var body: some View {
        ZStack {
            Color.black.opacity(0.28)
                .ignoresSafeArea()
                .onTapGesture {
                    withAnimation(LiquidGlassPhysics.standardSpring) {
                        isPresented = false
                    }
                }

            VStack(spacing: 0) {
                // Header Bar
                HStack(spacing: 14) {
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .fill(
                            LinearGradient(
                                colors: [Color(red: 0.14, green: 0.39, blue: 0.92), Color(red: 0.09, green: 0.25, blue: 0.68)],
                                startPoint: .topLeading,
                                endPoint: .bottomTrailing
                            )
                        )
                        .frame(width: 40, height: 40)
                        .overlay(
                            Text("V")
                                .font(.system(size: 21, weight: .heavy, design: .rounded))
                                .foregroundStyle(.white)
                        )

                    VStack(alignment: .leading, spacing: 2) {
                        Text(i18n.t("welcomeTitle"))
                            .font(.system(size: 19, weight: .bold))
                        Text(i18n.t("welcomeSubtitle"))
                            .font(.system(size: 12))
                            .foregroundStyle(.secondary)
                    }

                    Spacer()

                    // Quick Language Selector right inside Welcome / File Backstage
                    Picker(i18n.t("language"), selection: $i18n.language) {
                        ForEach(VexioAppLanguage.allCases) { lang in
                            Text(lang.nativeDisplayName).tag(lang)
                        }
                    }
                    .pickerStyle(.menu)
                    .frame(width: 165)

                    Button(i18n.t("continueToCanvas")) {
                        withAnimation(LiquidGlassPhysics.standardSpring) {
                            isPresented = false
                        }
                    }
                    .buttonStyle(.borderedProminent)
                }
                .padding(.horizontal, 24)
                .padding(.vertical, 18)

                Divider()

                // Main Content Columns: Left Actions & Recent | Right Blank Page Presets & Templates
                HStack(alignment: .top, spacing: 0) {
                    // Left Column: Open, Save, Save As, Import Stencil, Recent Files
                    VStack(alignment: .leading, spacing: 10) {
                        Button {
                            onOpenLocalFileSheet()
                        } label: {
                            HStack(spacing: 8) {
                                Image(systemName: "folder.badge.plus")
                                    .font(.system(size: 14, weight: .semibold))
                                Text(i18n.t("openFile"))
                                    .font(.system(size: 12.5, weight: .semibold))
                                    .lineLimit(1)
                                Spacer(minLength: 4)
                                Text("⌘O")
                                    .font(.system(size: 10.5, design: .monospaced))
                                    .foregroundStyle(.white.opacity(0.85))
                            }
                            .padding(.horizontal, 10)
                            .padding(.vertical, 8)
                            .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.borderedProminent)

                        Button {
                            onImportStencilSheet()
                        } label: {
                            HStack(spacing: 8) {
                                Image(systemName: "square.grid.3x1.folder.badge.plus")
                                Text(i18n.t("importStencil"))
                                    .lineLimit(1)
                                Spacer(minLength: 4)
                                Text(".vssx/.vss")
                                    .font(.system(size: 10, design: .monospaced))
                                    .foregroundStyle(.secondary)
                            }
                            .padding(.horizontal, 10)
                            .padding(.vertical, 6)
                            .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.bordered)

                        HStack(spacing: 8) {
                            Button {
                                onSaveSheet()
                            } label: {
                                HStack(spacing: 6) {
                                    Image(systemName: "square.and.arrow.down")
                                    Text(i18n.t("saveFile"))
                                        .lineLimit(1)
                                    Spacer(minLength: 2)
                                    Text("⌘S")
                                        .font(.system(size: 10, design: .monospaced))
                                        .foregroundStyle(.secondary)
                                }
                                .padding(.horizontal, 8)
                                .padding(.vertical, 6)
                                .frame(maxWidth: .infinity)
                            }
                            .buttonStyle(.bordered)

                            Button {
                                onSaveAsSheet()
                            } label: {
                                HStack(spacing: 6) {
                                    Image(systemName: "square.and.arrow.down.on.square")
                                    Text(i18n.t("saveAs"))
                                        .lineLimit(1)
                                    Spacer(minLength: 2)
                                    Text("⇧⌘S")
                                        .font(.system(size: 10, design: .monospaced))
                                        .foregroundStyle(.secondary)
                                }
                                .padding(.horizontal, 8)
                                .padding(.vertical, 6)
                                .frame(maxWidth: .infinity)
                            }
                            .buttonStyle(.bordered)
                        }

                        Menu {
                            ForEach(VexioExportFormat.allCases) { fmt in
                                Button(fmt.displayName) {
                                    isPresented = false
                                    onExportFormat(fmt)
                                }
                            }
                        } label: {
                            HStack {
                                Image(systemName: "square.and.arrow.up")
                                Text(i18n.t("export"))
                                    .lineLimit(1)
                                Spacer()
                            }
                            .frame(maxWidth: .infinity)
                        }
                        .controlSize(.regular)

                        Divider().padding(.vertical, 4)

                        Text(i18n.t("recentFiles").uppercased())
                            .font(.system(size: 10.5, weight: .bold))
                            .foregroundStyle(.secondary)

                        if recentURLs.isEmpty {
                            RoundedRectangle(cornerRadius: 10, style: .continuous)
                                .strokeBorder(
                                    isDropTargeted ? Color.accentColor : Color.secondary.opacity(0.25),
                                    style: StrokeStyle(lineWidth: 1.4, dash: [5, 4])
                                )
                                .background(
                                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                                        .fill(isDropTargeted ? Color.accentColor.opacity(0.08) : Color.clear)
                                )
                                .frame(height: 110)
                                .overlay(
                                    VStack(spacing: 6) {
                                        Image(systemName: "arrow.down.doc")
                                            .font(.system(size: 20))
                                            .foregroundStyle(.secondary)
                                        Text(dropZoneHint)
                                            .font(.system(size: 11))
                                            .foregroundStyle(.secondary)
                                            .multilineTextAlignment(.center)
                                    }
                                    .padding(8)
                                )
                        } else {
                            ScrollView {
                                VStack(alignment: .leading, spacing: 5) {
                                    ForEach(recentURLs.prefix(8), id: \.path) { url in
                                        Button {
                                            isPresented = false
                                            onOpenURLDirectly(url)
                                        } label: {
                                            HStack(spacing: 8) {
                                                Image(systemName: "doc.richtext")
                                                    .foregroundStyle(Color.accentColor)
                                                VStack(alignment: .leading, spacing: 1) {
                                                    Text(url.lastPathComponent)
                                                        .font(.system(size: 12, weight: .medium))
                                                        .lineLimit(1)
                                                    Text(url.deletingLastPathComponent().path)
                                                        .font(.system(size: 9.5))
                                                        .foregroundStyle(.secondary)
                                                        .lineLimit(1)
                                                }
                                                Spacer()
                                            }
                                            .padding(6)
                                            .background(RoundedRectangle(cornerRadius: 7).fill(Color.primary.opacity(0.04)))
                                        }
                                        .buttonStyle(.plain)
                                    }
                                }
                            }
                            .frame(maxHeight: 165)
                        }

                        Spacer()
                    }
                    .padding(20)
                    .frame(width: 336)
                    .onDrop(of: [.fileURL], isTargeted: $isDropTargeted) { providers in
                        guard let first = providers.first else { return false }
                        _ = first.loadObject(ofClass: URL.self) { droppedURL, _ in
                            guard let droppedURL else { return }
                            Task { @MainActor in
                                isPresented = false
                                onOpenURLDirectly(droppedURL)
                            }
                        }
                        return true
                    }

                    Divider()

                    // Right Column: Blank Metric/Imperial Sheets + Starter Templates
                    ScrollView {
                        VStack(alignment: .leading, spacing: 16) {
                            Text(i18n.t("pageSize").uppercased())
                                .font(.system(size: 10.5, weight: .bold))
                                .foregroundStyle(.secondary)

                            LazyVGrid(
                                columns: [
                                    GridItem(.flexible(), spacing: 10),
                                    GridItem(.flexible(), spacing: 10),
                                    GridItem(.flexible(), spacing: 10),
                                ],
                                spacing: 10
                            ) {
                                blankSheetCard(
                                    title: i18n.t("newBlankA4"),
                                    subtitle: "297 × 210 mm • \(metricLabelWord)",
                                    preset: .a4,
                                    badge: i18n.unitPreference.isMetric ? "DEFAULT" : "ISO"
                                )
                                blankSheetCard(
                                    title: i18n.t("newBlankA3"),
                                    subtitle: "420 × 297 mm • \(metricLabelWord)",
                                    preset: .a3,
                                    badge: "ISO"
                                )
                                blankSheetCard(
                                    title: i18n.t("newBlankLetter"),
                                    subtitle: "11 × 8.5 in • \(imperialLabelWord)",
                                    preset: .letter,
                                    badge: !i18n.unitPreference.isMetric ? "DEFAULT" : "ANSI"
                                )
                            }

                            Divider()

                            Text(i18n.t("templates").uppercased())
                                .font(.system(size: 10.5, weight: .bold))
                                .foregroundStyle(.secondary)

                            LazyVGrid(
                                columns: [
                                    GridItem(.flexible(), spacing: 10),
                                    GridItem(.flexible(), spacing: 10),
                                    GridItem(.flexible(), spacing: 10),
                                ],
                                spacing: 10
                            ) {
                                ForEach(starterTemplates, id: \.id) { tpl in
                                    Button {
                                        session.loadStarterTemplate(tpl.id, undoManager: undoManager)
                                        session.applyPagePreset(
                                            i18n.unitPreference.isMetric ? .a4 : .letter,
                                            landscape: true,
                                            undoManager: nil
                                        )
                                        withAnimation(LiquidGlassPhysics.standardSpring) {
                                            isPresented = false
                                        }
                                    } label: {
                                        VStack(alignment: .leading, spacing: 6) {
                                            HStack {
                                                Image(systemName: tpl.icon)
                                                    .font(.system(size: 18, weight: .semibold))
                                                    .foregroundStyle(Color.accentColor)
                                                Spacer()
                                            }
                                            Text(tpl.title)
                                                .font(.system(size: 12.5, weight: .semibold))
                                                .foregroundStyle(.primary)
                                                .lineLimit(1)
                                            Text(tpl.subtitle)
                                                .font(.system(size: 10.5))
                                                .foregroundStyle(.secondary)
                                                .lineLimit(2)
                                        }
                                        .padding(12)
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                        .background(
                                            RoundedRectangle(cornerRadius: 11, style: .continuous)
                                                .fill(Color.primary.opacity(0.045))
                                        )
                                        .overlay(
                                            RoundedRectangle(cornerRadius: 11, style: .continuous)
                                                .strokeBorder(Color.primary.opacity(0.09), lineWidth: 0.8)
                                        )
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                        }
                        .padding(20)
                    }
                }

                Divider()

                // Bottom Bar: Startup Preference, Units & Updates
                HStack(spacing: 16) {
                    Toggle(i18n.t("skipStartOnLaunch"), isOn: $skipStartScreenOnLaunch)
                        .font(.system(size: 11.5))

                    Spacer()

                    HStack(spacing: 6) {
                        Text("\(i18n.t("units")):")
                            .font(.system(size: 11.5))
                            .foregroundStyle(.secondary)
                        Picker("", selection: $i18n.unitPreference) {
                            Text("\(i18n.t("osStandard")) (\(VexioMeasurementUnit.osDefault.resolvedEngineSlug))").tag(VexioMeasurementUnit.osDefault)
                            Text("Millimeters (mm)").tag(VexioMeasurementUnit.mm)
                            Text("Centimeters (cm)").tag(VexioMeasurementUnit.cm)
                            Text("Meters (m)").tag(VexioMeasurementUnit.m)
                            Text("Inches (in)").tag(VexioMeasurementUnit.inch)
                        }
                        .labelsHidden()
                        .frame(width: 165)
                        .onChange(of: i18n.unitPreference) { _, newUnit in
                            session.applyMeasurementUnit(newUnit.resolvedEngineSlug, undoManager: undoManager)
                        }
                    }

                    Button {
                        Task { await updater.checkForUpdates(userInitiated: true) }
                    } label: {
                        Label(
                            updater.updateAvailable ? "Update v\(updater.latestVersionTag)!" : i18n.t("checkForUpdates"),
                            systemImage: updater.updateAvailable ? "arrow.down.circle.fill" : "arrow.triangle.2.circlepath"
                        )
                        .font(.system(size: 11.5, weight: .medium))
                    }
                    .buttonStyle(.bordered)
                }
                .padding(.horizontal, 24)
                .padding(.vertical, 12)
            }
            .frame(width: 920, height: 590)
            .liquidGlassSurface(cornerRadius: 20, elevation: .quickPopover)
            .shadow(color: .black.opacity(0.24), radius: 28, y: 12)
        }
    }

    @ViewBuilder
    private func blankSheetCard(title: String, subtitle: String, preset: VexioPagePreset, badge: String) -> some View {
        Button {
            session.resetToBlankSheet(
                preset: preset,
                landscape: true,
                unitSlug: preset == .letter ? "in" : "mm"
            )
            withAnimation(LiquidGlassPhysics.standardSpring) {
                isPresented = false
            }
        } label: {
            VStack(alignment: .leading, spacing: 7) {
                HStack {
                    Image(systemName: "doc.badge.plus")
                        .font(.system(size: 19, weight: .semibold))
                        .foregroundStyle(Color.accentColor)
                    Spacer()
                    Text(badge)
                        .font(.system(size: 9, weight: .bold, design: .monospaced))
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Capsule().fill(Color.accentColor.opacity(0.16)))
                }
                Text(title)
                    .font(.system(size: 12.5, weight: .semibold))
                    .foregroundStyle(.primary)
                    .lineLimit(2)
                Text(subtitle)
                    .font(.system(size: 10.5))
                    .foregroundStyle(.secondary)
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 11, style: .continuous)
                    .fill(Color.accentColor.opacity(0.06))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 11, style: .continuous)
                    .strokeBorder(Color.accentColor.opacity(0.25), lineWidth: 1.0)
            )
        }
        .buttonStyle(.plain)
    }
}
