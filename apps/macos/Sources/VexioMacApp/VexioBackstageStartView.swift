import AppKit
import SwiftUI
import UniformTypeIdentifiers

// MARK: - Focused In-App File Open & Startup Hub (Clean Open, Recent, New Blank & Starter Templates)

public struct VexioBackstageStartView: View {
    @ObservedObject public var session: VexioDocumentSession
    @ObservedObject public var i18n: VexioLocalization
    @ObservedObject public var updater: VexioUpdateManager
    @Binding public var isPresented: Bool

    @AppStorage("vexio.skipStartScreenOnLaunch") private var skipStartScreenOnLaunch: Bool = false
    @Environment(\.undoManager) private var undoManager

    public let onOpenLocalFileSheet: () -> Void
    public let onOpenURLDirectly: (URL) -> Void

    @State private var isDropTargeted: Bool = false

    public init(
        session: VexioDocumentSession,
        i18n: VexioLocalization = .shared,
        updater: VexioUpdateManager = .shared,
        isPresented: Binding<Bool>,
        onOpenLocalFileSheet: @escaping () -> Void,
        onOpenURLDirectly: @escaping (URL) -> Void
    ) {
        self.session = session
        self.i18n = i18n
        self.updater = updater
        self._isPresented = isPresented
        self.onOpenLocalFileSheet = onOpenLocalFileSheet
        self.onOpenURLDirectly = onOpenURLDirectly
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
                ("flowchart", "Flussdiagramm", "Prozess-, Entscheidungs- & Datenfluss", "arrow.triangle.branch"),
                ("bpmn", "BPMN 2.0 Prozess", "Gateways, Aufgaben & Sequenzflüsse", "point.3.connected.trianglepath.dotted"),
                ("network", "Netzwerk & Rack", "Firewalls, Switches, AD & Cloud", "server.rack"),
                ("floorplan", "Grundriss", "Räume, Wände & Maßstab", "ruler"),
                ("orgchart", "Organigramm", "Führungs- & Teamstruktur", "person.3.sequence"),
                ("uml", "UML 2.5 Architektur", "Klassen, Schnittstellen & Pakete", "square.stack.3d.up"),
            ]
        case .it:
            return [
                ("flowchart", "Diagramma di flusso", "Processi, decisioni e flussi dati", "arrow.triangle.branch"),
                ("bpmn", "Processo BPMN 2.0", "Gateway, attività e sequenze", "point.3.connected.trianglepath.dotted"),
                ("network", "Rete e Server Rack", "Firewall, switch, AD e cloud", "server.rack"),
                ("floorplan", "Planimetria", "Stanze, pareti e scala", "ruler"),
                ("orgchart", "Organigramma", "Struttura gerarchica e team", "person.3.sequence"),
                ("uml", "Architettura UML 2.5", "Classi, interfacce e componenti", "square.stack.3d.up"),
            ]
        case .fr:
            return [
                ("flowchart", "Flux de processus", "Processus, décisions et données", "arrow.triangle.branch"),
                ("bpmn", "Processus BPMN 2.0", "Passerelles, tâches et flux", "point.3.connected.trianglepath.dotted"),
                ("network", "Réseau et baie", "Pare-feu, switchs, AD et cloud", "server.rack"),
                ("floorplan", "Plan d’étage", "Pièces, murs et échelle", "ruler"),
                ("orgchart", "Organigramme", "Direction et équipes", "person.3.sequence"),
                ("uml", "Architecture UML 2.5", "Classes, interfaces et composants", "square.stack.3d.up"),
            ]
        case .es:
            return [
                ("flowchart", "Diagrama de flujo", "Procesos, decisiones y datos", "arrow.triangle.branch"),
                ("bpmn", "Proceso BPMN 2.0", "Compuertas, tareas y secuencias", "point.3.connected.trianglepath.dotted"),
                ("network", "Red y rack", "Firewalls, switches, AD y nube", "server.rack"),
                ("floorplan", "Plano de planta", "Salas, muros y escala", "ruler"),
                ("orgchart", "Organigrama", "Dirección y estructura de equipos", "person.3.sequence"),
                ("uml", "Arquitectura UML 2.5", "Clases, interfaces y componentes", "square.stack.3d.up"),
            ]
        case .el:
            return [
                ("flowchart", "Διάγραμμα ροής", "Διαδικασίες, αποφάσεις & ροή", "arrow.triangle.branch"),
                ("bpmn", "Διαδικασία BPMN 2.0", "Πύλες, εργασίες & ακολουθίες", "point.3.connected.trianglepath.dotted"),
                ("network", "Δίκτυο & Rack", "Firewalls, switches, AD & cloud", "server.rack"),
                ("floorplan", "Κάτοψη χώρου", "Χώροι, τοίχοι & κλίμακα", "ruler"),
                ("orgchart", "Οργανόγραμμα", "Διοίκηση & δομή ομάδων", "person.3.sequence"),
                ("uml", "Αρχιτεκτονική UML 2.5", "Κλάσεις, διεπαφές & στοιχεία", "square.stack.3d.up"),
            ]
        case .en:
            return [
                ("flowchart", "Basic Flowchart", "Process, decision & data flow", "arrow.triangle.branch"),
                ("bpmn", "BPMN 2.0 Process", "Gateways, tasks & sequence flows", "point.3.connected.trianglepath.dotted"),
                ("network", "Network & Server Rack", "Firewalls, switches, AD & cloud", "server.rack"),
                ("floorplan", "Architectural Floor Plan", "Rooms, walls & scaled layout", "ruler"),
                ("orgchart", "Organization Chart", "Executive & team reporting tree", "person.3.sequence"),
                ("uml", "UML 2.5 Architecture", "Classes, interfaces & components", "square.stack.3d.up"),
            ]
        }
    }

    private var dropZoneHint: String {
        switch i18n.language {
        case .de: return ".vsdx-, .vsd-, .dwg- oder .dxf-Datei hier ablegen"
        case .it: return "Rilascia qui un file .vsdx, .vsd, .dwg o .dxf"
        case .fr: return "Déposez un fichier .vsdx, .vsd, .dwg ou .dxf ici"
        case .es: return "Suelte aquí un archivo .vsdx, .vsd, .dwg o .dxf"
        case .el: return "Αποθέστε εδώ αρχείο .vsdx, .vsd, .dwg ή .dxf"
        case .en: return "Drop a .vsdx, .vsd, .dwg, or .dxf file here"
        }
    }

    public var body: some View {
        ZStack {
            Color.black.opacity(0.28)
                .ignoresSafeArea()
                .onTapGesture {
                    dismissHub()
                }

            VStack(spacing: 0) {
                headerBar
                Divider()
                HStack(alignment: .top, spacing: 0) {
                    openAndRecentColumn
                    Divider()
                    createNewAndTemplatesColumn
                }
                Divider()
                footerBar
            }
            .frame(width: 880, height: 540)
            .liquidGlassSurface(cornerRadius: 20, elevation: .quickPopover)
            .shadow(color: .black.opacity(0.24), radius: 28, y: 12)
        }
    }

    private func dismissHub() {
        withAnimation(LiquidGlassPhysics.standardSpring) {
            isPresented = false
        }
    }

    // MARK: - Header

    private var headerBar: some View {
        HStack(spacing: 14) {
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(
                    LinearGradient(
                        colors: [Color(red: 0.14, green: 0.39, blue: 0.92), Color(red: 0.09, green: 0.25, blue: 0.68)],
                        startPoint: .topLeading,
                        endPoint: .bottomTrailing
                    )
                )
                .frame(width: 38, height: 38)
                .overlay(
                    Text("V")
                        .font(.system(size: 20, weight: .heavy, design: .rounded))
                        .foregroundStyle(.white)
                )

            VStack(alignment: .leading, spacing: 2) {
                Text(i18n.t("welcomeTitle"))
                    .font(.system(size: 18, weight: .bold))
                    .lineLimit(1)
                Text(i18n.t("welcomeSubtitle"))
                    .font(.system(size: 11.5))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            Spacer(minLength: 12)

            Picker("", selection: $i18n.language) {
                ForEach(VexioAppLanguage.allCases) { lang in
                    Text(lang.nativeDisplayName).tag(lang)
                }
            }
            .labelsHidden()
            .pickerStyle(.menu)
            .frame(width: 148)
            .help(i18n.t("language"))

            Button(i18n.t("continueToCanvas")) {
                dismissHub()
            }
            .buttonStyle(.bordered)
            .fixedSize(horizontal: true, vertical: false)
        }
        .padding(.horizontal, 22)
        .padding(.vertical, 16)
    }

    // MARK: - Left Column: Open File, Drop Zone & Recent Files

    private var openAndRecentColumn: some View {
        VStack(alignment: .leading, spacing: 12) {
            Button {
                onOpenLocalFileSheet()
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: "folder.badge.plus")
                        .font(.system(size: 14, weight: .semibold))
                    Text(i18n.t("openFile"))
                        .font(.system(size: 12.5, weight: .semibold))
                        .lineLimit(1)
                        .minimumScaleFactor(0.9)
                    Spacer(minLength: 4)
                    Text("⌘O")
                        .font(.system(size: 10.5, weight: .medium, design: .monospaced))
                        .foregroundStyle(.white.opacity(0.85))
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)

            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .strokeBorder(
                    isDropTargeted ? Color.accentColor : Color.secondary.opacity(0.25),
                    style: StrokeStyle(lineWidth: 1.4, dash: [5, 4])
                )
                .background(
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .fill(isDropTargeted ? Color.accentColor.opacity(0.08) : Color.primary.opacity(0.02))
                )
                .frame(height: recentURLs.isEmpty ? 130 : 68)
                .overlay(
                    HStack(spacing: 8) {
                        Image(systemName: "arrow.down.doc")
                            .font(.system(size: 16, weight: .medium))
                            .foregroundStyle(isDropTargeted ? Color.accentColor : .secondary)
                        Text(dropZoneHint)
                            .font(.system(size: 11))
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.leading)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .padding(.horizontal, 12)
                )

            Text(i18n.t("recentFiles").uppercased())
                .font(.system(size: 10.5, weight: .bold))
                .foregroundStyle(.secondary)
                .padding(.top, 2)

            if recentURLs.isEmpty {
                Text("—")
                    .font(.system(size: 11.5))
                    .foregroundStyle(.tertiary)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 5) {
                        ForEach(recentURLs.prefix(10), id: \.path) { url in
                            Button {
                                isPresented = false
                                onOpenURLDirectly(url)
                            } label: {
                                HStack(spacing: 8) {
                                    Image(systemName: "doc.richtext")
                                        .font(.system(size: 13))
                                        .foregroundStyle(Color.accentColor)
                                    VStack(alignment: .leading, spacing: 1.5) {
                                        Text(url.lastPathComponent)
                                            .font(.system(size: 12, weight: .medium))
                                            .lineLimit(1)
                                            .truncationMode(.middle)
                                        Text(url.deletingLastPathComponent().path)
                                            .font(.system(size: 9.5))
                                            .foregroundStyle(.secondary)
                                            .lineLimit(1)
                                            .truncationMode(.head)
                                    }
                                    Spacer(minLength: 0)
                                }
                                .padding(.horizontal, 8)
                                .padding(.vertical, 6)
                                .background(
                                    RoundedRectangle(cornerRadius: 7, style: .continuous)
                                        .fill(Color.primary.opacity(0.04))
                                )
                            }
                            .buttonStyle(.plain)
                            .help(url.path)
                        }
                    }
                }
            }

            Spacer(minLength: 0)
        }
        .padding(20)
        .frame(width: 312)
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
    }

    // MARK: - Right Column: Single New Blank Diagram Hero + Starter Templates

    private var createNewAndTemplatesColumn: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Button {
                    session.createNewBlankDiagram()
                    dismissHub()
                } label: {
                    HStack(spacing: 14) {
                        ZStack {
                            RoundedRectangle(cornerRadius: 10, style: .continuous)
                                .fill(Color.accentColor.opacity(0.16))
                                .frame(width: 44, height: 44)
                            Image(systemName: "doc.badge.plus")
                                .font(.system(size: 20, weight: .semibold))
                                .foregroundStyle(Color.accentColor)
                        }

                        VStack(alignment: .leading, spacing: 3) {
                            Text(i18n.t("newBlankDiagram"))
                                .font(.system(size: 14, weight: .bold))
                                .foregroundStyle(.primary)
                                .lineLimit(1)
                            Text(i18n.t("newBlankSubtitle"))
                                .font(.system(size: 11.5))
                                .foregroundStyle(.secondary)
                                .fixedSize(horizontal: false, vertical: true)
                        }

                        Spacer(minLength: 8)

                        Image(systemName: "arrow.right.circle.fill")
                            .font(.system(size: 18))
                            .foregroundStyle(Color.accentColor)
                    }
                    .padding(14)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .fill(Color.accentColor.opacity(0.07))
                    )
                    .overlay(
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .strokeBorder(Color.accentColor.opacity(0.30), lineWidth: 1.1)
                    )
                }
                .buttonStyle(.plain)

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
                            session.fitViewportToActivePage()
                            dismissHub()
                        } label: {
                            VStack(alignment: .leading, spacing: 6) {
                                Image(systemName: tpl.icon)
                                    .font(.system(size: 18, weight: .semibold))
                                    .foregroundStyle(Color.accentColor)
                                Text(tpl.title)
                                    .font(.system(size: 12.5, weight: .semibold))
                                    .foregroundStyle(.primary)
                                    .fixedSize(horizontal: false, vertical: true)
                                Text(tpl.subtitle)
                                    .font(.system(size: 10.5))
                                    .foregroundStyle(.secondary)
                                    .lineLimit(2)
                                    .fixedSize(horizontal: false, vertical: true)
                                Spacer(minLength: 0)
                            }
                            .padding(12)
                            .frame(maxWidth: .infinity, minHeight: 96, alignment: .topLeading)
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

    // MARK: - Footer

    private var footerBar: some View {
        HStack(spacing: 16) {
            Toggle(i18n.t("skipStartOnLaunch"), isOn: $skipStartScreenOnLaunch)
                .font(.system(size: 11.5))

            Spacer()

            Button {
                Task { await updater.checkForUpdates(userInitiated: true) }
            } label: {
                Label(
                    updater.updateAvailable ? "Update v\(updater.latestVersionTag)!" : i18n.t("checkForUpdates"),
                    systemImage: updater.updateAvailable ? "arrow.down.circle.fill" : "arrow.triangle.2.circlepath"
                )
                .font(.system(size: 11.5, weight: .medium))
                .fixedSize(horizontal: true, vertical: false)
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
        }
        .padding(.horizontal, 22)
        .padding(.vertical, 11)
    }
}
