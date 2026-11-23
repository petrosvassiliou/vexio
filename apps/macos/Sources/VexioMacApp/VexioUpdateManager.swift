import AppKit
import Foundation
import SwiftUI

// MARK: - Automatic & Manual GitHub Releases Updater for macOS

public struct GitHubReleaseAssetPayload: Decodable, Sendable {
    public let name: String
    public let browser_download_url: String
}

public struct GitHubReleasePayload: Decodable, Sendable {
    public let tag_name: String
    public let name: String?
    public let body: String?
    public let html_url: String
    public let prerelease: Bool
    public let assets: [GitHubReleaseAssetPayload]
}

@MainActor
public final class VexioUpdateManager: ObservableObject {
    public static let shared = VexioUpdateManager()
    public static let currentVersion = "0.3.5-alpha"
    private static let releasesEndpoint = "https://api.github.com/repos/petrosvassiliou/vexio/releases"

    @Published public var autoCheckEnabled: Bool {
        didSet {
            UserDefaults.standard.set(autoCheckEnabled, forKey: "vexio.autoUpdateCheck")
        }
    }
    @Published public var isChecking: Bool = false
    @Published public var isDownloading: Bool = false
    @Published public var updateAvailable: Bool = false
    @Published public var latestVersionTag: String = currentVersion
    @Published public var releaseNotes: String = ""
    @Published public var downloadURL: URL?
    @Published public var releasePageURL: URL? = URL(string: "https://github.com/petrosvassiliou/vexio/releases")
    @Published public var statusSummary: String = "Up to date (v\(currentVersion))"
    @Published public var isShowingUpdateDialog: Bool = false

    private var hasPerformedLaunchCheck: Bool = false

    public init() {
        if UserDefaults.standard.object(forKey: "vexio.autoUpdateCheck") == nil {
            self.autoCheckEnabled = true
        } else {
            self.autoCheckEnabled = UserDefaults.standard.bool(forKey: "vexio.autoUpdateCheck")
        }
    }

    public func checkOnLaunchIfEnabled() {
        guard !hasPerformedLaunchCheck else { return }
        hasPerformedLaunchCheck = true
        guard autoCheckEnabled else { return }
        Task {
            await checkForUpdates(userInitiated: false)
        }
    }

    public func checkForUpdates(userInitiated: Bool) async {
        if userInitiated {
            isShowingUpdateDialog = true
        }
        guard !isChecking else { return }
        isChecking = true
        statusSummary = "Checking GitHub Releases…"
        defer { isChecking = false }

        guard let url = URL(string: Self.releasesEndpoint) else { return }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 8.0)
        req.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        req.setValue("Vexio-macOS/\(Self.currentVersion)", forHTTPHeaderField: "User-Agent")

