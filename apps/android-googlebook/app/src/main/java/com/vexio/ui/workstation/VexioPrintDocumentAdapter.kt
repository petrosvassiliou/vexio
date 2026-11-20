package com.vexio.ui.workstation

import android.content.Context
import android.os.Bundle
import android.os.CancellationSignal
import android.os.ParcelFileDescriptor
import android.print.PageRange
import android.print.PrintAttributes
import android.print.PrintDocumentAdapter
import android.print.PrintDocumentInfo
import android.print.PrintManager
import com.vexio.core.VexioSessionViewModel
import java.io.FileOutputStream
import java.io.IOException

/**
 * Native Android / Googlebook OS `PrintDocumentAdapter` streaming losslessly exported vector PDF
 * bytes (`NativeBridge.nativeSaveDocument(docPtr, "pdf")`) directly into the OS `PrintManager` spooler.
 */
class VexioVectorPdfPrintAdapter(
    private val documentTitle: String,
    private val pageCountProvider: () -> Int,
    private val pdfBytesExporter: () -> ByteArray
) : PrintDocumentAdapter() {

    private var cachedPdfBytes: ByteArray? = null

    override fun onLayout(
        oldAttributes: PrintAttributes?,
        newAttributes: PrintAttributes,
        cancellationSignal: CancellationSignal?,
        callback: LayoutResultCallback,
        extras: Bundle?
    ) {
        if (cancellationSignal?.isCanceled == true) {
            callback.onLayoutCancelled()
            return
        }

        val attributesChanged = oldAttributes != newAttributes
        if (cachedPdfBytes == null || attributesChanged) {
            cachedPdfBytes = pdfBytesExporter()
        }

        val sanitizedFileName = buildString {
            val base = documentTitle.substringBeforeLast('.', documentTitle)
                .ifBlank { "Vexio_Diagram" }
            append(base)
            append(".pdf")
        }

        val totalPages = pageCountProvider().coerceAtLeast(1)
        val info = PrintDocumentInfo.Builder(sanitizedFileName)
            .setContentType(PrintDocumentInfo.CONTENT_TYPE_DOCUMENT)
            .setPageCount(totalPages)
            .build()

        callback.onLayoutFinished(info, attributesChanged)
    }

    override fun onWrite(
        pages: Array<out PageRange>,
        destination: ParcelFileDescriptor,
        cancellationSignal: CancellationSignal?,
        callback: WriteResultCallback
    ) {
        if (cancellationSignal?.isCanceled == true) {
            callback.onWriteCancelled()
            return
        }

        val pdfPayload = cachedPdfBytes ?: pdfBytesExporter().also { cachedPdfBytes = it }
        try {
            FileOutputStream(destination.fileDescriptor).use { output ->
                val chunkSize = 16 * 1024
                var offset = 0
                while (offset < pdfPayload.size) {
                    if (cancellationSignal?.isCanceled == true) {
                        callback.onWriteCancelled()
                        return
                    }
                    val bytesToWrite = minOf(chunkSize, pdfPayload.size - offset)
                    output.write(pdfPayload, offset, bytesToWrite)
                    offset += bytesToWrite
                }
                output.flush()
            }
            callback.onWriteFinished(arrayOf(PageRange.ALL_PAGES))
        } catch (ioe: IOException) {
            callback.onWriteFailed(ioe.localizedMessage ?: "Failed writing vector PDF to PrintManager")
        }
    }

    override fun onFinish() {
        cachedPdfBytes = null
        super.onFinish()
    }
}

/**
 * Dispatches a vector-accurate print job to the native Android / Googlebook OS `PrintManager`.
 */
fun launchVexioVectorPrintJob(
    context: Context,
    viewModel: VexioSessionViewModel
) {
    val printManager = context.getSystemService(Context.PRINT_SERVICE) as? PrintManager ?: return
    val currentState = viewModel.uiState.value
    val jobName = "Vexio — ${currentState.documentTitle}"

    val attributes = PrintAttributes.Builder()
        .setMediaSize(PrintAttributes.MediaSize.NA_LETTER.asLandscape())
        .setColorMode(PrintAttributes.COLOR_MODE_COLOR)
        .setMinMargins(PrintAttributes.Margins.NO_MARGINS)
        .build()

    val adapter = VexioVectorPdfPrintAdapter(
        documentTitle = currentState.documentTitle,
        pageCountProvider = { viewModel.uiState.value.pages.size.coerceAtLeast(1) },
        pdfBytesExporter = { viewModel.exportDocumentBytes("pdf") }
    )

    printManager.print(jobName, adapter, attributes)
}
