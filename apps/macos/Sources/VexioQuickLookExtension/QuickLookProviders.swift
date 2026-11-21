import CoreGraphics
import Foundation
import QuickLookThumbnailing
import QuickLookUI
import UniformTypeIdentifiers
import VexioCoreFFI

// MARK: - Quick Look Error Diagnostics

public enum VexioQuickLookError: LocalizedError, Sendable {
    case unreadableFile(URL)
    case unsupportedExtension(String)
    case engineInitializationFailed
    case openFailed(statusCode: UInt32, ext: String)
    case renderFailed(statusCode: UInt32)
    case cgImageCreationFailed

    public var errorDescription: String? {
        switch self {
        case .unreadableFile(let url):
            return "Quick Look could not read local file bytes for '\(url.lastPathComponent)'."
        case .unsupportedExtension(let ext):
            return "Unsupported diagram extension '.\(ext)' for Vexio Quick Look."
        case .engineInitializationFailed:
            return "Failed to initialize offline Vexio Rust engine in Quick Look extension."
        case .openFailed(let code, let ext):
            return "vexio_doc_open_bytes failed with status \(code) for '.\(ext)'."
        case .renderFailed(let code):
            return "vexio_render_preview_rgba failed with status \(code)."
        case .cgImageCreationFailed:
            return "Failed to construct sRGB CGImage from rendered RGBA8 buffer."
        }
    }
}

// MARK: - Deterministic Offline C-ABI Renderer (Zero Network, Pure CoreGraphics + Rust FFI)

enum VexioOfflineQuickLookRenderer {
    static let supportedExtensions: Set<String> = [
        "vsdx", "vsdm", "vssx", "vstx", "vssm", "vstm",
        "vsd", "vss", "vst", "vdx", "dwg", "dxf",
    ]

    static func canonicalExtension(for fileURL: URL) throws -> String {
        let ext = fileURL.pathExtension.lowercased()
        if supportedExtensions.contains(ext) {
            return ext
        }
        if ext.isEmpty {
            return "vsdx"
        }
        throw VexioQuickLookError.unsupportedExtension(ext)
    }

    static func renderFileToCGImage(
        at fileURL: URL,
        pageId: UInt32 = 0,
        widthPx: UInt32,
        heightPx: UInt32
    ) throws -> CGImage {
        let ext = try canonicalExtension(for: fileURL)

        let fileBytes: Data
        do {
            fileBytes = try Data(contentsOf: fileURL, options: [.mappedIfSafe])
        } catch {
            throw VexioQuickLookError.unreadableFile(fileURL)
        }
        guard !fileBytes.isEmpty else {
            throw VexioQuickLookError.unreadableFile(fileURL)
        }

        guard let engine = vexio_engine_create() else {
            throw VexioQuickLookError.engineInitializationFailed
        }
        defer { vexio_engine_destroy(engine) }

        var openedDoc: OpaquePointer?
        let openStatus: VexioStatus = fileBytes.withUnsafeBytes { rawBuf in
            let baseAddress = rawBuf.bindMemory(to: UInt8.self).baseAddress
            let byteSlice = VexioByteSlice(ptr: baseAddress, len: rawBuf.count)
            return ext.withCString { extCStr in
                vexio_doc_open_bytes(engine, byteSlice, extCStr, &openedDoc)
            }
        }
        guard openStatus == VEXIO_OK, let doc = openedDoc else {
            if let partialDoc = openedDoc {
                vexio_doc_destroy(partialDoc)
            }
            throw VexioQuickLookError.openFailed(statusCode: openStatus.rawValue, ext: ext)
        }
        defer { vexio_doc_destroy(doc) }

        let clampedWidth = min(max(widthPx, 64), 4096)
        let clampedHeight = min(max(heightPx, 64), 4096)

        var rgbaOwned = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
        var renderStatus = vexio_render_preview_rgba(doc, pageId, clampedWidth, clampedHeight, &rgbaOwned)
        if renderStatus != VEXIO_OK && pageId == 0 {
            vexio_buffer_free(rgbaOwned)
            rgbaOwned = VexioOwnedBuffer(ptr: nil, len: 0, cap: 0)
            renderStatus = vexio_render_preview_rgba(doc, 1, clampedWidth, clampedHeight, &rgbaOwned)
        }
        defer { vexio_buffer_free(rgbaOwned) }

        guard renderStatus == VEXIO_OK,
              let rgbaPtr = rgbaOwned.ptr,
              rgbaOwned.len >= Int(clampedWidth) * Int(clampedHeight) * 4
        else {
            throw VexioQuickLookError.renderFailed(statusCode: renderStatus.rawValue)
        }

        // Copy owned Rust buffer into managed CFData before vexio_buffer_free runs on scope exit.
        let pixelData = Data(bytes: rgbaPtr, count: rgbaOwned.len)
        guard let sRGB = CGColorSpace(name: CGColorSpace.sRGB),
              let provider = CGDataProvider(data: pixelData as CFData),
              let cgImage = CGImage(
                  width: Int(clampedWidth),
                  height: Int(clampedHeight),
                  bitsPerComponent: 8,
                  bitsPerPixel: 32,
                  bytesPerRow: Int(clampedWidth) * 4,
                  space: sRGB,
                  bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
                  provider: provider,
                  decode: nil,
                  shouldInterpolate: true,
                  intent: .defaultIntent
              )
        else {
            throw VexioQuickLookError.cgImageCreationFailed
        }

        return cgImage
    }

