/**
 * Native ChromeOS OS-Level Integrations:
 * 1. Window Controls Overlay (`navigator.windowControlsOverlay`) titlebar geometry sync
 * 2. Launch Queue API (`window.launchQueue.setConsumer`) for double-clicking `.vsdx`/`.vsd`/`.dwg` in ChromeOS Files
 * 3. File System Access API (`showOpenFilePicker`, `showSaveFilePicker`, `createWritable`) for direct in-place disk save
 * 4. Local Font Access API (`window.queryLocalFonts`) for 1:1 system typography parity
 * 5. Async Clipboard API (`navigator.clipboard`) for rich SVG + PNG + Vexio ShapeSheet clipboard transfer
 */

export interface OpenedLocalFile {
  name: string;
  ext: string;
  bytes: Uint8Array;
  handle: FileSystemFileHandle | null;
}

interface LaunchParams {
  files: readonly FileSystemFileHandle[];
  targetURL?: string;
}

interface LaunchQueue {
  setConsumer(consumer: (launchParams: LaunchParams) => void | Promise<void>): void;
}

interface LocalFontData {
  family: string;
  fullName: string;
  postscriptName: string;
  style: string;
}

declare global {
  interface Window {
    launchQueue?: LaunchQueue;
    queryLocalFonts?: () => Promise<LocalFontData[]>;
    showOpenFilePicker?: (opts?: unknown) => Promise<FileSystemFileHandle[]>;
    showSaveFilePicker?: (opts?: unknown) => Promise<FileSystemFileHandle>;
  }
  interface Navigator {
    windowControlsOverlay?: {
      visible: boolean;
      getTitlebarAreaRect(): DOMRect;
      addEventListener(type: 'geometrychange', listener: () => void): void;
    };
  }
}

const VISIO_FILE_PICKER_TYPES = [
  {
    description: 'Microsoft Visio, Metafile & CAD Drawings (.vsdx, .vsd, .vssx, .vstx, .vsdm, .dwg, .dxf, .emf, .wmf, .ldif)',
    accept: {
      'application/vnd.ms-visio.drawing.main+xml': ['.vsdx'],
      'application/vnd.ms-visio.drawing.macroEnabled.main+xml': ['.vsdm'],
      'application/vnd.ms-visio.stencil.main+xml': ['.vssx', '.vssm'],
      'application/vnd.ms-visio.template.main+xml': ['.vstx', '.vstm'],
      'application/vnd.visio': ['.vsd', '.vss', '.vst', '.vdx', '.vdw'],
      'application/acad': ['.dwg'],
      'image/vnd.dxf': ['.dxf'],
      'image/x-emf': ['.emf'],
      'image/x-wmf': ['.wmf'],
      'text/x-ldif': ['.ldif'],
    },
  },
];

export function extractExtension(filename: string): string {
  const idx = filename.lastIndexOf('.');
  return idx >= 0 ? filename.slice(idx + 1).toLowerCase() : 'vsdx';
}

/**
 * Bind ChromeOS Launch Queue consumer so opening `.vsdx`, `.vsd`, `.vssx`, `.vstx`, `.vsdm`, or `.dwg`
 * directly from the ChromeOS Files app streams bytes and preserves the writable `FileSystemFileHandle`.
 */
export function registerChromeOsFileLaunchHandler(
  onFileLaunched: (opened: OpenedLocalFile) => void
): void {
  if (!window.launchQueue) return;
  window.launchQueue.setConsumer(async (launchParams) => {
    if (!launchParams.files || launchParams.files.length === 0) return;
    for (const handle of launchParams.files) {
      const file = await handle.getFile();
      const buf = await file.arrayBuffer();
      onFileLaunched({
        name: file.name,
        ext: extractExtension(file.name),
        bytes: new Uint8Array(buf),
        handle,
      });
    }
  });
}

/**
 * Open a Visio or CAD file from local disk using ChromeOS File System Access API (with `<input type="file">` fallback).
 */
export async function openLocalDiagramFile(): Promise<OpenedLocalFile | null> {
  if (window.showOpenFilePicker) {
    try {
      const [handle] = await window.showOpenFilePicker({
        types: VISIO_FILE_PICKER_TYPES,
        multiple: false,
      });
      const file = await handle.getFile();
      const bytes = new Uint8Array(await file.arrayBuffer());
      return {
        name: file.name,
        ext: extractExtension(file.name),
        bytes,
        handle,
      };
    } catch {
      return null;
    }
  }

  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.vsdx,.vsdm,.vssx,.vssm,.vstx,.vstm,.vsd,.vss,.vst,.vdx,.vdw,.dwg,.dxf,.emf,.wmf,.ldif';
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) {
        resolve(null);
        return;
      }
      resolve({
        name: file.name,
        ext: extractExtension(file.name),
        bytes: new Uint8Array(await file.arrayBuffer()),
        handle: null,
      });
    };
    input.click();
  });
}

/**
 * Pick an arbitrary local binary file (e.g. PNG/JPEG image or `.sqlite`/`.db`/`.xlsx`/`.csv`) for embedding or binding.
 */
export async function pickLocalBinaryFile(accept: string): Promise<OpenedLocalFile | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = accept;
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) {
        resolve(null);
        return;
      }
      resolve({
        name: file.name,
        ext: extractExtension(file.name),
        bytes: new Uint8Array(await file.arrayBuffer()),
        handle: null,
      });
    };
    input.click();
  });
}

