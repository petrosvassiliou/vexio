package com.vexio

import android.app.ActivityManager
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.lifecycle.lifecycleScope
import com.vexio.core.VexioSessionViewModel
import com.vexio.ui.theme.VexioMaterial3ExpressiveTheme
import com.vexio.ui.workstation.ACTION_NEW_DOCUMENT_WINDOW
import com.vexio.ui.workstation.EXTRA_DOCUMENT_TEMPLATE
import com.vexio.ui.workstation.GooglebookWorkstationScreen
import com.vexio.ui.workstation.launchNewDocumentWindow
import com.vexio.ui.workstation.launchVexioVectorPrintJob
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Native Googlebook OS & Android Workstation host Activity.
 *
 * Configured for freeform desktop multi-window (`resizeableActivity="true"`,
 * `allow_multiple_resumed_activities="true"`, multi-task document launching), zero-flicker live
 * window resizing, Storage Access Framework (SAF) direct local `.vsdx`/`.vsd`/`.dwg`/`.dxf` I/O,
 * and native vector PDF printing via `PrintManager`.
 */
class MainActivity : ComponentActivity() {

    private val sessionViewModel: VexioSessionViewModel by viewModels()

    /**
     * SAF OpenDocument contract supporting `.vsdx`, `.vsdm`, `.vssx`, `.vstx`, `.vsd`, `.dwg`, `.dxf`.
     */
    private val openDocumentLauncher = registerForActivityResult(
        ActivityResultContracts.OpenDocument()
    ) { uri: Uri? ->
        if (uri != null) {
            takePersistableReadWritePermissionsIfPossible(uri)
            loadDiagramFromSafUri(uri)
        }
    }

    /**
     * SAF CreateDocument contract for Save As persistence to local disk or mounted Google Drive / SMB shares.
     */
    private var finishAfterNextSave: Boolean = false

    private val createDocumentLauncher = registerForActivityResult(
        ActivityResultContracts.CreateDocument("application/vnd.ms-visio.drawing.main+xml")
    ) { targetUri: Uri? ->
        if (targetUri != null) {
            takePersistableReadWritePermissionsIfPossible(targetUri)
            writeDiagramToSafUri(targetUri)
        } else {
            finishAfterNextSave = false
        }
    }

    private var pendingExportFormat: String = "pdf"

    /**
     * SAF CreateDocument contract for 1-click High-DPI JPEG (.jpg) diagram export.
     */
    private val exportJpgLauncher = registerForActivityResult(
        ActivityResultContracts.CreateDocument("image/jpeg")
    ) { targetUri: Uri? ->
        if (targetUri != null) {
            writeExportFormatToSafUri(targetUri, "jpg")
        }
    }

    /**
     * SAF CreateDocument contract for PDF, SVG, DXF, PPTX, DOCX, Power Automate, and Power BI exports.
     */
    private val exportFormatLauncher = registerForActivityResult(
        ActivityResultContracts.CreateDocument("*/*")
    ) { targetUri: Uri? ->
        if (targetUri != null) {
            writeExportFormatToSafUri(targetUri, pendingExportFormat)
        }
    }

    /**
     * SAF OpenDocument contract for importing Visio Stencils (`.vssx`, `.vss`, `.vssm`, `.vsdx`, `.vstx`, `.vdx`).
     */
    private val importStencilLauncher = registerForActivityResult(
        ActivityResultContracts.OpenDocument()
    ) { uri: Uri? ->
        if (uri != null) {
            loadVisioStencilFromSafUri(uri)
        }
    }

    private var pendingWritebackRecordsetId: Int = 0
    private var pendingWritebackFormatExt: String = "xlsx"

    /**
     * SAF OpenDocument contract for linking External Data (`.xlsx`, `.csv`, `.tsv`, `.sqlite`, `.db`, `.json`).
     */
    private val importExternalDataLauncher = registerForActivityResult(
        ActivityResultContracts.OpenDocument()
    ) { uri: Uri? ->
        if (uri != null) {
            takePersistableReadWritePermissionsIfPossible(uri)
            loadExternalDataFromSafUri(uri)
        }
    }

