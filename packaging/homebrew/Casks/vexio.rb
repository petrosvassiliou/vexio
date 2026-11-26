cask "vexio" do
  version "0.5.0-beta"
  sha256 "c0c1789c0efa9bb4b3a1a678e94ad5890b617dc9eca565a7cf204c2dc89125ba"

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