function pickerSpecForExtension(ext: string): { mime: string; pickerTypes: unknown[] } {
  switch (ext) {
    case 'jpg':
    case 'jpeg':
      return {
        mime: 'image/jpeg',
        pickerTypes: [{ description: 'JPEG Image (.jpg)', accept: { 'image/jpeg': ['.jpg', '.jpeg'] } }],
      };
    case 'png':
      return {
        mime: 'image/png',
        pickerTypes: [{ description: 'PNG Image (.png)', accept: { 'image/png': ['.png'] } }],
      };
    case 'pdf':
      return {
        mime: 'application/pdf',
        pickerTypes: [{ description: 'PDF Document (.pdf)', accept: { 'application/pdf': ['.pdf'] } }],
      };
    case 'svg':
      return {
        mime: 'image/svg+xml',
        pickerTypes: [{ description: 'SVG Vector Drawing (.svg)', accept: { 'image/svg+xml': ['.svg'] } }],
      };
    case 'pptx':
      return {
        mime: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
        pickerTypes: [
          {
            description: 'PowerPoint Presentation (.pptx)',
            accept: { 'application/vnd.openxmlformats-officedocument.presentationml.presentation': ['.pptx'] },
          },
        ],
      };
    case 'docx':
      return {
        mime: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
        pickerTypes: [
          {
            description: 'Word Specification (.docx)',
            accept: { 'application/vnd.openxmlformats-officedocument.wordprocessingml.document': ['.docx'] },
          },
        ],
      };
    case 'xlsx':
      return {
        mime: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
        pickerTypes: [
          {
            description: 'Excel Workbook (.xlsx)',
            accept: { 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet': ['.xlsx'] },
          },
        ],
      };
    case 'dxf':
      return {
        mime: 'image/vnd.dxf',
        pickerTypes: [{ description: 'AutoCAD DXF (.dxf)', accept: { 'image/vnd.dxf': ['.dxf'] } }],
      };
    case 'json':
      return {
        mime: 'application/json',
        pickerTypes: [{ description: 'JSON Definition (.json)', accept: { 'application/json': ['.json'] } }],
      };
    default:
      return {
        mime: 'application/vnd.ms-visio.drawing.main+xml',
        pickerTypes: VISIO_FILE_PICKER_TYPES,
      };
  }
}

/**
 * Save directly in-place on ChromeOS local disk if an existing `FileSystemFileHandle` is held,
 * or prompt via `showSaveFilePicker` to acquire one!
 */
export async function saveLocalDiagramInPlace(
  existingHandle: FileSystemFileHandle | null,
  suggestedName: string,
  bytes: Uint8Array
): Promise<FileSystemFileHandle | null> {
  const ext = extractExtension(suggestedName);
  const { mime, pickerTypes } = pickerSpecForExtension(ext);
  let handle = existingHandle;
  if (!handle && window.showSaveFilePicker) {
    try {
      handle = await window.showSaveFilePicker({
        suggestedName,
        types: pickerTypes,
      });
    } catch (err) {
      if (err instanceof DOMException && err.name === 'AbortError') {
        return null;
      }
      handle = null;
    }
  }

  const standardBytes = new Uint8Array(bytes.byteLength);
  standardBytes.set(bytes);

  if (handle && 'createWritable' in handle) {
    const writable = await handle.createWritable();
    await writable.write(standardBytes);
    await writable.close();
    return handle;
  }

  // Fallback download anchor if browser doesn't expose File System Access API
  const blob = new Blob([standardBytes], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = suggestedName;
  a.click();
  URL.revokeObjectURL(url);
  return null;
}

/**
 * Sync ChromeOS `window-controls-overlay` titlebar metrics to CSS custom properties.
 */
export function bindWindowControlsOverlay(onChange?: (visible: boolean) => void): void {
  const wco = navigator.windowControlsOverlay;
  if (!wco) {
    onChange?.(false);
    return;
  }
  const sync = () => {
    const rect = wco.getTitlebarAreaRect();
    document.documentElement.style.setProperty('--wco-x', `${rect.x}px`);
    document.documentElement.style.setProperty('--wco-width', `${rect.width}px`);
    document.documentElement.style.setProperty('--wco-height', `${Math.max(rect.height, 42)}px`);
    document.documentElement.dataset.wcoVisible = String(wco.visible);
    onChange?.(wco.visible);
  };
  sync();
  wco.addEventListener('geometrychange', sync);
}

/**
 * Enumerate installed ChromeOS system fonts via Local Font Access API for 1:1 Visio typography parity.
 */
export async function enumerateChromeOsSystemFonts(): Promise<string[]> {
  const defaults = ['Roboto', 'Segoe UI', 'Liberation Sans', 'Noto Sans', 'JetBrains Mono', 'Arial'];
  if (!window.queryLocalFonts) return defaults;
  try {
    const fonts = await window.queryLocalFonts();
    const families = Array.from(new Set([...defaults, ...fonts.map((f) => f.family)])).sort();
    return families;
  } catch {
    return defaults;
  }
}

/**
 * Copy rich vector SVG + plain text ShapeSheet representation to ChromeOS Async Clipboard.
 */
export async function copyDiagramVectorToClipboard(svgMarkup: string, summaryText: string): Promise<boolean> {
  if (!navigator.clipboard?.write) return false;
  try {
    const item = new ClipboardItem({
      'text/plain': new Blob([summaryText], { type: 'text/plain' }),
      'image/svg+xml': new Blob([svgMarkup], { type: 'image/svg+xml' }),
    });
    await navigator.clipboard.write([item]);
    return true;
  } catch {
    await navigator.clipboard.writeText(summaryText);
    return true;
  }
}