    /**
     * SAF CreateDocument contract for Two-Way Writeback Export (`.xlsx`, `.csv`, `.sqlite`, `.json`).
     */
    private val exportWritebackLauncher = registerForActivityResult(
        ActivityResultContracts.CreateDocument("*/*")
    ) { targetUri: Uri? ->
        if (targetUri != null) {
            writeTwoWayWritebackToSafUri(targetUri, pendingWritebackRecordsetId, pendingWritebackFormatExt)
        }
    }

    /**
     * SAF CreateDocument contract for BOM Shape Inventory Report (`.xlsx`).
     */
    private val exportBomReportLauncher = registerForActivityResult(
        ActivityResultContracts.CreateDocument("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
    ) { targetUri: Uri? ->
        if (targetUri != null) {
            writeBomReportToSafUri(targetUri)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)

        sessionViewModel.attachStencilStorageDirectory(java.io.File(filesDir, "stencils"))

        if (savedInstanceState == null) {
            handleIncomingLaunchIntent(intent)
            sessionViewModel.checkForUpdatesAsync(userInitiated = false)
        }

        setContent {
            val uiState by sessionViewModel.uiState.collectAsState()

            // Synchronize Googlebook OS freeform desktop window title bar & shelf overview label
            LaunchedEffect(uiState.documentTitle, uiState.isDirty) {
                val windowLabel = buildString {
                    if (uiState.isDirty) append("• ")
                    append(uiState.documentTitle)
                    append(" — Vexio")
                }
                title = windowLabel
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    setTaskDescription(
                        ActivityManager.TaskDescription.Builder()
                            .setLabel(windowLabel)
                            .build()
                    )
                } else {
                    @Suppress("DEPRECATION")
                    setTaskDescription(ActivityManager.TaskDescription(windowLabel))
                }
            }

            VexioMaterial3ExpressiveTheme(darkTheme = uiState.isDarkTheme) {
                GooglebookWorkstationScreen(
                    viewModel = sessionViewModel,
                    onOpenDocumentRequested = ::triggerOpenSafPicker,
                    onSaveDocumentRequested = ::triggerSaveInPlaceOrPicker,
                    onSaveAsDocumentRequested = ::triggerSaveAsSafPicker,
                    onImportStencilRequested = ::triggerImportStencilSafPicker,
                    onLinkExternalDataRequested = ::triggerLinkExternalDataSafPicker,
                    onRefreshExternalDataRequested = ::triggerRefreshExternalDataFromDisk,
                    onTwoWayWritebackRequested = ::triggerTwoWayWritebackSafPicker,
                    onExportBomReportRequested = ::triggerExportBomReportSafPicker,
                    onExportJpgRequested = ::triggerExportJpgSafPicker,
                    onExportFormatRequested = ::triggerExportFormatSafPicker,
                    onExternalUriDropped = ::loadDiagramFromSafUri,
                    onSaveAndExitRequested = {
                        finishAfterNextSave = true
                        triggerSaveInPlaceOrPicker()
                    },
                    onDiscardAndExitRequested = {
                        finishAfterNextSave = false
                        finish()
                    },
                    onPrintRequested = {
                        launchVexioVectorPrintJob(this@MainActivity, sessionViewModel)
                    }
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleIncomingLaunchIntent(intent)
    }

    private fun handleIncomingLaunchIntent(incoming: Intent?) {
        if (incoming == null) return
        when (incoming.action) {
            Intent.ACTION_VIEW,
            Intent.ACTION_EDIT -> {
                val targetUri = incoming.data
                if (targetUri != null) {
                    loadDiagramFromSafUri(targetUri)
                }
            }
            ACTION_NEW_DOCUMENT_WINDOW -> {
                val template = incoming.getStringExtra(EXTRA_DOCUMENT_TEMPLATE) ?: "blank"
                val customTitle = incoming.getStringExtra("vexio.doc_title")
                val initialTitle = when {
                    !customTitle.isNullOrBlank() -> customTitle
                    template == "bpmn2" || template == "bpmn_fulfillment" -> "BPMN2_Order_Fulfillment.vsdx"
                    template == "cloud_microservices" -> "Cloud_Kubernetes_Architecture.vsdx"
                    template == "cad_floorplan" -> "Architectural_Floorplan.vsdx"
                    else -> "Untitled_Diagram.vsdx"
                }
                sessionViewModel.initializeEngineAndBlankDocument(initialTitle, hideStartBackstage = true)
                if (template != "blank") {
                    sessionViewModel.loadStarterTemplate(template)
                }
                val borderSlug = incoming.getStringExtra("vexio.border")
                if (!borderSlug.isNullOrBlank()) {
                    sessionViewModel.applyBorderAndTitle(borderSlug)
                }
                val stencilCat = incoming.getStringExtra("vexio.stencil_cat")
                if (!stencilCat.isNullOrBlank()) {
                    sessionViewModel.selectStencilCategory(stencilCat)
                }
                if (incoming.getBooleanExtra("vexio.shapesheet", false) &&
                    !sessionViewModel.uiState.value.isShapeSheetDrawerExpanded
                ) {
                    sessionViewModel.toggleShapeSheetDrawer()
                }
                if (incoming.getBooleanExtra("vexio.dark", false) &&
                    !sessionViewModel.uiState.value.isDarkTheme
                ) {
                    sessionViewModel.toggleDarkTheme()
                }
            }
        }
    }

    private fun triggerImportStencilSafPicker() {
        importStencilLauncher.launch(
            arrayOf(
                "application/vnd.ms-visio.stencil.main+xml",
                "application/vnd.ms-visio.stencil.macroEnabled.main+xml",
                "application/vnd.ms-visio.drawing.main+xml",
                "application/vnd.visio",
                "application/zip",
                "image/svg+xml",
                "image/png",
                "application/octet-stream",
                "*/*"
            )
        )
    }

    private fun loadVisioStencilFromSafUri(uri: Uri) {
        lifecycleScope.launch(Dispatchers.IO) {
            val resolvedName = queryDisplayName(uri)
                ?: uri.lastPathSegment?.substringAfterLast('/')
                ?: "Imported_Stencil.vssx"
            val bytes = runCatching {
                contentResolver.openInputStream(uri)?.use { it.readBytes() }
            }.getOrNull() ?: return@launch

            withContext(Dispatchers.Main) {
                sessionViewModel.importVisioStencilBytes(
                    fileName = resolvedName,
                    rawBytes = bytes
                )
            }
        }
    }

    private fun triggerOpenSafPicker() {
        openDocumentLauncher.launch(
            arrayOf(
                "application/vnd.ms-visio.drawing.main+xml",
                "application/vnd.ms-visio.drawing.macroEnabled.main+xml",
                "application/vnd.ms-visio.stencil.main+xml",
                "application/vnd.ms-visio.template.main+xml",
                "application/vnd.visio",
                "application/acad",
                "image/vnd.dwg",
                "image/vnd.dxf",
                "application/dxf",
                "application/octet-stream",
                "*/*"
            )
        )
    }

    private fun triggerSaveInPlaceOrPicker() {
        val currentUri = sessionViewModel.uiState.value.documentUri
        if (currentUri != null && currentUri.scheme == "content") {
            writeDiagramToSafUri(currentUri)
        } else {
            triggerSaveAsSafPicker()
        }
    }

    private fun triggerSaveAsSafPicker() {
        val currentTitle = sessionViewModel.uiState.value.documentTitle
        val suggestedName = if (currentTitle.contains('.')) {
            currentTitle
        } else {
            "$currentTitle.vsdx"
        }
        createDocumentLauncher.launch(suggestedName)
    }

    private fun triggerExportJpgSafPicker() {
        val stem = sessionViewModel.uiState.value.documentTitle.substringBeforeLast('.')
            .ifBlank { "Diagram_Export" }
        exportJpgLauncher.launch("$stem.jpg")
    }

    private fun triggerExportFormatSafPicker(rawFormat: String) {
        val clean = rawFormat.trim().trimStart('.').lowercase()
        if (clean == "jpg" || clean == "jpeg") {
            triggerExportJpgSafPicker()
            return
        }
        pendingExportFormat = clean
        if (clean == "vssx") {
            exportFormatLauncher.launch("My_Shapes.vssx")
            return
        }
        val fileSuffix = when (clean) {
            "powerautomate" -> "flow.json"
            "powerbi" -> "pbi.json"
            else -> clean
        }
        val stem = sessionViewModel.uiState.value.documentTitle.substringBeforeLast('.')
            .ifBlank { "Diagram_Export" }
        exportFormatLauncher.launch("$stem.$fileSuffix")
    }

    private fun writeExportFormatToSafUri(uri: Uri, ext: String) {
        lifecycleScope.launch(Dispatchers.IO) {
            val payload = sessionViewModel.exportPlan2Artifact(ext)
            runCatching {
                contentResolver.openOutputStream(uri, "wt")?.use { s ->
                    s.write(payload)
                    s.flush()
                } ?: contentResolver.openOutputStream(uri)?.use { s ->
                    s.write(payload)
                    s.flush()
                }
            }
        }
    }

    fun openIndependentDocumentWindow(template: String = "blank") {
        launchNewDocumentWindow(this, template = template)
    }

    private fun takePersistableReadWritePermissionsIfPossible(uri: Uri) {
        val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        try {
            contentResolver.takePersistableUriPermission(uri, flags)
        } catch (_: SecurityException) {
            try {
                contentResolver.takePersistableUriPermission(
                    uri,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION
                )
            } catch (_: SecurityException) {
                // Transient one-shot URI grants (e.g., cross-window drag & drop) do not support persistable flags
            }
        }
    }

    private fun loadDiagramFromSafUri(uri: Uri) {
        lifecycleScope.launch(Dispatchers.IO) {
            val resolvedName = queryDisplayName(uri)
                ?: uri.lastPathSegment?.substringAfterLast('/')
                ?: "Imported_Diagram.vsdx"
            val bytes = runCatching {
                contentResolver.openInputStream(uri)?.use { it.readBytes() }
            }.getOrNull() ?: return@launch

            val ext = resolvedName.substringAfterLast('.', "").lowercase()
            withContext(Dispatchers.Main) {
                if (ext in setOf("xlsx", "csv", "tsv", "sqlite", "db", "sqlite3", "json")) {
                    sessionViewModel.importExternalRecordsetBytes(
                        fileName = resolvedName,
                        rawBytes = bytes,
                        uri = uri
                    )
                } else {
                    sessionViewModel.openDocumentFromBytes(
                        displayName = resolvedName,
                        uri = uri,
                        rawBytes = bytes
                    )
                }
            }
        }
    }

    private fun triggerLinkExternalDataSafPicker() {
        importExternalDataLauncher.launch(
            arrayOf(
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                "text/csv",
                "text/comma-separated-values",
                "text/tab-separated-values",
                "application/json",
                "application/vnd.sqlite3",
                "application/x-sqlite3",
                "application/octet-stream",
                "*/*"
            )
        )
    }

    private fun loadExternalDataFromSafUri(uri: Uri) {
        lifecycleScope.launch(Dispatchers.IO) {
            val resolvedName = queryDisplayName(uri)
                ?: uri.lastPathSegment?.substringAfterLast('/')
                ?: "External_Data.xlsx"
            val bytes = runCatching {
                contentResolver.openInputStream(uri)?.use { it.readBytes() }
            }.getOrNull() ?: return@launch

            withContext(Dispatchers.Main) {
                sessionViewModel.importExternalRecordsetBytes(
                    fileName = resolvedName,
                    rawBytes = bytes,
                    uri = uri
                )
            }
        }
    }

    private fun triggerRefreshExternalDataFromDisk() {
        val urisByRs = sessionViewModel.getLinkedRecordsetUrisSnapshot()
        if (urisByRs.isEmpty()) {
            sessionViewModel.refreshExternalRecordsets()
            return
        }
        lifecycleScope.launch(Dispatchers.IO) {
            val loaded = linkedMapOf<Int, Pair<String, ByteArray>>()
            for ((rsId, uri) in urisByRs) {
                val name = queryDisplayName(uri).orEmpty()
                val bytes = runCatching {
                    contentResolver.openInputStream(uri)?.use { it.readBytes() }
                }.getOrNull()
                if (bytes != null && bytes.isNotEmpty()) {
                    loaded[rsId] = name to bytes
                }
            }
            withContext(Dispatchers.Main) {
                if (loaded.isEmpty()) {
                    sessionViewModel.refreshExternalRecordsets()
                } else {
                    for ((rsId, pair) in loaded) {
                        sessionViewModel.refreshExternalRecordsets(
                            recordsetId = rsId,
                            optionalBytes = pair.second,
                            formatHint = pair.first
                        )
                    }
                }
            }
        }
    }

    private fun triggerTwoWayWritebackSafPicker(recordsetId: Int) {
        val state = sessionViewModel.uiState.value
        val matched = state.externalRecordsets.firstOrNull { recordsetId == 0 || it.id == recordsetId }
        val ext = matched?.sourceKind?.ifBlank { "xlsx" } ?: "xlsx"
        val base = matched?.name?.ifBlank { state.documentTitle.substringBeforeLast('.') }
            ?: "${state.documentTitle.substringBeforeLast('.')}_Synced"
        pendingWritebackRecordsetId = recordsetId
        pendingWritebackFormatExt = ext
        exportWritebackLauncher.launch("$base.$ext")
    }

    private fun writeTwoWayWritebackToSafUri(uri: Uri, recordsetId: Int, defaultExt: String) {
        lifecycleScope.launch(Dispatchers.IO) {
            val resolvedName = queryDisplayName(uri).orEmpty()
            val ext = resolvedName.substringAfterLast('.', defaultExt).ifBlank { defaultExt }.lowercase()
            val bytes = sessionViewModel.syncAndExportRecordsetBytes(recordsetId, ext)
            if (bytes.isEmpty()) return@launch
            runCatching {
                contentResolver.openOutputStream(uri, "wt")?.use { s ->
                    s.write(bytes)
                    s.flush()
                } ?: contentResolver.openOutputStream(uri)?.use { s ->
                    s.write(bytes)
                    s.flush()
                }
            }
        }
    }

    private fun triggerExportBomReportSafPicker() {
        val stem = sessionViewModel.uiState.value.documentTitle.substringBeforeLast('.')
            .ifBlank { "Diagram" }
        exportBomReportLauncher.launch("${stem}_BOM_Report.xlsx")
    }

    private fun writeBomReportToSafUri(uri: Uri) {
        lifecycleScope.launch(Dispatchers.IO) {
            val resolvedName = queryDisplayName(uri).orEmpty()
            val ext = resolvedName.substringAfterLast('.', "xlsx").ifBlank { "xlsx" }.lowercase()
            val bytes = sessionViewModel.exportBomReportBytes(ext)
            if (bytes.isEmpty()) return@launch
            runCatching {
                contentResolver.openOutputStream(uri, "wt")?.use { s ->
                    s.write(bytes)
                    s.flush()
                } ?: contentResolver.openOutputStream(uri)?.use { s ->
                    s.write(bytes)
                    s.flush()
                }
            }
        }
    }

    private fun writeDiagramToSafUri(uri: Uri) {
        val stateSnapshot = sessionViewModel.uiState.value
        lifecycleScope.launch(Dispatchers.IO) {
            val resolvedName = queryDisplayName(uri) ?: stateSnapshot.documentTitle
            val ext = resolvedName.substringAfterLast('.', stateSnapshot.documentExtension)
                .lowercase()
            val serializedBytes = sessionViewModel.exportDocumentBytes(ext)

            val writeSucceeded = runCatching {
                contentResolver.openOutputStream(uri, "wt")?.use { stream ->
                    stream.write(serializedBytes)
                    stream.flush()
                } ?: contentResolver.openOutputStream(uri)?.use { stream ->
                    stream.write(serializedBytes)
                    stream.flush()
                }
            }.isSuccess

            if (writeSucceeded) {
                withContext(Dispatchers.Main) {
                    sessionViewModel.markSaved(
                        uri = uri,
                        displayName = resolvedName,
                        ext = ext
                    )
                    if (finishAfterNextSave) {
                        finishAfterNextSave = false
                        finish()
                    }
                }
            } else {
                finishAfterNextSave = false
            }
        }
    }

    private fun queryDisplayName(uri: Uri): String? {
        if (uri.scheme != "content") {
            return uri.lastPathSegment
        }
        return runCatching {
            contentResolver.query(
                uri,
                arrayOf(OpenableColumns.DISPLAY_NAME),
                null,
                null,
                null
            )?.use { cursor ->
                val nameIndex = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                if (nameIndex >= 0 && cursor.moveToFirst()) {
                    cursor.getString(nameIndex)
                } else {
                    null
                }
            }
        }.getOrNull()
    }
}
