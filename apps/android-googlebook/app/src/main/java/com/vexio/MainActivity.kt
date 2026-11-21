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
    private val createDocumentLauncher = registerForActivityResult(
        ActivityResultContracts.CreateDocument("application/vnd.ms-visio.drawing.main+xml")
    ) { targetUri: Uri? ->
        if (targetUri != null) {
            takePersistableReadWritePermissionsIfPossible(targetUri)
            writeDiagramToSafUri(targetUri)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)

        if (savedInstanceState == null) {
            handleIncomingLaunchIntent(intent)
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
                    onExternalUriDropped = ::loadDiagramFromSafUri,
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
                val initialTitle = when (template) {
                    "bpmn2" -> "BPMN2_Process_Flow.vsdx"
                    "cad_floorplan" -> "Architectural_Floorplan.vsdx"
                    else -> "Untitled_Diagram.vsdx"
                }
                sessionViewModel.initializeEngineAndBlankDocument(initialTitle)
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

            withContext(Dispatchers.Main) {
                sessionViewModel.openDocumentFromBytes(
                    displayName = resolvedName,
                    uri = uri,
                    rawBytes = bytes
                )
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
                }
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