        do {
            let (data, response) = try await URLSession.shared.data(for: req)
            guard let http = response as? HTTPURLResponse, (200...299).contains(http.statusCode) else {
                statusSummary = "Unable to reach update server (offline mode active)."
                return
            }
            let releases = try JSONDecoder().decode([GitHubReleasePayload].self, from: data)
            guard let newest = releases.first else {
                statusSummary = "You are running the latest version (v\(Self.currentVersion))."
                return
            }

            let remoteTag = newest.tag_name.trimmingCharacters(in: CharacterSet(charactersIn: "vV "))
            latestVersionTag = remoteTag
            releaseNotes = newest.body ?? "Performance, stencil fidelity, and Ribbon enhancements."
            releasePageURL = URL(string: newest.html_url)

            #if arch(arm64)
            let preferredArchToken = "macos-arm64"
            #else
            let preferredArchToken = "macos-x86_64"
            #endif

            if let matchedAsset = newest.assets.first(where: {
                $0.name.lowercased().contains(preferredArchToken) && $0.name.lowercased().hasSuffix(".dmg")
            }) ?? newest.assets.first(where: { $0.name.lowercased().hasSuffix(".dmg") }) {
                downloadURL = URL(string: matchedAsset.browser_download_url)
            }

            if Self.isNewerVersion(remote: remoteTag, current: Self.currentVersion) {
                updateAvailable = true
                statusSummary = "Update available: v\(remoteTag)"
                isShowingUpdateDialog = true
            } else {
                updateAvailable = false
                statusSummary = "Vexio v\(Self.currentVersion) is up to date."
            }
        } catch {
            statusSummary = "Offline or network unavailable — Vexio continues 100% offline."
        }
    }

    public func downloadAndOpenInstaller() {
        guard let targetURL = downloadURL ?? releasePageURL else { return }
        if targetURL.pathExtension.lowercased() != "dmg" {
            NSWorkspace.shared.open(targetURL)
            return
        }
        isDownloading = true
        statusSummary = "Downloading \(targetURL.lastPathComponent)…"
        Task {
            defer { isDownloading = false }
            do {
                let (tempURL, _) = try await URLSession.shared.download(from: targetURL)
                let downloadsDir = FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask).first
                    ?? FileManager.default.temporaryDirectory
                let destURL = downloadsDir.appendingPathComponent(targetURL.lastPathComponent)
                try? FileManager.default.removeItem(at: destURL)
                try FileManager.default.moveItem(at: tempURL, to: destURL)
                statusSummary = "Downloaded \(destURL.lastPathComponent) — opening installer…"
                NSWorkspace.shared.open(destURL)
            } catch {
                statusSummary = "Opening release page in browser…"
                if let page = releasePageURL {
                    NSWorkspace.shared.open(page)
                }
            }
        }
    }

    public static func isNewerVersion(remote: String, current: String) -> Bool {
        func parse(_ raw: String) -> ([Int], String) {
            let cleaned = raw.trimmingCharacters(in: CharacterSet(charactersIn: "vV "))
            let parts = cleaned.split(separator: "-", maxSplits: 1).map(String.init)
            let nums = (parts.first ?? "0").split(separator: ".").map { Int($0) ?? 0 }
            let pre = parts.count > 1 ? parts[1] : ""
            return (nums, pre)
        }
        let (rNums, rPre) = parse(remote)
        let (cNums, cPre) = parse(current)
        let count = max(rNums.count, cNums.count)
        for i in 0..<count {
            let rv = i < rNums.count ? rNums[i] : 0
            let cv = i < cNums.count ? cNums[i] : 0
            if rv != cv { return rv > cv }
        }
        if rPre.isEmpty && !cPre.isEmpty { return true }
        if !rPre.isEmpty && cPre.isEmpty { return false }
        return rPre.localizedStandardCompare(cPre) == .orderedDescending
    }
}

public struct VexioUpdateDialogSheet: View {
    @ObservedObject var updater: VexioUpdateManager
    @ObservedObject var i18n: VexioLocalization

    public init(updater: VexioUpdateManager = .shared, i18n: VexioLocalization = .shared) {
        self.updater = updater
        self.i18n = i18n
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 12) {
                Image(systemName: updater.updateAvailable ? "arrow.down.circle.fill" : "checkmark.seal.fill")
                    .font(.system(size: 28))
                    .foregroundStyle(updater.updateAvailable ? Color.accentColor : Color.green)
                VStack(alignment: .leading, spacing: 2) {
                    Text(i18n.t("checkForUpdates"))
                        .font(.system(size: 16, weight: .bold))
                    Text("Installed: v\(VexioUpdateManager.currentVersion) • Latest: v\(updater.latestVersionTag)")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }
                Spacer()
            }

            Text(updater.statusSummary)
                .font(.system(size: 12.5, weight: .medium))

            if !updater.releaseNotes.isEmpty {
                ScrollView {
                    Text(updater.releaseNotes)
                        .font(.system(size: 11.5))
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(8)
                }
                .frame(height: 120)
                .background(RoundedRectangle(cornerRadius: 8).fill(Color.primary.opacity(0.05)))
            }

            Toggle(i18n.t("autoUpdates"), isOn: $updater.autoCheckEnabled)
                .font(.system(size: 12))

            HStack {
                Button("Check Again") {
                    Task { await updater.checkForUpdates(userInitiated: true) }
                }
                .disabled(updater.isChecking || updater.isDownloading)

                Spacer()

                Button(i18n.t("done")) {
                    updater.isShowingUpdateDialog = false
                }

                if updater.updateAvailable || updater.downloadURL != nil {
                    Button(updater.updateAvailable ? "Download & Install v\(updater.latestVersionTag)" : "Re-download DMG") {
                        updater.downloadAndOpenInstaller()
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(updater.isDownloading)
                }
            }
        }
        .padding(20)
        .frame(width: 460)
    }
}