    static func drawAspectFittedPage(
        image: CGImage,
        in context: CGContext,
        canvasSize: CGSize
    ) {
        let fullRect = CGRect(origin: .zero, size: canvasSize)

        // Crisp paper background so transparent diagram regions remain legible in Dark & Light Finder views
        context.saveGState()
        context.setFillColor(CGColor(red: 1.0, green: 1.0, blue: 1.0, alpha: 1.0))
        context.fill(fullRect)

        let imageWidth = CGFloat(max(image.width, 1))
        let imageHeight = CGFloat(max(image.height, 1))
        let scale = min(canvasSize.width / imageWidth, canvasSize.height / imageHeight)
        let fittedSize = CGSize(width: imageWidth * scale, height: imageHeight * scale)
        let fittedOrigin = CGPoint(
            x: (canvasSize.width - fittedSize.width) * 0.5,
            y: (canvasSize.height - fittedSize.height) * 0.5
        )

        context.interpolationQuality = .high
        context.draw(image, in: CGRect(origin: fittedOrigin, size: fittedSize))
        context.restoreGState()
    }
}

// MARK: - Finder & Spotlight Thumbnail Provider (.vsdx, .vssx, .vstx, .vsd)

public final class VexioThumbnailProvider: QLThumbnailProvider {
    override public init() {
        super.init()
    }

    override public func provideThumbnail(
        for request: QLFileThumbnailRequest,
        _ handler: @escaping (QLThumbnailReply?, (any Error)?) -> Void
    ) {
        let maxPointSize = request.maximumSize
        let scale = max(request.scale, 1.0)
        let targetWidthPx = UInt32(max((maxPointSize.width * scale).rounded(.up), 64))
        let targetHeightPx = UInt32(max((maxPointSize.height * scale).rounded(.up), 64))

        do {
            let renderedImage = try VexioOfflineQuickLookRenderer.renderFileToCGImage(
                at: request.fileURL,
                pageId: 0,
                widthPx: targetWidthPx,
                heightPx: targetHeightPx
            )

            let reply = QLThumbnailReply(contextSize: maxPointSize) { cgContext in
                VexioOfflineQuickLookRenderer.drawAspectFittedPage(
                    image: renderedImage,
                    in: cgContext,
                    canvasSize: maxPointSize
                )
                return true
            }
            handler(reply, nil)
        } catch {
            handler(nil, error)
        }
    }
}

// MARK: - Spacebar Full-Resolution Offline Quick Look Preview Provider (.vsdx, .vssx, .vstx, .vsd)

public final class VexioPreviewProvider: QLPreviewProvider, QLPreviewingController {
    override public init() {
        super.init()
    }

    public func providePreview(
        for request: QLFilePreviewRequest,
        completionHandler handler: @escaping (QLPreviewReply?, (any Error)?) -> Void
    ) {
        let fileURL = request.fileURL
        // Letter/A4 landscape-proportioned full-resolution preview canvas (Retina 2x -> 2560x1810 RGBA)
        let previewContextSize = CGSize(width: 1280, height: 905)
        let retinaWidthPx: UInt32 = 2560
        let retinaHeightPx: UInt32 = 1810

        let reply = QLPreviewReply(
            contextSize: previewContextSize,
            isBitmap: true
        ) { cgContext, previewReply in
            previewReply.title = fileURL.lastPathComponent
            let renderedImage = try VexioOfflineQuickLookRenderer.renderFileToCGImage(
                at: fileURL,
                pageId: 0,
                widthPx: retinaWidthPx,
                heightPx: retinaHeightPx
            )
            VexioOfflineQuickLookRenderer.drawAspectFittedPage(
                image: renderedImage,
                in: cgContext,
                canvasSize: previewContextSize
            )
        }
        reply.title = fileURL.lastPathComponent
        handler(reply, nil)
    }
}
