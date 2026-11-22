export type VexioLangCode = 'en' | 'de' | 'it' | 'fr' | 'es' | 'gr';

export interface VexioStrings {
  code: VexioLangCode;
  badge: string;
  nativeName: string;
  fileMenu: string;
  editMenu: string;
  exportMenu: string;
  helpMenu: string;
  homeTab: string;
  insertTab: string;
  designTab: string;
  dataTab: string;
  processTab: string;
  reviewTab: string;
  viewTab: string;
  developerTab: string;
  startHeroTitle: string;
  startHeroSubtitle: string;
  continueToCanvas: string;
  newBlankA4: string;
  newBlankA3: string;
  newBlankLetter: string;
  openLocalFile: string;
  importVisioStencil: string;
  checkUpdates: string;
  autoCheckUpdates: string;
  rotateLeft90: string;
  rotateRight90: string;
  flipHoriz: string;
  flipVert: string;
  editLabel: string;
  renameHint: string;
}

export const VEXIO_TRANSLATIONS: Record<VexioLangCode, VexioStrings> = {
  en: {
    code: 'en',
    badge: 'EN',
    nativeName: 'English',
    fileMenu: 'File',
    editMenu: 'Edit',
    exportMenu: 'Export',
    helpMenu: 'Help',
    homeTab: 'Home',
    insertTab: 'Insert',
    designTab: 'Design',
    dataTab: 'Data',
    processTab: 'Process',
    reviewTab: 'Review',
    viewTab: 'View',
    developerTab: 'Developer',
    startHeroTitle: 'Create or Open a Diagram Inside Vexio',
    startHeroSubtitle: '1:1 Microsoft Visio Professional Parity • 100% Offline-First Workstation',
    continueToCanvas: 'Continue to Canvas',
    newBlankA4: 'Blank ISO A4 (297×210 mm)',
    newBlankA3: 'Blank ISO A3 (420×297 mm)',
    newBlankLetter: 'Blank US Letter (11×8.5 in)',
    openLocalFile: 'Open Visio / CAD File…',
    importVisioStencil: '+ Import Visio Stencil (.vssx / .vss)',
    checkUpdates: 'Check for Updates…',
    autoCheckUpdates: 'Automatically check for updates on startup',
    rotateLeft90: 'Rotate Left 90° (↺)',
    rotateRight90: 'Rotate Right 90° (↻)',
    flipHoriz: 'Flip Horizontal (⇆)',
    flipVert: 'Flip Vertical (⇅)',
    editLabel: 'Edit Shape Label',
    renameHint: 'Double-click file name to select & rename inline',
  },
  de: {
    code: 'de',
    badge: 'DE',
    nativeName: 'Deutsch',
    fileMenu: 'Datei',
    editMenu: 'Bearbeiten',
    exportMenu: 'Exportieren',
    helpMenu: 'Hilfe',
    homeTab: 'Start',
    insertTab: 'Einfügen',
    designTab: 'Entwurf',
    dataTab: 'Daten',
    processTab: 'Prozess',
    reviewTab: 'Überprüfen',
    viewTab: 'Ansicht',
    developerTab: 'Entwickler',
    startHeroTitle: 'Diagramm in Vexio erstellen oder öffnen',
    startHeroSubtitle: '1:1 Microsoft Visio Professional Parität • 100 % Offline-Workstation',
    continueToCanvas: 'Weiter zur Zeichenfläche',
    newBlankA4: 'Leeres ISO A4 (297×210 mm)',
    newBlankA3: 'Leeres ISO A3 (420×297 mm)',
    newBlankLetter: 'Leeres US Letter (11×8,5 Zoll)',
    openLocalFile: 'Visio- / CAD-Datei öffnen…',
    importVisioStencil: '+ Visio-Schablone importieren (.vssx / .vss)',
    checkUpdates: 'Nach Updates suchen…',
    autoCheckUpdates: 'Beim Start automatisch nach Updates suchen',
    rotateLeft90: '90° links drehen (↺)',
    rotateRight90: '90° rechts drehen (↻)',
    flipHoriz: 'Horizontal spiegeln (⇆)',
    flipVert: 'Vertikal spiegeln (⇅)',
    editLabel: 'Beschriftung bearbeiten',
    renameHint: 'Doppelklicken zum Umbenennen',
  },
  it: {
    code: 'it',
    badge: 'IT',
    nativeName: 'Italiano',
    fileMenu: 'File',
    editMenu: 'Modifica',
    exportMenu: 'Esporta',
    helpMenu: 'Aiuto',
    homeTab: 'Home',
    insertTab: 'Inserisci',
    designTab: 'Progettazione',
    dataTab: 'Dati',
    processTab: 'Processo',
    reviewTab: 'Revisione',
    viewTab: 'Visualizza',
    developerTab: 'Sviluppo',
    startHeroTitle: 'Crea o apri un diagramma in Vexio',
    startHeroSubtitle: 'Parità 1:1 Microsoft Visio Professional • Workstation 100% Offline',
    continueToCanvas: 'Vai al foglio di disegno',
    newBlankA4: 'Vuoto ISO A4 (297×210 mm)',
    newBlankA3: 'Vuoto ISO A3 (420×297 mm)',
    newBlankLetter: 'Vuoto US Letter (11×8,5 in)',
    openLocalFile: 'Apri file Visio / CAD…',
    importVisioStencil: '+ Importa Stencil Visio (.vssx / .vss)',
    checkUpdates: 'Controlla aggiornamenti…',
    autoCheckUpdates: 'Controlla automaticamente aggiornamenti all’avvio',
    rotateLeft90: 'Ruota 90° a sinistra (↺)',
    rotateRight90: 'Ruota 90° a destra (↻)',
    flipHoriz: 'Capovolgi orizzontale (⇆)',
    flipVert: 'Capovolgi verticale (⇅)',
    editLabel: 'Modifica testo forma',
    renameHint: 'Doppio clic sul nome file per rinominare',
  },
  fr: {
    code: 'fr',
    badge: 'FR',
    nativeName: 'Français',
    fileMenu: 'Fichier',
    editMenu: 'Édition',
    exportMenu: 'Exporter',
    helpMenu: 'Aide',
    homeTab: 'Accueil',
    insertTab: 'Insertion',
    designTab: 'Création',
    dataTab: 'Données',
    processTab: 'Processus',
    reviewTab: 'Révision',
    viewTab: 'Affichage',
    developerTab: 'Développeur',
    startHeroTitle: 'Créer ou ouvrir un diagramme dans Vexio',
    startHeroSubtitle: 'Parité 1:1 Microsoft Visio Professionnel • Poste 100 % hors ligne',
    continueToCanvas: 'Continuer vers le dessin',
    newBlankA4: 'Vierge ISO A4 (297×210 mm)',
    newBlankA3: 'Vierge ISO A3 (420×297 mm)',
    newBlankLetter: 'Vierge US Letter (11×8,5 po)',
    openLocalFile: 'Ouvrir un fichier Visio / CAO…',
    importVisioStencil: '+ Importer gabarit Visio (.vssx / .vss)',
    checkUpdates: 'Rechercher des mises à jour…',
    autoCheckUpdates: 'Vérifier automatiquement les mises à jour au démarrage',
    rotateLeft90: 'Pivoter 90° gauche (↺)',
    rotateRight90: 'Pivoter 90° droite (↻)',
    flipHoriz: 'Retourner horizontalement (⇆)',
    flipVert: 'Retourner verticalement (⇅)',
    editLabel: 'Modifier le texte',
    renameHint: 'Double-cliquer sur le nom pour renommer',
  },
  es: {
    code: 'es',
    badge: 'ES',
    nativeName: 'Español',
    fileMenu: 'Archivo',
    editMenu: 'Editar',
    exportMenu: 'Exportar',
    helpMenu: 'Ayuda',
    homeTab: 'Inicio',
    insertTab: 'Insertar',
    designTab: 'Diseño',
    dataTab: 'Datos',
    processTab: 'Proceso',
    reviewTab: 'Revisar',
    viewTab: 'Vista',
    developerTab: 'Programador',
    startHeroTitle: 'Crear o abrir un diagrama en Vexio',
    startHeroSubtitle: 'Paridad 1:1 con Microsoft Visio Professional • 100 % sin conexión',
    continueToCanvas: 'Continuar al lienzo',
    newBlankA4: 'En blanco ISO A4 (297×210 mm)',
    newBlankA3: 'En blanco ISO A3 (420×297 mm)',
    newBlankLetter: 'En blanco US Letter (11×8,5 in)',
    openLocalFile: 'Abrir archivo Visio / CAD…',
    importVisioStencil: '+ Importar galería Visio (.vssx / .vss)',
    checkUpdates: 'Buscar actualizaciones…',
    autoCheckUpdates: 'Buscar actualizaciones automáticamente al iniciar',
    rotateLeft90: 'Girar 90° izquierda (↺)',
    rotateRight90: 'Girar 90° derecha (↻)',
    flipHoriz: 'Voltear horizontalmente (⇆)',
    flipVert: 'Voltear verticalmente (⇅)',
    editLabel: 'Editar etiqueta',
    renameHint: 'Doble clic en el nombre para renombrar',
  },
  gr: {
    code: 'gr',
    badge: 'GR',
    nativeName: 'Ελληνικά',
    fileMenu: 'Αρχείο',
    editMenu: 'Επεξεργασία',
    exportMenu: 'Εξαγωγή',
    helpMenu: 'Βοήθεια',
    homeTab: 'Κεντρική',
    insertTab: 'Εισαγωγή',
    designTab: 'Σχεδίαση',
    dataTab: 'Δεδομένα',
    processTab: 'Διαδικασία',
    reviewTab: 'Αναθεώρηση',
    viewTab: 'Προβολή',
    developerTab: 'Προγραμματιστής',
    startHeroTitle: 'Δημιουργία ή άνοιγμα διαγράμματος στο Vexio',
    startHeroSubtitle: 'Πλήρης συμβατότητα 1:1 Microsoft Visio Professional • 100% εκτός σύνδεσης',
    continueToCanvas: 'Συνέχεια στον καμβά',
    newBlankA4: 'Κενό ISO A4 (297×210 mm)',
    newBlankA3: 'Κενό ISO A3 (420×297 mm)',
    newBlankLetter: 'Κενό US Letter (11×8,5 in)',
    openLocalFile: 'Άνοιγμα αρχείου Visio / CAD…',
    importVisioStencil: '+ Εισαγωγή Stencil Visio (.vssx / .vss)',
    checkUpdates: 'Έλεγχος για ενημερώσεις…',
    autoCheckUpdates: 'Αυτόματος έλεγχος ενημερώσεων κατά την εκκίνηση',
    rotateLeft90: 'Περιστροφή 90° αριστερά (↺)',
    rotateRight90: 'Περιστροφή 90° δεξιά (↻)',
    flipHoriz: 'Οριζόντια αναστροφή (⇆)',
    flipVert: 'Κατακόρυφη αναστροφή (⇅)',
    editLabel: 'Επεξεργασία ετικέτας',
  renameHint: 'Διπλό κλικ στο όνομα αρχείου για μετονομασία',
  },
};

