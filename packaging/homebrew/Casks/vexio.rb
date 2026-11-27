cask "vexio" do
  version "0.8.0-beta"
  sha256 "337d2755a72b9e17c48b33e2c7b95332d44efd20074aed80c3818ea8ce2c3c8e"

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
