#!/usr/bin/env bash
# Automatically fetches published GitHub Release SHA-256 digests for Vexio installers
# and stamps them into Homebrew (`packaging/homebrew/Casks/vexio.rb`) and WinGet manifests (`packaging/winget/*.yaml`).
set -euo pipefail

TAG="${1:-v0.5.0-beta}"
VERSION="${TAG#v}"
REPO="petrosvassiliou/vexio"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

echo "Fetching release assets for ${REPO} @ ${TAG}..."
gh release download "$TAG" --repo "$REPO" \
  --pattern "Vexio-${TAG}-Universal2.dmg" \
  --pattern "Vexio-Windows-${TAG}-x64.msix" \
  --pattern "Vexio-Windows-${TAG}-arm64.msix" \
  --dir "$TMP_DIR"

MAC_SHA="$(shasum -a 256 "$TMP_DIR/Vexio-${TAG}-Universal2.dmg" | awk '{print $1}')"
WIN_X64_SHA="$(shasum -a 256 "$TMP_DIR/Vexio-Windows-${TAG}-x64.msix" | awk '{print $1}' | tr '[:lower:]' '[:upper:]')"
WIN_ARM64_SHA="$(shasum -a 256 "$TMP_DIR/Vexio-Windows-${TAG}-arm64.msix" | awk '{print $1}' | tr '[:lower:]' '[:upper:]')"

cat > packaging/homebrew/Casks/vexio.rb <<EOF
cask "vexio" do
  version "${VERSION}"
  sha256 "${MAC_SHA}"

  url "https://github.com/${REPO}/releases/download/v#{version}/Vexio-v#{version}-Universal2.dmg"
  name "Vexio"
  desc "1:1 Microsoft Visio Professional compatible diagramming, CAD & ShapeSheet suite"
  homepage "https://github.com/${REPO}"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :sequoia"

  app "Vexio.app"

  zap trash: [
    "~/Library/Application Support/app.vexio.macos",
    "~/Library/Caches/app.vexio.macos",
    "~/Library/Preferences/app.vexio.macos.plist",
    "~/Library/Saved Application State/app.vexio.macos.savedState",
  ]
end
EOF

sed -i.bak "s/^PackageVersion: .*/PackageVersion: ${VERSION}/" \
  packaging/winget/PetrosVassiliou.Vexio.yaml \
  packaging/winget/PetrosVassiliou.Vexio.locale.en-US.yaml
rm -f packaging/winget/*.bak

cat > packaging/winget/PetrosVassiliou.Vexio.installer.yaml <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.1.6.0.schema.json
PackageIdentifier: PetrosVassiliou.Vexio
PackageVersion: ${VERSION}
Platform:
  - Windows.Desktop
MinimumOSVersion: 10.0.19041.0
InstallerType: msix
Scope: user
InstallModes:
  - interactive
  - silent
UpgradeBehavior: install
FileExtensions:
  - vsdx
  - vsdm
  - vssx
  - vssm
  - vstx
  - vstm
  - vsd
  - vss
  - vst
  - vdx
  - vdw
  - dwg
  - dxf
  - mmd
  - puml
  - dot
Installers:
  - Architecture: x64
    InstallerUrl: https://github.com/${REPO}/releases/download/v${VERSION}/Vexio-Windows-v${VERSION}-x64.msix
    InstallerSha256: ${WIN_X64_SHA}
  - Architecture: arm64
    InstallerUrl: https://github.com/${REPO}/releases/download/v${VERSION}/Vexio-Windows-v${VERSION}-arm64.msix
    InstallerSha256: ${WIN_ARM64_SHA}
ManifestType: installer
ManifestVersion: 1.6.0
EOF

echo "Updated Homebrew & WinGet manifests for ${TAG}:"
echo "  macOS DMG SHA256:    ${MAC_SHA}"
echo "  Win x64 MSIX SHA256: ${WIN_X64_SHA}"
echo "  Win ARM MSIX SHA256: ${WIN_ARM64_SHA}"