export interface PagePresetOption {
  id: string;
  slug: string;
  landscape: boolean;
  isMetric: boolean;
  label: string;
  shortLabel: string;
  widthIn: number;
  heightIn: number;
}

export const PAGE_PRESET_OPTIONS: PagePresetOption[] = [
  { id: 'a4_l', slug: 'a4', landscape: true, isMetric: true, label: 'ISO A4 Landscape (297 × 210 mm)', shortLabel: 'A4 Land', widthIn: 11.6929, heightIn: 8.2677 },
  { id: 'a4_p', slug: 'a4', landscape: false, isMetric: true, label: 'ISO A4 Portrait (210 × 297 mm)', shortLabel: 'A4 Port', widthIn: 8.2677, heightIn: 11.6929 },
  { id: 'a3_l', slug: 'a3', landscape: true, isMetric: true, label: 'ISO A3 Landscape (420 × 297 mm)', shortLabel: 'A3 Land', widthIn: 16.5354, heightIn: 11.6929 },
  { id: 'a3_p', slug: 'a3', landscape: false, isMetric: true, label: 'ISO A3 Portrait (297 × 420 mm)', shortLabel: 'A3 Port', widthIn: 11.6929, heightIn: 16.5354 },
  { id: 'a5_l', slug: 'a5', landscape: true, isMetric: true, label: 'ISO A5 Landscape (210 × 148 mm)', shortLabel: 'A5', widthIn: 8.2677, heightIn: 5.8268 },
  { id: 'a2_l', slug: 'a2', landscape: true, isMetric: true, label: 'ISO A2 Landscape (594 × 420 mm)', shortLabel: 'A2', widthIn: 23.3858, heightIn: 16.5354 },
  { id: 'a1_l', slug: 'a1', landscape: true, isMetric: true, label: 'ISO A1 Landscape (841 × 594 mm)', shortLabel: 'A1', widthIn: 33.1102, heightIn: 23.3858 },
  { id: 'a0_l', slug: 'a0', landscape: true, isMetric: true, label: 'ISO A0 Landscape (1189 × 841 mm)', shortLabel: 'A0', widthIn: 46.8110, heightIn: 33.1102 },
  { id: 'letter_l', slug: 'letter', landscape: true, isMetric: false, label: 'US Letter Landscape (11 × 8.5 in)', shortLabel: 'Letter Land', widthIn: 11.0, heightIn: 8.5 },
  { id: 'letter_p', slug: 'letter', landscape: false, isMetric: false, label: 'US Letter Portrait (8.5 × 11 in)', shortLabel: 'Letter Port', widthIn: 8.5, heightIn: 11.0 },
  { id: 'legal_l', slug: 'legal', landscape: true, isMetric: false, label: 'US Legal Landscape (14 × 8.5 in)', shortLabel: 'Legal', widthIn: 14.0, heightIn: 8.5 },
  { id: 'tabloid_l', slug: 'tabloid', landscape: true, isMetric: false, label: 'Tabloid / ANSI B (17 × 11 in)', shortLabel: 'Tabloid', widthIn: 17.0, heightIn: 11.0 },
];

export function detectDefaultLanguage(): VexioLangCode {
  try {
    const saved = localStorage.getItem('vexio.lang')?.toLowerCase();
    if (saved && saved in VEXIO_TRANSLATIONS) {
      return saved as VexioLangCode;
    }
  } catch {
    // Ignore storage block
  }
  const nav = (typeof navigator !== 'undefined' ? navigator.language : 'en').toLowerCase();
  const primary = nav.split('-')[0];
  if (primary === 'de') return 'de';
  if (primary === 'it') return 'it';
  if (primary === 'fr') return 'fr';
  if (primary === 'es') return 'es';
  if (primary === 'el' || primary === 'gr') return 'gr';
  return 'en';
}

export function detectDefaultMetricUnits(): boolean {
  try {
    const saved = localStorage.getItem('vexio.metric');
    if (saved === 'true') return true;
    if (saved === 'false') return false;
  } catch {
    // Ignore storage block
  }
  const nav = typeof navigator !== 'undefined' ? navigator.language : '';
  const parts = nav.split('-');
  const region = (parts[1] ?? '').toUpperCase();
  if (region === 'US' || region === 'LR' || region === 'MM') {
    return false;
  }
  return true;
}
