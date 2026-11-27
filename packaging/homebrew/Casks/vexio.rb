cask "vexio" do
  version "0.9.5-beta"
  sha256 "89c1e44244acf5c29bb3d661245b1dbe199c5dbf928e81345f3f46349f164c9e"

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
