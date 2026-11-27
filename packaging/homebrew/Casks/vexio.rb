cask "vexio" do
  version "0.9.2-beta"
  sha256 "3007681e93ec9ffed301a4bfb2936ef95bf26cbbf8f23638defdcceb0de3df9c"

  url "https://github.com/petrosvassiliou/vexio/releases/download/v#{version}/Vexio-v#{version}-Universal2.dmg"
  name "Vexio"
  desc "1:1 Microsoft Visio Professional compatible diagramming, CAD & ShapeSheet suite"
  homepage "https://github.com/petrosvassiliou/vexio"

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
