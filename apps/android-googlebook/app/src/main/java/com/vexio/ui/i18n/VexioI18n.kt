package com.vexio.ui.i18n

import com.vexio.core.VexioAppLanguage
import com.vexio.core.VexioAppLanguage.DE
import com.vexio.core.VexioAppLanguage.EN
import com.vexio.core.VexioAppLanguage.ES
import com.vexio.core.VexioAppLanguage.FR
import com.vexio.core.VexioAppLanguage.GR
import com.vexio.core.VexioAppLanguage.IT
import com.vexio.core.VexioBooleanOp
import com.vexio.core.VexioCursorKind
import com.vexio.core.VexioPagePresetSpec
import com.vexio.core.VexioThemePreset
import com.vexio.core.VexioTool
import com.vexio.core.VexioVisualPreset

/**
 * Comprehensive 6-language (`EN`, `DE`, `IT`, `FR`, `ES`, `GR`) workstation localization helpers
 * covering Backstage, Header Menus, Ribbon Tabs & Actions, Stencils, Official Stencil Catalog,
 * ShapeSheet, Inspector, Context Menus, Status Bar, Color Picker, and Update/Offline statuses.
 */
object VexioI18n {

    fun suiteSubtitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "1:1 Visio & Offline CAD Suite"
        DE -> "1:1-Visio- & Offline-CAD-Suite"
        IT -> "Suite 1:1 Visio & CAD offline"
        FR -> "Suite 1:1 Visio & CAO hors ligne"
        ES -> "Suite 1:1 de Visio y CAD sin conexión"
        GR -> "Σουίτα 1:1 Visio & Offline CAD"
    }

    fun blankIsoA4Short(lang: VexioAppLanguage): String = when (lang) {
        EN -> "+ Blank ISO A4 (mm)"
        DE -> "+ Leeres ISO A4 (mm)"
        IT -> "+ ISO A4 vuoto (mm)"
        FR -> "+ ISO A4 vierge (mm)"
        ES -> "+ ISO A4 en blanco (mm)"
        GR -> "+ Κενό ISO A4 (mm)"
    }

    fun printPdfLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Print / PDF (Ctrl+P)"
        DE -> "Drucken / PDF (Ctrl+P)"
        IT -> "Stampa / PDF (Ctrl+P)"
        FR -> "Imprimer / PDF (Ctrl+P)"
        ES -> "Imprimir / PDF (Ctrl+P)"
        GR -> "Εκτύπωση / PDF (Ctrl+P)"
    }

    fun startHeroSubtitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Create a blank ISO Metric (A4 / A3) or US Letter page, start from a 1:1 Visio template, or open/import Visio (.vsdx, .vsd, .vssx, .vss) and CAD (.dwg, .dxf) files directly inside the app."
        DE -> "Erstellen Sie ein leeres metrisches ISO-Blatt (A4 / A3) oder US-Letter-Blatt, starten Sie mit einer 1:1-Visio-Vorlage oder öffnen bzw. importieren Sie Visio- (.vsdx, .vsd, .vssx, .vss) und CAD-Dateien (.dwg, .dxf) direkt in der App."
        IT -> "Crea una pagina metrica ISO vuota (A4 / A3) o US Letter, inizia da un modello Visio 1:1 oppure apri/importa file Visio (.vsdx, .vsd, .vssx, .vss) e CAD (.dwg, .dxf) direttamente nell’app."
        FR -> "Créez une page métrique ISO vierge (A4 / A3) ou US Letter, démarrez depuis un modèle Visio 1:1 ou ouvrez/importez des fichiers Visio (.vsdx, .vsd, .vssx, .vss) et CAO (.dwg, .dxf) directement dans l’application."
        ES -> "Cree una página métrica ISO en blanco (A4 / A3) o US Letter, comience desde una plantilla Visio 1:1 o abra/importe archivos Visio (.vsdx, .vsd, .vssx, .vss) y CAD (.dwg, .dxf) directamente en la aplicación."
        GR -> "Δημιουργήστε κενή μετρική σελίδα ISO (A4 / A3) ή US Letter, ξεκινήστε από πρότυπο Visio 1:1 ή ανοίξτε/εισαγάγετε αρχεία Visio (.vsdx, .vsd, .vssx, .vss) και CAD (.dwg, .dxf) απευθείας στην εφαρμογή."
    }

    fun blankPresetsHeading(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Blank Page Presets (Metric Default / OS Standard)"
        DE -> "Leere Zeichenblätter (Metrischer Standard / Systemstandard)"
        IT -> "Formati pagina vuoti (Metrico predefinito / Standard SO)"
        FR -> "Formats de page vierge (Métrique par défaut / Standard OS)"
        ES -> "Formatos de página en blanco (Métrico predeterminado / Estándar SO)"
        GR -> "Πρότυπα κενής σελίδας (Μετρικό προεπιλογή / Πρότυπο ΛΣ)"
    }

    fun blankA4LandscapeTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Blank ISO A4 Landscape"
        DE -> "Leeres ISO A4 Querformat"
        IT -> "ISO A4 vuoto Orizzontale"
        FR -> "ISO A4 vierge Paysage"
        ES -> "ISO A4 en blanco Horizontal"
        GR -> "Κενό ISO A4 Οριζόντια"
    }

    fun blankA3LandscapeTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Blank ISO A3 Landscape"
        DE -> "Leeres ISO A3 Querformat"
        IT -> "ISO A3 vuoto Orizzontale"
        FR -> "ISO A3 vierge Paysage"
        ES -> "ISO A3 en blanco Horizontal"
        GR -> "Κενό ISO A3 Οριζόντια"
    }

    fun blankA4PortraitTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Blank ISO A4 Portrait"
        DE -> "Leeres ISO A4 Hochformat"
        IT -> "ISO A4 vuoto Verticale"
        FR -> "ISO A4 vierge Portrait"
        ES -> "ISO A4 en blanco Vertical"
        GR -> "Κενό ISO A4 Κατακόρυφα"
    }

    fun blankUsLetterTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Blank US Letter"
        DE -> "Leeres US Letter"
        IT -> "US Letter vuoto"
        FR -> "US Letter vierge"
        ES -> "US Letter en blanco"
        GR -> "Κενό US Letter"
    }

    fun metricUnitSubtitle(dims: String, lang: VexioAppLanguage): String {
        val unitWord = when (lang) {
            EN -> "Metric"; DE -> "Metrisch"; IT -> "Metrico"; FR -> "Métrique"; ES -> "Métrico"; GR -> "Μετρικό"
        }
        return "$dims • $unitWord"
    }

    fun imperialUnitSubtitle(dims: String, lang: VexioAppLanguage): String {
        val unitWord = when (lang) {
            EN -> "Imperial"; DE -> "Zoll (Imperial)"; IT -> "Imperiale"; FR -> "Impérial"; ES -> "Imperial"; GR -> "ίντσες (Imperial)"
        }
        return "$dims • $unitWord"
    }

    fun starterTemplatesHeading(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Starter Diagram Templates"
        DE -> "Diagramm-Startvorlagen"
        IT -> "Modelli di diagramma iniziali"
        FR -> "Modèles de diagrammes de départ"
        ES -> "Plantillas iniciales de diagramas"
        GR -> "Αρχικά πρότυπα διαγραμμάτων"
    }

    fun templateTitle(slug: String, lang: VexioAppLanguage): String = when (slug) {
        "flowchart" -> when (lang) {
            EN -> "Flowchart Starter"; DE -> "Flussdiagramm"; IT -> "Diagramma di flusso"; FR -> "Organigramme de flux"; ES -> "Diagrama de flujo"; GR -> "Διάγραμμα ροής"
        }
        "bpmn" -> when (lang) {
            EN -> "BPMN 2.0 Process"; DE -> "BPMN-2.0-Prozess"; IT -> "Processo BPMN 2.0"; FR -> "Processus BPMN 2.0"; ES -> "Proceso BPMN 2.0"; GR -> "Διαδικασία BPMN 2.0"
        }
        "network" -> when (lang) {
            EN -> "Network & Rack Topology"; DE -> "Netzwerk- & Rack-Topologie"; IT -> "Topologia rete e rack"; FR -> "Topologie réseau et baie"; ES -> "Topología de red y rack"; GR -> "Τοπολογία δικτύου & Rack"
        }
        "floorplan" -> when (lang) {
            EN -> "CAD Floor Plan"; DE -> "CAD-Grundriss"; IT -> "Planimetria CAD"; FR -> "Plan d’étage CAO"; ES -> "Plano de planta CAD"; GR -> "Κάτοψη CAD"
        }
        "orgchart" -> when (lang) {
            EN -> "Organization Chart"; DE -> "Organigramm"; IT -> "Organigramma"; FR -> "Organigramme hiérarchique"; ES -> "Organigrama"; GR -> "Οργανόγραμμα"
        }
        "uml" -> when (lang) {
            EN -> "UML 2.5 Architecture"; DE -> "UML-2.5-Architektur"; IT -> "Architettura UML 2.5"; FR -> "Architecture UML 2.5"; ES -> "Arquitectura UML 2.5"; GR -> "Αρχιτεκτονική UML 2.5"
        }
        else -> slug
    }

    fun languageAndUpdatesHeading(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Language (EN, DE, IT, FR, ES, GR) & Updates"
        DE -> "Sprache (EN, DE, IT, FR, ES, GR) & Updates"
        IT -> "Lingua (EN, DE, IT, FR, ES, GR) e aggiornamenti"
        FR -> "Langue (EN, DE, IT, FR, ES, GR) et mises à jour"
        ES -> "Idioma (EN, DE, IT, FR, ES, GR) y actualizaciones"
        GR -> "Γλώσσα (EN, DE, IT, FR, ES, GR) & Ενημερώσεις"
    }

    fun autoCheckUpdatesCheckbox(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Automatically check for updates on startup"
        DE -> "Beim Start automatisch nach Updates suchen"
        IT -> "Controlla automaticamente gli aggiornamenti all’avvio"
        FR -> "Rechercher automatiquement les mises à jour au démarrage"
        ES -> "Buscar actualizaciones automáticamente al iniciar"
        GR -> "Αυτόματος έλεγχος για ενημερώσεις κατά την εκκίνηση"
    }

    fun localizeUpdateStatus(raw: String, availableTag: String?, lang: VexioAppLanguage): String {
        if ("Offline mode active" in raw) {
            return when (lang) {
                EN -> "Offline mode active • Update check skipped cleanly"
                DE -> "Offline-Modus aktiv • Update-Prüfung übersprungen"
                IT -> "Modalità offline attiva • Controllo aggiornamenti ignorato"
                FR -> "Mode hors ligne actif • Vérification des mises à jour ignorée"
                ES -> "Modo sin conexión activo • Comprobación de actualizaciones omitida"
                GR -> "Ενεργή λειτουργία εκτός σύνδεσης • Παραλείφθηκε ο έλεγχος ενημερώσεων"
            }
        }
        if ("Automatic update check enabled" in raw) {
            val ver = raw.substringAfter("Vexio ", "").substringBefore(" •").ifBlank { "v0.9.1-beta" }
            return when (lang) {
                EN -> "Vexio $ver • Automatic update check enabled"
                DE -> "Vexio $ver • Automatische Update-Prüfung aktiviert"
                IT -> "Vexio $ver • Controllo automatico aggiornamenti attivo"
                FR -> "Vexio $ver • Recherche automatique des mises à jour activée"
                ES -> "Vexio $ver • Búsqueda automática de actualizaciones activada"
                GR -> "Vexio $ver • Ενεργός αυτόματος έλεγχος ενημερώσεων"
            }
        }
        if ("Automatic update check disabled" in raw) {
            return when (lang) {
                EN -> "Automatic update check disabled"
                DE -> "Automatische Update-Prüfung deaktiviert"
                IT -> "Controllo automatico aggiornamenti disattivato"
                FR -> "Recherche automatique des mises à jour désactivée"
                ES -> "Búsqueda automática de actualizaciones desactivada"
                GR -> "Απενεργοποιήθηκε ο αυτόματος έλεγχος ενημερώσεων"
            }
        }
        if ("Vexio is up to date" in raw) {
            val tag = raw.substringAfter("(", "").substringBefore(")").ifBlank { "latest" }
            return when (lang) {
                EN -> "Vexio is up to date ($tag)"
                DE -> "Vexio ist auf dem neuesten Stand ($tag)"
                IT -> "Vexio è aggiornato ($tag)"
                FR -> "Vexio est à jour ($tag)"
                ES -> "Vexio está actualizado ($tag)"
                GR -> "Το Vexio είναι ενημερωμένο ($tag)"
            }
        }
        if (availableTag != null || "Update available:" in raw) {
            val tag = availableTag ?: raw.substringAfter("Update available: ").substringBefore(" ")
            return when (lang) {
                EN -> "Update available: $tag"
                DE -> "Update verfügbar: $tag"
                IT -> "Aggiornamento disponibile: $tag"
                FR -> "Mise à jour disponible : $tag"
                ES -> "Actualización disponible: $tag"
                GR -> "Διαθέσιμη ενημέρωση: $tag"
            }
        }
        return raw
    }

    // --- Header Bar, Menus & Dialogs ---

    fun editedSuffix(lang: VexioAppLanguage): String = when (lang) {
        EN -> " • Edited"; DE -> " • Bearbeitet"; IT -> " • Modificato"; FR -> " • Modifié"; ES -> " • Editado"; GR -> " • Τροποποιήθηκε"
    }

    fun startHubMenu(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Start / Templates Hub…"
        DE -> "Start- & Vorlagen-Übersicht…"
        IT -> "Hub iniziale / Modelli…"
        FR -> "Accueil / Modèles…"
        ES -> "Inicio / Centro de plantillas…"
        GR -> "Αρχική / Πρότυπα…"
    }

    fun exportMenuButton(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Export ▾"; DE -> "Exportieren ▾"; IT -> "Esporta ▾"; FR -> "Exporter ▾"; ES -> "Exportar ▾"; GR -> "Εξαγωγή ▾"
    }

    fun exportJpgItem(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Export High-DPI JPEG (.jpg)"
        DE -> "Als High-DPI-JPEG exportieren (.jpg)"
        IT -> "Esporta JPEG ad alta risoluzione (.jpg)"
        FR -> "Exporter en JPEG haute résolution (.jpg)"
        ES -> "Exportar JPEG de alta resolución (.jpg)"
        GR -> "Εξαγωγή σε High-DPI JPEG (.jpg)"
    }

    fun exportPdfItem(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Export Archival PDF/A (.pdf)"
        DE -> "Als Archivierungs-PDF/A exportieren (.pdf)"
        IT -> "Esporta PDF/A archivistico (.pdf)"
        FR -> "Exporter en PDF/A d’archivage (.pdf)"
        ES -> "Exportar PDF/A de archivo (.pdf)"
        GR -> "Εξαγωγή σε αρχειακό PDF/A (.pdf)"
    }

    fun exportSvgItem(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Export Vector SVG (.svg)"
        DE -> "Als Vektor-SVG exportieren (.svg)"
        IT -> "Esporta SVG vettoriale (.svg)"
        FR -> "Exporter en SVG vectoriel (.svg)"
        ES -> "Exportar SVG vectorial (.svg)"
        GR -> "Εξαγωγή σε διανυσματικό SVG (.svg)"
    }

    fun exportDxfItem(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Export AutoCAD DXF (.dxf)"
        DE -> "Als AutoCAD-DXF exportieren (.dxf)"
        IT -> "Esporta AutoCAD DXF (.dxf)"
        FR -> "Exporter en AutoCAD DXF (.dxf)"
        ES -> "Exportar AutoCAD DXF (.dxf)"
        GR -> "Εξαγωγή σε AutoCAD DXF (.dxf)"
    }

    fun updatesMenuButton(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Updates ▾"; DE -> "Updates ▾"; IT -> "Aggiorna ▾"; FR -> "Mises à jour ▾"; ES -> "Actualizar ▾"; GR -> "Ενημερώσεις ▾"
    }

    fun newWindowButton(lang: VexioAppLanguage): String = when (lang) {
        EN -> "New Window"; DE -> "Neues Fenster"; IT -> "Nuova finestra"; FR -> "Nouvelle fenêtre"; ES -> "Nueva ventana"; GR -> "Νέο παράθυρο"
    }

    fun closeButton(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Close"; DE -> "Schließen"; IT -> "Chiudi"; FR -> "Fermer"; ES -> "Cerrar"; GR -> "Κλείσιμο"
    }

    fun cancelButton(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Cancel"; DE -> "Abbrechen"; IT -> "Annulla"; FR -> "Annuler"; ES -> "Cancelar"; GR -> "Ακύρωση"
    }

    fun dontSaveButton(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Don’t Save"; DE -> "Nicht speichern"; IT -> "Non salvare"; FR -> "Ne pas enregistrer"; ES -> "No guardar"; GR -> "Μη αποθήκευση"
    }

    fun unsavedPromptTitle(docTitle: String, lang: VexioAppLanguage): String = when (lang) {
        EN -> "Save changes to “$docTitle”?"
        DE -> "Änderungen an „$docTitle“ speichern?"
        IT -> "Salvare le modifiche apportate a “$docTitle”?"
        FR -> "Enregistrer les modifications apportées à « $docTitle » ?"
        ES -> "¿Guardar los cambios en “$docTitle”?"
        GR -> "Θέλετε να αποθηκεύσετε τις αλλαγές στο «$docTitle»;"
    }

    fun unsavedPromptMessage(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Your changes will be lost if you close without saving."
        DE -> "Nicht gespeicherte Änderungen gehen beim Schließen verloren."
        IT -> "Le modifiche andranno perse se chiudi senza salvare."
        FR -> "Vos modifications seront perdues si vous fermez sans enregistrer."
        ES -> "Los cambios se perderán si cierras sin guardar."
        GR -> "Οι αλλαγές σας θα χαθούν αν κλείσετε χωρίς αποθήκευση."
    }

    fun applyButton(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Apply"; DE -> "Übernehmen"; IT -> "Applica"; FR -> "Appliquer"; ES -> "Aplicar"; GR -> "Εφαρμογή"
    }

    fun applyColorButton(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Apply Color"; DE -> "Farbe übernehmen"; IT -> "Applica colore"; FR -> "Appliquer la couleur"; ES -> "Aplicar color"; GR -> "Εφαρμογή χρώματος"
    }

    fun redChannel(v: Int, lang: VexioAppLanguage): String = when (lang) {
        EN -> "Red: $v"; DE -> "Rot: $v"; IT -> "Rosso: $v"; FR -> "Rouge : $v"; ES -> "Rojo: $v"; GR -> "Κόκκινο: $v"
    }

    fun greenChannel(v: Int, lang: VexioAppLanguage): String = when (lang) {
        EN -> "Green: $v"; DE -> "Grün: $v"; IT -> "Verde: $v"; FR -> "Vert : $v"; ES -> "Verde: $v"; GR -> "Πράσινο: $v"
    }

    fun blueChannel(v: Int, lang: VexioAppLanguage): String = when (lang) {
        EN -> "Blue: $v"; DE -> "Blau: $v"; IT -> "Blu: $v"; FR -> "Bleu : $v"; ES -> "Azul: $v"; GR -> "Μπλε: $v"
    }

    fun downloadApkUpdate(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Download APK Update"; DE -> "APK-Update herunterladen"; IT -> "Scarica aggiornamento APK"; FR -> "Télécharger la mise à jour APK"; ES -> "Descargar actualización APK"; GR -> "Λήψη ενημέρωσης APK"
    }

    fun releaseNotesButton(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Release Notes"; DE -> "Versionshinweise"; IT -> "Note di rilascio"; FR -> "Notes de version"; ES -> "Notas de la versión"; GR -> "Σημειώσεις έκδοσης"
    }

    fun officialCatalogTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Official & Enterprise Stencil Catalog"
        DE -> "Offizieller & Enterprise-Schablonenkatalog"
        IT -> "Catalogo Stencil Ufficiale & Enterprise"
        FR -> "Catalogue officiel de gabarits d’entreprise"
        ES -> "Catálogo oficial y empresarial de galerías"
        GR -> "Επίσημος κατάλογος Stencil Enterprise"
    }

    fun officialCatalogSubtitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "1-click download & local persistence across restarts (Microsoft 365, Azure V25, Cisco, Panduit, APC & NetEquip .vssx / .vss / .zip)."
        DE -> "1-Klick-Download & dauerhafte lokale Speicherung (Microsoft 365, Azure V25, Cisco, Panduit, APC & NetEquip .vssx / .vss / .zip)."
        IT -> "Download con 1 clic e salvataggio locale permanente (Microsoft 365, Azure V25, Cisco, Panduit, APC & NetEquip .vssx / .vss / .zip)."
        FR -> "Téléchargement en 1 clic et stockage local permanent (Microsoft 365, Azure V25, Cisco, Panduit, APC & NetEquip .vssx / .vss / .zip)."
        ES -> "Descarga en 1 clic y almacenamiento local permanente (Microsoft 365, Azure V25, Cisco, Panduit, APC y NetEquip .vssx / .vss / .zip)."
        GR -> "Λήψη με 1 κλικ & μόνιμη τοπική αποθήκευση (Microsoft 365, Azure V25, Cisco, Panduit, APC & NetEquip .vssx / .vss / .zip)."
    }

    fun installingLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Installing…"; DE -> "Wird installiert…"; IT -> "Installazione…"; FR -> "Installation…"; ES -> "Instalando…"; GR -> "Εγκατάσταση…"
    }

    fun installedBadge(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Installed ✓"; DE -> "Installiert ✓"; IT -> "Installato ✓"; FR -> "Installé ✓"; ES -> "Instalado ✓"; GR -> "Εγκατεστημένο ✓"
    }

    fun installButton(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Install"; DE -> "Installieren"; IT -> "Installa"; FR -> "Installer"; ES -> "Instalar"; GR -> "Εγκατάσταση"
    }

    fun installAllPacksButton(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Install All Packs"; DE -> "Alle Pakete installieren"; IT -> "Installa tutti i pacchetti"; FR -> "Installer tous les packs"; ES -> "Instalar todos los paquetes"; GR -> "Εγκατάσταση όλων"
    }

    // --- Ribbon Tabs & Actions ---

    fun filterFontsPlaceholder(count: Int, lang: VexioAppLanguage): String = when (lang) {
        EN -> "Filter $count OS fonts…"
        DE -> "$count Systemschriften filtern…"
        IT -> "Filtra $count font di sistema…"
        FR -> "Filtrer $count polices système…"
        ES -> "Filtrar $count fuentes del sistema…"
        GR -> "Φιλτράρισμα $count γραμματοσειρών…"
    }

    fun bulletListPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "• List"; DE -> "• Liste"; IT -> "• Elenco"; FR -> "• Liste"; ES -> "• Lista"; GR -> "• Λίστα"
    }

    fun alignMenuPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Align ▾"; DE -> "Ausrichten ▾"; IT -> "Allinea ▾"; FR -> "Aligner ▾"; ES -> "Alinear ▾"; GR -> "Στοίχιση ▾"
    }

    fun alignOptionLabel(modeSlug: String, lang: VexioAppLanguage): String = when (modeSlug) {
        "left" -> when (lang) { EN -> "Align Left"; DE -> "Links ausrichten"; IT -> "Allinea a sinistra"; FR -> "Aligner à gauche"; ES -> "Alinear a la izquierda"; GR -> "Στοίχιση αριστερά" }
        "center" -> when (lang) { EN -> "Align Center"; DE -> "Horizontal zentrieren"; IT -> "Allinea al centro"; FR -> "Centrer"; ES -> "Centrar"; GR -> "Στοίχιση στο κέντρο" }
        "right" -> when (lang) { EN -> "Align Right"; DE -> "Rechts ausrichten"; IT -> "Allinea a destra"; FR -> "Aligner à droite"; ES -> "Alinear a la derecha"; GR -> "Στοίχιση δεξιά" }
        "top" -> when (lang) { EN -> "Align Top"; DE -> "Oben ausrichten"; IT -> "Allinea in alto"; FR -> "Aligner en haut"; ES -> "Alinear arriba"; GR -> "Στοίχιση επάνω" }
        "middle" -> when (lang) { EN -> "Align Middle"; DE -> "Vertikal zentrieren"; IT -> "Allinea al centro vert."; FR -> "Aligner au milieu"; ES -> "Alinear al medio"; GR -> "Στοίχιση στη μέση" }
        "bottom" -> when (lang) { EN -> "Align Bottom"; DE -> "Unten ausrichten"; IT -> "Allinea in basso"; FR -> "Aligner en bas"; ES -> "Alinear abajo"; GR -> "Στοίχιση κάτω" }
        else -> modeSlug
    }

    fun distributeMenuPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Distribute ▾"; DE -> "Verteilen ▾"; IT -> "Distribuisci ▾"; FR -> "Répartir ▾"; ES -> "Distribuir ▾"; GR -> "Κατανομή ▾"
    }

    fun distributeHorizontally(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Distribute Horizontally"; DE -> "Horizontal verteilen"; IT -> "Distribuisci orizzontalmente"; FR -> "Répartir horizontalement"; ES -> "Distribuir horizontalmente"; GR -> "Οριζόντια κατανομή"
    }

    fun distributeVertically(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Distribute Vertically"; DE -> "Vertikal verteilen"; IT -> "Distribuisci verticalmente"; FR -> "Répartir verticalement"; ES -> "Distribuir verticalmente"; GR -> "Κατακόρυφη κατανομή"
    }

    fun groupPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Group"; DE -> "Gruppieren"; IT -> "Raggruppa"; FR -> "Grouper"; ES -> "Agrupar"; GR -> "Ομαδοποίηση"
    }

    fun ungroupPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Ungroup"; DE -> "Gruppierung aufheben"; IT -> "Separa"; FR -> "Dissocier"; ES -> "Desagrupar"; GR -> "Κατάργηση ομάδας"
    }

    fun flipHPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "⇆ Flip H"; DE -> "⇆ Horizontal spiegeln"; IT -> "⇆ Rifletti orizz."; FR -> "⇆ Retourner H"; ES -> "⇆ Voltear H"; GR -> "⇆ Οριζ. αναστροφή"
    }

    fun flipVPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "⇅ Flip V"; DE -> "⇅ Vertikal spiegeln"; IT -> "⇅ Rifletti vert."; FR -> "⇅ Retourner V"; ES -> "⇅ Voltear V"; GR -> "⇅ Κατακ. αναστροφή"
    }

    fun newPagePill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "+ New Page"; DE -> "+ Neues Zeichenblatt"; IT -> "+ Nuova pagina"; FR -> "+ Nouvelle page"; ES -> "+ Nueva página"; GR -> "+ Νέα σελίδα"
    }

    fun textBoxPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Text Box (Ctrl+2)"; DE -> "Textfeld (Ctrl+2)"; IT -> "Casella di testo (Ctrl+2)"; FR -> "Zone de texte (Ctrl+2)"; ES -> "Cuadro de texto (Ctrl+2)"; GR -> "Πλαίσιο κειμένου (Ctrl+2)"
    }

    fun containerPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "+ Container"; DE -> "+ Container"; IT -> "+ Contenitore"; FR -> "+ Conteneur"; ES -> "+ Contenedor"; GR -> "+ Πλαίσιο υποδοχής"
    }

    fun swimlanesPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "+ Swimlanes"; DE -> "+ Funktionsbereiche"; IT -> "+ Corsie (Swimlane)"; FR -> "+ Couloirs"; ES -> "+ Calles funcionales"; GR -> "+ Λωρίδες ρόλων"
    }

    fun calloutPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "+ Callout"; DE -> "+ Legende / Hinweis"; IT -> "+ Didascalia"; FR -> "+ Légende"; ES -> "+ Llamada"; GR -> "+ Επεξήγηση"
    }

    fun subprocessPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Subprocess (+)"; DE -> "Teilprozess (+)"; IT -> "Sottoprocesso (+)"; FR -> "Sous-processus (+)"; ES -> "Subproceso (+)"; GR -> "Υποδιαδικασία (+)"
    }

    fun pageSizePrefix(presetLabel: String, lang: VexioAppLanguage): String = when (lang) {
        EN -> "Page Size: $presetLabel ▾"
        DE -> "Blattgröße: $presetLabel ▾"
        IT -> "Formato pagina: $presetLabel ▾"
        FR -> "Taille de page : $presetLabel ▾"
        ES -> "Tamaño de página: $presetLabel ▾"
        GR -> "Μέγεθος σελίδας: $presetLabel ▾"
    }

    fun autoLayoutPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Auto-Layout"; DE -> "Auto-Layout"; IT -> "Layout automatico"; FR -> "Disposition auto"; ES -> "Diseño automático"; GR -> "Αυτόματη διάταξη"
    }

    fun dataVisualizerPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Data Visualizer (.csv/.xlsx)"
        DE -> "Daten-Visualizer (.csv/.xlsx)"
        IT -> "Visualizzatore dati (.csv/.xlsx)"
        FR -> "Visualiseur de données (.csv/.xlsx)"
        ES -> "Visualizador de datos (.csv/.xlsx)"
        GR -> "Οπτικοποίηση δεδομένων (.csv/.xlsx)"
    }

    fun cloudDiscoveryPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Cloud Discovery"; DE -> "Cloud-Erkennung"; IT -> "Rilevamento Cloud"; FR -> "Découverte Cloud"; ES -> "Descubrimiento Cloud"; GR -> "Ανακάλυψη Cloud"
    }

    fun entraOrgChartPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Entra / LDIF Org Chart"; DE -> "Entra-/LDIF-Organigramm"; IT -> "Organigramma Entra / LDIF"; FR -> "Organigramme Entra / LDIF"; ES -> "Organigrama Entra / LDIF"; GR -> "Οργανόγραμμα Entra / LDIF"
    }

    fun validateBpmnPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Validate BPMN 2.0 ✓"; DE -> "BPMN 2.0 prüfen ✓"; IT -> "Convalida BPMN 2.0 ✓"; FR -> "Valider BPMN 2.0 ✓"; ES -> "Validar BPMN 2.0 ✓"; GR -> "Έλεγχος BPMN 2.0 ✓"
    }

    fun validateUmlPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Validate UML 2.5 ✓"; DE -> "UML 2.5 prüfen ✓"; IT -> "Convalida UML 2.5 ✓"; FR -> "Valider UML 2.5 ✓"; ES -> "Validar UML 2.5 ✓"; GR -> "Έλεγχος UML 2.5 ✓"
    }

    fun createSubprocessDrillDown(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Create Subprocess Drill-Down"
        DE -> "Teilprozess-Detailblatt erstellen"
        IT -> "Crea dettaglio sottoprocesso"
        FR -> "Créer une page de sous-processus"
        ES -> "Crear desglose de subproceso"
        GR -> "Δημιουργία σελίδας υποδιαδικασίας"
    }

    fun markupPenPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Markup Pen"; DE -> "Freihandstift"; IT -> "Penna annotazioni"; FR -> "Stylet d’annotation"; ES -> "Lápiz de revisión"; GR -> "Πένα σχολιασμού"
    }

    fun highlighterPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Highlighter"; DE -> "Textmarker"; IT -> "Evidenziatore"; FR -> "Surligneur"; ES -> "Marcador"; GR -> "Μαρκαδόρος"
    }

    fun diagramComparePill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Diagram Compare (Visual Diff)"
        DE -> "Diagrammvergleich (Visueller Diff)"
        IT -> "Confronta diagrammi (Diff visivo)"
        FR -> "Comparer les diagrammes"
        ES -> "Comparar diagramas"
        GR -> "Σύγκριση διαγραμμάτων"
    }

    fun smartGuidesPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Smart Guides"; DE -> "Intelligente Hilfslinien"; IT -> "Guide intelligenti"; FR -> "Repères intelligents"; ES -> "Guías inteligentes"; GR -> "Έξυπνοι οδηγοί"
    }

    fun stencilsNavigatorPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Stencils Navigator"; DE -> "Schablonen-Bereich"; IT -> "Pannello Stencil"; FR -> "Volet Gabarits"; ES -> "Panel de galerías"; GR -> "Πίνακας Stencil"
    }

    fun inspectorPanePill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Inspector Pane"; DE -> "Eigenschaften-Inspektor"; IT -> "Pannello proprietà"; FR -> "Inspecteur"; ES -> "Panel de propiedades"; GR -> "Πίνακας ιδιοτήτων"
    }

    fun zoom100Pill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "100% Zoom"; DE -> "100 % Zoom"; IT -> "Zoom 100%"; FR -> "Zoom 100 %"; ES -> "Zoom 100 %"; GR -> "Ζουμ 100%"
    }

    fun plan2StripPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Plan 2 Strip"; DE -> "Plan-2-Leiste"; IT -> "Barra Plan 2"; FR -> "Barre Plan 2"; ES -> "Barra Plan 2"; GR -> "Γραμμή Plan 2"
    }

    fun booleanGroupTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Boolean"; DE -> "Boolesch"; IT -> "Booleane"; FR -> "Booléen"; ES -> "Booleano"; GR -> "Λογικές"
    }

    fun themeLabelPrefix(themeName: String, lang: VexioAppLanguage): String = when (lang) {
        EN -> "Theme: $themeName"; DE -> "Design: $themeName"; IT -> "Tema: $themeName"; FR -> "Thème : $themeName"; ES -> "Tema: $themeName"; GR -> "Θέμα: $themeName"
    }

    fun variantsSuffix(lang: VexioAppLanguage): String = when (lang) {
        EN -> "4 Variants"; DE -> "4 Varianten"; IT -> "4 varianti"; FR -> "4 variantes"; ES -> "4 variantes"; GR -> "4 παραλλαγές"
    }

    // --- Stencil Drawer ---

    fun stencilsHeader(shapeCount: Int, packCount: Int, lang: VexioAppLanguage): String = when (lang) {
        EN -> "Stencils ($shapeCount • $packCount Packs)"
        DE -> "Schablonen ($shapeCount • $packCount Pakete)"
        IT -> "Stencil ($shapeCount • $packCount pacchetti)"
        FR -> "Gabarits ($shapeCount • $packCount packs)"
        ES -> "Galerías ($shapeCount • $packCount paquetes)"
        GR -> "Stencil ($shapeCount • $packCount πακέτα)"
    }

    fun expandAllLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Expand All"; DE -> "Alle öffnen"; IT -> "Espandi tutto"; FR -> "Tout développer"; ES -> "Expandir todo"; GR -> "Ανάπτυξη όλων"
    }

    fun collapseLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Collapse"; DE -> "Einklappen"; IT -> "Comprimi"; FR -> "Réduire"; ES -> "Contraer"; GR -> "Σύμπτυξη"
    }

    fun addStencilShort(lang: VexioAppLanguage): String = when (lang) {
        EN -> "+ Stencil"; DE -> "+ Schablone"; IT -> "+ Stencil"; FR -> "+ Gabarit"; ES -> "+ Galería"; GR -> "+ Stencil"
    }

    fun officialPacksShort(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Catalog"; DE -> "Katalog"; IT -> "Catalogo"; FR -> "Catalogue"; ES -> "Catálogo"; GR -> "Κατάλογος"
    }

    fun addToMyShapesShort(lang: VexioAppLanguage): String = when (lang) {
        EN -> "+ My Shapes"; DE -> "+ Eigene Shapes"; IT -> "+ Mie forme"; FR -> "+ Mes formes"; ES -> "+ Mis formas"; GR -> "+ Σχήματά μου"
    }

    fun newBlankDiagramMenu(lang: VexioAppLanguage): String = when (lang) {
        EN -> "New Blank Diagram"; DE -> "Neues leeres Diagramm"; IT -> "Nuovo diagramma vuoto"; FR -> "Nouveau diagramme vierge"; ES -> "Nuevo diagrama en blanco"; GR -> "Νέο κενό διάγραμμα"
    }

    fun printMenuLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Print…"; DE -> "Drucken…"; IT -> "Stampa…"; FR -> "Imprimer…"; ES -> "Imprimir…"; GR -> "Εκτύπωση…"
    }

    fun closeDocumentMenuLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Close Document"; DE -> "Dokument schließen"; IT -> "Chiudi documento"; FR -> "Fermer le document"; ES -> "Cerrar documento"; GR -> "Κλείσιμο εγγράφου"
    }

    fun exportSubmenuLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Export As"; DE -> "Exportieren als"; IT -> "Esporta come"; FR -> "Exporter sous"; ES -> "Exportar como"; GR -> "Εξαγωγή ως"
    }

    fun alignTextLeftTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Align Text Left"; DE -> "Text linksbündig"; IT -> "Allinea testo a sinistra"; FR -> "Aligner le texte à gauche"; ES -> "Alinear texto a la izquierda"; GR -> "Στοίχιση κειμένου αριστερά"
    }

    fun alignTextCenterTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Center Text"; DE -> "Text zentrieren"; IT -> "Centra testo"; FR -> "Centrer le texte"; ES -> "Centrar texto"; GR -> "Κεντράρισμα κειμένου"
    }

    fun alignTextRightTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Align Text Right"; DE -> "Text rechtsbündig"; IT -> "Allinea testo a destra"; FR -> "Aligner le texte à droite"; ES -> "Alinear texto a la derecha"; GR -> "Στοίχιση κειμένου δεξιά"
    }

    fun valignTopTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Top Vertical Align"; DE -> "Text oben ausrichten"; IT -> "Allinea in alto"; FR -> "Aligner en haut"; ES -> "Alinear arriba"; GR -> "Στοίχιση επάνω"
    }

    fun valignMiddleTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Middle Vertical Align"; DE -> "Text vertikal zentrieren"; IT -> "Centra verticalmente"; FR -> "Centrer verticalement"; ES -> "Centrar verticalmente"; GR -> "Κατακόρυφο κεντράρισμα"
    }

    fun valignBottomTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Bottom Vertical Align"; DE -> "Text unten ausrichten"; IT -> "Allinea in basso"; FR -> "Aligner en bas"; ES -> "Alinear abajo"; GR -> "Στοίχιση κάτω"
    }

    fun bulletListTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Bulleted List"; DE -> "Aufzählungsliste"; IT -> "Elenco puntato"; FR -> "Liste à puces"; ES -> "Lista con viñetas"; GR -> "Λίστα με κουκκίδες"
    }

    fun stylesDropdownLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Styles"; DE -> "Formatvorlagen"; IT -> "Stili"; FR -> "Styles"; ES -> "Estilos"; GR -> "Στυλ"
    }

    fun groupClipboard(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Clipboard"; DE -> "Zwischenablage"; IT -> "Appunti"; FR -> "Presse-papiers"; ES -> "Portapapeles"; GR -> "Πρόχειρο"
    }

    fun groupFont(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Font"; DE -> "Schriftart"; IT -> "Carattere"; FR -> "Police"; ES -> "Fuente"; GR -> "Γραμματοσειρά"
    }

    fun groupParagraph(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Paragraph"; DE -> "Absatz"; IT -> "Paragrafo"; FR -> "Paragraphe"; ES -> "Párrafo"; GR -> "Παράγραφος"
    }

    fun groupShapeStyle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Shape Style"; DE -> "Formstil"; IT -> "Stile forma"; FR -> "Style de forme"; ES -> "Estilo de forma"; GR -> "Στυλ σχήματος"
    }

    fun groupPages(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Pages & Stencils"; DE -> "Blätter & Schablonen"; IT -> "Pagine e stencil"; FR -> "Pages et gabarits"; ES -> "Páginas y galerías"; GR -> "Σελίδες & Stencil"
    }

    fun groupDiagramParts(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Diagram Parts"; DE -> "Diagrammteile"; IT -> "Parti diagramma"; FR -> "Éléments"; ES -> "Partes del diagrama"; GR -> "Στοιχεία"
    }

    fun groupAec(lang: VexioAppLanguage): String = when (lang) {
        EN -> "AEC & Architecture"; DE -> "AEC & Grundriss"; IT -> "AEC e planimetria"; FR -> "AEC et plan"; ES -> "AEC y plano"; GR -> "AEC & Κάτοψη"
    }

    fun groupWizardsCad(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Wizards & CAD"; DE -> "Assistenten & CAD"; IT -> "Procedure e CAD"; FR -> "Assistants et CAO"; ES -> "Asistentes y CAD"; GR -> "Οδηγοί & CAD"
    }

    fun shortNewPage(lang: VexioAppLanguage): String = when (lang) {
        EN -> "New Page"; DE -> "Neues Blatt"; IT -> "Nuova pagina"; FR -> "Nouvelle page"; ES -> "Nueva página"; GR -> "Νέα σελίδα"
    }

    fun shortImportStencil(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Stencil"; DE -> "Schablone"; IT -> "Stencil"; FR -> "Gabarit"; ES -> "Galería"; GR -> "Stencil"
    }

    fun shortContainer(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Container"; DE -> "Container"; IT -> "Contenitore"; FR -> "Conteneur"; ES -> "Contenedor"; GR -> "Πλαίσιο"
    }

    fun shortSwimlanes(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Swimlanes"; DE -> "Bahnen"; IT -> "Corsie"; FR -> "Couloirs"; ES -> "Calles"; GR -> "Λωρίδες"
    }

    fun shortCallout(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Callout"; DE -> "Hinweis"; IT -> "Nota"; FR -> "Légende"; ES -> "Llamada"; GR -> "Σημείωση"
    }

    fun shortWallOpening(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Opening"; DE -> "Öffnung"; IT -> "Apertura"; FR -> "Ouverture"; ES -> "Abertura"; GR -> "Άνοιγμα"
    }

    fun shortHealWalls(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Heal Walls"; DE -> "Wände heilen"; IT -> "Unisci muri"; FR -> "Joindre murs"; ES -> "Unir muros"; GR -> "Ένωση τοίχων"
    }

    fun shortWizards(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Wizards…"; DE -> "Assistenten…"; IT -> "Procedure…"; FR -> "Assistants…"; ES -> "Asistentes…"; GR -> "Οδηγοί…"
    }

    fun searchStencilsPlaceholder(shapeCount: Int, packCount: Int, lang: VexioAppLanguage): String = when (lang) {
        EN -> "Search $shapeCount shapes in $packCount packs…"
        DE -> "$shapeCount Shapes in $packCount Paketen suchen…"
        IT -> "Cerca $shapeCount forme in $packCount pacchetti…"
        FR -> "Rechercher $shapeCount formes dans $packCount packs…"
        ES -> "Buscar $shapeCount formas en $packCount paquetes…"
        GR -> "Αναζήτηση $shapeCount σχημάτων σε $packCount πακέτα…"
    }

    fun allCategoryPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "All"; DE -> "Alle"; IT -> "Tutti"; FR -> "Tous"; ES -> "Todos"; GR -> "Όλα"
    }

    fun localizeStencilCategory(category: String, lang: VexioAppLanguage): String {
        if (category == "All") return allCategoryPill(lang)
        if (lang == EN) return category
        return when (category) {
            "Basic Shapes" -> when (lang) { DE -> "Standard-Shapes"; IT -> "Forme base"; FR -> "Formes de base"; ES -> "Formas básicas"; GR -> "Βασικά σχήματα"; else -> category }
            "Basic Flowchart" -> when (lang) { DE -> "Standard-Flussdiagramm"; IT -> "Diagramma di flusso base"; FR -> "Organigramme simple"; ES -> "Diagrama de flujo básico"; GR -> "Βασικό διάγραμμα ροής"; else -> category }
            "Arrows & Connectors" -> when (lang) { DE -> "Pfeile & Verbinder"; IT -> "Frecce e connettori"; FR -> "Flèches et connecteurs"; ES -> "Flechas y conectores"; GR -> "Βέλη & γραμμές σύνδεσης"; else -> category }
            "Callouts, Notes & Containers" -> when (lang) { DE -> "Legenden, Notizen & Container"; IT -> "Didascalie, note e contenitori"; FR -> "Légendes, notes et conteneurs"; ES -> "Llamadas, notas y contenedores"; GR -> "Επεξηγήσεις, σημειώσεις & πλαίσια"; else -> category }
            "Titles, Headings & Banners" -> when (lang) { DE -> "Titel, Überschriften & Banner"; IT -> "Titoli, intestazioni e banner"; FR -> "Titres, en-têtes et bannières"; ES -> "Títulos, encabezados y banners"; GR -> "Τίτλοι, επικεφαλίδες & banner"; else -> category }
            "Cross-Functional Swimlanes" -> when (lang) { DE -> "Funktionsübergreifende Swimlanes"; IT -> "Corsie interfunzionali"; FR -> "Couloirs interfonctionnels"; ES -> "Calles multifuncionales"; GR -> "Διαλειτουργικές λωρίδες"; else -> category }
            "Database Crow's Foot ERD" -> when (lang) { DE -> "Datenbank-ERD (Krähenfuß)"; IT -> "Database ERD Crow's Foot"; FR -> "Base de données ERD"; ES -> "Base de datos ERD"; GR -> "Βάση δεδομένων ERD"; else -> category }
            "Microsoft 365 & Office" -> when (lang) { DE -> "Microsoft 365 & Office"; IT -> "Microsoft 365 & Office"; FR -> "Microsoft 365 & Office"; ES -> "Microsoft 365 y Office"; GR -> "Microsoft 365 & Office"; else -> category }
            "Microsoft Servers & Active Directory" -> when (lang) { DE -> "Microsoft Server & Active Directory"; IT -> "Server Microsoft & Active Directory"; FR -> "Serveurs Microsoft & Active Directory"; ES -> "Servidores Microsoft y Active Directory"; GR -> "Διακομιστές Microsoft & Active Directory"; else -> category }
            "Microsoft Azure & Hybrid Cloud" -> when (lang) { DE -> "Microsoft Azure & Hybrid Cloud"; IT -> "Microsoft Azure & Cloud ibrido"; FR -> "Microsoft Azure & Cloud hybride"; ES -> "Microsoft Azure y nube híbrida"; GR -> "Microsoft Azure & Υβριδικό Cloud"; else -> category }
            "Microsoft Power Platform & Dynamics 365" -> when (lang) { DE -> "Microsoft Power Platform & Dynamics 365"; IT -> "Microsoft Power Platform & Dynamics 365"; FR -> "Microsoft Power Platform & Dynamics 365"; ES -> "Microsoft Power Platform y Dynamics 365"; GR -> "Microsoft Power Platform & Dynamics 365"; else -> category }
            "Servers (Standard Roles)" -> when (lang) { DE -> "Server (Standardrollen)"; IT -> "Server (Ruoli standard)"; FR -> "Serveurs (Rôles standard)"; ES -> "Servidores (Roles estándar)"; GR -> "Διακομιστές (Τυπικοί ρόλοι)"; else -> category }
            "Network & Rack Equipment" -> when (lang) { DE -> "Netzwerk- & Rack-Geräte"; IT -> "Apparati di rete e rack"; FR -> "Équipements réseau et baie"; ES -> "Equipos de red y rack"; GR -> "Εξοπλισμός δικτύου & Rack"; else -> category }
            "Cloud Architecture (AWS / Azure / GCP)" -> when (lang) { DE -> "Cloud-Architektur (AWS / Azure / GCP)"; IT -> "Architettura Cloud (AWS / Azure / GCP)"; FR -> "Architecture Cloud (AWS / Azure / GCP)"; ES -> "Arquitectura Cloud (AWS / Azure / GCP)"; GR -> "Αρχιτεκτονική Cloud (AWS / Azure / GCP)"; else -> category }
            "Architectural Floor Plan & Office" -> when (lang) { DE -> "Architektur-Grundriss & Büro"; IT -> "Planimetria architettonica e ufficio"; FR -> "Plan d’étage et bureau"; ES -> "Plano arquitectónico y oficina"; GR -> "Αρχιτεκτονική κάτοψη & γραφείο"; else -> category }
            "Electrical & IEEE Schematics" -> when (lang) { DE -> "Elektrotechnik- & IEEE-Schaltpläne"; IT -> "Schemi elettrici IEEE"; FR -> "Schémas électriques IEEE"; ES -> "Esquemas eléctricos IEEE"; GR -> "Ηλεκτρολογικά σχέδια IEEE"; else -> category }
            "P&ID Process Engineering" -> when (lang) { DE -> "R&I-Verfahrenstechnik (P&ID)"; IT -> "Ingegneria di processo P&ID"; FR -> "Ingénierie des procédés P&ID"; ES -> "Ingeniería de procesos P&ID"; GR -> "Μηχανική διεργασιών P&ID"; else -> category }
            "Organization Chart" -> when (lang) { DE -> "Organigramm"; IT -> "Organigramma"; FR -> "Organigramme"; ES -> "Organigrama"; GR -> "Οργανόγραμμα"; else -> category }
            "Gantt, PERT & Project Timeline" -> when (lang) { DE -> "Gantt-, PERT- & Projektzeitachse"; IT -> "Gantt, PERT e cronologia"; FR -> "Gantt, PERT et chronologie"; ES -> "Gantt, PERT y línea de tiempo"; GR -> "Gantt, PERT & Χρονοδιάγραμμα"; else -> category }
            "Wireframe & UI Mockups" -> when (lang) { DE -> "Wireframe- & UI-Entwürfe"; IT -> "Wireframe e mockup UI"; FR -> "Maquettes UI et wireframes"; ES -> "Wireframes y maquetas UI"; GR -> "Wireframes & προσχέδια UI"; else -> category }
            "Business Matrix, SWOT & Venn" -> when (lang) { DE -> "Business-Matrix, SWOT & Venn"; IT -> "Matrice aziendale, SWOT e Venn"; FR -> "Matrice métier, SWOT et Venn"; ES -> "Matriz empresarial, DAFO y Venn"; GR -> "Πίνακες SWOT & διαγράμματα Venn"; else -> category }
            "IT Generic & Security Scanning (ECCEMAN / MTMnet / OpenVAS)" -> when (lang) { DE -> "IT-Allgemein & Sicherheits-Scan (ECCEMAN / MTMnet / OpenVAS)"; IT -> "IT generico e sicurezza (ECCEMAN / MTMnet / OpenVAS)"; FR -> "IT générique & sécurité (ECCEMAN / MTMnet / OpenVAS)"; ES -> "TI genérica y seguridad (ECCEMAN / MTMnet / OpenVAS)"; GR -> "Γενικά IT & σάρωση ασφαλείας (OpenVAS)"; else -> category }
            "Security & Carrier Vendors (Palo Alto / Fortinet / Check Point / Juniper / Huawei)" -> when (lang) { DE -> "Sicherheits- & Netzwerk-Hersteller (Palo Alto / Fortinet / Check Point / Juniper / Huawei)"; IT -> "Fornitori sicurezza e rete (Palo Alto / Fortinet / Check Point / Juniper / Huawei)"; FR -> "Éditeurs sécurité & réseau (Palo Alto / Fortinet / Check Point / Juniper / Huawei)"; ES -> "Fabricantes de seguridad y red (Palo Alto / Fortinet / Check Point / Juniper / Huawei)"; GR -> "Κατασκευαστές ασφαλείας & δικτύων (Palo Alto / Fortinet / Juniper)"; else -> category }
            "Multi-Cloud, Kubernetes & VMware (AWS / GCP / OCI / K8s / vSphere)" -> when (lang) { DE -> "Multi-Cloud, Kubernetes & VMware (AWS / GCP / OCI / K8s / vSphere)"; IT -> "Multi-Cloud, Kubernetes e VMware (AWS / GCP / OCI / K8s / vSphere)"; FR -> "Multi-Cloud, Kubernetes & VMware (AWS / GCP / OCI / K8s / vSphere)"; ES -> "Multi-Cloud, Kubernetes y VMware (AWS / GCP / OCI / K8s / vSphere)"; GR -> "Multi-Cloud, Kubernetes & VMware (AWS / GCP / OCI / K8s)"; else -> category }
            "Rack Power, Cabling & KVM (CyberPower / Tripp Lite / Raritan / BlackBox / HellermannTyton)" -> when (lang) { DE -> "Rack-Strom, Verkabelung & KVM (CyberPower / Tripp Lite / Raritan / BlackBox / HellermannTyton)"; IT -> "Alimentazione rack, cablaggio e KVM (CyberPower / Tripp Lite / Raritan / BlackBox)"; FR -> "Alimentation baie, câblage & KVM (CyberPower / Tripp Lite / Raritan / BlackBox)"; ES -> "Energía rack, cableado y KVM (CyberPower / Tripp Lite / Raritan / BlackBox)"; GR -> "Τροφοδοσία Rack, καλωδίωση & KVM (CyberPower / Raritan)"; else -> category }
            "Enterprise NAS, SMB Network & Unified Comms (Synology / QNAP / TP-Link / Poly / Avaya)" -> when (lang) { DE -> "Enterprise-NAS, Netzwerk & UC (Synology / QNAP / TP-Link / Poly / Avaya)"; IT -> "NAS Enterprise, rete e UC (Synology / QNAP / TP-Link / Poly / Avaya)"; FR -> "NAS d’entreprise, réseau & UC (Synology / QNAP / TP-Link / Poly / Avaya)"; ES -> "NAS empresarial, red y UC (Synology / QNAP / TP-Link / Poly / Avaya)"; GR -> "Συστήματα NAS, δίκτυα & UC (Synology / QNAP / TP-Link / Poly)"; else -> category }
            "C4 Architecture & Enterprise Integration Patterns (EIP)" -> when (lang) { DE -> "C4-Architektur & Enterprise Integration Patterns (EIP)"; IT -> "Architettura C4 e Enterprise Integration Patterns (EIP)"; FR -> "Architecture C4 & Enterprise Integration Patterns (EIP)"; ES -> "Arquitectura C4 y Enterprise Integration Patterns (EIP)"; GR -> "Αρχιτεκτονική C4 & πρότυπα ολοκλήρωσης (EIP)"; else -> category }
            "Decision, Case & Risk Modeling (CMMN 1.1 / DMN 1.3 / CORAS)" -> when (lang) { DE -> "Entscheidungs-, Fall- & Risikomodellierung (CMMN / DMN / CORAS)"; IT -> "Modellazione decisioni, casi e rischi (CMMN / DMN / CORAS)"; FR -> "Modélisation décisions, cas & risques (CMMN / DMN / CORAS)"; ES -> "Modelado de decisiones, casos y riesgos (CMMN / DMN / CORAS)"; GR -> "Μοντελοποίηση αποφάσεων, υποθέσεων & κινδύνου (CMMN / DMN / CORAS)"; else -> category }
            "Data Vault 2.0 & ADAPT OLAP BI" -> when (lang) { DE -> "Data Vault 2.0 & ADAPT OLAP-BI"; IT -> "Data Vault 2.0 e ADAPT OLAP BI"; FR -> "Data Vault 2.0 & BI OLAP ADAPT"; ES -> "Data Vault 2.0 y BI OLAP ADAPT"; GR -> "Data Vault 2.0 & ADAPT OLAP BI"; else -> category }
            "Systems Engineering & Ontology (SysML & IDEF0 / IDEF3 / IDEF5)" -> when (lang) { DE -> "Systemtechnik & Ontologie (SysML & IDEF0 / IDEF3 / IDEF5)"; IT -> "Ingegneria dei sistemi e ontologia (SysML & IDEF0 / IDEF3 / IDEF5)"; FR -> "Ingénierie système & ontologie (SysML & IDEF0 / IDEF3 / IDEF5)"; ES -> "Ingeniería de sistemas y ontología (SysML e IDEF0 / IDEF3 / IDEF5)"; GR -> "Μηχανική συστημάτων & οντολογία (SysML & IDEF)"; else -> category }
            "Smart Home IoT, Digital Logic & GDS / IA UX" -> when (lang) { DE -> "Smart-Home-IoT, Digitallogik & GDS / IA UX"; IT -> "Smart Home IoT, logica digitale e UX GDS / IA"; FR -> "Domotique IoT, logique numérique & UX GDS / IA"; ES -> "Domótica IoT, lógica digital y UX GDS / IA"; GR -> "Smart Home IoT, ψηφιακή λογική & UX GDS / IA"; else -> category }
            else -> category
        }
    }

    fun addMasterToPageMenu(title: String, lang: VexioAppLanguage): String = when (lang) {
        EN -> "Add '$title' to Page"
        DE -> "„$title“ auf Zeichenblatt einfügen"
        IT -> "Aggiungi '$title' alla pagina"
        FR -> "Ajouter « $title » à la page"
        ES -> "Añadir '$title' a la página"
        GR -> "Προσθήκη «$title» στη σελίδα"
    }

    fun openSideBySideWindowMenu(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Open New Side-by-Side Window"
        DE -> "Neues Fenster nebeneinander öffnen"
        IT -> "Apri nuova finestra affiancata"
        FR -> "Ouvrir une fenêtre côte à côte"
        ES -> "Abrir nueva ventana en paralelo"
        GR -> "Άνοιγμα νέου παραθύρου δίπλα-δίπλα"
    }

    // --- Canvas Drop Overlay & Right-Click Context Menu ---

    fun dropOverlayBanner(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Drop Stencil Master or .vsdx / .vssx / .vsd / .dwg / .dxf Here"
        DE -> "Schablonen-Master oder .vsdx / .vssx / .vsd / .dwg / .dxf hier ablegen"
        IT -> "Rilascia qui uno Stencil o un file .vsdx / .vssx / .vsd / .dwg / .dxf"
        FR -> "Déposez un gabarit ou un fichier .vsdx / .vssx / .vsd / .dwg / .dxf ici"
        ES -> "Suelte aquí una forma o archivo .vsdx / .vssx / .vsd / .dwg / .dxf"
        GR -> "Αποθέστε εδώ Stencil ή αρχείο .vsdx / .vssx / .vsd / .dwg / .dxf"
    }

    fun ctxEditShapeLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Edit Shape Label (Double-Click)"; DE -> "Shape-Text bearbeiten (Doppelklick)"; IT -> "Modifica testo forma (Doppio clic)"; FR -> "Modifier le texte (Double-clic)"; ES -> "Editar texto de forma (Doble clic)"; GR -> "Επεξεργασία κειμένου (Διπλό κλικ)"
    }

    fun ctxGroupSelected(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Group Selected Shapes (Ctrl+G)"; DE -> "Auswahl gruppieren (Ctrl+G)"; IT -> "Raggruppa selezione (Ctrl+G)"; FR -> "Grouper la sélection (Ctrl+G)"; ES -> "Agrupar selección (Ctrl+G)"; GR -> "Ομαδοποίηση επιλογής (Ctrl+G)"
    }

    fun ctxUngroupSelected(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Ungroup Selected Group (Ctrl+Shift+G)"; DE -> "Gruppierung aufheben (Ctrl+Shift+G)"; IT -> "Separa gruppo (Ctrl+Shift+G)"; FR -> "Dissocier le groupe (Ctrl+Shift+G)"; ES -> "Desagrupar selección (Ctrl+Shift+G)"; GR -> "Κατάργηση ομαδοποίησης (Ctrl+Shift+G)"
    }

    fun ctxRotateLeft90(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Rotate Left 90° (↺)"; DE -> "90° nach links drehen (↺)"; IT -> "Ruota di 90° a sinistra (↺)"; FR -> "Pivoter de 90° à gauche (↺)"; ES -> "Girar 90° a la izquierda (↺)"; GR -> "Περιστροφή 90° αριστερά (↺)"
    }

    fun ctxRotateRight90(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Rotate Right 90° (↻)"; DE -> "90° nach rechts drehen (↻)"; IT -> "Ruota di 90° a destra (↻)"; FR -> "Pivoter de 90° à droite (↻)"; ES -> "Girar 90° a la derecha (↻)"; GR -> "Περιστροφή 90° δεξιά (↻)"
    }

    fun ctxFlipHorizontal(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Flip Horizontal (⇆)"; DE -> "Horizontal spiegeln (⇆)"; IT -> "Rifletti orizzontalmente (⇆)"; FR -> "Retourner horizontalement (⇆)"; ES -> "Voltear horizontalmente (⇆)"; GR -> "Οριζόντια αναστροφή (⇆)"
    }

    fun ctxFlipVertical(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Flip Vertical (⇅)"; DE -> "Vertikal spiegeln (⇅)"; IT -> "Rifletti verticalmente (⇅)"; FR -> "Retourner verticalement (⇅)"; ES -> "Voltear verticalmente (⇅)"; GR -> "Κατακόρυφη αναστροφή (⇅)"
    }

    fun ctxDuplicate(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Duplicate Selection (Ctrl+D)"; DE -> "Auswahl duplizieren (Ctrl+D)"; IT -> "Duplica selezione (Ctrl+D)"; FR -> "Dupliquer la sélection (Ctrl+D)"; ES -> "Duplicar selección (Ctrl+D)"; GR -> "Διπλασιασμός επιλογής (Ctrl+D)"
    }

    fun ctxDelete(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Delete Selected Shape (Del)"; DE -> "Auswahl löschen (Entf)"; IT -> "Elimina forma (Canc)"; FR -> "Supprimer la sélection (Suppr)"; ES -> "Eliminar selección (Supr)"; GR -> "Διαγραφή επιλογής (Del)"
    }

    fun ctxBringToFront(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Bring to Front"; DE -> "In den Vordergrund"; IT -> "Porta in primo piano"; FR -> "Mettre au premier plan"; ES -> "Traer al frente"; GR -> "Μεταφορά σε πρώτο πλάνο"
    }

    fun ctxSendToBack(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Send to Back"; DE -> "In den Hintergrund"; IT -> "Porta in secondo piano"; FR -> "Mettre à l’arrière-plan"; ES -> "Enviar al fondo"; GR -> "Μεταφορά στο φόντο"
    }

    fun ctxInsertPrimitive(kind: String, lang: VexioAppLanguage): String = when (kind) {
        "rect" -> when (lang) { EN -> "Insert Rectangle Here"; DE -> "Rechteck hier einfügen"; IT -> "Inserisci rettangolo qui"; FR -> "Insérer un rectangle ici"; ES -> "Insertar rectángulo aquí"; GR -> "Εισαγωγή ορθογωνίου εδώ" }
        "ellipse" -> when (lang) { EN -> "Insert Circle / Ellipse Here"; DE -> "Kreis / Ellipse hier einfügen"; IT -> "Inserisci cerchio / ellisse qui"; FR -> "Insérer un cercle / ellipse ici"; ES -> "Insertar círculo / elipse aquí"; GR -> "Εισαγωγή κύκλου / έλλειψης εδώ" }
        "diamond" -> when (lang) { EN -> "Insert Diamond Here"; DE -> "Raute hier einfügen"; IT -> "Inserisci rombo qui"; FR -> "Insérer un losange ici"; ES -> "Insertar rombo aquí"; GR -> "Εισαγωγή ρόμβου εδώ" }
        "hexagon" -> when (lang) { EN -> "Insert Hexagon Here"; DE -> "Sechseck hier einfügen"; IT -> "Inserisci esagono qui"; FR -> "Insérer un hexagone ici"; ES -> "Insertar hexágono aquí"; GR -> "Εισαγωγή εξαγώνου εδώ" }
        "star" -> when (lang) { EN -> "Insert 5-Point Star Here"; DE -> "5-zackigen Stern hier einfügen"; IT -> "Inserisci stella a 5 punte qui"; FR -> "Insérer une étoile ici"; ES -> "Insertar estrella aquí"; GR -> "Εισαγωγή αστεριού εδώ" }
        "line" -> when (lang) { EN -> "Insert Straight Line Here"; DE -> "Gerade Linie hier einfügen"; IT -> "Inserisci linea retta qui"; FR -> "Insérer une ligne droite ici"; ES -> "Insertar línea recta aquí"; GR -> "Εισαγωγή ευθείας γραμμής εδώ" }
        else -> kind
    }

    fun ctxAutoLayoutPage(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Auto-Layout Active Page"; DE -> "Aktives Zeichenblatt automatisch anordnen"; IT -> "Layout automatico pagina attiva"; FR -> "Disposition auto de la page"; ES -> "Diseño automático de la página"; GR -> "Αυτόματη διάταξη ενεργής σελίδας"
    }

    fun ctxLinkSubprocess(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Link Subprocess Page (+)"; DE -> "Teilprozess-Zeichenblatt verknüpfen (+)"; IT -> "Collega pagina sottoprocesso (+)"; FR -> "Lier une page de sous-processus (+)"; ES -> "Vincular página de subproceso (+)"; GR -> "Σύνδεση σελίδας υποδιαδικασίας (+)"
    }

    fun ctxInspectShapeSheet(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Inspect ShapeSheet (Shift+F5)"; DE -> "ShapeSheet untersuchen (Shift+F5)"; IT -> "Ispeziona ShapeSheet (Shift+F5)"; FR -> "Inspecter la ShapeSheet (Shift+F5)"; ES -> "Inspeccionar ShapeSheet (Shift+F5)"; GR -> "Επιθεώρηση ShapeSheet (Shift+F5)"
    }

    fun ctxOpenNewWindow(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Open New Diagram Window (Ctrl+N)"; DE -> "Neues Diagrammfenster öffnen (Ctrl+N)"; IT -> "Apri nuova finestra (Ctrl+N)"; FR -> "Ouvrir une nouvelle fenêtre (Ctrl+N)"; ES -> "Abrir nueva ventana (Ctrl+N)"; GR -> "Άνοιγμα νέου παραθύρου (Ctrl+N)"
    }

    // --- Inspector, ShapeSheet & Status Bar ---

    fun shapeSheetSubtitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Canonical Units: in / deg • Reactive Topological DAG"
        DE -> "Kanonische Einheiten: in / deg • Reaktiver topologischer DAG"
        IT -> "Unità canoniche: in / deg • DAG topologico reattivo"
        FR -> "Unités canoniques : in / deg • DAG topologique réactif"
        ES -> "Unidades canónicas: in / deg • DAG topológico reactivo"
        GR -> "Κανονικές μονάδες: in / deg • Αντιδραστικός DAG"
    }

    fun shapeAndLayerInspectorTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Shape & Layer Inspector"
        DE -> "Shape- & Ebenen-Inspektor"
        IT -> "Ispettore forme e livelli"
        FR -> "Inspecteur de formes et calques"
        ES -> "Inspector de formas y capas"
        GR -> "Επιθεωρητής σχημάτων & επιπέδων"
    }

    fun fontButtonLabel(family: String, sizePt: Int, lang: VexioAppLanguage): String = when (lang) {
        EN -> "Font: $family ($sizePt pt) ▾"
        DE -> "Schriftart: $family ($sizePt pt) ▾"
        IT -> "Carattere: $family ($sizePt pt) ▾"
        FR -> "Police : $family ($sizePt pt) ▾"
        ES -> "Fuente: $family ($sizePt pt) ▾"
        GR -> "Γραμματοσειρά: $family ($sizePt pt) ▾"
    }

    fun telemetryCardTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Visio Plan 2 Telemetry"; DE -> "Visio-Plan-2-Status"; IT -> "Stato Visio Plan 2"; FR -> "État Visio Plan 2"; ES -> "Estado de Visio Plan 2"; GR -> "Κατάσταση Visio Plan 2"
    }

    fun effectsLabel(effectsList: String, lang: VexioAppLanguage): String {
        val clean = if (effectsList.isEmpty()) {
            when (lang) { EN -> "Standard"; DE -> "Standard"; IT -> "Standard"; FR -> "Standard"; ES -> "Estándar"; GR -> "Τυπικό" }
        } else {
            effectsList
        }
        return when (lang) {
            EN -> "Effects: $clean"; DE -> "Effekte: $clean"; IT -> "Effetti: $clean"; FR -> "Effets : $clean"; ES -> "Efectos: $clean"; GR -> "Εφέ: $clean"
        }
    }

    fun hardwareShortcutsTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Googlebook Hardware Shortcuts"
        DE -> "Googlebook-Tastenkombinationen"
        IT -> "Scorciatoie da tastiera Googlebook"
        FR -> "Raccourcis clavier Googlebook"
        ES -> "Atajos de teclado de Googlebook"
        GR -> "Συντομεύσεις πληκτρολογίου Googlebook"
    }

    fun switchToPageItem(pageName: String, scale: String, lang: VexioAppLanguage): String = when (lang) {
        EN -> "Switch to $pageName ($scale)"
        DE -> "Zu $pageName wechseln ($scale)"
        IT -> "Passa a $pageName ($scale)"
        FR -> "Basculer vers $pageName ($scale)"
        ES -> "Cambiar a $pageName ($scale)"
        GR -> "Μετάβαση σε $pageName ($scale)"
    }

    fun statusCursorAndTool(cursor: VexioCursorKind, tool: VexioTool, lang: VexioAppLanguage): String {
        val tName = toolName(tool, lang)
        return when (lang) {
            EN -> "Cursor: ${cursor.name} • Tool: $tName"
            DE -> "Zeiger: ${cursor.name} • Werkzeug: $tName"
            IT -> "Cursore: ${cursor.name} • Strumento: $tName"
            FR -> "Curseur : ${cursor.name} • Outil : $tName"
            ES -> "Cursor: ${cursor.name} • Herramienta: $tName"
            GR -> "Δρομέας: ${cursor.name} • Εργαλείο: $tName"
        }
    }

    fun toolName(tool: VexioTool, lang: VexioAppLanguage): String = when (tool) {
        VexioTool.POINTER -> when (lang) { EN -> "Pointer"; DE -> "Zeiger"; IT -> "Puntatore"; FR -> "Pointeur"; ES -> "Puntero"; GR -> "Δείκτης" }
        VexioTool.PAN_HAND -> when (lang) { EN -> "Pan"; DE -> "Verschieben"; IT -> "Panoramica"; FR -> "Main"; ES -> "Desplazar"; GR -> "Μετακίνηση" }
        VexioTool.CONNECTOR -> when (lang) { EN -> "Connector"; DE -> "Verbinder"; IT -> "Connettore"; FR -> "Connecteur"; ES -> "Conector"; GR -> "Σύνδεση" }
        VexioTool.LINE -> when (lang) { EN -> "Line"; DE -> "Linie"; IT -> "Linea"; FR -> "Ligne"; ES -> "Línea"; GR -> "Γραμμή" }
        VexioTool.ARC -> when (lang) { EN -> "Arc"; DE -> "Bogen"; IT -> "Arco"; FR -> "Arc"; ES -> "Arco"; GR -> "Τόξο" }
        VexioTool.FREEFORM -> when (lang) { EN -> "Freeform"; DE -> "Freihandform"; IT -> "Forma libera"; FR -> "Forme libre"; ES -> "Forma libre"; GR -> "Ελεύθερο σχήμα" }
        VexioTool.RECTANGLE -> when (lang) { EN -> "Rectangle"; DE -> "Rechteck"; IT -> "Rettangolo"; FR -> "Rectangle"; ES -> "Rectángulo"; GR -> "Ορθογώνιο" }
        VexioTool.ROUNDED_RECT -> when (lang) { EN -> "Rounded"; DE -> "Abgerundet"; IT -> "Arrotondato"; FR -> "Arrondi"; ES -> "Redondeado"; GR -> "Στρογγυλεμένο" }
        VexioTool.ELLIPSE -> when (lang) { EN -> "Ellipse"; DE -> "Ellipse"; IT -> "Ellisse"; FR -> "Ellipse"; ES -> "Elipse"; GR -> "Έλλειψη" }
        VexioTool.DIAMOND -> when (lang) { EN -> "Diamond"; DE -> "Raute"; IT -> "Rombo"; FR -> "Losange"; ES -> "Rombo"; GR -> "Ρόμβος" }
        VexioTool.TRIANGLE -> when (lang) { EN -> "Triangle"; DE -> "Dreieck"; IT -> "Triangolo"; FR -> "Triangle"; ES -> "Triángulo"; GR -> "Τρίγωνο" }
        VexioTool.HEXAGON -> when (lang) { EN -> "Hexagon"; DE -> "Sechseck"; IT -> "Esagono"; FR -> "Hexagone"; ES -> "Hexágono"; GR -> "Εξάγωνο" }
        VexioTool.STAR -> when (lang) { EN -> "Star"; DE -> "Stern"; IT -> "Stella"; FR -> "Étoile"; ES -> "Estrella"; GR -> "Αστέρι" }
        VexioTool.TEXT_BLOCK -> when (lang) { EN -> "Text"; DE -> "Text"; IT -> "Testo"; FR -> "Texte"; ES -> "Texto"; GR -> "Κείμενο" }
        VexioTool.STYLUS_INK -> when (lang) { EN -> "Pen Ink"; DE -> "Freihand"; IT -> "Inchiostro"; FR -> "Encre"; ES -> "Tinta"; GR -> "Μελάνι" }
        VexioTool.HIGHLIGHTER -> highlighterPill(lang)
        VexioTool.EDIT_VERTICES -> when (lang) { EN -> "Edit Vertices"; DE -> "Scheitelpunkte"; IT -> "Modifica vertici"; FR -> "Modifier sommets"; ES -> "Editar vértices"; GR -> "Κορυφές" }
        VexioTool.INK_ERASER -> when (lang) { EN -> "Ink Eraser"; DE -> "Radierer"; IT -> "Gomma"; FR -> "Gomme"; ES -> "Borrador"; GR -> "Γόμα" }
        VexioTool.CONNECTION_POINT -> when (lang) { EN -> "Connection Point"; DE -> "Verbindungspunkt"; IT -> "Punto connessione"; FR -> "Point de connexion"; ES -> "Punto de conexión"; GR -> "Σημείο σύνδεσης" }
        VexioTool.TEXT_BLOCK_TRANSFORM -> when (lang) { EN -> "Text Block"; DE -> "Textblock"; IT -> "Blocco testo"; FR -> "Bloc texte"; ES -> "Bloque texto"; GR -> "Πλαίσιο κειμένου" }
        VexioTool.FORMAT_PAINTER -> when (lang) { EN -> "Format Painter"; DE -> "Format übertragen"; IT -> "Copia formato"; FR -> "Reproduire format"; ES -> "Copiar formato"; GR -> "Πινέλο μορφοποίησης" }
        VexioTool.WALL -> when (lang) { EN -> "Wall"; DE -> "Wand"; IT -> "Muro"; FR -> "Mur"; ES -> "Muro"; GR -> "Τοίχος" }
        VexioTool.DIMENSION -> when (lang) { EN -> "Dimension"; DE -> "Bemaßung"; IT -> "Quota"; FR -> "Cotation"; ES -> "Cota"; GR -> "Διάσταση" }
    }

    fun booleanOpName(op: VexioBooleanOp, lang: VexioAppLanguage): String = when (op) {
        VexioBooleanOp.UNION -> when (lang) { EN -> "Union"; DE -> "Vereinigen"; IT -> "Unione"; FR -> "Union"; ES -> "Unión"; GR -> "Ένωση" }
        VexioBooleanOp.COMBINE -> when (lang) { EN -> "Combine"; DE -> "Kombinieren"; IT -> "Combina"; FR -> "Combiner"; ES -> "Combinar"; GR -> "Συνδυασμός" }
        VexioBooleanOp.FRAGMENT -> when (lang) { EN -> "Fragment"; DE -> "Zerlegen"; IT -> "Frammenta"; FR -> "Fragmenter"; ES -> "Fragmentar"; GR -> "Κατάτμηση" }
        VexioBooleanOp.INTERSECT -> when (lang) { EN -> "Intersect"; DE -> "Schnittmenge"; IT -> "Interseca"; FR -> "Intersection"; ES -> "Intersecar"; GR -> "Τομή" }
        VexioBooleanOp.SUBTRACT -> when (lang) { EN -> "Subtract"; DE -> "Subtrahieren"; IT -> "Sottrai"; FR -> "Soustraire"; ES -> "Restar"; GR -> "Αφαίρεση" }
        VexioBooleanOp.TRIM -> when (lang) { EN -> "Trim"; DE -> "Zuschneiden"; IT -> "Taglia"; FR -> "Ajuster"; ES -> "Recortar"; GR -> "Περικοπή" }
        VexioBooleanOp.JOIN -> when (lang) { EN -> "Join"; DE -> "Verbinden"; IT -> "Unisci"; FR -> "Joindre"; ES -> "Unir"; GR -> "Σύνδεση" }
        VexioBooleanOp.OFFSET -> when (lang) { EN -> "Offset"; DE -> "Versetzen"; IT -> "Scostamento"; FR -> "Décalage"; ES -> "Desfase"; GR -> "Μετατόπιση" }
    }

    fun themePresetName(preset: VexioThemePreset, lang: VexioAppLanguage): String = when (preset) {
        VexioThemePreset.MODERN -> when (lang) { EN -> "Modern"; DE -> "Modern"; IT -> "Moderno"; FR -> "Moderne"; ES -> "Moderno"; GR -> "Μοντέρνο" }
        VexioThemePreset.PROFESSIONAL -> when (lang) { EN -> "Professional"; DE -> "Professionell"; IT -> "Professionale"; FR -> "Professionnel"; ES -> "Profesional"; GR -> "Επαγγελματικό" }
        VexioThemePreset.VIBRANT -> when (lang) { EN -> "Vibrant"; DE -> "Lebendig"; IT -> "Vivace"; FR -> "Dynamique"; ES -> "Vibrante"; GR -> "Ζωντανό" }
        VexioThemePreset.BLUEPRINT -> when (lang) { EN -> "Blueprint"; DE -> "Blaupause"; IT -> "Cianografia"; FR -> "Plan bleu"; ES -> "Plano azul"; GR -> "Σχέδιο Blueprint" }
        VexioThemePreset.MONOCHROME -> when (lang) { EN -> "Monochrome"; DE -> "Monochrom"; IT -> "Monocromatico"; FR -> "Monochrome"; ES -> "Monocromo"; GR -> "Μονόχρωμο" }
        VexioThemePreset.HAND_DRAWN -> when (lang) { EN -> "Hand Drawn"; DE -> "Handgezeichnet"; IT -> "Disegnato a mano"; FR -> "Dessiné à la main"; ES -> "Dibujado a mano"; GR -> "Χειρόγραφο" }
    }

    fun visualPresetName(preset: VexioVisualPreset, lang: VexioAppLanguage): String = when (preset) {
        VexioVisualPreset.GRAD_LINEAR -> when (lang) { EN -> "Linear Gradient"; DE -> "Linearer Verlauf"; IT -> "Sfumatura lineare"; FR -> "Dégradé linéaire"; ES -> "Degradado lineal"; GR -> "Γραμμική διαβάθμιση" }
        VexioVisualPreset.SHADOW_SOFT -> when (lang) { EN -> "Soft Shadow"; DE -> "Weicher Schatten"; IT -> "Ombra morbida"; FR -> "Ombre douce"; ES -> "Sombra suave"; GR -> "Απαλή σκιά" }
        VexioVisualPreset.BEVEL_3D -> when (lang) { EN -> "3D Bevel"; DE -> "3D-Abschrägung"; IT -> "Smusso 3D"; FR -> "Biseau 3D"; ES -> "Bisel 3D"; GR -> "Ανάγλυφο 3D" }
        VexioVisualPreset.SKETCHY -> when (lang) { EN -> "Sketchy Outline"; DE -> "Skizzenkontur"; IT -> "Contorno schizzo"; FR -> "Contour esquissé"; ES -> "Contorno boceto"; GR -> "Περίγραμμα σκίτσου" }
    }

    fun pagePresetLabel(preset: VexioPagePresetSpec, lang: VexioAppLanguage): String = when (preset) {
        VexioPagePresetSpec.A4_LANDSCAPE -> blankA4LandscapeTitle(lang).removePrefix("Blank ").removePrefix("Leeres ").removePrefix("Κενό ")
        VexioPagePresetSpec.A4_PORTRAIT -> blankA4PortraitTitle(lang).removePrefix("Blank ").removePrefix("Leeres ").removePrefix("Κενό ")
        VexioPagePresetSpec.A3_LANDSCAPE -> blankA3LandscapeTitle(lang).removePrefix("Blank ").removePrefix("Leeres ").removePrefix("Κενό ")
        VexioPagePresetSpec.LETTER_LANDSCAPE -> "US Letter (11 × 8.5 in)"
        else -> preset.label
    }

    fun objectInspectorTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Object Inspector"; DE -> "Objektinspektor"; IT -> "Ispettore oggetto"; FR -> "Inspecteur d’objet"; ES -> "Inspector de objetos"; GR -> "Επιθεωρητής αντικειμένου"
    }

    fun pagePropertiesTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Page Properties"; DE -> "Seiteneigenschaften"; IT -> "Proprietà pagina"; FR -> "Propriétés de page"; ES -> "Propiedades de página"; GR -> "Ιδιότητες σελίδας"
    }

    fun objectNameLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Name"; DE -> "Name"; IT -> "Nome"; FR -> "Nom"; ES -> "Nombre"; GR -> "Όνομα"
    }

    fun objectCaptionLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Text"; DE -> "Text"; IT -> "Testo"; FR -> "Texte"; ES -> "Texto"; GR -> "Κείμενο"
    }

    fun transparencyLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Transparency"; DE -> "Transparenz"; IT -> "Trasparenza"; FR -> "Transparence"; ES -> "Transparencia"; GR -> "Διαφάνεια"
    }

    fun lineWeightLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Line Weight"; DE -> "Linienstärke"; IT -> "Spessore linea"; FR -> "Épaisseur"; ES -> "Grosor de línea"; GR -> "Πάχος γραμμής"
    }

    fun widthLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Width"; DE -> "Breite"; IT -> "Larghezza"; FR -> "Largeur"; ES -> "Ancho"; GR -> "Πλάτος"
    }

    fun heightLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Height"; DE -> "Höhe"; IT -> "Altezza"; FR -> "Hauteur"; ES -> "Alto"; GR -> "Ύψος"
    }

    fun angleLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Angle"; DE -> "Winkel"; IT -> "Angolo"; FR -> "Angle"; ES -> "Ángulo"; GR -> "Γωνία"
    }

    fun protectionTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Protection"; DE -> "Schutz"; IT -> "Protezione"; FR -> "Protection"; ES -> "Protección"; GR -> "Προστασία"
    }

    fun lockAspectLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Lock Aspect Ratio"; DE -> "Seitenverhältnis sperren"; IT -> "Blocca proporzioni"; FR -> "Verrouiller proportions"; ES -> "Bloquear proporción"; GR -> "Κλείδωμα αναλογίας"
    }

    fun lockSizeLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Lock Width & Height"; DE -> "Größe sperren"; IT -> "Blocca dimensioni"; FR -> "Verrouiller taille"; ES -> "Bloquear tamaño"; GR -> "Κλείδωμα μεγέθους"
    }

    fun lockPositionLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Lock Position"; DE -> "Position sperren"; IT -> "Blocca posizione"; FR -> "Verrouiller position"; ES -> "Bloquear posición"; GR -> "Κλείδωμα θέσης"
    }

    fun fitToScreenLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Fit to Screen"; DE -> "An Bildschirm anpassen"; IT -> "Adatta allo schermo"; FR -> "Ajuster à l’écran"; ES -> "Ajustar a pantalla"; GR -> "Προσαρμογή στην οθόνη"
    }

    fun addFieldLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Add Shape Data Field"; DE -> "Formdatenfeld hinzufügen"; IT -> "Aggiungi campo dati"; FR -> "Ajouter un champ"; ES -> "Añadir campo de datos"; GR -> "Προσθήκη πεδίου δεδομένων"
    }

    fun addLayerLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Add Layer"; DE -> "Layer hinzufügen"; IT -> "Aggiungi layer"; FR -> "Ajouter un calque"; ES -> "Añadir capa"; GR -> "Προσθήκη επιπέδου"
    }

    fun pageSetupMenuLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Page Setup…"; DE -> "Seite einrichten…"; IT -> "Imposta pagina…"; FR -> "Mise en page…"; ES -> "Configurar página…"; GR -> "Διαμόρφωση σελίδας…"
    }

    fun duplicatePageMenuLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Duplicate Page"; DE -> "Seite duplizieren"; IT -> "Duplica pagina"; FR -> "Dupliquer la page"; ES -> "Duplicar página"; GR -> "Διπλότυπο σελίδας"
    }

    fun deletePageMenuLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Delete Page"; DE -> "Seite löschen"; IT -> "Elimina pagina"; FR -> "Supprimer la page"; ES -> "Eliminar página"; GR -> "Διαγραφή σελίδας"
    }

    fun pageBackgroundTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Background"; DE -> "Hintergrund"; IT -> "Sfondo"; FR -> "Arrière-plan"; ES -> "Fondo"; GR -> "Φόντο"
    }

    fun newBackgroundPageLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "New Background Page"; DE -> "Neue Hintergrundseite"; IT -> "Nuova pagina di sfondo"; FR -> "Nouvel arrière-plan"; ES -> "Nueva página de fondo"; GR -> "Νέα σελίδα φόντου"
    }

    fun markAsBackgroundLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Background Page"; DE -> "Als Hintergrundseite"; IT -> "Pagina di sfondo"; FR -> "Page d’arrière-plan"; ES -> "Página de fondo"; GR -> "Σελίδα φόντου"
    }

    fun orientationLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Orientation"; DE -> "Ausrichtung"; IT -> "Orientamento"; FR -> "Orientation"; ES -> "Orientación"; GR -> "Προσανατολισμός"
    }

    fun landscapeLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Landscape"; DE -> "Querformat"; IT -> "Orizzontale"; FR -> "Paysage"; ES -> "Horizontal"; GR -> "Οριζόντια"
    }

    fun portraitLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Portrait"; DE -> "Hochformat"; IT -> "Verticale"; FR -> "Portrait"; ES -> "Vertical"; GR -> "Κατακόρυφη"
    }

    fun paperSizeLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Paper Size & Dimensions"; DE -> "Papierformat & Maße"; IT -> "Formato carta"; FR -> "Format du papier"; ES -> "Tamaño de papel"; GR -> "Μέγεθος χαρτιού"
    }

    fun paperColorLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Page Color"; DE -> "Seitenfarbe"; IT -> "Colore pagina"; FR -> "Couleur de page"; ES -> "Color de página"; GR -> "Χρώμα σελίδας"
    }

    fun linkedBackgroundLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Assigned Background Page"; DE -> "Zugewiesene Hintergrundseite"; IT -> "Sfondo assegnato"; FR -> "Arrière-plan associé"; ES -> "Página de fondo asignada"; GR -> "Σελίδα φόντου"
    }

    fun noneLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "None"; DE -> "Keine"; IT -> "Nessuno"; FR -> "Aucun"; ES -> "Ninguno"; GR -> "Καμία"
    }

    fun fillSectionTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Fill"; DE -> "Füllung"; IT -> "Riempimento"; FR -> "Remplissage"; ES -> "Relleno"; GR -> "Γέμισμα"
    }

    fun lineArrowsSectionTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Line & Arrows"; DE -> "Linie & Pfeile"; IT -> "Linea e frecce"; FR -> "Trait et flèches"; ES -> "Línea y flechas"; GR -> "Γραμμή & βέλη"
    }

    fun typographySectionTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Typography"; DE -> "Schrift & Text"; IT -> "Tipografia"; FR -> "Typographie"; ES -> "Tipografía"; GR -> "Τυπογραφία"
    }

    fun transformSectionTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Position & Size"; DE -> "Position & Größe"; IT -> "Posizione e dimensioni"; FR -> "Position et taille"; ES -> "Posición y tamaño"; GR -> "Θέση & μέγεθος"
    }

    fun arrangeSectionTitle(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Arrange"; DE -> "Anordnen"; IT -> "Disponi"; FR -> "Organiser"; ES -> "Organizar"; GR -> "Διάταξη"
    }

    fun noFillPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "None"; DE -> "Keine"; IT -> "Nessuno"; FR -> "Aucun"; ES -> "Sin"; GR -> "Χωρίς"
    }

    fun solidPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Solid"; DE -> "Vollton"; IT -> "Unito"; FR -> "Uni"; ES -> "Sólido"; GR -> "Συμπαγές"
    }

    fun gradientPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Gradient"; DE -> "Verlauf"; IT -> "Sfumatura"; FR -> "Dégradé"; ES -> "Degradado"; GR -> "Διαβάθμιση"
    }

    fun dashedPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Dashed"; DE -> "Gestrichelt"; IT -> "Tratt."; FR -> "Tirets"; ES -> "Guiones"; GR -> "Παύλες"
    }

    fun dottedPill(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Dotted"; DE -> "Gepunktet"; IT -> "Punt."; FR -> "Pointillés"; ES -> "Puntos"; GR -> "Τελείες"
    }

    fun lockRotateLabel(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Lock Rotation"; DE -> "Drehung sperren"; IT -> "Blocca rotazione"; FR -> "Verrouiller rotation"; ES -> "Bloquear rotación"; GR -> "Κλείδωμα περιστροφής"
    }

    fun undoTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Undo (Ctrl+Z)"; DE -> "Rückgängig (Ctrl+Z)"; IT -> "Annulla (Ctrl+Z)"; FR -> "Annuler (Ctrl+Z)"; ES -> "Deshacer (Ctrl+Z)"; GR -> "Αναίρεση (Ctrl+Z)"
    }

    fun redoTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Redo (Ctrl+Y)"; DE -> "Wiederholen (Ctrl+Y)"; IT -> "Ripeti (Ctrl+Y)"; FR -> "Rétablir (Ctrl+Y)"; ES -> "Rehacer (Ctrl+Y)"; GR -> "Επανάληψη (Ctrl+Y)"
    }

    fun zoomOutTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Zoom Out"; DE -> "Verkleinern"; IT -> "Riduci zoom"; FR -> "Zoom arrière"; ES -> "Alejar"; GR -> "Σμίκρυνση"
    }

    fun zoomInTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Zoom In"; DE -> "Vergrößern"; IT -> "Ingrandisci"; FR -> "Zoom avant"; ES -> "Acercar"; GR -> "Μεγέθυνση"
    }

    fun zoomResetTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Reset Zoom (100%)"; DE -> "Zoom zurücksetzen (100%)"; IT -> "Reimposta zoom (100%)"; FR -> "Réinitialiser le zoom (100%)"; ES -> "Restablecer zoom (100%)"; GR -> "Επαναφορά ζουμ (100%)"
    }

    fun floatToolbarTooltip(isFloating: Boolean, lang: VexioAppLanguage): String = when (lang) {
        EN -> if (isFloating) "Dock Toolbar" else "Float Toolbar"
        DE -> if (isFloating) "Symbolleiste andocken" else "Symbolleiste abdocken"
        IT -> if (isFloating) "Ancora barra" else "Barra mobile"
        FR -> if (isFloating) "Ancrer la barre" else "Barre flottante"
        ES -> if (isFloating) "Anclar barra" else "Barra flotante"
        GR -> if (isFloating) "Αγκύρωση γραμμής" else "Πλωτή γραμμή"
    }

    fun rotateLeftTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Rotate 90° Left"; DE -> "90° links drehen"; IT -> "Ruota 90° a sinistra"; FR -> "Rotation 90° gauche"; ES -> "Girar 90° izq."; GR -> "Περιστροφή 90° αριστ."
    }

    fun rotateRightTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Rotate 90° Right"; DE -> "90° rechts drehen"; IT -> "Ruota 90° a destra"; FR -> "Rotation 90° droite"; ES -> "Girar 90° der."; GR -> "Περιστροφή 90° δεξιά"
    }

    fun flipHorizTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Flip Horizontal"; DE -> "Horizontal spiegeln"; IT -> "Rifletti orizz."; FR -> "Retourner horiz."; ES -> "Voltear horiz."; GR -> "Οριζόντια αναστροφή"
    }

    fun flipVertTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Flip Vertical"; DE -> "Vertikal spiegeln"; IT -> "Rifletti vert."; FR -> "Retourner vert."; ES -> "Voltear vert."; GR -> "Κατακόρυφη αναστροφή"
    }

    fun bringToFrontTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Bring to Front"; DE -> "In den Vordergrund"; IT -> "Porta in primo piano"; FR -> "Mettre au premier plan"; ES -> "Traer al frente"; GR -> "Μεταφορά μπροστά"
    }

    fun sendToBackTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Send to Back"; DE -> "In den Hintergrund"; IT -> "Porta in secondo piano"; FR -> "Mettre à l’arrière-plan"; ES -> "Enviar al fondo"; GR -> "Μεταφορά πίσω"
    }

    fun toggleStencilsTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Toggle Stencil Pane"; DE -> "Schablonen ein-/ausblenden"; IT -> "Mostra/nascondi stencil"; FR -> "Afficher/masquer gabarits"; ES -> "Mostrar/ocultar galerías"; GR -> "Εμφάνιση/απόκρυψη stencil"
    }

    fun toggleShapeSheetTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "ShapeSheet Studio (Shift+F5)"; DE -> "ShapeSheet-Studio (Shift+F5)"; IT -> "ShapeSheet Studio (Shift+F5)"; FR -> "Studio ShapeSheet (Shift+F5)"; ES -> "Estudio ShapeSheet (Shift+F5)"; GR -> "ShapeSheet Studio (Shift+F5)"
    }

    fun toggleInspectorTooltip(lang: VexioAppLanguage): String = when (lang) {
        EN -> "Toggle Object Inspector"; DE -> "Objektinspektor ein-/ausblenden"; IT -> "Mostra/nascondi ispettore"; FR -> "Afficher/masquer inspecteur"; ES -> "Mostrar/ocultar inspector"; GR -> "Εμφάνιση/απόκρυψη επιθεωρητή"
    }
}
